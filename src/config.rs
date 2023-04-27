//! Configuration for the PerNet manager.
//!
//! This module contains the configuration for the PeerNet manager.
//! It regroups all the information needed to initialize a PeerNet manager.

use crate::messages::MessagesHandler;
use crate::peer::InitConnectionHandler;
use crate::types::KeyPair;

/// Struct containing the configuration for the PeerNet manager.
pub struct PeerNetConfiguration<T: InitConnectionHandler, M: MessagesHandler> {
    /// Number of peers we want to have in IN connection
    pub max_in_connections: usize,
    /// Number of peers we want to have in OUT connection
    pub max_out_connections: usize,
    /// Our peer id
    pub self_keypair: KeyPair,
    /// Optional function to trigger at handshake
    /// (local keypair, endpoint to the peer, remote peer_id, active_connections)
    pub init_connection_handler: T,
    /// Optional features to enable for the manager
    pub optional_features: PeerNetFeatures,
    /// Structure for message handler
    pub message_handler: M,
}

impl<T: InitConnectionHandler, M: MessagesHandler> PeerNetConfiguration<T, M> {
    pub fn default(init_connection_handler: T, message_handler: M) -> Self {
        PeerNetConfiguration {
            max_in_connections: 0,
            max_out_connections: 0,
            self_keypair: KeyPair::generate(),
            init_connection_handler,
            optional_features: PeerNetFeatures::default(),
            message_handler,
        }
    }
}

#[derive(Clone)]
pub struct PeerNetFeatures {
    pub reject_same_ip_addr: bool,
}

impl Default for PeerNetFeatures {
    fn default() -> PeerNetFeatures {
        PeerNetFeatures {
            reject_same_ip_addr: true,
        }
    }
}

impl PeerNetFeatures {
    #[allow(clippy::needless_update)]
    // Built this way instead of a mutable reference to allow setting
    //  a default config with only a specific feature enabled / disabled
    //      (ex: PeerNetFeatures::default().set_reject_same_ip_addr(false))
    /// Set the IP address spoofing rejection
    pub fn set_reject_same_ip_addr(self, val: bool) -> PeerNetFeatures {
        PeerNetFeatures {
            reject_same_ip_addr: val,
            ..self
        }
    }
}
