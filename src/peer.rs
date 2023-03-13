//! Every information about a peer (not used for now)

use std::{fmt::Debug, net::SocketAddr, thread::spawn, time::Duration};

use crossbeam::{
    channel::{unbounded, RecvTimeoutError, Sender, TryRecvError},
    select,
};
use massa_signature::KeyPair;

use crate::{
    error::PeerNetError,
    handlers::MessageHandlers,
    network_manager::{HandshakeFunction, SharedActiveConnections},
    transports::{endpoint::Endpoint, InternalTransportType},
};

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
        let handler_id_bytes = handler_id.to_be_bytes();
        data.splice(0..0, handler_id_bytes);
        if high_priority {
            self.high_priority
                .send(data)
                .map_err(|err| PeerNetError::SendError(err.to_string()))?;
        } else {
            self.low_priority
                .send(data)
                .map_err(|err| PeerNetError::SendError(err.to_string()))?;
        }
        Ok(())
    }
}

pub struct PeerConnection {
    // if handshake passed then the channel with write thread is created
    pub send_channels: SendChannels,
    //TODO: Should be only the field that allow to shutdown the connection. As it's
    //transport specific, it should be a wrapped type `ShutdownHandle`
    pub endpoint: Endpoint,
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
    handshake_function: Option<&'static HandshakeFunction>,
    message_handlers: MessageHandlers,
    active_connections: SharedActiveConnections,
) {
    //TODO: All the unwrap should pass the error to a function that remove the peer from our records
    spawn(move || {
        let listeners = {
            let active_connections = active_connections.read();
            active_connections.listeners.clone()
        };
        //HANDSHAKE
        let peer_id = match endpoint.receive().unwrap()[0] {
            0 => endpoint.handshake(&self_keypair).unwrap(),
            1 => {
                if let Some(handshake_function) = handshake_function {
                    handshake_function(&self_keypair, &mut endpoint, &listeners, &message_handlers)
                        .unwrap()
                } else {
                    panic!("Handshake required but no handshake function provided");
                }
            }
            _ => panic!("Invalid handshake"),
        };

        //TODO: Bounded

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
        // SPAWN WRITING THREAD
        //TODO: Bound
        let mut write_endpoint = endpoint.clone();
        let write_thread_handle = std::thread::spawn(move || loop {
            match high_write_rx.try_recv() {
                Ok(data) => {
                    println!("writer thread: high priority message received");
                    write_endpoint.send(&data).unwrap();
                    continue;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    println!("writer thread: disconnected");
                    return;
                }
            }
            select! {
                recv(low_write_rx) -> msg => {
                    match msg {
                        Ok(data) => {
                            println!("writer thread: low priority message received");
                            write_endpoint.send(&data).unwrap();
                        }
                        Err(_) => {
                            println!("writer thread: disconnected");
                            return;
                        }
                    }
                }
                recv(high_write_rx) -> msg => {
                    match msg {
                        Ok(data) => {
                            println!("writer thread: high priority message received");
                            write_endpoint.send(&data).unwrap();
                        }
                        Err(_) => {
                            println!("writer thread: disconnected");
                            return;
                        }
                    }
                }
            }
        });
        // READER LOOP
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
