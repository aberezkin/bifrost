mod tcp;
mod udp;

use duration_string::DurationString;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use tcp::TcpServer;
use udp::UdpServer;

use crate::protocol::StreamProtocol;
use crate::service::config::StreamServiceConfig;
use crate::service::{TcpService, UdpService};

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct TcpFields {
    pub(crate) port: u16,
    pub(crate) name: String,
    pub(crate) service: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct UdpFields {
    pub(crate) port: u16,
    pub(crate) name: String,
    pub(crate) service: String,

    /// Time during which the server is going to be holding a biderectional connection.
    ///
    /// When the server gets a message it's going to pass it to the specified backend
    /// and wait for response on a dedicated port. This virtual connection is closed when there's
    /// no message from peer or upstream for the specified duration.
    ///
    /// (NOTE: what to do when ports run out is there a
    /// way to use the same port and underrstand which messages are for which peers?)
    pub(crate) biderectional_connection_ttl: Option<DurationString>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "kebab-case", tag = "protocol")]
pub(crate) enum StreamServerConfig {
    Tcp(TcpFields),
    Udp(UdpFields),
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
    pub(crate) fn tcp(config: TcpFields, service: TcpService) -> Self {
        Self::Tcp(TcpServer { config, service })
    }

    pub(crate) fn udp(config: UdpFields, service: UdpService) -> Self {
        Self::Udp(UdpServer::new(config, service))
    }

    pub(crate) async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            StreamServer::Tcp(server) => server.run().await,
            StreamServer::Udp(server) => server.run().await,
        }
    }
}
