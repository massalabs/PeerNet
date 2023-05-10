use crate::{error::PeerNetResult, types::PeerNetId};

pub trait MessagesSerializer<M> {
    /// Serialize the id of a message
    fn serialize_id(&self, message: &M, buffer: &mut Vec<u8>) -> PeerNetResult<()>;
    /// Serialize the message
    fn serialize(&self, message: &M, buffer: &mut Vec<u8>) -> PeerNetResult<()>;
}

pub trait MessagesHandler: Clone + Send + 'static {
    /// Deserialize the id of a message returning it and the rest of the message
    fn deserialize_id<'a, Id: PeerNetId>(
        &self,
        data: &'a [u8],
        peer_id: &Id,
    ) -> PeerNetResult<(&'a [u8], u64)>;
    /// Handle the message received from the network
    fn handle<Id: PeerNetId>(&self, id: u64, data: &[u8], peer_id: &Id) -> PeerNetResult<()>;
}
