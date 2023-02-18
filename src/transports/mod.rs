use std::{net::SocketAddr, time::Duration};

use enum_dispatch::enum_dispatch;

use crate::{error::PeerNetError, network_manager::SharedPeerDB};

use self::{quic::QuicTransport, tcp::TcpTransport};

mod quic;
mod tcp;

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

impl InternalTransportType {
    pub(crate) fn from_transport_type(
        transport_type: TransportType,
        peer_db: SharedPeerDB,
    ) -> Self {
        match transport_type {
            TransportType::Tcp => InternalTransportType::Tcp(TcpTransport::new(peer_db)),
            TransportType::Quic => InternalTransportType::Quic(QuicTransport::new()),
        }
    }
}

/// This trait is used to abstract the transport layer
/// so that the network manager can be used with different
/// transport layers
#[enum_dispatch(InternalTransportType)]
pub trait Transport {
    /// Start a listener in a separate thread.
    /// A listener must accept connections when arriving create a new peer
    /// TODO: Determine when we check we don't have too many peers and how.
    fn start_listener(&mut self, address: SocketAddr) -> Result<(), PeerNetError>;
    fn try_connect(&mut self, address: SocketAddr, timeout: Duration) -> Result<(), PeerNetError>;
    fn stop_listener(&mut self, address: SocketAddr) -> Result<(), PeerNetError>;
}
