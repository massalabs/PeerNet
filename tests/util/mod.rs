#![allow(dead_code)]
use std::{
    thread::{sleep, JoinHandle},
    time::Duration,
};

use peernet::{
    config::PeerNetCategoryInfo,
    context::Context,
    error::PeerNetResult,
    messages::MessagesHandler,
    peer_id::PeerId,
    transports::{ConnectionConfig, TcpTransportConfig},
};
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

pub fn get_default_tcp_config() -> ConnectionConfig {
    TcpTransportConfig {
        max_in_connections: 10,
        max_message_size_read: 1048576000,
        default_category_info: PeerNetCategoryInfo {
            max_in_connections_pre_handshake: 10,
            max_in_connections_post_handshake: 10,
            max_in_connections_per_ip: 2,
        },
        ..Default::default()
    }
    .into()
}
