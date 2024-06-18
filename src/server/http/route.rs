use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use hyper::{body::Incoming, Request, Response};
use std::{convert::Infallible, sync::Arc};
use tokio::sync::Mutex;

use crate::server::host::HostSpec;

use super::{matchers::Matcher, service::HttpService};

#[derive(Debug)]
pub(crate) struct HttpRule {
    pub(crate) matchers: Vec<Matcher>,
    backend: Arc<Mutex<HttpService>>,
}

impl HttpRule {
    fn matches(&self, req: &Request<Incoming>) -> bool {
        if self.matchers.is_empty() {
            return true;
        }

        self.matchers.iter().all(|matcher| matcher.matches(req))
    }

    pub(super) async fn send_request(
        &self,
        req: Request<Incoming>,
    ) -> Result<Response<BoxBody<Bytes, hyper::Error>>, Infallible> {
        self.backend.lock().await.send_request(req).await
    }
}

// This route is def on steroids
// Thanks networking-sig
impl HttpRule {
    pub(crate) fn new(matchers: Vec<Matcher>, backend: Arc<Mutex<HttpService>>) -> Self {
        Self { matchers, backend }
    }
}

#[derive(Debug)]
pub(crate) struct HttpRoute {
    pub(crate) hostnames: Vec<HostSpec>,
    pub(crate) rules: Vec<HttpRule>,
}

impl HttpRoute {
    pub(crate) fn find_matching_rule(&self, req: &Request<Incoming>) -> Option<&HttpRule> {
        self.rules.iter().find(|rule| rule.matches(req))
    }
}
