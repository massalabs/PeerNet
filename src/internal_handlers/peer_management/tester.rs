use std::{collections::HashMap, net::SocketAddr, thread::JoinHandle, time::Duration};

use crossbeam::channel::Sender;
use massa_signature::KeyPair;

use crate::{
    config::PeerNetConfiguration,
    error::PeerNetError,
    handlers::{MessageHandler, MessageHandlers},
    internal_handlers::peer_management::{announcement::Announcement, PeerInfo},
    network_manager::PeerNetManager,
    peer::HandshakeHandler,
    peer_id::PeerId,
    transports::{endpoint::Endpoint, OutConnectionConfig, TcpOutConnectionConfig, TransportType},
};

use super::{PeerDB, SharedPeerDB};

#[derive(Clone)]
pub struct EmptyHandshake {
    peer_db: SharedPeerDB,
}

impl HandshakeHandler for EmptyHandshake {
    fn perform_handshake(
        &mut self,
        keypair: &KeyPair,
        endpoint: &mut Endpoint,
        _: &HashMap<SocketAddr, TransportType>,
        _: &MessageHandlers,
    ) -> Result<PeerId, PeerNetError> {
        let id = PeerId::from_public_key(keypair.get_public_key());
        let announcement_data = endpoint.receive()?;
        let announcement = Announcement::from_bytes(&announcement_data[..32], &id)?;
        self.peer_db.write().peers.insert(
            id.clone(),
            PeerInfo {
                last_announce: announcement,
            },
        );
        Ok(id)
    }
}

pub struct Tester {
    handler: Option<JoinHandle<()>>,
}

impl Tester {
    pub fn new(peer_db: SharedPeerDB, listener: (SocketAddr, TransportType)) -> Self {
        let handle = std::thread::spawn(move || {
            let mut message_handlers: MessageHandlers = Default::default();
            let (_announcement_handle, announcement_handler) =
                AnnouncementHandler::new(peer_db.clone());
            message_handlers.add_handler(0, MessageHandler::new(announcement_handler));
            let mut config = PeerNetConfiguration::default(EmptyHandshake { peer_db });
            config.fallback_function = Some(&empty_fallback);
            config.max_out_connections = 1;
            config.message_handlers = message_handlers;
            let mut network_manager = PeerNetManager::new(config);
            network_manager
                .try_connect(
                    listener.0,
                    Duration::from_millis(200),
                    &OutConnectionConfig::Tcp(TcpOutConnectionConfig {}),
                )
                .unwrap();
            std::thread::sleep(Duration::from_millis(10));
        });
        Self {
            handler: Some(handle),
        }
    }
}

struct AnnouncementHandler {
    join_handle: Option<JoinHandle<()>>,
}

impl AnnouncementHandler {
    fn new(peer_db: SharedPeerDB) -> (Self, Sender<(PeerId, Vec<u8>)>) {
        let (sender, receiver) = crossbeam::channel::unbounded::<(PeerId, Vec<u8>)>();
        let handle = std::thread::spawn(move || match receiver.recv() {
            Ok((_, message)) => {
                let peer_id = PeerId::from_bytes(&message[..32].try_into().unwrap()).unwrap();
                let announcement = Announcement::from_bytes(&message[32..], &peer_id).unwrap();
                {
                    let mut peer_db = peer_db.write();
                    println!("Received announcement TESTER: {:?}", announcement);
                    peer_db.peers.insert(
                        peer_id,
                        PeerInfo {
                            last_announce: announcement,
                        },
                    );
                }
            }
            Err(err) => {
                println!("Error while receiving announcement: {}", err);
            }
        });
        (
            Self {
                join_handle: Some(handle),
            },
            sender,
        )
    }
}

pub fn empty_fallback(
    _keypair: &KeyPair,
    _endpoint: &mut Endpoint,
    _listeners: &HashMap<SocketAddr, TransportType>,
    _message_handlers: &MessageHandlers,
) -> Result<(), PeerNetError> {
    println!("Fallback function called");
    std::thread::sleep(Duration::from_millis(10000));
    Ok(())
}
