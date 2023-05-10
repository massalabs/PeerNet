use std::{
    collections::HashMap,
    net::{SocketAddr, UdpSocket},
    sync::Arc,
    thread::JoinHandle,
    time::Duration,
};

use crate::{
    config::PeerNetCategoryInfo, messages::MessagesHandler, peer::PeerConnectionType,
    peer_id::PeerNetIdTrait, types::KeyPair,
};
use crossbeam::{channel, sync::WaitGroup};
use mio::{net::UdpSocket as MioUdpSocket, Events, Interest, Poll, Token, Waker};
use parking_lot::RwLock;

use crate::{
    config::PeerNetFeatures,
    error::{PeerNetError, PeerNetResult},
    network_manager::SharedActiveConnections,
    peer::{new_peer, InitConnectionHandler},
    transports::{Endpoint, TransportErrorType},
};

use crossbeam::channel::{unbounded, Receiver, Sender};

use super::Transport;

const NEW_PACKET_SERVER: Token = Token(0);
const STOP_LISTENER: Token = Token(10);

#[derive(Debug, PartialEq, Eq)]
pub enum QuicError {
    InitListener,
    StopListener,
    SocketConfig,
    QuicheConfig,
    ConnectionError,
    InternalFail,
}

impl QuicError {
    fn wrap(self) -> PeerNetError {
        PeerNetError::TransportError(TransportErrorType::Quic(self))
    }
}

type QuicConnection = (
    quiche::Connection,
    channel::Receiver<QuicInternalMessage>,
    channel::Sender<QuicInternalMessage>,
    bool,
);
type QuicConnectionsMap = Arc<RwLock<HashMap<SocketAddr, QuicConnection>>>;

pub(crate) struct QuicTransport<Id: PeerNetIdTrait> {
    pub active_connections: SharedActiveConnections<Id>,
    //pub fallback_function: Option<&'static FallbackFunction>,
    pub out_connection_attempts: WaitGroup,
    pub listeners: HashMap<SocketAddr, (Waker, UdpSocket, JoinHandle<PeerNetResult<()>>)>,
    //(quiche::Connection, data_receiver, data_sender, is_established)
    pub connections: QuicConnectionsMap,
    _features: PeerNetFeatures,
    stop_peer_tx: Sender<()>,
    stop_peer_rx: Receiver<()>,
}

pub(crate) enum QuicInternalMessage {
    Data(Vec<u8>),
    Shutdown,
}

#[derive(Clone)]
pub struct QuicEndpoint {
    pub(crate) data_sender: channel::Sender<QuicInternalMessage>,
    pub(crate) data_receiver: channel::Receiver<QuicInternalMessage>,
    pub address: SocketAddr,
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
    pub local_addr: SocketAddr,
}

impl<Id: PeerNetIdTrait> QuicTransport<Id> {
    pub fn new(
        active_connections: SharedActiveConnections<Id>,
        features: PeerNetFeatures,
    ) -> QuicTransport<Id> {
        let (stop_peer_tx, stop_peer_rx) = unbounded();
        QuicTransport {
            out_connection_attempts: WaitGroup::new(),
            listeners: Default::default(),
            connections: Arc::new(RwLock::new(HashMap::new())),
            active_connections,
            _features: features,
            stop_peer_tx,
            stop_peer_rx,
        }
    }
}

impl<Id: PeerNetIdTrait> Transport for QuicTransport<Id> {
    type OutConnectionConfig = QuicOutConnectionConfig;

    type Endpoint = QuicEndpoint;

    fn start_listener<H: InitConnectionHandler, M: MessagesHandler>(
        &mut self,
        self_keypair: KeyPair,
        address: SocketAddr,
        message_handler: M,
        init_connection_handler: H,
    ) -> PeerNetResult<()> {
        let mut poll = Poll::new()
            .map_err(|err| QuicError::InitListener.wrap().new("init poll", err, None))?;
        //TODO: Configurable capacity
        let mut events = Events::with_capacity(128);
        let waker = Waker::new(poll.registry(), STOP_LISTENER)
            .map_err(|err| QuicError::InitListener.wrap().new("init waker", err, None))?;
        let connections = self.connections.clone();
        let server = UdpSocket::bind(address)
            .unwrap_or_else(|_| panic!("Can't bind QUIC transport to address {}", address));
        server.set_nonblocking(true).map_err(|err| {
            QuicError::InitListener
                .wrap()
                .new("server set nonblocking", err, None)
        })?;

        let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION).map_err(|err| {
            QuicError::QuicheConfig.wrap().new(
                "new from protocol",
                err,
                Some(format!("version: {:?}", quiche::PROTOCOL_VERSION)),
            )
        })?;
        config.set_max_recv_udp_payload_size(1200);
        // Create certificate from ed25519 as made in libp2p tls
        config
            .load_cert_chain_from_pem_file("./src/cert.crt")
            .map_err(|err| {
                QuicError::QuicheConfig
                    .wrap()
                    .new("load_cert_chain", err, None)
            })?;
        config
            .load_priv_key_from_pem_file("./src/cert.key")
            .map_err(|err| {
                QuicError::QuicheConfig
                    .wrap()
                    .new("load_priv_key", err, None)
            })?;
        config
            .set_application_protos(&[b"massa/1.0"])
            .map_err(|err| {
                QuicError::QuicheConfig
                    .wrap()
                    .new("cfg set_protocol", err, None)
            })?;
        config.enable_dgram(true, 10, 10);

