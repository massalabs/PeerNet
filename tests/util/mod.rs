use std::{net::SocketAddr, thread::JoinHandle};

pub fn create_clients(nb_clients: usize) -> Vec<JoinHandle<()>> {
    let mut clients = Vec::new();
    for _ in 0..nb_clients {
        let client = std::thread::spawn(|| {
            let mut stream = std::net::TcpStream::connect("127.0.0.1:8080").unwrap();
        });
        clients.push(client);
    }
    clients
}
