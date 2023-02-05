use std::{io::{self, Error}, thread::{spawn, JoinHandle, sleep}, time::Duration, sync::{mpsc::{Sender, channel, RecvTimeoutError}, Arc}, net::TcpListener};

use parking_lot::RwLock;

use crate::{peer::Peer, transport::Transport, network_manager::{InternalMessage, PeerDB}};

/// Public structure in the main thread
pub struct ConnectionListener {
    handler: Option<JoinHandle<()>>,
    thread_sender: Sender<Message> 
}

/// Enum that define the messages that can be sent to the thread
enum Message {
    Stop
}

impl ConnectionListener {
    pub(crate) fn new(ip: &String, port: &String, max_peers: usize, peers: Arc<RwLock<PeerDB>>, peer_creation_sender: Sender<InternalMessage>) -> ConnectionListener {

        let (tx, rx) = channel();

        //TODO: Config to be a read/write lock
        let ip = ip.clone();
        let port = port.clone();

        let handler = spawn(move || {
            
            let listener = TcpListener::bind(format!("{}:{}", ip, port)).expect("Cannot bind listener");
            listener.set_nonblocking(true).expect("Cannot set non-blocking");
            loop {
                for stream in listener.incoming() {
                    match stream {
                        Ok(s) => {
                            let mut peers_db_write = peers.write();
                            if peers_db_write.nb_peers < max_peers {
                                println!("New connection");
                                // Can't initialize peers directly here because otherwise they can't have a sender (that will be used to ask them information)
                                // for each accessible by an other thread and we don't want to have all messages passing by this listener 
                                // thread to be able to be sent to the peers.
                                // Possible solution: Having a list of sender and receiver already initialized in the main thread and passing the receiver to the peers
                                // here but it changes the way peers are managed as they can exists but not connected to anyone
                                peer_creation_sender.send(InternalMessage::CreatePeer(Transport::Tcp(s))).unwrap();
                            }
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            break;
                        }
                        Err(e) => panic!("encountered IO error: {}", e),
                    }
                }
                //TODO: Configure timeout
                match rx.recv_timeout(Duration::from_secs(1)) {
                    Ok(Message::Stop) => {
                        break;
                    },
                    Err(err) => {
                        if err == RecvTimeoutError::Disconnected {
                            println!("Disconnected");
                        }
                    }
                }
            }
        });

        ConnectionListener {
            handler: Some(handler),
            thread_sender: tx
        }
    }
}

impl Drop for ConnectionListener {
    fn drop(&mut self) {
        self.thread_sender.send(Message::Stop).unwrap();
        self.handler.take().unwrap().join().unwrap();
    }
}