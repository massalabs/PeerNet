use std::{sync::{Arc, mpsc::channel}, thread::JoinHandle};

use parking_lot::RwLock;

use crate::{peer::{Peer, PeerMetadata}, connection_listener::ConnectionListener, config::PeerNetConfiguration, transport::Transport};

pub(crate) struct PeerDB {
    pub(crate) nb_peers: usize,
}

pub struct PeerNetManager {
    config: PeerNetConfiguration,
    peers_metadata: Vec<PeerMetadata>,
    peer_db: Arc<RwLock<PeerDB>>,
    peers: Vec<Peer>,
    connection_listener: ConnectionListener,
}

pub(crate) enum InternalMessage {
    CreatePeer(Transport)
}

impl PeerNetManager {
    pub fn new(config: PeerNetConfiguration, peers_metadata: Vec<PeerMetadata>) -> PeerNetManager {
        let peer_db = Arc::new(RwLock::new(PeerDB { nb_peers: 0 } ));
        let peers = Vec::new();
        let (peer_creation_sender, peer_creation_receiver) = channel();
        let connection_listener = ConnectionListener::new(&config.ip, &config.port, config.max_peers, peer_db.clone(), peer_creation_sender);
        PeerNetManager {
            config,
            peers_metadata,
            peer_db,
            peers,
            connection_listener,
        }
    }
}

impl Drop for PeerNetManager {
    fn drop(&mut self) {
        //self.manager_thread.take().unwrap().join().unwrap();
    }
}