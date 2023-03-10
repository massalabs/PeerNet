//! Definition of the PeerId type

use massa_hash::Hash;
use massa_signature::{PublicKey, PUBLIC_KEY_SIZE_BYTES, Signature};

use crate::error::PeerNetError;

/// Representation of a peer id
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PeerId {
    /// The public key of the peer
    /// TODO: Offer multiple possibilities
    public_key: PublicKey,
}

impl PeerId {
    /// Create a new PeerId from a public key
    pub fn from_public_key(public_key: PublicKey) -> PeerId {
        PeerId { public_key }
    }

    /// Create a new PeerId from a byte array
    pub fn from_bytes(bytes: &[u8; PUBLIC_KEY_SIZE_BYTES]) -> Result<PeerId, PeerNetError> {
        Ok(PeerId {
            public_key: PublicKey::from_bytes(bytes)
                .map_err(|err| PeerNetError::PeerIdError(err.to_string()))?,
        })
    }

    /// Convert the PeerId to a byte array
    pub fn to_bytes(&self) -> Vec<u8> {
        self.public_key.to_bytes().to_vec()
    }

    /// Verify a signature
    pub fn verify_signature(
        &self,
        hash: &Hash,
        signature: &Signature,
    ) -> Result<(), PeerNetError> {
        self.public_key
            .verify_signature(hash, signature)
            .map_err(|err| PeerNetError::PeerIdError(err.to_string()))
    }
}
