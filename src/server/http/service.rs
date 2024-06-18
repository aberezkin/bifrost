use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;

use crate::service::config::BackendDefinition;
use hyper::{body::Incoming, Request, Response};
use hyper_util::rt::TokioIo;
use std::convert::Infallible;

#[derive(Deserialize, Serialize, Debug, Default)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum LoadBalancingAlgorithm {
    #[default]
    RoundRobin,
    Random,
}

#[derive(Deserialize, Serialize, Debug)]
struct LoadBalancer {
    #[serde(default)]
    current_connection_index: usize,
    #[serde(default, rename = "load_balancing_algorithm")]
    algo: LoadBalancingAlgorithm,
    backends: Vec<BackendDefinition>,
}

#[derive(Debug)]
pub(crate) enum ConnectionError {
    BackendNotFound,
    IoError(std::io::Error),
}

impl LoadBalancer {
    async fn get_connection(&mut self) -> Result<TcpStream, ConnectionError> {
        // TODO: load balancing
        // e.g. give connections to different backends according
        // to specified load balancing algo
        let backend = self
            .backends
            .get(self.current_connection_index)
            .ok_or(ConnectionError::BackendNotFound)?;

        println!("{}", backend.port);

        let connection = backend
            .get_connection()
            .await
            .map_err(ConnectionError::IoError);

        self.current_connection_index = (self.current_connection_index + 1) % self.backends.len();

        connection
    }
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct HttpService {
    #[serde(flatten)]
    load_balancer: LoadBalancer,
}

impl HttpService {
    pub(super) async fn send_request(
        &mut self,
        req: Request<Incoming>,
    ) -> Result<Response<BoxBody<Bytes, hyper::Error>>, Infallible> {
        use hyper::client::conn::http1;

        // FIX: unwrap
        let stream = self.load_balancer.get_connection().await.unwrap();

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
