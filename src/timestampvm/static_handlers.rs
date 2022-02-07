use crate::encoding;
use encoding::{Checksum, Encoding};
use jsonrpc_core::{Error as JsonRpcError, IoHandler, Result};
use jsonrpc_derive::rpc;
use num::FromPrimitive;
use serde::{Deserialize, Serialize};

pub fn new() -> IoHandler {
    let mut io = IoHandler::new();
    let static_handlers = StaticHandlersImpl;

    io.extend_with(static_handlers.to_delegate());

    io
}

#[derive(Serialize, Deserialize)]
pub struct EncodeArgs {
    data: String,
    encoding: u8,
    length: i32,
}

#[derive(Serialize, Deserialize)]
pub struct EncodeReply {
    bytes: String,
    encoding: u8,
}

#[derive(Serialize, Deserialize)]
pub struct DecodeArgs {
    bytes: String,
    encoding: u8,
}

#[derive(Serialize, Deserialize)]
pub struct DecodeReply {
    data: String,
    encoding: u8,
}

#[rpc(server)]
pub trait StaticHandlers {
    #[rpc(name = "encode", alias("timestampvm.encode"))]
    fn encode(&self, args: EncodeArgs) -> Result<EncodeReply>;

    #[rpc(name = "decode", alias("timestampvm.decode"))]
    fn decode(&self, args: DecodeArgs) -> Result<DecodeReply>;
}

pub struct StaticHandlersImpl;

impl StaticHandlers for StaticHandlersImpl {
    fn encode(&self, args: EncodeArgs) -> Result<EncodeReply> {
        log::info!("Encode called");
        if args.data.is_empty() {
            return Err(JsonRpcError::invalid_params("data length was zero"));
        }

        let mut rawstr = args.data.clone();

        if args.length > 0 {
            rawstr.truncate(usize::try_from(args.length).map_err(|e| {
                let errmsg = format!(
                    "Error truncating data to be encoded from length {} to length {}: {}",
                    args.data.len(),
                    args.length,
                    e
                );
                log::error!("{}", errmsg);
                jsonrpc_core::Error::invalid_params(errmsg)
            })?);
        }

        let encoding_impl = Encoding::from_u8(args.encoding).ok_or_else(|| {
            jsonrpc_core::Error::invalid_params(format!(
                "Unknown Encoding requested: {}",
                args.encoding
            ))
        })?;

        let bytes = encoding_impl
            .encode(rawstr.as_bytes(), Checksum::Yes)
            .map_err(|e| {
                let errmsg = format!("Error encoding data into format {}: {}", args.encoding, e);
                log::error!("{}", errmsg);
                jsonrpc_core::Error::invalid_params(errmsg)
            })?;

        Ok(EncodeReply {
            bytes,
            encoding: args.encoding,
        })
    }

