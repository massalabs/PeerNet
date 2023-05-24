#![allow(dead_code)]
use std::{
    thread::{sleep, JoinHandle},
    time::Duration,
};

use peernet::{error::PeerNetResult, messages::MessagesHandler, peer_id::PeerId};

#[derive(Clone)]
pub struct DefaultMessagesHandler {}

impl MessagesHandler for DefaultMessagesHandler {
    fn handle(&self, _data: &[u8], _peer_id: &PeerId) -> PeerNetResult<()> {
        Ok(())
    }
}

pub fn create_clients(nb_clients: usize, to_ip: &str) -> Vec<JoinHandle<()>> {
    let mut clients = Vec::new();
    for ncli in 0..nb_clients {
        let to_ip = to_ip.to_string();
        let client = std::thread::Builder::new()
            .name(format!("test_client_{}", ncli))
            .spawn(|| {
                let stream = std::net::TcpStream::connect(to_ip).unwrap();
                sleep(Duration::from_secs(100));
                stream.shutdown(std::net::Shutdown::Both).unwrap();
            })
            .expect("Failed to spawn thread test_client");
        sleep(Duration::from_millis(100));
        clients.push(client);
    }
    clients
}
