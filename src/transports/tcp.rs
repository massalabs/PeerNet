use std::collections::HashMap;
use std::net::{SocketAddr, TcpStream};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::endpoint::Endpoint;
use crate::error::PeerNetError;
use crate::network_manager::SharedPeerDB;
use crate::peer::Peer;

use super::Transport;

use crossbeam::sync::WaitGroup;
use mio::net::TcpListener;
use mio::{Events, Interest, Poll, Token, Waker};

pub(crate) struct TcpTransport {
    pub peer_db: SharedPeerDB,
    pub out_connection_attempts: WaitGroup,
    pub listeners: HashMap<SocketAddr, (Waker, JoinHandle<()>)>,
}

const NEW_CONNECTION: Token = Token(0);
const STOP_LISTENER: Token = Token(10);

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
    fn start_listener(&mut self, address: SocketAddr) -> Result<(), PeerNetError> {
        let mut poll = Poll::new().expect(&format!(
            "Can't initialize polling in TCP transport of address {}",
            address
        ));
        let mut events = Events::with_capacity(128);
        let waker = Waker::new(poll.registry(), STOP_LISTENER)
            .map_err(|err| PeerNetError::ListenerError(err.to_string()))?;
        let listener_handle: JoinHandle<()> = std::thread::spawn(move || {
            let mut server = TcpListener::bind(address)
                .expect(&format!("Can't bind TCP transport to address {}", address));
            // Start listening for incoming connections.
            poll.registry()
                .register(&mut server, NEW_CONNECTION, Interest::READABLE)
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
                            let connection = server.accept();
                            println!("New connection");
                            drop(connection);
                        }
                        STOP_LISTENER => {
                            return;
                        }
                        _ => {}
                    }
                }
            }
        });
        self.listeners.insert(address, (waker, listener_handle));
        Ok(())
    }

    fn try_connect(&mut self, address: SocketAddr, timeout: Duration) -> Result<(), PeerNetError> {
        std::thread::spawn({
            let peer_db = self.peer_db.clone();
            let wg = self.out_connection_attempts.clone();
            move || {
                let Ok(_connection) = TcpStream::connect_timeout(&address, timeout) else {
                return;
                };
                println!("Connected to {}", address);
                let mut peer_db = peer_db.write();
                if peer_db.nb_in_connections < peer_db.config.max_in_connections {
                    peer_db.nb_in_connections += 1;
                    let peer = Peer::new(Endpoint {});
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
}
