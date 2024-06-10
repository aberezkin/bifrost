pub(crate) mod cli;

mod protocol;
mod server;
mod service;

use clap::Parser;
use cli::Args;
use futures::future::join_all;
use server::stream::{StreamServer, StreamServerConfig};
use service::Service;
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
        .map(|(name, config)| (name, Service::new(config)))
        .collect();

    let servers = config.stream.servers.into_iter().map(|config| {
        let service_name = match &config {
            StreamServerConfig::Tcp(config) => config.service.clone(),
            StreamServerConfig::Udp(config) => config.service.clone(),
        };

        let service = services
            .get(&service_name)
            .expect("Service not found")
            .clone();

        match (config, service) {
            (StreamServerConfig::Tcp(config), Service::Tcp(service)) => {
                StreamServer::tcp(config, service)
            }
            (StreamServerConfig::Udp(config), Service::Udp(service)) => {
                StreamServer::udp(config, service)
            }
            (server_config, service) => {
                // NOTE: What are we going to do when we have a dynamic configuration? Maybe some
                // pre-validation step?
                panic!(
                    "Invalid stream service config, server and an upstream service must use same protocol. Server is {:?}, service is {:?}",
                    server_config.get_protocol(),
                    service.get_protocol()
                );
            }
        }
    });

    let futures = servers.map(|server| server.run());

    join_all(futures).await;
}
