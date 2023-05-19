// All the tests related to the limitations on the system.
mod util;
use peernet::{
    config::{PeerNetCategoryInfo, PeerNetConfiguration, PeerNetFeatures},
    network_manager::PeerNetManager,
    peer::InitConnectionHandler,
    peer_id::PeerId,
    transports::{ConnectionConfig, TransportType},
};
use std::{collections::HashMap, net::IpAddr, str::FromStr, time::Duration};

// use peernet::types::KeyPair;

use util::{DefaultContext, DefaultMessagesHandler, DefaultPeerId};

#[derive(Clone)]
pub struct DefaultInitConnection;
impl InitConnectionHandler<DefaultPeerId, DefaultContext, DefaultMessagesHandler>
    for DefaultInitConnection
{
    fn perform_handshake(
        &mut self,
        _keypair: &DefaultContext,
        _endpoint: &mut peernet::transports::endpoint::Endpoint,
        _listeners: &std::collections::HashMap<std::net::SocketAddr, TransportType>,
        _messages_handler: DefaultMessagesHandler,
    ) -> peernet::error::PeerNetResult<DefaultPeerId> {
        Ok(DefaultPeerId::generate())
    }
}

#[test]
fn check_multiple_connection_refused() {
    let context = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };

    let config = PeerNetConfiguration {
        context: context,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        max_message_size_read: 1048576000,
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 1,
            max_in_connections_post_handshake: 1,
            max_in_connections_per_ip: 1,
        },
        _phantom: std::marker::PhantomData,
    };

    let mut manager: PeerNetManager<
        DefaultPeerId,
        DefaultContext,
        DefaultInitConnection,
        DefaultMessagesHandler,
    > = PeerNetManager::new(config);

    manager
        .start_listener(TransportType::Tcp, "127.0.0.1:8081".parse().unwrap())
        .unwrap();

    let context2 = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };
    let config = PeerNetConfiguration {
        context: context2,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        max_message_size_read: 1048576000,
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 10,
            max_in_connections_post_handshake: 10,
            max_in_connections_per_ip: 2,
        },
        _phantom: std::marker::PhantomData,
    };

    let mut manager2: PeerNetManager<
        DefaultPeerId,
        DefaultContext,
        DefaultInitConnection,
        DefaultMessagesHandler,
    > = PeerNetManager::new(config);
    manager2
        .try_connect(
            "127.0.0.1:8081".parse().unwrap(),
            Duration::from_secs(3),
            &mut ConnectionConfig::Tcp(Box::default()),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));

    let context3 = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };
    let config = PeerNetConfiguration {
        context: context3,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        max_message_size_read: 1048576000,
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 10,
            max_in_connections_post_handshake: 10,
            max_in_connections_per_ip: 2,
        },
        _phantom: std::marker::PhantomData,
    };
    let mut manager3: PeerNetManager<
        DefaultPeerId,
        DefaultContext,
        DefaultInitConnection,
        DefaultMessagesHandler,
    > = PeerNetManager::new(config);
    manager3
        .try_connect(
            "127.0.0.1:8081".parse().unwrap(),
            Duration::from_secs(3),
            &mut ConnectionConfig::Tcp(Box::default()),
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
    let context = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };
    let config = PeerNetConfiguration {
        context: context,
        max_in_connections: 1,
        max_message_size_read: 1048576000,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 10,
            max_in_connections_post_handshake: 10,
            max_in_connections_per_ip: 10,
        },
        _phantom: std::marker::PhantomData,
    };
    let mut manager: PeerNetManager<
        DefaultPeerId,
        DefaultContext,
        DefaultInitConnection,
        DefaultMessagesHandler,
    > = PeerNetManager::new(config);

    manager
        .start_listener(TransportType::Tcp, "127.0.0.1:8080".parse().unwrap())
        .unwrap();

    let context2 = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };
    let config = PeerNetConfiguration {
        context: context2,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        max_message_size_read: 1048576000,
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 10,
            max_in_connections_post_handshake: 10,
            max_in_connections_per_ip: 2,
        },
        _phantom: std::marker::PhantomData,
    };

    let mut manager2: PeerNetManager<
        DefaultPeerId,
        DefaultContext,
        DefaultInitConnection,
        DefaultMessagesHandler,
    > = PeerNetManager::new(config);
    manager2
        .try_connect(
            "127.0.0.1:8080".parse().unwrap(),
            Duration::from_secs(3),
            &mut ConnectionConfig::Tcp(Box::default()),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));

    let context3 = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };
    let config = PeerNetConfiguration {
        context: context3,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        max_message_size_read: 1048576000,
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 10,
            max_in_connections_post_handshake: 10,
            max_in_connections_per_ip: 2,
        },
        _phantom: std::marker::PhantomData,
    };

    let mut manager3: PeerNetManager<
        DefaultPeerId,
        DefaultContext,
        DefaultInitConnection,
        DefaultMessagesHandler,
    > = PeerNetManager::new(config);
    manager3
        .try_connect(
            "127.0.0.1:8080".parse().unwrap(),
            Duration::from_secs(3),
            &mut ConnectionConfig::Tcp(Box::default()),
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
    let context = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };
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
        context: context,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection {},
        max_message_size_read: 1048576000,
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories,
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 0,
            max_in_connections_post_handshake: 0,
            max_in_connections_per_ip: 0,
        },
        _phantom: std::marker::PhantomData,
    };

    let mut manager: PeerNetManager<
        DefaultPeerId,
        DefaultContext,
        DefaultInitConnection,
        DefaultMessagesHandler,
    > = PeerNetManager::new(config);
    manager
        .start_listener(TransportType::Tcp, "127.0.0.1:8082".parse().unwrap())
        .unwrap();

    let context2 = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };
    let config = PeerNetConfiguration {
        context: context2,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection {},
        max_message_size_read: 1048576000,
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 10,
            max_in_connections_post_handshake: 10,
            max_in_connections_per_ip: 2,
        },
        _phantom: std::marker::PhantomData,
    };

    let mut manager2: PeerNetManager<
        DefaultPeerId,
        DefaultContext,
        DefaultInitConnection,
        DefaultMessagesHandler,
    > = PeerNetManager::new(config);
    manager2
        .try_connect(
            "127.0.0.1:8082".parse().unwrap(),
            Duration::from_secs(3),
            &mut ConnectionConfig::Tcp(Box::default()),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));

    let context3 = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };
    let config = PeerNetConfiguration {
        context: context3,
        max_in_connections: 10,
        max_message_size_read: 1048576000,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 10,
            max_in_connections_post_handshake: 10,
            max_in_connections_per_ip: 2,
        },
        _phantom: std::marker::PhantomData,
    };

    let mut manager3: PeerNetManager<
        DefaultPeerId,
        DefaultContext,
        DefaultInitConnection,
        DefaultMessagesHandler,
    > = PeerNetManager::new(config);
    manager3
        .try_connect(
            "127.0.0.1:8082".parse().unwrap(),
            Duration::from_secs(3),
            &mut ConnectionConfig::Tcp(Box::default()),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));

    assert_eq!(manager.nb_in_connections(), 1);
    manager
        .stop_listener(TransportType::Tcp, "127.0.0.1:8082".parse().unwrap())
        .unwrap();
}

// TODO Perform limit tests for QUIC also
