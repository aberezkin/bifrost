// TODO: break this file down
pub(crate) mod cli;

mod protocol;
mod server;
mod service;

use clap::Parser;
use cli::Args;
use futures::join;
use server::{http::cluster::HttpServerCluster, stream::cluster::StreamServerCluster};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let config_contents =
        std::fs::read_to_string(&args.config).expect("Failed to read config file");

    let config: server::Config =
        serde_yaml::from_str(&config_contents).expect("Failed to parse config file");

    println!("{:#?}", config);

    let server::Config { stream, http } = config;

    let stream_cluster = stream.map(StreamServerCluster::from_config);
    let http_cluster = http.map(HttpServerCluster::from_config);

    // Maybe a way to improve this piece? buth clusters are Option
    match (http_cluster, stream_cluster) {
        (Some(http), Some(stream)) => {
            join!(http.run_all(), stream.run_all());
        }
        (Some(http), None) => {
            http.run_all().await;
        }
        (None, Some(stream)) => {
            stream.run_all().await;
        }
        _ => {
            println!("No servers configured, shutting down");
        }
    }
}
