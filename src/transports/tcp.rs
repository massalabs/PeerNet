use std::net::SocketAddr;

use crate::error::PeerNetError;

use super::Transport;

pub struct TcpTransport {
}

impl TcpTransport {
    pub fn new() -> TcpTransport {
        TcpTransport {  }
    }
}

impl Transport for TcpTransport {
    fn start_listener(&mut self, address: SocketAddr) -> Result<(), PeerNetError> {
        Ok(())
    }

    fn try_connect(&mut self) -> Result<usize, PeerNetError> {
        Ok(0)
    }

    fn stop_listener(&mut self) -> Result<(), PeerNetError> {
        Ok(())
    }
}