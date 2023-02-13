mod util;
use std::{thread::sleep, time::Duration, collections::HashMap};

use peernet::{config::PeerNetConfiguration, network_manager::PeerNetManager, transport::TransportType};
use util::create_clients;
#[test]
fn simple() {
    let mut transports = HashMap::new();
    transports.insert(TransportType::Tcp, "127.0.0.1:8080".parse().unwrap());
    transports.insert(TransportType::Udp, "127.0.0.1:8081".parse().unwrap());
    let config = PeerNetConfiguration { transports, max_peers: 10 };
    let manager = PeerNetManager::new(config, Vec::new());
    let clients = create_clients(11);
    sleep(Duration::from_secs(3));
    for client in clients {
        client.join().unwrap();
    }
}
