pub(crate) mod cluster;
pub(crate) mod matchers;
pub(crate) mod route;
pub(crate) mod server;
pub(crate) mod service;

use service::HttpService;
use std::collections::HashMap;

use super::host::HostSpec;

use matchers::Matcher;
use serde::{Deserialize, Serialize};
use server::HttpServerFields;

pub(crate) use server::HttpServer;

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "version")]
pub(crate) enum HttpServerConfig {
    #[serde(rename = "1")]
    V1(HttpServerFields),
    #[serde(rename = "2")]
    V2(HttpServerFields),
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct HttpRouteRuleConfig {
    // NOTE: These ones are chained using OR
    pub(crate) matches: Vec<Matcher>,
    pub(crate) backend: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct HttpRouteConfig {
    pub(crate) name: String,
    pub(crate) hostnames: Option<Vec<HostSpec>>,
    pub(crate) server: String,
    pub(crate) rules: Vec<HttpRouteRuleConfig>,
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct HttpConfig {
    pub(crate) servers: Vec<HttpServerFields>,
    pub(crate) services: HashMap<String, HttpService>,
    pub(crate) routes: Vec<HttpRouteConfig>,
}
