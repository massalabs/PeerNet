//! The PeerNetManager is the main struct of the PeerNet library.
//!
//! It is the entry point of the library and is used to create and manage the transports and the peers.

use std::thread::JoinHandle;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use crate::messages::MessagesHandler;
use crate::peer::PeerConnectionType;
use crate::types::KeyPair;
use parking_lot::RwLock;

use crate::{
    config::PeerNetConfiguration,
    error::PeerNetResult,
    peer::{InitConnectionHandler, PeerConnection, SendChannels},
    peer_id::PeerId,
    transports::{
        endpoint::Endpoint, InternalTransportType, OutConnectionConfig, Transport, TransportType,
    },
};

#[derive(Debug)]
pub struct ActiveConnections {
    pub nb_in_connections: usize,
    pub nb_out_connections: usize,
    /// Number of peers we want to have in IN connection
    pub max_in_connections: usize,
    /// Number of peers we want to have in OUT connection
    pub max_out_connections: usize,
    /// Peers attempting to connect but not yet finished initialization
    pub connection_queue: Vec<SocketAddr>,
    pub connections: HashMap<PeerId, PeerConnection>,
    pub listeners: HashMap<SocketAddr, TransportType>,
}

impl ActiveConnections {
    /// Check if a new connection from a specific address can be accepted or not
    pub fn check_addr_accepted(&self, addr: &SocketAddr) -> bool {
        if self.connections.is_empty() && self.connection_queue.is_empty() {
            true
        } else {
            let active_connection_match = self
                .connections
                .iter()
                .any(|(_, connection)| connection.endpoint.get_target_addr().ip() == addr.ip());
            let queue_connection_match = self
                .connection_queue
                .iter()
                .any(|peer| peer.ip() == addr.ip());
            !(queue_connection_match || active_connection_match)
        }
    }

    pub fn confirm_connection(
        &mut self,
        id: PeerId,
        endpoint: Endpoint,
        send_channels: SendChannels,
        connection_type: PeerConnectionType,
    ) {
        self.connection_queue
            .retain(|addr| addr != endpoint.get_target_addr());
        self.connections.insert(
            id,
            PeerConnection {
                send_channels,
                //TODO: Should be only the field that allow to shutdown the connection. As it's
                //transport specific, it should be a wrapped type `ShutdownHandle`
                endpoint,
                connection_type,
            },
        );
    }
}

pub type SharedActiveConnections = Arc<RwLock<ActiveConnections>>;

/// Main structure of the PeerNet library used to manage the transports and the peers.
pub struct PeerNetManager<T: InitConnectionHandler, M: MessagesHandler> {
    pub config: PeerNetConfiguration<T, M>,
    pub active_connections: SharedActiveConnections,
    message_handler: M,
    init_connection_handler: T,
    self_keypair: KeyPair,
    transports: HashMap<TransportType, InternalTransportType>,
}

impl<T: InitConnectionHandler, M: MessagesHandler> PeerNetManager<T, M> {
    /// Creates a new PeerNetManager. Initializes a new database of peers and have no transports by default.
    pub fn new(config: PeerNetConfiguration<T, M>) -> PeerNetManager<T, M> {
        let self_keypair = config.self_keypair.clone();
        let active_connections = Arc::new(RwLock::new(ActiveConnections {
            nb_in_connections: 0,
            nb_out_connections: 0,
            max_in_connections: config.max_in_connections,
            max_out_connections: config.max_out_connections,
            connection_queue: vec![],
            connections: Default::default(),
            listeners: Default::default(),
        }));
        PeerNetManager {
            init_connection_handler: config.init_connection_handler.clone(),
            message_handler: config.message_handler.clone(),
            config,
            self_keypair,
            transports: Default::default(),
            active_connections,
        }
    }

    /// Starts a listener on the given address and transport type.
    /// The listener will accept incoming connections, verify we have seats for the peer and then create a new peer and his thread.
    pub fn start_listener(
        &mut self,
        transport_type: TransportType,
        addr: SocketAddr,
    ) -> PeerNetResult<()> {
        let transport = self.transports.entry(transport_type).or_insert_with(|| {
            InternalTransportType::from_transport_type(
                transport_type,
                self.active_connections.clone(),
                self.config.optional_features.clone(),
            )
        });
        transport.start_listener(
            self.self_keypair.clone(),
            addr,
            self.message_handler.clone(),
            self.init_connection_handler.clone(),
        )?;
        Ok(())
    }

    /// Stops a listener on the given address and transport type.
    /// TODO: Maybe have listener ids
    pub fn stop_listener(
        &mut self,
        transport_type: TransportType,
        addr: SocketAddr,
    ) -> PeerNetResult<()> {
        let transport = self.transports.entry(transport_type).or_insert_with(|| {
            InternalTransportType::from_transport_type(
                transport_type,
                self.active_connections.clone(),
                self.config.optional_features.clone(),
            )
        });
        transport.stop_listener(addr)?;
        Ok(())
    }

    /// Tries to connect to the given address and transport type.
    /// The transport used is defined by the variant of the OutConnectionConfig.
    /// If the connection can be established, a new peer is created and his thread is started.
    pub fn try_connect(
        &mut self,
        addr: SocketAddr,
        timeout: std::time::Duration,
        out_connection_config: &OutConnectionConfig,
    ) -> PeerNetResult<JoinHandle<PeerNetResult<()>>> {
        let transport = self
            .transports
            .entry(TransportType::from_out_connection_config(
                out_connection_config,
            ))
            .or_insert_with(|| {
                InternalTransportType::from_transport_type(
                    TransportType::from_out_connection_config(out_connection_config),
                    self.active_connections.clone(),
                    self.config.optional_features.clone(),
                )
            });
        transport.try_connect(
            self.self_keypair.clone(),
            addr,
            timeout,
            out_connection_config,
            self.message_handler.clone(),
            self.init_connection_handler.clone(),
        )
    }

    /// Get the nb_in_connections of manager
    pub fn nb_in_connections(&self) -> usize {
        self.active_connections.read().nb_in_connections
    }
}

impl<T: InitConnectionHandler, M: MessagesHandler> Drop for PeerNetManager<T, M> {
    fn drop(&mut self) {
        {
            let mut active_connections = self.active_connections.write();
            for (_, mut peer) in active_connections.connections.drain() {
                peer.shutdown();
            }
        }
    }
}
