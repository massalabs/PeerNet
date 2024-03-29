// All the tests related to the limitations on the system.
mod util;
use parking_lot::RwLock;
use peernet::{
    config::{PeerNetCategoryInfo, PeerNetConfiguration, PeerNetFeatures},
    network_manager::PeerNetManager,
    peer::InitConnectionHandler,
    peer_id::PeerId,
    transports::{endpoint::Endpoint, TcpConnectionConfig, TcpEndpoint, TransportType},
};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::Arc,
    time::Duration,
};
use stream_limiter::Limiter;

// use peernet::types::KeyPair;

use util::{DefaultContext, DefaultMessagesHandler, DefaultPeerId};

use crate::util::get_tcp_port;

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
        read_timeout: Duration::from_secs(10),
        write_timeout: Duration::from_secs(10),
        context,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        max_message_size: 1048576000,
        rate_bucket_size: 60 * 1024,
        rate_limit: 10000,
        rate_time_window: Duration::from_secs(1),
        send_data_channel_size: 1000,
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections: 1,
            max_in_connections_per_ip: 1,
            max_out_connections: 10,
        },
        _phantom: std::marker::PhantomData,
    };

    let mut manager: PeerNetManager<
        DefaultPeerId,
        DefaultContext,
        DefaultInitConnection,
        DefaultMessagesHandler,
    > = PeerNetManager::new(config);

    let port = get_tcp_port(10000..u16::MAX);
    manager
        .start_listener(
            TransportType::Tcp,
            format!("127.0.0.1:{port}").parse().unwrap(),
        )
        .unwrap();

    let context2 = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };
    let config = PeerNetConfiguration {
        read_timeout: Duration::from_secs(10),
        write_timeout: Duration::from_secs(10),
        context: context2,
        max_in_connections: 10,
        send_data_channel_size: 1000,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        max_message_size: 1048576000,
        rate_bucket_size: 60 * 1024,
        rate_limit: 10000,
        rate_time_window: Duration::from_secs(1),
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections: 10,
            max_in_connections_per_ip: 2,
            max_out_connections: 10,
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
            TransportType::Tcp,
            format!("127.0.0.1:{port}").parse().unwrap(),
            Duration::from_secs(3),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));

    let context3 = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };
    let config = PeerNetConfiguration {
        read_timeout: Duration::from_secs(10),
        write_timeout: Duration::from_secs(10),
        context: context3,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        max_message_size: 1048576000,
        rate_bucket_size: 60 * 1024,
        rate_limit: 10000,
        rate_time_window: Duration::from_secs(1),
        send_data_channel_size: 1000,
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections: 10,
            max_in_connections_per_ip: 2,
            max_out_connections: 10,
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
            TransportType::Tcp,
            format!("127.0.0.1:{port}").parse().unwrap(),
            Duration::from_secs(3),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));

    assert_eq!(manager.nb_in_connections(), 1);
    manager
        .stop_listener(
            TransportType::Tcp,
            format!("127.0.0.1:{port}").parse().unwrap(),
        )
        .unwrap();
}

