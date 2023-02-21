use std::{collections::HashMap, net::SocketAddr, thread::JoinHandle, time::Duration};

use crossbeam::sync::WaitGroup;
use massa_signature::{PUBLIC_KEY_SIZE_BYTES, SIGNATURE_SIZE_BYTES};
use mio::{net::UdpSocket, Events, Interest, Poll, Token, Waker};
use quiche::{accept, connect, ConnectionId};

use crate::{
    endpoint::Endpoint, error::PeerNetError, network_manager::SharedPeerDB, peer::Peer,
    peer_id::PeerId,
};

use super::Transport;

const NEW_CONNECTION_SERVER: Token = Token(0);
const NEW_CONNECTION_CLIENT: Token = Token(1);
const STOP_LISTENER: Token = Token(10);

pub(crate) struct QuicTransport {
    pub peer_db: SharedPeerDB,
    pub out_connection_attempts: WaitGroup,
    pub listeners: HashMap<SocketAddr, (Waker, JoinHandle<()>)>,
}

#[derive(Clone)]
pub struct QuicOutConnectionConfig {
    pub identity: PeerId,
    pub local_addr: SocketAddr,
}

impl QuicTransport {
    pub fn new(peer_db: SharedPeerDB) -> QuicTransport {
        QuicTransport {
            peer_db,
            out_connection_attempts: WaitGroup::new(),
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
        let listener_handle = std::thread::spawn({
            let peer_db = self.peer_db.clone();
            move || {
                let mut socket = UdpSocket::bind(address)
                    .expect(&format!("Can't bind TCP transport to address {}", address));
                // Start listening for incoming connections.
                poll.registry()
                    .register(&mut socket, NEW_CONNECTION_SERVER, Interest::READABLE)
                    .expect(&format!(
                        "Can't register polling on TCP transport of address {}",
                        address
                    ));
                let mut buf = [0; PUBLIC_KEY_SIZE_BYTES + SIGNATURE_SIZE_BYTES];
                loop {
                    println!("Listening for new connections...");
                    // Poll Mio for events, blocking until we get an event.
                    poll.poll(&mut events, None)
                        .expect(&format!("Can't poll QUIC transport of address {}", address));

                    // Process each event.
                    for event in events.iter() {
                        match event.token() {
                            NEW_CONNECTION_SERVER => {
                                //TODO: Error handling
                                let (_num_recv, from_addr) = socket.recv_from(&mut buf).unwrap();
                                let mut config =
                                    quiche::Config::new(quiche::PROTOCOL_VERSION).unwrap();
                                accept(
                                    &ConnectionId::from_ref(&buf[..PUBLIC_KEY_SIZE_BYTES]),
                                    None,
                                    address,
                                    from_addr,
                                    &mut config,
                                )
                                .unwrap();
                                let mut peer_db = peer_db.write();
                                if peer_db.nb_in_connections < peer_db.config.max_in_connections {
                                    peer_db.nb_in_connections += 1;
                                    let peer = Peer::new(Endpoint {});
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
        _timeout: Duration,
        config: &Self::OutConnectionConfig,
    ) -> Result<(), PeerNetError> {
        let mut poll = Poll::new().map_err(|err| PeerNetError::ListenerError(err.to_string()))?;
        let mut events = Events::with_capacity(128);
        let config = config.clone();
        std::thread::spawn({
            let peer_db = self.peer_db.clone();
            let wg = self.out_connection_attempts.clone();
            move || {
                let mut out = [0; PUBLIC_KEY_SIZE_BYTES + SIGNATURE_SIZE_BYTES];
                out[..PUBLIC_KEY_SIZE_BYTES].copy_from_slice(&config.identity.to_bytes());
                //TODO: Add a signature of the public key
                let mut socket = UdpSocket::bind(config.local_addr)
                    .expect(&format!("Can't bind TCP transport to address {}", address));
                // Start listening for incoming connections.
                poll.registry()
                    .register(&mut socket, NEW_CONNECTION_CLIENT, Interest::READABLE)
                    .expect(&format!(
                        "Can't register polling on TCP transport of address {}",
                        address
                    ));
                println!("Connecting to {}", address);
                //TODO: Use configs for quiche passed from config object.
                let mut quiche_config = quiche::Config::new(quiche::PROTOCOL_VERSION).unwrap();
                quiche_config.verify_peer(false);
                quiche_config
                    .set_application_protos(&[b"massa/1.0"])
                    .unwrap();
                //TODO:Use the distant peerid as the connection id.
                let mut conn = connect(
                    None,
                    &ConnectionId::from_vec(config.identity.to_bytes()),
                    config.local_addr,
                    address,
                    &mut quiche_config,
                )
                .unwrap();
                let (write, send_info) = conn.send(&mut out).unwrap();

                while let Err(e) = socket.send_to(&out[..write], send_info.to) {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        println!(
                            "{} -> {}: send() would block",
                            socket.local_addr().unwrap(),
                            send_info.to
                        );
                        continue;
                    }

                    println!("send() failed: {:?}", e);
                    return;
                }

                let mut peer_db = peer_db.write();
                if peer_db.nb_out_connections < peer_db.config.max_out_connections {
                    peer_db.nb_out_connections += 1;
                    let peer = Peer::new(Endpoint {});
                    peer_db.peers.push(peer);
                }
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
