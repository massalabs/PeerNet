//! Every information about a peer (not used for now)

use std::collections::HashMap;
use std::{fmt::Debug, net::SocketAddr};

use crate::error::{PeerNetError, PeerNetResult};
use crate::messages::{MessagesHandler, MessagesSerializer};
use crate::types::KeyPair;
use crossbeam::{
    channel::{unbounded, Receiver, Sender, TryRecvError},
    select,
};

use crate::{
    network_manager::SharedActiveConnections,
    peer_id::PeerId,
    transports::{endpoint::Endpoint, TransportType},
};

pub trait InitConnectionHandler: Send + Clone + 'static {
    fn perform_handshake<M: MessagesHandler>(
        &mut self,
        keypair: &KeyPair,
        endpoint: &mut Endpoint,
        _listeners: &HashMap<SocketAddr, TransportType>,
        _messages_handler: M,
    ) -> PeerNetResult<PeerId> {
        endpoint.handshake(keypair)
    }

    fn fallback_function(
        &mut self,
        _keypair: &KeyPair,
        _endpoint: &mut Endpoint,
        _listeners: &HashMap<SocketAddr, TransportType>,
    ) -> PeerNetResult<()> {
        Ok(())
    }
}

pub struct SendChannels {
    low_priority: Sender<Vec<u8>>,
    high_priority: Sender<Vec<u8>>,
}

impl SendChannels {
    pub fn send<T, MS: MessagesSerializer<T>>(
        &self,
        message_serializer: &MS,
        message: T,
        high_priority: bool,
    ) -> PeerNetResult<()> {
        let mut data = Vec::new();
        message_serializer.serialize_id(&message, &mut data)?;
        message_serializer.serialize(&message, &mut data)?;
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

#[derive(Debug, Clone, Copy)]
pub enum ConnectionType {
    IN,
    OUT,
}

pub(crate) fn new_peer<T: InitConnectionHandler, M: MessagesHandler>(
    self_keypair: KeyPair,
    mut endpoint: Endpoint,
    mut handshake_handler: T,
    message_handler: M,
    active_connections: SharedActiveConnections,
    peer_stop: Receiver<()>,
    connection_type: ConnectionType,
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
                {
                    let mut write_active_connections = active_connections.write();
                    write_active_connections
                        .connection_queue
                        .retain(|addr| addr != endpoint.get_target_addr());
                    match connection_type {
                        ConnectionType::IN => {
                            write_active_connections.nb_in_connections -= 1;
                        }
                        ConnectionType::OUT => {
                            write_active_connections.nb_out_connections -= 1;
                        }
                    }
                }
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
        // https://github.com/crossbeam-rs/crossbeam/issues/288
        let write_thread_handle = std::thread::spawn({
            let write_peer_id = peer_id.clone();
            let write_active_connections = active_connections.clone();
            let mut write_endpoint = endpoint.clone();
            move || loop {
                match high_write_rx.try_recv() {
                    Ok(data) => {
                        println!("writer thread: high priority message received");
                        if write_endpoint.send(&data).is_err() {
                            {
                                let mut write_active_connections = write_active_connections.write();
                                write_active_connections
                                    .connections
                                    .remove(&write_peer_id)
                                    .expect("Unable to remove peer id");
                                match connection_type {
                                    ConnectionType::IN => {
                                        write_active_connections.nb_in_connections -= 1;
                                    }
                                    ConnectionType::OUT => {
                                        write_active_connections.nb_out_connections -= 1;
                                    }
                                }
                            }
                            break;
                        }
                        continue;
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => {
                        println!("writer thread high priority: disconnected");
                        return;
                    }
                }
                select! {
                    recv(peer_stop) -> _ => {
                        return;
                    }
                    recv(low_write_rx) -> msg => {
                        match msg {
                            Ok(data) => {
                                println!("writer thread: low priority message received");
                                if write_endpoint.send(&data).is_err() {
                                    {
                                        let mut write_active_connections =
                                            write_active_connections.write();
                                        write_active_connections.connections.remove(&write_peer_id).expect("Unable to remove peer id");
                                        match connection_type {
                                            ConnectionType::IN => {
                                                write_active_connections.nb_in_connections -= 1;
                                            }
                                            ConnectionType::OUT => {
                                                write_active_connections
                                                    .nb_out_connections -= 1;
                                            }
                                        }
                                    }
                                    break;
                                }
                            }
                            Err(_) => {
                                println!("writer thread low priority: disconnected");
                                return;
                            }
                        }
                    }
                    recv(high_write_rx) -> msg => {
                        match msg {
                            Ok(data) => {
                                println!("writer thread: high priority message received");
                                if write_endpoint.send(&data).is_err() {
                                    {
                                        let mut write_active_connections =
                                            write_active_connections.write();
                                        write_active_connections.connections.remove(&write_peer_id).expect("Unable to remove peer id");
                                        match connection_type {
                                            ConnectionType::IN => {
                                                write_active_connections.nb_in_connections -= 1;
                                            }
                                            ConnectionType::OUT => {
                                                write_active_connections
                                                    .nb_out_connections -= 1;
                                            }
                                        }
                                    }
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
            }
        });
        // READER LOOP
        loop {
            match endpoint.receive() {
                Ok(data) => {
                    if data.is_empty() {
                        println!("Empty data received, closing connection");
                        // We arrive here in two cases:
                        // 1. When we shutdown the endpoint from the clone that is in the manager
                        // 2. When the other side closes the connection
                        // In the first case the peer will already be removed from `connections` and so the remove is useless
                        // but in the second case we need to remove it. We have no possibilities to know which case we are in
                        // so we just try to remove it and ignore the error if it's not there.
                        {
                            let mut write_active_connections = active_connections.write();
                            if write_active_connections
                                .connections
                                .remove(&peer_id)
                                .is_some()
                            {
                                match connection_type {
                                    ConnectionType::IN => {
                                        write_active_connections.nb_in_connections -= 1;
                                    }
                                    ConnectionType::OUT => {
                                        write_active_connections.nb_out_connections -= 1;
                                    }
                                }
                            }
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
