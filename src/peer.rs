//! Every information about a peer (not used for now)

use std::collections::HashMap;
use std::{fmt::Debug, net::SocketAddr};

use crate::config::PeerNetCategoryInfo;
use crate::error::{PeerNetError, PeerNetResult};
use crate::messages::{MessagesHandler, MessagesSerializer};
use crate::types::{PeerNetId, PeerNetKeyPair, PeerNetPubKey, PeerNetSignature};
use crossbeam::{
    channel::{unbounded, Receiver, Sender, TryRecvError},
    select,
};

use crate::{
    network_manager::SharedActiveConnections,
    transports::{endpoint::Endpoint, TransportType},
};

pub trait InitConnectionHandler: Send + Clone + 'static {
    fn perform_handshake<
        M: MessagesHandler,
        Id: PeerNetId,
        K: PeerNetKeyPair<PubKey>,
        S: PeerNetSignature,
        PubKey: PeerNetPubKey,
    >(
        &mut self,
        keypair: &K,
        endpoint: &mut Endpoint,
        _listeners: &HashMap<SocketAddr, TransportType>,
        _messages_handler: M,
    ) -> PeerNetResult<Id> {
        endpoint.handshake::<Id, K, S, PubKey>(keypair)
    }

    fn fallback_function<K: PeerNetKeyPair<PubKey>, PubKey: PeerNetPubKey>(
        &mut self,
        _keypair: &K,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerConnectionType {
    IN,
    OUT,
}

pub struct PeerConnection {
    // if handshake passed then the channel with write thread is created
    pub send_channels: SendChannels,
    //TODO: Should be only the field that allow to shutdown the connection. As it's
    //transport specific, it should be a wrapped type `ShutdownHandle`
    pub endpoint: Endpoint,
    // Determine if the connection is an out or in one
    pub connection_type: PeerConnectionType,
    // Category name
    pub category_name: Option<String>,
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
            .field("category_nae", &format!("{:?}", self.category_name))
            .finish()
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn new_peer<
    T: InitConnectionHandler,
    M: MessagesHandler,
    Id: PeerNetId,
    K: PeerNetKeyPair<PubKey>,
    PubKey: PeerNetPubKey,
    S: PeerNetSignature,
>(
    self_keypair: K,
    mut endpoint: Endpoint,
    mut handshake_handler: T,
    message_handler: M,
    active_connections: SharedActiveConnections<Id>,
    peer_stop: Receiver<()>,
    connection_type: PeerConnectionType,
    category_name: Option<String>,
    category_info: PeerNetCategoryInfo,
) {
    //TODO: All the unwrap should pass the error to a function that remove the peer from our records
    std::thread::Builder::new()
        .name("peer_thread".into())
        .spawn(move || {
        let listeners = {
            let active_connections = active_connections.read();
            active_connections.listeners.clone()
        };
        //HANDSHAKE
        let peer_id = match handshake_handler.perform_handshake::<M, Id, K, S, PubKey>(
            &self_keypair,
            &mut endpoint,
            &listeners,
            message_handler.clone(),
        ) {
            Ok(peer_id) => peer_id,
            Err(_) => {
                {
                    let mut write_active_connections = active_connections.write();
                    write_active_connections
                        .connection_queue
                        .retain(|(addr, _)| addr != endpoint.get_target_addr());
                    write_active_connections.compute_counters();
                }
                return;
            }
        };

        //TODO: Bounded

        let (low_write_tx, low_write_rx) = unbounded::<Vec<u8>>();
        let (high_write_tx, high_write_rx) = unbounded::<Vec<u8>>();

        let endpoint_connection = match endpoint.try_clone() {
            Ok(write_endpoint) => write_endpoint,
            Err(err) => {
                println!("Error while cloning endpoint: {:?}", err);
                {
                    let mut write_active_connections = active_connections.write();
                    write_active_connections.remove_connection(&peer_id);
                }
                return;
            }
        };

        let id = Id::from_public_key(self_keypair.get_public_key());
        // if peer_id == PeerId::from_public_key(self_keypair.get_public_key()) || !active_connections.write().confirm_connection(
        if peer_id == id || !active_connections.write().confirm_connection(
            peer_id.clone(),
            endpoint_connection,
            SendChannels {
                low_priority: low_write_tx,
                high_priority: high_write_tx,
            },
            connection_type,
            category_name,
            category_info
        ) {
            return;
        }

        // SPAWN WRITING THREAD
        // https://github.com/crossbeam-rs/crossbeam/issues/288
        let write_thread_handle = std::thread::spawn({
            let write_peer_id = peer_id.clone();
            let write_active_connections = active_connections.clone();
            let mut write_endpoint = match endpoint.try_clone() {
                Ok(write_endpoint) => write_endpoint,
                Err(err) => {
                    println!("Error while cloning endpoint: {:?}", err);
                    {
                        let mut write_active_connections = write_active_connections.write();
                        write_active_connections.remove_connection(&write_peer_id);
                    }
                    return;
                }
            };
            move || loop {
                match high_write_rx.try_recv() {
                    Ok(data) => {
                        if write_endpoint.send::<Id>(&data).is_err() {
                            {
                                let mut write_active_connections = write_active_connections.write();
                                write_active_connections.remove_connection(&write_peer_id);
                            }
                            break;
                        }
                        continue;
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => {
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
                                if write_endpoint.send::<Id>(&data).is_err() {
                                    {
                                        let mut write_active_connections = write_active_connections.write();
                                        write_active_connections.remove_connection(&write_peer_id);
                                    }
                                    break;
                                }
                            }
                            Err(_) => {
                                return;
                            }
                        }
                    }
                    recv(high_write_rx) -> msg => {
                        match msg {
                            Ok(data) => {
                                if write_endpoint.send::<Id>(&data).is_err() {
                                    {
                                        let mut write_active_connections =
                                            write_active_connections.write();
                                        write_active_connections.remove_connection(&write_peer_id);
                                    }
                                    break;
                                }
                            }
                            Err(_) => {
                                return;
                            }
                        }
                    }
                }
            }
        });
        // READER LOOP
        loop {
            match endpoint.receive::<Id>() {
                Ok(data) => {
                    if data.is_empty() {
                        // We arrive here in two cases:
                        // 1. When we shutdown the endpoint from the clone that is in the manager
                        // 2. When the other side closes the connection
                        // In the first case the peer will already be removed from `connections` and so the remove is useless
                        // but in the second case we need to remove it. We have no possibilities to know which case we are in
                        // so we just try to remove it and ignore the error if it's not there.
                        {
                            let mut write_active_connections = active_connections.write();
                            write_active_connections.remove_connection(&peer_id);
                        }
                        let _ = write_thread_handle.join();
                        return;
                    }
                    match message_handler.deserialize_id(&data, &peer_id) {
                        Ok((rest, id)) => {
                            if let Err(err) = message_handler.handle(id, rest, &peer_id) {
                                println!("Error handling message: {:?}", err);
                                {
                                    let mut write_active_connections = active_connections.write();
                                    write_active_connections.remove_connection(&peer_id);
                                }
                            }
                        }
                        Err(err) => {
                            if PeerNetError::InvalidMessage == err.error_type {
                                println!("Invalid message received.");
                                continue;
                            }
                            {
                                let mut write_active_connections = active_connections.write();
                                write_active_connections.remove_connection(&peer_id);
                            }
                        }
                    }
                }
                Err(_) => {
                    {
                        let mut write_active_connections = active_connections.write();
                        write_active_connections.remove_connection(&peer_id);
                    }
                    return;
                }
            }
        }
    }).expect("Failed to spawn peer_thread");
}
