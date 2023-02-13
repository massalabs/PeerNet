use std::net::{TcpStream, UdpSocket};

#[derive(PartialEq, Eq, Hash, Copy, Clone)]
pub enum TransportType {
    Tcp,
    Udp,
}

pub(crate) enum Transport {
    Tcp(TcpStream),
    Udp(UdpSocket),
}
