//! Error types for the PeerNet library

use std::error::Error;
use thiserror::Error;

use crate::transports::TransportErrorType;

pub type PeerNetResult<T> = Result<T, PeerNetErrorData>;

#[derive(Debug)]
pub enum PeerNetError {
    ListenerError,
    PeerIdError,
    WrongConfigType,
    PeerConnectionError,
    SendError,
    ReceiveError,
    HandshakeError,
    HandlerError,
    SignError,
    SocketError,
    BoundReached,
    TransportError(TransportErrorType),
}

impl PeerNetError {
    /// Create a PeerNetErrorData from the variant
    pub fn new<E: Error>(
        self,
        location: &'static str,
        error: E,
        add_msg: Option<String>,
    ) -> PeerNetErrorData {
        PeerNetErrorData {
            location,
            error_type: self,
            error: Some(error.to_string()),
            add_msg,
        }
    }

    /// Create a PeerNetErrorData from scratch, not linked to any exterior error type
    pub fn error(self, location: &'static str, add_msg: Option<String>) -> PeerNetErrorData {
        PeerNetErrorData {
            location,
            error_type: self,
            error: None,
            add_msg,
        }
    }
}

/// Error types for the PeerNet library
#[derive(Error, Debug)]
pub struct PeerNetErrorData {
    location: &'static str,
    error_type: PeerNetError,
    error: Option<String>,
    add_msg: Option<String>,
}

impl std::fmt::Display for PeerNetErrorData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        writeln!(f, "Location: {}", self.location)?;
        writeln!(f, "Error type: {:?}", self.error_type)?;
        if let Some(ref err) = self.error {
            writeln!(f, "Error: {:?}", err)?;
        }
        if let Some(ref msg) = self.add_msg {
            writeln!(f, "Additionnal debug data: {}", msg)?;
        }
        Ok(())
    }
}

impl From<TransportErrorType> for PeerNetError {
    fn from(err: TransportErrorType) -> PeerNetError {
        PeerNetError::TransportError(err)
    }
}
