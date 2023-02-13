use std::{net::SocketAddr, thread::{JoinHandle, sleep}, time::Duration};

pub fn create_clients(nb_clients: usize) -> Vec<JoinHandle<()>> {
    let mut clients = Vec::new();
    for _ in 0..nb_clients {
        let client = std::thread::spawn(|| {
            let stream = std::net::TcpStream::connect("127.0.0.1:8080").unwrap();
        });
        sleep(Duration::from_millis(10));
        clients.push(client);
    }
    clients
}
