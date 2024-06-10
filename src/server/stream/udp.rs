use std::collections::hash_map::Entry;
use std::time::{Duration, Instant};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use tokio::net::UdpSocket;
use tokio::sync::{oneshot, Mutex};

use crate::service::UdpService;

const DEFAULT_BUFFER_SIZE: usize = 8 * 1024; // 8KB
const DEFAULT_TIME_TO_LIVE: Duration = Duration::from_secs(10);

pub(crate) struct UdpServer {
    pub(crate) config: StreamFields,
    pub(crate) service: UdpService,
}

use super::StreamFields;

struct UdpConnection {
    client: SocketAddr,
    receiver_socket: Arc<UdpSocket>,
    upstream_address: SocketAddr,
    server: Arc<UdpSocket>,
    close_tx: Option<oneshot::Sender<()>>,
    is_serving: bool,

    // NOTE: Maybe it makes sense to separate this into a separate struct
    last_activity: Arc<Mutex<Instant>>,
    time_to_live: Duration,
}

impl UdpConnection {
    async fn new(client: SocketAddr, upstream_address: SocketAddr, server: Arc<UdpSocket>) -> Self {
        Self {
            client,
            receiver_socket: Arc::new(UdpSocket::bind("0.0.0.0:0").await.unwrap()),
            upstream_address,
            server,
            close_tx: None,
            is_serving: false,

            last_activity: Arc::new(Mutex::new(Instant::now())),
            time_to_live: DEFAULT_TIME_TO_LIVE,
        }
    }

    async fn relay_client_message(&self, message: Vec<u8>) {
        {
            *self.last_activity.lock().await = Instant::now();
        }

        self.receiver_socket
            .send_to(&message, self.upstream_address)
            .await
            .unwrap();
    }

    fn serve_bidirectional(&mut self) {
        if self.is_serving {
            return;
        }

        let mut buffer = [0; DEFAULT_BUFFER_SIZE];
        let receiver_socket = self.receiver_socket.clone();
        let upstream_address = self.upstream_address.clone();
        let server = self.server.clone();
        let client = self.client.clone();
        let last_activity = self.last_activity.clone();

        let (close_tx, close_rx) = oneshot::channel();
        self.close_tx = Some(close_tx);

        self.is_serving = true;

        tokio::spawn(async move {
            println!(
                "Serving bidirectional connection for {} and {}",
                client, upstream_address
            );

            tokio::pin!(close_rx);

            loop {
                tokio::select! {
                    result = receiver_socket.recv_from(&mut buffer) => {
                        match result {
                            Ok((bytes_read, peer_addr)) => {
                                if peer_addr != upstream_address {
                                    println!("Received message from an unknown peer. Skipping the message.");

                                    continue;
                                }

                                {
                                    *last_activity.lock().await = Instant::now();
                                }

                                println!("Received message from {}", peer_addr);

                                server.send_to(&buffer[..bytes_read], client).await.unwrap();

                                println!("Sent message to {}", client);
                            }
                            Err(e) => {
                                eprintln!("Error receiving from upstream: {}", e);
                                break;
                            }
                        }
                    }
                    _ = &mut close_rx => {
                        println!("Connection {} to {} is closing", client, upstream_address);
                        break;
                    }
                }
            }
        });
    }

    fn close(mut self) {
        if let Some(close_tx) = self.close_tx.take() {
            let _ = close_tx.send(()); // Send the close signal
        }
    }

    async fn is_stale(&self) -> bool {
        self.last_activity.lock().await.elapsed() > self.time_to_live
    }
}

impl UdpServer {
    pub(crate) async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let fields = &self.config;

        let client_map: Arc<Mutex<HashMap<SocketAddr, UdpConnection>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let server_socket = Arc::new(UdpSocket::bind(("0.0.0.0", fields.port)).await?);

        let client_map_clone = client_map.clone();

        tokio::spawn(async move {
            let mut sec = tokio::time::interval(Duration::from_secs(1));

            loop {
                sec.tick().await;

                let mut client_map = client_map_clone.lock().await;
                let vec: Vec<SocketAddr> = client_map.keys().map(SocketAddr::clone).collect();

                for addr in vec {
                    if client_map.get(&addr).unwrap().is_stale().await {
                        println!("Closing connection to {}", addr);
                        if let Some(connection) = client_map.remove(&addr) {
                            connection.close();
                        }
                    }
                }
            }
        });

        println!("Listening for UDP on port {}", fields.port);

        loop {
            let mut buffer = [0; DEFAULT_BUFFER_SIZE];
            let (bytes_read, peer_addr) = server_socket.recv_from(&mut buffer).await?;

            let upstream_address = self.service.get_address();

            println!("Received {} bytes from {}", bytes_read, peer_addr);

            let client_map = client_map.clone();
            let server_socket = server_socket.clone();

            let mut client_map = client_map.lock().await;

            match client_map.entry(peer_addr) {
                Entry::Occupied(mut entry) => {
                    let connection: &mut UdpConnection = entry.get_mut();

                    connection
                        .relay_client_message(buffer[..bytes_read].to_vec())
                        .await;
                }
                Entry::Vacant(entry) => {
                    let mut new_connection =
                        UdpConnection::new(peer_addr, upstream_address, server_socket.clone())
                            .await;

                    new_connection
                        .relay_client_message(buffer[..bytes_read].to_vec())
                        .await;

                    new_connection.serve_bidirectional();

                    entry.insert(new_connection);
                }
            }
        }
    }
}
