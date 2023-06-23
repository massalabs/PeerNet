//! Configuration for the PerNet manager.
//!
//! This module contains the configuration for the PeerNet manager.
//! It regroups all the information needed to initialize a PeerNet manager.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::context::Context;
use crate::messages::MessagesHandler;
use crate::peer::InitConnectionHandler;
use crate::peer_id::PeerId;

#[derive(Clone, Copy, Default, Debug, Serialize, Deserialize)]
pub struct PeerNetCategoryInfo {
    pub max_in_connections: usize,
    pub max_in_connections_per_ip: usize,
}

pub type PeerNetCategories = HashMap<String, (Vec<IpAddr>, PeerNetCategoryInfo)>;

/// Struct containing the configuration for the PeerNet manager.
pub struct PeerNetConfiguration<
    Id: PeerId,
    Ctx: Context<Id>,
    I: InitConnectionHandler<Id, Ctx, M>,
    M: MessagesHandler<Id>,
> {
    /// Context that can save useful data such as our private key or our peer_id
    pub context: Ctx,
    /// Optional function to trigger at handshake
    /// (local keypair, endpoint to the peer, remote peer_id, active_connections)
    pub init_connection_handler: I,
    /// Optional features to enable for the manager
    pub optional_features: PeerNetFeatures,
    /// Structure for message handler
    pub message_handler: M,
    /// Maximum number of in connections if we have more we just don't accept the connection
    pub max_in_connections: usize,
    /// Maximum size of a message that we can read
    pub max_message_size: usize,
    /// Size of send data channel
    pub send_data_channel_size: usize,
    /// Number of bytes we can read/write in `rate_time_window`
    pub rate_limit: u128,
    /// Window of time we wait between each read/write
    pub rate_time_window: Duration,
    /// Maximum tokens store in the Limiter. Refer to the stream_limiter crate documentation.
    pub rate_bucket_size: usize,
    /// List of categories of peers
    pub peers_categories: PeerNetCategories,
    /// Default category info for all peers not in a specific category (category info, number of connections accepted only for handshake //TODO: Remove when refactored on massa side)
    pub default_category_info: PeerNetCategoryInfo,
    pub _phantom: std::marker::PhantomData<Id>,
}

impl<
        Id: PeerId,
        Ctx: Context<Id>,
        I: InitConnectionHandler<Id, Ctx, M>,
        M: MessagesHandler<Id>,
    > PeerNetConfiguration<Id, Ctx, I, M>
{
    pub fn default(init_connection_handler: I, message_handler: M, context: Ctx) -> Self {
        PeerNetConfiguration {
            context,
            max_in_connections: 10,
            init_connection_handler,
            optional_features: PeerNetFeatures::default(),
            message_handler,
            peers_categories: HashMap::new(),
            max_message_size: 1048576000,
            send_data_channel_size: 10000,
            default_category_info: PeerNetCategoryInfo {
                max_in_connections: 0,
                max_in_connections_per_ip: 0,
            },
            rate_time_window: Duration::from_secs(1),
            rate_bucket_size: 10000,
            rate_limit: 100000,
            _phantom: std::marker::PhantomData,
        }
    }
}

#[derive(Clone, Default)]
pub struct PeerNetFeatures {}
