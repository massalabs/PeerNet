//! This modules define the prototype of a transport and the dispatching between the different transports.
//!
//! This module use enum dispatch to avoid using trait objects and to save runtime costs.

use std::thread::JoinHandle;
use std::{net::SocketAddr, time::Duration};

use crate::config::{PeerNetCategories, PeerNetCategoryInfo};
use crate::messages::MessagesHandler;
use crate::{
    config::PeerNetFeatures,
    error::{PeerNetError, PeerNetResult},
    network_manager::SharedActiveConnections,
    peer::InitConnectionHandler,
};

use self::{endpoint::Endpoint, quic::QuicTransport, tcp::TcpTransport};

pub mod endpoint;
mod quic;
mod tcp;

use crate::types::{PeerNetHasher, PeerNetId, PeerNetKeyPair, PeerNetPubKey, PeerNetSignature};
pub use quic::QuicOutConnectionConfig;
use serde::{Deserialize, Serialize};
pub use tcp::TcpOutConnectionConfig;

#[derive(Debug, PartialEq, Eq)]
pub enum TransportErrorType {
    Tcp(tcp::TcpError),
    Quic(quic::QuicError),
}

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
    pub fn from_out_connection_config<Id: PeerNetId>(config: &OutConnectionConfig) -> Self {
        match config {
            OutConnectionConfig::Tcp(_) => TransportType::Tcp,
            OutConnectionConfig::Quic(_) => TransportType::Quic,
        }
    }
}

// We define an enum instead of using a trait object because
// we want to save runtime costs
// Only problem with that, people can't implement their own transport
pub(crate) enum InternalTransportType<Id: PeerNetId> {
    Tcp(TcpTransport<Id>),
    Quic(QuicTransport<Id>),
}

/// All configurations for out connection depending on the transport type
#[derive(Clone)]
pub enum OutConnectionConfig {
    Tcp(Box<TcpOutConnectionConfig>),
    Quic(Box<QuicOutConnectionConfig>),
}

// impl From<<TcpTransport as Transport>::OutConnectionConfig> for OutConnectionConfig {
//     fn from(inner: TcpOutConnectionConfig) -> Self {
//         OutConnectionConfig::Tcp(Box::new(inner))
//     }
// }

// impl<T: PeerNetIdTrait> From<<QuicTransport<T> as Transport>::OutConnectionConfig>
//     for OutConnectionConfig
// {
//     fn from(inner: QuicOutConnectionConfig) -> Self {
//         OutConnectionConfig::Quic(Box::new(inner))
//     }
// }

// TODO: Macroize this I don't use enum_dispatch or enum_delegate as it generates a lot of code
// to have everything generic and we don't need this.
impl<Id: PeerNetId> Transport for InternalTransportType<Id> {
    type OutConnectionConfig = OutConnectionConfig;
    type Endpoint = Endpoint;

    fn start_listener<
        H: InitConnectionHandler + 'static,
        M: MessagesHandler,
        K: PeerNetKeyPair,
        PubKey: PeerNetPubKey,
        S: PeerNetSignature,
        Hasher: PeerNetHasher,
    >(
        &mut self,
        self_keypair: K,
        address: SocketAddr,
        message_handler: M,
        init_connection_handler: H,
    ) -> PeerNetResult<()> {
        match self {
            InternalTransportType::Tcp(transport) => transport
                .start_listener::<_, M, K, PubKey, S, Hasher>(
                    self_keypair,
                    address,
                    message_handler,
                    init_connection_handler,
                ),
            InternalTransportType::Quic(transport) => transport
                .start_listener::<_, M, K, PubKey, S, Hasher>(
                    self_keypair,
                    address,
                    message_handler,
                    init_connection_handler,
                ),
        }
    }

