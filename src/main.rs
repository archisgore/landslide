// Common modules required by any VM
pub mod context;
pub mod error;
pub mod id;
mod unix;
pub mod vm;

// timestamp VM
mod timestampvm;

use error::LandslideError;
use simplelog::{Config, WriteLogger};
use std::env;
use std::error::Error;
use std::fs::File;
use tempfile::tempdir;
use timestampvm::TimestampVm;
use tokio::net::UnixListener;
use tonic::transport::Server;
use unix::UnixStream;
use vm::{Vm, VmServer};

// The constants are for generating the go-plugin string
// https://github.com/hashicorp/go-plugin/blob/master/docs/guide-plugin-write-non-go.md
const GRPC_CORE_PROTOCOL_VERSION: usize = 1;

//https://github.com/ava-labs/avalanchego/blob/master/vms/rpcchainvm/vm.go#L19
const GRPC_APP_PROTOCOL_VERSION: usize = 9;

// https://github.com/ava-labs/avalanchego/blob/master/vms/rpcchainvm/vm.go#L20
const MAGIC_COOKIE_KEY: &str = "VM_PLUGIN";
const MAGIC_COOKIE_VALUE: &str = "dynamic";

const LANDSLIDE_ENDPOINT_KEY: &str = "LANDSLIDE_ENDPOINT";
const LANDSLIDE_ENDPOINT_VALUE_TCP: &str = "tcp";
const LANDSLIDE_ENDPOINT_VALUE_UNIX: &str = "unix";

// bind to ALL addresses on Localhost
const LOCALHOST_BIND_ADDR: &str = "0.0.0.0";

// How should other processes on the localhost address localhost?
const LOCALHOST_ADVERTISE_ADDR: &str = "127.0.0.1";

enum EndpointType {
    Unix,
    Tcp,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let logfile = log_then_bubble_error!(File::create("/tmp/landslide.log"));
    log_then_bubble_error!(WriteLogger::init(
        log::LevelFilter::Debug,
        Config::default(),
        logfile
    ));

    log::info!("Initialized the timestampvm logger");

    // ensuring magic cookie
    validate_magic_cookie()?;

    let vm = TimestampVm::new()?;
    let vm_service = VmServer::new(vm);
    log::info!("TimestampVm Created");

    let endpoint_type = match env::var(LANDSLIDE_ENDPOINT_KEY) {
        Err(_) => EndpointType::Tcp,
        Ok(val) => match val.to_lowercase().as_str() {
            LANDSLIDE_ENDPOINT_VALUE_TCP => EndpointType::Tcp,
            LANDSLIDE_ENDPOINT_VALUE_UNIX => EndpointType::Unix,
            _ => {
                let err: Box<dyn Error> = Box::new(LandslideError::Generic(format!("Endpoint type '{}' set in the environment is invalid. Please set it to 'tcp' or 'unix' (or unset it so the default will be used.)", val)));
                return Err(err);
            }
        },
    };

    match endpoint_type {
        EndpointType::Unix => service_unix(vm_service).await,
        EndpointType::Tcp => service_tcp(vm_service).await,
    }
}

// Copied from: https://github.com/hashicorp/go-plugin/blob/master/server.go#L247
fn validate_magic_cookie() -> Result<(), LandslideError> {
    if let Ok(value) = env::var(MAGIC_COOKIE_KEY) {
        if value == MAGIC_COOKIE_VALUE {
            return Ok(());
        }
    }

    Err(LandslideError::GRPCHandshakeMagicCookieValueMismatch)
}

async fn service_unix<T: Vm>(vm_service: VmServer<T>) -> Result<(), Box<dyn Error>> {
    log::info!("Serving over a Unix Socket...");

    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter.set_serving::<VmServer<T>>().await;
    log::info!("Set up health service");

    let temp_root_path = log_then_bubble_error!(tempdir());
    let unix_socket_pathbuf = temp_root_path.path().join("plugin.sock");

    let unix_socket_path = match unix_socket_pathbuf.to_str() {
        Some(usp) => usp,
        None => {
            let err: Box<dyn Error> = Box::new(LandslideError::Generic(format!(
                "Unable to convert temp_path into a string: {:?}.",
                unix_socket_pathbuf
            )));
            return Err(err);
        }
    };

    log::info!("Picked temporary Unix socket path: {}", unix_socket_path);

    let incoming = {
        let unix_socket = log_then_bubble_error!(UnixListener::bind(unix_socket_path));

        async_stream::stream! {
            loop {
                let item = unix_socket.accept().await.map(|(st, _)| UnixStream(st));

                yield item;
            }
        }
    };

    log::info!("Bound to Unix socket at path: {}", unix_socket_path);

    let handshakestr = format!(
        "{}|{}|{}|{}|grpc|",
        GRPC_CORE_PROTOCOL_VERSION,
        GRPC_APP_PROTOCOL_VERSION,
        LANDSLIDE_ENDPOINT_VALUE_UNIX,
        unix_socket_path
    );

    log::info!("About to print Handshake string: {}", handshakestr);
    println!("{}", handshakestr);

    log::info!("About to begin serving....");
    Server::builder()
        .add_service(health_service)
        .add_service(vm_service)
        .serve_with_incoming(incoming)
        .await?;

    log::info!("Serving ended! Plugin about to shut down.");

    Ok(())
}

async fn service_tcp<T: Vm>(vm_service: VmServer<T>) -> Result<(), Box<dyn Error>> {
    log::info!("Serving over a Tcp Socket...");

    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter.set_serving::<VmServer<T>>().await;
    log::info!("Set up health service");

    let port = match portpicker::pick_unused_port() {
        Some(p) => p,
        None => {
            let err: Box<dyn Error> = Box::new(LandslideError::Generic(
                "Unable to find a free unused TCP port to bind the gRPC server to".to_string(),
            ));
            return Err(err);
        }
    };

    log::info!("Picked port: {}", port);

    let addrstr = format!("{}:{}", LOCALHOST_BIND_ADDR, port);
    let addr = addrstr.parse()?;

    let handshakestr = format!(
        "{}|{}|{}|{}:{}|grpc|",
        GRPC_CORE_PROTOCOL_VERSION,
        GRPC_APP_PROTOCOL_VERSION,
        LANDSLIDE_ENDPOINT_VALUE_TCP,
        LOCALHOST_ADVERTISE_ADDR,
        port
    );

    log::info!("About to print Handshake string: {}", handshakestr);
    println!("{}", handshakestr);

    log::info!("About to begin serving....");
    Server::builder()
        .add_service(health_service)
        .add_service(vm_service)
        .serve(addr)
        .await?;

    log::info!("Serving ended! Plugin about to shut down.");

    Ok(())
}
