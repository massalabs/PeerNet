use std::collections::HashMap;

use crossbeam::channel::Sender;

use crate::{error::PeerNetError, peer_id::PeerId};

#[derive(Clone)]
pub struct MessageHandler {
    sender: Sender<(PeerId, Vec<u8>)>,
}

impl MessageHandler {
    // Create a message handler
    // Args:
    // * sender: the sender of the channel used by peer connection to dispatch messages received from the network
    pub fn new(sender: Sender<(PeerId, Vec<u8>)>) -> MessageHandler {
        MessageHandler { sender }
    }

    pub(crate) fn send_message(&self, message: (PeerId, Vec<u8>)) -> Result<(), PeerNetError> {
        //TODO: Add timeout
        self.sender
            .send(message)
            .map_err(|err| PeerNetError::HandlerError(err.to_string()))
    }
}

#[derive(Default, Clone)]
pub struct MessageHandlers(HashMap<u64, MessageHandler>);

impl MessageHandlers {
    pub fn new() -> MessageHandlers {
        MessageHandlers(Default::default())
    }

    pub fn add_handler(&mut self, id: u64, handler: MessageHandler) {
        self.0.insert(id, handler);
    }

    pub fn new_with_handlers(handlers: HashMap<u64, MessageHandler>) -> MessageHandlers {
        MessageHandlers(handlers)
    }

    pub fn get_handler(&self, id: u64) -> Option<&MessageHandler> {
        self.0.get(&id)
    }
}
