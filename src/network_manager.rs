//! The PeerNetManager is the main struct of the PeerNet library.
//!
//! It is the entry point of the library and is used to create and manage the transports and the peers.

use std::collections::HashSet;
use std::net::IpAddr;
use std::thread::JoinHandle;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use crate::config::PeerNetCategoryInfo;
use crate::context::Context;
use crate::messages::MessagesHandler;
use crate::peer::PeerConnectionType;
use crate::peer_id::PeerId;
use crate::transports::{
    QuicConnectionConfig, QuicTransportConfig, TcpConnectionConfig, TcpTransportConfig,
    TransportConfig,
};
use parking_lot::RwLock;

use crate::{
    config::PeerNetConfiguration,
    error::PeerNetResult,
    peer::{InitConnectionHandler, PeerConnection, SendChannels},
    transports::{endpoint::Endpoint, InternalTransportType, Transport, TransportType},
};

#[derive(Debug)]
pub struct ActiveConnections<Id: PeerId> {
    pub nb_in_connections: usize,
    pub nb_out_connections: usize,
    /// Peers attempting to connect but not yet finished initialization
    pub connection_queue: HashSet<SocketAddr>,
    pub connections: HashMap<Id, PeerConnection>,
    pub listeners: HashMap<SocketAddr, TransportType>,
}

// TODO: Use std one when stable
pub(crate) fn to_canonical(ip: IpAddr) -> IpAddr {
    match ip {
        v4 @ IpAddr::V4(_) => v4,
        IpAddr::V6(v6) => {
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return IpAddr::V4(mapped);
            }
            IpAddr::V6(v6)
        }
    }
}

impl<Id: PeerId> ActiveConnections<Id> {
    /// Check if a new connection from a specific address can be accepted or not
    pub fn check_addr_accepted_pre_handshake(
        &self,
        addr: &SocketAddr,
        category_name: Option<String>,
        category_info: PeerNetCategoryInfo,
    ) -> bool {
        let mut nb_connection_for_this_ip = 0;
        let mut nb_connection_for_this_category = 0;
        let ip = to_canonical(addr.ip());

        for connection in self.connections.values() {
            if connection.connection_type == PeerConnectionType::IN {
                let connection_ip = to_canonical(connection.endpoint.get_target_addr().ip());
                // Check if a connection is already established with the same IP
                if connection_ip == ip {
                    nb_connection_for_this_ip += 1;
                }
                // Check the number of connection for the same category
                if connection.category_name == category_name {
                    nb_connection_for_this_category += 1;
                }
            }
        }
        println!("AURELIEN: category {:?}, nb_connection_for_this_ip: {}, nb_connection_for_this_category: {}, max_in_connections_per_ip: {}, max_in_connections_per_category: {}", category_name, nb_connection_for_this_ip, nb_connection_for_this_category, category_info.max_in_connections_per_ip, category_info.max_in_connections);
        nb_connection_for_this_ip < category_info.max_in_connections_per_ip
            && nb_connection_for_this_category < category_info.max_in_connections
    }

    pub fn check_addr_accepted_post_handshake(
        &self,
        addr: &SocketAddr,
        category_name: Option<String>,
        category_info: PeerNetCategoryInfo,
        id: &Id,
        connection_type: PeerConnectionType,
    ) -> bool {
        let mut nb_connection_for_this_ip = 0;
        let mut nb_connection_for_this_category = 0;
        let ip = to_canonical(addr.ip());
        if self.connections.contains_key(id) {
            return false;
        }
        for connection in self.connections.values() {
            if connection.connection_type == connection_type {
                let connection_ip = to_canonical(connection.endpoint.get_target_addr().ip());
                // Check if a connection is already established with the same IP
                if connection_ip == ip {
                    nb_connection_for_this_ip += 1;
                }
                // Check the number of connection for the same category
                if connection.category_name == category_name {
                    nb_connection_for_this_category += 1;
                }
            }
        }
        println!("AURELIEN: category {:?} connection_type: {:?}, nb_connection_for_this_ip: {}, nb_connection_for_this_category: {}, max_in_connections_per_ip: {}, max_in_connections_per_category: {}, max_out_connections_per_category: {}", category_name, connection_type, nb_connection_for_this_ip, nb_connection_for_this_category, category_info.max_in_connections_per_ip, category_info.max_in_connections, category_info.max_out_connections);
        let category_check = if connection_type == PeerConnectionType::IN {
            nb_connection_for_this_category < category_info.max_in_connections
        } else {
            nb_connection_for_this_category < category_info.max_out_connections
        };

        nb_connection_for_this_ip < category_info.max_in_connections_per_ip && category_check
    }

    pub fn confirm_connection(
        &mut self,
        id: Id,
        mut endpoint: Endpoint,
        send_channels: SendChannels,
        connection_type: PeerConnectionType,
        category_name: Option<String>,
        category_info: PeerNetCategoryInfo,
    ) -> bool {
        if self.check_addr_accepted_post_handshake(
            endpoint.get_target_addr(),
            category_name.clone(),
            category_info,
            &id,
            connection_type,
        ) {
            self.connections.insert(
                id,
                PeerConnection {
                    send_channels,
                    category_name,
                    //TODO: Should be only the field that allow to shutdown the connection. As it's
                    //transport specific, it should be a wrapped type `ShutdownHandle`
                    endpoint,
                    connection_type,
                },
            );
            self.compute_counters();
            true
        } else {
            endpoint.shutdown();
            self.compute_counters();
            false
        }
    }

