use std::thread::{spawn, JoinHandle};

use crossbeam::channel::{unbounded, Sender};

use crate::transport::{Transport, TransportType};

pub struct PeerMetadata {
    // The IP address of the peer
    ip: String,
    // The port of the peer
    port: u16,
    // The public key of the peer
    public_key: String,
    // Transport type
    transport: TransportType,
}

pub(crate) struct Peer {
    // The socket connected with the peer
    stream: Transport,
    thread_handler: Option<JoinHandle<()>>,
    thread_sender: Sender<PeerMessage>,
}

enum PeerMessage {
    Stop,
}

impl Peer {
    pub(crate) fn new(stream: Transport) -> Peer {
        //TODO: Bounded
        let (tx, rx) = unbounded();
        let handler = spawn(move || loop {
            match rx.recv() {
                Ok(PeerMessage::Stop) => {
                    break;
                }
                Err(err) => {
                    println!("Error: {}", err);
                }
            }
        });
        Peer {
            stream,
            thread_handler: Some(handler),
            thread_sender: tx,
        }
    }
}
