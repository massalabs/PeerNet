use crate::{error::PeerNetResult, peer_id::PeerId};

pub trait MessagesSerializer<M> {
    /// Serialize the message
    fn serialize(&self, message: &M, buffer: &mut Vec<u8>) -> PeerNetResult<()>;
}

pub trait MessagesHandler: Clone + Send + 'static {
    /// Handle the message received from the network
    fn handle(&self, data: &[u8], peer_id: &PeerId) -> PeerNetResult<()>;
}
