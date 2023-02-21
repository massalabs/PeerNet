use crate::{peer::PeerMetadata, peer_id::PeerId};

pub struct PeerNetConfiguration {
    /// Number of peers we want to have in IN connection
    pub max_in_connections: usize,
    /// Number of peers we want to have in OUT connection
    pub max_out_connections: usize,
    pub peer_id: PeerId,
    /// Initial peer list
    pub initial_peer_list: Vec<PeerMetadata>,
}
