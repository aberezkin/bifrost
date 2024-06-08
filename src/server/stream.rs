use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct StreamFields {
    pub(crate) port: u16,
    pub(crate) name: String,
    pub(crate) service: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "kebab-case", tag = "protocol")]
pub(crate) enum StreamServerConfig {
    Tcp(StreamFields),
    Udp(StreamFields),
}

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, UdpSocket};

use crate::service::{Service, TcpService};

pub(crate) struct StreamServer {
    pub(crate) config: StreamServerConfig,
    // TODO: refactor for more different protocols
    pub(crate) service: TcpService,
}

impl StreamServer {
    pub(crate) fn new(config: StreamServerConfig, service: TcpService) -> Self {
        Self { config, service }
    }

    pub(crate) async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        match self.config {
            StreamServerConfig::Tcp(fields) => {
                let listener = TcpListener::bind(("0.0.0.0", fields.port)).await?;

                println!("Listening for TCP on port {}", fields.port);

                loop {
                    let (stream, _) = listener.accept().await?;
                    let mut upstream = self.service.get_connection().await?;

                    let peer_addr = stream.peer_addr()?;

                    println!("Accepted connection from {}", peer_addr);

                    tokio::spawn(async move {
                        let mut stream = stream;
                        let mut buffer = [0; 1024];

                        loop {
                            let bytes_from_client = stream.read(&mut buffer).await.unwrap();

                            println!(
                                "Received {} bytes from client, sending to upstream",
                                bytes_from_client
                            );

                            upstream.write(&buffer[..bytes_from_client]).await.unwrap();

                            let bytes_from_upstream = upstream.read(&mut buffer).await.unwrap();

                            println!(
                                "Received {} bytes from upstream, sending to client",
                                bytes_from_upstream
                            );

                            stream.write(&buffer[..bytes_from_client]).await.unwrap();
                        }
                    });
                }
            }
            StreamServerConfig::Udp(fields) => {
                let socket = UdpSocket::bind(("0.0.0.0", fields.port)).await?;

                println!("Listening for UDP on port {}", fields.port);

                loop {
                    let mut buffer = [0; 1024];

                    let (bytes_read, peer_addr) = socket.recv_from(&mut buffer).await?;

                    println!("Received {} bytes from {}", bytes_read, peer_addr);

                    socket.send_to(&buffer[..bytes_read], peer_addr).await?;
                }
            }
        }
    }
}