    fn try_connect<
        H: InitConnectionHandler + 'static,
        M: MessagesHandler,
        K: PeerNetKeyPair,
        PubKey: PeerNetPubKey,
        S: PeerNetSignature,
        Hasher: PeerNetHasher,
    >(
        &mut self,
        self_keypair: K,
        address: SocketAddr,
        timeout: Duration,
        config: &Self::OutConnectionConfig,
        message_handler: M,
        init_connection_handler: H,
    ) -> PeerNetResult<JoinHandle<PeerNetResult<()>>> {
        match self {
            InternalTransportType::Tcp(transport) => match config {
                OutConnectionConfig::Tcp(config) => transport
                    .try_connect::<H, M, K, PubKey, S, Hasher>(
                        self_keypair,
                        address,
                        timeout,
                        config,
                        message_handler,
                        init_connection_handler,
                    ),
                _ => Err(PeerNetError::WrongConfigType.error("try_connect match tcp", None)),
            },
            InternalTransportType::Quic(transport) => match config {
                OutConnectionConfig::Quic(config) => transport
                    .try_connect::<H, M, K, PubKey, S, Hasher>(
                        self_keypair,
                        address,
                        timeout,
                        config,
                        message_handler,
                        init_connection_handler,
                    ),
                _ => Err(PeerNetError::WrongConfigType.error("try_connect match quic", None)),
            },
        }
    }

    fn stop_listener(&mut self, address: SocketAddr) -> PeerNetResult<()> {
        match self {
            InternalTransportType::Tcp(transport) => transport.stop_listener(address),
            InternalTransportType::Quic(transport) => transport.stop_listener(address),
        }
    }

    fn send(endpoint: &mut Self::Endpoint, data: &[u8]) -> PeerNetResult<()> {
        match endpoint {
            Endpoint::Tcp(endpoint) => TcpTransport::<Id>::send(endpoint, data),
            Endpoint::Quic(endpoint) => QuicTransport::<Id>::send(endpoint, data),
        }
    }

    fn receive(endpoint: &mut Self::Endpoint) -> PeerNetResult<Vec<u8>> {
        match endpoint {
            Endpoint::Tcp(endpoint) => TcpTransport::<Id>::receive(endpoint),
            Endpoint::Quic(endpoint) => QuicTransport::<Id>::receive(endpoint),
        }
    }
}

impl<Id: PeerNetId> InternalTransportType<Id> {
    pub(crate) fn from_transport_type(
        transport_type: TransportType,
        active_connections: SharedActiveConnections<Id>,
        features: PeerNetFeatures,
        peer_categories: PeerNetCategories,
        default_category_info: PeerNetCategoryInfo,
    ) -> Self {
        match transport_type {
            TransportType::Tcp => InternalTransportType::Tcp(TcpTransport::new(
                active_connections,
                peer_categories,
                default_category_info,
                features,
            )),
            TransportType::Quic => {
                InternalTransportType::Quic(QuicTransport::new(active_connections, features))
            }
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
    fn start_listener<
        H: InitConnectionHandler,
        M: MessagesHandler,
        K: PeerNetKeyPair,
        PubKey: PeerNetPubKey,
        S: PeerNetSignature,
        Hasher: PeerNetHasher,
    >(
        &mut self,
        self_keypair: K,
        address: SocketAddr,
        message_handler: M,
        init_connection_handler: H,
    ) -> PeerNetResult<()>;
    /// Try to connect to a peer
    fn try_connect<
        H: InitConnectionHandler,
        M: MessagesHandler,
        K: PeerNetKeyPair,
        PubKey: PeerNetPubKey,
        S: PeerNetSignature,
        Hasher: PeerNetHasher,
    >(
        &mut self,
        self_keypair: K,
        address: SocketAddr,
        timeout: Duration,
        config: &Self::OutConnectionConfig,
        message_handler: M,
        init_connection_handler: H,
    ) -> PeerNetResult<JoinHandle<PeerNetResult<()>>>;
    /// Stop a listener of a given address
    fn stop_listener(&mut self, address: SocketAddr) -> PeerNetResult<()>;
    fn send(endpoint: &mut Self::Endpoint, data: &[u8]) -> PeerNetResult<()>;
    fn receive(endpoint: &mut Self::Endpoint) -> PeerNetResult<Vec<u8>>;
}
