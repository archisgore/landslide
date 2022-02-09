// This is pulled from vm.proto
pub mod vm_proto {
    tonic::include_proto!("vmproto");
}

// This is pulled from metrics.proto
pub mod io {
    pub mod prometheus {
        pub mod client {
            tonic::include_proto!("io.prometheus.client");
        }
    }
}

pub mod appsender {
    tonic::include_proto!("appsenderproto");
}

pub mod galiasreader {
    tonic::include_proto!("galiasreaderproto");
}

pub mod ghttp {
    tonic::include_proto!("ghttpproto");
}

pub mod gkeystore {
    tonic::include_proto!("gkeystoreproto");
}

pub mod gsharedmemory {
    tonic::include_proto!("gsharedmemoryproto");
}

pub mod gsubnetlookup {
    tonic::include_proto!("gsubnetlookupproto");
}

pub mod messenger {
    tonic::include_proto!("messengerproto");
}

pub mod rpcdb {
    tonic::include_proto!("rpcdbproto");
}

pub mod greadcloser {
    tonic::include_proto!("greadcloserproto");
}

pub mod gresponsewriter {
    tonic::include_proto!("gresponsewriterproto");
}

use super::error::into_status;
use anyhow::{Context, Result};
use ghttp::http_server::HttpServer;
use ghttp::{HttpRequest, HttpResponse};
use greadcloser::{reader_client::ReaderClient, ReadRequest};
use gresponsewriter::{writer_client::WriterClient, WriteRequest};
use grr_plugin::GRpcBroker;
use jsonrpc_core::IoHandler;
use num_derive::FromPrimitive;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::transport::Channel;
use tonic::{Request, Response, Status};

// Copied from: https://github.com/ava-labs/avalanchego/blob/master/snow/engine/common/message.go#L13
#[derive(Debug, FromPrimitive, Clone, Copy)]
pub enum Message {
    PendingTransactions = 0,
}

// Copied from: https://github.com/ava-labs/avalanchego/blob/master/snow/engine/common/http_handler.go#L11
// To get a u32 representation of this, just pick any one variant 'as u32'. For example:
//     lock: Lock::WriteLock as u32
#[derive(Debug, FromPrimitive, Clone, Copy)]
pub enum Lock {
    WriteLock = 0,
    ReadLock,
    NoLock,
}

// https://github.com/ava-labs/avalanchego/blob/master/database/rpcdb/errors.go
#[derive(Debug, FromPrimitive, Clone, Copy)]
pub enum DatabaseError {
    None = 0,
    Closed = 1,
    NotFound = 2,
}

pub struct GHttpServer {
    grpc_broker: Arc<Mutex<GRpcBroker>>,
    io_handler: IoHandler,
}

impl GHttpServer {
    pub fn new_server(
        grpc_broker: Arc<Mutex<GRpcBroker>>,
        io_handler: IoHandler,
    ) -> HttpServer<GHttpServer> {
        HttpServer::new(GHttpServer {
            grpc_broker,
            io_handler,
        })
    }

    // The Rust JSON-RPC request expects this:
    // {
    //      "jsonrpc": "2.0",
    //      "method": "timestampvm.getBlock",
    //      "params":[{
    //          "id":"xqQV1jDnCXDxhfnNT7tDBcXeoH2jC3Hh7Pyv4GXE1z1hfup5K"
    //      }],
    //      "id": 1
    // }
    //
    // However, all the avalanche examples use this:
    //  {
    //      "jsonrpc": "2.0",
    //      "method": "timestampvm.getBlock",
    //      "params":{
    //          "id":"xqQV1jDnCXDxhfnNT7tDBcXeoH2jC3Hh7Pyv4GXE1z1hfup5K"
    //      },
    //      "id": 1
    // }
    //
    // To point out the obvious - our current client expects params to be
    // an array of structs: params: [{ <params go here> }]
    // meanwhile, Avalanche examples expect params to just be a struct:
    // just a struct: params: { <params go here> }
    //
    // This function wraps just-a-struct into an array-of-one-struct
    // This function never fails. For any errors, it returns the original
    // string as it was found. No judgement.
    fn array_wrap_params(request_json: String) -> String {
        let mut parsed_json: Value = match serde_json::from_str(request_json.as_str()) {
            // if parsing failure, return original request
            Err(_) => return request_json,
            Ok(j) => j,
        };

        // wrap it in an array
        match parsed_json.as_object_mut() {
            None => return request_json,
            Some(parsed_request) => {
                if let Some(params) = parsed_request.get("params") {
                    if !params.is_array() {
                        // if params are not an array, make them one...
                        if let Some(removed_params) = parsed_request.remove("params") {
                            parsed_request.insert(
                                "params".to_string(),
                                serde_json::Value::Array(vec![removed_params]),
                            );
                        }
                    }
                }
            }
        }

        parsed_json.to_string()
    }
}

