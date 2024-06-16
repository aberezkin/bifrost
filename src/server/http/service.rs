use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;

use crate::service::config::BackendDefinition;
use hyper::{body::Incoming, Request, Response};
use hyper_util::rt::TokioIo;
use std::convert::Infallible;

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct HttpService {
    backends: Vec<BackendDefinition>,
}

impl HttpService {
    pub(super) async fn get_connection(&self) -> std::io::Result<TcpStream> {
        // TODO: load balancing
        // e.g. give connections to different backends according
        // to specified load balancing algo
        self.backends.first().unwrap().get_connection().await
    }

    pub(super) async fn send_request(
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
