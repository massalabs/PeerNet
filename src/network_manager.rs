use std::{
    sync::{mpsc::channel, Arc},
    thread::JoinHandle,
};

use parking_lot::RwLock;

use crate::{
    config::PeerNetConfiguration,
    connection_listener::ConnectionListener,
    peer::{Peer, PeerMetadata},
};

pub(crate) struct PeerDB {
    pub(crate) peers: Vec<Peer>,
}

pub struct PeerNetManager {
    config: PeerNetConfiguration,
    peers_metadata: Vec<PeerMetadata>,
    peer_db: Arc<RwLock<PeerDB>>,
    peers: Vec<Peer>,
    connection_listener: ConnectionListener,
}

impl PeerNetManager {
    pub fn new(config: PeerNetConfiguration, peers_metadata: Vec<PeerMetadata>) -> PeerNetManager {
        let peer_db = Arc::new(RwLock::new(PeerDB {
            peers: Default::default(),
        }));
        let peers = Vec::new();
        //TODO: Launch multiple thread depending on the number of different transports
        let connection_listener =
            ConnectionListener::new(&config.ip, &config.port, config.max_peers, peer_db.clone());
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
