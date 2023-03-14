use std::{
    collections::HashMap,
    net::{SocketAddr, UdpSocket},
    sync::Arc,
    thread::JoinHandle,
    time::Duration,
};

use crate::types::KeyPair;
use crossbeam::{channel, sync::WaitGroup};
use mio::{net::UdpSocket as MioUdpSocket, Events, Interest, Poll, Token, Waker};
use parking_lot::RwLock;

use crate::{
    error::PeerNetError,
    handlers::MessageHandlers,
    network_manager::{FallbackFunction, HandshakeFunction, SharedActiveConnections},
    peer::{new_peer, HandshakeHandler},
    peer_id::PeerId,
    transports::Endpoint,
};

use super::Transport;

const NEW_PACKET_SERVER: Token = Token(0);
const STOP_LISTENER: Token = Token(10);

pub(crate) struct QuicTransport {
    pub active_connections: SharedActiveConnections,
    pub fallback_function: Option<&'static FallbackFunction>,
    pub message_handlers: MessageHandlers,
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
    Data(Vec<u8>),
    Shutdown,
}

#[derive(Clone)]
pub struct QuicEndpoint {
    pub(crate) data_sender: channel::Sender<QuicInternalMessage>,
    pub(crate) data_receiver: channel::Receiver<QuicInternalMessage>,
}

impl QuicEndpoint {
    pub fn shutdown(&mut self) {
        self.data_sender
            .send(QuicInternalMessage::Shutdown)
            .unwrap();
    }
}

#[derive(Clone)]
pub struct QuicOutConnectionConfig {
    // the peer we want to connect to
    pub identity: PeerId,
    pub local_addr: SocketAddr,
}

impl QuicTransport {
    pub fn new(
        active_connections: SharedActiveConnections,
        fallback_function: Option<&'static FallbackFunction>,
        message_handlers: MessageHandlers,
    ) -> QuicTransport {
        QuicTransport {
            out_connection_attempts: WaitGroup::new(),
            fallback_function,
            listeners: Default::default(),
            connections: Arc::new(RwLock::new(HashMap::new())),
            active_connections,
            message_handlers,
        }
    }
}

impl Transport for QuicTransport {
    type OutConnectionConfig = QuicOutConnectionConfig;

    type Endpoint = QuicEndpoint;

