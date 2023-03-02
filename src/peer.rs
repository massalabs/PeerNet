//! Every information about a peer (not used for now)

use std::{
    net::SocketAddr,
    thread::{spawn, JoinHandle}, sync::Arc,
};

use crossbeam::channel::{unbounded, Sender};
use massa_signature::KeyPair;
use parking_lot::RwLock;

use crate::transports::{Endpoint, InternalTransportType};

pub struct PeerMetadata {
    // The IP address of the peer
    address: SocketAddr,
    // The public key of the peer
    public_key: String,
    // InternalTransportType type
    transport: InternalTransportType,
}

pub(crate) struct Peer {
    // Peer thread handler
    thread_handler: Option<JoinHandle<()>>,
    // if handshake passed then the channel with write thread is created
    write_channel: Arc<RwLock<Option<Sender<Vec<u8>>>>>
}

struct PeerWorker<'a> {
    self_keypair: KeyPair,
    endpoint: &'a mut Endpoint,
    write_thread_handle: Option<JoinHandle<()>>
}


impl Peer {
    pub(crate) fn new(self_keypair: KeyPair, mut endpoint: Endpoint) -> Peer {
        //TODO: Bounded
        let write_channel = Arc::new(RwLock::new(None));
        let write_channel_clone = write_channel.clone();
        let handler = spawn(move || {
            //HANDSHAKE
            endpoint
                .handshake(&self_keypair)
                .unwrap();

            // SPAWN WRITING THREAD
            //TODO: Bound
            let (write_tx, write_rx) = unbounded::<Vec<u8>>();
            let write_thread_handle = std::thread::spawn(move || {
                loop {
                    match write_rx.recv() {
                        Ok(data) => {
                            //TODO: Send. Not trivial because when read s
                            //endpoint.send(&data).unwrap()
                        },
                        Err(err) => {
                            println!("err in writer thread: {}", err);
                            return;
                        }
                    }
                }
            });
            {
                let mut write_write_channel = write_channel_clone.write();
                *write_write_channel = Some(write_tx);
            }
            let peer_worker = PeerWorker {
                endpoint: &mut endpoint,
                self_keypair,
                write_thread_handle: Some(write_thread_handle)
            };
            //MAIN LOOP
            loop {
                match endpoint.receive() {
                    Ok(data) => {
                        println!("Peer: Received {} bytes", data.len());
                    }
                    Err(_) => {
                        println!("Peer stop");
                        return;
                    }
                }
            }
        });
        Peer {
            thread_handler: Some(handler),
            write_channel
        }
    }
}
