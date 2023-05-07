use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::config::{PeerNetCategories, PeerNetCategoryInfo, PeerNetFeatures};
use crate::error::{PeerNetError, PeerNetResult};
use crate::messages::MessagesHandler;
use crate::network_manager::{to_canonical, SharedActiveConnections};
use crate::peer::{new_peer, InitConnectionHandler, PeerConnectionType};
use crate::transports::Endpoint;

use super::{Transport, TransportErrorType};

use crate::types::KeyPair;
use crossbeam::channel::{unbounded, Receiver, Sender};
use crossbeam::sync::WaitGroup;
use mio::net::TcpListener as MioTcpListener;
use mio::{Events, Interest, Poll, Token, Waker};

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

#[derive(Default, Clone)]
pub struct TcpTransportConfig {
    out_connection_config: TcpOutConnectionConfig,
    peer_categories: PeerNetCategories,
    default_category_info: PeerNetCategoryInfo,
}

pub(crate) struct TcpTransport {
    pub active_connections: SharedActiveConnections,
    pub out_connection_attempts: WaitGroup,
    pub listeners: HashMap<SocketAddr, (Waker, JoinHandle<PeerNetResult<()>>)>,
    _features: PeerNetFeatures,

    peer_stop_tx: Sender<()>,
    peer_stop_rx: Receiver<()>,
    config: TcpTransportConfig,
}

const NEW_CONNECTION: Token = Token(0);
const STOP_LISTENER: Token = Token(10);

#[derive(Clone)]
pub struct TcpOutConnectionConfig {
    _rate_limit: u128,
    _rate_time_window: Duration,
}

impl TcpOutConnectionConfig {
    pub fn new(rate_limit: u128, rate_time_window: Duration) -> Self {
        TcpOutConnectionConfig {
            _rate_limit: rate_limit,
            _rate_time_window: rate_time_window,
        }
    }
}

impl Default for TcpOutConnectionConfig {
    fn default() -> Self {
        TcpOutConnectionConfig {
            _rate_limit: 10 * 1024,
            _rate_time_window: Duration::from_secs(1),
        }
    }
}

//TODO: IN/OUT different types because TCP ports are not reliable
pub struct TcpEndpoint {
    config: TcpOutConnectionConfig,
    pub address: SocketAddr,
    pub stream: TcpStream,
}

impl TcpEndpoint {
    pub fn try_clone(&self) -> PeerNetResult<Self> {
        Ok(TcpEndpoint {
            config: self.config.clone(),
            address: self.address,
            stream: self.stream.try_clone().map_err(|err| {
                TcpError::ConnectionError
                    .wrap()
                    .new("cannot clone stream", err, None)
            })?,
        })
    }
}

impl TcpEndpoint {
    pub fn shutdown(&mut self) {
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
}

impl TcpTransport {
    pub fn new(
        active_connections: SharedActiveConnections,
        peer_categories: PeerNetCategories,
        default_category_info: PeerNetCategoryInfo,
        features: PeerNetFeatures,
    ) -> TcpTransport {
        let (peer_stop_tx, peer_stop_rx) = unbounded();
        TcpTransport {
            active_connections,
            out_connection_attempts: WaitGroup::new(),
            listeners: Default::default(),
            _features: features,
            peer_stop_rx,
            peer_stop_tx,
            config: TcpTransportConfig {
                out_connection_config: Default::default(),
                peer_categories,
                default_category_info,
            },
        }
    }
}

impl Drop for TcpTransport {
    fn drop(&mut self) {
        let all_addresses: Vec<SocketAddr> = self.listeners.keys().cloned().collect();
        all_addresses
            .into_iter()
            .for_each(|a| self.stop_listener(a).unwrap());
    }
}

impl Transport for TcpTransport {
    type OutConnectionConfig = TcpOutConnectionConfig;

    type Endpoint = TcpEndpoint;

