use tokio::net::{TcpStream, UdpSocket};

pub(crate) mod config;

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

pub(crate) struct UdpService {
    pub(crate) config: config::ServiceConfigFields,
}

impl UdpService {
    pub(crate) fn new(config: config::ServiceConfigFields) -> Self {
        Self { config }
    }

    pub(crate) async fn get_connection(&self) -> Result<UdpSocket, tokio::io::Error> {
        // TODO: load balancing
        let ip = self.config.backends[0].ip.clone();
        let port = self.config.backends[0].port.clone();

        UdpSocket::bind((ip, port)).await
    }
}

pub(crate) enum Service {
    Tcp(TcpService),
    Udp(UdpService),
}

impl Service {
    pub(crate) fn new(config: config::StreamServiceConfig) -> Self {
        match config {
            config::StreamServiceConfig::Tcp(config) => Service::Tcp(TcpService::new(config)),
            config::StreamServiceConfig::Udp(config) => Service::Udp(UdpService::new(config)),
        }
    }
}
