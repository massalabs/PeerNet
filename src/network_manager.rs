use std::{
    sync::{mpsc::channel, Arc},
    thread::JoinHandle, collections::HashMap,
};

use parking_lot::RwLock;

use crate::{
    config::PeerNetConfiguration,
    connection_listener::ConnectionListener,
    peer::{Peer, PeerMetadata}, transport::TransportType,
};

pub(crate) struct PeerDB {
    pub(crate) peers: Vec<Peer>,
}

pub struct PeerNetManager {
    config: PeerNetConfiguration,
    peer_list: Vec<PeerMetadata>,
    peer_db: Arc<RwLock<PeerDB>>,
    connection_listeners: HashMap<TransportType, ConnectionListener>,
}

impl PeerNetManager {
    pub fn new(config: PeerNetConfiguration, peer_list: Vec<PeerMetadata>) -> PeerNetManager {
        let peer_db = Arc::new(RwLock::new(PeerDB {
            peers: Default::default(),
        }));
        let mut connection_listeners = HashMap::new();
        for (transport_type, transport_address) in config.transports.iter() {
            connection_listeners.insert(*transport_type, ConnectionListener::new(
                transport_address.clone(),
                transport_type,
                config.max_peers,
                peer_db.clone(),
            ));
        }
        PeerNetManager {
            config,
            peer_list,
            peer_db,
            connection_listeners,
        }
    }
}

impl Drop for PeerNetManager {
    fn drop(&mut self) {
        //self.manager_thread.take().unwrap().join().unwrap();
    }
}
