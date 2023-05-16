// All the tests related to the limitations on the system.
mod util;
use peernet::{
    config::{PeerNetCategoryInfo, PeerNetConfiguration, PeerNetFeatures},
    network_manager::PeerNetManager,
    peer::InitConnectionHandler,
    transports::{OutConnectionConfig, TransportType},
    types::{PeerNetHash, PeerNetId, PeerNetKeyPair, PeerNetPubKey, PeerNetSignature},
};
use std::{collections::HashMap, net::IpAddr, str::FromStr, time::Duration};

// use peernet::types::KeyPair;

use util::{DefaultMessagesHandler, TestKeyPair};

use crate::util::{TestHasher, TestId, TestPubKey, TestSignature};

#[derive(Clone)]
pub struct DefaultInitConnection;
impl InitConnectionHandler for DefaultInitConnection {
    fn perform_handshake<
        M: peernet::messages::MessagesHandler,
        Id: PeerNetId,
        K: PeerNetKeyPair<PubKey>,
        S: PeerNetSignature,
        PubKey: PeerNetPubKey,
        Hasher: PeerNetHash,
    >(
        &mut self,
        _keypair: &K,
        _endpoint: &mut peernet::transports::endpoint::Endpoint,
        _listeners: &HashMap<std::net::SocketAddr, TransportType>,
        _messages_handler: M,
    ) -> peernet::error::PeerNetResult<Id> {
        let keypair = TestKeyPair::generate();
        Ok(Id::from_public_key(keypair.get_public_key()))
    }
}

#[test]
fn check_multiple_connection_refused() {
    let keypair1 = TestKeyPair::generate();
    let pub_key = keypair1.get_public_key();
    let config = PeerNetConfiguration {
        self_keypair: keypair1,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 1,
            max_in_connections_post_handshake: 1,
            max_in_connections_per_ip: 1,
        },
        public_key: pub_key,
    };

    let mut manager: PeerNetManager<
        DefaultInitConnection,
        DefaultMessagesHandler,
        TestId,
        TestKeyPair,
        TestPubKey,
    > = PeerNetManager::new(config);

    manager
        .start_listener::<TestHasher, TestSignature>(
            TransportType::Tcp,
            "127.0.0.1:8081".parse().unwrap(),
        )
        .unwrap();

    let keypair2 = TestKeyPair::generate();
    let pub_key2 = keypair2.get_public_key();
    let config = PeerNetConfiguration {
        self_keypair: keypair2.clone(),
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 10,
            max_in_connections_post_handshake: 10,
            max_in_connections_per_ip: 2,
        },
        public_key: pub_key2,
    };

    let mut manager2: PeerNetManager<
        DefaultInitConnection,
        DefaultMessagesHandler,
        TestId,
        TestKeyPair,
        TestPubKey,
    > = PeerNetManager::new(config);
    manager2
        .try_connect::<TestHasher, TestSignature>(
            "127.0.0.1:8081".parse().unwrap(),
            Duration::from_secs(3),
            &mut OutConnectionConfig::Tcp(Box::default()),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));

    let keypair3 = TestKeyPair::generate();
    let pub_key3 = keypair3.get_public_key();
    let config = PeerNetConfiguration {
        self_keypair: keypair3,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 10,
            max_in_connections_post_handshake: 10,
            max_in_connections_per_ip: 2,
        },
        public_key: pub_key3,
    };
    let mut manager3: PeerNetManager<
        DefaultInitConnection,
        DefaultMessagesHandler,
        TestId,
        TestKeyPair,
        TestPubKey,
    > = PeerNetManager::new(config);
    manager3
        .try_connect::<TestHasher, TestSignature>(
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
fn check_too_much_in_refuse() {
    let keypair1 = TestKeyPair::generate();
    let pub_key = keypair1.get_public_key();
    let config = PeerNetConfiguration {
        self_keypair: keypair1,
        max_in_connections: 1,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 10,
            max_in_connections_post_handshake: 10,
            max_in_connections_per_ip: 10,
        },
        public_key: pub_key,
    };
    let mut manager: PeerNetManager<
        DefaultInitConnection,
        DefaultMessagesHandler,
        TestId,
        TestKeyPair,
        TestPubKey,
    > = PeerNetManager::new(config);

    manager
        .start_listener::<TestHasher, TestSignature>(
            TransportType::Tcp,
            "127.0.0.1:8080".parse().unwrap(),
        )
        .unwrap();

    let keypair2 = TestKeyPair::generate();
    let pub_key2 = keypair2.get_public_key();
    let config = PeerNetConfiguration {
        self_keypair: keypair2.clone(),
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 10,
            max_in_connections_post_handshake: 10,
            max_in_connections_per_ip: 2,
        },
        public_key: pub_key2,
    };

    let mut manager2: PeerNetManager<
        DefaultInitConnection,
        DefaultMessagesHandler,
        TestId,
        TestKeyPair,
        TestPubKey,
    > = PeerNetManager::new(config);
    manager2
        .try_connect::<TestHasher, TestSignature>(
            "127.0.0.1:8080".parse().unwrap(),
            Duration::from_secs(3),
            &mut OutConnectionConfig::Tcp(Box::default()),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));

    let keypair3 = TestKeyPair::generate();
    let pub_key3 = keypair3.get_public_key();
    let config = PeerNetConfiguration {
        self_keypair: keypair3,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 10,
            max_in_connections_post_handshake: 10,
            max_in_connections_per_ip: 2,
        },
        public_key: pub_key3,
    };
    let mut manager3: PeerNetManager<
        DefaultInitConnection,
        DefaultMessagesHandler,
        TestId,
        TestKeyPair,
        TestPubKey,
    > = PeerNetManager::new(config);
    manager3
        .try_connect::<TestHasher, TestSignature>(
            "127.0.0.1:8080".parse().unwrap(),
            Duration::from_secs(3),
            &mut OutConnectionConfig::Tcp(Box::default()),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));

    assert_eq!(manager.nb_in_connections(), 1);
    manager
        .stop_listener(TransportType::Tcp, "127.0.0.1:8080".parse().unwrap())
        .unwrap();
}

