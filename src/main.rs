pub(crate) mod cli;

mod server;
mod service;

use clap::Parser;
use cli::Args;
use futures::future::join_all;
use server::stream::{StreamServer, StreamServerConfig};
use service::{config::StreamServiceConfig, Service, TcpService, UdpService};
use std::{collections::HashMap, fs};

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let config_contents = fs::read_to_string(&args.config).expect("Failed to read config file");

    let config: server::Config =
        serde_yaml::from_str(&config_contents).expect("Failed to parse config file");

    println!("{:#?}", config);

    let services: HashMap<_, _> = config
        .stream
        .services
        .into_iter()
        .map(|(name, config)| {
            (
                name,
                match config {
                    StreamServiceConfig::Tcp(config) => TcpService::new(config),
                    // TODO: UDP
                    StreamServiceConfig::Udp(config) => TcpService::new(config),
                },
            )
        })
        .collect();

    let servers = config.stream.servers.into_iter().map(|config| {
        let service = match &config {
            StreamServerConfig::Tcp(config) => config,
            // TODO: UDP
            StreamServerConfig::Udp(config) => config,
        };

        let service = services
            .get(&service.service)
            .expect("Service not found")
            .clone();

        StreamServer::new(config, service)
    });

    let futures = servers.map(|server| server.run());

    join_all(futures).await;
}
