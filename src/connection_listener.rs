use std::{
    io::{self, Error, Write},
    net::{TcpListener, SocketAddr},
    sync::{
        mpsc::{channel, RecvTimeoutError, Sender},
        Arc,
    },
    thread::{sleep, spawn, JoinHandle},
    time::Duration,
};

use parking_lot::RwLock;

use crate::{network_manager::PeerDB, peer::Peer, transport::{Transport, TransportType}};

/// Public structure in the main thread
pub struct ConnectionListener {
    handler: Option<JoinHandle<()>>,
    thread_sender: Sender<Message>,
}

/// Enum that define the messages that can be sent to the thread
enum Message {
    Stop,
}

impl ConnectionListener {
    pub(crate) fn new(
        addr: SocketAddr,
        transport_type: &TransportType,
        max_peers: usize,
        peers: Arc<RwLock<PeerDB>>,
    ) -> ConnectionListener {
        let (tx, rx) = channel();
        let handler = match transport_type {
            TransportType::Tcp => {
                spawn(move || {
                    //TODO: Maybe optimize with mio.
                    let listener =
                        TcpListener::bind(addr).expect("Cannot bind listener");
                    listener
                        .set_nonblocking(true)
                        .expect("Cannot set non-blocking");
                    loop {
                        for stream in listener.incoming() {
                            match stream {
                                Ok(s) => {
                                    let mut peers_db_write = peers.write();
                                    if peers_db_write.peers.len() < max_peers {
                                        println!("New connection");
                                        peers_db_write.peers.push(Peer::new(Transport::Tcp(s)));
                                    } else {
                                        // TODO: Move Other thread/async tasks
                                        println!("Too many peers");
                                        let mut buffer = [0; 1];
                                        buffer[0] = 0;
                                        let mut stream = s;
                                        stream.write(&buffer).expect("Cannot write to stream");
                                    }
                                }
                                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                    break;
                                }
                                Err(e) => panic!("encountered IO error: {}", e),
                            }
                        }
                        //TODO: Configure timeout
                        match rx.recv_timeout(Duration::from_millis(10)) {
                            Ok(Message::Stop) => {
                                break;
                            }
                            Err(err) => {
                                if err == RecvTimeoutError::Disconnected {
                                    println!("Disconnected");
                                }
                            }
                        }
                    }
                })
            }
            TransportType::Udp => {
                spawn(move || {
                    //TODO: Do we use a range of port or a port that send a new one to the user ?
                })
            }
        };
        ConnectionListener {
            handler: Some(handler),
            thread_sender: tx,
        }
    }
}

impl Drop for ConnectionListener {
    fn drop(&mut self) {
        self.thread_sender.send(Message::Stop).unwrap();
        self.handler.take().unwrap().join().unwrap();
    }
}
