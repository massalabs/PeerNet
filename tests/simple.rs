mod util;
use std::collections::HashMap;
use std::{thread::sleep, time::Duration};

use peernet::config::PeerNetCategoryInfo;
use peernet::peer_id::PeerId;
use peernet::{
    config::{PeerNetConfiguration, PeerNetFeatures},
    network_manager::PeerNetManager,
    peer::InitConnectionHandler,
    transports::TransportType,
};
use std::net::IpAddr;
use std::str::FromStr;
use util::{create_clients, DefaultMessagesHandler};

use crate::util::{DefaultContext, DefaultPeerId};

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
fn simple() {
    let context = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };

    let config = PeerNetConfiguration {
        context,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection,
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        send_data_channel_size: 1000,
        max_message_size: 10000,
        rate_bucket_size: 10000,
        rate_limit: 10000,
        rate_time_window: Duration::from_secs(1),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 10,
            max_in_connections_post_handshake: 10,
            max_in_connections_per_ip: 10,
        },
        _phantom: std::marker::PhantomData,
        read_timeout: Duration::from_secs(10),
        write_timeout: Duration::from_secs(10),
    };

    let mut manager: PeerNetManager<
        DefaultPeerId,
        DefaultContext,
        DefaultInitConnection,
        DefaultMessagesHandler,
    > = PeerNetManager::new(config);

    manager
        .start_listener(TransportType::Tcp, "127.0.0.1:64850".parse().unwrap())
        .unwrap();

    //manager.start_listener(TransportType::Quic, "127.0.0.1:64850".parse().unwrap()).unwrap();
    sleep(Duration::from_secs(3));
    let _ = create_clients(11, "127.0.0.1:64850");
    sleep(Duration::from_secs(6));

    // we have max_in_connections = 10
    assert_eq!(manager.nb_in_connections(), 10);

    manager
        .stop_listener(TransportType::Tcp, "127.0.0.1:64850".parse().unwrap())
        .unwrap();
}

#[test]
fn simple_no_place() {
    let context = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };

    let config = PeerNetConfiguration {
        context,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection,
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        send_data_channel_size: 1000,
        max_message_size: 1048576000,
        rate_bucket_size: 10000,
        rate_limit: 10000,
        rate_time_window: Duration::from_secs(1),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 0,
            max_in_connections_post_handshake: 0,
            max_in_connections_per_ip: 1,
        },
        _phantom: std::marker::PhantomData,
        read_timeout: Duration::from_secs(10),
        write_timeout: Duration::from_secs(10),
    };
    let mut manager: PeerNetManager<
        DefaultPeerId,
        DefaultContext,
        DefaultInitConnection,
        DefaultMessagesHandler,
    > = PeerNetManager::new(config);

    manager
        .start_listener(TransportType::Tcp, "127.0.0.1:64851".parse().unwrap())
        .unwrap();
    //manager.start_listener(TransportType::Quic, "127.0.0.1:64850".parse().unwrap()).unwrap();
    sleep(Duration::from_secs(3));
    let _ = create_clients(11, "127.0.0.1:64851");
    sleep(Duration::from_secs(6));

    // we have max_in_connections = 10
    assert_eq!(manager.nb_in_connections(), 0);

    manager
        .stop_listener(TransportType::Tcp, "127.0.0.1:64851".parse().unwrap())
        .unwrap();
}

#[test]
fn simple_no_place_after_handshake() {
    let context = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };

    let config = PeerNetConfiguration {
        context,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection,
        optional_features: PeerNetFeatures::default(),
        max_message_size: 1048576000,
        rate_bucket_size: 10000,
        rate_limit: 10000,
        rate_time_window: Duration::from_secs(1),
        send_data_channel_size: 1000,
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 10,
            max_in_connections_post_handshake: 0,
            max_in_connections_per_ip: 1,
        },
        _phantom: std::marker::PhantomData,
        read_timeout: Duration::from_secs(10),
        write_timeout: Duration::from_secs(10),
    };
    let mut manager: PeerNetManager<
        DefaultPeerId,
        DefaultContext,
        DefaultInitConnection,
        DefaultMessagesHandler,
    > = PeerNetManager::new(config);

    manager
        .start_listener(TransportType::Tcp, "127.0.0.1:64852".parse().unwrap())
        .unwrap();
    //manager.start_listener(TransportType::Quic, "127.0.0.1:64850".parse().unwrap()).unwrap();
    sleep(Duration::from_secs(3));
    let _ = create_clients(11, "127.0.0.1:64852");
    sleep(Duration::from_secs(6));

    // we have max_in_connections = 10
    assert_eq!(manager.nb_in_connections(), 0);

    manager
        .stop_listener(TransportType::Tcp, "127.0.0.1:64852".parse().unwrap())
        .unwrap();
}

#[test]
fn simple_with_different_limit_pre_post_handshake() {
    let context = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };

    let config = PeerNetConfiguration {
        context,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection,
        optional_features: PeerNetFeatures::default(),
        max_message_size: 1048576000,
        rate_bucket_size: 10000,
        read_timeout: Duration::from_secs(10),
        write_timeout: Duration::from_secs(10),
        rate_limit: 10000,
        rate_time_window: Duration::from_secs(1),
        send_data_channel_size: 1000,
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 10,
            max_in_connections_post_handshake: 5,
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
        .start_listener(TransportType::Tcp, "127.0.0.1:64854".parse().unwrap())
        .unwrap();
    //manager.start_listener(TransportType::Quic, "127.0.0.1:64850".parse().unwrap()).unwrap();
    sleep(Duration::from_secs(3));
    let _ = create_clients(11, "127.0.0.1:64854");
    sleep(Duration::from_secs(6));

    // we have max_in_connections = 10
    assert_eq!(manager.nb_in_connections(), 5);

    manager
        .stop_listener(TransportType::Tcp, "127.0.0.1:64854".parse().unwrap())
        .unwrap();
}

