use massa_signature::PublicKey;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeerId {
    // The public key of the peer
    // TODO: Offer multiple possibilities
    public_key: PublicKey,
}

impl PeerId {
    pub fn from_public_key(public_key: PublicKey) -> PeerId {
        PeerId { public_key }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.public_key.to_bytes().to_vec()
    }
}
