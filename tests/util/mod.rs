#![allow(dead_code)]
use std::{
    thread::{sleep, JoinHandle},
    time::Duration,
};

use peernet::{context::Context, error::PeerNetResult, messages::MessagesHandler, peer_id::PeerId};
use rand::Rng;

#[derive(Clone)]
pub struct DefaultContext {
    pub our_id: DefaultPeerId,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DefaultPeerId {
    pub id: u64,
}

impl PeerId for DefaultPeerId {
    fn generate() -> Self {
        //TODO: Add rand
        let mut rng = rand::thread_rng();
        let random_number: u64 = rng.gen();
        DefaultPeerId { id: random_number }
    }
}

impl Context<DefaultPeerId> for DefaultContext {
    fn get_peer_id(&self) -> DefaultPeerId {
        self.our_id.clone()
    }
}

#[derive(Clone)]
pub struct DefaultMessagesHandler {}

impl MessagesHandler<DefaultPeerId> for DefaultMessagesHandler {
    fn deserialize_id<'a>(
        &self,
        data: &'a [u8],
        _peer_id: &DefaultPeerId,
    ) -> PeerNetResult<(&'a [u8], u64)> {
        Ok((data, 0))
    }

    fn handle(&self, _id: u64, _data: &[u8], _peer_id: &DefaultPeerId) -> PeerNetResult<()> {
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
