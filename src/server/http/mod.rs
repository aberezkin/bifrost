pub(crate) mod matchers;
pub(crate) mod server;
pub(crate) mod service;

use hyper::{body::Incoming, Request};
use service::HttpService;
use std::collections::HashMap;

use super::host::HostSpec;

use matchers::Matcher;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub(crate) use server::HttpServer;

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct HttpServerFields {
    pub(crate) port: u16,
    pub(crate) name: String,
}

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

#[derive(Debug)]
pub(crate) struct HttpRule {
    // TODO: stricter type
    pub(crate) matchers: Vec<Matcher>,
    backend: Arc<HttpService>,
}

impl HttpRule {
    fn matches(&self, req: &Request<Incoming>) -> bool {
        if self.matchers.is_empty() {
            return true;
        }

        self.matchers.iter().all(|matcher| matcher.matches(req))
    }
}

// This route is def on steroids
// Thanks networking-sig
impl HttpRule {
    pub(crate) fn new(matchers: Vec<Matcher>, backend: Arc<HttpService>) -> Self {
        Self { matchers, backend }
    }
}

#[derive(Debug)]
pub(crate) struct HttpRoute {
    pub(crate) hostnames: Vec<HostSpec>,
    pub(crate) rules: Vec<HttpRule>,
}

impl HttpRoute {
    fn find_matching_rule(&self, req: &Request<Incoming>) -> Option<&HttpRule> {
        self.rules.iter().find(|rule| rule.matches(req))
    }
}
