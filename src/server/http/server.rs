use crate::server::host::Hostname;
use bytes::Bytes;
use http::StatusCode;
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use hyper::{body::Incoming, server::conn::http1, service::service_fn, Request, Response};
use hyper_util::rt::TokioIo;
use std::{convert::Infallible, net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;

use super::{HttpRoute, HttpServerFields};

pub(crate) struct HttpServer {
    port: u16,
    routes: Arc<Vec<HttpRoute>>,
}

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
        // NOTE: We can start sending the mirrored request in parallel tokio task. Maybe even force it to be in a
        // different thread so it doesn't affect the main volume of traffic in any way, but that
        // might be complicated and actually less performant.

        let host = Hostname::parse(req.headers().get("host").unwrap().to_str().unwrap()).unwrap();

        // TODO: There might be a better way to do this.
        // a hashmap cache can be an option
        let route = routes.iter().find(|route| {
            route
                .hostnames
                .iter()
                .any(|hostname| hostname.matches(&host))
        });

        println!("{:?}", route);

        if let Some(route) = route {
            let matching_rule = route.find_matching_rule(&req);

            if let Some(rule) = matching_rule {
                rule.backend.send_request(req).await
            } else {
                Ok(not_found())
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

fn not_found() -> Response<BoxBody<Bytes, hyper::Error>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(full("Not found"))
        // FIX: expect
        .expect("Failed to build response")
}
