//! Error types for the PeerNet library

use std::fmt::Display;

//TODO: Increase consistency of error handling this structure has been created just to have a place to put the errors but not designed yet.
/// Error types for the PeerNet library
#[derive(Debug)]
pub enum PeerNetError {
    /// Error when trying to start/stop a listener
    ListenerError(String),
    /// PeerId error
    PeerIdError(String),
    /// Error when trying to connect to a peer with a configuration that don't match the transport type
    WrongConfigType,
    /// Error when trying to connect to a peer
    PeerConnectionError(String),
    /// Error when trying to send a message to a peer
    SendError(String),
    /// Error when trying to receive a message from a peer
    ReceiveError(String),
    /// Error when trying to perform a handshake with a peer
    HandshakeError(String),
    /// Error when communicating with an handler
    HandlerError(String),
    /// Error when signing a message
    SignError(String),
    /// Error invalid message
    InvalidMessage,
}

impl Display for PeerNetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_string())
    }
}