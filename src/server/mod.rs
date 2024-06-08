pub(crate) mod http;
pub(crate) mod stream;

use std::collections::HashMap;

use stream::StreamServerConfig;

use crate::service::config::StreamServiceConfig;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct StreamSectionConfig {
    pub(crate) servers: Vec<StreamServerConfig>,
    pub(crate) services: HashMap<String, StreamServiceConfig>,
}
#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct Config {
    pub(crate) stream: StreamSectionConfig,
}
