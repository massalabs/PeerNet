mod util;
use std::{thread::sleep, time::Duration};

use massa_signature::KeyPair;
use peernet::{
    config::PeerNetConfiguration,
    network_manager::PeerNetManager,
    peer_id::PeerId,
    transports::{
        OutConnectionConfig, QuicOutConnectionConfig, TcpOutConnectionConfig, TransportType,
    },
};
use util::create_clients;

#[test]
fn simple() {
    let keypair = KeyPair::generate();
    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        self_keypair: keypair.clone(),
        initial_peer_list: Vec::new(),
    };
    let mut manager = PeerNetManager::new(config);
    manager
        .start_listener(TransportType::Tcp, "127.0.0.1:8080".parse().unwrap())
        .unwrap();
    //manager.start_listener(TransportType::Quic, "127.0.0.1:8081".parse().unwrap()).unwrap();
    let clients = create_clients(11);
    sleep(Duration::from_secs(3));
    for client in clients {
        client.join().unwrap();
    }
    manager
        .stop_listener(TransportType::Tcp, "127.0.0.1:8080".parse().unwrap())
        .unwrap();
}

#[test]
fn two_peers_tcp() {
    let keypair1 = KeyPair::generate();
    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        self_keypair: keypair1.clone(),
        initial_peer_list: Vec::new(),
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
        initial_peer_list: Vec::new(),
    };
    let mut manager2 = PeerNetManager::new(config);
    manager2
        .try_connect(
            "127.0.0.1:8081".parse().unwrap(),
            Duration::from_secs(3),
            &mut OutConnectionConfig::Tcp(TcpOutConnectionConfig {
                identity: PeerId::from_public_key(keypair2.get_public_key()),
            }),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));
    manager
        .stop_listener(TransportType::Tcp, "127.0.0.1:8081".parse().unwrap())
        .unwrap();
}

#[test]
fn two_peers_quic() {
    let keypair1 = KeyPair::generate();
    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        self_keypair: keypair1.clone(),
        initial_peer_list: Vec::new(),
    };
    let mut manager = PeerNetManager::new(config);
    manager
        .start_listener(TransportType::Quic, "127.0.0.1:8082".parse().unwrap())
        .unwrap();

    let keypair2 = KeyPair::generate();
    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        self_keypair: keypair2.clone(),
        initial_peer_list: Vec::new(),
    };
    let mut manager2 = PeerNetManager::new(config);
    manager2
        .try_connect(
            "127.0.0.1:8082".parse().unwrap(),
            Duration::from_secs(5),
            //TODO: Use the one in manager instead of asking. Need a wrapper structure ?
            &mut OutConnectionConfig::Quic(QuicOutConnectionConfig {
                identity: PeerId::from_public_key(keypair1.get_public_key()),
                local_addr: "127.0.0.1:8083".parse().unwrap(),
            }),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(5));
    manager
        .stop_listener(TransportType::Quic, "127.0.0.1:8082".parse().unwrap())
        .unwrap();
}
