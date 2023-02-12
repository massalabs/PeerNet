use std::net::{TcpStream, UdpSocket};

pub(crate) enum TransportType {
    Tcp,
    Udp,
}

pub(crate) enum Transport {
    Tcp(TcpStream),
    Udp(UdpSocket),
}
