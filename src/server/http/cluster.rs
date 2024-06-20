use std::{
    collections::{hash_map::Entry, HashMap},
    io,
    sync::Arc,
};

use futures::future::join_all;
use tokio::sync::Mutex;

use super::{
    route::{HttpRoute, HttpRule},
    HttpConfig, HttpServer,
};

pub(crate) struct HttpServerCluster {
    servers: Vec<HttpServer>,
}

impl HttpServerCluster {
    pub(crate) fn from_config(config: HttpConfig) -> Self {
        let HttpConfig {
            servers,
            routes,
            services,
        } = config;

        let services_map = services
            .into_iter()
            .map(|(name, backend)| (name, Arc::new(Mutex::new(backend))))
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

        Self {
            servers: servers
                .into_iter()
                .map(|config| {
                    let routes = route_map.remove(&config.name).unwrap_or_default();

                    HttpServer::new(config, routes)
                })
                .collect(),
        }
    }

    pub(crate) async fn run_all(self) -> Vec<Result<(), io::Error>> {
        join_all(self.servers.into_iter().map(HttpServer::run)).await
    }
}
