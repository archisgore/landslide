
mod landslide;
mod error;

use std::error::Error;
use landslide::{Landslide, VmServer, Server};
use portpicker;
use error::LandslideError;

// The constants are for generating the go-plugin string
// https://github.com/hashicorp/go-plugin/blob/master/docs/guide-plugin-write-non-go.md
const GRPC_CORE_PROTOCOL_VERSION: usize = 1;
const GRPC_APP_PROTOCOL_VERSION: usize = 1;

const IPV6_LOCALHOST: &str = "[::1]";


#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<VmServer<Landslide>>()
        .await;

    health_reporter.set_serving::<VmServer<Landslide>>().await;

    let port = match portpicker::pick_unused_port() {
        Some(port) => port,
        None => return Err(Box::new(LandslideError::NoTCPPortAvailable) as Box<dyn Error>),
    };

    let addrstr = format!("{}:{}", IPV6_LOCALHOST, port);
    let addr = addrstr.parse()?;
    let vm = Landslide::default();

    println!("{}|{}|{}|{}:{}|{}", GRPC_CORE_PROTOCOL_VERSION, GRPC_APP_PROTOCOL_VERSION, "tcp", IPV6_LOCALHOST, port, "grpc");

    Server::builder()
        .add_service(health_service)
        .add_service(VmServer::new(vm))
        .serve(addr)
        .await?;

    Ok(())
}
