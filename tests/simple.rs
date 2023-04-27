mod util;
use std::{thread::sleep, time::Duration};

use peernet::types::KeyPair;
use peernet::{
    config::{PeerNetConfiguration, PeerNetFeatures},
    network_manager::PeerNetManager,
    peer::InitConnectionHandler,
    peer_id::PeerId,
    transports::{OutConnectionConfig, QuicOutConnectionConfig, TransportType},
};
use util::{create_clients, DefaultMessagesHandler};

#[derive(Clone)]
pub struct DefaultInitConnection;
impl InitConnectionHandler for DefaultInitConnection {
    fn perform_handshake<M: peernet::messages::MessagesHandler>(
        &mut self,
        _keypair: &KeyPair,
        _endpoint: &mut peernet::transports::endpoint::Endpoint,
        _listeners: &std::collections::HashMap<std::net::SocketAddr, TransportType>,
        _messages_handler: M,
    ) -> peernet::error::PeerNetResult<PeerId> {
        let keypair = KeyPair::generate();
        Ok(PeerId::from_public_key(keypair.get_public_key()))
    }
}

#[test]
fn simple() {
    let keypair = KeyPair::generate();
    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        self_keypair: keypair,
        init_connection_handler: DefaultInitConnection,
        optional_features: PeerNetFeatures::default().set_reject_same_ip_addr(false),
        message_handler: DefaultMessagesHandler {},
    };
    let mut manager = PeerNetManager::new(config);
    manager
        .start_listener(TransportType::Tcp, "127.0.0.1:64850".parse().unwrap())
        .unwrap();
    //manager.start_listener(TransportType::Quic, "127.0.0.1:64850".parse().unwrap()).unwrap();
    sleep(Duration::from_secs(3));
    let _ = create_clients(11);
    sleep(Duration::from_secs(6));

    // we have max_in_connections = 10
    assert!(manager.nb_in_connections().eq(&10));

    manager
        .stop_listener(TransportType::Tcp, "127.0.0.1:64850".parse().unwrap())
        .unwrap();
}

#[test]
fn two_peers_tcp() {
    let keypair1 = KeyPair::generate();
    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        self_keypair: keypair1,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default().set_reject_same_ip_addr(false),
        message_handler: DefaultMessagesHandler {},
    };
    let mut manager = PeerNetManager::new(config);
    manager
        .start_listener(TransportType::Tcp, "127.0.0.1:8081".parse().unwrap())
        .unwrap();

    let keypair2 = KeyPair::generate();
    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        self_keypair: keypair2,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default().set_reject_same_ip_addr(false),
        message_handler: DefaultMessagesHandler {},
    };
    let mut manager2 = PeerNetManager::new(config);
    sleep(Duration::from_secs(3));
    manager2
        .try_connect(
            "127.0.0.1:8081".parse().unwrap(),
            Duration::from_secs(3),
            &OutConnectionConfig::Tcp(Box::default()),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));
    manager
        .stop_listener(TransportType::Tcp, "127.0.0.1:8081".parse().unwrap())
        .unwrap();
    assert!(manager.nb_in_connections().eq(&1));
}

#[test]
fn two_peers_quic() {
    let keypair1 = KeyPair::generate();
    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        self_keypair: keypair1.clone(),
        init_connection_handler: DefaultInitConnection {},
        message_handler: DefaultMessagesHandler {},
        optional_features: PeerNetFeatures::default().set_reject_same_ip_addr(false),
    };
    let mut manager = PeerNetManager::new(config);
    manager
        .start_listener(TransportType::Quic, "127.0.0.1:8082".parse().unwrap())
        .unwrap();

    let keypair2 = KeyPair::generate();
    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        self_keypair: keypair2,
        init_connection_handler: DefaultInitConnection {},
        optional_features: PeerNetFeatures::default().set_reject_same_ip_addr(false),
        message_handler: DefaultMessagesHandler {},
    };
    let mut manager2 = PeerNetManager::new(config);
    sleep(Duration::from_secs(3));
    manager2
        .try_connect(
            "127.0.0.1:8082".parse().unwrap(),
            Duration::from_secs(5),
            //TODO: Use the one in manager instead of asking. Need a wrapper structure ?
            &mut OutConnectionConfig::Quic(Box::new(QuicOutConnectionConfig {
                identity: PeerId::from_public_key(keypair1.get_public_key()),
                local_addr: "127.0.0.1:8083".parse().unwrap(),
            })),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(5));
    manager
        .stop_listener(TransportType::Quic, "127.0.0.1:8082".parse().unwrap())
        .unwrap();
}
