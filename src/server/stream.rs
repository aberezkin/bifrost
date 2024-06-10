use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

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
                let mut buffer_client = [0; DEFAULT_BUFFER_SIZE];
                let mut buffer_upstream = [0; DEFAULT_BUFFER_SIZE];

                // TODO: fix unwraps?
                loop {
                    let bytes_from_client = peer_stream.read(&mut buffer_client);
                    let bytes_from_upstream = upstream.read(&mut buffer_upstream);

                    // Bidirectional listen implemented as a race of messeages from two sources
                    // on every iteration. This works because read() is cancel safe and if one of
                    // the futures wins the race it's guaranteed that the other one has not read
                    // the stream so no bytes are lost.
                    tokio::select! {
                        // Listen for client messages and send them to upstream
                        bytes_from_client = bytes_from_client => {
                            let bytes_from_client = bytes_from_client.unwrap();
                            if bytes_from_client == 0 {
                                println!(
                                    "Peer {} disconnected closing connection to upstream",
                                    peer_addr
                                );

                                upstream.shutdown().await.unwrap();
                                break;
                            }

                            println!(
                                "Received {} bytes from client, sending to upstream {}",
                                bytes_from_client,
                                upstream.peer_addr().unwrap()
                            );

                            upstream.write(&buffer_client[..bytes_from_client]).await.unwrap();

                            println!("Sent");

                        },
                        // Listen for upstream messages and send them to client
                        bytes_from_upstream = bytes_from_upstream => {
                            let bytes_from_upstream = bytes_from_upstream.unwrap();

                            if bytes_from_upstream == 0 {
                                println!(
                                    "Upstream {} disconnected closing connection to peer",
                                    peer_addr
                                );
                                peer_stream.shutdown().await.unwrap();
                                break;
                            }

                            println!(
                                "Received {} bytes from upstream, sending to client",
                                bytes_from_upstream
                            );

                            peer_stream
                                .write(&buffer_upstream[..bytes_from_upstream])
                                .await
                                .unwrap();
                        }
                    }
                }
            });
        }
    }
}

pub(crate) struct UdpServer {
    pub(crate) config: StreamFields,
    pub(crate) service: UdpService,
}

const UDP_RESPONSE_TIMEOUT: tokio::time::Duration = Duration::from_secs(10);

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::sync::Mutex;

struct UdpConection {
    client: SocketAddr,
    reciever_socket: Arc<UdpSocket>,
    upstream_address: SocketAddr,
    server: Arc<UdpSocket>,

    timeout: tokio::time::Duration,

    pub(crate) is_serving: bool,
    pub(crate) fake_connection: bool,
}

impl UdpConection {
    async fn new(client: SocketAddr, upstream_address: SocketAddr, server: Arc<UdpSocket>) -> Self {
        Self {
            client,
            reciever_socket: Arc::new(UdpSocket::bind("0.0.0.0:0").await.unwrap()),
            upstream_address,
            server,
            timeout: UDP_RESPONSE_TIMEOUT,
            is_serving: false,
            fake_connection: false,
        }
    }

    async fn register_client_message(&self, message: Vec<u8>) {
        self.reciever_socket
            .send_to(&message, self.upstream_address)
            .await
            .unwrap();
    }

    fn serve_bidirectional(&mut self) {
        use tokio::time::interval;

        let mut buffer = [0; DEFAULT_BUFFER_SIZE];
        let reciever_socket = self.reciever_socket.clone();
        let upstream_address = self.upstream_address.clone();
        let server = self.server.clone();
        let client = self.client.clone();

        self.is_serving = true;

        tokio::spawn(async move {
            println!(
                "Serving bidirectional connection for {} and {}",
                client, upstream_address
            );

            // TODO add race with timeout to finish the task and clean up the connection after some time
            loop {
                tokio::select! {
                    result = reciever_socket.recv_from(&mut buffer) => {
                        match result {
                            Ok((bytes_read, peer_addr)) => {
                                if peer_addr != upstream_address {
                                    println!("Received message from an unknown peer. Skipping the message.",);

                                    continue;
                                }

                                println!("Received message from {}", peer_addr);

                                server.send_to(&buffer[..bytes_read], client).await.unwrap();

                                println!("Sent message to {}", client);
                            }
                            Err(_) => {
                                todo!()
                            }
                        }
                    }
                }
            }
        });
    }

    pub(crate) fn is_fake_connection(&self) -> bool {
        self.fake_connection
    }
}

impl UdpServer {
    pub(crate) async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let fields = &self.config;

        let mut client_map = HashMap::new();

        let server_socket = Arc::new(UdpSocket::bind(("0.0.0.0", fields.port)).await?);

        println!("Listening for UDP on port {}", fields.port);
        let mut counter = 0;

        loop {
            let mut buffer = [0; DEFAULT_BUFFER_SIZE];
            let (bytes_read, peer_addr) = server_socket.recv_from(&mut buffer).await?;
            println!("{}", counter);

            let upstream_address = self.service.get_address();

            println!("Received {} bytes from {}", bytes_read, peer_addr);

            let possible_new_connection =
                UdpConection::new(peer_addr, upstream_address, server_socket.clone()).await;
            let connection = client_map
                .entry(peer_addr)
                .or_insert_with(|| possible_new_connection);

            connection
                .register_client_message(buffer[..bytes_read].to_vec())
                .await;

            if !connection.is_serving {
                connection.serve_bidirectional();
            }

            counter += 1;
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
