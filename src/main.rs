// TODO: break this file down
pub(crate) mod cli;

mod protocol;
mod server;
mod service;

use clap::Parser;
use cli::Args;
use futures::{future::OptionFuture, join};
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

    let stream_cluster: OptionFuture<_> = stream
        .map(StreamServerCluster::from_config)
        .map(StreamServerCluster::run_all)
        .into();
    let http_cluster: OptionFuture<_> = http
        .map(HttpServerCluster::from_config)
        .map(HttpServerCluster::run_all)
        .into();

    join!(stream_cluster, http_cluster);
}
