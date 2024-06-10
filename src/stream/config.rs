use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum StreamProtocol {
    Tcp,
    Udp,
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct StreamConfig {
    pub(crate) port: u16,
    pub(crate) protocol: StreamProtocol,
}

impl StreamConfig {
    pub(crate) fn new(port: u16, protocol: StreamProtocol) -> Self {
        Self { port, protocol }
    }
}
