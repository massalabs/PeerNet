//! This crate abstracts the network layer of a P2P network and provides a simple interface to connect to other peers.
//! Simple example with two peers on the same code to demonstrate:
//! ``` rust
//! use std::{thread::sleep, time::Duration};
//!
//! use peernet::{
//!    config::{PeerNetConfiguration, PeerNetFeatures},
//!    network_manager::PeerNetManager,
//!    peer_id::PeerId,
//!    types::KeyPair,
//!    error::PeerNetResult,
//!    messages::MessagesHandler,
//!    transports::{OutConnectionConfig, TcpOutConnectionConfig, QuicOutConnectionConfig, TransportType},
//!    peer::HandshakeHandler,
//!};
//!
//! // Declare a handshake handler to use
//! #[derive(Clone)]
//! pub struct DefaultHandshake;
//! // Use the default implementation of the handshake
//! impl HandshakeHandler for DefaultHandshake {}
//! #[derive(Clone)]
//! pub struct MessageHandler;
//! impl MessagesHandler for MessageHandler {
//!     fn deserialize_id<'a>(&self, data: &'a [u8], _peer_id: &PeerId) -> PeerNetResult<(&'a [u8], u64)> {
//!         Ok((data, 0))
//!     }
//!
//!     fn handle(&self, _id: u64, _data: &[u8], _peer_id: &PeerId) -> PeerNetResult<()> {
//!         Ok(())
//!     }
//! }
//!
//!
//! // Generating a keypair for the first peer
//! let keypair1 = KeyPair::generate();
//! // Setup configuration for the first peer
//! let config = PeerNetConfiguration {
//!     max_in_connections: 10,
//!     max_out_connections: 20,
//!     self_keypair: keypair1.clone(),
//!     message_handler: MessageHandler {},
//!     fallback_function: None,
//!     handshake_handler: DefaultHandshake,
//!     optional_features: PeerNetFeatures::default(),
//! };
//! // Setup the manager for the first peer
//! let mut manager = PeerNetManager::new(config);
//! // Setup the listener for the TCP transport to 8081 port.
//! manager
//!     .start_listener(TransportType::Tcp, "127.0.0.1:8081".parse().unwrap())
//!     .unwrap();
//!

//! // Generating a keypair for the second peer
//! let keypair2 = KeyPair::generate();
//! // Setup configuration for the second peer
//! let config = PeerNetConfiguration {
//!     max_in_connections: 10,
//!     max_out_connections: 20,
//!     self_keypair: keypair1.clone(),
//!     message_handler: MessageHandler {},
//!     fallback_function: None,
//!     handshake_handler: DefaultHandshake,
//!     optional_features: PeerNetFeatures::default(),
//! };
//! // Setup the manager for the second peer
//! let mut manager2 = PeerNetManager::new(config);
//! // Try to connect to the first peer listener on TCP port 8081.
//! manager2
//!     .try_connect(
//!         "127.0.0.1:8081".parse().unwrap(),
//!         Duration::from_secs(3),
//!         &mut OutConnectionConfig::Tcp(Box::new(TcpOutConnectionConfig {})),
//!     )
//!     .unwrap();
//! std::thread::sleep(std::time::Duration::from_secs(3));
//! // Close the listener of the first peer
//! manager
//!     .stop_listener(TransportType::Tcp, "127.0.0.1:8081".parse().unwrap())
//!    .unwrap();
//! ```
//!

pub mod config;
pub mod error;
pub mod messages;
pub mod network_manager;
pub mod peer;
pub mod peer_id;
pub mod transports;
pub mod types;
