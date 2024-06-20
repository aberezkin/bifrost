use control::{
    control_server::{Control, ControlServer},
    GetConfigReply, GetConfigRequest,
};
use tonic::{Request, Response, Status};

pub mod control {
    tonic::include_proto!("control");
}

#[derive(Debug, Default)]
pub struct MyControl;

#[tonic::async_trait]
impl Control for MyControl {
    async fn get_config(
        &self,
        request: Request<GetConfigRequest>,
    ) -> Result<Response<GetConfigReply>, Status> {
        println!("Got a request: {:?}", request);

        let config = GetConfigReply {
            contents: "No config yet, amateur".to_owned(),
        };

        Ok(Response::new(config))
    }
}
