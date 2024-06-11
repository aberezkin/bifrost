use std::{collections::HashMap, convert::Infallible};

use serde::{Deserialize, Serialize};

use crate::service::config::BackendDefinition;

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct HttpServerFields {
    pub(crate) port: u16,
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

#[derive(Deserialize, Serialize, Debug)]
pub(crate) enum StringMatchType {
    Exact,
    Prefix,
    Regex,
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct StringMatch {
    pub(crate) r#type: StringMatchType,
    pub(crate) value: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct HttpRouteMatch {
    // NOTE: All fields here should be matched using AND
    pub(crate) path: StringMatch,
    // TODO: method, headers, query
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct HttpRouteRule {
    // NOTE: These ones are chained using OR
    pub(crate) matches: Vec<HttpRouteMatch>,
    pub(crate) backend: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct HttpRoute {
    pub(crate) name: String,
    pub(crate) server: String,
    pub(crate) rules: Vec<HttpRouteRule>,
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct HttpConfig {
    pub(crate) servers: Vec<HttpServerConfig>,
    pub(crate) services: HashMap<String, HttpService>,
    pub(crate) routes: Vec<HttpRoute>,
}

pub(crate) enum HttpVersion {
    V1,
    V2,
}

pub(crate) struct HttpServer {
    version: HttpVersion,
    port: u16,
}

use bytes::Bytes;
use http_body_util::Full;
use hyper::{server::conn::http1, service::service_fn, Request, Response};
use hyper_util::rt::{TokioIo, TokioTimer};
use std::net::SocketAddr;
use tokio::net::TcpListener;

// An async function that consumes a request, does nothing with it and returns a
// response.
async fn hello(_: Request<impl hyper::body::Body>) -> Result<Response<Full<Bytes>>, Infallible> {
    Ok(Response::new(Full::new(Bytes::from("Hello World!"))))
}

impl HttpServer {
    pub(crate) fn new(config: HttpServerConfig) -> Self {
        match config {
            HttpServerConfig::V1(config) => HttpServer {
                version: HttpVersion::V1,
                port: config.port,
            },
            HttpServerConfig::V2(config) => HttpServer {
                version: HttpVersion::V2,
                port: config.port,
            },
        }
    }

    pub(crate) async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let addr: SocketAddr = ([0, 0, 0, 0], self.port).into();

        let listener = TcpListener::bind(addr).await?;

        println!("Listening for HTTP on port {}", self.port);

        loop {
            let (stream, _) = listener.accept().await.unwrap();

            let io = TokioIo::new(stream);

            tokio::spawn(async move {
                if let Err(err) = http1::Builder::new()
                    .timer(TokioTimer::new())
                    .serve_connection(io, service_fn(hello))
                    .await
                {
                    println!("Error serving connection: {:?}", err);
                }
            });
        }
    }

    pub(crate) fn close() {}
}
