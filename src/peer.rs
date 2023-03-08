//! Every information about a peer (not used for now)

use std::{
    net::SocketAddr,
    thread::{spawn, JoinHandle},
};

use crossbeam::channel::{unbounded, Sender};

use crate::{endpoint::Endpoint, transports::InternalTransportType};

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
    //TODO: Don't need this the crossbeam channel already disconnect himself
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
                Err(_) => {
                    return;
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
