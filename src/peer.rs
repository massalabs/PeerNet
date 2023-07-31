//! Every information about a peer (not used for now)

use std::collections::HashMap;
use std::{fmt::Debug, net::SocketAddr};

use crate::config::PeerNetCategoryInfo;
use crate::context::Context;
use crate::error::{PeerNetError, PeerNetResult};
use crate::messages::{MessagesHandler, MessagesSerializer};
use crate::peer_id::PeerId;
use crossbeam::channel::bounded;
use crossbeam::{
    channel::{Receiver, Sender, TryRecvError},
    select,
};

use crate::{
    network_manager::SharedActiveConnections,
    transports::{endpoint::Endpoint, TransportType},
};

pub trait InitConnectionHandler<Id: PeerId, Ctx: Context<Id>, M: MessagesHandler<Id>>:
    Send + Clone + 'static
{
    fn perform_handshake(
        &mut self,
        context: &Ctx,
        endpoint: &mut Endpoint,
        _listeners: &HashMap<SocketAddr, TransportType>,
        _messages_handler: M,
    ) -> PeerNetResult<Id> {
        endpoint.handshake(context.clone())
    }

    fn fallback_function(
        &mut self,
        _context: &Ctx,
        _endpoint: &mut Endpoint,
        _listeners: &HashMap<SocketAddr, TransportType>,
    ) -> PeerNetResult<()> {
        // TODO ?
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
        message_serializer.serialize(&message, &mut data)?;
        if high_priority {
            self.high_priority.send(data).map_err(|err| {
                PeerNetError::SendError.new("send sendchannels highprio", err, None)
            })?;
        } else {
            self.low_priority.send(data).map_err(|err| {
                PeerNetError::SendError.new("send sendchannels lowprio", err, None)
            })?;
        }
        Ok(())
    }

    pub fn try_send<T, MS: MessagesSerializer<T>>(
        &self,
        message_serializer: &MS,
        message: T,
        high_priority: bool,
    ) -> PeerNetResult<()> {
        let mut data = Vec::new();
        message_serializer.serialize(&message, &mut data)?;
        if high_priority {
            self.high_priority.try_send(data).map_err(|err| {
                PeerNetError::SendError.new("try_send sendchannels highprio", err, None)
            })?;
        } else {
            self.low_priority.try_send(data).map_err(|err| {
                PeerNetError::SendError.new("try_send sendchannels lowprio", err, None)
            })?;
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
    Id: PeerId,
    Ctx: Context<Id>,
    T: InitConnectionHandler<Id, Ctx, M>,
    M: MessagesHandler<Id>,
>(
    context: Ctx,
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
        let peer_id = match handshake_handler.perform_handshake(
            &context,
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
                        .retain(|addr| addr != endpoint.get_target_addr());
                    write_active_connections.compute_counters();
                }
                return;
            }
        };

        let channel_size = endpoint.get_data_channel_size();

        let (low_write_tx, low_write_rx) = bounded::<Vec<u8>>(channel_size);
        let (high_write_tx, high_write_rx) = bounded::<Vec<u8>>(channel_size);

        let endpoint_connection = match endpoint.try_clone() {
            Ok(write_endpoint) => write_endpoint,
            Err(err) => {
                println!("Error while cloning endpoint: {:?}", err);
                {
                    let mut write_active_connections = active_connections.write();
                    write_active_connections
                    .connection_queue
                    .retain(|addr| addr != endpoint.get_target_addr());
                    write_active_connections.remove_connection(&peer_id);
                }
                return;
            }
        };

         {
            let id: Id = context.get_peer_id();

            let mut write_active_connections = active_connections.write();
            write_active_connections.connection_queue
            .retain(|addr| addr != endpoint.get_target_addr());
            // if peer_id == PeerId::from_public_key(self_keypair.get_public_key()) || !active_connections.write().confirm_connection(
            if peer_id == id || !write_active_connections.confirm_connection(
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
                        if let Err(e) = write_endpoint.send::<Id>(&data) {
                            dbg!("TIM error sending data on high prio:", &e);
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
                                if let Err(e) = write_endpoint.send::<Id>(&data) {
                                    dbg!("TIM error sending data on low prio:", &e);
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
                                if let Err(e) = write_endpoint.send::<Id>(&data) {
                                    dbg!("TIM error sending data on high prio (2):", &e);
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
                            dbg!("TIM     receive data is empty");
                            let mut write_active_connections = active_connections.write();
                            write_active_connections.remove_connection(&peer_id);
                        }
                        let _ = write_thread_handle.join();
                        return;
                    }
                    if let Err(err) = message_handler.handle(&data, &peer_id) {
                        println!("Error handling message: {:?}", err);
                        {
                            let mut write_active_connections = active_connections.write();
                            write_active_connections.remove_connection(&peer_id);
                        }
                    }
                }
                Err(e) => {
                    {
                        dbg!("TIM     receive error:", &e);
                        endpoint.print_debug_infos();
                        let mut write_active_connections = active_connections.write();
                        write_active_connections.remove_connection(&peer_id);
                    }
                    return;
                }
            }
        }
    }).expect("Failed to spawn peer_thread");
}
