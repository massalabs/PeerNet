#![allow(dead_code)]
use std::{
    thread::{sleep, JoinHandle},
    time::Duration,
};

use peernet::{error::PeerNetResult, messages::MessagesHandler, peer_id::PeerId};

#[derive(Clone)]
pub struct DefaultMessagesHandler {}

impl MessagesHandler for DefaultMessagesHandler {
    fn deserialize_and_handle(&self, _data: &[u8], _peer_id: &PeerId) -> PeerNetResult<()> {
        Ok(())
    }
}

pub fn create_clients(nb_clients: usize) -> Vec<JoinHandle<()>> {
    let mut clients = Vec::new();
    for _ in 0..nb_clients {
        let client = std::thread::spawn(|| {
            let _ = std::net::TcpStream::connect("127.0.0.1:64850").unwrap();
        });
        sleep(Duration::from_millis(100));
        clients.push(client);
    }
    clients
}
