use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct HttpFields {
    pub(crate) port: u16,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "version")]
pub(crate) enum HttpConfig {
    #[serde(rename = "1.1")]
    V1_1(HttpFields),
    #[serde(rename = "2.0")]
    V2_0(HttpFields),
}