    pub fn remove_connection(&mut self, id: &Id) {
        println!("Removing connection from: {:?}", id);
        if let Some(mut connection) = self.connections.remove(id) {
            connection.shutdown();
            self.compute_counters();
        }
    }

    pub fn compute_counters(&mut self) {
        self.nb_in_connections = self
            .connections
            .iter()
            .filter(|(_, connection)| connection.connection_type == PeerConnectionType::IN)
            .count();
        self.nb_out_connections = self
            .connections
            .iter()
            .filter(|(_, connection)| connection.connection_type == PeerConnectionType::OUT)
            .count();
    }
}

pub type SharedActiveConnections<Id> = Arc<RwLock<ActiveConnections<Id>>>;

/// Main structure of the PeerNet library used to manage the transports and the peers.
pub struct PeerNetManager<
    Id: PeerId,
    Ctx: Context<Id>,
    I: InitConnectionHandler<Id, Ctx, M>,
    M: MessagesHandler<Id>,
> {
    pub config: PeerNetConfiguration<Id, Ctx, I, M>,
    pub active_connections: SharedActiveConnections<Id>,
    message_handler: M,
    init_connection_handler: I,
    context: Ctx,
    transports: HashMap<TransportType, InternalTransportType<Id>>,
    total_bytes_received: Arc<RwLock<u64>>,
    total_bytes_sent: Arc<RwLock<u64>>,
}

