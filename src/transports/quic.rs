use std::{
    collections::HashMap,
    net::{SocketAddr, UdpSocket},
    sync::Arc,
    thread::JoinHandle,
    time::Duration,
};

use crossbeam::{channel, sync::WaitGroup};
use mio::{net::UdpSocket as MioUdpSocket, Events, Interest, Poll, Token, Waker};
use parking_lot::RwLock;

use crate::{
    error::PeerNetError, network_manager::SharedPeerDB, peer::Peer, peer_id::PeerId,
    transports::Endpoint,
};

use super::Transport;

const NEW_PACKET_SERVER: Token = Token(0);
const STOP_LISTENER: Token = Token(10);

pub(crate) struct QuicTransport {
    pub peer_db: SharedPeerDB,
    pub out_connection_attempts: WaitGroup,
    pub listeners: HashMap<SocketAddr, (Waker, UdpSocket, JoinHandle<()>)>,
    //(quiche::Connection, data_receiver, data_sender, is_established)
    pub connections: Arc<
        RwLock<
            HashMap<
                SocketAddr,
                (
                    quiche::Connection,
                    channel::Receiver<QuicInternalMessage>,
                    channel::Sender<QuicInternalMessage>,
                    bool,
                ),
            >,
        >,
    >,
}

pub(crate) enum QuicInternalMessage {
    HandshakeFinished,
    Data(Vec<u8>),
}

pub(crate) struct QuicEndpoint {
    pub data_sender: channel::Sender<QuicInternalMessage>,
    pub data_receiver: channel::Receiver<QuicInternalMessage>,
}

#[derive(Clone)]
pub struct QuicOutConnectionConfig {
    // the peer we want to connect to
    pub identity: PeerId,
    pub local_addr: SocketAddr,
}

