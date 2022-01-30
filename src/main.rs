// Common modules required by any VM
pub mod context;
pub mod error;
pub mod id;
pub mod vm;

// timestamp VM
mod timestampvm;

use error::LandslideError;
use portpicker::pick_unused_port;
use std::error::Error;
use timestampvm::TimestampVm;
use vm::{Server, VmServer};

// The constants are for generating the go-plugin string
// https://github.com/hashicorp/go-plugin/blob/master/docs/guide-plugin-write-non-go.md
const GRPC_CORE_PROTOCOL_VERSION: usize = 1;
const GRPC_APP_PROTOCOL_VERSION: usize = 1;

const IPV6_LOCALHOST: &str = "[::1]";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter.set_serving::<VmServer<TimestampVm>>().await;

    health_reporter.set_serving::<VmServer<TimestampVm>>().await;

    let port = match pick_unused_port() {
        Some(port) => port,
        None => return Err(Box::new(LandslideError::NoTCPPortAvailable) as Box<dyn Error>),
    };

    let addrstr = format!("{}:{}", IPV6_LOCALHOST, port);
    let addr = addrstr.parse()?;

    let vm = TimestampVm::new()?;

    println!(
        "{}|{}|tcp|{}:{}|grpc",
        GRPC_CORE_PROTOCOL_VERSION, GRPC_APP_PROTOCOL_VERSION, IPV6_LOCALHOST, port
    );

    Server::builder()
        .add_service(health_service)
        .add_service(VmServer::new(vm))
        .serve(addr)
        .await?;

    Ok(())
}
