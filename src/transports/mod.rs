use std::net::SocketAddr;

use enum_dispatch::enum_dispatch;

use crate::error::PeerNetError;

use self::{tcp::TcpTransport, quic::QuicTransport};

mod tcp;
mod quic;

// TODO: Maybe try to fusion with the InternalTransportType enum above
#[derive(Hash, Eq, PartialEq, Debug, Copy, Clone)]
pub enum TransportType {
    Tcp,
    Quic,
}

// We define an enum instead of using a trait object because
// we want to save runtime costs
#[enum_dispatch]
pub(crate) enum InternalTransportType {
    Tcp(TcpTransport),
    Quic(QuicTransport),
}

impl From<TransportType> for InternalTransportType {
    fn from(transport_type: TransportType) -> Self {
        match transport_type {
            TransportType::Tcp => InternalTransportType::Tcp(TcpTransport::new()),
            TransportType::Quic => InternalTransportType::Quic(QuicTransport::new()),
        }
    }
}

/// This trait is used to abstract the transport layer
/// so that the network manager can be used with different
/// transport layers
#[enum_dispatch(InternalTransportType)]
pub trait Transport {
    /// Start the listener in a separate thread.
    /// A listener must accept connections when arriving create a new peer
    /// TODO: Determine when we check we don't have too many peers and how.
    fn start_listener(&mut self, address: SocketAddr) -> Result<(), PeerNetError>;
    fn try_connect(&mut self) -> Result<usize, PeerNetError>;
    fn stop_listener(&mut self) -> Result<(), PeerNetError>;
}