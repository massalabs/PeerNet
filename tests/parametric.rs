use std::collections::HashMap;
use std::println;
use std::time::{Duration, Instant};

use peernet::config::{PeerNetCategoryInfo, PeerNetConfiguration, PeerNetFeatures};
use peernet::network_manager::PeerNetManager;
use peernet::peer_id::PeerId;
use peernet::{peer::InitConnectionHandler, transports::TransportType};

mod util;

use util::paramtests::start_parametric_test;
use util::{DefaultContext, DefaultMessagesHandler, DefaultPeerId};

use rand::Rng;

use crate::util::get_tcp_port;

pub struct TestParameters<R: Rng> {
    misc_data_len: usize,
    rbs: u64,
    rl: u64,
    rtw: Duration,
    rng: R,
}

impl<R: Rng> TestParameters<R> {
    pub fn generate(mut rng: R) -> TestParameters<R> {
        TestParameters {
            misc_data_len: rng.gen_range(10..10240),
            rbs: rng.gen_range((60 * 1024)..(1024 * 512)),
            rl: rng.gen_range(1024..(1024 * 512)),
            rtw: Duration::from_millis(rng.gen_range(1..100)),
            rng,
        }
    }

    pub fn build_config(&mut self, init: Option<DefaultInitConnection>) -> PeerNetCfg {
        let context = DefaultContext {
            our_id: DefaultPeerId::generate(),
        };

        PeerNetConfiguration {
            read_timeout: Duration::from_secs(10),
            write_timeout: Duration::from_secs(10),
            optional_features: PeerNetFeatures::default(),
            message_handler: DefaultMessagesHandler {},
            peers_categories: HashMap::default(),

            // Got from existing config if any
            rate_bucket_size: self.rbs,
            rate_limit: self.rl,
            rate_time_window: self.rtw,
            init_connection_handler: if let Some(mut i) = init {
                i.id = context.our_id.clone();
                i
            } else {
                self.into()
            },

            // Constants
            max_in_connections: 10,
            send_data_channel_size: 1000,
            max_message_size: 1048576000,
            default_category_info: PeerNetCategoryInfo {
                max_in_connections: 10,
                max_in_connections_per_ip: 10,
                max_out_connections: 10,
            },
            _phantom: std::marker::PhantomData,
            context,
        }
    }
}

pub type PeerNetCfg = PeerNetConfiguration<
    DefaultPeerId,
    DefaultContext,
    DefaultInitConnection,
    DefaultMessagesHandler,
>;

impl<R: Rng> Into<PeerNetCfg> for &mut TestParameters<R> {
    fn into(self) -> PeerNetCfg {
        self.build_config(None)
    }
}

#[derive(Clone)]
pub struct DefaultInitConnection {
    datalen: usize,
    misc_data: Vec<u8>,
    pub id: DefaultPeerId,
}

impl<R: Rng> From<&mut TestParameters<R>> for DefaultInitConnection {
    fn from(param: &mut TestParameters<R>) -> Self {
        let misc_data: Vec<u8> = (0..param.misc_data_len)
            .map(|_| param.rng.gen::<u8>())
            .collect();
        DefaultInitConnection {
            datalen: param.misc_data_len,
            misc_data,
            id: DefaultPeerId::generate(),
        }
    }
}

impl InitConnectionHandler<DefaultPeerId, DefaultContext, DefaultMessagesHandler>
    for DefaultInitConnection
{
    fn perform_handshake(
        &mut self,
        _keypair: &DefaultContext,
        endpoint: &mut peernet::transports::endpoint::Endpoint,
        _listeners: &std::collections::HashMap<std::net::SocketAddr, TransportType>,
        _messages_handler: DefaultMessagesHandler,
    ) -> peernet::error::PeerNetResult<DefaultPeerId> {
        let now = std::time::Instant::now();

        endpoint.send::<DefaultPeerId>(&self.misc_data)?;
        let received = endpoint.receive::<DefaultPeerId>()?;
        assert_eq!(received.len(), self.datalen);
        assert_eq!(received, self.misc_data);

        endpoint.send::<DefaultPeerId>(&self.id.id.to_be_bytes())?;
        let remote_id = endpoint.receive::<DefaultPeerId>()?;
        let remote_id = u64::from_be_bytes(remote_id.try_into().unwrap());

        println!("Handshake OK in {:?}", now.elapsed());
        Ok(DefaultPeerId { id: remote_id })
    }
}

#[test]
fn handshake_with_limiter() {
    fn test_handshake<T: Rng>(rng: T) {
        let mut test_parameters = TestParameters::generate(rng);
        let config: PeerNetCfg = test_parameters.build_config(None);
        let config2: PeerNetCfg =
            test_parameters.build_config(Some(config.init_connection_handler.clone()));

        let mut manager: PeerNetManager<
            DefaultPeerId,
            DefaultContext,
            DefaultInitConnection,
            DefaultMessagesHandler,
        > = PeerNetManager::new(config);

        let port = get_tcp_port(1024..u16::MAX);

        manager
            .start_listener(
                TransportType::Tcp,
                format!("127.0.0.1:{port}").parse().unwrap(),
            )
            .expect("Unable to start listener on manager");

        std::thread::sleep(Duration::from_millis(50));
        let mut manager2: PeerNetManager<
            DefaultPeerId,
            DefaultContext,
            DefaultInitConnection,
            DefaultMessagesHandler,
        > = PeerNetManager::new(config2);

        let now = Instant::now();
        manager2
            .try_connect(
                TransportType::Tcp,
                format!("127.0.0.1:{port}").parse().unwrap(),
                Duration::from_secs(3),
            )
            .unwrap();
        while manager.nb_in_connections() < 1 {
            std::thread::sleep(Duration::from_millis(10));

            // Timeout if takes too long
            if now.elapsed() > Duration::from_secs(3) {
                assert!(false, "Took {:?}", now.elapsed(),);
            }
        }

        manager
            .stop_listener(
                TransportType::Tcp,
                format!("127.0.0.1:{port}").parse().unwrap(),
            )
            .unwrap();
        println!("Done");
        println!("");
    }

    start_parametric_test(
        50,
        vec![
            10824795490488834629,
            15469480549121256480,
            2624411005066766620,
            9562377269806463922,
            5557481319223321195,
            17383559344573641903,
        ],
        test_handshake,
    );
}
