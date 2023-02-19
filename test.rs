#![feature(prelude_import)]
#[prelude_import]
use std::prelude::rust_2021::*;
#[macro_use]
extern crate std;
mod endpoint {
    pub struct Endpoint {}
}
pub mod peer_id {
    use massa_signature::PublicKey;
    pub struct PeerId {
        public_key: PublicKey,
    }
    impl PeerId {
        pub fn from_public_key(public_key: PublicKey) -> PeerId {
            PeerId { public_key }
        }
        pub fn to_bytes(&self) -> Vec<u8> {
            self.public_key.to_bytes().to_vec()
        }
    }
}
pub mod config {
    use massa_signature::PublicKey;
    use crate::{peer::PeerMetadata, peer_id::PeerId};
    pub struct PeerNetConfiguration {
        /// Number of peers we want to have in IN connection
        pub max_in_connections: usize,
        /// Number of peers we want to have in OUT connection
        pub max_out_connections: usize,
        pub peer_id: PeerId,
        /// Initial peer list
        pub initial_peer_list: Vec<PeerMetadata>,
    }
}
pub mod error {
    pub enum PeerNetError {
        ListenerError(String),
        PeerConnectionError(String),
        InitializationError(String),
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for PeerNetError {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match self {
                PeerNetError::ListenerError(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(f, "ListenerError", &__self_0)
                }
                PeerNetError::PeerConnectionError(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "PeerConnectionError",
                        &__self_0,
                    )
                }
                PeerNetError::InitializationError(__self_0) => {
                    ::core::fmt::Formatter::debug_tuple_field1_finish(
                        f,
                        "InitializationError",
                        &__self_0,
                    )
                }
            }
        }
    }
}
pub mod network_manager {
    use std::{collections::HashMap, net::SocketAddr, sync::Arc};
    use parking_lot::RwLock;
    use crate::{
        config::PeerNetConfiguration,
        error::PeerNetError,
        peer::Peer,
        transports::{InternalTransportType, Transport, TransportType},
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
            Ok(())
        }
        pub fn try_connect<T>(
            &mut self,
            transport_type: TransportType,
            addr: SocketAddr,
            timeout: std::time::Duration,
        ) -> Result<(), PeerNetError> {
            let transport = self.transports.entry(transport_type).or_insert_with(|| {
                InternalTransportType::from_transport_type(transport_type, self.peer_db.clone())
            });
            Ok(())
        }
    }
    impl Drop for PeerNetManager {
        fn drop(&mut self) {}
    }
}
pub mod peer {
    use std::{
        net::SocketAddr,
        thread::{spawn, JoinHandle},
    };
    use crossbeam::channel::{unbounded, RecvError, Sender};
    use crate::{endpoint::Endpoint, transports::InternalTransportType};
    pub struct PeerMetadata {
        address: SocketAddr,
        public_key: String,
        transport: InternalTransportType,
    }
    pub(crate) struct Peer {
        endpoint: Endpoint,
        thread_handler: Option<JoinHandle<()>>,
        thread_sender: Sender<PeerMessage>,
    }
    enum PeerMessage {
        Stop,
    }
    impl Peer {
        pub(crate) fn new(endpoint: Endpoint) -> Peer {
            let (tx, rx) = unbounded();
            let handler = spawn(move || loop {
                match rx.recv() {
                    Ok(PeerMessage::Stop) => {
                        break;
                    }
                    Err(_) => {
                        return;
                    }
                }
            });
            Peer {
                endpoint,
                thread_handler: Some(handler),
                thread_sender: tx,
            }
        }
    }
}
pub mod transports {
    use std::{net::SocketAddr, time::Duration};
    use crate::{error::PeerNetError, network_manager::SharedPeerDB, peer_id::PeerId};
    use self::{quic::QuicTransport, tcp::TcpTransport};
    mod quic {
        use std::{collections::HashMap, net::SocketAddr, thread::JoinHandle, time::Duration};
        use mio::{net::UdpSocket, Events, Interest, Poll, Token, Waker};
        use quiche::{connect, ConnectionId};
        use crate::{error::PeerNetError, peer_id::PeerId};
        use super::Transport;
        const NEW_CONNECTION: Token = Token(0);
        const STOP_LISTENER: Token = Token(10);
        const MAX_BUF_SIZE: usize = 65507;
        pub struct QuicTransport {
            pub listeners: HashMap<SocketAddr, (Waker, JoinHandle<()>)>,
        }
        pub struct QuicOutConnectionConfig {
            pub identity: PeerId,
        }
        impl QuicTransport {
            pub fn new() -> QuicTransport {
                QuicTransport {
                    listeners: Default::default(),
                }
            }
        }
        impl Transport for QuicTransport {
            type OutConnectionConfig = QuicOutConnectionConfig;
            fn start_listener(&mut self, address: SocketAddr) -> Result<(), PeerNetError> {
                let mut poll =
                    Poll::new().map_err(|err| PeerNetError::ListenerError(err.to_string()))?;
                let mut events = Events::with_capacity(128);
                let waker = Waker::new(poll.registry(), STOP_LISTENER)
                    .map_err(|err| PeerNetError::ListenerError(err.to_string()))?;
                let listener_handle = std::thread::spawn(move || {
                    let mut socket = UdpSocket::bind(address).expect(&{
                        let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                            &["Can\'t bind TCP transport to address "],
                            &[::core::fmt::ArgumentV1::new_display(&address)],
                        ));
                        res
                    });
                    poll.registry()
                        .register(&mut socket, NEW_CONNECTION, Interest::READABLE)
                        .expect(&{
                            let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                                &["Can\'t register polling on TCP transport of address "],
                                &[::core::fmt::ArgumentV1::new_display(&address)],
                            ));
                            res
                        });
                    let mut buf = [0; MAX_BUF_SIZE];
                    loop {
                        poll.poll(&mut events, None).expect(&{
                            let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                                &["Can\'t poll QUIC transport of address "],
                                &[::core::fmt::ArgumentV1::new_display(&address)],
                            ));
                            res
                        });
                        for event in events.iter() {
                            match event.token() {
                                NEW_CONNECTION => {
                                    let (_num_recv, _from_addr) =
                                        socket.recv_from(&mut buf).unwrap();
                                    {
                                        ::std::io::_print(::core::fmt::Arguments::new_v1(
                                            &["New connection\n"],
                                            &[],
                                        ));
                                    };
                                }
                                STOP_LISTENER => {
                                    return;
                                }
                                _ => {}
                            }
                        }
                    }
                });
                self.listeners.insert(address, (waker, listener_handle));
                Ok(())
            }
            fn try_connect(
                &mut self,
                _address: SocketAddr,
                _timeout: Duration,
                _config: QuicOutConnectionConfig,
            ) -> Result<(), PeerNetError> {
                Ok(())
            }
            fn stop_listener(&mut self, address: SocketAddr) -> Result<(), PeerNetError> {
                let (waker, handle) =
                    self.listeners
                        .remove(&address)
                        .ok_or(PeerNetError::ListenerError({
                            let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                                &["Can\'t find listener for address "],
                                &[::core::fmt::ArgumentV1::new_display(&address)],
                            ));
                            res
                        }))?;
                waker
                    .wake()
                    .map_err(|e| PeerNetError::ListenerError(e.to_string()))?;
                handle.join().expect(&{
                    let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                        &["Couldn\'t join listener for address "],
                        &[::core::fmt::ArgumentV1::new_display(&address)],
                    ));
                    res
                });
                Ok(())
            }
        }
    }
    mod tcp {
        use std::collections::HashMap;
        use std::net::{SocketAddr, TcpStream};
        use std::thread::JoinHandle;
        use std::time::Duration;
        use crate::endpoint::Endpoint;
        use crate::error::PeerNetError;
        use crate::network_manager::SharedPeerDB;
        use crate::peer::Peer;
        use crate::peer_id::PeerId;
        use super::Transport;
        use crossbeam::sync::WaitGroup;
        use mio::net::TcpListener;
        use mio::{Events, Interest, Poll, Token, Waker};
        pub(crate) struct TcpTransport {
            pub peer_db: SharedPeerDB,
            pub out_connection_attempts: WaitGroup,
            pub listeners: HashMap<SocketAddr, (Waker, JoinHandle<()>)>,
        }
        const NEW_CONNECTION: Token = Token(0);
        const STOP_LISTENER: Token = Token(10);
        impl TcpTransport {
            pub fn new(peer_db: SharedPeerDB) -> TcpTransport {
                TcpTransport {
                    peer_db,
                    out_connection_attempts: WaitGroup::new(),
                    listeners: Default::default(),
                }
            }
        }
        impl Transport for TcpTransport {
            type OutConnectionConfig = ();
            fn start_listener(&mut self, address: SocketAddr) -> Result<(), PeerNetError> {
                let mut poll =
                    Poll::new().map_err(|err| PeerNetError::ListenerError(err.to_string()))?;
                let mut events = Events::with_capacity(128);
                let waker = Waker::new(poll.registry(), STOP_LISTENER)
                    .map_err(|err| PeerNetError::ListenerError(err.to_string()))?;
                let listener_handle: JoinHandle<()> = std::thread::spawn(move || {
                    let mut server = TcpListener::bind(address).expect(&{
                        let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                            &["Can\'t bind TCP transport to address "],
                            &[::core::fmt::ArgumentV1::new_display(&address)],
                        ));
                        res
                    });
                    poll.registry()
                        .register(&mut server, NEW_CONNECTION, Interest::READABLE)
                        .expect(&{
                            let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                                &["Can\'t register polling on TCP transport of address "],
                                &[::core::fmt::ArgumentV1::new_display(&address)],
                            ));
                            res
                        });
                    loop {
                        poll.poll(&mut events, None).expect(&{
                            let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                                &["Can\'t poll TCP transport of address "],
                                &[::core::fmt::ArgumentV1::new_display(&address)],
                            ));
                            res
                        });
                        for event in events.iter() {
                            match event.token() {
                                NEW_CONNECTION => {
                                    let connection = server.accept();
                                    {
                                        ::std::io::_print(::core::fmt::Arguments::new_v1(
                                            &["New connection\n"],
                                            &[],
                                        ));
                                    };
                                    drop(connection);
                                }
                                STOP_LISTENER => {
                                    return;
                                }
                                _ => {}
                            }
                        }
                    }
                });
                self.listeners.insert(address, (waker, listener_handle));
                Ok(())
            }
            fn try_connect(
                &mut self,
                address: SocketAddr,
                timeout: Duration,
                config: (),
            ) -> Result<(), PeerNetError> {
                std::thread::spawn({
                    let peer_db = self.peer_db.clone();
                    let wg = self.out_connection_attempts.clone();
                    move || {
                        let Ok (_connection) = TcpStream :: connect_timeout (& address , timeout) else { return ; } ;
                        {
                            ::std::io::_print(::core::fmt::Arguments::new_v1(
                                &["Connected to ", "\n"],
                                &[::core::fmt::ArgumentV1::new_display(&address)],
                            ));
                        };
                        let mut peer_db = peer_db.write();
                        if peer_db.nb_in_connections < peer_db.config.max_in_connections {
                            peer_db.nb_in_connections += 1;
                            let peer = Peer::new(Endpoint {});
                            peer_db.peers.push(peer);
                        }
                        drop(wg);
                    }
                });
                Ok(())
            }
            fn stop_listener(&mut self, address: SocketAddr) -> Result<(), PeerNetError> {
                let (waker, handle) =
                    self.listeners
                        .remove(&address)
                        .ok_or(PeerNetError::ListenerError({
                            let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                                &["Can\'t find listener for address "],
                                &[::core::fmt::ArgumentV1::new_display(&address)],
                            ));
                            res
                        }))?;
                waker
                    .wake()
                    .map_err(|e| PeerNetError::ListenerError(e.to_string()))?;
                handle.join().expect(&{
                    let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                        &["Couldn\'t join listener for address "],
                        &[::core::fmt::ArgumentV1::new_display(&address)],
                    ));
                    res
                });
                Ok(())
            }
        }
    }
    pub enum TransportType {
        Tcp,
        Quic,
    }
    #[automatically_derived]
    impl ::core::hash::Hash for TransportType {
        fn hash<__H: ::core::hash::Hasher>(&self, state: &mut __H) -> () {
            let __self_tag = ::core::intrinsics::discriminant_value(self);
            ::core::hash::Hash::hash(&__self_tag, state)
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralEq for TransportType {}
    #[automatically_derived]
    impl ::core::cmp::Eq for TransportType {
        #[inline]
        #[doc(hidden)]
        #[no_coverage]
        fn assert_receiver_is_total_eq(&self) -> () {}
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for TransportType {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for TransportType {
        #[inline]
        fn eq(&self, other: &TransportType) -> bool {
            let __self_tag = ::core::intrinsics::discriminant_value(self);
            let __arg1_tag = ::core::intrinsics::discriminant_value(other);
            __self_tag == __arg1_tag
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for TransportType {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match self {
                TransportType::Tcp => ::core::fmt::Formatter::write_str(f, "Tcp"),
                TransportType::Quic => ::core::fmt::Formatter::write_str(f, "Quic"),
            }
        }
    }
    #[automatically_derived]
    impl ::core::marker::Copy for TransportType {}
    #[automatically_derived]
    impl ::core::clone::Clone for TransportType {
        #[inline]
        fn clone(&self) -> TransportType {
            *self
        }
    }
    pub(crate) enum InternalTransportType {
        Tcp(TcpTransport),
        Quic(QuicTransport),
    }
    #[allow(non_snake_case)]
    mod enum_delegate_helpers_Transport_for_InternalTransportType {
        /// Indicates that a pair of types (A, B) are the same
        ///
        /// Used in generated where-clauses as a workaround for https://github.com/rust-lang/rust/issues/20041
        pub trait EqualTypes: private::Sealed {}
        impl<T> EqualTypes for (T, T) {}
        mod private {
            pub trait Sealed {}
            impl<T> Sealed for (T, T) {}
        }
    }
    #[allow(non_camel_case_types)]
    enum Transport_for_InternalTransportType_associated_type_OutConnectionConfig_generated_by_enum_delegate
    {
        Tcp(<TcpTransport as Transport>::OutConnectionConfig),
        Quic(<QuicTransport as Transport>::OutConnectionConfig),
    }
    impl From < < TcpTransport as Transport > :: OutConnectionConfig > for Transport_for_InternalTransportType_associated_type_OutConnectionConfig_generated_by_enum_delegate { fn from (inner : < TcpTransport as Transport > :: OutConnectionConfig) -> Self { Transport_for_InternalTransportType_associated_type_OutConnectionConfig_generated_by_enum_delegate :: Tcp (inner) } }
    impl TryInto < < TcpTransport as Transport > :: OutConnectionConfig > for Transport_for_InternalTransportType_associated_type_OutConnectionConfig_generated_by_enum_delegate { type Error = () ; fn try_into (self) -> Result < < TcpTransport as Transport > :: OutConnectionConfig , Self :: Error > { match self { Transport_for_InternalTransportType_associated_type_OutConnectionConfig_generated_by_enum_delegate :: Tcp (inner) => Ok (inner) , _ => Err (()) , } } }
    impl < 'a > TryInto < & 'a < TcpTransport as Transport > :: OutConnectionConfig > for & 'a Transport_for_InternalTransportType_associated_type_OutConnectionConfig_generated_by_enum_delegate { type Error = () ; fn try_into (self) -> Result < & 'a < TcpTransport as Transport > :: OutConnectionConfig , Self :: Error > { match self { Transport_for_InternalTransportType_associated_type_OutConnectionConfig_generated_by_enum_delegate :: Tcp (inner) => Ok (inner) , _ => Err (()) , } } }
    impl < 'a > TryInto < & 'a mut < TcpTransport as Transport > :: OutConnectionConfig > for & 'a mut Transport_for_InternalTransportType_associated_type_OutConnectionConfig_generated_by_enum_delegate { type Error = () ; fn try_into (self) -> Result < & 'a mut < TcpTransport as Transport > :: OutConnectionConfig , Self :: Error > { match self { Transport_for_InternalTransportType_associated_type_OutConnectionConfig_generated_by_enum_delegate :: Tcp (inner) => Ok (inner) , _ => Err (()) , } } }
    impl From < < QuicTransport as Transport > :: OutConnectionConfig > for Transport_for_InternalTransportType_associated_type_OutConnectionConfig_generated_by_enum_delegate { fn from (inner : < QuicTransport as Transport > :: OutConnectionConfig) -> Self { Transport_for_InternalTransportType_associated_type_OutConnectionConfig_generated_by_enum_delegate :: Quic (inner) } }
    impl TryInto < < QuicTransport as Transport > :: OutConnectionConfig > for Transport_for_InternalTransportType_associated_type_OutConnectionConfig_generated_by_enum_delegate { type Error = () ; fn try_into (self) -> Result < < QuicTransport as Transport > :: OutConnectionConfig , Self :: Error > { match self { Transport_for_InternalTransportType_associated_type_OutConnectionConfig_generated_by_enum_delegate :: Quic (inner) => Ok (inner) , _ => Err (()) , } } }
    impl < 'a > TryInto < & 'a < QuicTransport as Transport > :: OutConnectionConfig > for & 'a Transport_for_InternalTransportType_associated_type_OutConnectionConfig_generated_by_enum_delegate { type Error = () ; fn try_into (self) -> Result < & 'a < QuicTransport as Transport > :: OutConnectionConfig , Self :: Error > { match self { Transport_for_InternalTransportType_associated_type_OutConnectionConfig_generated_by_enum_delegate :: Quic (inner) => Ok (inner) , _ => Err (()) , } } }
    impl < 'a > TryInto < & 'a mut < QuicTransport as Transport > :: OutConnectionConfig > for & 'a mut Transport_for_InternalTransportType_associated_type_OutConnectionConfig_generated_by_enum_delegate { type Error = () ; fn try_into (self) -> Result < & 'a mut < QuicTransport as Transport > :: OutConnectionConfig , Self :: Error > { match self { Transport_for_InternalTransportType_associated_type_OutConnectionConfig_generated_by_enum_delegate :: Quic (inner) => Ok (inner) , _ => Err (()) , } } }
    impl From<TcpTransport> for InternalTransportType {
        fn from(inner: TcpTransport) -> Self {
            InternalTransportType::Tcp(inner)
        }
    }
    impl TryInto<TcpTransport> for InternalTransportType {
        type Error = ();
        fn try_into(self) -> Result<TcpTransport, Self::Error> {
            match self {
                InternalTransportType::Tcp(inner) => Ok(inner),
                _ => Err(()),
            }
        }
    }
    impl<'a> TryInto<&'a TcpTransport> for &'a InternalTransportType {
        type Error = ();
        fn try_into(self) -> Result<&'a TcpTransport, Self::Error> {
            match self {
                InternalTransportType::Tcp(inner) => Ok(inner),
                _ => Err(()),
            }
        }
    }
    impl<'a> TryInto<&'a mut TcpTransport> for &'a mut InternalTransportType {
        type Error = ();
        fn try_into(self) -> Result<&'a mut TcpTransport, Self::Error> {
            match self {
                InternalTransportType::Tcp(inner) => Ok(inner),
                _ => Err(()),
            }
        }
    }
    impl From<QuicTransport> for InternalTransportType {
        fn from(inner: QuicTransport) -> Self {
            InternalTransportType::Quic(inner)
        }
    }
    impl TryInto<QuicTransport> for InternalTransportType {
        type Error = ();
        fn try_into(self) -> Result<QuicTransport, Self::Error> {
            match self {
                InternalTransportType::Quic(inner) => Ok(inner),
                _ => Err(()),
            }
        }
    }
    impl<'a> TryInto<&'a QuicTransport> for &'a InternalTransportType {
        type Error = ();
        fn try_into(self) -> Result<&'a QuicTransport, Self::Error> {
            match self {
                InternalTransportType::Quic(inner) => Ok(inner),
                _ => Err(()),
            }
        }
    }
    impl<'a> TryInto<&'a mut QuicTransport> for &'a mut InternalTransportType {
        type Error = ();
        fn try_into(self) -> Result<&'a mut QuicTransport, Self::Error> {
            match self {
                InternalTransportType::Quic(inner) => Ok(inner),
                _ => Err(()),
            }
        }
    }
    impl Transport for InternalTransportType {
        type OutConnectionConfig = Transport_for_InternalTransportType_associated_type_OutConnectionConfig_generated_by_enum_delegate ;
        fn start_listener(&mut self, address: SocketAddr) -> Result<(), PeerNetError> {
            match self {
                InternalTransportType::Tcp(target) => Transport::start_listener(
                    target,
                    address
                        .try_into()
                        .expect("Received argument of incorrect type"),
                )
                .into(),
                InternalTransportType::Quic(target) => Transport::start_listener(
                    target,
                    address
                        .try_into()
                        .expect("Received argument of incorrect type"),
                )
                .into(),
            }
        }
        fn try_connect(
            &mut self,
            address: SocketAddr,
            timeout: Duration,
            config: Self::OutConnectionConfig,
        ) -> Result<(), PeerNetError> {
            match self {
                InternalTransportType::Tcp(target) => Transport::try_connect(
                    target,
                    address
                        .try_into()
                        .expect("Received argument of incorrect type"),
                    timeout
                        .try_into()
                        .expect("Received argument of incorrect type"),
                    config
                        .try_into()
                        .expect("Received argument of incorrect type"),
                )
                .into(),
                InternalTransportType::Quic(target) => Transport::try_connect(
                    target,
                    address
                        .try_into()
                        .expect("Received argument of incorrect type"),
                    timeout
                        .try_into()
                        .expect("Received argument of incorrect type"),
                    config
                        .try_into()
                        .expect("Received argument of incorrect type"),
                )
                .into(),
            }
        }
        fn stop_listener(&mut self, address: SocketAddr) -> Result<(), PeerNetError> {
            match self {
                InternalTransportType::Tcp(target) => Transport::stop_listener(
                    target,
                    address
                        .try_into()
                        .expect("Received argument of incorrect type"),
                )
                .into(),
                InternalTransportType::Quic(target) => Transport::stop_listener(
                    target,
                    address
                        .try_into()
                        .expect("Received argument of incorrect type"),
                )
                .into(),
            }
        }
    }
    impl InternalTransportType {
        pub(crate) fn from_transport_type(
            transport_type: TransportType,
            peer_db: SharedPeerDB,
        ) -> Self {
            match transport_type {
                TransportType::Tcp => InternalTransportType::Tcp(TcpTransport::new(peer_db)),
                TransportType::Quic => InternalTransportType::Quic(QuicTransport::new()),
            }
        }
    }
    #[doc(hidden)]
    pub use enum_delegate_Transport_jN08W5PeanD4nsbRbPDJhl8L518ETpL7c1R6ngm3cw as Transport;
    /// This trait is used to abstract the transport layer
    /// so that the network manager can be used with different
    /// transport layers
    trait Transport {
        type OutConnectionConfig;
        /// Start a listener in a separate thread.
        /// A listener must accept connections when arriving create a new peer
        /// TODO: Determine when we check we don't have too many peers and how.
        fn start_listener(&mut self, address: SocketAddr) -> Result<(), PeerNetError>;
        fn try_connect(
            &mut self,
            address: SocketAddr,
            timeout: Duration,
            config: Self::OutConnectionConfig,
        ) -> Result<(), PeerNetError>;
        fn stop_listener(&mut self, address: SocketAddr) -> Result<(), PeerNetError>;
    }
}
