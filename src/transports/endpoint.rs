use std::time::Duration;

use crate::context::Context;
use crate::error::PeerNetResult;
use crate::peer_id::PeerId;

use super::tcp::TcpEndpoint;
use super::{
    quic::{QuicEndpoint, QuicTransport},
    tcp::TcpTransport,
    Transport,
};

#[cfg(feature = "testing")]
use crate::error::PeerNetError;
#[cfg(feature = "testing")]
use crossbeam::channel::{Receiver, Sender};
#[cfg(feature = "testing")]
use std::net::SocketAddr;

#[allow(clippy::large_enum_variant)]
pub enum Endpoint {
    Tcp(TcpEndpoint),
    Quic(QuicEndpoint),
    #[cfg(feature = "testing")]
    // First parameter is a sender that should be received by the user and the second is
    // a receiver that the user should send to
    MockEndpoint((Sender<Vec<u8>>, Receiver<Vec<u8>>, SocketAddr)),
}

impl Endpoint {
    pub fn get_target_addr(&self) -> &std::net::SocketAddr {
        match self {
            Endpoint::Tcp(TcpEndpoint { address, .. }) => address,
            Endpoint::Quic(QuicEndpoint { address, .. }) => address,
            #[cfg(feature = "testing")]
            Endpoint::MockEndpoint((_, _, address)) => address,
        }
    }

    pub(crate) fn get_data_channel_size(&self) -> usize {
        match self {
            Endpoint::Tcp(TcpEndpoint { config, .. }) => config.data_channel_size,
            //TODO: Real value
            Endpoint::Quic(QuicEndpoint { .. }) => 0,
            #[cfg(feature = "testing")]
            Endpoint::MockEndpoint(_) => 0,
        }
    }

    pub fn try_clone(&self) -> PeerNetResult<Endpoint> {
        match self {
            Endpoint::Tcp(endpoint) => Ok(Endpoint::Tcp(endpoint.try_clone()?)),
            Endpoint::Quic(endpoint) => Ok(Endpoint::Quic(endpoint.clone())),
            #[cfg(feature = "testing")]
            Endpoint::MockEndpoint((sender, receiver, addr)) => Ok(Endpoint::MockEndpoint((
                sender.clone(),
                receiver.clone(),
                *addr,
            ))),
        }
    }

    pub fn send<Id: PeerId>(&mut self, data: &[u8]) -> PeerNetResult<()> {
        match self {
            Endpoint::Tcp(endpoint) => TcpTransport::<Id>::send(endpoint, data),
            Endpoint::Quic(endpoint) => QuicTransport::<Id>::send(endpoint, data),
            #[cfg(feature = "testing")]
            Endpoint::MockEndpoint((sender, _, _)) => sender
                .send(data.to_vec())
                .map_err(|err| PeerNetError::ReceiveError.new("MockEndpoint", err, None)),
        }
    }

    pub fn send_timeout<Id: PeerId>(
        &mut self,
        data: &[u8],
        timeout: Duration,
    ) -> PeerNetResult<()> {
        match self {
            Endpoint::Tcp(endpoint) => TcpTransport::<Id>::send_timeout(endpoint, data, timeout),
            Endpoint::Quic(endpoint) => QuicTransport::<Id>::send_timeout(endpoint, data, timeout),
            #[cfg(feature = "testing")]
            Endpoint::MockEndpoint((sender, _, _)) => sender
                .send(data.to_vec())
                .map_err(|err| PeerNetError::ReceiveError.new("MockEndpoint", err, None)),
        }
    }

    pub fn receive<Id: PeerId>(&mut self) -> PeerNetResult<Vec<u8>> {
        match self {
            Endpoint::Tcp(endpoint) => TcpTransport::<Id>::receive(endpoint),
            Endpoint::Quic(endpoint) => QuicTransport::<Id>::receive(endpoint),
            #[cfg(feature = "testing")]
            Endpoint::MockEndpoint((_, receiver, _)) => receiver
                .recv()
                .map_err(|err| PeerNetError::ReceiveError.new("MockEndpoint", err, None)),
        }
    }

    pub(crate) fn handshake<Id: PeerId, Ctx: Context<Id>>(
        &mut self,
        _context: Ctx,
    ) -> PeerNetResult<Id> {
        Ok(Id::generate())
    }

    pub fn shutdown(&mut self) {
        match self {
            Endpoint::Tcp(endpoint) => endpoint.shutdown(),
            Endpoint::Quic(endpoint) => endpoint.shutdown(),
            #[cfg(feature = "testing")]
            Endpoint::MockEndpoint(_) => {}
        }
    }

    /// return total bytes sent and received for the endpoint (sent, received)
    pub fn get_bandwidth(&self) -> (u64, u64) {
        match self {
            Endpoint::Tcp(endpoint) => {
                let receive = endpoint.get_bytes_received();
                let sent = endpoint.get_bytes_sent();
                (sent, receive)
            }
            Endpoint::Quic(endpoint) => {
                let receive = endpoint.get_bytes_received();
                let sent = endpoint.get_bytes_sent();
                (sent, receive)
            }
            #[cfg(feature = "testing")]
            Endpoint::MockEndpoint(_) => (0, 0),
        }
    }
}

//TODO: Create trait for endpoint and match naming convention
