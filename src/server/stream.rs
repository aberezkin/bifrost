use crate::protocol::StreamProtocol;
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

impl StreamServerConfig {
    pub(crate) fn get_protocol(&self) -> StreamProtocol {
        match self {
            StreamServerConfig::Tcp(_) => StreamProtocol::Tcp,
            StreamServerConfig::Udp(_) => StreamProtocol::Udp,
        }
    }
}

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, UdpSocket};

use crate::service::{TcpService, UdpService};

// This buffer size is closest to the size of a memory page in most systems.
// Ideally we can read the actual size using a package, but for now this is good enough.
// Also it's possible to make it configurable.
const DEFAULT_BUFFER_SIZE: usize = 4 * 1024; // 2KB

pub(crate) struct TcpServer {
    pub(crate) config: StreamFields,
    pub(crate) service: TcpService,
}

impl TcpServer {
    pub(crate) async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let fields = &self.config;

        let listener = TcpListener::bind(("0.0.0.0", fields.port)).await?;

        println!("Listening for TCP on port {}", fields.port);

        loop {
            let (stream, _) = listener.accept().await?;
            let mut upstream = self.service.get_connection().await?;

            let peer_addr = stream.peer_addr()?;

            println!("Accepted connection from {}", peer_addr);

            tokio::spawn(async move {
                let mut peer_stream = stream;
                let mut buffer = [0; DEFAULT_BUFFER_SIZE];

                // TODO: fix unwraps?
                loop {
                    let bytes_from_client = peer_stream.read(&mut buffer).await.unwrap();

                    if bytes_from_client == 0 {
                        println!(
                            "Peer {} disconnected closing connection to upstream",
                            peer_addr
                        );

                        upstream.shutdown().await.unwrap();
                        break;
                    }

                    println!(
                        "Received {} bytes from client, sending to upstream",
                        bytes_from_client
                    );

                    upstream.write(&buffer[..bytes_from_client]).await.unwrap();

                    let bytes_from_upstream = upstream.read(&mut buffer).await.unwrap();

                    if bytes_from_upstream == 0 {
                        peer_stream.shutdown().await.unwrap();
                        break;
                    }

                    println!(
                        "Received {} bytes from upstream, sending to client",
                        bytes_from_upstream
                    );

                    peer_stream
                        .write(&buffer[..bytes_from_client])
                        .await
                        .unwrap();
                }
            });
        }
    }
}

pub(crate) struct UdpServer {
    pub(crate) config: StreamFields,
    pub(crate) service: UdpService,
}

impl UdpServer {
    pub(crate) async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let fields = &self.config;

        let server_socket = UdpSocket::bind(("0.0.0.0", fields.port)).await?;
        let upstream_address = self.service.get_address();

        // TODO: Implement responding with a configurable timeoout
        // For now it's just sending the UDP packet to the upstream
        let local_receiver_socket = UdpSocket::bind("0.0.0.0:0").await?;

        println!("Listening for UDP on port {}", fields.port);

        let mut buffer = [0; DEFAULT_BUFFER_SIZE];

        loop {
            let (bytes_read, peer_addr) = server_socket.recv_from(&mut buffer).await?;

            println!("Received {} bytes from {}", bytes_read, peer_addr);

            local_receiver_socket
                .send_to(&buffer[..bytes_read], upstream_address)
                .await?;
        }
    }
}

pub(crate) enum StreamServer {
    Tcp(TcpServer),
    Udp(UdpServer),
}

impl StreamServer {
    pub(crate) fn tcp(config: StreamFields, service: TcpService) -> Self {
        Self::Tcp(TcpServer { config, service })
    }

    pub(crate) fn udp(config: StreamFields, service: UdpService) -> Self {
        Self::Udp(UdpServer { config, service })
    }

    pub(crate) async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            StreamServer::Tcp(server) => server.run().await,
            StreamServer::Udp(server) => server.run().await,
        }
    }
}
