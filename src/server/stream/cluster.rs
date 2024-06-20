use std::collections::HashMap;

use futures::future::join_all;

use crate::service::Service;

use super::{StreamServer, StreamServerConfig, StreamingConfig};

pub(crate) struct StreamServerCluster {
    servers: Vec<StreamServer>,
}

impl StreamServerCluster {
    pub(crate) fn from_config(config: StreamingConfig) -> Self {
        let services: HashMap<_, _> = config
            .services
            .into_iter()
            .map(|(name, config)| (name, Service::new(config)))
            .collect();

        let servers= config.servers.into_iter().map(|config| {
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
        }).collect();

        Self { servers }
    }

    pub(crate) async fn run_all(self) -> Vec<Result<(), Box<dyn std::error::Error>>> {
        join_all(self.servers.into_iter().map(StreamServer::run)).await
    }
}