impl<
        Id: PeerId,
        Ctx: Context<Id>,
        I: InitConnectionHandler<Id, Ctx, M>,
        M: MessagesHandler<Id>,
    > PeerNetManager<Id, Ctx, I, M>
{
    /// Creates a new PeerNetManager. Initializes a new database of peers and have no transports by default.
    pub fn new(config: PeerNetConfiguration<Id, Ctx, I, M>) -> PeerNetManager<Id, Ctx, I, M> {
        let context = config.context.clone();
        let active_connections = Arc::new(RwLock::new(ActiveConnections {
            nb_out_connections: 0,
            nb_in_connections: 0,
            connection_queue: HashSet::new(),
            connections: Default::default(),
            listeners: Default::default(),
        }));

        #[cfg(feature = "deadlock_detection")]
        {
            // only for #[cfg]
            use parking_lot::deadlock;
            use std::thread;
            use std::time::Duration;

            // Create a background thread which checks for deadlocks every 10s
            thread::spawn(move || loop {
                thread::sleep(Duration::from_secs(10));
                println!("Checking for deadlocks");
                let deadlocks = deadlock::check_deadlock();
                if deadlocks.is_empty() {
                    continue;
                }

                println!("{} deadlocks detected", deadlocks.len());
                for (i, threads) in deadlocks.iter().enumerate() {
                    println!("Deadlock #{}", i);
                    for t in threads {
                        println!("Thread Id {:#?}", t.thread_id());
                        println!("{:#?}", t.backtrace());
                    }
                }
            });
        } // only for #[cfg]
        PeerNetManager {
            init_connection_handler: config.init_connection_handler.clone(),
            message_handler: config.message_handler.clone(),
            config,
            context,
            transports: Default::default(),
            active_connections,
            total_bytes_received: Arc::new(RwLock::new(0)),
            total_bytes_sent: Arc::new(RwLock::new(0)),
        }
    }

    /// Starts a listener on the given address and transport type.
    /// The listener will accept incoming connections, verify we have seats for the peer and then create a new peer and his thread.
    pub fn start_listener(
        &mut self,
        transport_type: TransportType,
        addr: SocketAddr,
    ) -> PeerNetResult<()> {
        let transport = self.transports.entry(transport_type).or_insert_with(|| {
            InternalTransportType::from_transport_type(
                transport_type,
                self.active_connections.clone(),
                //TODO: Find a better way to avoid match there
                match transport_type {
                    TransportType::Tcp => TransportConfig::Tcp(Box::new(TcpTransportConfig {
                        max_in_connections: self.config.max_in_connections,
                        peer_categories: self.config.peers_categories.clone(),
                        default_category_info: self.config.default_category_info,
                        connection_config: TcpConnectionConfig {
                            rate_limit: self.config.rate_limit,
                            rate_time_window: self.config.rate_time_window,
                            rate_bucket_size: self.config.rate_bucket_size,
                            data_channel_size: self.config.send_data_channel_size,
                            max_message_size: self.config.max_message_size,
                            read_timeout: self.config.read_timeout,
                            write_timeout: self.config.write_timeout,
                        },
                        read_timeout: self.config.read_timeout,
                        write_timeout: self.config.write_timeout,
                    })),
                    TransportType::Quic => TransportConfig::Quic(Box::new(QuicTransportConfig {
                        connection_config: QuicConnectionConfig {
                            local_addr: "127.0.0.1:8080".parse().unwrap(),
                            data_channel_size: self.config.send_data_channel_size,
                        },
                    })),
                },
                self.config.optional_features.clone(),
                addr,
                self.total_bytes_received.clone(),
                self.total_bytes_sent.clone(),
            )
        });
        transport.start_listener(
            self.context.clone(),
            addr,
            self.message_handler.clone(),
            self.init_connection_handler.clone(),
        )?;
        Ok(())
    }

    /// Stops a listener on the given address and transport type.
    /// TODO: Maybe have listener ids
    pub fn stop_listener(
        &mut self,
        transport_type: TransportType,
        addr: SocketAddr,
    ) -> PeerNetResult<()> {
        let transport = self.transports.entry(transport_type).or_insert_with(|| {
            InternalTransportType::from_transport_type(
                transport_type,
                self.active_connections.clone(),
                //TODO: Find a better way to avoid match there
                match transport_type {
                    TransportType::Tcp => TransportConfig::Tcp(Box::new(TcpTransportConfig {
                        max_in_connections: self.config.max_in_connections,
                        peer_categories: self.config.peers_categories.clone(),
                        default_category_info: self.config.default_category_info,
                        connection_config: TcpConnectionConfig {
                            rate_limit: self.config.rate_limit,
                            rate_time_window: self.config.rate_time_window,
                            rate_bucket_size: self.config.rate_bucket_size,
                            data_channel_size: self.config.send_data_channel_size,
                            max_message_size: self.config.max_message_size,
                            read_timeout: self.config.read_timeout,
                            write_timeout: self.config.write_timeout,
                        },
                        read_timeout: self.config.read_timeout,
                        write_timeout: self.config.write_timeout,
                    })),
                    TransportType::Quic => TransportConfig::Quic(Box::new(QuicTransportConfig {
                        connection_config: QuicConnectionConfig {
                            local_addr: "127.0.0.1:8080".parse().unwrap(),
                            data_channel_size: self.config.send_data_channel_size,
                        },
                    })),
                },
                self.config.optional_features.clone(),
                addr,
                self.total_bytes_received.clone(),
                self.total_bytes_sent.clone(),
            )
        });
        transport.stop_listener(addr)?;
        Ok(())
    }

    /// Tries to connect to the given address and transport type.
    /// The transport used is defined by the variant of the OutConnectionConfig.
    /// If the connection can be established, a new peer is created and his thread is started.
    pub fn try_connect(
        &mut self,
        transport_type: TransportType,
        addr: SocketAddr,
        timeout: std::time::Duration,
    ) -> PeerNetResult<JoinHandle<PeerNetResult<()>>> {
        let transport = self.transports.entry(transport_type).or_insert_with(|| {
            InternalTransportType::from_transport_type(
                transport_type,
                self.active_connections.clone(),
                //TODO: Find a better way to avoid match there
                match transport_type {
                    TransportType::Tcp => TransportConfig::Tcp(Box::new(TcpTransportConfig {
                        max_in_connections: self.config.max_in_connections,
                        peer_categories: self.config.peers_categories.clone(),
                        default_category_info: self.config.default_category_info,
                        connection_config: TcpConnectionConfig {
                            rate_limit: self.config.rate_limit,
                            rate_time_window: self.config.rate_time_window,
                            rate_bucket_size: self.config.rate_bucket_size,
                            data_channel_size: self.config.send_data_channel_size,
                            max_message_size: self.config.max_message_size,
                            read_timeout: self.config.read_timeout,
                            write_timeout: self.config.write_timeout,
                        },
                        read_timeout: self.config.read_timeout,
                        write_timeout: self.config.write_timeout,
                    })),
                    TransportType::Quic => TransportConfig::Quic(Box::new(QuicTransportConfig {
                        connection_config: QuicConnectionConfig {
                            local_addr: "127.0.0.1:8080".parse().unwrap(),
                            data_channel_size: self.config.send_data_channel_size,
                        },
                    })),
                },
                self.config.optional_features.clone(),
                addr,
                self.total_bytes_received.clone(),
                self.total_bytes_sent.clone(),
            )
        });
        transport.try_connect(
            self.context.clone(),
            addr,
            timeout,
            self.message_handler.clone(),
            self.init_connection_handler.clone(),
        )
    }

    /// Get the nb_in_connections of manager
    pub fn nb_in_connections(&self) -> usize {
        self.active_connections.read().nb_in_connections
    }

    pub fn get_total_bytes_received(&self) -> u64 {
        *self.total_bytes_received.read()
    }

    pub fn get_total_bytes_sent(&self) -> u64 {
        *self.total_bytes_sent.read()
    }
}

impl<
        Id: PeerId,
        Ctx: Context<Id>,
        I: InitConnectionHandler<Id, Ctx, M>,
        M: MessagesHandler<Id>,
    > Drop for PeerNetManager<Id, Ctx, I, M>
{
    fn drop(&mut self) {
        {
            let mut active_connections = self.active_connections.write();
            for (_, mut peer) in active_connections.connections.drain() {
                peer.shutdown();
            }
        }
    }
}