#[test]
fn check_too_much_in_refuse() {
    let context = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };
    let config = PeerNetConfiguration {
        read_timeout: Duration::from_secs(10),
        write_timeout: Duration::from_secs(10),
        context,
        max_in_connections: 1,
        max_message_size: 1048576000,
        rate_bucket_size: 60 * 1024,
        rate_limit: 10000,
        rate_time_window: Duration::from_secs(1),
        send_data_channel_size: 1000,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections: 10,
            max_in_connections_per_ip: 10,
            max_out_connections: 10,
        },
        _phantom: std::marker::PhantomData,
    };
    let mut manager: PeerNetManager<
        DefaultPeerId,
        DefaultContext,
        DefaultInitConnection,
        DefaultMessagesHandler,
    > = PeerNetManager::new(config);

    let port = get_tcp_port(10000..u16::MAX);
    manager
        .start_listener(
            TransportType::Tcp,
            format!("127.0.0.1:{port}").parse().unwrap(),
        )
        .unwrap();

    let context2 = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };
    let config = PeerNetConfiguration {
        read_timeout: Duration::from_secs(10),
        write_timeout: Duration::from_secs(10),
        context: context2,
        max_in_connections: 10,
        send_data_channel_size: 1000,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        max_message_size: 1048576000,
        rate_bucket_size: 60 * 1024,
        rate_limit: 10000,
        rate_time_window: Duration::from_secs(1),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections: 10,
            max_in_connections_per_ip: 2,
            max_out_connections: 10,
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
            TransportType::Tcp,
            format!("127.0.0.1:{port}").parse().unwrap(),
            Duration::from_secs(3),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));

    let context3 = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };
    let config = PeerNetConfiguration {
        read_timeout: Duration::from_secs(10),
        write_timeout: Duration::from_secs(10),
        context: context3,
        max_in_connections: 10,
        send_data_channel_size: 1000,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        max_message_size: 1048576000,
        rate_bucket_size: 60 * 1024,
        rate_limit: 10000,
        rate_time_window: Duration::from_secs(1),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections: 10,
            max_in_connections_per_ip: 2,
            max_out_connections: 10,
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
            TransportType::Tcp,
            format!("127.0.0.1:{port}").parse().unwrap(),
            Duration::from_secs(3),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));

    assert_eq!(manager.nb_in_connections(), 1);
    manager
        .stop_listener(
            TransportType::Tcp,
            format!("127.0.0.1:{port}").parse().unwrap(),
        )
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
                max_in_connections: 1,
                max_in_connections_per_ip: 1,
                max_out_connections: 1,
            },
        ),
    );
    let config = PeerNetConfiguration {
        read_timeout: Duration::from_secs(10),
        write_timeout: Duration::from_secs(10),
        context,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection {},
        max_message_size: 1048576000,
        rate_bucket_size: 60 * 1024,
        rate_limit: 10000,
        rate_time_window: Duration::from_secs(1),
        send_data_channel_size: 1000,
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories,
        default_category_info: PeerNetCategoryInfo {
            max_in_connections: 0,
            max_in_connections_per_ip: 0,
            max_out_connections: 0,
        },
        _phantom: std::marker::PhantomData,
    };

    let mut manager: PeerNetManager<
        DefaultPeerId,
        DefaultContext,
        DefaultInitConnection,
        DefaultMessagesHandler,
    > = PeerNetManager::new(config);
    let port = get_tcp_port(10000..u16::MAX);
    manager
        .start_listener(
            TransportType::Tcp,
            format!("127.0.0.1:{port}").parse().unwrap(),
        )
        .unwrap();

    let context2 = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };
    let config = PeerNetConfiguration {
        read_timeout: Duration::from_secs(10),
        write_timeout: Duration::from_secs(10),
        context: context2,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection {},
        max_message_size: 1048576000,
        rate_bucket_size: 60 * 1024,
        rate_limit: 10000,
        rate_time_window: Duration::from_secs(1),
        send_data_channel_size: 1000,
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections: 10,
            max_in_connections_per_ip: 2,
            max_out_connections: 10,
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
            TransportType::Tcp,
            format!("127.0.0.1:{port}").parse().unwrap(),
            Duration::from_secs(3),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));

    let context3 = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };
    let config = PeerNetConfiguration {
        read_timeout: Duration::from_secs(10),
        write_timeout: Duration::from_secs(10),
        context: context3,
        max_in_connections: 10,
        max_message_size: 1048576000,
        rate_bucket_size: 60 * 1024,
        rate_limit: 10000,
        rate_time_window: Duration::from_secs(1),
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections: 10,
            max_in_connections_per_ip: 2,
            max_out_connections: 10,
        },
        send_data_channel_size: 1000,
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
            TransportType::Tcp,
            format!("127.0.0.1:{port}").parse().unwrap(),
            Duration::from_secs(3),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));

    assert_eq!(manager.nb_in_connections(), 1);
    manager
        .stop_listener(
            TransportType::Tcp,
            format!("127.0.0.1:{port}").parse().unwrap(),
        )
        .unwrap();
}

