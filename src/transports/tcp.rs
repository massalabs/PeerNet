use std::collections::HashMap;
use std::io::{Read, Write};
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
use massa_hash::Hash;
use massa_signature::{KeyPair, PublicKey, Signature};
use mio::net::TcpListener as MioTcpListener;
use mio::{Events, Interest, Poll, Token, Waker};
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};

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

    fn start_listener(
        &mut self,
        self_keypair: KeyPair,
        address: SocketAddr,
    ) -> Result<(), PeerNetError> {
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
                                let (stream, address) = server.accept().unwrap();
                                println!("New connection");
                                let mut peer_db = peer_db.write();
                                if peer_db.nb_in_connections < peer_db.config.max_in_connections {
                                    peer_db.nb_in_connections += 1;
                                    let peer = Peer::new(
                                        self_keypair.clone(),
                                        Endpoint::Tcp(TcpEndpoint { address, stream }),
                                        peer_db.config.message_handlers.clone()
                                    );
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
        self_keypair: KeyPair,
        address: SocketAddr,
        timeout: Duration,
        _config: &Self::OutConnectionConfig,
    ) -> Result<(), PeerNetError> {
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
                    let peer = Peer::new(
                        self_keypair.clone(),
                        Endpoint::Tcp(TcpEndpoint {
                            address,
                            stream: connection,
                        }),
                        peer_db.config.message_handlers.clone()
                    );
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

    fn handshake(
        self_keypair: &KeyPair,
        endpoint: &mut Self::Endpoint,
    ) -> Result<(), PeerNetError> {
        //TODO: Add version in handshake not here now because no here in quic
        let mut self_random_bytes = [0u8; 32];
        StdRng::from_entropy().fill_bytes(&mut self_random_bytes);
        let self_random_hash = Hash::compute_from(&self_random_bytes);
        let mut buf = [0u8; 64];
        buf[..32].copy_from_slice(&self_random_bytes);
        buf[32..].copy_from_slice(self_keypair.get_public_key().to_bytes());

        Self::send(endpoint, &buf)?;
        let received = Self::receive(endpoint)?;
        let other_random_bytes: &[u8; 32] = received.as_slice()[..32].try_into().unwrap();
        let other_public_key = PublicKey::from_bytes(received[32..].try_into().unwrap()).unwrap();

        // sign their random bytes
        let other_random_hash = Hash::compute_from(other_random_bytes);
        let self_signature = self_keypair.sign(&other_random_hash).unwrap();

        buf.copy_from_slice(&self_signature.to_bytes());

        Self::send(endpoint, &buf)?;
        let received = Self::receive(endpoint)?;

        let other_signature =
            Signature::from_bytes(received.as_slice().try_into().unwrap()).unwrap();

        // check their signature
        other_public_key
            .verify_signature(&self_random_hash, &other_signature)
            .map_err(|err| PeerNetError::HandshakeError(err.to_string()))?;
        println!("Handshake finished");
        Ok(())
    }

    fn send(endpoint: &mut Self::Endpoint, data: &[u8]) -> Result<(), PeerNetError> {
        //TODO: Rate limiting
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
        //TODO: Rate limiting
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
