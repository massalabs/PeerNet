#[cfg(test)]
mod tests {
    use std::{thread::sleep, time::Duration};

    use peernet::{network_manager::PeerNetManager, config::PeerNetConfiguration};

    #[test]
    fn simple() {
        let config = PeerNetConfiguration {
            ip: "127.0.0.1".to_string(),
            port: "8080".to_string(),
            max_peers: 10,
        };
        let manager = PeerNetManager::new(config, Vec::new());
        sleep(Duration::from_secs(10));
    }
}