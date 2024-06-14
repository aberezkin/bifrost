// TODO: break this filed down
pub(crate) mod cli;

mod protocol;
mod server;
mod service;

use clap::Parser;
use cli::Args;
use futures::future::join_all;
use futures::join;
use server::{
    http::{HttpRoute, HttpRule},
    stream::{StreamServer, StreamServerConfig},
};
use service::Service;
use std::{collections::HashMap, fs};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let config_contents = fs::read_to_string(&args.config).expect("Failed to read config file");

    let config: server::Config =
        serde_yaml::from_str(&config_contents).expect("Failed to parse config file");

    println!("{:#?}", config);

    let server::Config { stream, http } = config;

    let stream_servers = stream.map_or(vec![], |stream| {
        let services: HashMap<_, _> = stream
            .services
            .into_iter()
            .map(|(name, config)| (name, Service::new(config)))
            .collect();

        let servers = stream.servers.into_iter().map(|config| {
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

        servers.map(|server| server.run()).collect()
    });

    use crate::server::http::HttpServer;
    use std::collections::hash_map::Entry;
    use std::sync::Arc;

    let http_servers = http.map_or(vec![], |http| {
        let servers = http.servers;
        let routes = http.routes;

        let services_map = http
            .services
            .into_iter()
            .map(|(name, backend)| (name, Arc::new(backend)))
            .collect::<HashMap<_, _>>();

        let mut route_map = HashMap::<String, Vec<HttpRoute>>::new();

        for route in routes {
            let server_name = route.server;

            let hostnames = route.hostnames;
            let rules = route
                .rules
                .into_iter()
                .map(|rule| {
                    let backend = services_map.get(&rule.backend).unwrap().clone();

                    HttpRule::new(rule.matches, backend)
                })
                .collect();

            let route = HttpRoute {
                hostnames: hostnames.unwrap_or_default(),
                rules,
            };

            match route_map.entry(server_name) {
                Entry::Occupied(mut entry) => {
                    entry.get_mut().push(route);
                }
                Entry::Vacant(entry) => {
                    entry.insert(vec![route]);
                }
            }
        }

        let servers = servers
            .into_iter()
            .map(|config| {
                let routes = route_map.remove(&config.name).unwrap();

                if routes.is_empty() {
                    panic!("No routes found for server {}", config.name);
                }

                HttpServer::new(config, routes)
            })
            .collect();

        servers
    });

    // We need to do these join hoops to make all servers run in parallel
    let stream_servers = join_all(stream_servers);
    let http_servers = join_all(http_servers.into_iter().map(|server| server.run()));

    // NOTE: we can't directly join the two vectors of futurees because they are not the same type
    // see: https://users.rust-lang.org/t/expected-opaque-type-found-a-different-opaque-type-when-trying-futures-join-all/40596
    join!(stream_servers, http_servers);
}
