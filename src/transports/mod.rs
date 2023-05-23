//! This modules define the prototype of a transport and the dispatching between the different transports.
//!
//! This module use enum dispatch to avoid using trait objects and to save runtime costs.

use std::thread::JoinHandle;
use std::{net::SocketAddr, time::Duration};

use crate::config::{PeerNetCategories, PeerNetCategoryInfo};
use crate::context::Context;
use crate::messages::MessagesHandler;
use crate::peer_id::PeerId;
use crate::{
    config::PeerNetFeatures,
    error::{PeerNetError, PeerNetResult},
    network_manager::SharedActiveConnections,
    peer::InitConnectionHandler,
};

use self::quic::QuicConnectionConfig;
use self::{endpoint::Endpoint, quic::QuicTransport, tcp::TcpTransport};

pub mod endpoint;
mod quic;
mod tcp;

pub use quic::QuicOutConnectionConfig;
use serde::{Deserialize, Serialize};
pub use tcp::{TcpEndpoint, TcpOutConnectionConfig, TcpTransportConfig};

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
    pub fn from_out_connection_config(config: &ConnectionConfig) -> Self {
        match config {
            ConnectionConfig::Tcp(_) => TransportType::Tcp,
            ConnectionConfig::Quic(_) => TransportType::Quic,
        }
    }
}

// We define an enum instead of using a trait object because
// we want to save runtime costs
// Only problem with that, people can't implement their own transport
pub(crate) enum InternalTransportType<Id: PeerId> {
    Tcp(TcpTransport<Id>),
    Quic(QuicTransport<Id>),
}

/// All configurations for out connection depending on the transport type
#[derive(Clone, Debug)]
pub enum ConnectionConfig {
    Tcp(Box<TcpTransportConfig>),
    Quic(Box<QuicConnectionConfig>),
}

impl From<TcpTransportConfig> for ConnectionConfig {
    fn from(inner: TcpTransportConfig) -> Self {
        ConnectionConfig::Tcp(Box::new(inner))
    }
}

impl From<QuicConnectionConfig> for ConnectionConfig {
    fn from(inner: QuicConnectionConfig) -> Self {
        ConnectionConfig::Quic(Box::new(inner))
    }
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
impl<Id: PeerId> Transport<Id> for InternalTransportType<Id> {
    type TransportConfig = ConnectionConfig;
    type Endpoint = Endpoint;

    fn start_listener<
        Ctx: Context<Id>,
        M: MessagesHandler<Id>,
        I: InitConnectionHandler<Id, Ctx, M>,
    >(
        &mut self,
        context: Ctx,
        address: SocketAddr,
        message_handler: M,
        init_connection_handler: I,
    ) -> PeerNetResult<()> {
        match self {
            InternalTransportType::Tcp(transport) => {
                transport.start_listener(context, address, message_handler, init_connection_handler)
            }
            InternalTransportType::Quic(transport) => {
                transport.start_listener(context, address, message_handler, init_connection_handler)
            }
        }
    }

    fn try_connect<
        Ctx: Context<Id>,
        M: MessagesHandler<Id>,
        I: InitConnectionHandler<Id, Ctx, M>,
    >(
        &mut self,
        context: Ctx,
        address: SocketAddr,
        timeout: Duration,
        config: &Self::TransportConfig,
        message_handler: M,
        init_connection_handler: I,
    ) -> PeerNetResult<JoinHandle<PeerNetResult<()>>> {
        match (self, config) {
            (InternalTransportType::Tcp(transport), ConnectionConfig::Tcp(config)) => transport
                .try_connect(
                    context,
                    address,
                    timeout,
                    config,
                    message_handler,
                    init_connection_handler,
                ),
            (InternalTransportType::Quic(transport), ConnectionConfig::Quic(config)) => transport
                .try_connect(
                    context,
                    address,
                    timeout,
                    config,
                    message_handler,
                    init_connection_handler,
                ),
            _ => Err(PeerNetError::WrongConfigType.error("try_connect match Transport", None)),
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

    fn receive(endpoint: &mut Self::Endpoint, config: &ConnectionConfig) -> PeerNetResult<Vec<u8>> {
        match (endpoint, config) {
            (Endpoint::Tcp(endpoint), ConnectionConfig::Tcp(config)) => {
                TcpTransport::<Id>::receive(endpoint, config)
            }
            (Endpoint::Quic(endpoint), ConnectionConfig::Quic(config)) => {
                QuicTransport::<Id>::receive(endpoint, config)
            }
            _ => Err(PeerNetError::WrongConfigType.error("mod receive match", None)),
        }
    }

    fn send_timeout(
        endpoint: &mut Self::Endpoint,
        data: &[u8],
        timeout: Duration,
    ) -> PeerNetResult<()> {
        match endpoint {
            Endpoint::Tcp(endpoint) => TcpTransport::<Id>::send_timeout(endpoint, data, timeout),
            Endpoint::Quic(endpoint) => QuicTransport::<Id>::send_timeout(endpoint, data, timeout),
        }
    }
}

impl<Id: PeerId> InternalTransportType<Id> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_transport_type(
        transport_type: TransportType,
        active_connections: SharedActiveConnections<Id>,
        max_in_connections: usize,
        max_message_size_read: usize,
        data_channel_size: usize,
        features: PeerNetFeatures,
        peer_categories: PeerNetCategories,
        default_category_info: PeerNetCategoryInfo,
        local_addr: SocketAddr,
    ) -> Self {
        match transport_type {
            TransportType::Tcp => InternalTransportType::Tcp(TcpTransport::new(
                active_connections,
                max_in_connections,
                max_message_size_read,
                peer_categories,
                data_channel_size,
                default_category_info,
                features,
            )),
            TransportType::Quic => InternalTransportType::Quic(QuicTransport::new(
                active_connections,
                features,
                data_channel_size,
                local_addr,
            )),
        }
    }
}

/// This trait is used to abstract the transport layer
/// so that the network manager can be used with different
/// transport layers
pub trait Transport<Id: PeerId> {
    type TransportConfig: Clone + std::fmt::Debug;
    type Endpoint;
    /// Start a listener in a separate thread.
    /// A listener must accept connections when arriving create a new peer
    fn start_listener<
        Ctx: Context<Id>,
        M: MessagesHandler<Id>,
        I: InitConnectionHandler<Id, Ctx, M>,
    >(
        &mut self,
        context: Ctx,
        address: SocketAddr,
        message_handler: M,
        init_connection_handler: I,
    ) -> PeerNetResult<()>;
    /// Try to connect to a peer
    fn try_connect<Ctx: Context<Id>, M: MessagesHandler<Id>, I: InitConnectionHandler<Id, Ctx, M>>(
        &mut self,
        context: Ctx,
        address: SocketAddr,
        timeout: Duration,
        config: &Self::TransportConfig,
        message_handler: M,
        init_connection_handler: I,
    ) -> PeerNetResult<JoinHandle<PeerNetResult<()>>>;
    /// Stop a listener of a given address
    fn stop_listener(&mut self, address: SocketAddr) -> PeerNetResult<()>;
    fn send(endpoint: &mut Self::Endpoint, data: &[u8]) -> PeerNetResult<()>;
    fn send_timeout(
        endpoint: &mut Self::Endpoint,
        data: &[u8],
        timeout: Duration,
    ) -> PeerNetResult<()>;
    fn receive(
        endpoint: &mut Self::Endpoint,
        config: &Self::TransportConfig,
    ) -> PeerNetResult<Vec<u8>>;
}
