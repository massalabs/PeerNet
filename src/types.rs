use massa_hash::Hash;
pub use massa_signature::KeyPair;
pub use massa_signature::PublicKey;
pub use massa_signature::Signature;
use std::fmt::Debug;
use std::fmt::Display;
use std::hash::Hash as HashTrait;

use crate::error::PeerNetResult;

pub const PUBLIC_KEY_SIZE_BYTES: usize = massa_signature::PUBLIC_KEY_SIZE_BYTES;

pub trait PeerNetKeyPair:
    Send + Sync + Clone + Debug + HashTrait + Eq + PartialEq + Display
{
    fn get_public_key<U>(&self) -> U;
    // fn sign(&self, hash: &Hash) -> PeerNetResult<Signature>;
    fn to_bytes(&self) -> Vec<u8>;
    // fn from_bytes(bytes: &[u8]) -> Self;
}

/// Trait to implement with generic ID
pub trait PeerNetId: PartialEq + Eq + HashTrait + Debug + Clone + Send + Sync {
    fn from_bytes(bytes: &[u8; PUBLIC_KEY_SIZE_BYTES]) -> PeerNetResult<Self>
    where
        Self: Sized;
    fn verify_signature(&self, hash: &Hash, signature: &Signature) -> PeerNetResult<()>;

    fn from_public_key<K>(public_key: K) -> Self;
}