#[test]
fn check_multiple_connection_refused_in_category() {
    let keypair1 = TestKeyPair::generate();
    let pub_key = keypair1.get_public_key();
    let mut peers_categories = HashMap::default();
    peers_categories.insert(
        String::from("Bootstrap"),
        (
            vec![IpAddr::from_str("127.0.0.1").unwrap()],
            PeerNetCategoryInfo {
                max_in_connections_pre_handshake: 1,
                max_in_connections_post_handshake: 1,
                max_in_connections_per_ip: 1,
            },
        ),
    );
    let config = PeerNetConfiguration {
        self_keypair: keypair1,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories,
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 0,
            max_in_connections_post_handshake: 0,
            max_in_connections_per_ip: 0,
        },
        public_key: pub_key,
    };
    let mut manager: PeerNetManager<
        DefaultInitConnection,
        DefaultMessagesHandler,
        TestId,
        TestKeyPair,
        TestPubKey,
    > = PeerNetManager::new(config);
    manager
        .start_listener::<TestHasher, TestSignature>(
            TransportType::Tcp,
            "127.0.0.1:8082".parse().unwrap(),
        )
        .unwrap();

    let keypair2 = TestKeyPair::generate();
    let pub_key2 = keypair2.get_public_key();
    let config = PeerNetConfiguration {
        self_keypair: keypair2.clone(),
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 10,
            max_in_connections_post_handshake: 10,
            max_in_connections_per_ip: 2,
        },
        public_key: pub_key2.clone(),
    };
    let mut manager2: PeerNetManager<
        DefaultInitConnection,
        DefaultMessagesHandler,
        TestId,
        TestKeyPair,
        TestPubKey,
    > = PeerNetManager::new(config);
    manager2
        .try_connect::<TestHasher, TestSignature>(
            "127.0.0.1:8082".parse().unwrap(),
            Duration::from_secs(3),
            &mut OutConnectionConfig::Tcp(Box::default()),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));

    let config = PeerNetConfiguration {
        self_keypair: keypair2,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 10,
            max_in_connections_post_handshake: 10,
            max_in_connections_per_ip: 2,
        },
        public_key: pub_key2,
    };
    let mut manager3: PeerNetManager<
        DefaultInitConnection,
        DefaultMessagesHandler,
        TestId,
        TestKeyPair,
        TestPubKey,
    > = PeerNetManager::new(config);
    manager3
        .try_connect::<TestHasher, TestSignature>(
            "127.0.0.1:8082".parse().unwrap(),
            Duration::from_secs(3),
            &mut OutConnectionConfig::Tcp(Box::default()),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));

    assert_eq!(manager.nb_in_connections(), 1);
    manager
        .stop_listener(TransportType::Tcp, "127.0.0.1:8082".parse().unwrap())
        .unwrap();
}

// TODO Perform limit tests for QUIC also
