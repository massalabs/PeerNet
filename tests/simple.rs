mod util;
use std::{thread::sleep, time::Duration};

use peernet::types::KeyPair;
use peernet::{
    config::{PeerNetConfiguration, PeerNetFeatures},
    network_manager::PeerNetManager,
    peer::HandshakeHandler,
    peer_id::PeerId,
    transports::{
        OutConnectionConfig, QuicOutConnectionConfig, TcpOutConnectionConfig, TransportType,
    },
};
use util::{create_clients, DefaultMessagesHandler};

#[derive(Clone)]
pub struct DefaultHandshake;
impl HandshakeHandler for DefaultHandshake {}

#[test]
fn simple() {
    let keypair = KeyPair::generate();
    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        self_keypair: keypair,
        fallback_function: None,
        handshake_handler: DefaultHandshake,
        optional_features: PeerNetFeatures::default().set_reject_same_ip_addr(false),
        message_handler: DefaultMessagesHandler {},
    };
    let mut manager = PeerNetManager::new(config);
    manager
        .start_listener(TransportType::Tcp, "127.0.0.1:64850".parse().unwrap())
        .unwrap();
    //manager.start_listener(TransportType::Quic, "127.0.0.1:64850".parse().unwrap()).unwrap();
    sleep(Duration::from_secs(3));
    let clients = create_clients(11);
    sleep(Duration::from_secs(6));
    for client in clients {
        client.join().unwrap();
    }

    // we have max_in_connections = 10
    assert!(manager.nb_in_connections().eq(&10));

    manager
        .stop_listener(TransportType::Tcp, "127.0.0.1:64850".parse().unwrap())
        .unwrap();
}

#[test]
fn two_peers_tcp() {
    let keypair1 = KeyPair::generate();
    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        self_keypair: keypair1,
        handshake_handler: DefaultHandshake {},
        fallback_function: None,
        optional_features: PeerNetFeatures::default().set_reject_same_ip_addr(false),
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
        self_keypair: keypair2,
        fallback_function: None,
        handshake_handler: DefaultHandshake {},
        optional_features: PeerNetFeatures::default().set_reject_same_ip_addr(false),
        message_handler: DefaultMessagesHandler {},
    };
    let mut manager2 = PeerNetManager::new(config);
    sleep(Duration::from_secs(3));
    manager2
        .try_connect(
            "127.0.0.1:8081".parse().unwrap(),
            Duration::from_secs(3),
            &OutConnectionConfig::Tcp(Box::new(TcpOutConnectionConfig {})),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));
    manager
        .stop_listener(TransportType::Tcp, "127.0.0.1:8081".parse().unwrap())
        .unwrap();
    assert!(manager.nb_in_connections().eq(&1));
}

#[test]
fn two_peers_quic() {
    let keypair1 = KeyPair::generate();
    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        self_keypair: keypair1.clone(),
        fallback_function: None,
        handshake_handler: DefaultHandshake {},
        message_handler: DefaultMessagesHandler {},
        optional_features: PeerNetFeatures::default().set_reject_same_ip_addr(false),
    };
    let mut manager = PeerNetManager::new(config);
    manager
        .start_listener(TransportType::Quic, "127.0.0.1:8082".parse().unwrap())
        .unwrap();

    let keypair2 = KeyPair::generate();
    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        self_keypair: keypair2,
        fallback_function: None,
        handshake_handler: DefaultHandshake {},
        optional_features: PeerNetFeatures::default().set_reject_same_ip_addr(false),
        message_handler: DefaultMessagesHandler {},
    };
    let mut manager2 = PeerNetManager::new(config);
    sleep(Duration::from_secs(3));
    manager2
        .try_connect(
            "127.0.0.1:8082".parse().unwrap(),
            Duration::from_secs(5),
            //TODO: Use the one in manager instead of asking. Need a wrapper structure ?
            &mut OutConnectionConfig::Quic(Box::new(QuicOutConnectionConfig {
                identity: PeerId::from_public_key(keypair1.get_public_key()),
                local_addr: "127.0.0.1:8083".parse().unwrap(),
            })),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(5));
    manager
        .stop_listener(TransportType::Quic, "127.0.0.1:8082".parse().unwrap())
        .unwrap();
}
