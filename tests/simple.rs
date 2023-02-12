mod util;
use std::{thread::sleep, time::Duration};

use peernet::{config::PeerNetConfiguration, network_manager::PeerNetManager};
use util::create_clients;
#[test]
fn simple() {
    let config = PeerNetConfiguration {
        ip: "127.0.0.1".to_string(),
        port: "8080".to_string(),
        max_peers: 10,
    };
    let manager = PeerNetManager::new(config, Vec::new());
    let clients = create_clients(11);
    sleep(Duration::from_secs(3));
    for client in clients {
        client.join().unwrap();
    }
}
