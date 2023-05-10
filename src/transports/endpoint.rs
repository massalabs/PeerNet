use massa_hash::Hash;
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};

use crate::error::{PeerNetError, PeerNetResult};
use crate::peer_id::{PeerId, PeerNetIdTrait};
use crate::types::{KeyPair, PublicKey, Signature};

use super::quic::QuicEndpoint;
use super::tcp::TcpEndpoint;
use super::{quic::QuicTransport, tcp::TcpTransport, Transport};

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

    pub fn send<Id: PeerNetIdTrait>(&mut self, data: &[u8]) -> PeerNetResult<()> {
        match self {
            Endpoint::Tcp(endpoint) => TcpTransport::<Id>::send(endpoint, data),
            Endpoint::Quic(endpoint) => QuicTransport::<Id>::send(endpoint, data),
        }
    }
    pub fn receive<Id: PeerNetIdTrait>(&mut self) -> PeerNetResult<Vec<u8>> {
        match self {
            Endpoint::Tcp(endpoint) => TcpTransport::<Id>::receive(endpoint),
            Endpoint::Quic(endpoint) => QuicTransport::<Id>::receive(endpoint),
        }
    }

    pub(crate) fn handshake<Id: PeerNetIdTrait>(
        &mut self,
        self_keypair: &KeyPair,
    ) -> PeerNetResult<Id> {
        //TODO: Add version in handshake
        let mut self_random_bytes = [0u8; 32];
        StdRng::from_entropy().fill_bytes(&mut self_random_bytes);
        let self_random_hash = Hash::compute_from(&self_random_bytes);
        let mut buf = [0u8; 64];
        buf[..32].copy_from_slice(&self_random_bytes);
        buf[32..].copy_from_slice(self_keypair.get_public_key().to_bytes());

        self.send(&buf)?;
        let received = self.receive()?;
        let other_random_bytes: &[u8; 32] = received.as_slice()[..32].try_into().unwrap();
        let other_public_key = PublicKey::from_bytes(received[32..].try_into().unwrap()).unwrap();

        // sign their random bytes
        let other_random_hash = Hash::compute_from(other_random_bytes);
        let self_signature = self_keypair.sign(&other_random_hash).unwrap();

        buf.copy_from_slice(&self_signature.to_bytes());

        self.send(&buf)?;
        let received = self.receive()?;

        let other_signature =
            Signature::from_bytes(received.as_slice().try_into().unwrap()).unwrap();

        // check their signature
        other_public_key
            .verify_signature(&self_random_hash, &other_signature)
            .map_err(|err| {
                PeerNetError::HandshakeError.new(
                    "handshake verify signature",
                    err,
                    Some(format!(
                        "hash: {:?}, signature: {:?}",
                        self_random_hash, other_signature
                    )),
                )
            })?;

        let other_peer_id = PeerId::from_public_key(other_public_key);
        Ok(other_peer_id)
    }

    pub fn shutdown(&mut self) {
        match self {
            Endpoint::Tcp(endpoint) => endpoint.shutdown(),
            Endpoint::Quic(endpoint) => endpoint.shutdown(),
        }
    }
}

//TODO: Create trait for endpoint and match naming convention
