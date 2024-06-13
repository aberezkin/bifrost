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
use http_body_util::{combinators::BoxBody, BodyExt, Empty, Full};
use hyper::{body::Incoming, server::conn::http1, service::service_fn, Method, Request, Response};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use tokio::net::TcpListener;

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
                    .serve_connection(io, service_fn(Self::proxy_request))
                    .await
                {
                    println!("Error serving connection: {:?}", err);
                }
            });
        }
    }

    async fn proxy_request(
        req: Request<Incoming>,
    ) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
        // TODO: http2 backend support
        use hyper::client::conn::http1;
        use tokio::net::TcpStream;

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

        let backend_addr: SocketAddr = ([0, 0, 0, 0], 3000).into();
        let stream = TcpStream::connect(backend_addr).await.unwrap();
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
