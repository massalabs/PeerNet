use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::config::{PeerNetCategories, PeerNetCategoryInfo, PeerNetFeatures};
use crate::context::Context;
use crate::error::{PeerNetError, PeerNetErrorData, PeerNetResult};
use crate::messages::MessagesHandler;
use crate::network_manager::{to_canonical, SharedActiveConnections};
use crate::peer::{new_peer, InitConnectionHandler, PeerConnectionType};
use crate::peer_id::PeerId;
use crate::transports::Endpoint;

use super::{Transport, TransportErrorType};

use crossbeam::channel::{unbounded, Receiver, Sender};
use crossbeam::sync::WaitGroup;
use mio::net::TcpListener as MioTcpListener;
use mio::{Events, Interest, Poll, Token, Waker};
use parking_lot::RwLock;
use stream_limiter::LimiterOptions;

#[derive(Debug, PartialEq, Eq)]
pub enum TcpError {
    InitListener,
    ConnectionError,
    StopListener,
}

impl TcpError {
    fn wrap(self) -> PeerNetError {
        PeerNetError::TransportError(TransportErrorType::Tcp(self))
    }
}

#[derive(Default, Debug, Clone)]
#[allow(dead_code)]
pub struct TcpTransportConfig {
    pub max_in_connections: usize,
    pub connection_config: TcpConnectionConfig,
    pub peer_categories: PeerNetCategories,
    pub default_category_info: PeerNetCategoryInfo,
    pub write_timeout: Duration,
    pub read_timeout: Duration,
}

pub(crate) struct TcpTransport<Id: PeerId> {
    pub active_connections: SharedActiveConnections<Id>,
    pub out_connection_attempts: WaitGroup,
    pub listeners: HashMap<SocketAddr, (Waker, JoinHandle<PeerNetResult<()>>)>,
    _features: PeerNetFeatures,

    peer_stop_tx: Sender<()>,
    peer_stop_rx: Receiver<()>,
    pub config: TcpTransportConfig,
    pub total_bytes_received: Arc<RwLock<u64>>,
    pub total_bytes_sent: Arc<RwLock<u64>>,
}

const NEW_CONNECTION: Token = Token(0);
const STOP_LISTENER: Token = Token(10);

#[derive(Clone, Debug)]
pub struct TcpConnectionConfig {
    pub rate_limit: u128,
    pub rate_time_window: Duration,
    pub rate_bucket_size: usize,
    pub data_channel_size: usize,
    pub max_message_size: usize,
    pub write_timeout: Duration,
    pub read_timeout: Duration,
}

impl From<TcpConnectionConfig> for LimiterOptions {
    fn from(val: TcpConnectionConfig) -> Self {
        LimiterOptions::new(val.rate_limit, val.rate_time_window, val.rate_bucket_size)
    }
}

impl Default for TcpConnectionConfig {
    fn default() -> Self {
        TcpConnectionConfig {
            rate_limit: 10 * 1024,
            rate_time_window: Duration::from_secs(1),
            rate_bucket_size: 10 * 1024,
            max_message_size: 100000,
            data_channel_size: 10000,
            write_timeout: Duration::from_secs(7),
            read_timeout: Duration::from_secs(7),
        }
    }
}

//TODO: IN/OUT different types because TCP ports are not reliable
pub struct TcpEndpoint {
    pub config: TcpConnectionConfig,
    pub address: SocketAddr,
    pub stream: TcpStream,
    // shared between all endpoints
    pub total_bytes_received: Arc<RwLock<u64>>,
    // shared between all endpoints
    pub total_bytes_sent: Arc<RwLock<u64>>,
    // received by this endpoint
    pub endpoint_bytes_received: Arc<RwLock<u64>>,
    // sent by this endpoint
    pub endpoint_bytes_sent: Arc<RwLock<u64>>,
}

