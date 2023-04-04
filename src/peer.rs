//! Every information about a peer (not used for now)

use std::collections::HashMap;
use std::{fmt::Debug, net::SocketAddr};

use crate::error::{PeerNetError, PeerNetResult};
use crate::messages::MessagesHandler;
use crate::types::KeyPair;
use crossbeam::{
    channel::{unbounded, Sender, TryRecvError},
    select,
};

use crate::{
    network_manager::SharedActiveConnections,
    peer_id::PeerId,
    transports::{endpoint::Endpoint, TransportType},
};

pub trait HandshakeHandler: Send + Clone + 'static {
    fn perform_handshake<M: MessagesHandler>(
        &mut self,
        keypair: &KeyPair,
        endpoint: &mut Endpoint,
        _listeners: &HashMap<SocketAddr, TransportType>,
        _messages_handler: M,
    ) -> PeerNetResult<PeerId> {
        endpoint.handshake(keypair)
    }
}

pub struct SendChannels {
    low_priority: Sender<Vec<u8>>,
    high_priority: Sender<Vec<u8>>,
}

impl SendChannels {
    pub fn send(&self, handler_id: u64, data: Vec<u8>, high_priority: bool) -> PeerNetResult<()> {
        let mut data = data;
        let handler_id_bytes = handler_id.to_be_bytes();
        data.splice(0..0, handler_id_bytes);
        if high_priority {
            self.high_priority
                .send(data)
                .map_err(|err| PeerNetError::SendError.new("sendchannels highprio", err, None))?;
        } else {
            self.low_priority
                .send(data)
                .map_err(|err| PeerNetError::SendError.new("sendchannels lowprio", err, None))?;
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

pub(crate) fn new_peer<T: HandshakeHandler, M: MessagesHandler>(
    self_keypair: KeyPair,
    mut endpoint: Endpoint,
    mut handshake_handler: T,
    message_handler: M,
    active_connections: SharedActiveConnections,
) {
    //TODO: All the unwrap should pass the error to a function that remove the peer from our records
    std::thread::spawn(move || {
        let listeners = {
            let active_connections = active_connections.read();
            active_connections.listeners.clone()
        };
        //HANDSHAKE
        let peer_id = match handshake_handler.perform_handshake(
            &self_keypair,
            &mut endpoint,
            &listeners,
            message_handler.clone(),
        ) {
            Ok(peer_id) => peer_id,
            Err(err) => {
                println!("Handshake error: {:?}", err);
                return;
            }
        };

        //TODO: Bounded

        let (low_write_tx, low_write_rx) = unbounded::<Vec<u8>>();
        let (high_write_tx, high_write_rx) = unbounded::<Vec<u8>>();

        active_connections.write().confirm_connection(
            peer_id.clone(),
            endpoint.clone(),
            SendChannels {
                low_priority: low_write_tx,
                high_priority: high_write_tx,
            },
        );

        // SPAWN WRITING THREAD
        //TODO: Bound
        let mut write_endpoint = endpoint.clone();
        let write_active_connections = active_connections.clone();
        let write_peer_id = peer_id.clone();
        // https://github.com/crossbeam-rs/crossbeam/issues/288
        let write_thread_handle = std::thread::spawn(move || loop {
            match high_write_rx.try_recv() {
                Ok(data) => {
                    println!("writer thread: high priority message received");
                    if write_endpoint.send(&data).is_err() {
                        write_active_connections
                            .write()
                            .connections
                            .remove(&write_peer_id)
                            .expect("Unable to remove peer id");
                        break;
                    }
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
                            if write_endpoint.send(&data).is_err() {
                                write_active_connections.write().connections.remove(&write_peer_id).expect("Unable to remove peer id");
                                break;
                            }
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
                            if write_endpoint.send(&data).is_err() {
                                write_active_connections.write().connections.remove(&write_peer_id).expect("Unable to remove peer id");
                                break;
                            }
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
                        if active_connections
                            .write()
                            .connections
                            .remove(&peer_id)
                            .is_none()
                        {
                            println!(
                                "Unable to remove peer {:?}, not found in active connections",
                                peer_id
                            );
                        }
                        let _ = write_thread_handle.join();
                        return;
                    }
                    println!("Received data from peer: {:?}", data.len());
                    match message_handler.deserialize_id(&data, &peer_id) {
                        Ok((rest, id)) => {
                            if let Err(err) = message_handler.handle(id, rest, &peer_id) {
                                println!("Error handling message: {:?}", err);
                                if active_connections
                                    .write()
                                    .connections
                                    .remove(&peer_id)
                                    .is_none()
                                {
                                    println!(
                                    "Unable to remove peer {:?}, not found in active connections",
                                    peer_id
                                );
                                }
                            }
                        }
                        Err(err) => {
                            println!("Error handling message: {:?}", err);
                            if active_connections
                                .write()
                                .connections
                                .remove(&peer_id)
                                .is_none()
                            {
                                println!(
                                    "Unable to remove peer {:?}, not found in active connections",
                                    peer_id
                                );
                            }
                        }
                    }
                }
                Err(err) => {
                    println!("Peer err {:?}", err);
                    if active_connections
                        .write()
                        .connections
                        .remove(&peer_id)
                        .is_none()
                    {
                        println!(
                            "Unable to remove peer {:?}, not found in active connections",
                            peer_id
                        );
                    }
                    return;
                }
            }
        }
    });
}
