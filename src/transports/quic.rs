use std::{collections::HashMap, net::SocketAddr, thread::JoinHandle, time::Duration};

use mio::{net::UdpSocket, Events, Interest, Poll, Token, Waker};
use quiche::{connect, ConnectionId};

use crate::{error::PeerNetError, peer_id::PeerId};

use super::Transport;

const NEW_CONNECTION: Token = Token(0);
const STOP_LISTENER: Token = Token(10);
const MAX_BUF_SIZE: usize = 65507;

pub(crate) struct QuicTransport {
    pub listeners: HashMap<SocketAddr, (Waker, JoinHandle<()>)>,
}

pub struct QuicOutConnectionConfig {
    pub identity: PeerId,
    pub local_addr: SocketAddr,
    pub quiche_config: quiche::Config,
}

impl QuicTransport {
    pub fn new() -> QuicTransport {
        QuicTransport {
            listeners: Default::default(),
        }
    }
}

impl Transport for QuicTransport {
    type OutConnectionConfig = QuicOutConnectionConfig;

    fn start_listener(&mut self, address: SocketAddr) -> Result<(), PeerNetError> {
        let mut poll = Poll::new().map_err(|err| PeerNetError::ListenerError(err.to_string()))?;
        let mut events = Events::with_capacity(128);
        let waker = Waker::new(poll.registry(), STOP_LISTENER)
            .map_err(|err| PeerNetError::ListenerError(err.to_string()))?;
        let listener_handle = std::thread::spawn(move || {
            let mut socket = UdpSocket::bind(address)
                .expect(&format!("Can't bind TCP transport to address {}", address));
            // Start listening for incoming connections.
            poll.registry()
                .register(&mut socket, NEW_CONNECTION, Interest::READABLE)
                .expect(&format!(
                    "Can't register polling on TCP transport of address {}",
                    address
                ));
            let mut buf = [0; MAX_BUF_SIZE];
            loop {
                println!("Listening for new connections...");
                // Poll Mio for events, blocking until we get an event.
                poll.poll(&mut events, None)
                    .expect(&format!("Can't poll QUIC transport of address {}", address));

                // Process each event.
                for event in events.iter() {
                    match event.token() {
                        NEW_CONNECTION => {
                            //TODO: Error handling
                            let (_num_recv, _from_addr) = socket.recv_from(&mut buf).unwrap();
                            println!("New connection");
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

    fn try_connect(
        &mut self,
        address: SocketAddr,
        _timeout: Duration,
        config: &mut Self::OutConnectionConfig,
    ) -> Result<(), PeerNetError> {
        connect(None, &ConnectionId::from_vec(config.identity.to_bytes()), config.local_addr, address, &mut config.quiche_config).unwrap();
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