#[test]
fn simple_with_category() {
    // let keypair = KeyPair::generate();
    let mut peers_categories = HashMap::default();
    peers_categories.insert(
        String::from("Bootstrap"),
        (
            vec![IpAddr::from_str("127.0.0.1").unwrap()],
            PeerNetCategoryInfo {
                max_in_connections_pre_handshake: 10,
                max_in_connections_post_handshake: 10,
                max_in_connections_per_ip: 10,
            },
        ),
    );
    let context = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };

    let config = PeerNetConfiguration {
        read_timeout: Duration::from_secs(10),
        write_timeout: Duration::from_secs(10),
        context,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection,
        max_message_size: 1048576000,
        rate_bucket_size: 10000,
        rate_limit: 10000,
        rate_time_window: Duration::from_secs(1),
        send_data_channel_size: 1000,
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories,
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 10,
            max_in_connections_post_handshake: 10,
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
        .start_listener(TransportType::Tcp, "127.0.0.1:64859".parse().unwrap())
        .unwrap();
    //manager.start_listener(TransportType::Quic, "127.0.0.1:64850".parse().unwrap()).unwrap();
    sleep(Duration::from_secs(3));
    let _ = create_clients(11, "127.0.0.1:64859");
    sleep(Duration::from_secs(6));

    // we have max_in_connections = 10
    assert_eq!(manager.nb_in_connections(), 10);

    manager
        .stop_listener(TransportType::Tcp, "127.0.0.1:64859".parse().unwrap())
        .unwrap();
}

#[test]
fn two_peers_tcp() {
    let context = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };

    let config = PeerNetConfiguration {
        context,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        max_message_size: 1048576000,
        rate_bucket_size: 10000,
        rate_limit: 10000,
        rate_time_window: Duration::from_secs(1),
        send_data_channel_size: 1000,
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 10,
            max_in_connections_post_handshake: 10,
            max_in_connections_per_ip: 2,
        },
        _phantom: std::marker::PhantomData,
        read_timeout: Duration::from_secs(10),
        write_timeout: Duration::from_secs(10),
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
        max_message_size: 1048576000,
        rate_bucket_size: 10000,
        rate_limit: 10000,
        rate_time_window: Duration::from_secs(1),
        send_data_channel_size: 1000,
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 10,
            max_in_connections_post_handshake: 10,
            max_in_connections_per_ip: 2,
        },
        _phantom: std::marker::PhantomData,
        read_timeout: Duration::from_secs(10),
        write_timeout: Duration::from_secs(10),
    };

    let mut manager2: PeerNetManager<
        DefaultPeerId,
        DefaultContext,
        DefaultInitConnection,
        DefaultMessagesHandler,
    > = PeerNetManager::new(config);
    std::thread::sleep(std::time::Duration::from_secs(1));

    manager2
        .try_connect(
            TransportType::Tcp,
            "127.0.0.1:8081".parse().unwrap(),
            Duration::from_secs(3),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(1));
    assert!(manager.nb_in_connections().eq(&1));
    manager
        .stop_listener(TransportType::Tcp, "127.0.0.1:8081".parse().unwrap())
        .unwrap();
}

// #[test]
// fn two_peers_quic() {
//     let keypair1 = KeyPair::generate();
//     let config = PeerNetConfiguration {
//         max_in_connections: 10,
//         max_out_connections: 20,
//         self_keypair: keypair1.clone(),
//         init_connection_handler: DefaultInitConnection {},
//         message_handler: DefaultMessagesHandler {},
//         optional_features: PeerNetFeatures::default().set_reject_same_ip_addr(false),
//     };
//     let mut manager = PeerNetManager::new(config);
//     manager
//         .start_listener(TransportType::Quic, "127.0.0.1:8082".parse().unwrap())
//         .unwrap();

//     let keypair2 = KeyPair::generate();
//     let config = PeerNetConfiguration {
//         max_in_connections: 10,
//         max_out_connections: 20,
//         self_keypair: keypair2,
//         init_connection_handler: DefaultInitConnection {},
//         optional_features: PeerNetFeatures::default().set_reject_same_ip_addr(false),
//         message_handler: DefaultMessagesHandler {},
//     };
//     let mut manager2 = PeerNetManager::new(config);
//     sleep(Duration::from_secs(3));
//     manager2
//         .try_connect(
//             "127.0.0.1:8082".parse().unwrap(),
//             Duration::from_secs(5),
//             //TODO: Use the one in manager instead of asking. Need a wrapper structure ?
//             &mut OutConnectionConfig::Quic(Box::new(QuicConnectionConfig {
//                 identity: PeerId::from_public_key(keypair1.get_public_key()),
//                 local_addr: "127.0.0.1:8083".parse().unwrap(),
//             })),
//         )
//         .unwrap();
//     std::thread::sleep(std::time::Duration::from_secs(5));
//     manager
//         .stop_listener(TransportType::Quic, "127.0.0.1:8082".parse().unwrap())
//         .unwrap();
// }
