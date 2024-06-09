pub(crate) mod config;

use std::{
    net::{SocketAddr, SocketAddrV4},
    str::FromStr,
};

use crate::protocol::StreamProtocol;
use tokio::net::{TcpStream, UdpSocket};

#[derive(Clone)]
pub(crate) struct TcpService {
    pub(crate) config: config::ServiceConfigFields,
}

impl TcpService {
    pub(crate) fn new(config: config::ServiceConfigFields) -> Self {
        Self { config }
    }

    pub(crate) async fn get_connection(&self) -> Result<TcpStream, tokio::io::Error> {
        // TODO: load balancing
        let ip = self.config.backends[0].ip.clone();
        let port = self.config.backends[0].port.clone();

        TcpStream::connect((ip, port)).await
    }
}

#[derive(Clone)]
pub(crate) struct UdpService {
    pub(crate) config: config::ServiceConfigFields,
}

impl UdpService {
    pub(crate) fn new(config: config::ServiceConfigFields) -> Self {
        Self { config }
    }

    pub(crate) fn get_address(&self) -> SocketAddr {
        // TODO: load balancing
        let ip = self.config.backends[0].ip.clone();
        let port = self.config.backends[0].port.clone();

        // TODO : check on instantiation
        SocketAddr::V4(SocketAddrV4::from_str(&format!("{}:{}", ip, port)).unwrap())
    }
}

#[derive(Clone)]
pub(crate) enum Service {
    Tcp(TcpService),
    Udp(UdpService),
}

impl Service {
    pub(crate) fn get_protocol(&self) -> StreamProtocol {
        match self {
            Service::Tcp(_) => StreamProtocol::Tcp,
            Service::Udp(_) => StreamProtocol::Udp,
        }
    }
}

impl Service {
    pub(crate) fn new(config: config::StreamServiceConfig) -> Self {
        match config {
            config::StreamServiceConfig::Tcp(config) => Service::Tcp(TcpService::new(config)),
            config::StreamServiceConfig::Udp(config) => Service::Udp(UdpService::new(config)),
        }
    }
}
