use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use parking_lot::RwLock;

use crate::{
    config::PeerNetConfiguration,
    error::PeerNetError,
    peer::Peer,
    transports::{InternalTransportType, OutConnectionConfig, Transport, TransportType},
};

pub(crate) struct PeerDB {
    pub(crate) config: PeerNetConfiguration,
    pub(crate) peers: Vec<Peer>,
    pub(crate) nb_in_connections: usize,
    pub(crate) nb_out_connections: usize,
}

pub(crate) type SharedPeerDB = Arc<RwLock<PeerDB>>;

pub struct PeerNetManager {
    peer_db: SharedPeerDB,
    transports: HashMap<TransportType, InternalTransportType>,
}

impl PeerNetManager {
    // TODO: Doc
    // We don't put the transports in the config has they don't implement Clone and so we will have to define
    // an other config structure without the transports for internal usages if we move the transports out of the config.
    pub fn new(config: PeerNetConfiguration) -> PeerNetManager {
        let peer_db = Arc::new(RwLock::new(PeerDB {
            peers: Default::default(),
            nb_in_connections: 0,
            nb_out_connections: 0,
            config: config,
        }));
        PeerNetManager {
            peer_db,
            transports: Default::default(),
        }
    }

    pub fn start_listener(
        &mut self,
        transport_type: TransportType,
        addr: SocketAddr,
    ) -> Result<(), PeerNetError> {
        let transport = self.transports.entry(transport_type).or_insert_with(|| {
            InternalTransportType::from_transport_type(transport_type, self.peer_db.clone())
        });
        transport.start_listener(addr)?;
        Ok(())
    }

    pub fn stop_listener(
        &mut self,
        transport_type: TransportType,
        addr: SocketAddr,
    ) -> Result<(), PeerNetError> {
        let transport = self.transports.entry(transport_type).or_insert_with(|| {
            InternalTransportType::from_transport_type(transport_type, self.peer_db.clone())
        });
        transport.stop_listener(addr)?;
        Ok(())
    }

    pub fn try_connect(
        &mut self,
        addr: SocketAddr,
        timeout: std::time::Duration,
        out_connection_config: &OutConnectionConfig,
    ) -> Result<(), PeerNetError> {
        let transport = self
            .transports
            .entry(TransportType::from_out_connection_config(
                &out_connection_config,
            ))
            .or_insert_with(|| {
                InternalTransportType::from_transport_type(
                    TransportType::from_out_connection_config(&out_connection_config),
                    self.peer_db.clone(),
                )
            });
        transport.try_connect(addr, timeout, out_connection_config.into())?;
        Ok(())
    }
}

impl Drop for PeerNetManager {
    fn drop(&mut self) {
        //self.manager_thread.take().unwrap().join().unwrap();
    }
}