    fn decode(&self, args: DecodeArgs) -> Result<DecodeReply> {
        log::info!("Decode called");
        let encoding = Encoding::from_u8(args.encoding).ok_or_else(|| {
            jsonrpc_core::Error::invalid_params(format!(
                "Unknown Encoding requested: {}",
                args.encoding
            ))
        })?;

        let bytes = String::from_utf8(encoding.decode(args.bytes, Checksum::Yes).map_err(|e| {
            let errmsg = format!("Error decoding data from format {}: {}", args.encoding, e);
            log::error!("{}", errmsg);
            jsonrpc_core::Error::invalid_params(errmsg)
        })?)
        .map_err(|e| {
            let errmsg = format!("Error creating a utf-8 string from decoded bytes: {}", e);
            log::error!("{}", errmsg);
            jsonrpc_core::Error::invalid_params(errmsg)
        })?;

        Ok(DecodeReply {
            data: bytes,
            encoding: args.encoding,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use log::LevelFilter;
    use log4rs::append::console::ConsoleAppender;
    use log4rs::config::{Appender, Config, Root};
    use serde_json::json;

    fn init() {
        let stdout = ConsoleAppender::builder().build();

        let config = Config::builder()
            .appender(Appender::builder().build("stdout", Box::new(stdout)))
            .build(Root::builder().appender("stdout").build(LevelFilter::Info))
            .unwrap();

        let handle = log4rs::init_config(config).unwrap();
    }

    #[tokio::test]
    async fn test_encode_cb58() {
        let req = json!({
            "jsonrpc": "2.0",
            "method": "encode",
            "params": [{
                "data":"helloworld",
                "encoding": 0,
                "length": 0,
            }],
            "id": 1
        })
        .to_string();

        let io = new();
        let response = io.handle_request(&req).await.unwrap();
        assert_eq!(response, "{\"jsonrpc\":\"2.0\",\"result\":{\"bytes\":\"fP1vxkpyLWnH9dJoiyh\",\"encoding\":0},\"id\":1}");
    }

    #[tokio::test]
    async fn test_encode_cb58_alias() {
        let req = json!({
            "jsonrpc": "2.0",
            "method": "timestampvm.encode",
            "params": [{
                "data":"helloworld",
                "encoding": 0,
                "length": 0,
            }],
            "id": 1
        })
        .to_string();

        let io = new();
        let response = io.handle_request(&req).await.unwrap();
        assert_eq!(response, "{\"jsonrpc\":\"2.0\",\"result\":{\"bytes\":\"fP1vxkpyLWnH9dJoiyh\",\"encoding\":0},\"id\":1}");
    }

    #[tokio::test]
    async fn test_encode_hex_length() {
        let io = new();

        let req_with_foobar = json!({
            "jsonrpc": "2.0",
            "method": "encode",
            "params": [{
                "data":"helloworldfoobar",
                "encoding": 1,
                "length": 10,
            }],
            "id": 1
        })
        .to_string();

        let req_without_foobar = json!({
            "jsonrpc": "2.0",
            "method": "encode",
            "params": [{
                "data":"helloworld",
                "encoding": 1,
                "length": 10,
            }],
            "id": 1
        })
        .to_string();

        let response_with_foobar = io.handle_request(&req_with_foobar).await.unwrap();
        let response_without_foobar = io.handle_request(&req_without_foobar).await.unwrap();
        assert_eq!(response_with_foobar, response_without_foobar);
    }

    #[tokio::test]
    async fn test_encode_hex() {
        let req = json!({
            "jsonrpc": "2.0",
            "method": "encode",
            "params": [{
                "data":"helloworld",
                "encoding": 1,
                "length": 0,
            }],
            "id": 1
        })
        .to_string();

        let io = new();
        let response = io.handle_request(&req).await.unwrap();
        assert_eq!(response, "{\"jsonrpc\":\"2.0\",\"result\":{\"bytes\":\"0x68656c6c6f776f726c64936a185c\",\"encoding\":1},\"id\":1}");
    }

    #[tokio::test]
    async fn test_decode_cb58() {
        let req = json!({
            "jsonrpc": "2.0",
            "method": "decode",
            "params": [{
                "bytes":"fP1vxkpyLWnH9dJoiyh",
                "encoding": 0,
            }],
            "id": 1
        })
        .to_string();

        let io = new();
        let response = io.handle_request(&req).await.unwrap();
        assert_eq!(
            response,
            "{\"jsonrpc\":\"2.0\",\"result\":{\"data\":\"helloworld\",\"encoding\":0},\"id\":1}"
        );
    }

    #[tokio::test]
    async fn test_decode_cb58_alias() {
        let req = json!({
            "jsonrpc": "2.0",
            "method": "timestampvm.decode",
            "params": [{
                "bytes":"fP1vxkpyLWnH9dJoiyh",
                "encoding": 0,
            }],
            "id": 1
        })
        .to_string();

        let io = new();
        let response = io.handle_request(&req).await.unwrap();
        assert_eq!(
            response,
            "{\"jsonrpc\":\"2.0\",\"result\":{\"data\":\"helloworld\",\"encoding\":0},\"id\":1}"
        );
    }
}