impl TcpEndpoint {
    pub fn try_clone(&self) -> PeerNetResult<Self> {
        Ok(TcpEndpoint {
            address: self.address,
            stream: self.stream.try_clone().map_err(|err| {
                TcpError::ConnectionError
                    .wrap()
                    .new("cannot clone stream", err, None)
            })?,
            config: self.config.clone(),
            total_bytes_received: self.total_bytes_received.clone(),
            total_bytes_sent: self.total_bytes_sent.clone(),
            endpoint_bytes_received: self.endpoint_bytes_received.clone(),
            endpoint_bytes_sent: self.endpoint_bytes_sent.clone(),
        })
    }

    pub fn shutdown(&mut self) {
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }

    pub fn get_bytes_received(&self) -> u64 {
        *self.endpoint_bytes_received.read()
    }

    pub fn get_bytes_sent(&self) -> u64 {
        *self.endpoint_bytes_sent.read()
    }
}

impl<Id: PeerId> TcpTransport<Id> {
    pub fn new(
        active_connections: SharedActiveConnections<Id>,
        config: TcpTransportConfig,
        features: PeerNetFeatures,
        total_bytes_received: Arc<RwLock<u64>>,
        total_bytes_sent: Arc<RwLock<u64>>,
    ) -> TcpTransport<Id> {
        let (peer_stop_tx, peer_stop_rx) = unbounded();
        TcpTransport {
            active_connections,
            out_connection_attempts: WaitGroup::new(),
            listeners: Default::default(),
            _features: features,
            peer_stop_rx,
            peer_stop_tx,
            config,
            total_bytes_received,
            total_bytes_sent,
        }
    }
}

impl<Id: PeerId> Drop for TcpTransport<Id> {
    fn drop(&mut self) {
        let all_addresses: Vec<SocketAddr> = self.listeners.keys().cloned().collect();
        all_addresses
            .into_iter()
            .for_each(|a| self.stop_listener(a).unwrap());
    }
}

impl<Id: PeerId> Transport<Id> for TcpTransport<Id> {
    type TransportConfig = TcpTransportConfig;

    type Endpoint = TcpEndpoint;

