pub(crate) mod plane;

use plane::control::control_server::ControlServer;
use plane::MyControl;
use tonic::transport::Server;

pub(crate) async fn run_grpc() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50005".parse()?;
    let greeter = MyControl;

    Server::builder()
        .add_service(ControlServer::new(greeter))
        .serve(addr)
        .await?;

    Ok(())
}
