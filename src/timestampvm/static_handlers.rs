use jsonrpc_core::{Result, IoHandler};
use jsonrpc_derive::rpc;

const LOG_PREFIX: &str = "TimestampVM::StaticHandlers: ";

pub fn new() -> IoHandler {
    let mut io = IoHandler::new();
    let static_handlers = StaticHandlersImpl;
  
    io.extend_with(static_handlers.to_delegate());

    io
}

#[rpc(server)]
pub trait StaticHandlers {

    #[rpc(name = "")]
    fn catch_all(&self) -> Result<u64>;

	#[rpc(name = "Encode")]
	fn encode(&self, data: String, encoding: String, length: i32) -> Result<u64>;

	#[rpc(name = "Decode")]
	fn decode(&self, a: u64, b: u64) -> Result<u64>;
}

pub struct StaticHandlersImpl;
impl StaticHandlers for StaticHandlersImpl {
    fn catch_all(&self) -> Result<u64> {
        log::info!("{} Catch all", LOG_PREFIX);
        Ok(23)
    }

	fn encode(&self, data: String, encoding: String, length: i32) -> Result<u64> {
        log::info!("{} Encode called", LOG_PREFIX);
        Ok(21)
    }
    
	fn decode(&self, a: u64, b: u64) -> Result<u64> {
        log::info!("{} Decode called", LOG_PREFIX);
        Ok(32)
    }
}
