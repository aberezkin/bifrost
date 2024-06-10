use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use tokio::net::UdpSocket;

use crate::service::UdpService;

const DEFAULT_BUFFER_SIZE: usize = 8 * 1024; // 2KB

pub(crate) struct UdpServer {
    pub(crate) config: StreamFields,
    pub(crate) service: UdpService,
}

use super::StreamFields;

struct UdpConection {
    client: SocketAddr,
    reciever_socket: Arc<UdpSocket>,
    upstream_address: SocketAddr,
    server: Arc<UdpSocket>,

    pub(crate) is_serving: bool,
}

impl UdpConection {
    async fn new(client: SocketAddr, upstream_address: SocketAddr, server: Arc<UdpSocket>) -> Self {
        Self {
            client,
            reciever_socket: Arc::new(UdpSocket::bind("0.0.0.0:0").await.unwrap()),
            upstream_address,
            server,
            is_serving: false,
        }
    }

    async fn register_client_message(&self, message: Vec<u8>) {
        self.reciever_socket
            .send_to(&message, self.upstream_address)
            .await
            .unwrap();
    }

    fn serve_bidirectional(&mut self) {
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
