use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::error::PeerNetError;
use crate::handlers::MessageHandlers;
use crate::network_manager::{FallbackFunction, SharedActiveConnections};
use crate::peer::{new_peer, HandshakeHandler};
use crate::transports::Endpoint;

use super::Transport;

use crate::types::KeyPair;
use crossbeam::sync::WaitGroup;
use mio::net::TcpListener as MioTcpListener;
use mio::{Events, Interest, Poll, Token, Waker};
use stream_limiter::Limiter;

pub(crate) struct TcpTransport {
    pub active_connections: SharedActiveConnections,
    pub fallback_function: Option<&'static FallbackFunction>,
    pub message_handlers: MessageHandlers,
    pub out_connection_attempts: WaitGroup,
    pub listeners: HashMap<SocketAddr, (Waker, JoinHandle<()>)>,
}

const NEW_CONNECTION: Token = Token(0);
const STOP_LISTENER: Token = Token(10);
const RATE_LIMIT: u128 = 10 * 1024;

#[derive(Clone)]
pub struct TcpOutConnectionConfig;

//TODO: IN/OUT different types because TCP ports are not reliable
pub struct TcpEndpoint {
    pub address: SocketAddr,
    pub stream: Limiter<TcpStream>,
}

impl Clone for TcpEndpoint {
    fn clone(&self) -> Self {
        TcpEndpoint {
            address: self.address,
            stream: Limiter::new(
                self.stream.stream.try_clone().unwrap(),
                RATE_LIMIT,
                Duration::from_secs(1),
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
        fallback_function: Option<&'static FallbackFunction>,
        message_handlers: MessageHandlers,
    ) -> TcpTransport {
        TcpTransport {
            active_connections,
            fallback_function,
            out_connection_attempts: WaitGroup::new(),
            listeners: Default::default(),
            message_handlers,
        }
    }
}

impl Transport for TcpTransport {
    type OutConnectionConfig = TcpOutConnectionConfig;

    type Endpoint = TcpEndpoint;

    fn start_listener<T: HandshakeHandler>(
        &mut self,
        self_keypair: KeyPair,
        address: SocketAddr,
        handshake_handler: T,
    ) -> Result<(), PeerNetError> {
        let mut poll = Poll::new().map_err(|err| PeerNetError::ListenerError(err.to_string()))?;
        let mut events = Events::with_capacity(128);
        let waker = Waker::new(poll.registry(), STOP_LISTENER)
            .map_err(|err| PeerNetError::ListenerError(err.to_string()))?;
        let listener_handle: JoinHandle<()> = std::thread::spawn({
            let active_connections = self.active_connections.clone();
            let fallback_function = self.fallback_function;
            let message_handlers = self.message_handlers.clone();
            move || {
                let server = TcpListener::bind(address)
                    .unwrap_or_else(|_| panic!("Can't bind TCP transport to address {}", address));
                let mut mio_server = MioTcpListener::from_std(server.try_clone().unwrap());
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
                                //TODO: Error handling
                                let (stream, address) = server.accept().unwrap();
                                let mut endpoint = Endpoint::Tcp(TcpEndpoint {
                                    address,
                                    stream: Limiter::new(
                                        stream,
                                        RATE_LIMIT,
                                        Duration::from_secs(1),
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
                                        if let Some(fallback_function) = fallback_function {
                                            fallback_function(
                                                &self_keypair,
                                                &mut endpoint,
                                                &active_connections.listeners,
                                                &message_handlers,
                                            )
                                            .unwrap();
                                        }
                                        continue;
                                    }
                                }
                                println!("New connection");
                                new_peer(
                                    self_keypair.clone(),
                                    endpoint,
                                    handshake_handler.clone(),
                                    message_handlers.clone(),
                                    active_connections.clone(),
                                );
                            }
                            STOP_LISTENER => {
                                return;
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

    fn try_connect<T: HandshakeHandler>(
        &mut self,
        self_keypair: KeyPair,
        address: SocketAddr,
        timeout: Duration,
        _config: &Self::OutConnectionConfig,
        handshake_handler: T,
    ) -> Result<(), PeerNetError> {
        std::thread::spawn({
            let active_connections = self.active_connections.clone();
            let wg = self.out_connection_attempts.clone();
            let message_handlers = self.message_handlers.clone();
            move || {
                let Ok(stream) = TcpStream::connect_timeout(&address, timeout) else {
                    return;
                };
                let stream = Limiter::new(stream, RATE_LIMIT, Duration::from_secs(1));
                println!("Connected to {}", address);

                {
                    let mut active_connections = active_connections.write();
                    if active_connections.nb_out_connections
                        < active_connections.max_out_connections
                    {
                        active_connections.nb_out_connections += 1;
                    } else {
                        return;
                    }
                }
                new_peer(
                    self_keypair.clone(),
                    Endpoint::Tcp(TcpEndpoint { address, stream }),
                    handshake_handler.clone(),
                    message_handlers.clone(),
                    active_connections.clone(),
                );
                drop(wg);
            }
        });
        Ok(())
    }

    fn stop_listener(&mut self, address: SocketAddr) -> Result<(), PeerNetError> {
        let (waker, handle) =
            self.listeners
                .remove(&address)
                .ok_or(PeerNetError::ListenerError(format!(
                    "Can't find listener for address {}",
                    address
                )))?;
        {
            let mut active_connections = self.active_connections.write();
            active_connections.listeners.remove(&address);
        }
        waker
            .wake()
            .map_err(|e| PeerNetError::ListenerError(e.to_string()))?;
        handle
            .join()
            .unwrap_or_else(|_| panic!("Couldn't join listener for address {}", address));
        Ok(())
    }

    fn send(endpoint: &mut Self::Endpoint, data: &[u8]) -> Result<(), PeerNetError> {
        endpoint
            .stream
            .write(&data.len().to_le_bytes())
            .map_err(|err| PeerNetError::SendError(err.to_string()))?;
        endpoint
            .stream
            .write(data)
            .map_err(|err| PeerNetError::SendError(err.to_string()))?;
        Ok(())
    }

    fn receive(endpoint: &mut Self::Endpoint) -> Result<Vec<u8>, PeerNetError> {
        let mut len_bytes = [0u8; 8];
        endpoint
            .stream
            .read(&mut len_bytes)
            .map_err(|err| PeerNetError::ReceiveError(err.to_string()))?;
        let len = usize::from_le_bytes(len_bytes);
        let mut data = vec![0u8; len];
        endpoint
            .stream
            .read(&mut data)
            .map_err(|err| PeerNetError::ReceiveError(err.to_string()))?;
        Ok(data)
    }
}
