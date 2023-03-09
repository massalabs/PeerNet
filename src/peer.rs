//! Every information about a peer (not used for now)

use std::{fmt::Debug, net::SocketAddr, thread::spawn, time::Duration};

use crossbeam::channel::{unbounded, RecvTimeoutError, Sender, TryRecvError};
use massa_signature::KeyPair;

use crate::{
    error::PeerNetError,
    handlers::MessageHandlers,
    network_manager::{SharedActiveConnections, SharedPeerDB},
    transports::{endpoint::Endpoint, InternalTransportType},
};

pub struct PeerMetadata {
    // The IP address of the peer
    _address: SocketAddr,
    // The public key of the peer
    _public_key: String,
    // InternalTransportType type
    _transport: InternalTransportType,
}

pub struct SendChannels {
    low_priority: Sender<Vec<u8>>,
    high_priority: Sender<Vec<u8>>,
}

impl SendChannels {
    pub fn send(
        &self,
        handler_id: u64,
        data: Vec<u8>,
        high_priority: bool,
    ) -> Result<(), PeerNetError> {
        let mut data = data;
        let hander_id_bytes = handler_id.to_be_bytes();
        data.splice(0..0, hander_id_bytes);
        if high_priority {
            self.high_priority.send(data).unwrap();
        } else {
            self.low_priority.send(data).unwrap();
        }
        Ok(())
    }
}

pub struct PeerConnection {
    // if handshake passed then the channel with write thread is created
    pub send_channels: SendChannels,
    //TODO: Should be only the field that allow to shutdown the connection. As it's
    //transport specific, it should be a wrapped type `ShutdownHandle`
    endpoint: Endpoint,
}

impl PeerConnection {
    pub fn shutdown(&mut self) {
        self.endpoint.shutdown();
    }
}

//TODO: Proper debug
impl Debug for PeerConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PeerConnection")
            .field("send_channels", &"SendChannels")
            .field("endpoint", &"Endpoint")
            .finish()
    }
}

pub(crate) fn new_peer(
    self_keypair: KeyPair,
    mut endpoint: Endpoint,
    message_handlers: MessageHandlers,
    _peer_db: SharedPeerDB,
    active_connections: SharedActiveConnections,
) {
    //TODO: Bounded
    spawn(move || {
        //HANDSHAKE
        let peer_id = endpoint.handshake(&self_keypair).unwrap();

        let (low_write_tx, low_write_rx) = unbounded::<Vec<u8>>();
        let (high_write_tx, high_write_rx) = unbounded::<Vec<u8>>();

        {
            let mut active_connections = active_connections.write();
            active_connections.connections.insert(
                peer_id.clone(),
                PeerConnection {
                    //TODO: Should be only the field that allow to shutdown the connection. As it's
                    //transport specific, it should be a wrapped type `ShutdownHandle`
                    endpoint: endpoint.clone(),
                    send_channels: SendChannels {
                        low_priority: low_write_tx,
                        high_priority: high_write_tx,
                    },
                },
            );
        }
        //TODO: Add to peer db
        // {
        //     let mut peer_db = peer_db.write();
        //     peer_db.peers.insert(endpoint, PeerState::ESTABLISHED(peer_id));
        // }
        // SPAWN WRITING THREAD
        //TODO: Bound
        let mut write_endpoint = endpoint.clone();
        let write_thread_handle = std::thread::spawn(move || {
            loop {
                //TODO: Better priority handling
                match high_write_rx.recv_timeout(Duration::from_millis(10)) {
                    Ok(data) => {
                        write_endpoint.send(&data).unwrap();
                        continue;
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => {
                        println!("writer thread: disconnected");
                        return;
                    }
                }
                match low_write_rx.try_recv() {
                    Ok(data) => write_endpoint.send(&data).unwrap(),
                    Err(TryRecvError::Disconnected) => {
                        println!("writer thread: disconnected");
                        return;
                    }
                    Err(TryRecvError::Empty) => {}
                }
            }
        });
        loop {
            match endpoint.receive() {
                Ok(data) => {
                    if data.is_empty() {
                        println!("Peer stop");
                        {
                            let mut active_connections = active_connections.write();
                            active_connections.connections.remove(&peer_id);
                        }
                        let _ = write_thread_handle.join();
                        return;
                    }
                    println!("Received data from peer: {:?}", data.len());
                    let Ok(handler_id) = data[0..8].try_into() else {
                        println!("Peer err received message couldn't determine handler id");
                        continue;
                    };
                    let handler_id = u64::from_be_bytes(handler_id);
                    let data = data[8..].to_vec();
                    message_handlers
                        .get_handler(handler_id)
                        .unwrap()
                        .send_message((peer_id.clone(), data))
                        .unwrap();
                }
                Err(err) => {
                    println!("Peer err {:?}", err);
                }
            }
        }
    });
}
