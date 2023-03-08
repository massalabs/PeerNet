use massa_hash::Hash;
use massa_signature::{KeyPair, PublicKey, Signature};
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};

use crate::error::PeerNetError;

use super::quic::QuicEndpoint;
use super::tcp::TcpEndpoint;
use super::{quic::QuicTransport, tcp::TcpTransport, Transport};

pub(crate) enum Endpoint {
    Tcp(TcpEndpoint),
    Quic(QuicEndpoint),
}

impl Endpoint {
    pub(crate) fn send(&mut self, data: &[u8]) -> Result<(), PeerNetError> {
        match self {
            Endpoint::Tcp(endpoint) => TcpTransport::send(endpoint, data),
            Endpoint::Quic(endpoint) => QuicTransport::send(endpoint, data),
        }
    }
    pub(crate) fn receive(&mut self) -> Result<Vec<u8>, PeerNetError> {
        match self {
            Endpoint::Tcp(endpoint) => TcpTransport::receive(endpoint),
            Endpoint::Quic(endpoint) => QuicTransport::receive(endpoint),
        }
    }

    pub(crate) fn handshake(&mut self, self_keypair: &KeyPair) -> Result<(), PeerNetError> {
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
            .map_err(|err| PeerNetError::HandshakeError(err.to_string()))?;
        println!("Handshake finished");
        Ok(())
    }
}
