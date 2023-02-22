//! Error types for the PeerNet library

//TODO: Increase consistency of error handling this structure has been created just to have a place to put the errors but not designed yet.
/// Error types for the PeerNet library
#[derive(Debug)]
pub enum PeerNetError {
    /// Error when trying to start/stop a listener
    ListenerError(String),
    /// Error when trying to connect to a peer with a configuration that don't match the transport type
    WrongConfigType,
    /// Error when trying to connect to a peer
    PeerConnectionError(String),
}
