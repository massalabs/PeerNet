use crate::{error::PeerNetResult, peer_id::PeerId};

pub trait MessagesHandler: Clone + Send + 'static {
    /// Get a message from a byte array received from the network deserialize it and handle it
    fn deserialize_and_handle(&self, data: &[u8], peer_id: &PeerId) -> PeerNetResult<()>;
}
