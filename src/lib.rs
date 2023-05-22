//! This crate abstracts the network layer of a P2P network and provides a simple interface to connect to other peers.
//! Simple example with two peers on the same code to demonstrate:
//! ``` rust
//! use std::{thread::sleep, collections::HashMap, time::Duration};
//! use peernet::{
//!     context::Context, error::PeerNetResult, messages::MessagesHandler, peer_id::PeerId,
//!     config::{PeerNetConfiguration, PeerNetFeatures, PeerNetCategoryInfo},
//!     network_manager::PeerNetManager,
//!     peer::InitConnectionHandler,
//!     transports::{TransportType, ConnectionConfig},
//! };
//!
//! use rand::Rng;
//! #[derive(Clone)]
//! pub struct DefaultContext {
//!     pub our_id: DefaultPeerId,
//! }
//!
//! #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
//! pub struct DefaultPeerId {
//!     pub id: u64,
//! }
//!
//! impl PeerId for DefaultPeerId {
//!     fn generate() -> Self {
//!         let mut rng = rand::thread_rng();
//!         let random_number: u64 = rng.gen();
//!         DefaultPeerId { id: random_number }
//!     }
//! }
//!
//! impl Context<DefaultPeerId> for DefaultContext {
//!     fn get_peer_id(&self) -> DefaultPeerId {
//!         self.our_id.clone()
//!     }
//! }
//!
//! #[derive(Clone)]
//! pub struct DefaultMessagesHandler {}
//!
//! impl MessagesHandler<DefaultPeerId> for DefaultMessagesHandler {
//!     fn deserialize_id<'a>(
//!         &self,
//!         data: &'a [u8],
//!         _peer_id: &DefaultPeerId,
//!     ) -> PeerNetResult<(&'a [u8], u64)> {
//!         Ok((data, 0))
//!     }
//!
//!     fn handle(&self, _id: u64, _data: &[u8], _peer_id: &DefaultPeerId) -> PeerNetResult<()> {
//!         Ok(())
//!     }
//! }
//!
//! #[derive(Clone)]
//! pub struct DefaultInitConnection;
//! impl InitConnectionHandler<DefaultPeerId, DefaultContext, DefaultMessagesHandler>
//!     for DefaultInitConnection
//! {
//!     fn perform_handshake(
//!         &mut self,
//!         _keypair: &DefaultContext,
//!         _endpoint: &mut peernet::transports::endpoint::Endpoint,
//!         _listeners: &std::collections::HashMap<std::net::SocketAddr, TransportType>,
//!         _messages_handler: DefaultMessagesHandler,
//!     ) -> peernet::error::PeerNetResult<DefaultPeerId> {
//!         Ok(DefaultPeerId::generate())
//!     }
//! }
//!
//!
//! // Generating a context for the first peer
//! let context = DefaultContext {
//!   our_id: DefaultPeerId::generate(),
//! };
//! // Setup configuration for the first peer
//! let config = PeerNetConfiguration {
//!     context: context,
//!     max_in_connections: 10,
//!     max_message_size_read: 1048576000,
//!     data_channel_size: 1000,
//!     init_connection_handler: DefaultInitConnection,
//!     optional_features: PeerNetFeatures::default(),
//!     message_handler: DefaultMessagesHandler {},
//!     peers_categories: HashMap::default(),
//!     default_category_info: PeerNetCategoryInfo {
//!         max_in_connections_pre_handshake: 10,
//!         max_in_connections_post_handshake: 10,
//!         max_in_connections_per_ip: 10,
//!     },
//!     _phantom: std::marker::PhantomData,
//! };
//! // Setup the manager for the first peer
//! let mut manager: PeerNetManager<
//! DefaultPeerId,
//! DefaultContext,
//! DefaultInitConnection,
//! DefaultMessagesHandler,
//! > = PeerNetManager::new(config);
//! // Setup the listener for the TCP transport to 8081 port.
//! manager
//!     .start_listener(TransportType::Tcp, "127.0.0.1:8081".parse().unwrap())
//!     .unwrap();
//!

//! // Generating a context for the second peer
//! let context2 = DefaultContext {
//!   our_id: DefaultPeerId::generate(),
//! };
//! // Setup configuration for the second peer
//! let config = PeerNetConfiguration {
//!     context: context2,
//!     max_in_connections: 10,
//!     data_channel_size: 1000,
//!     max_message_size_read: 1048576000,
//!     message_handler: DefaultMessagesHandler {},
//!     init_connection_handler: DefaultInitConnection,
//!     optional_features: PeerNetFeatures::default(),
//!     peers_categories: HashMap::default(),
//!     default_category_info: PeerNetCategoryInfo {
//!         max_in_connections_pre_handshake: 10,
//!         max_in_connections_post_handshake: 10,
//!         max_in_connections_per_ip: 10,
//!     },
//!     _phantom: std::marker::PhantomData,
//! };
//! // Setup the manager for the second peer
//! let mut manager2: PeerNetManager<
//! DefaultPeerId,
//! DefaultContext,
//! DefaultInitConnection,
//! DefaultMessagesHandler,
//! > = PeerNetManager::new(config);
//! // Try to connect to the first peer listener on TCP port 8081.
//! manager2
//!     .try_connect(
//!         "127.0.0.1:8081".parse().unwrap(),
//!         Duration::from_secs(3),
//!         &mut ConnectionConfig::Tcp(Box::default()),
//!     )
//!     .unwrap();
//! std::thread::sleep(std::time::Duration::from_secs(3));
//! // Close the listener of the first peer
//! manager
//!     .stop_listener(TransportType::Tcp, "127.0.0.1:8081".parse().unwrap())
//!    .unwrap();
//! ```

pub mod config;
pub mod context;
pub mod error;
pub mod messages;
pub mod network_manager;
pub mod peer;
pub mod peer_id;
pub mod transports;
