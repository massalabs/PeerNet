//! Configuration for the PerNet manager.
//!
//! This module contains the configuration for the PeerNet manager.
//! It regroups all the information needed to initialize a PeerNet manager.

use crate::{peer::PeerMetadata, peer_id::PeerId};

/// Struct containing the configuration for the PeerNet manager.
pub struct PeerNetConfiguration {
    /// Number of peers we want to have in IN connection
    pub max_in_connections: usize,
    /// Number of peers we want to have in OUT connection
    pub max_out_connections: usize,
    /// Our peer id
    pub peer_id: PeerId,
    /// Initial peer list
    pub initial_peer_list: Vec<PeerMetadata>,
}
