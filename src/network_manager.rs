//! The PeerNetManager is the main struct of the PeerNet library.
//!
//! It is the entry point of the library and is used to create and manage the transports and the peers.

use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use massa_signature::KeyPair;
use parking_lot::RwLock;

use crate::{
    config::PeerNetConfiguration,
    error::PeerNetError,
    handlers::MessageHandlers,
    peer::PeerConnection,
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
    pub connections: HashMap<PeerId, PeerConnection>,
    pub listeners: HashMap<SocketAddr, TransportType>,
}
// Send some data to a peer that we didn't accept his connection
pub type FallbackFunction = dyn Fn(
        &KeyPair,
        &mut Endpoint,
        &HashMap<SocketAddr, TransportType>,
        &MessageHandlers,
    ) -> Result<(), PeerNetError>
    + Sync;
pub type HandshakeFunction = dyn Fn(
        &KeyPair,
        &mut Endpoint,
        &HashMap<SocketAddr, TransportType>,
        &MessageHandlers,
    ) -> Result<PeerId, PeerNetError>
    + Sync;
pub(crate) type SharedActiveConnections = Arc<RwLock<ActiveConnections>>;

/// Main structure of the PeerNet library used to manage the transports and the peers.
pub struct PeerNetManager {
    pub config: PeerNetConfiguration,
    pub active_connections: SharedActiveConnections,
    // (local keypair, endpoint to the peer, remote peer_id, active_connections)
    //TODO: Maybe make a type
    handshake_function: Option<&'static HandshakeFunction>,
    fallback_function: Option<&'static FallbackFunction>,
    self_keypair: KeyPair,
    transports: HashMap<TransportType, InternalTransportType>,
}

impl PeerNetManager {
    /// Creates a new PeerNetManager. Initializes a new database of peers and have no transports by default.
    pub fn new(config: PeerNetConfiguration) -> PeerNetManager {
        let self_keypair = config.self_keypair.clone();
        let active_connections = Arc::new(RwLock::new(ActiveConnections {
            nb_in_connections: 0,
            nb_out_connections: 0,
            max_in_connections: config.max_in_connections,
            max_out_connections: config.max_out_connections,
            connections: Default::default(),
            listeners: Default::default(),
        }));
        PeerNetManager {
            handshake_function: config.handshake_function.clone(),
            fallback_function: config.fallback_function.clone(),
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
    ) -> Result<(), PeerNetError> {
        let transport = self.transports.entry(transport_type).or_insert_with(|| {
            InternalTransportType::from_transport_type(
                transport_type,
                self.active_connections.clone(),
                self.handshake_function.clone(),
                self.fallback_function.clone(),
                self.config.message_handlers.clone(),
            )
        });
        transport.start_listener(self.self_keypair.clone(), addr)?;
        Ok(())
    }

    /// Stops a listener on the given address and transport type.
    /// TODO: Maybe have listener ids
    pub fn stop_listener(
        &mut self,
        transport_type: TransportType,
        addr: SocketAddr,
    ) -> Result<(), PeerNetError> {
        let transport = self.transports.entry(transport_type).or_insert_with(|| {
            InternalTransportType::from_transport_type(
                transport_type,
                self.active_connections.clone(),
                self.handshake_function.clone(),
                self.fallback_function.clone(),
                self.config.message_handlers.clone(),
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
    ) -> Result<(), PeerNetError> {
        let transport = self
            .transports
            .entry(TransportType::from_out_connection_config(
                &out_connection_config,
            ))
            .or_insert_with(|| {
                InternalTransportType::from_transport_type(
                    TransportType::from_out_connection_config(&out_connection_config),
                    self.active_connections.clone(),
                    self.handshake_function.clone(),
                    self.fallback_function.clone(),
                    self.config.message_handlers.clone(),
                )
            });
        transport.try_connect(
            self.self_keypair.clone(),
            addr,
            timeout,
            out_connection_config.into(),
        )?;
        Ok(())
    }

    /// Get the nb_in_connections of manager
    pub fn nb_in_connections(&self) -> usize {
        self.active_connections.read().nb_in_connections
    }
}

impl Drop for PeerNetManager {
    fn drop(&mut self) {
        {
            let mut active_connections = self.active_connections.write();
            for (_, mut peer) in active_connections.connections.drain() {
                peer.shutdown();
            }
        }
    }
}
