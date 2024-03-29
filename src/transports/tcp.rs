use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::config::{PeerNetCategories, PeerNetCategoryInfo, PeerNetFeatures};
use crate::context::Context;
use crate::error::{PeerNetError, PeerNetResult};
use crate::messages::MessagesHandler;
use crate::network_manager::{to_canonical, SharedActiveConnections};
use crate::peer::{new_peer, InitConnectionHandler, PeerConnectionType};
use crate::peer_id::PeerId;
use crate::transports::Endpoint;

use super::{Transport, TransportErrorType};

use crossbeam::channel::{unbounded, Receiver, Sender};
use crossbeam::sync::WaitGroup;
use mio::net::TcpListener;
use mio::{Events, Interest, Poll, Token, Waker};
use parking_lot::RwLock;
use stream_limiter::{Limiter, LimiterOptions};

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
    pub rate_limit: u64,
    pub rate_time_window: Duration,
    pub rate_bucket_size: u64,
    pub data_channel_size: usize,
    pub max_message_size: usize,
    pub write_timeout: Duration,
    pub read_timeout: Duration,
}

impl From<TcpConnectionConfig> for LimiterOptions {
    fn from(val: TcpConnectionConfig) -> Self {
        let mut opts =
            LimiterOptions::new(val.rate_limit, val.rate_time_window, val.rate_bucket_size);
        opts.set_min_operation_size(60 * 1024); // Min packet size for TCP: 60 Kb
        opts
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
    pub stream_limiter: Limiter<TcpStream>,
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
            stream_limiter: Limiter::new(
                self.stream_limiter.stream.try_clone().map_err(|err| {
                    TcpError::ConnectionError
                        .wrap()
                        .new("cannot clone stream", err, None)
                })?,
                Some(self.config.clone().into()),
                Some(self.config.clone().into()),
            ),
            config: self.config.clone(),
            total_bytes_received: self.total_bytes_received.clone(),
            total_bytes_sent: self.total_bytes_sent.clone(),
            endpoint_bytes_received: self.endpoint_bytes_received.clone(),
            endpoint_bytes_sent: self.endpoint_bytes_sent.clone(),
        })
    }

    pub fn shutdown(&mut self) {
        let _ = self
            .stream_limiter
            .stream
            .shutdown(std::net::Shutdown::Both);
    }

    pub fn get_bytes_sent(&self) -> u64 {
        *self.endpoint_bytes_sent.read()
    }

    pub fn get_bytes_received(&self) -> u64 {
        *self.endpoint_bytes_received.read()
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
                    let mut server = TcpListener::bind(address).unwrap_or_else(|_| {
                        panic!("Can't bind TCP transport to address {}", address)
                    });

                    // Start listening for incoming connections.
                    poll.registry()
                        .register(&mut server, NEW_CONNECTION, Interest::READABLE)
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
                                    loop {
                                        let (stream, address) = match server.accept() {
                                            Ok((mut stream, address)) => {
                                                if let Err(e) = poll.registry().deregister(&mut stream) {
                                                    log::error!("Could not deregister the stream {:?} from the mio poll: {:?}", stream, e);
                                                };
                                                let stream: std::net::TcpStream = mio_stream_to_std(stream);
                                                (stream, address)
                                            },
                                            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                                                break;
                                            }
                                            Err(e) => {
                                                log::error!("Error accepting connection: {:?}", e);
                                                continue;
                                            }
                                        };
                                        {
                                            let read_active_connections = active_connections.read();
                                            let total_in_connections = read_active_connections
                                                .connections
                                                .iter()
                                                .filter(|(_, connection)| connection.connection_type == PeerConnectionType::IN)
                                                .count() +  read_active_connections
                                                .in_connection_queue.len();
                                            if total_in_connections >= config.max_in_connections {
                                                continue;
                                            }
                                        }
                                        set_tcp_stream_config(&stream, &config);
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
                                            stream_limiter: Limiter::new(
                                                stream,
                                                Some(config.connection_config.clone().into()),
                                                Some(config.connection_config.clone().into()),
                                            ),
                                            config: config.connection_config.clone(),
                                            total_bytes_received: total_bytes_received.clone(),
                                            total_bytes_sent: total_bytes_sent.clone(),
                                            endpoint_bytes_received: Arc::new(RwLock::new(0)),
                                            endpoint_bytes_sent: Arc::new(RwLock::new(0)),
                                        });
                                        let listeners = {
                                            let mut active_connections = active_connections.write();
                                            active_connections
                                            .in_connection_queue
                                            .insert(address);
                                            if active_connections.check_addr_accepted_pre_handshake(
                                                &address,
                                                category_name.clone(),
                                                category_info,
                                            ) {
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
                                                log::error!("Error while sending fallback to address {}, err:{}", address, err)
                                            }
                                            //TODO: Wait end of thread to remove connection from queue
                                            let mut active_connections = active_connections.write();
                                            active_connections
                                            .in_connection_queue
                                            .remove(&address);
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
                    active_connections
                        .write()
                        .out_connection_queue
                        .insert(address);
                    let connection = TcpStream::connect_timeout(&address, timeout).map_err(|err| {
                        log::error!("try_connect stream connect: {err:?}");
                        TcpError::ConnectionError.wrap().new(
                            "try_connect stream connect",
                            err,
                            Some(format!("address: {}, timeout: {:?}", address, timeout)),
                        )
                    });
                    match connection {
                        Err(e) => {
                            active_connections
                                .write()
                                .out_connection_queue
                                .remove(&address);
                            Err(e)
                        }
                        Ok(stream) => {
                            set_tcp_stream_config(&stream, &config);
                            let stream_limiter = Limiter::new(
                                stream,
                                Some(config.connection_config.clone().into()),
                                Some(config.connection_config.clone().into()),
                            );
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
                            new_peer(
                                context.clone(),
                                Endpoint::Tcp(TcpEndpoint {
                                    address,
                                    stream_limiter,
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
                    }
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
            log::error!("Send len too long: {:?}", data.len());
            TcpError::ConnectionError
                .wrap()
                .error("send len too long", Some(format!("{:?}", data.len())))
        })?;

        // send message size first
        let elapsed = write_exact_timeout(
            endpoint,
            &msg_size.to_be_bytes(),
            endpoint.config.write_timeout,
        )?;

        let timeout = endpoint.config.write_timeout.saturating_sub(elapsed);

        // then send message
        write_exact_timeout(endpoint, data, timeout)?;

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
        let msg_size: u32 = data.len().try_into().map_err(|_| {
            log::error!("Send_timeout len too long: {:?}", data.len());
            TcpError::ConnectionError
                .wrap()
                .error("send len too long", Some(format!("{:?}", data.len())))
        })?;
        //TODO: Use config one

        let elapsed = write_exact_timeout(endpoint, &msg_size.to_be_bytes(), timeout)?;

        let timeout = timeout.saturating_sub(elapsed);

        write_exact_timeout(endpoint, data, timeout)?;

        let mut write = endpoint.total_bytes_sent.write();
        *write += data.len() as u64;

        let mut endpoint_write = endpoint.endpoint_bytes_sent.write();
        *endpoint_write += data.len() as u64;

        Ok(())
    }

    fn receive(endpoint: &mut Self::Endpoint) -> PeerNetResult<Vec<u8>> {
        //TODO: Config one
        let mut len_bytes = vec![0u8; 4];

        // read message size first
        let elapsed = read_exact_timeout(endpoint, &mut len_bytes, endpoint.config.read_timeout)?;

        let res_size = u32::from_be_bytes(len_bytes.try_into().map_err(|err| {
            log::error!("receive len: {err:?}");
            TcpError::ConnectionError
                .wrap()
                .error("recv len", Some(format!("{:?}", err)))
        })?);

        if res_size > endpoint.config.max_message_size as u32 {
            log::error!("receive len too long: {res_size:?}");
            return Err(
                PeerNetError::InvalidMessage.error("len too long", Some(format!("{:?}", res_size)))
            );
        }
        let timeout = endpoint.config.read_timeout.saturating_sub(elapsed);

        // then read message
        let mut data = vec![0u8; res_size as usize];
        read_exact_timeout(endpoint, &mut data, timeout)?;

        {
            let mut write = endpoint.total_bytes_received.write();
            *write += res_size as u64;

            let mut endpoint_write = endpoint.endpoint_bytes_received.write();
            *endpoint_write += res_size as u64;
        }

        Ok(data)
    }
}

fn set_tcp_stream_config(stream: &TcpStream, config: &TcpTransportConfig) {
    if let Err(e) = stream.set_nonblocking(false) {
        log::error!("Error setting nonblocking: {:?}", e);
    }
    // if let Err(e) = stream.set_linger(Some(config.write_timeout)) {
    //     log::error!("Error setting linger: {:?}", e);
    // }
    if let Err(e) = stream.set_read_timeout(Some(config.read_timeout)) {
        log::error!("Error setting read timeout: {:?}", e);
    }
    if let Err(e) = stream.set_write_timeout(Some(config.write_timeout)) {
        log::error!("Error setting write timeout: {:?}", e);
    }
}

fn read_exact_timeout(
    endpoint: &mut TcpEndpoint,
    data: &mut [u8],
    timeout: Duration,
) -> PeerNetResult<Duration> {
    let start_time = Instant::now();
    let mut total_read: usize = 0;
    while total_read < data.len() {
        let remaining_time = timeout.saturating_sub(start_time.elapsed());
        if remaining_time.is_zero() {
            log::error!("send read timeout");
            return Err(PeerNetError::TimeOut.error("timeout read data", None));
        }

        if let Some(ref mut opts) = endpoint.stream_limiter.read_opt {
            opts.set_timeout(remaining_time);
        }
        endpoint
            .stream_limiter
            .stream
            .set_read_timeout(Some(remaining_time))
            .map_err(|e| {
                log::error!("error setting read timeout: {e:?}");
                PeerNetError::CouldNotSetTimeout
                    .error("error setting read timeout", Some(e.to_string()))
            })?;

        match endpoint.stream_limiter.read(&mut data[total_read..]) {
            Ok(0) => {
                endpoint.shutdown();
                log::error!("error reading: len = 0");
                return Err(PeerNetError::ConnectionClosed.error("Receive data read len = 0", None));
            }
            Ok(n) => total_read += n,
            Err(err) => {
                match err.kind() {
                    // Handle timeout error for both Unix and Windows.
                    ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted => {
                        continue;
                    }
                    // Handle other IO errors.
                    _ => {
                        log::error!("error read data stream: {err:?}");
                        return Err(PeerNetError::ReceiveError
                            .error("error read data stream", Some(format!("{:?}", err))));
                    }
                }
            }
        }
    }

    Ok(start_time.elapsed())
}

fn write_exact_timeout(
    endpoint: &mut TcpEndpoint,
    data: &[u8],
    timeout: Duration,
) -> PeerNetResult<Duration> {
    let start_time = Instant::now();
    let msg_size: u32 = data.len().try_into().map_err(|_| {
        log::error!("write error len: {:?}", data.len());
        PeerNetError::SendError.error("error with send len", Some(format!("{:?}", data.len())))
    })?;

    if msg_size > endpoint.config.max_message_size as u32 {
        log::error!("write len too long: {:?}", data.len());
        return Err(
            PeerNetError::SendError.error("send len too long", Some(format!("{:?}", data.len())))
        );
    }

    let mut write_count = 0;
    while write_count < data.len() {
        let remaining_time = timeout.saturating_sub(start_time.elapsed());

        if remaining_time.is_zero() {
            log::error!("send write timeout");
            return Err(PeerNetError::TimeOut.error("send write timeout", None));
        }

        if let Some(ref mut opts) = endpoint.stream_limiter.write_opt {
            opts.set_timeout(remaining_time);
        }
        endpoint
            .stream_limiter
            .stream
            .set_write_timeout(Some(remaining_time))
            .map_err(|e| {
                log::error!("error setting write timeout: {:?}", e);
                PeerNetError::CouldNotSetTimeout
                    .error("error setting write timeout", Some(e.to_string()))
            })?;

        match endpoint.stream_limiter.write(data[write_count..].as_ref()) {
            Ok(0) => {
                endpoint.shutdown();
                log::error!("error on write: len = 0");
                return Err(PeerNetError::SendError.error("write len = 0", None));
            }
            Ok(count) => write_count += count,
            Err(err) => {
                log::error!("error on write: {:?}", err);
                return Err(PeerNetError::SendError.error("error on write", Some(err.to_string())));
            }
        }
    }

    Ok(start_time.elapsed())
}

/// Convert a mio stream to std
/// Adapted from Tokio
pub(crate) fn mio_stream_to_std(mio_socket: mio::net::TcpStream) -> std::net::TcpStream {
    #[cfg(unix)]
    {
        use std::os::unix::io::{FromRawFd, IntoRawFd};
        unsafe { std::net::TcpStream::from_raw_fd(mio_socket.into_raw_fd()) }
    }

    #[cfg(windows)]
    {
        use std::os::windows::io::{FromRawSocket, IntoRawSocket};
        unsafe { std::net::TcpStream::from_raw_socket(mio_socket.into_raw_socket()) }
    }

    #[cfg(target_os = "wasi")]
    {
        use std::os::wasi::io::{FromRawFd, IntoRawFd};
        unsafe { std::net::TcpStream::from_raw_fd(io.into_raw_fd()) }
    }
}
