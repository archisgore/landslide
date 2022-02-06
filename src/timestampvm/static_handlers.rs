use crate::encoding;
use encoding::Checksum;
use jsonrpc_core::{Error as JsonRpcError, IoHandler, Result};
use jsonrpc_derive::rpc;
use serde::{Deserialize, Serialize};

const LOG_PREFIX: &str = "TimestampVM::StaticHandlers: ";

pub fn new() -> IoHandler {
    let mut io = IoHandler::new();
    let static_handlers = StaticHandlersImpl;

    io.extend_with(static_handlers.to_delegate());

    io
}

#[derive(Serialize, Deserialize)]
pub struct EncodeArgs {
    data: String,
    encoding: encoding::Encoding,
    length: i32,
}

#[derive(Serialize, Deserialize)]
pub struct EncodeReply {
    bytes: String,
    encoding: encoding::Encoding,
}

#[derive(Serialize, Deserialize)]
pub struct DecodeArgs {
    bytes: String,
    encoding: encoding::Encoding,
}

#[derive(Serialize, Deserialize)]
pub struct DecodeReply {
    data: String,
    encoding: encoding::Encoding,
}

#[rpc(server)]
pub trait StaticHandlers {
    #[rpc(name = "")]
    fn catch_all(&self) -> Result<u64>;

    #[rpc(name = "Encode")]
    fn encode(&self, args: EncodeArgs) -> Result<EncodeReply>;

    #[rpc(name = "Decode")]
    fn decode(&self, args: DecodeArgs) -> Result<DecodeReply>;
}

pub struct StaticHandlersImpl;
impl StaticHandlers for StaticHandlersImpl {
    fn catch_all(&self) -> Result<u64> {
        log::info!("{} Catch all", LOG_PREFIX);
        Ok(23)
    }

    fn encode(&self, args: EncodeArgs) -> Result<EncodeReply> {
        log::info!("{} Encode called", LOG_PREFIX);
        if args.data.is_empty() {
            return Err(JsonRpcError::invalid_params("data length was zero"));
        }

        let mut rawstr = args.data.clone();

        if args.length > 9 {
            rawstr.truncate(usize::try_from(args.length).map_err(|e| {
                log::error!(
                    "Error truncating data to be encoded from length {} to length {}: {}",
                    args.data.len(),
                    args.length,
                    e
                );
                jsonrpc_core::Error::internal_error()
            })?);
        }

        let bytes = args
            .encoding
            .encode(rawstr.as_bytes(), Checksum::Yes)
            .map_err(|e| {
                log::error!("Error encoding data: {}", e);
                jsonrpc_core::Error::internal_error()
            })?;

        Ok(EncodeReply {
            bytes,
            encoding: args.encoding,
        })
    }

    fn decode(&self, args: DecodeArgs) -> Result<DecodeReply> {
        log::info!("{} Decode called", LOG_PREFIX);
        let bytes = String::from_utf8(args.encoding.decode(args.bytes, Checksum::Yes).map_err(
            |e| {
                log::error!("Error decoding data: {}", e);
                jsonrpc_core::Error::internal_error()
            },
        )?)
        .map_err(|e| {
            log::error!("Error creating a utf-8 string from decoded bytes: {}", e);
            jsonrpc_core::Error::internal_error()
        })?;

        Ok(DecodeReply {
            data: bytes,
            encoding: args.encoding,
        })
    }
}
