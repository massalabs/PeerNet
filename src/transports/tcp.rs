use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::config::PeerNetFeatures;
use crate::error::{PeerNetError, PeerNetResult};
use crate::messages::MessagesHandler;
use crate::network_manager::SharedActiveConnections;
use crate::peer::{new_peer, ConnectionType, InitConnectionHandler};
use crate::transports::Endpoint;

use super::{Transport, TransportErrorType};

use crate::types::KeyPair;
use crossbeam::channel::{unbounded, Receiver, Sender};
use crossbeam::sync::WaitGroup;
use mio::net::TcpListener as MioTcpListener;
use mio::{Events, Interest, Poll, Token, Waker};
use stream_limiter::Limiter;

#[derive(Debug)]
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

#[derive(Default)]
pub struct TcpTransportConfig {
    out_connection_config: TcpOutConnectionConfig,
}

pub(crate) struct TcpTransport {
    pub active_connections: SharedActiveConnections,
    pub out_connection_attempts: WaitGroup,
    pub listeners: HashMap<SocketAddr, (Waker, JoinHandle<PeerNetResult<()>>)>,
    features: PeerNetFeatures,

    peer_stop_tx: Sender<()>,
    peer_stop_rx: Receiver<()>,
    config: TcpTransportConfig,
}

const NEW_CONNECTION: Token = Token(0);
const STOP_LISTENER: Token = Token(10);

#[derive(Clone)]
pub struct TcpOutConnectionConfig {
    rate_limit: u128,
    rate_time_window: Duration,
}

impl Default for TcpOutConnectionConfig {
    fn default() -> Self {
        TcpOutConnectionConfig {
            rate_limit: 10*1024,
            rate_time_window: Duration::from_secs(1),
        }
    }
}

//TODO: IN/OUT different types because TCP ports are not reliable
pub struct TcpEndpoint {
    config: TcpOutConnectionConfig,
    pub address: SocketAddr,
    pub stream: Limiter<TcpStream>,
}

impl Clone for TcpEndpoint {
    fn clone(&self) -> Self {
        TcpEndpoint {
            config: self.config.clone(),
            address: self.address,
            stream: Limiter::new(
                self.stream.stream.try_clone().unwrap_or_else(|_| {
                    panic!(
                        "Unable to clone stream, when cloning TcpEndpoint {}",
                        self.address
                    )
                }),
                self.config.rate_limit,
                self.config.rate_time_window,
            ),
        }
    }
}

impl TcpEndpoint {
    pub fn shutdown(&mut self) {
        let _ = self.stream.stream.shutdown(std::net::Shutdown::Both);
    }
}

impl TcpTransport {
    pub fn new(
        active_connections: SharedActiveConnections,
        features: PeerNetFeatures,
    ) -> TcpTransport {
        let (peer_stop_tx, peer_stop_rx) = unbounded();
        TcpTransport {
            active_connections,
            out_connection_attempts: WaitGroup::new(),
            listeners: Default::default(),
            features,
            peer_stop_rx,
            peer_stop_tx,
            config: TcpTransportConfig::default(),
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
        let mut poll =
            Poll::new().map_err(|err| TcpError::InitListener.wrap().new("poll new", err, None))?;
        let mut events = Events::with_capacity(128);
        let waker = Waker::new(poll.registry(), STOP_LISTENER)
            .map_err(|err| TcpError::InitListener.wrap().new("waker new", err, None))?;
        let listener_handle: JoinHandle<PeerNetResult<()>> = std::thread::spawn({
            let active_connections = self.active_connections.clone();
            let reject_same_ip_addr = self.features.reject_same_ip_addr;
            let peer_stop_rx = self.peer_stop_rx.clone();
            let peer_stop_tx = self.peer_stop_tx.clone();
            let out_conn_config = self.config.out_connection_config.clone();
            move || {
                let server = TcpListener::bind(address)
                    .unwrap_or_else(|_| panic!("Can't bind TCP transport to address {}", address));
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
                                let (stream, address) = server.accept().map_err(|err| {
                                    TcpError::ConnectionError.wrap().new(
                                        "listener accept",
                                        err,
                                        None,
                                    )
                                })?;
                                if reject_same_ip_addr
                                    && !active_connections.read().check_addr_accepted(&address)
                                {
                                    println!("Address {:?} refused", address);
                                    continue;
                                }
                                println!("Address {:?}, accepted", address);

                                let mut endpoint = Endpoint::Tcp(TcpEndpoint {
                                    config: out_conn_config.clone(),
                                    address,
                                    stream: Limiter::new(
                                        stream,
                                        out_conn_config.rate_limit,
                                        out_conn_config.rate_time_window,
                                    ),
                                });
                                {
                                    let mut active_connections = active_connections.write();
                                    if active_connections.nb_in_connections
                                        < active_connections.max_in_connections
                                    {
                                        active_connections.nb_in_connections += 1;
                                    } else {
                                        println!("Connection attempt by {}  : max_in_connections reached", address);
                                        init_connection_handler.fallback_function(
                                            &self_keypair,
                                            &mut endpoint,
                                            &active_connections.listeners,
                                        )?;
                                        continue;
                                    }
                                    active_connections.connection_queue.push(address);
                                }
                                new_peer(
                                    self_keypair.clone(),
                                    endpoint,
                                    init_connection_handler.clone(),
                                    message_handler.clone(),
                                    active_connections.clone(),
                                    peer_stop_rx.clone(),
                                    ConnectionType::IN,
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
        });
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
        Ok(std::thread::spawn({
            let active_connections = self.active_connections.clone();
            let wg = self.out_connection_attempts.clone();
            let out_conn_config = self.config.out_connection_config.clone();
            move || {
                let stream = TcpStream::connect_timeout(&address, timeout).map_err(|err| {
                    TcpError::ConnectionError.wrap().new(
                        "try_connect stream connect",
                        err,
                        Some(format!("address: {}, timeout: {:?}", address, timeout)),
                    )
                })?;
                let stream = Limiter::new(stream, out_conn_config.rate_limit, out_conn_config.rate_time_window);
                println!("Connected to {}", address);

                {
                    let mut active_connections = active_connections.write();
                    if active_connections.nb_out_connections
                        < active_connections.max_out_connections
                    {
                        active_connections.nb_out_connections += 1;
                    } else {
                        return Err(PeerNetError::BoundReached.error(
                            "tcp try_connect max_out_conn",
                            Some(format!(
                                "max: {}, nb: {}",
                                active_connections.max_out_connections,
                                active_connections.nb_out_connections
                            )),
                        ))?;
                    }
                }
                new_peer(
                    self_keypair.clone(),
                    Endpoint::Tcp(TcpEndpoint { address, stream, config: out_conn_config.clone(), }),
                    handshake_handler.clone(),
                    message_handler.clone(),
                    active_connections.clone(),
                    peer_stop_rx,
                    ConnectionType::OUT,
                );
                drop(wg);
                Ok(())
            }
        }))
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
        endpoint
            .stream
            .write(&data.len().to_le_bytes())
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
        let mut len_bytes = [0u8; 8];
        endpoint
            .stream
            .read(&mut len_bytes)
            .map_err(|err| TcpError::ConnectionError.wrap().new("recv len", err, None))?;
        let len = usize::from_le_bytes(len_bytes);
        let mut data = vec![0u8; len];
        endpoint
            .stream
            .read(&mut data)
            .map_err(|err| TcpError::ConnectionError.wrap().new("recv data", err, None))?;
        Ok(data)
    }
}
