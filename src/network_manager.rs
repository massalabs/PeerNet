use std::{
    sync::Arc, collections::HashMap, net::SocketAddr,
};

use parking_lot::RwLock;

use crate::{
    config::PeerNetConfiguration,
    peer::Peer, transports::{InternalTransportType, TransportType, Transport}, error::PeerNetError
};

pub(crate) struct PeerDB {
    pub(crate) peers: Vec<Peer>,
}

pub struct PeerNetManager {
    config: PeerNetConfiguration,
    peer_db: Arc<RwLock<PeerDB>>,
    transports: HashMap<TransportType, InternalTransportType>,
}

impl PeerNetManager {
    // TODO: Doc
    // We don't put the transports in the config has they don't implement Clone and so we will have to define
    // an other config structure without the transports for internal usages if we move the transports out of the config.
    pub fn new(config: PeerNetConfiguration) -> PeerNetManager {
        let peer_db = Arc::new(RwLock::new(PeerDB {
            peers: Default::default(),
        }));
        PeerNetManager {
            config,
            peer_db,
            transports: Default::default()
        }
    }

    pub fn start_listener(&mut self, transport_type: TransportType, addr: SocketAddr) -> Result<(), PeerNetError> {
        let mut transport = InternalTransportType::from(transport_type);
        transport.start_listener(addr)?;
        self.transports.insert(transport_type, transport);
        Ok(())
    }
}

impl Drop for PeerNetManager {
    fn drop(&mut self) {
        //self.manager_thread.take().unwrap().join().unwrap();
    }
}
