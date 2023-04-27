// All the tests related to the limitations on the system.
mod util;
use peernet::{
    config::{PeerNetConfiguration, PeerNetFeatures},
    network_manager::PeerNetManager,
    peer::InitConnectionHandler,
    transports::{OutConnectionConfig, TransportType},
};
use std::time::Duration;

use peernet::types::KeyPair;

use util::DefaultMessagesHandler;

#[derive(Clone)]
pub struct DefaultInitConnection;
impl InitConnectionHandler for DefaultInitConnection {}

#[test]
fn check_mutliple_connection_refused() {
    let keypair1 = KeyPair::generate();
    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        self_keypair: keypair1,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default().set_reject_same_ip_addr(true),
        message_handler: DefaultMessagesHandler {},
    };
    let mut manager = PeerNetManager::new(config);
    manager
        .start_listener(TransportType::Tcp, "127.0.0.1:8081".parse().unwrap())
        .unwrap();

    let keypair2 = KeyPair::generate();
    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        self_keypair: keypair2.clone(),
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default().set_reject_same_ip_addr(true),
        message_handler: DefaultMessagesHandler {},
    };
    let mut manager2 = PeerNetManager::new(config);
    manager2
        .try_connect(
            "127.0.0.1:8081".parse().unwrap(),
            Duration::from_secs(3),
            &mut OutConnectionConfig::Tcp(Box::default()),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));

    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        self_keypair: keypair2,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default().set_reject_same_ip_addr(true),
        message_handler: DefaultMessagesHandler {},
    };
    let mut manager3 = PeerNetManager::new(config);
    manager3
        .try_connect(
            "127.0.0.1:8081".parse().unwrap(),
            Duration::from_secs(3),
            &mut OutConnectionConfig::Tcp(Box::default()),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));

    assert_eq!(manager.nb_in_connections(), 1);
    manager
        .stop_listener(TransportType::Tcp, "127.0.0.1:8081".parse().unwrap())
        .unwrap();
}

// TODO Perform limit tests for QUIC also
