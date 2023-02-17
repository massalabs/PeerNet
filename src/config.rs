use std::{collections::HashMap, net::SocketAddr};

use crate::{peer::PeerMetadata, transports::TransportType};

pub struct PeerNetConfiguration {
    /// Number of peers we want to have
    pub max_peers: usize,
    /// Initial peer list
    pub initial_peer_list: Vec<PeerMetadata>
}