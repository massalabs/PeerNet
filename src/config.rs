//! Configuration for the PerNet manager.
//!
//! This module contains the configuration for the PeerNet manager.
//! It regroups all the information needed to initialize a PeerNet manager.

use std::collections::HashMap;
use std::net::IpAddr;

use serde::{Deserialize, Serialize};

use crate::messages::MessagesHandler;
use crate::peer::InitConnectionHandler;
use crate::types::{PeerNetKeyPair, PeerNetPubKey, PeerNetSignature};

#[derive(Clone, Copy, Default, Debug, Serialize, Deserialize)]
pub struct PeerNetCategoryInfo {
    pub max_in_connections_pre_handshake: usize,
    pub max_in_connections_post_handshake: usize,
    pub max_in_connections_per_ip: usize,
}

pub type PeerNetCategories = HashMap<String, (Vec<IpAddr>, PeerNetCategoryInfo)>;

/// Struct containing the configuration for the PeerNet manager.
pub struct PeerNetConfiguration<
    T: InitConnectionHandler,
    M: MessagesHandler,
    K: PeerNetKeyPair<PubKey, S>,
    PubKey: PeerNetPubKey,
    S: PeerNetSignature,
> {
    /// Our peer id
    pub self_keypair: K,
    /// Optional function to trigger at handshake
    /// (local keypair, endpoint to the peer, remote peer_id, active_connections)
    pub init_connection_handler: T,
    /// Optional features to enable for the manager
    pub optional_features: PeerNetFeatures,
    /// Structure for message handler
    pub message_handler: M,
    /// List of categories of peers
    pub peers_categories: PeerNetCategories,
    /// Default category info for all peers not in a specific category (category info, number of connections accepted only for handshake //TODO: Remove when refactored on massa side)
    pub default_category_info: PeerNetCategoryInfo,

    pub public_key: PubKey,

    pub signature: S,
}

impl<
        T: InitConnectionHandler,
        M: MessagesHandler,
        K: PeerNetKeyPair<PubKey, S>,
        PubKey: PeerNetPubKey,
        S: PeerNetSignature,
    > PeerNetConfiguration<T, M, K, PubKey, S>
{
    pub fn default(
        init_connection_handler: T,
        message_handler: M,
        keypair: K,
        public_key: PubKey,
        signature: S,
    ) -> Self {
        PeerNetConfiguration {
            self_keypair: keypair,
            init_connection_handler,
            optional_features: PeerNetFeatures::default(),
            message_handler,
            peers_categories: HashMap::new(),
            default_category_info: PeerNetCategoryInfo {
                max_in_connections_pre_handshake: 0,
                max_in_connections_post_handshake: 0,
                max_in_connections_per_ip: 0,
            },
            public_key,
            signature,
        }
    }
}

#[derive(Clone, Default)]
pub struct PeerNetFeatures {}
