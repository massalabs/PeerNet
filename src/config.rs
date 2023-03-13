//! Configuration for the PerNet manager.
//!
//! This module contains the configuration for the PeerNet manager.
//! It regroups all the information needed to initialize a PeerNet manager.

use massa_signature::KeyPair;

use crate::{
    handlers::MessageHandlers,
    network_manager::FallbackFunction,
    peer::HandshakeHandler,
};


/// Struct containing the configuration for the PeerNet manager.
pub struct PeerNetConfiguration<T: HandshakeHandler> {
    /// Number of peers we want to have in IN connection
    pub max_in_connections: usize,
    /// Number of peers we want to have in OUT connection
    pub max_out_connections: usize,
    /// Our peer id
    pub self_keypair: KeyPair,
    /// Message handlers (id, sender that should be listen in the thread that manage the messages)
    /// The handlers are in the conf because they should be define at compilation time and can't be changed at runtime
    pub message_handlers: MessageHandlers,
    /// Optional function to trigger at handshake
    /// (local keypair, endpoint to the peer, remote peer_id, active_connections)
    pub handshake_handler: T,
    /// Optional function to trigger when we receive a connection from a peer and we don't accept it
    pub fallback_function: Option<&'static FallbackFunction>,
}

impl<T: HandshakeHandler> PeerNetConfiguration<T> {
    pub fn default(handshake_handler: T) -> Self {
        PeerNetConfiguration {
            max_in_connections: 0,
            max_out_connections: 0,
            self_keypair: KeyPair::generate(),
            message_handlers: MessageHandlers::default(),
            fallback_function: None,
            handshake_handler,
        }
    }
}