    fn start_listener<T: InitConnectionHandler, M: MessagesHandler>(
        &mut self,
        self_keypair: KeyPair,
        address: SocketAddr,
        message_handler: M,
        mut init_connection_handler: T,
    ) -> PeerNetResult<()> {
        println!("categories: {:?} default: {:?}", self.config.peer_categories, self.config.default_category_info);
        let mut poll =
            Poll::new().map_err(|err| TcpError::InitListener.wrap().new("poll new", err, None))?;
        let mut events = Events::with_capacity(128);
        let waker = Waker::new(poll.registry(), STOP_LISTENER)
            .map_err(|err| TcpError::InitListener.wrap().new("waker new", err, None))?;
        let listener_handle: JoinHandle<PeerNetResult<()>> = std::thread::Builder::new()
            .name(format!("tcp_listener_handle_{:?}", address))
            .spawn({
                let active_connections = self.active_connections.clone();
                let peer_stop_rx = self.peer_stop_rx.clone();
                let peer_stop_tx = self.peer_stop_tx.clone();
                let out_conn_config = self.config.out_connection_config.clone();
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
                                    println!("For addr: {} category found is {:?}, infos are {:?}", ip_canonical, category_name, category_info);

                                    let mut endpoint = Endpoint::Tcp(TcpEndpoint {
                                        config: out_conn_config.clone(),
                                        address,
                                        stream, // stream: Limiter::new(
                                                //     stream,
                                                //     out_conn_config.rate_limit,
                                                //     out_conn_config.rate_time_window,
                                                // ),
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
                                        init_connection_handler.fallback_function(
                                            &self_keypair,
                                            &mut endpoint,
                                            &listeners,
                                        )?;
                                        continue;
                                    }
                                    new_peer(
                                        self_keypair.clone(),
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

    fn try_connect<T: InitConnectionHandler, M: MessagesHandler>(
        &mut self,
        self_keypair: KeyPair,
        address: SocketAddr,
        timeout: Duration,
        _config: &Self::OutConnectionConfig,
        message_handler: M,
        handshake_handler: T,
    ) -> PeerNetResult<JoinHandle<PeerNetResult<()>>> {
        let peer_stop_rx = self.peer_stop_rx.clone();
        Ok(std::thread::Builder::new()
            .name(format!("tcp_try_connect_{:?}", address))
            .spawn({
                let active_connections = self.active_connections.clone();
                let wg = self.out_connection_attempts.clone();
                let config = self.config.clone();
                move || {
                    let stream = TcpStream::connect_timeout(&address, timeout).map_err(|err| {
                        TcpError::ConnectionError.wrap().new(
                            "try_connect stream connect",
                            err,
                            Some(format!("address: {}, timeout: {:?}", address, timeout)),
                        )
                    })?;
                    // let stream = Limiter::new(
                    //     stream,
                    //     out_conn_config.rate_limit,
                    //     out_conn_config.rate_time_window,
                    // );
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
                    println!("For addr: {} category found is {:?}, infos are {:?}", ip_canonical, category_name, category_info);
                    new_peer(
                        self_keypair.clone(),
                        Endpoint::Tcp(TcpEndpoint {
                            address,
                            stream,
                            config: config.out_connection_config.clone(),
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
            .write(&msg_size.to_be_bytes())
            .map_err(|err| {
                TcpError::ConnectionError.wrap().new(
                    "send len write",
                    err,
                    Some(format!("{:?}", data.len().to_le_bytes())),
                )
            })?;
        endpoint.stream.write(data).map_err(|err| {
            TcpError::ConnectionError
                .wrap()
                .new("send data write", err, None)
        })?;
        Ok(())
    }

    fn receive(endpoint: &mut Self::Endpoint) -> PeerNetResult<Vec<u8>> {
        //TODO: Config one
        let mut len_bytes = vec![0u8; 4];
        endpoint
            .stream
            .read(&mut len_bytes)
            .map_err(|err| TcpError::ConnectionError.wrap().new("recv len", err, None))?;
        let res_size = u32::from_be_bytes(len_bytes.try_into().map_err(|err| {
            TcpError::ConnectionError
                .wrap()
                .error("recv len", Some(format!("{:?}", err)))
        })?);
        if res_size > 1048576000 {
            return Err(
                PeerNetError::InvalidMessage.error("len too long", Some(format!("{:?}", res_size)))
            );
        }
        let mut data = vec![0u8; res_size as usize];
        endpoint
            .stream
            .read_exact(&mut data)
            .map_err(|err| TcpError::ConnectionError.wrap().new("recv data", err, None))?;
        Ok(data)
    }
}
