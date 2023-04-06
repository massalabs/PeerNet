use crate::{error::PeerNetResult, peer_id::PeerId};

pub trait MessagesSerializer<M>: Clone + Send + 'static {
    /// Serialize the id of a message
    fn serialize_id(&self, message: &M, buffer: &mut Vec<u8>) -> PeerNetResult<()>;
    /// Serialize the message
    fn serialize(&self, message: &M, buffer: &mut Vec<u8>) -> PeerNetResult<()>;
}

pub trait MessagesHandler: Clone + Send + 'static {
    /// Deserialize the id of a message returning it and the rest of the message
    fn deserialize_id<'a>(
        &self,
        data: &'a [u8],
        peer_id: &PeerId,
    ) -> PeerNetResult<(&'a [u8], u64)>;
    /// Handle the message received from the network
    fn handle(&self, id: u64, data: &[u8], peer_id: &PeerId) -> PeerNetResult<()>;
}
