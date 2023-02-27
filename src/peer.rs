//! Every information about a peer (not used for now)

use std::{
    net::SocketAddr,
    thread::{spawn, JoinHandle},
};

use crossbeam::channel::{unbounded, Sender};

use crate::{
    peer_id::PeerId,
    transports::{Endpoint, InternalTransportType, TransportType},
};

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
    thread_handler: Option<JoinHandle<()>>,
    thread_sender: Sender<PeerMessage>,
}

struct PeerWorker {
    endpoint: Endpoint,
}

enum PeerMessage {
    //TODO: Don't need this the crossbeam channel already disconnect himself
    Stop,
}

impl Peer {
    pub(crate) fn new(endpoint: Endpoint) -> Peer {
        //TODO: Bounded
        let (tx, rx) = unbounded();
        let handler = spawn(move || {
            let peer_worker = PeerWorker { endpoint };
            //HANDSHAKE
            //TODO: Take this function as parameter

            //MAIN LOOP
            loop {
                match rx.recv() {
                    Ok(PeerMessage::Stop) => {
                        break;
                    }
                    Err(_) => {
                        return;
                    }
                }
            }
        });
        Peer {
            thread_handler: Some(handler),
            thread_sender: tx,
        }
    }
}
