use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

use crate::service::TcpService;

use super::StreamFields;

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

                            upstream.write_all(&buffer_client[..bytes_from_client]).await.unwrap();

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
                                .write_all(&buffer_upstream[..bytes_from_upstream])
                                .await
                                .unwrap();
                        }
                    }
                }
            });
        }
    }
}
