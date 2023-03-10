mod util;
use std::{collections::HashMap, time::Duration};

use massa_signature::KeyPair;
use peernet::{
    config::PeerNetConfiguration,
    handlers::MessageHandlers,
    internal_handlers::peer_management::{
        fallback_function, handshake, InitialPeers, PeerManagementHandler,
    },
    network_manager::PeerNetManager,
    peer_id::PeerId,
    transports::{OutConnectionConfig, TcpOutConnectionConfig, TransportType},
};
use util::create_basic_handler;

#[test]
fn two_peers_tcp_with_one_handler() {
    let keypair2 = KeyPair::generate();
    let (receiver, handler) = create_basic_handler();
    let keypair2_clone = keypair2.clone();
    std::thread::spawn(move || {
        let (peer_id, message) = receiver.recv().unwrap();
        assert_eq!(
            peer_id,
            PeerId::from_public_key(keypair2_clone.get_public_key())
        );
        assert_eq!(message, vec![1, 2, 3]);
        println!("Well received")
    });
    let mut message_handlers: MessageHandlers = Default::default();
    message_handlers.add_handler(0, handler);
    let keypair1 = KeyPair::generate();
    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        self_keypair: keypair1.clone(),
        handshake_function: None,
        fallback_function: None,
        message_handlers: message_handlers.clone(),
    };
    let mut manager = PeerNetManager::new(config);
    manager
        .start_listener(TransportType::Tcp, "127.0.0.1:8081".parse().unwrap())
        .unwrap();

    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        self_keypair: keypair2.clone(),
        handshake_function: None,
        fallback_function: None,
        message_handlers,
    };
    let mut manager2 = PeerNetManager::new(config);
    manager2
        .try_connect(
            "127.0.0.1:8081".parse().unwrap(),
            Duration::from_secs(3),
            &mut OutConnectionConfig::Tcp(TcpOutConnectionConfig {}),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(1));
    let active_connections = manager2.active_connections.clone();
    {
        let mut connections = active_connections.write();
        for (peer_id, connection) in connections.connections.iter_mut() {
            println!("Sending message to {:?}", peer_id);
            connection
                .send_channels
                .send(0, vec![1, 2, 3], true)
                .unwrap();
        }
        println!("Connections: {:?}", connections);
    }
    std::thread::sleep(std::time::Duration::from_secs(5));
    manager
        .stop_listener(TransportType::Tcp, "127.0.0.1:8081".parse().unwrap())
        .unwrap();
}

#[test]
fn two_peers_tcp_with_peer_management_handler() {
    let keypair2 = KeyPair::generate();
    let (handler, message_handler) = PeerManagementHandler::new(Default::default());
    let mut message_handlers: MessageHandlers = Default::default();
    message_handlers.add_handler(0, message_handler);
    let keypair1 = KeyPair::generate();
    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        self_keypair: keypair1.clone(),
        handshake_function: Some(&handshake),
        fallback_function: Some(&fallback_function),
        message_handlers: message_handlers.clone(),
    };
    let mut manager = PeerNetManager::new(config);
    manager
        .start_listener(TransportType::Tcp, "127.0.0.1:8082".parse().unwrap())
        .unwrap();

    let mut initial_peers = InitialPeers::new();
    let mut listeners = HashMap::new();
    listeners.insert("127.0.0.1:8082".parse().unwrap(), TransportType::Tcp);
    initial_peers.insert(
        PeerId::from_public_key(keypair1.get_public_key()),
        listeners,
    );
    let (handler, message_handler) = PeerManagementHandler::new(initial_peers);
    let mut message_handlers: MessageHandlers = Default::default();
    message_handlers.add_handler(0, message_handler);
    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        self_keypair: keypair2.clone(),
        handshake_function: Some(&handshake),
        fallback_function: Some(&fallback_function),
        message_handlers,
    };
    let manager2 = PeerNetManager::new(config);
    std::thread::sleep(std::time::Duration::from_secs(10));
    manager
        .stop_listener(TransportType::Tcp, "127.0.0.1:8082".parse().unwrap())
        .unwrap();
}
