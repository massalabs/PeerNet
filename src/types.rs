use std::fmt::Debug;
use std::fmt::Display;
use std::hash::Hash;

use crate::error::PeerNetResult;

// pub const PUBLIC_KEY_SIZE_BYTES: usize = massa_signature::PUBLIC_KEY_SIZE_BYTES;

pub trait PeerNetKeyPair<K>: Send + Sync + Clone + Debug + Display + 'static
where
    K: PeerNetPubKey,
{
    fn get_public_key(&self) -> K;
    fn sign(&self, hash: &impl PeerNetHasher) -> PeerNetResult<Vec<u8>>;

    //fn to_bytes(&self) -> Vec<u8>;
    // fn from_bytes(bytes: &[u8]) -> Self;
}

/// Trait to implement with generic ID
pub trait PeerNetId: PartialEq + Eq + Hash + Debug + Clone + Send + Sync + Ord + 'static {

    fn to_bytes(&self) -> Vec<u8>;

    fn verify_signature(
        &self,
        hash: &impl PeerNetHasher,
        signature: &impl PeerNetSignature,
    ) -> PeerNetResult<()>;

    fn from_public_key<K: PeerNetPubKey>(key: K) -> Self;
}

pub trait PeerNetPubKey: Clone {
    fn to_bytes(&self) -> Vec<u8>;
    fn from_bytes(bytes: &[u8]) -> PeerNetResult<Self>;
}

pub trait PeerNetSignature: Debug + Clone {
    fn to_bytes(&self) -> Vec<u8>;
    fn from_bytes(bytes: &[u8]) -> PeerNetResult<Self>;
}

pub trait PeerNetHasher: Debug {
    fn compute_from(data: &[u8]) -> Self;
    fn to_bytes(&self) -> Vec<u8>;
}
