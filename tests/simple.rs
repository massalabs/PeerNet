mod util;
use std::{thread::sleep, time::Duration};

use peernet::{
    config::PeerNetConfiguration, network_manager::PeerNetManager, transports::TransportType,
};
use util::create_clients;

#[test]
fn simple() {
    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
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
fn two_peers() {
    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        initial_peer_list: Vec::new(),
    };
    let mut manager = PeerNetManager::new(config);
    manager
        .start_listener(TransportType::Tcp, "127.0.0.1:8080".parse().unwrap())
        .unwrap();

    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        initial_peer_list: Vec::new(),
    };
    let mut manager2 = PeerNetManager::new(config);
    manager2.try_connect(TransportType::Tcp, "127.0.0.1:8080".parse().unwrap(), Duration::from_secs(3)).unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));
    manager
        .stop_listener(TransportType::Tcp, "127.0.0.1:8080".parse().unwrap())
        .unwrap();
}
