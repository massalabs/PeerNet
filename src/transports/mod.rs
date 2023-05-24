//! This modules define the prototype of a transport and the dispatching between the different transports.
//!
//! This module use enum dispatch to avoid using trait objects and to save runtime costs.

use std::thread::JoinHandle;
use std::{net::SocketAddr, time::Duration};

use crate::context::Context;
use crate::messages::MessagesHandler;
use crate::peer_id::PeerId;
use crate::{
    config::PeerNetFeatures, error::PeerNetResult, network_manager::SharedActiveConnections,
    peer::InitConnectionHandler,
};

use self::{endpoint::Endpoint, quic::QuicTransport, tcp::TcpTransport};

pub mod endpoint;
mod quic;
mod tcp;

pub use quic::{QuicConnectionConfig, QuicTransportConfig};
use serde::{Deserialize, Serialize};
pub use tcp::{TcpConnectionConfig, TcpEndpoint, TcpTransportConfig};

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
    /// Extract the transport type from `TransportConfig`
    pub fn from_transport_config(config: &TransportConfig) -> Self {
        match config {
            TransportConfig::Tcp(_) => TransportType::Tcp,
            TransportConfig::Quic(_) => TransportType::Quic,
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
pub enum TransportConfig {
    Tcp(Box<TcpTransportConfig>),
    Quic(Box<QuicTransportConfig>),
}

impl From<TcpTransportConfig> for TransportConfig {
    fn from(inner: TcpTransportConfig) -> Self {
        TransportConfig::Tcp(Box::new(inner))
    }
}

impl From<QuicTransportConfig> for TransportConfig {
    fn from(inner: QuicTransportConfig) -> Self {
        TransportConfig::Quic(Box::new(inner))
    }
}

// impl From<<TcpTransport as Transport>::OutConnectionConfig> for OutConnectionConfig {
//     fn from(inner: TcpConnectionConfig) -> Self {
//         OutConnectionConfig::Tcp(Box::new(inner))
//     }
// }

// impl<T: PeerNetIdTrait> From<<QuicTransport<T> as Transport>::OutConnectionConfig>
//     for OutConnectionConfig
// {
//     fn from(inner: QuicConnectionConfig) -> Self {
//         OutConnectionConfig::Quic(Box::new(inner))
//     }
// }

// TODO: Macroize this I don't use enum_dispatch or enum_delegate as it generates a lot of code
// to have everything generic and we don't need this.
impl<Id: PeerId> Transport<Id> for InternalTransportType<Id> {
    type TransportConfig = TransportConfig;
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
        message_handler: M,
        init_connection_handler: I,
    ) -> PeerNetResult<JoinHandle<PeerNetResult<()>>> {
        match self {
            InternalTransportType::Tcp(transport) => transport.try_connect(
                context,
                address,
                timeout,
                message_handler,
                init_connection_handler,
            ),
            InternalTransportType::Quic(transport) => transport.try_connect(
                context,
                address,
                timeout,
                message_handler,
                init_connection_handler,
            ),
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
        config: TransportConfig,
        features: PeerNetFeatures,
        local_addr: SocketAddr,
    ) -> Self {
        match (transport_type, config) {
            (TransportType::Tcp, TransportConfig::Tcp(config)) => {
                InternalTransportType::Tcp(TcpTransport::new(active_connections, *config, features))
            }
            //TODO: Use config
            (TransportType::Quic, TransportConfig::Quic(_config)) => InternalTransportType::Quic(
                QuicTransport::new(active_connections, features, 0, local_addr),
            ),
            _ => panic!("Wrong transport type"),
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
    fn receive(endpoint: &mut Self::Endpoint) -> PeerNetResult<Vec<u8>>;
}
