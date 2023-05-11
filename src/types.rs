use std::fmt::Debug;
use std::fmt::Display;
use std::hash::Hash as HashTrait;

use crate::error::PeerNetResult;

// pub const PUBLIC_KEY_SIZE_BYTES: usize = massa_signature::PUBLIC_KEY_SIZE_BYTES;

pub trait PeerNetKeyPair: Send + Sync + Clone + Debug + Display + 'static {
    fn get_public_key<K: PeerNetPubKey>(&self) -> K;
    fn sign<S: PeerNetSignature, H: PeerNetHasher>(&self, hash: &H) -> PeerNetResult<S>;
    fn to_bytes(&self) -> Vec<u8>;
    // fn from_bytes(bytes: &[u8]) -> Self;
}

/// Trait to implement with generic ID
pub trait PeerNetId: PartialEq + Eq + HashTrait + Debug + Clone + Send + Sync + 'static {
    // fn from_bytes(bytes: &[u8; PUBLIC_KEY_SIZE_BYTES]) -> PeerNetResult<Self>
    // where
    //     Self: Sized;
    fn verify_signature<S: PeerNetSignature, H: PeerNetHasher>(
        &self,
        hash: &H,
        signature: &S,
    ) -> PeerNetResult<()>;

    fn from_public_key<K: PeerNetPubKey>(public_key: K) -> Self;
}

pub trait PeerNetPubKey: Clone {
    fn to_bytes(&self) -> &[u8];
    fn from_bytes(bytes: &[u8]) -> PeerNetResult<Self>;
}

pub trait PeerNetSignature: Debug + Clone {
    fn to_bytes(&self) -> Vec<u8>;
    fn from_bytes(bytes: &[u8]) -> PeerNetResult<Self>;
}

pub trait PeerNetHasher: Debug {
    fn compute_from(data: &[u8]) -> Self;
}
