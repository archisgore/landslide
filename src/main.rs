// Common modules required by any VM
pub mod appsender;
pub mod context;
pub mod error;
pub mod id;
pub mod proto;

// timestamp VM
mod timestampvm;

use anyhow::{Context, Result};
use grr_plugin::{HandshakeConfig, Server};
use proto::vm_proto::vm_server::VmServer;
use std::env;
use std::error::Error;
use timestampvm::TimestampVm;

const LANDSLIDE_LOG_CONFIG_FILE: &str = "LANDSLIDE_LOG_CONFIG_FILE";

//https://github.com/ava-labs/avalanchego/blob/master/vms/rpcchainvm/vm.go#L19
const AVALANCHE_VM_PROTOCOL_VERSION: u32 = 9;

// https://github.com/ava-labs/avalanchego/blob/master/vms/rpcchainvm/vm.go#L20
const MAGIC_COOKIE_KEY: &str = "VM_PLUGIN";
const MAGIC_COOKIE_VALUE: &str = "dynamic";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    init_logger();

    log::info!("creating grr-plugin (go-plugin) Server...");
    let mut plugin = Server::new(
        AVALANCHE_VM_PROTOCOL_VERSION,
        HandshakeConfig {
            magic_cookie_key: MAGIC_COOKIE_KEY.to_string(),
            magic_cookie_value: MAGIC_COOKIE_VALUE.to_string(),
        },
    ).with_context(|| format!("Error creating the plugin server with Avalanche protocol version {} and Handshake configuration with magic_cookie_key: {} and magic_cookie_value: {}", AVALANCHE_VM_PROTOCOL_VERSION, MAGIC_COOKIE_KEY, MAGIC_COOKIE_VALUE))?;

    // extract the JSON-RPC Broker
    let jsonrpc_broker = plugin.jsonrpc_broker().await?;

    let tsvm = TimestampVm::new(jsonrpc_broker).context("Unable to create TimestampVm")?;

    log::info!("Initialized the timestampvm logger");
    let vm = VmServer::new(tsvm);
    log::info!("TimestampVm Service Created");

    plugin
        .serve(vm)
        .await
        .context("Error serving the plugin vm using the grr-plugin scaffolding server")?;

    Ok(())
}

fn init_logger() {
    // is there a RUST_LOG environment variable?
    if let Ok(log_config_file_path) = env::var(LANDSLIDE_LOG_CONFIG_FILE) {
        log4rs::init_file(log_config_file_path, Default::default()).unwrap();
    } else {
        // else construct a default logger
        let stderr = log4rs::append::console::ConsoleAppender::builder()
            .target(log4rs::append::console::Target::Stderr)
            .build();
        let config = log4rs::Config::builder()
            .appender(
                log4rs::config::runtime::Appender::builder().build("stderr", Box::new(stderr)),
            )
            .build(
                log4rs::config::runtime::Root::builder()
                    .appender("stderr")
                    .build(log::LevelFilter::Info),
            )
            .unwrap();

        log4rs::init_config(config).unwrap();
    }
}
