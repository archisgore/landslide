// Common modules required by any VM
pub mod context;
pub mod error;
pub mod id;
pub mod vm;

// timestamp VM
mod timestampvm;

use grr_plugin::{HandshakeConfig, Server};
use simplelog::{Config, WriteLogger};
use std::error::Error;
use std::fs::File;
use timestampvm::TimestampVm;
use vm::PluginVmServer;

//https://github.com/ava-labs/avalanchego/blob/master/vms/rpcchainvm/vm.go#L19
const AVALANCHE_VM_PROTOCOL_VERSION: u32 = 9;

// https://github.com/ava-labs/avalanchego/blob/master/vms/rpcchainvm/vm.go#L20
const MAGIC_COOKIE_KEY: &str = "VM_PLUGIN";
const MAGIC_COOKIE_VALUE: &str = "dynamic";
const LOCALHOST_ADVERTISE_ADDR: &str = "127.0.0.1";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let logfile = log_and_escalate!(File::create("/tmp/landslide.log"));
    log_and_escalate!(WriteLogger::init(
        log::LevelFilter::Info,
        Config::default(),
        logfile
    ));

    log::info!("Initialized the timestampvm logger");

    let vm = PluginVmServer::new(TimestampVm::new()?);
    log::info!("TimestampVm Service Created");

    log::info!("creating grr-plugin (go-plugin) Server...");
    let plugin = Server::new(
        AVALANCHE_VM_PROTOCOL_VERSION,
        HandshakeConfig {
            magic_cookie_key: MAGIC_COOKIE_KEY.to_string(),
            magic_cookie_value: MAGIC_COOKIE_VALUE.to_string(),
        },
    );

    log_and_escalate!(plugin.serve(vm).await);

    Ok(())
}
