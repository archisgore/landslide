use crate::encoding;
use encoding::{Checksum, Encoding};
use jsonrpc_core::{BoxFuture, Error as JsonRpcError, IoHandler, Result};
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
    encoding: Option<u8>,
    length: Option<usize>,
}

#[derive(Serialize, Deserialize)]
pub struct EncodeReply {
    bytes: String,
    encoding: u8,
}

#[derive(Serialize, Deserialize)]
pub struct DecodeArgs {
    bytes: String,
    encoding: Option<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct DecodeReply {
    data: String,
    encoding: u8,
}

#[rpc(server)]
pub trait StaticHandlers {
    #[rpc(name = "encode", alias("timestampvm.encode"))]
    fn encode(&self, args: EncodeArgs) -> BoxFuture<Result<EncodeReply>>;

    #[rpc(name = "decode", alias("timestampvm.decode"))]
    fn decode(&self, args: DecodeArgs) -> BoxFuture<Result<DecodeReply>>;
}

pub struct StaticHandlersImpl;

impl StaticHandlers for StaticHandlersImpl {
    fn encode(&self, args: EncodeArgs) -> BoxFuture<Result<EncodeReply>> {
        Box::pin(async move {
            log::trace!("Encode called");
            if args.data.is_empty() {
                return Err(JsonRpcError::invalid_params("data length was zero"));
            }

            let mut rawstr = args.data.clone();

            let length = args.length.unwrap_or(0);

            if length > 0 {
                rawstr.truncate(length);
            }

            let encoding_u8 = args.encoding.unwrap_or(0);
            let encoding = Encoding::from_u8(encoding_u8).ok_or_else(|| {
                JsonRpcError::invalid_params(format!("Encoding {} unknown", encoding_u8))
            })?;

            let bytes = encoding
                .encode(rawstr.as_bytes(), Checksum::Yes)
                .map_err(|e| {
                    let errmsg = format!("Error encoding data into format {:?}: {}", encoding, e);
                    log::error!("{}", errmsg);
                    JsonRpcError::invalid_params(errmsg)
                })?;

            Ok(EncodeReply {
                bytes,
                encoding: encoding as u8,
            })
        })
    }

    fn decode(&self, args: DecodeArgs) -> BoxFuture<Result<DecodeReply>> {
        Box::pin(async move {
            log::trace!("Decode called");

            let encoding_u8 = args.encoding.unwrap_or(0);
            let encoding = Encoding::from_u8(encoding_u8).ok_or_else(|| {
                JsonRpcError::invalid_params(format!("Unknown Encoding requested: {}", encoding_u8))
            })?;

            let bytes =
                String::from_utf8(encoding.decode(args.bytes, Checksum::Yes).map_err(|e| {
                    let errmsg = format!("Error decoding data from format {:?}: {}", encoding, e);
                    log::error!("{}", errmsg);
                    JsonRpcError::invalid_params(errmsg)
                })?)
                .map_err(|e| {
                    let errmsg = format!("Error creating a utf-8 string from decoded bytes: {}", e);
                    log::error!("{}", errmsg);
                    JsonRpcError::invalid_params(errmsg)
                })?;

            Ok(DecodeReply {
                data: bytes,
                encoding: encoding as u8,
            })
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::json;

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
    async fn test_encode_cb58_partial() {
        let req = json!({
            "jsonrpc": "2.0",
            "method": "encode",
            "params": [{
                "data":"helloworld",
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