#[test]
fn max_message_size() {
    let context = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };

    let config = PeerNetConfiguration {
        read_timeout: Duration::from_secs(10),
        write_timeout: Duration::from_secs(10),
        context,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        max_message_size: 40,
        rate_time_window: Duration::from_secs(1),
        rate_bucket_size: 60 * 1024,
        rate_limit: 10000,
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections: 10,
            max_in_connections_per_ip: 2,
            max_out_connections: 10,
        },
        _phantom: std::marker::PhantomData,
        send_data_channel_size: 1000,
    };

    let mut manager: PeerNetManager<
        DefaultPeerId,
        DefaultContext,
        DefaultInitConnection,
        DefaultMessagesHandler,
    > = PeerNetManager::new(config);

    let port = get_tcp_port(10000..u16::MAX);
    manager
        .start_listener(
            TransportType::Tcp,
            format!("127.0.0.1:{port}").parse().unwrap(),
        )
        .unwrap();

    std::thread::sleep(std::time::Duration::from_millis(500));
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let stream = std::net::TcpStream::connect(addr).unwrap();

    let mut endpoint = Endpoint::Tcp(TcpEndpoint {
        config: TcpConnectionConfig {
            rate_time_window: Duration::from_secs(1),
            rate_bucket_size: 60 * 1024,
            rate_limit: 10000,
            data_channel_size: 1000,
            max_message_size: 10,
            read_timeout: Duration::from_secs(10),
            write_timeout: Duration::from_secs(10),
        },
        address: format!("127.0.0.1:{port}").parse().unwrap(),
        stream_limiter: Limiter::new(stream, None, None),
        total_bytes_received: Arc::new(RwLock::new(0)),
        total_bytes_sent: Arc::new(RwLock::new(0)),
        endpoint_bytes_received: Arc::new(RwLock::new(0)),
        endpoint_bytes_sent: Arc::new(RwLock::new(0)),
    });

    std::thread::sleep(std::time::Duration::from_secs(1));
    assert!(manager.nb_in_connections().eq(&1));

    let handle = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        if let Some((_peer_id, conn)) = manager
            .active_connections
            .write()
            .connections
            .iter_mut()
            .next()
        {
            // send msg with 20 bytes length
            conn.endpoint.send::<DefaultPeerId>(&[0; 20]).unwrap();
        }
        manager
    });

    let result = endpoint.receive::<DefaultPeerId>();

    let err = result.unwrap_err();
    assert!(err.to_string().contains("len too long"));

    std::thread::sleep(std::time::Duration::from_secs(1));

    let mut manager = handle.join().unwrap();

    manager
        .stop_listener(
            TransportType::Tcp,
            format!("127.0.0.1:{port}").parse().unwrap(),
        )
        .unwrap();
}

#[test]
fn send_timeout() {
    let context = DefaultContext {
        our_id: DefaultPeerId::generate(),
    };

    let config = PeerNetConfiguration {
        read_timeout: Duration::from_secs(10),
        write_timeout: Duration::from_secs(10),
        context,
        max_in_connections: 10,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default(),
        message_handler: DefaultMessagesHandler {},
        max_message_size: 9000000,
        rate_time_window: Duration::from_secs(1),
        rate_bucket_size: 60 * 1024,
        rate_limit: 1000,
        peers_categories: HashMap::default(),
        default_category_info: PeerNetCategoryInfo {
            max_in_connections: 10,
            max_in_connections_per_ip: 2,
            max_out_connections: 10,
        },
        _phantom: std::marker::PhantomData,
        send_data_channel_size: 1000,
    };

    let mut manager: PeerNetManager<
        DefaultPeerId,
        DefaultContext,
        DefaultInitConnection,
        DefaultMessagesHandler,
    > = PeerNetManager::new(config);

    let port = get_tcp_port(10000..u16::MAX);
    manager
        .start_listener(
            TransportType::Tcp,
            format!("127.0.0.1:{port}").parse().unwrap(),
        )
        .unwrap();

    std::thread::sleep(std::time::Duration::from_millis(500));

    // add connection to the manager
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let stream = std::net::TcpStream::connect(addr).unwrap();
    let _endpoint = Endpoint::Tcp(TcpEndpoint {
        config: TcpConnectionConfig {
            rate_time_window: Duration::from_secs(1),
            rate_bucket_size: 60 * 1024,
            rate_limit: 100,
            data_channel_size: 1000,
            max_message_size: 9000000,
            read_timeout: Duration::from_secs(10),
            write_timeout: Duration::from_secs(10),
        },
        address: format!("127.0.0.1:{port}").parse().unwrap(),
        stream_limiter: Limiter::new(stream, None, None),
        total_bytes_received: Arc::new(RwLock::new(0)),
        total_bytes_sent: Arc::new(RwLock::new(0)),
        endpoint_bytes_received: Arc::new(RwLock::new(0)),
        endpoint_bytes_sent: Arc::new(RwLock::new(0)),
    });

    std::thread::sleep(std::time::Duration::from_secs(1));
    assert!(manager.nb_in_connections().eq(&1));

    if let Some((_peer_id, conn)) = manager
        .active_connections
        .write()
        .connections
        .iter_mut()
        .next()
    {
        // send msg with large data that trigger the timeout
        let result = conn
            .endpoint
            .send_timeout::<DefaultPeerId>(&[0; 9000000], Duration::from_millis(200));
        let err = result.unwrap_err();
        println!("Err: {:?}", err);
        assert!(err.to_string().contains("timeout"));
    }

    manager
        .stop_listener(
            TransportType::Tcp,
            format!("127.0.0.1:{port}").parse().unwrap(),
        )
        .unwrap();
}

// TODO Perform limit tests for QUIC also
