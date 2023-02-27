use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::error::PeerNetError;
use crate::network_manager::SharedPeerDB;
use crate::peer::Peer;
use crate::peer_id::PeerId;
use crate::transports::Endpoint;

use super::Transport;

use crossbeam::sync::WaitGroup;
use mio::net::TcpListener as MioTcpListener;
use mio::{Events, Interest, Poll, Token, Waker};

pub(crate) struct TcpTransport {
    pub peer_db: SharedPeerDB,
    pub out_connection_attempts: WaitGroup,
    pub listeners: HashMap<SocketAddr, (Waker, JoinHandle<()>)>,
}

const NEW_CONNECTION: Token = Token(0);
const STOP_LISTENER: Token = Token(10);

#[derive(Clone)]
pub struct TcpOutConnectionConfig {
    // the peer we want to connect to
    pub identity: PeerId,
}

//TODO: IN/OUT different types because TCP ports are not reliable
pub struct TcpEndpoint {
    pub address: SocketAddr,
    pub stream: TcpStream,
}

impl TcpTransport {
    pub fn new(peer_db: SharedPeerDB) -> TcpTransport {
        TcpTransport {
            peer_db,
            out_connection_attempts: WaitGroup::new(),
            listeners: Default::default(),
        }
    }
}

impl Transport for TcpTransport {
    type OutConnectionConfig = TcpOutConnectionConfig;

    type Endpoint = TcpEndpoint;

    fn start_listener(&mut self, address: SocketAddr) -> Result<(), PeerNetError> {
        let mut poll = Poll::new().map_err(|err| PeerNetError::ListenerError(err.to_string()))?;
        let mut events = Events::with_capacity(128);
        let waker = Waker::new(poll.registry(), STOP_LISTENER)
            .map_err(|err| PeerNetError::ListenerError(err.to_string()))?;
        let listener_handle: JoinHandle<()> = std::thread::spawn({
            let peer_db = self.peer_db.clone();
            move || {
                let server = TcpListener::bind(address)
                    .expect(&format!("Can't bind TCP transport to address {}", address));
                let mut mio_server = MioTcpListener::from_std(server.try_clone().unwrap());
                // Start listening for incoming connections.
                poll.registry()
                    .register(&mut mio_server, NEW_CONNECTION, Interest::READABLE)
                    .expect(&format!(
                        "Can't register polling on TCP transport of address {}",
                        address
                    ));
                loop {
                    // Poll Mio for events, blocking until we get an event.
                    poll.poll(&mut events, None)
                        .expect(&format!("Can't poll TCP transport of address {}", address));

                    // Process each event.
                    for event in events.iter() {
                        match event.token() {
                            NEW_CONNECTION => {
                                //TODO: Error handling
                                //TODO: Use rate limiting
                                let (mut stream, address) = server.accept().unwrap();
                                println!("New connection");
                                let mut peer_db = peer_db.write();
                                if peer_db.nb_in_connections < peer_db.config.max_in_connections {
                                    peer_db.nb_in_connections += 1;
                                    let peer =
                                        Peer::new(Endpoint::Tcp(TcpEndpoint { address, stream }));
                                    peer_db.peers.push(peer);
                                }
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
        self.listeners.insert(address, (waker, listener_handle));
        Ok(())
    }

    fn try_connect(
        &mut self,
        address: SocketAddr,
        timeout: Duration,
        config: &Self::OutConnectionConfig,
    ) -> Result<(), PeerNetError> {
        let config = config.clone();
        std::thread::spawn({
            let peer_db = self.peer_db.clone();
            let wg = self.out_connection_attempts.clone();
            move || {
                //TODO: Rate limiting
                let Ok(connection) = TcpStream::connect_timeout(&address, timeout) else {
                return;
                };
                println!("Connected to {}", address);
                let mut peer_db = peer_db.write();
                if peer_db.nb_out_connections < peer_db.config.max_out_connections {
                    peer_db.nb_out_connections += 1;
                    let peer = Peer::new(Endpoint::Tcp(TcpEndpoint {
                        address,
                        stream: connection,
                    }));
                    peer_db.peers.push(peer);
                }
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
        waker
            .wake()
            .map_err(|e| PeerNetError::ListenerError(e.to_string()))?;
        handle
            .join()
            .expect(&format!("Couldn't join listener for address {}", address));
        Ok(())
    }

    fn send(endpoint: &Self::Endpoint) -> Result<(), PeerNetError> {
        Ok(())
    }

    fn receive(endpoint: &Self::Endpoint) -> Result<Vec<u8>, PeerNetError> {
        Ok(vec![])
    }
}
