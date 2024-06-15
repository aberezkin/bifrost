use std::{collections::HashMap, convert::Infallible, net::IpAddr};

use serde::{Deserialize, Serialize};

use crate::{server::host::Hostname, service::config::BackendDefinition};

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
pub(crate) struct HttpService {
    backends: Vec<BackendDefinition>,
}

impl HttpService {
    async fn get_connection(&self) -> std::io::Result<TcpStream> {
        // TODO: load balancing
        self.backends.first().unwrap().get_connection().await
    }

    async fn send_request(
        &self,
        req: Request<Incoming>,
    ) -> Result<Response<BoxBody<Bytes, hyper::Error>>, Infallible> {
        use hyper::client::conn::http1;

        let stream = self.get_connection().await.unwrap();

        let io = TokioIo::new(stream);

        let (mut sender, conn) = http1::Builder::new().handshake(io).await.unwrap();

        tokio::spawn(async move {
            if let Err(err) = conn.await {
                println!("Connection failed: {:?}", err);
            }
        });

        let res = sender.send_request(req).await.unwrap();

        Ok(res.map(|res| res.boxed()))
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) enum StringMatchType {
    Exact,
    Prefix,
    // TODO: regex support
    //Regex,
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct StringMatch {
    pub(crate) r#type: StringMatchType,
    // TODO: better type?
    pub(crate) value: String,
}

// TODO: tests and matchers module
impl StringMatch {
    pub(crate) fn matches(&self, value: &str) -> bool {
        match self.r#type {
            StringMatchType::Exact => value == self.value,
            // TODO: proper prefix matching Prefix:/abc should match /abc/def but not /abcdef
            StringMatchType::Prefix => value.starts_with(&self.value),
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct Matcher {
    // NOTE: All fields here should be matched using AND
    pub(crate) path: StringMatch,
    // TODO: method, headers, query
}

impl Matcher {
    pub(crate) fn matches(&self, req: &Request<Incoming>) -> bool {
        // TODO: method, headers, query
        self.path.matches(req.uri().path())
    }
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

pub(crate) enum HttpVersion {
    V1,
    V2,
}

use std::sync::Arc;

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

pub(crate) struct HttpServer {
    port: u16,
    routes: Arc<Vec<HttpRoute>>,
}

use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use hyper::{body::Incoming, server::conn::http1, service::service_fn, Request, Response};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};

use super::host::HostSpec;

impl HttpServer {
    pub(crate) fn new(config: HttpServerFields, routes: Vec<HttpRoute>) -> Self {
        Self {
            port: config.port,
            routes: Arc::new(routes),
        }
    }

    pub(crate) async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let addr: SocketAddr = ([0, 0, 0, 0], self.port).into();

        let listener = TcpListener::bind(addr).await?;

        println!("Listening for HTTP on port {}", self.port);
        loop {
            let (stream, _) = listener.accept().await.unwrap();

            let io = TokioIo::new(stream);

            let routes = self.routes.clone();

            let service = service_fn(move |req| {
                let routes = routes.clone();

                async move { Self::proxy_request(req, routes).await }
            });

            tokio::spawn(async move {
                if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                    println!("Error serving connection: {:?}", err);
                }
            });
        }
    }

    // TODO: http2 backend support
    async fn proxy_request(
        req: Request<Incoming>,
        routes: Arc<Vec<HttpRoute>>,
    ) -> Result<Response<BoxBody<Bytes, hyper::Error>>, Infallible> {
        println!("{:?}", req);
        // NOTE: Some considerations:
        //
        // NOTE: There're route matchers that can match on route, method, headers and query
        // which means that before we can route a request we need to check these and
        // find the service they match. Finding the service should be the FIRST step as if
        // there's no service found, any work done with the request is for nothing.
        //
        // NOTE: After we foudn the service, we might need to apply so called "filters" to the request.
        // https://gateway-api.sigs.k8s.io/reference/spec/#gateway.networking.k8s.io%2fv1.HTTPRouteFilter
        // HttpRouteFilter are modifying the request before it's sent to the service
        // These can modify request headers and response headers, mirror a request to another
        // service, rewrite a URL or redirect a request.
        // When implementing these, consider only implement Core ones first and get back to
        // Extended onles later.
        //
        // NOTE: If we have a redirect filter we just respond with a redirect applying response headers filter
        //
        // NOTE: Now we actually need to send the request to the service. We the service to get the
        // first byte as sook as possible. But we can't send it before we actually apply the
        // request headers and the url rewrite filters. Once we have done that we can finally send
        // the headers to the service and start sending the body.
        //
        // NOTE: We can start sending the mirrored request in parallel tokio task. Maybe even force it to be in a
        // different thread so it doesn't affect the main volume of traffic in any way, but that
        // might be complicated and actually less performant.
        //
        // NOTE: It would be nice to get the headers as early as possible so that we can start
        // applying the filters and stream the response to the client.

        let host = Hostname::parse(req.headers().get("host").unwrap().to_str().unwrap()).unwrap();

        // TODO: There might be a better way to do this.
        // maybe hashmap as always???
        let route = routes.iter().find(|route| {
            route.hostnames.iter().any(|hostname| {
                println!("I matched");

                hostname.matches(&host)
            })
        });

        println!("{:?}", route);

        if let Some(route) = route {
            let matching_rule = route.find_matching_rule(&req);

            if let Some(rule) = matching_rule {
                rule.backend.send_request(req).await
            } else {
                Ok(Response::new(full("Not found")))
            }
        } else {
            Ok(Response::new(full("Not found")))
        }
    }
}

fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, hyper::Error> {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}
