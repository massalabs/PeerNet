/* //! Definition of the PeerId type

use crate::error::{PeerNetError, PeerNetErrorData, PeerNetResult};
use crate::types::{PeerNetId, PeerNetKeyPair, PublicKey, Signature, PUBLIC_KEY_SIZE_BYTES};
use massa_hash::Hash;
use std::fmt::Debug;

use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

/// Representation of a peer id
#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct PeerId<K: PeerNetKeyPair> {
    /// The public key of the peer
    /// TODO: Offer multiple possibilities
    public_key: K,
}

impl<K: PeerNetKeyPair> PeerNetId for PeerId<K> {
    /// Create a new PeerId from a byte array
    fn from_bytes(bytes: &[u8; PUBLIC_KEY_SIZE_BYTES]) -> PeerNetResult<PeerId<K>> {
        // todo unwrap
        Ok(PeerId {
            public_key: PublicKey::from_bytes(bytes)
                .map_err(|err| {
                    PeerNetError::PeerIdError.new(
                        "peerid pubk from bytes",
                        err,
                        Some(format!("{:?}", bytes)),
                    )
                })
                .unwrap(),
        })
    }
    /// Verify a signature
    fn verify_signature(&self, hash: &Hash, signature: &Signature) -> PeerNetResult<()> {
        self.verify_signature(hash, signature).map_err(|err| {
            PeerNetError::PeerIdError.new(
                "peeid verify sign",
                err,
                Some(format!("hash: {:?}, signature: {:?}", hash, signature)),
            )
        })
    }

    fn from_public_key(public_key: K) -> Self {
        PeerId {
            public_key: public_key,
        }
    }
}

impl<K: PeerNetKeyPair> PeerId<K> {
    /// Create a new PeerId from a public key
    pub fn from_public_key(public_key: K) -> PeerId<K> {
        PeerId { public_key }
    }

    /// Convert the PeerId to a byte array
    pub fn to_bytes(&self) -> Vec<u8> {
        self.public_key.to_bytes().to_vec()
    }

    // /// Create a new PeerId from a byte array
    // pub fn from_bytes(bytes: &[u8; PUBLIC_KEY_SIZE_BYTES]) -> PeerNetResult<PeerId> {
    //     Ok(PeerId {
    //         public_key: PublicKey::from_bytes(bytes).map_err(|err| {
    //             PeerNetError::PeerIdError.new(
    //                 "peerid pubk from bytes",
    //                 err,
    //                 Some(format!("{:?}", bytes)),
    //             )
    //         })?,
    //     })
    // }
    // /// Verify a signature
    // pub fn verify_signature(&self, hash: &Hash, signature: &Signature) -> PeerNetResult<()> {
    //     self.public_key
    //         .verify_signature(hash, signature)
    //         .map_err(|err| {
    //             PeerNetError::PeerIdError.new(
    //                 "peeid verify sign",
    //                 err,
    //                 Some(format!("hash: {:?}, signature: {:?}", hash, signature)),
    //             )
    //         })
    // }
}

impl<K: PeerNetKeyPair> Display for PeerId<K> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.public_key)
    }
}

impl<K: PeerNetKeyPair> FromStr for PeerId<K> {
    type Err = PeerNetErrorData;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // todo unwrap
        Ok(PeerId {
            public_key: PublicKey::from_str(s)
                .map_err(|err| {
                    PeerNetError::PeerIdError.new("peerid from str", err, Some(s.to_string()))
                })
                .unwrap(),
        })
    }
}

impl<K: PeerNetKeyPair> ::serde::Serialize for PeerId<K> {
    /// `::serde::Serialize` trait for `PeerId`
    ///
    /// # Example
    ///
    /// Human readable serialization :
    /// ```
    /// # use massa_signature::KeyPair;
    /// # use serde::{Deserialize, Serialize};
    /// let keypair = KeyPair::generate();
    /// let serialized: String = serde_json::to_string(&keypair.get_public_key()).unwrap();
    /// ```
    ///
    fn serialize<S: ::serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(&self.to_string())
    }
}

impl<'de, K: PeerNetKeyPair> ::serde::Deserialize<'de> for PeerId<K> {
    /// `::serde::Deserialize` trait for `PeerId`
    fn deserialize<D: ::serde::Deserializer<'de>>(d: D) -> Result<PeerId<K>, D::Error> {
        struct Base58CheckVisitor;

        impl<'de> ::serde::de::Visitor<'de> for Base58CheckVisitor {
            type Value = PeerId<K>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("an ASCII base58check string")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: ::serde::de::Error,
            {
                if let Ok(v_str) = std::str::from_utf8(v) {
                    PeerId::from_str(v_str).map_err(E::custom)
                } else {
                    Err(E::invalid_value(::serde::de::Unexpected::Bytes(v), &self))
                }
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: ::serde::de::Error,
            {
                PeerId::from_str(v).map_err(E::custom)
            }
        }
        d.deserialize_str(Base58CheckVisitor)
    }
}
 */
