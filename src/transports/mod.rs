//! This modules define the prototype of a transport and the dispatching between the different transports.
//!
//! This module use enum dispatch to avoid using trait objects and to save runtime costs.

use std::{net::SocketAddr, time::Duration};

use crate::{
    error::PeerNetError,
    handlers::MessageHandlers,
    network_manager::{FallbackFunction, SharedActiveConnections},
    peer::HandshakeHandler,
};

use self::{endpoint::Endpoint, quic::QuicTransport, tcp::TcpTransport};

pub mod endpoint;
mod quic;
mod tcp;

use crate::types::KeyPair;
pub use quic::QuicOutConnectionConfig;
use serde::{Deserialize, Serialize};
pub use tcp::TcpOutConnectionConfig;

/// Define the different transports available
/// TODO: Maybe try to fusion with the InternalTransportType enum above
#[derive(Hash, Eq, PartialEq, Debug, Copy, Clone, Serialize, Deserialize)]
#[repr(u8)]
pub enum TransportType {
    Tcp = 0,
    Quic = 1,
}

impl TransportType {
    /// Extract the transport type from `OutConnectionConfig`
    pub fn from_out_connection_config(config: &OutConnectionConfig) -> Self {
        match config {
            OutConnectionConfig::Tcp(_) => TransportType::Tcp,
            OutConnectionConfig::Quic(_) => TransportType::Quic,
        }
    }
}

// We define an enum instead of using a trait object because
// we want to save runtime costs
// Only problem with that, people can't implement their own transport
pub(crate) enum InternalTransportType {
    Tcp(TcpTransport),
    Quic(QuicTransport),
}

/// All configurations for out connection depending on the transport type
#[derive(Clone)]
pub enum OutConnectionConfig {
    Tcp(Box<TcpOutConnectionConfig>),
    Quic(Box<QuicOutConnectionConfig>),
}

impl From<<TcpTransport as Transport>::OutConnectionConfig> for OutConnectionConfig {
    fn from(inner: TcpOutConnectionConfig) -> Self {
        OutConnectionConfig::Tcp(Box::new(inner))
    }
}

impl From<<QuicTransport as Transport>::OutConnectionConfig> for OutConnectionConfig {
    fn from(inner: QuicOutConnectionConfig) -> Self {
        OutConnectionConfig::Quic(Box::new(inner))
    }
}
// TODO: Macroize this I don't use enum_dispatch or enum_delegate as it generates a lot of code
// to have everything generic and we don't need this.
impl Transport for InternalTransportType {
    type OutConnectionConfig = OutConnectionConfig;
    type Endpoint = Endpoint;

    fn start_listener<T: HandshakeHandler + 'static>(
        &mut self,
        self_keypair: KeyPair,
        address: SocketAddr,
        handshake_handler: T,
    ) -> Result<(), PeerNetError> {
        match self {
            InternalTransportType::Tcp(transport) => {
                transport.start_listener(self_keypair, address, handshake_handler)
            }
            InternalTransportType::Quic(transport) => {
                transport.start_listener(self_keypair, address, handshake_handler)
            }
        }
    }

    fn try_connect<T: HandshakeHandler + 'static>(
        &mut self,
        self_keypair: KeyPair,
        address: SocketAddr,
        timeout: Duration,
        config: &Self::OutConnectionConfig,
        handshake_handler: T,
    ) -> Result<(), PeerNetError> {
        match self {
            InternalTransportType::Tcp(transport) => match config {
                OutConnectionConfig::Tcp(config) => {
                    transport.try_connect(self_keypair, address, timeout, config, handshake_handler)
                }
                _ => Err(PeerNetError::WrongConfigType),
            },
            InternalTransportType::Quic(transport) => match config {
                OutConnectionConfig::Quic(config) => {
                    transport.try_connect(self_keypair, address, timeout, config, handshake_handler)
                }
                _ => Err(PeerNetError::WrongConfigType),
            },
        }
    }

    fn stop_listener(&mut self, address: SocketAddr) -> Result<(), PeerNetError> {
        match self {
            InternalTransportType::Tcp(transport) => transport.stop_listener(address),
            InternalTransportType::Quic(transport) => transport.stop_listener(address),
        }
    }

    fn send(endpoint: &mut Self::Endpoint, data: &[u8]) -> Result<(), PeerNetError> {
        match endpoint {
            Endpoint::Tcp(endpoint) => TcpTransport::send(endpoint, data),
            Endpoint::Quic(endpoint) => QuicTransport::send(endpoint, data),
        }
    }

    fn receive(endpoint: &mut Self::Endpoint) -> Result<Vec<u8>, PeerNetError> {
        match endpoint {
            Endpoint::Tcp(endpoint) => TcpTransport::receive(endpoint),
            Endpoint::Quic(endpoint) => QuicTransport::receive(endpoint),
        }
    }
}

impl InternalTransportType {
    pub(crate) fn from_transport_type(
        transport_type: TransportType,
        active_connections: SharedActiveConnections,
        fallback_function: Option<&'static FallbackFunction>,
        message_handlers: MessageHandlers,
    ) -> Self {
        match transport_type {
            TransportType::Tcp => InternalTransportType::Tcp(TcpTransport::new(
                active_connections,
                fallback_function,
                message_handlers,
            )),
            TransportType::Quic => InternalTransportType::Quic(QuicTransport::new(
                active_connections,
                fallback_function,
                message_handlers,
            )),
        }
    }
}

/// This trait is used to abstract the transport layer
/// so that the network manager can be used with different
/// transport layers
pub trait Transport {
    type OutConnectionConfig: Clone;
    type Endpoint;
    /// Start a listener in a separate thread.
    /// A listener must accept connections when arriving create a new peer
    fn start_listener<T: HandshakeHandler>(
        &mut self,
        self_keypair: KeyPair,
        address: SocketAddr,
        handshake_handler: T,
    ) -> Result<(), PeerNetError>;
    /// Try to connect to a peer
    fn try_connect<T: HandshakeHandler>(
        &mut self,
        self_keypair: KeyPair,
        address: SocketAddr,
        timeout: Duration,
        config: &Self::OutConnectionConfig,
        handshake_handler: T,
    ) -> Result<(), PeerNetError>;
    /// Stop a listener of a given address
    fn stop_listener(&mut self, address: SocketAddr) -> Result<(), PeerNetError>;
    fn send(endpoint: &mut Self::Endpoint, data: &[u8]) -> Result<(), PeerNetError>;
    fn receive(endpoint: &mut Self::Endpoint) -> Result<Vec<u8>, PeerNetError>;
}
