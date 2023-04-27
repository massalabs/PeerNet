use std::time::Duration;

use crossbeam::channel::Sender;
use peernet::error::{PeerNetError, PeerNetResult};
use peernet::messages::{MessagesHandler, MessagesSerializer};
use peernet::types::KeyPair;
use peernet::{
    config::{PeerNetConfiguration, PeerNetFeatures},
    network_manager::PeerNetManager,
    peer::InitConnectionHandler,
    peer_id::PeerId,
    transports::{OutConnectionConfig, TcpOutConnectionConfig, TransportType},
};

#[derive(Clone)]
struct EmptyInitConnection;
impl InitConnectionHandler for EmptyInitConnection {}

#[derive(Clone, PartialEq, Eq, Debug)]
enum TestMessages {
    Ping,
}

#[derive(Clone)]
struct TestMessagesHandler {
    pub test_sender: Sender<(PeerId, TestMessages)>,
}

impl MessagesHandler for TestMessagesHandler {
    fn handle(&self, id: u64, _data: &[u8], peer_id: &PeerId) -> PeerNetResult<()> {
        match id {
            0 => {
                println!("Received ping from {}", peer_id);
                self.test_sender
                    .send((peer_id.clone(), TestMessages::Ping))
                    .map_err(|err| {
                        PeerNetError::HandlerError.error("test", Some(err.to_string()))
                    })?;
                Ok(())
            }
            _ => Err(PeerNetError::ReceiveError.error("test", None)),
        }
    }

    fn deserialize_id<'a>(
        &self,
        data: &'a [u8],
        _peer_id: &PeerId,
    ) -> PeerNetResult<(&'a [u8], u64)> {
        Ok((&data[1..], data[0] as u64))
    }
}

struct MessageSerializer;

impl MessagesSerializer<Vec<u8>> for MessageSerializer {
    fn serialize_id(&self, _message: &Vec<u8>, buffer: &mut Vec<u8>) -> PeerNetResult<()> {
        buffer.push(0);
        Ok(())
    }

    fn serialize(&self, message: &Vec<u8>, buffer: &mut Vec<u8>) -> PeerNetResult<()> {
        buffer.extend_from_slice(message);
        Ok(())
    }
}

#[test]
fn two_peers_tcp_with_one_message() {
    let keypair2 = KeyPair::generate();
    let keypair2_clone = keypair2.clone();
    let (sender, receiver) = crossbeam::channel::unbounded();
    std::thread::spawn(move || {
        let (peer_id, message) = receiver.recv().unwrap();
        assert_eq!(
            peer_id,
            PeerId::from_public_key(keypair2_clone.get_public_key())
        );
        assert_eq!(message, TestMessages::Ping);
        println!("Well received")
    });
    let keypair1 = KeyPair::generate();
    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        self_keypair: keypair1,
        init_connection_handler: EmptyInitConnection {},
        message_handler: TestMessagesHandler {
            test_sender: sender.clone(),
        },
        optional_features: PeerNetFeatures::default().set_reject_same_ip_addr(false),
    };
    let mut manager = PeerNetManager::new(config);
    manager
        .start_listener(TransportType::Tcp, "127.0.0.1:8081".parse().unwrap())
        .unwrap();

    let config = PeerNetConfiguration {
        max_in_connections: 10,
        max_out_connections: 20,
        self_keypair: keypair2,
        init_connection_handler: EmptyInitConnection {},
        message_handler: TestMessagesHandler {
            test_sender: sender,
        },
        optional_features: PeerNetFeatures::default().set_reject_same_ip_addr(false),
    };
    let mut manager2 = PeerNetManager::new(config);
    manager2
        .try_connect(
            "127.0.0.1:8081".parse().unwrap(),
            Duration::from_secs(3),
            &mut OutConnectionConfig::Tcp(Box::new(TcpOutConnectionConfig::default())),
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(1));
    let active_connections = manager2.active_connections.clone();
    {
        let mut connections = active_connections.write();
        for (peer_id, connection) in connections.connections.iter_mut() {
            println!("Sending message to {:?}", peer_id);
            connection
                .send_channels
                .send(&MessageSerializer {}, vec![1, 2, 3], true)
                .unwrap();
        }
        println!("Connections: {:?}", connections);
    }
    std::thread::sleep(std::time::Duration::from_secs(5));
    manager
        .stop_listener(TransportType::Tcp, "127.0.0.1:8081".parse().unwrap())
        .unwrap();
}