    fn start_listener<
        Ctx: Context<Id>,
        M: MessagesHandler<Id>,
        I: InitConnectionHandler<Id, Ctx, M>,
    >(
        &mut self,
        context: Ctx,
        address: SocketAddr,
        message_handler: M,
        mut init_connection_handler: I,
    ) -> PeerNetResult<()> {
        let mut poll =
            Poll::new().map_err(|err| TcpError::InitListener.wrap().new("poll new", err, None))?;
        let mut events = Events::with_capacity(128);
        let waker = Waker::new(poll.registry(), STOP_LISTENER)
            .map_err(|err| TcpError::InitListener.wrap().new("waker new", err, None))?;
        let listener_handle: JoinHandle<PeerNetResult<()>> = std::thread::Builder::new()
            .name(format!("tcp_listener_handle_{:?}", address))
            .spawn({
                let active_connections = self.active_connections.clone();
                let total_bytes_received = self.total_bytes_received.clone();
                let total_bytes_sent = self.total_bytes_sent.clone();
                let peer_stop_rx = self.peer_stop_rx.clone();
                let peer_stop_tx = self.peer_stop_tx.clone();
                let config = self.config.clone();
                move || {
                    let server = TcpListener::bind(address).unwrap_or_else(|_| {
                        panic!("Can't bind TCP transport to address {}", address)
                    });
                    let mut mio_server = MioTcpListener::from_std(
                        server.try_clone().expect("Unable to clone server socket"),
                    );

                    // Start listening for incoming connections.
                    poll.registry()
                        .register(&mut mio_server, NEW_CONNECTION, Interest::READABLE)
                        .unwrap_or_else(|_| {
                            panic!(
                                "Can't register polling on TCP transport of address {}",
                                address
                            )
                        });
                    loop {
                        // Poll Mio for events, blocking until we get an event.
                        poll.poll(&mut events, None).unwrap_or_else(|_| {
                            panic!("Can't poll TCP transport of address {}", address)
                        });

                        // Process each event.
                        for event in events.iter() {
                            match event.token() {
                                NEW_CONNECTION => {
                                    {
                                        let read_active_connections = active_connections.read();
                                        let total_in_connections = read_active_connections
                                        .connections
                                        .iter()
                                        .filter(|(_, connection)| connection.connection_type == PeerConnectionType::IN)
                                        .count() +  read_active_connections
                                        .connection_queue.len();
                                        if total_in_connections >= config.max_in_connections {
                                            continue;
                                        }
                                    }
                                    let (stream, address) = match server.accept().map_err(|err| {
                                        TcpError::ConnectionError.wrap().new(
                                            "listener accept",
                                            err,
                                            None,
                                        )
                                    }) {
                                        Ok((stream, address)) => (stream, address),
                                        Err(err) => {
                                            println!("Error accepting connection: {:?}", err);
                                            continue;
                                        }
                                    };
                                    // if let Err(e) = stream.set_nonblocking(true) {
                                    //     println!("Error setting nonblocking: {:?}", e);
                                    // }
                                    if let Err(e) = stream.set_linger(Some(config.write_timeout)) {
                                        println!("Error setting linger: {:?}", e);
                                    }
                                    let ip_canonical = to_canonical(address.ip());
                                    let (category_name, category_info) = match config
                                        .peer_categories
                                        .iter()
                                        .find(|(_, info)| info.0.contains(&ip_canonical))
                                    {
                                        Some((category_name, info)) => {
                                            (Some(category_name.clone()), info.1)
                                        }
                                        None => (None, config.default_category_info),
                                    };

                                    let mut endpoint = Endpoint::Tcp(TcpEndpoint {
                                        address,
                                        stream,
                                        config: config.connection_config.clone(),
                                        total_bytes_received: total_bytes_received.clone(),
                                        total_bytes_sent: total_bytes_sent.clone(),
                                        endpoint_bytes_received: Arc::new(RwLock::new(0)),
                                        endpoint_bytes_sent: Arc::new(RwLock::new(0)),
                                    });
                                    let listeners = {
                                        let mut active_connections = active_connections.write();
                                        if active_connections.check_addr_accepted_pre_handshake(
                                            &address,
                                            category_name.clone(),
                                            category_info,
                                        ) {
                                            active_connections
                                                .connection_queue
                                                .push((address, category_name.clone()));
                                            active_connections.compute_counters();
                                            None
                                        } else {
                                            Some(active_connections.listeners.clone())
                                        }
                                    };
                                    if let Some(listeners) = listeners {
                                        if let Err(err) = init_connection_handler.fallback_function(
                                            &context,
                                            &mut endpoint,
                                            &listeners,
                                        ) {
                                            println!("Error while sending fallback to address {}, err:{}", address, err)
                                        }
                                        continue;
                                    }
                                    new_peer(
                                        context.clone(),
                                        endpoint,
                                        init_connection_handler.clone(),
                                        message_handler.clone(),
                                        active_connections.clone(),
                                        peer_stop_rx.clone(),
                                        PeerConnectionType::IN,
                                        category_name,
                                        category_info,
                                    );
                                }
                                STOP_LISTENER => {
                                    peer_stop_tx.send(()).unwrap();
                                    return Ok(());
                                }
                                _ => {}
                            }
                        }
                    }
                }
            })
            .expect("Failed to spawn thread tcp_listener_handle");
        {
            let mut active_connections = self.active_connections.write();
            active_connections
                .listeners
                .insert(address, super::TransportType::Tcp);
        }
        self.listeners.insert(address, (waker, listener_handle));
        Ok(())
    }

