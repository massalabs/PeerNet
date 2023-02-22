//! The PeerNetManager is the main struct of the PeerNet library.
//!
//! It is the entry point of the library and is used to create and manage the transports and the peers.

use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use parking_lot::RwLock;

use crate::{
    config::PeerNetConfiguration,
    error::PeerNetError,
    peer::Peer,
    transports::{InternalTransportType, OutConnectionConfig, Transport, TransportType},
};

pub(crate) struct PeerDB {
    pub(crate) config: PeerNetConfiguration,
    pub(crate) peers: Vec<Peer>,
    pub(crate) nb_in_connections: usize,
    pub(crate) nb_out_connections: usize,
}

pub(crate) type SharedPeerDB = Arc<RwLock<PeerDB>>;

/// Main structure of the PeerNet library used to manage the transports and the peers.
pub struct PeerNetManager {
    peer_db: SharedPeerDB,
    transports: HashMap<TransportType, InternalTransportType>,
}

impl PeerNetManager {
    /// Creates a new PeerNetManager. Initializes a new database of peers and have no transports by default.
    pub fn new(config: PeerNetConfiguration) -> PeerNetManager {
        let peer_db = Arc::new(RwLock::new(PeerDB {
            peers: Default::default(),
            nb_in_connections: 0,
            nb_out_connections: 0,
            config: config,
        }));
        PeerNetManager {
            peer_db,
            transports: Default::default(),
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
            InternalTransportType::from_transport_type(transport_type, self.peer_db.clone())
        });
        transport.start_listener(addr)?;
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
            InternalTransportType::from_transport_type(transport_type, self.peer_db.clone())
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
                    self.peer_db.clone(),
                )
            });
        transport.try_connect(addr, timeout, out_connection_config.into())?;
        Ok(())
    }
}
