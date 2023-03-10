use std::{collections::HashMap, net::SocketAddr, thread::JoinHandle};

use crossbeam::channel::Receiver;
use massa_hash::Hash;
use massa_signature::{KeyPair, PublicKey, Signature};
use rand::{rngs::StdRng, RngCore, SeedableRng};

use crate::{
    error::PeerNetError,
    handlers::{MessageHandler, MessageHandlers},
    network_manager::{ActiveConnections, PeerNetManager},
    peer_id::PeerId,
    transports::{endpoint::Endpoint, TransportType},
};

use self::announcement::Announcement;

/// This file contains the definition of the peer management handler
/// This handler is here to check that announcements we receive are valid and
/// that all the endpoints we received are active.
mod announcement;

pub type InitialPeers = HashMap<PeerId, HashMap<SocketAddr, TransportType>>;

#[derive(Default)]
pub struct PeerDB {
    //TODO: Add state of the peer (banned, trusted, ...)
    pub peers: HashMap<PeerId, PeerInfo>,
}

pub struct PeerManagementHandler {
    thread_join: Option<JoinHandle<()>>,
}

pub enum PeerManagementMessage {
    // Receive the announcement sent by a peer when connecting.
    // It has already been verified that the signature is valid.
    NEW_PEER_CONNECTED((PeerId, Announcement)),
    // Receive the announcements sent by a peer that is already connected.
    LIST_PEERS(Vec<(PeerId, Announcement)>),
}

//TODO: Use a proper serialization system like we have in massa.
impl PeerManagementMessage {
    fn from_bytes(bytes: &[u8]) -> Result<Self, PeerNetError> {
        match bytes[0] {
            0 => {
                let peer_id = PeerId::from_bytes(&bytes[1..33].try_into().unwrap())?;
                let announcement = Announcement::from_bytes(&bytes[33..], &peer_id)?;
                Ok(PeerManagementMessage::NEW_PEER_CONNECTED((
                    peer_id,
                    announcement,
                )))
            }
            1 => {
                let nb_peers = u64::from_le_bytes(bytes[1..9].try_into().unwrap());
                let mut peers = Vec::with_capacity(nb_peers as usize);
                let mut offset = 9;
                for _ in 0..nb_peers {
                    let peer_id =
                        PeerId::from_bytes(&bytes[offset..offset + 32].try_into().unwrap())?;
                    offset += 32;
                    let announcement = Announcement::from_bytes(&bytes[offset..], &peer_id)?;
                    offset += announcement.to_bytes().len();
                    peers.push((peer_id, announcement));
                }
                Ok(PeerManagementMessage::LIST_PEERS(peers))
            }
            _ => Err(PeerNetError::InvalidMessage),
        }
    }

    fn to_bytes(&self) -> Vec<u8> {
        match self {
            PeerManagementMessage::NEW_PEER_CONNECTED((peer_id, announcement)) => {
                let mut bytes = vec![0];
                bytes.extend_from_slice(&peer_id.to_bytes());
                bytes.extend_from_slice(&announcement.to_bytes());
                bytes
            }
            PeerManagementMessage::LIST_PEERS(peers) => {
                let mut bytes = vec![1];
                let nb_peers = peers.len() as u64;
                bytes.extend_from_slice(&nb_peers.to_le_bytes());
                for (peer_id, announcement) in peers {
                    bytes.extend_from_slice(&peer_id.to_bytes());
                    bytes.extend_from_slice(&announcement.to_bytes());
                }
                bytes
            }
        }
    }
}

pub struct PeerInfo {
    pub last_announce: Announcement,
}

impl PeerManagementHandler {
    pub fn new(initial_peers: InitialPeers) -> (Self, MessageHandler) {
        let (sender, receiver) = crossbeam::channel::unbounded();
        let thread_join = std::thread::spawn(move || {
            let mut peer_db: PeerDB = Default::default();
            //TODO: Manage initials peers
            for (peer_id, listeners) in initial_peers {
                //TODO: How to sign? Timestamp?
                //peer_db.peers.insert(peer_id, PeerInfo { last_announce: Announcement::new(listeners, &KeyPair::new()).unwrap() });
            }
            loop {
                let (peer_id, message) = receiver.recv().unwrap();

                println!("Received message from peer: {:?}", peer_id);
                println!("Message: {:?}", message);
            }
        });
        (
            Self {
                thread_join: Some(thread_join),
            },
            MessageHandler::new(sender),
        )
    }
}

pub fn handshake(
    keypair: &KeyPair,
    endpoint: &mut Endpoint,
    listeners: &HashMap<SocketAddr, TransportType>,
    message_handlers: &MessageHandlers,
) -> Result<PeerId, PeerNetError> {
    let mut buf = PeerId::from_public_key(keypair.get_public_key()).to_bytes();
    //TODO: Add version in handshake
    let listeners_announcement = Announcement::new(listeners.clone(), keypair).unwrap();
    buf.extend_from_slice(&listeners_announcement.to_bytes());
    endpoint.send(&buf)?;

    let received = endpoint.receive()?;
    //TODO: We use this to verify the signature before sending it to the handler.
    //This will be done also in the handler but as we are in the handshake we want to do it to invalid the handshake in case it fails.
    let peer_id = PeerId::from_bytes(&received[..32].try_into().unwrap())?;
    let announcement = Announcement::from_bytes(&received[32..], &peer_id)?;
    //TODO: Verify that the handler is defined
    message_handlers
        .get_handler(0)
        .unwrap()
        .send_message((peer_id.clone(), received))
        .unwrap();
    println!("Received announcement: {:?}", announcement);

    let mut self_random_bytes = [0u8; 32];
    StdRng::from_entropy().fill_bytes(&mut self_random_bytes);
    let self_random_hash = Hash::compute_from(&self_random_bytes);
    let mut buf = [0u8; 32];
    buf[..32].copy_from_slice(&self_random_bytes);

    endpoint.send(&buf)?;
    let received = endpoint.receive()?;
    let other_random_bytes: &[u8; 32] = received.as_slice()[..32].try_into().unwrap();

    // sign their random bytes
    let other_random_hash = Hash::compute_from(other_random_bytes);
    let self_signature = keypair.sign(&other_random_hash).unwrap();

    let mut buf = [0u8; 64];
    buf.copy_from_slice(&self_signature.to_bytes());

    endpoint.send(&buf)?;
    let received = endpoint.receive()?;

    let other_signature = Signature::from_bytes(received.as_slice().try_into().unwrap()).unwrap();

    // check their signature
    peer_id.verify_signature(&self_random_hash, &other_signature)?;

    println!("Handshake finished");
    Ok(peer_id)
}
