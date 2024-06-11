pub(crate) mod http;
pub(crate) mod stream;

use http::HttpConfig;
use serde::{Deserialize, Serialize};
use stream::StreamingConfig;

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct Config {
    pub(crate) stream: Option<StreamingConfig>,
    pub(crate) http: Option<HttpConfig>,
}
