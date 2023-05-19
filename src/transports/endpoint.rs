use crate::context::Context;
use crate::error::{PeerNetError, PeerNetResult};
use crate::peer_id::PeerId;

use super::tcp::TcpEndpoint;
use super::ConnectionConfig;
use super::{
    quic::{QuicEndpoint, QuicTransport},
    tcp::TcpTransport,
    Transport,
};

pub enum Endpoint {
    Tcp(TcpEndpoint),
    Quic(QuicEndpoint),
}

impl Endpoint {
    pub fn get_target_addr(&self) -> &std::net::SocketAddr {
        match self {
            Endpoint::Tcp(TcpEndpoint { address, .. }) => address,
            Endpoint::Quic(QuicEndpoint { address, .. }) => address,
        }
    }

    pub fn try_clone(&self) -> PeerNetResult<Endpoint> {
        match self {
            Endpoint::Tcp(endpoint) => Ok(Endpoint::Tcp(endpoint.try_clone()?)),
            Endpoint::Quic(endpoint) => Ok(Endpoint::Quic(endpoint.clone())),
        }
    }

    pub fn send<Id: PeerId>(&mut self, data: &[u8]) -> PeerNetResult<()> {
        match self {
            Endpoint::Tcp(endpoint) => TcpTransport::<Id>::send(endpoint, data),
            Endpoint::Quic(endpoint) => QuicTransport::<Id>::send(endpoint, data),
        }
    }
    pub fn receive<Id: PeerId>(&mut self, config: ConnectionConfig) -> PeerNetResult<Vec<u8>> {
        match self {
            Endpoint::Tcp(endpoint) => {
                let tcp_config = match config {
                    ConnectionConfig::Tcp(conf) => conf,
                    _ => return Err(PeerNetError::WrongConfigType.error("receive match tcp", None)),
                };
                TcpTransport::<Id>::receive(endpoint, &tcp_config)
            }
            Endpoint::Quic(endpoint) => {
                let quic_config = match config {
                    ConnectionConfig::Quic(conf) => conf,
                    _ => {
                        return Err(PeerNetError::WrongConfigType.error("receive match quic", None))
                    }
                };
                QuicTransport::<Id>::receive(endpoint, &quic_config)
            }
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
        }
    }
}

//TODO: Create trait for endpoint and match naming convention
