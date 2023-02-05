pub struct PeerNetConfiguration {
    // The IP address
    pub ip: String,
    // The port
    pub port: String,
    // Number of peers we want to have
    pub max_peers: usize,
}