        let listener_handle: JoinHandle<PeerNetResult<()>> = std::thread::Builder::new()
            .name(format!("quic_listener_handle_{:?}", address))
            .spawn({
                let active_connections = self.active_connections.clone();
                let server = server.try_clone().unwrap();
                let stop_peer_rx = self.stop_peer_rx.clone();
                let stop_peer_tx = self.stop_peer_tx.clone();
                move || {
                    let mut socket = MioUdpSocket::from_std(server);
                    // Start listening for incoming connections.
                    poll.registry()
                        .register(&mut socket, NEW_PACKET_SERVER, Interest::READABLE)
                        .unwrap_or_else(|_| {
                            panic!(
                                "Can't register polling on QUIC transport of address {}",
                                address
                            )
                        });
                    let mut buf = [0; 65507];
                    loop {
                        // Poll Mio for events, blocking until we get an event.
                        //TODO: Configurable timeout (cf. https://github.com/cloudflare/quiche/blob/master/apps/src/bin/quiche-server.rs#L177)
                        poll.poll(&mut events, Some(Duration::from_millis(100)))
                            .unwrap_or_else(|_| {
                                panic!("Can't poll QUIC transport of address {}", address)
                            });

                        // Process each event.
                        for event in events.iter() {
                            match event.token() {
                                NEW_PACKET_SERVER => {
                                    'read: loop {
                                        //TODO: Error handling
                                        let (num_recv, from_addr) = match socket.recv_from(&mut buf)
                                        {
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
                                            .map_err(|err| {
                                                QuicError::ConnectionError.wrap().new(
                                                    "accept",
                                                    err,
                                                    Some(format!(
                                                        "address: {}, from_addr: {}",
                                                        address, from_addr
                                                    )),
                                                )
                                            })?;

                                            //TODO: Make filter connection quic
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
                                                    address,
                                                }),
                                                init_connection_handler.clone(),
                                                message_handler.clone(),
                                                active_connections.clone(),
                                                stop_peer_rx.clone(),
                                                PeerConnectionType::IN,
                                                Some(String::from("quic")),
                                                PeerNetCategoryInfo {
                                                    max_in_connections_per_ip: 0,
                                                    max_in_connections_post_handshake: 0,
                                                    max_in_connections_pre_handshake: 0,
                                                },
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
                                            connection
                                                .recv(&mut buf[..num_recv], recv_info)
                                                .map_err(|err| {
                                                    QuicError::ConnectionError.wrap().new(
                                                        "recv",
                                                        err,
                                                        Some(format!(
                                                            "RecvInfo: from: {}, to: {}",
                                                            from_addr, address
                                                        )),
                                                    )
                                                })?;
                                            if *is_established {
                                                let mut dgram_buf = [0; 512];
                                                while let Ok(len) =
                                                    connection.dgram_recv(&mut dgram_buf)
                                                {
                                                    sender
                                                        .send(QuicInternalMessage::Data(
                                                            dgram_buf[..len].to_vec(),
                                                        ))
                                                        .map_err(|err| {
                                                            QuicError::InternalFail.wrap().new(
                                                                "send internal msg",
                                                                err,
                                                                None,
                                                            )
                                                        })?;
                                                }
                                            }
                                        }
                                    }
                                }
                                STOP_LISTENER => {
                                    stop_peer_tx.send(()).unwrap();
                                    return Ok(());
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
                                if !*is_established && connection.is_established() {
                                    println!("server {}: Connection established", address);
                                    *is_established = true;
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
                                    socket.send_to(&buf[..write], send_info.to).map_err(|err| {
                                        QuicError::ConnectionError.wrap().new(
                                            "listener send_to",
                                            err,
                                            Some(format!(
                                                "from {}, to {}, {} bytes",
                                                address, send_info.to, write
                                            )),
                                        )
                                    })?;
                                }
                            }
                        }
                    }
                }
            })
            .expect("Failed to spawn thread quic_listener_handle");
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

    fn try_connect<H: InitConnectionHandler, M: MessagesHandler>(
        &mut self,
        self_keypair: KeyPair,
        address: SocketAddr,
        _timeout: Duration,
        config: &Self::OutConnectionConfig,
        message_handler: M,
        init_connection_handler: H,
    ) -> PeerNetResult<JoinHandle<PeerNetResult<()>>> {
        let stop_peer_rx = self.stop_peer_rx.clone();
        //TODO: Use timeout
        let config = config.clone();
        let (_, socket, _) = if self.listeners.contains_key(&config.local_addr) {
            self.listeners
                .get(&config.local_addr)
                .expect("Listener not found")
        } else {
            self.start_listener(
                self_keypair.clone(),
                config.local_addr,
                message_handler.clone(),
                init_connection_handler.clone(),
            )?;
            //TODO: Make things more elegant with waker etc
            std::thread::sleep(Duration::from_millis(100));
            self.listeners
                .get(&config.local_addr)
                .expect("Listener not found")
        };
        let socket = socket.try_clone().unwrap();
        let connection_handler: JoinHandle<PeerNetResult<()>> = std::thread::Builder::new()
            .name(format!("quic_try_connect_{:?}", address))
            .spawn({
                let active_connections = self.active_connections.clone();
                let wg = self.out_connection_attempts.clone();
                move || {
                    let mut out = [0; 65507];
                    println!("Connecting to {}", address);
                    //TODO: Use configs for quiche passed from config object.
                    //and error handling
                    let mut quiche_config = quiche::Config::new(quiche::PROTOCOL_VERSION)
                        .expect("Default config failed");
                    quiche_config.verify_peer(false);
                    //TODO: Config
                    quiche_config
                        .set_application_protos(&[b"massa/1.0"])
                        .map_err(|err| {
                            QuicError::QuicheConfig.wrap().new("cfg proto", err, None)
                        })?;
                    quiche_config.enable_dgram(true, 10, 10);
                    //TODO: random bytes
                    let scid = [0; quiche::MAX_CONN_ID_LEN];
                    let scid = quiche::ConnectionId::from_ref(&scid);
                    let mut conn = quiche::connect(
                        None,
                        &scid,
                        config.local_addr,
                        address,
                        &mut quiche_config,
                    )
                    .map_err(|err| {
                        QuicError::ConnectionError.wrap().new(
                            "try_connect connect",
                            err,
                            Some(format!(
                                "local_addr: {:?}, addr: {:?}",
                                config.local_addr, address
                            )),
                        )
                    })?;
                    loop {
                        let (write, send_info) = match conn.send(&mut out) {
                            Ok(v) => v,
                            Err(quiche::Error::Done) => {
                                break;
                            }
                            Err(e) => {
                                println!("send failed: {:?}", e);
                                return Err(QuicError::ConnectionError.wrap().new(
                                    "try_connect conn.send",
                                    e,
                                    None,
                                ));
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
                            return Err(QuicError::ConnectionError.wrap().new(
                                "quic try_connect socket.send_to",
                                e,
                                None,
                            ));
                        }
                    }
                    //TODO: Config
                    let (send_tx, _send_rx) = channel::bounded(10000);
                    let (_recv_tx, recv_rx) = channel::bounded(10000);
                    new_peer(
                        self_keypair.clone(),
                        Endpoint::Quic(QuicEndpoint {
                            data_receiver: recv_rx,
                            data_sender: send_tx,
                            address,
                        }),
                        init_connection_handler.clone(),
                        message_handler.clone(),
                        active_connections.clone(),
                        stop_peer_rx.clone(),
                        PeerConnectionType::OUT,
                        //TODO: Change
                        Some(String::from("quic")),
                        PeerNetCategoryInfo {
                            max_in_connections_per_ip: 0,
                            max_in_connections_post_handshake: 0,
                            max_in_connections_pre_handshake: 0,
                        },
                    );
                    drop(wg);
                    Ok(())
                }
            })
            .expect("Failed to spawn thread quic_listener_handle");
        Ok(connection_handler)
    }

    fn stop_listener(&mut self, address: SocketAddr) -> PeerNetResult<()> {
        let (waker, _, handle) =
            self.listeners
                .remove(&address)
                .ok_or(QuicError::InternalFail.wrap().error(
                    "stop_listener rm addr",
                    Some(format!("address: {}", address)),
                ))?;
        {
            let mut active_connections = self.active_connections.write();
            active_connections.listeners.remove(&address);
        }
        waker
            .wake()
            .map_err(|e| QuicError::StopListener.wrap().new("waker wake", e, None))?;
        let _ = handle
            .join()
            .unwrap_or_else(|_| panic!("Couldn't join listener for address {}", address));
        Ok(())
    }

    fn send(endpoint: &mut Self::Endpoint, data: &[u8]) -> PeerNetResult<()> {
        endpoint
            .data_sender
            .send(QuicInternalMessage::Data(data.to_vec()))
            .map_err(|err| {
                QuicError::ConnectionError
                    .wrap()
                    .new("data_sender send", err, None)
            })
    }

    fn receive(endpoint: &mut Self::Endpoint) -> PeerNetResult<Vec<u8>> {
        let data = endpoint.data_receiver.recv().map_err(|err| {
            QuicError::ConnectionError
                .wrap()
                .new("data_receiver recv", err, None)
        })?;
        match data {
            QuicInternalMessage::Data(data) => Ok(data),
            QuicInternalMessage::Shutdown => Err(QuicError::InternalFail
                .wrap()
                .error("recv shutdown", Some("Connection closed".to_string()))),
        }
    }
}
