//! Definition of the PeerId type

use massa_signature::PublicKey;

/// Representation of a peer id
#[derive(Clone, Debug, Eq, PartialEq)]
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

    /// Convert the PeerId to a byte array
    pub fn to_bytes(&self) -> Vec<u8> {
        self.public_key.to_bytes().to_vec()
    }
}
