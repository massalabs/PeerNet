#![allow(dead_code)]
use std::{
    thread::{sleep, JoinHandle},
    time::Duration,
};

use peernet::{error::PeerNetResult, messages::MessagesHandler, peer_id::PeerId};

#[derive(Clone)]
pub struct DefaultMessagesHandler {}

impl MessagesHandler for DefaultMessagesHandler {
    fn deserialize_id<'a>(
        &self,
        data: &'a [u8],
        _peer_id: &PeerId,
    ) -> PeerNetResult<(&'a [u8], u64)> {
        Ok((data, 0))
    }

    fn handle(&self, _id: u64, _data: &[u8], _peer_id: &PeerId) -> PeerNetResult<()> {
        Ok(())
    }
}

pub fn create_clients(nb_clients: usize) -> Vec<JoinHandle<()>> {
    let mut clients = Vec::new();
    for ncli in 0..nb_clients {
        let client = std::thread::Builder::new()
            .name(format!("test_client_{}", ncli))
            .spawn(|| {
            let stream = std::net::TcpStream::connect("127.0.0.1:64850").unwrap();
            sleep(Duration::from_secs(100));
            stream.shutdown(std::net::Shutdown::Both).unwrap();
        }).expect("Failed to spawn thread test_client");
        sleep(Duration::from_millis(100));
        clients.push(client);
    }
    clients
}
