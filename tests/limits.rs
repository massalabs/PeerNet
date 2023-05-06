// All the tests related to the limitations on the system.
mod util;
use peernet::{
    config::{PeerNetCategoryInfo, PeerNetConfiguration, PeerNetFeatures},
    network_manager::PeerNetManager,
    peer::InitConnectionHandler,
    peer_id::PeerId,
    transports::{OutConnectionConfig, TransportType},
};
use std::{collections::HashMap, net::IpAddr, str::FromStr, time::Duration};

use peernet::types::KeyPair;

use util::DefaultMessagesHandler;

#[derive(Clone)]
pub struct DefaultInitConnection;
impl InitConnectionHandler for DefaultInitConnection {
    fn perform_handshake<M: peernet::messages::MessagesHandler>(
        &mut self,
        keypair: &KeyPair,
        _endpoint: &mut peernet::transports::endpoint::Endpoint,
        _listeners: &HashMap<std::net::SocketAddr, TransportType>,
        _messages_handler: M,
    ) -> peernet::error::PeerNetResult<peernet::peer_id::PeerId> {
        Ok(PeerId::from_public_key(keypair.get_public_key()))
    }
}

#[test]
fn check_multiple_connection_refused() {
    let keypair1 = KeyPair::generate();
    let config = PeerNetConfiguration {
        self_keypair: keypair1,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections: 1,
            max_in_connections_per_ip: 1,
        },
    };
    let mut manager = PeerNetManager::new(config);
    manager
        .start_listener(TransportType::Tcp, "127.0.0.1:8081".parse().unwrap())
        .unwrap();

    let keypair2 = KeyPair::generate();
    let config = PeerNetConfiguration {
        self_keypair: keypair2.clone(),
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections: 10,
            max_in_connections_per_ip: 2,
        },
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
        self_keypair: keypair2,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections: 10,
            max_in_connections_per_ip: 2,
        },
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

#[test]
fn check_multiple_connection_refused_in_category() {
    let keypair1 = KeyPair::generate();
    let mut peers_categories = HashMap::default();
    peers_categories.insert(
        String::from("Bootstrap"),
        (
            vec![IpAddr::from_str("127.0.0.1").unwrap()],
            PeerNetCategoryInfo {
                max_in_connections: 1,
                max_in_connections_per_ip: 1,
            },
        ),
    );
    let config = PeerNetConfiguration {
        self_keypair: keypair1,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories,
        default_category_info: PeerNetCategoryInfo {
            max_in_connections: 0,
            max_in_connections_per_ip: 0,
        },
    };
    let mut manager = PeerNetManager::new(config);
    manager
        .start_listener(TransportType::Tcp, "127.0.0.1:8081".parse().unwrap())
        .unwrap();

    let keypair2 = KeyPair::generate();
    let config = PeerNetConfiguration {
        self_keypair: keypair2.clone(),
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections: 10,
            max_in_connections_per_ip: 2,
        },
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
        self_keypair: keypair2,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections: 10,
            max_in_connections_per_ip: 2,
        },
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