impl QuicTransport {
    pub fn new(peer_db: SharedPeerDB) -> QuicTransport {
        QuicTransport {
            peer_db,
            out_connection_attempts: WaitGroup::new(),
            listeners: Default::default(),
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Transport for QuicTransport {
    type OutConnectionConfig = QuicOutConnectionConfig;

    type Endpoint = QuicEndpoint;

    fn start_listener(&mut self, address: SocketAddr) -> Result<(), PeerNetError> {
        let mut poll = Poll::new().map_err(|err| PeerNetError::ListenerError(err.to_string()))?;
        //TODO: Configurable capacity
        let mut events = Events::with_capacity(128);
        let waker = Waker::new(poll.registry(), STOP_LISTENER)
            .map_err(|err| PeerNetError::ListenerError(err.to_string()))?;
        let connections = self.connections.clone();
        let server = UdpSocket::bind(address)
            .expect(&format!("Can't bind QUIC transport to address {}", address));
        server.set_nonblocking(true).unwrap();

        let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION).unwrap();
        config.set_max_recv_udp_payload_size(1200);
        // Create certificate from ed25519 as made in libp2p tls
        config
            .load_cert_chain_from_pem_file("./src/cert.crt")
            .unwrap();
        config
            .load_priv_key_from_pem_file("./src/cert.key")
            .unwrap();
        config.set_application_protos(&[b"massa/1.0"]).unwrap();

        let listener_handle = std::thread::spawn({
            let peer_db = self.peer_db.clone();
            let server = server.try_clone().unwrap();
            move || {
                let mut socket = MioUdpSocket::from_std(server);
                // Start listening for incoming connections.
                poll.registry()
                    .register(&mut socket, NEW_PACKET_SERVER, Interest::READABLE)
                    .expect(&format!(
                        "Can't register polling on QUIC transport of address {}",
                        address
                    ));
                let mut buf = [0; 65507];
                loop {
                    // Poll Mio for events, blocking until we get an event.
                    //TODO: Configurable timeout (cf. https://github.com/cloudflare/quiche/blob/master/apps/src/bin/quiche-server.rs#L177)
                    poll.poll(&mut events, Some(Duration::from_millis(100)))
                        .expect(&format!("Can't poll QUIC transport of address {}", address));

                    // Process each event.
                    for event in events.iter() {
                        match event.token() {
                            NEW_PACKET_SERVER => {
                                'read: loop {
                                    //TODO: Error handling
                                    let (num_recv, from_addr) = match socket.recv_from(&mut buf) {
                                        Ok(v) => v,
                                        Err(e) => {
                                            // There are no more UDP packets to read, so end the read
                                            // loop.
                                            if e.kind() == std::io::ErrorKind::WouldBlock {
                                                break 'read;
                                            }
                                            panic!("recv() failed: {:?}", e);
                                        }
                                    };
                                    println!(
                                        "server {}: Received {} bytes from {} ",
                                        address, num_recv, from_addr
                                    );
                                    // Parse the QUIC packet's header.
                                    let hdr = match quiche::Header::from_slice(
                                        &mut buf,
                                        quiche::MAX_CONN_ID_LEN,
                                    ) {
                                        Ok(v) => v,

                                        Err(e) => {
                                            println!("Parsing packet header failed: {:?}", e);
                                            panic!("Parsing packet header failed: {:?}", e)
                                        }
                                    };
                                    let new_connection = {
                                        let connections = connections.read();
                                        !connections.contains_key(&from_addr)
                                    };
                                    if new_connection {
                                        println!(
                                            "server {}: New connection {}",
                                            address, from_addr
                                        );
                                        if hdr.ty != quiche::Type::Initial {
                                            println!("Packet is not Initial");
                                            continue;
                                        }

                                        let connection = quiche::accept(
                                            &hdr.scid,
                                            None,
                                            address,
                                            from_addr,
                                            &mut config,
                                        )
                                        .unwrap();

                                        let mut peer_db = peer_db.write();
                                        if peer_db.nb_in_connections
                                            < peer_db.config.max_in_connections
                                        {
                                            peer_db.nb_in_connections += 1;
                                            let (send_tx, send_rx) = channel::bounded(10000);
                                            let (recv_tx, recv_rx) = channel::bounded(10000);
                                            {
                                                let mut connections = connections.write();
                                                connections.insert(
                                                    from_addr,
                                                    (connection, send_rx, recv_tx, false),
                                                );
                                            }
                                            let peer = Peer::new(Endpoint::Quic(QuicEndpoint {
                                                data_receiver: recv_rx,
                                                data_sender: send_tx,
                                            }));
                                            peer_db.peers.push(peer);
                                        }
                                    }
                                    {
                                        let mut connections = connections.write();
                                        //TODO: Handle if the peer wasn't created because no place it will fail
                                        let (connection, _, _, _) =
                                            connections.get_mut(&from_addr).unwrap();
                                        let recv_info = quiche::RecvInfo {
                                            from: from_addr,
                                            to: address,
                                        };
                                        connection.recv(&mut buf[..num_recv], recv_info).unwrap();
                                    }
                                }
                            }
                            STOP_LISTENER => {
                                return;
                            }
                            // We don't expect any events with tokens other than those we provided. (from mio doc)
                            _ => unreachable!(),
                        }
                    }

                    //Try fetching packet from
                    //Write packets to quic if needed
                    {
                        let mut connections = connections.write();
                        let mut buf = [0; 65507];
                        for (address, (connection, send_rx, recv_tx, is_established)) in
                            connections.iter_mut()
                        {
                            if !*is_established {
                                if connection.is_established() {
                                    println!("server {}: Connection established", address);
                                    *is_established = true;
                                    recv_tx
                                        .send(QuicInternalMessage::HandshakeFinished)
                                        .unwrap();
                                }
                            }
                            while let Ok(data) = send_rx.try_recv() {
                                match data {
                                    QuicInternalMessage::Data(data) => {
                                        if *is_established {
                                            let sent =
                                                connection.stream_send(1, &data, true).unwrap();
                                            println!(
                                                "server {}: Sent {} bytes on stream 1",
                                                address, sent
                                            );
                                        } else {
                                            println!(
                                                "server {}: Connection not established",
                                                address
                                            );
                                        }
                                    }
                                    QuicInternalMessage::HandshakeFinished => {}
                                }
                            }
                            loop {
                                let (write, send_info) = match connection.send(&mut buf) {
                                    Ok(v) => v,

                                    Err(quiche::Error::Done) => {
                                        // Done writing.
                                        break;
                                    }

                                    Err(e) => {
                                        println!("server {}: send failed: {:?}", address, e);
                                        // An error occurred, handle it.
                                        break;
                                    }
                                };
                                println!(
                                    "server {}: Sending {} bytes to {} ",
                                    address, write, send_info.to
                                );
                                socket.send_to(&buf[..write], send_info.to).unwrap();
                            }
                        }
                    }
                }
            }
        });
        self.listeners.insert(
            address,
            (waker, server.try_clone().unwrap(), listener_handle),
        );
        Ok(())
    }

    fn try_connect(
        &mut self,
        address: SocketAddr,
        _timeout: Duration,
        config: &Self::OutConnectionConfig,
    ) -> Result<(), PeerNetError> {
        //TODO: Use timeout
        let config = config.clone();
        let connections = self.connections.clone();
        let (_, socket, _) = if self.listeners.contains_key(&config.local_addr) {
            self.listeners
                .get(&config.local_addr)
                .expect("Listener not found")
        } else {
            self.start_listener(config.local_addr)?;
            //TODO: Make things more elegant with waker etc
            std::thread::sleep(Duration::from_millis(100));
            self.listeners
                .get(&config.local_addr)
                .expect("Listener not found")
        };
        let socket = socket.try_clone().unwrap();
        std::thread::spawn({
            let peer_db = self.peer_db.clone();
            let wg = self.out_connection_attempts.clone();
            move || {
                let mut out = [0; 65507];
                println!("Connecting to {}", address);
                //TODO: Use configs for quiche passed from config object.
                //and error handling
                let mut quiche_config =
                    quiche::Config::new(quiche::PROTOCOL_VERSION).expect("Default config failed");
                quiche_config.verify_peer(false);
                //TODO: Config
                quiche_config
                    .set_application_protos(&[b"massa/1.0"])
                    .unwrap();
                //TODO: random bytes
                let scid = [0; quiche::MAX_CONN_ID_LEN];
                let scid = quiche::ConnectionId::from_ref(&scid);
                let mut conn =
                    quiche::connect(None, &scid, config.local_addr, address, &mut quiche_config)
                        .unwrap();
                loop {
                    let (write, send_info) = match conn.send(&mut out) {
                        Ok(v) => v,
                        Err(quiche::Error::Done) => {
                            break;
                        }
                        Err(e) => {
                            println!("send failed: {:?}", e);
                            return;
                        }
                    };

                    println!(
                        "client: init: send_info: {:?} sent {} bytes",
                        send_info, write
                    );
                    while let Err(e) = socket.send_to(&out[..write], send_info.to) {
                        if e.kind() == std::io::ErrorKind::WouldBlock {
                            continue;
                        }

                        println!("send() failed: {:?}", e);
                        return;
                    }
                }

                let mut peer_db = peer_db.write();
                if peer_db.nb_out_connections < peer_db.config.max_out_connections {
                    peer_db.nb_out_connections += 1;
                    //TODO: Config
                    let (send_tx, send_rx) = channel::bounded(10000);
                    let (recv_tx, recv_rx) = channel::bounded(10000);
                    {
                        let mut connections = connections.write();
                        connections.insert(address, (conn, send_rx, recv_tx, false));
                    }
                    let peer = Peer::new(Endpoint::Quic(QuicEndpoint {
                        data_sender: send_tx,
                        data_receiver: recv_rx,
                    }));
                    peer_db.peers.push(peer);
                }
                drop(wg);
            }
        });
        Ok(())
    }

    fn stop_listener(&mut self, address: SocketAddr) -> Result<(), PeerNetError> {
        let (waker, _, handle) =
            self.listeners
                .remove(&address)
                .ok_or(PeerNetError::ListenerError(format!(
                    "Can't find listener for address {}",
                    address
                )))?;
        waker
            .wake()
            .map_err(|e| PeerNetError::ListenerError(e.to_string()))?;
        handle
            .join()
            .expect(&format!("Couldn't join listener for address {}", address));
        Ok(())
    }

    fn send(endpoint: &mut Self::Endpoint, data: &[u8]) -> Result<(), PeerNetError> {
        endpoint
            .data_sender
            .send(QuicInternalMessage::Data(data.to_vec()))
            .map_err(|err| PeerNetError::ReceiveError(err.to_string()))?;
        Ok(())
    }

    fn receive(endpoint: &mut Self::Endpoint) -> Result<Vec<u8>, PeerNetError> {
        endpoint
            .data_receiver
            .recv()
            .map_err(|err| PeerNetError::ReceiveError(err.to_string()))?;
        Ok(Vec::new())
    }

    fn handshake(endpoint: &mut Self::Endpoint) -> Result<(), PeerNetError> {
        // In quic handshake is done in the background, so we just wait for the handshake to be finished
        match endpoint
            .data_receiver
            .recv()
            .map_err(|err| PeerNetError::ReceiveError(err.to_string()))?
        {
            QuicInternalMessage::HandshakeFinished => {
                println!("Handshake finished");
                Ok(())
            }
            _ => Err(PeerNetError::ReceiveError("Unexpected message".to_string())),
        }
    }
}
