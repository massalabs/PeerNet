mod util;
use std::time::Duration;

use peernet::types::KeyPair;
use peernet::{
    config::PeerNetConfiguration,
    handlers::MessageHandlers,
    network_manager::PeerNetManager,
    peer::HandshakeHandler,
    peer_id::PeerId,
    transports::{OutConnectionConfig, TcpOutConnectionConfig, TransportType},
};
use util::create_basic_handler;

#[derive(Clone)]
struct EmptyHandshake;
impl HandshakeHandler for EmptyHandshake {}

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
        handshake_handler: EmptyHandshake {},
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
        handshake_handler: EmptyHandshake {},
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
