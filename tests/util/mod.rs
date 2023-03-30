#![allow(dead_code)]
use std::{
    thread::{sleep, JoinHandle},
    time::Duration,
};

use crossbeam::channel::Receiver;
use peernet::{handlers::MessageHandler, peer_id::PeerId};

pub fn create_basic_handler() -> (Receiver<(PeerId, Vec<u8>)>, MessageHandler) {
    let (tx, rx) = crossbeam::channel::unbounded();
    let handler = MessageHandler::new(tx);
    (rx, handler)
}

pub fn create_clients(nb_clients: usize) -> Vec<JoinHandle<()>> {
    let mut clients = Vec::new();
    for _ in 0..nb_clients {
        let client = std::thread::spawn(|| {
            let _ = std::net::TcpStream::connect("127.0.0.1:64850").unwrap();
        });
        sleep(Duration::from_millis(10));
        clients.push(client);
    }
    clients
}
