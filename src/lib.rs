//! This crate abstracts the network layer of a P2P network and provides a simple interface to connect to other peers.
//! Simple example with two peers on the same code to demonstrate:
//! ```ignore
//! use std::{thread::sleep, time::Duration};
//!
//! use crate::types::KeyPair;
//! use peernet::{
//!    config::PeerNetConfiguration,
//!    network_manager::PeerNetManager,
//!    peer_id::PeerId,
//!    transports::{OutConnectionConfig, TcpOutConnectionConfig, QuicOutConnectionConfig, TransportType},
//!};
//! // Generating a keypair for the first peer
//! let keypair1 = KeyPair::generate();
//! // Setup configuration for the first peer
//! let config = PeerNetConfiguration {
//!     max_in_connections: 10,
//!     max_out_connections: 20,
//!     initial_peer_list: Vec::new(),
//!     self_keypair: keypair1.clone(),
//!     message_handlers: Default::default(),
//! };
//! // Setup the manager for the first peer
//! let mut manager = PeerNetManager::new(config);
//! // Setup the listener for the TCP transport to 8081 port.
//! manager
//!     .start_listener(TransportType::Tcp, "127.0.0.1:8081".parse().unwrap())
//!     .unwrap();

//! // Generating a keypair for the second peer
//! let keypair2 = KeyPair::generate();
//! // Setup configuration for the second peer
//! let config = PeerNetConfiguration {
//!     max_in_connections: 10,
//!     max_out_connections: 20,
//!     initial_peer_list: Vec::new(),
//!     self_keypair: keypair1.clone(),
//!     message_handlers: Default::default(),
//! };
//! // Setup the manager for the second peer
//! let mut manager2 = PeerNetManager::new(config);
//! // Try to connect to the first peer listener on TCP port 8081.
//! manager2
//!     .try_connect(
//!         "127.0.0.1:8081".parse().unwrap(),
//!         Duration::from_secs(3),
//!         &mut OutConnectionConfig::Tcp(TcpOutConnectionConfig { identity: PeerId::from_public_key(keypair2.get_public_key()) }),
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
pub mod handlers;
pub mod network_manager;
pub mod peer;
pub mod peer_id;
pub mod transports;
pub mod types;
