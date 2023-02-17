use std::{thread::{spawn, JoinHandle}, net::SocketAddr};

use crossbeam::channel::{unbounded, Sender};

use crate::{transports::InternalTransportType, endpoint::Endpoint};

pub struct PeerMetadata {
    // The IP address of the peer
    address: SocketAddr,
    // The public key of the peer
    public_key: String,
    // InternalTransportType type
    transport: InternalTransportType,
}

pub(crate) struct Peer {
    // The socket connected with the peer
    // TODO
    endpoint: Endpoint,
    thread_handler: Option<JoinHandle<()>>,
    thread_sender: Sender<PeerMessage>,
}

enum PeerMessage {
    Stop,
}

impl Peer {
    pub(crate) fn new(endpoint: Endpoint) -> Peer {
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
            endpoint,
            thread_handler: Some(handler),
            thread_sender: tx,
        }
    }
}
