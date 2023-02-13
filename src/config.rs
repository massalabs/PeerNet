use std::{net::SocketAddr, collections::HashMap};

use crate::transport::TransportType;

pub struct PeerNetConfiguration {
    pub transports: HashMap<TransportType, SocketAddr>,
    // Number of peers we want to have
    pub max_peers: usize,
}