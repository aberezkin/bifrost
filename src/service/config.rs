use std::net::IpAddr;

use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub(crate) enum LoadBalancingAlgorithm {
    #[default]
    RoundRobin,
    Random,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct BackendDefinition {
    pub(crate) port: u16,
    // TODO: support for hostnames
    pub(crate) ip: IpAddr,
}

impl BackendDefinition {
    pub(crate) async fn get_connection(&self) -> std::io::Result<TcpStream> {
        TcpStream::connect((self.ip, self.port)).await
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct ServiceConfigFields {
    pub(crate) backends: Vec<BackendDefinition>,
    #[serde(default)]
    pub(crate) load_balancing_algorithm: LoadBalancingAlgorithm,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "kebab-case", tag = "protocol")]
pub(crate) enum StreamServiceConfig {
    Tcp(ServiceConfigFields),
    Udp(ServiceConfigFields),
}
