mod tcp;
mod udp;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use tcp::TcpServer;
use udp::UdpServer;

use crate::protocol::StreamProtocol;
use crate::service::config::StreamServiceConfig;
use crate::service::{TcpService, UdpService};

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct StreamFields {
    pub(crate) port: u16,
    pub(crate) name: String,
    pub(crate) service: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "kebab-case", tag = "protocol")]
pub(crate) enum StreamServerConfig {
    Tcp(StreamFields),
    Udp(StreamFields),
}
#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct StreamingConfig {
    pub(crate) servers: Vec<StreamServerConfig>,
    pub(crate) services: HashMap<String, StreamServiceConfig>,
}

impl StreamServerConfig {
    pub(crate) fn get_protocol(&self) -> StreamProtocol {
        match self {
            StreamServerConfig::Tcp(_) => StreamProtocol::Tcp,
            StreamServerConfig::Udp(_) => StreamProtocol::Udp,
        }
    }
}

pub(crate) enum StreamServer {
    Tcp(TcpServer),
    Udp(UdpServer),
}

impl StreamServer {
    pub(crate) fn tcp(config: StreamFields, service: TcpService) -> Self {
        Self::Tcp(TcpServer { config, service })
    }

    pub(crate) fn udp(config: StreamFields, service: UdpService) -> Self {
        Self::Udp(UdpServer { config, service })
    }

    pub(crate) async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            StreamServer::Tcp(server) => server.run().await,
            StreamServer::Udp(server) => server.run().await,
        }
    }
}