    fn try_connect<
        Ctx: Context<Id>,
        M: MessagesHandler<Id>,
        I: InitConnectionHandler<Id, Ctx, M>,
    >(
        &mut self,
        context: Ctx,
        address: SocketAddr,
        timeout: Duration,
        message_handler: M,
        handshake_handler: I,
    ) -> PeerNetResult<JoinHandle<PeerNetResult<()>>> {
        let peer_stop_rx = self.peer_stop_rx.clone();
        let config = self.config.clone();
        Ok(std::thread::Builder::new()
            .name(format!("tcp_try_connect_{:?}", address))
            .spawn({
                let active_connections = self.active_connections.clone();
                let total_bytes_received = self.total_bytes_received.clone();
                let total_bytes_sent = self.total_bytes_sent.clone();
                let wg = self.out_connection_attempts.clone();
                move || {
                    let stream = TcpStream::connect_timeout(&address, timeout).map_err(|err| {
                        TcpError::ConnectionError.wrap().new(
                            "try_connect stream connect",
                            err,
                            Some(format!("address: {}, timeout: {:?}", address, timeout)),
                        )
                    })?;
                    // if let Err(e) = stream.set_nonblocking(true) {
                    //     println!("Error setting nonblocking: {:?}", e);
                    // }
                    if let Err(e) = stream.set_linger(Some(config.write_timeout)) {
                        println!("Error setting linger: {:?}", e);
                    }
                    let ip_canonical = to_canonical(address.ip());
                    let (category_name, category_info) = match config
                        .peer_categories
                        .iter()
                        .find(|(_, info)| info.0.contains(&ip_canonical))
                    {
                        Some((category_name, info)) => (Some(category_name.clone()), info.1),
                        None => (None, config.default_category_info),
                    };
                    new_peer(
                        context.clone(),
                        Endpoint::Tcp(TcpEndpoint {
                            address,
                            stream,
                            config: config.connection_config.clone(),
                            total_bytes_received: total_bytes_received.clone(),
                            total_bytes_sent: total_bytes_sent.clone(),
                            endpoint_bytes_received: Arc::new(RwLock::new(0)),
                            endpoint_bytes_sent: Arc::new(RwLock::new(0)),
                        }),
                        handshake_handler.clone(),
                        message_handler.clone(),
                        active_connections.clone(),
                        peer_stop_rx,
                        PeerConnectionType::OUT,
                        category_name,
                        category_info,
                    );
                    drop(wg);
                    Ok(())
                }
            })
            .expect("Failed to spawn thread tcp_try_connect"))
    }

    fn stop_listener(&mut self, address: SocketAddr) -> PeerNetResult<()> {
        let (waker, handle) = self.listeners.remove(&address).ok_or(
            TcpError::StopListener
                .wrap()
                .error("rm addr", Some(format!("address: {}", address))),
        )?;
        {
            let mut active_connections = self.active_connections.write();
            active_connections.listeners.remove(&address);
        }
        waker
            .wake()
            .map_err(|e| TcpError::StopListener.wrap().new("waker wake", e, None))?;
        handle
            .join()
            .unwrap_or_else(|_| panic!("Couldn't join listener for address {}", address))
    }

    fn send(endpoint: &mut Self::Endpoint, data: &[u8]) -> PeerNetResult<()> {
        let msg_size: u32 = data.len().try_into().map_err(|_| {
            TcpError::ConnectionError
                .wrap()
                .error("send len too long", Some(format!("{:?}", data.len())))
        })?;
        if msg_size > 1048576000 {
            return Err(TcpError::ConnectionError
                .wrap()
                .error("send len too long", Some(format!("{:?}", data.len()))))?;
        }
        //TODO: Use config one
        endpoint
            .stream
            .write_all(&msg_size.to_be_bytes())
            .map_err(|err| {
                TcpError::ConnectionError.wrap().new(
                    "send len write",
                    err,
                    Some(format!("{:?}", data.len().to_le_bytes())),
                )
            })?;
        endpoint.stream.write_all(data).map_err(|err| {
            TcpError::ConnectionError
                .wrap()
                .new("send data write", err, None)
        })?;

        let mut write = endpoint.total_bytes_sent.write();
        *write += data.len() as u64;

        let mut endpoint_write = endpoint.endpoint_bytes_sent.write();
        *endpoint_write += data.len() as u64;

        Ok(())
    }