    fn start_listener<T: HandshakeHandler>(
        &mut self,
        self_keypair: KeyPair,
        address: SocketAddr,
        handshake_handler: T,
    ) -> Result<(), PeerNetError> {
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
        config.enable_dgram(true, 10, 10);

        let listener_handle = std::thread::spawn({
            let active_connections = self.active_connections.clone();
            let message_handlers = self.message_handlers.clone();
            let fallback_function = self.fallback_function.clone();
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

                                        {
                                            let mut active_connections = active_connections.write();
                                            if active_connections.nb_in_connections
                                                < active_connections.max_in_connections
                                            {
                                                active_connections.nb_in_connections += 1;
                                            } else {
                                                continue;
                                            }
                                        }
                                        let (send_tx, send_rx) = channel::bounded(10000);
                                        let (recv_tx, recv_rx) = channel::bounded(10000);
                                        {
                                            let mut connections = connections.write();
                                            connections.insert(
                                                from_addr,
                                                (connection, send_rx, recv_tx, false),
                                            );
                                        }
                                        new_peer(
                                            self_keypair.clone(),
                                            Endpoint::Quic(QuicEndpoint {
                                                data_receiver: recv_rx,
                                                data_sender: send_tx,
                                            }),
                                            handshake_handler.clone(),
                                            message_handlers.clone(),
                                            active_connections.clone(),
                                        );
                                    }
                                    {
                                        let mut connections = connections.write();
                                        //TODO: Handle if the peer wasn't created because no place it will fail
                                        let (connection, _, sender, is_established) =
                                            connections.get_mut(&from_addr).unwrap();
                                        let recv_info = quiche::RecvInfo {
                                            from: from_addr,
                                            to: address,
                                        };
                                        connection.recv(&mut buf[..num_recv], recv_info).unwrap();
                                        if *is_established {
                                            let mut dgram_buf = [0; 512];
                                            while let Ok(len) =
                                                connection.dgram_recv(&mut dgram_buf)
                                            {
                                                sender
                                                    .send(QuicInternalMessage::Data(
                                                        dgram_buf[..len].to_vec(),
                                                    ))
                                                    .unwrap();
                                            }
                                        }
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
                        for (address, (connection, send_rx, _, is_established)) in
                            connections.iter_mut()
                        {
                            if !*is_established {
                                if connection.is_established() {
                                    println!("server {}: Connection established", address);
                                    *is_established = true;
                                }
                            }
                            if *is_established {
                                while let Ok(data) = send_rx.try_recv() {
                                    match data {
                                        QuicInternalMessage::Data(data) => {
                                            //TODO: Use stream send didn't know how to use it
                                            let _ = connection.dgram_send(&data);
                                        }
                                        QuicInternalMessage::Shutdown => {
                                            println!("server {}: Connection closed", address);
                                            //TODO: Close
                                            //connection.close(app, err, reason)
                                            break;
                                        }
                                    }
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
        {
            let mut active_connections = self.active_connections.write();
            active_connections
                .listeners
                .insert(address, super::TransportType::Quic);
        }
        self.listeners.insert(
            address,
            (waker, server.try_clone().unwrap(), listener_handle),
        );
        Ok(())
    }

    fn try_connect<T: HandshakeHandler>(
        &mut self,
        self_keypair: KeyPair,
        address: SocketAddr,
        _timeout: Duration,
        config: &Self::OutConnectionConfig,
        handshake_handler: T,
    ) -> Result<(), PeerNetError> {
        //TODO: Use timeout
        let config = config.clone();
        let connections = self.connections.clone();
        let (_, socket, _) = if self.listeners.contains_key(&config.local_addr) {
            self.listeners
                .get(&config.local_addr)
                .expect("Listener not found")
        } else {
            self.start_listener(
                self_keypair.clone(),
                config.local_addr,
                handshake_handler.clone(),
            )?;
            //TODO: Make things more elegant with waker etc
            std::thread::sleep(Duration::from_millis(100));
            self.listeners
                .get(&config.local_addr)
                .expect("Listener not found")
        };
        let socket = socket.try_clone().unwrap();
        std::thread::spawn({
            let active_connections = self.active_connections.clone();
            let message_handlers = self.message_handlers.clone();
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
                quiche_config.enable_dgram(true, 10, 10);
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
                //TODO: Config
                let (send_tx, send_rx) = channel::bounded(10000);
                let (recv_tx, recv_rx) = channel::bounded(10000);
                {
                    let mut active_connections = active_connections.write();
                    if active_connections.nb_out_connections
                        < active_connections.max_out_connections
                    {
                        active_connections.nb_out_connections += 1;
                        {
                            let mut connections = connections.write();
                            connections.insert(address, (conn, send_rx, recv_tx, false));
                        }
                    }
                }
                new_peer(
                    self_keypair.clone(),
                    Endpoint::Quic(QuicEndpoint {
                        data_receiver: recv_rx,
                        data_sender: send_tx,
                    }),
                    handshake_handler.clone(),
                    message_handlers.clone(),
                    active_connections.clone(),
                );
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
        {
            let mut active_connections = self.active_connections.write();
            active_connections.listeners.remove(&address);
        }
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
        let data = endpoint
            .data_receiver
            .recv()
            .map_err(|err| PeerNetError::ReceiveError(err.to_string()))?;
        match data {
            QuicInternalMessage::Data(data) => Ok(data),
            QuicInternalMessage::Shutdown => {
                Err(PeerNetError::ReceiveError("Connection closed".to_string()))
            }
        }
    }
}
