use crate::peer_id::PeerId;

pub trait Context<Id: PeerId>: Clone + Send + 'static {
    // Returns our peer id
    fn get_peer_id(&self) -> Id;
}