    fn send_timeout(
        endpoint: &mut TcpEndpoint,
        data: &[u8],
        timeout: Duration,
    ) -> Result<(), crate::error::PeerNetErrorData> {
        let start_time = std::time::Instant::now();

        let msg_size: u32 = data.len().try_into().map_err(|_| {
            TcpError::ConnectionError
                .wrap()
                .error("send len too long", Some(format!("{:?}", data.len())))
        })?;
        if msg_size > endpoint.config.max_message_size as u32 {
            return Err(TcpError::ConnectionError
                .wrap()
                .error("send len too long", Some(format!("{:?}", data.len()))))?;
        }
        //TODO: Use config one

        let mut try_write = || -> Result<(), PeerNetErrorData> {
            endpoint
                .stream
                .set_write_timeout(Some(timeout))
                .map_err(|e| {
                    PeerNetError::SendError.error("error set write timeout", Some(e.to_string()))
                })?;

            endpoint
                .stream
                .write_all(&msg_size.to_be_bytes())
                .map_err(|err| {
                    TcpError::ConnectionError.wrap().new(
                        "send len write",
                        err,
                        Some(format!("{:?}", data.len().to_le_bytes())),
                    )
                })?;

            let mut total_bytes_written = 0;
            let chunk_size = 1024;

            while total_bytes_written < data.len() {
                if start_time.elapsed() >= timeout {
                    return Err(PeerNetError::SendError.error("write timeout", None));
                }
                let end = (total_bytes_written + chunk_size).min(data.len());
                match endpoint.stream.write(&data[total_bytes_written..end]) {
                    Ok(bytes_written) => {
                        total_bytes_written += bytes_written;
                    }
                    Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                        // Timeout exceeded
                        if start_time.elapsed() >= timeout {
                            return Err(PeerNetError::SendError.error("write timeout", None));
                        }
                    }
                    Err(err) => {
                        return Err(
                            PeerNetError::SendError.error("write error", Some(err.to_string()))
                        );
                    }
                }
            }

            let mut write = endpoint.total_bytes_sent.write();
            *write += data.len() as u64;

            let mut endpoint_write = endpoint.endpoint_bytes_sent.write();
            *endpoint_write += data.len() as u64;

            Ok(())
        };

        let result = try_write();

        // after try_write we need to reset the timeout on the stream
        endpoint.stream.set_write_timeout(None).map_err(|e| {
            PeerNetError::SendError.error("error reset write timeout", Some(e.to_string()))
        })?;

        result
    }

    fn receive(endpoint: &mut Self::Endpoint) -> PeerNetResult<Vec<u8>> {
        //TODO: Config one

        let start_time = Instant::now();
        let mut len_bytes = vec![0u8; 4];

        // read the length of the message
        let read = endpoint.stream.read(&mut len_bytes).map_err(|err| {
            let str = format!("{:?}", err);
            TcpError::ConnectionError
                .wrap()
                .new("recv len", err, Some(str))
        })?;

        if read == 0 {
            endpoint.shutdown();
            return Err(PeerNetError::ReceiveError.error("read len = 0", None));
        }

        // Check if the timeout has occurred
        if start_time.elapsed() >= endpoint.config.read_timeout {
            return Err(PeerNetError::ReceiveError.error("timeout read len", None));
        }

        let res_size = u32::from_be_bytes(len_bytes.try_into().map_err(|err| {
            TcpError::ConnectionError
                .wrap()
                .error("recv len", Some(format!("{:?}", err)))
        })?);

        if res_size > endpoint.config.max_message_size as u32 {
            return Err(
                PeerNetError::InvalidMessage.error("len too long", Some(format!("{:?}", res_size)))
            );
        }

        let mut data = vec![0u8; res_size as usize];
        let mut total_read: usize = 0;

        while total_read < res_size as usize {
            if start_time.elapsed() >= endpoint.config.read_timeout {
                return Err(PeerNetError::ReceiveError.error("timeout read data", None));
            }

            let read = endpoint
                .stream
                .read(&mut data[total_read..])
                .map_err(|err| TcpError::ConnectionError.wrap().new("recv data", err, None))?;

            if read == 0 {
                endpoint.shutdown();
                return Err(PeerNetError::ReceiveError.error("read data = 0", None));
            }

            total_read += read;
        }

        {
            let mut write = endpoint.total_bytes_received.write();
            *write += res_size as u64;

            let mut endpoint_write = endpoint.endpoint_bytes_received.write();
            *endpoint_write += res_size as u64;
        }

        Ok(data)
    }
}