#[tonic::async_trait]
impl ghttp::http_server::Http for GHttpServer {
    async fn handle(&self, req: Request<HttpRequest>) -> Result<Response<HttpResponse>, Status> {
        let http_req = req.into_inner();
        let read_conn_id = http_req
            .request
            .ok_or_else(|| Status::unknown("request was expected to be non-empty"))?
            .body;
        let write_conn_id = http_req
            .response_writer
            .ok_or_else(|| Status::unknown("response_writer was expected to be non-empty"))?
            .id;

        log::info!(
            "read_conn_id: {} write_conn_id: {}",
            read_conn_id,
            write_conn_id
        );

        let read_conn: Channel = self
            .grpc_broker
            .lock()
            .await
            .dial_to_host_service(read_conn_id)
            .await
            .map_err(|e| e.into())
            .map_err(into_status)?;
        let mut reader_client = ReaderClient::new(read_conn);

        let write_conn: Channel = self
            .grpc_broker
            .lock()
            .await
            .dial_to_host_service(write_conn_id)
            .await
            .map_err(|e| e.into())
            .map_err(into_status)?;
        let mut responsewriter_client = WriterClient::new(write_conn);

        let read_response = reader_client
            .read(ReadRequest {
                // Should be isize::MAX, but the length field is i32, so we'll take the max allowable length
                // https://doc.rust-lang.org/stable/reference/types/numeric.html#machine-dependent-integer-types
                length: std::i32::MAX,
            })
            .await?
            .into_inner();

        let body_bytes = match read_response.errored {
            true => match read_response.error.as_str() {
                "EOF" => read_response.read,
                _ => {
                    return Err(Status::internal(format!(
                    "Error occurred when reading the ghttp request body from the read channel: {}",
                    read_response.error
                )))
                }
            },
            false => read_response.read,
        };

        let body_str = String::from_utf8(body_bytes)
            .context("In GHttpClient, error converting bytes from request body into a UTF8 string.")
            .map_err(|e| e.into())
            .map_err(into_status)?;

        let body_str_array_wrapped_params = Self::array_wrap_params(body_str);

        log::info!("In GHttpClient, body: {}", body_str_array_wrapped_params);
        let response = self
            .io_handler
            .handle_request(body_str_array_wrapped_params.as_str())
            .await
            .ok_or_else(|| Status::internal("no response from inner handler"))?;

        log::info!(
            "In GHttpClient, response from inner io_handler: {:?}",
            response
        );
        let written_bytes = responsewriter_client
            .write(WriteRequest {
                headers: vec![],
                payload: response.into_bytes(),
            })
            .await?;

        log::trace!("In GHttpClient, written response bytes {:?}", written_bytes);
        Ok(Response::new(HttpResponse {}))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use assert_json_diff::assert_json_eq;
    use serde_json::json;

    #[test]
    fn test_array_wrap_params_simple() {
        let unwrapped_json_str = json!({
            "jsonrpc": "2.0",
            "method": "timestampvm.getBlock",
            "params":{
                "id":"xqQV1jDnCXDxhfnNT7tDBcXeoH2jC3Hh7Pyv4GXE1z1hfup5K"
            },
            "id": 1
        })
        .to_string();

        let wrapped_json_str = json!({
            "jsonrpc": "2.0",
            "method": "timestampvm.getBlock",
            "params":[{
                "id":"xqQV1jDnCXDxhfnNT7tDBcXeoH2jC3Hh7Pyv4GXE1z1hfup5K"
            }],
            "id": 1
        })
        .to_string();

        let auto_wrapped_json_str = GHttpServer::array_wrap_params(unwrapped_json_str);

        assert_json_eq!(auto_wrapped_json_str, wrapped_json_str);
    }

    #[test]
    fn test_array_wrap_params_primitive_int() {
        let unwrapped_json_str = json!({
            "jsonrpc": "2.0",
            "method": "timestampvm.getBlock",
            "params": 5,
            "id": 1
        })
        .to_string();

        let wrapped_json_str = json!({
            "jsonrpc": "2.0",
            "method": "timestampvm.getBlock",
            "params": [5],
            "id": 1
        })
        .to_string();

        let auto_wrapped_json_str = GHttpServer::array_wrap_params(unwrapped_json_str);

        assert_json_eq!(auto_wrapped_json_str, wrapped_json_str);
    }

    #[test]
    fn test_array_wrap_params_complex() {
        let unwrapped_json_str = json!({
            "jsonrpc": "2.0",
            "method": "timestampvm.getBlock",
            "params":{
                "struct":{
                    "foo": "bar"
                },
                "array": [
                    "one",
                    "two",
                    "three"
                ]
            },
            "id": 1
        })
        .to_string();

        let wrapped_json_str = json!({
            "jsonrpc": "2.0",
            "method": "timestampvm.getBlock",
            "params":[{
                "struct":{
                    "foo": "bar"
                },
                "array": [
                    "one",
                    "two",
                    "three"
                ]
            }],
            "id": 1
        })
        .to_string();

        let auto_wrapped_json_str = GHttpServer::array_wrap_params(unwrapped_json_str);

        assert_json_eq!(auto_wrapped_json_str, wrapped_json_str);
    }
}
