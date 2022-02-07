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

use ghttp::http_server::HttpServer;
use ghttp::{HttpRequest, HttpResponse};
use grr_plugin::GRpcBroker;
use num_derive::FromPrimitive;
use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};
use tonic::transport::Channel;

// Copied from: https://github.com/ava-labs/avalanchego/blob/master/snow/engine/common/message.go#L13
#[derive(Debug, FromPrimitive, Clone, Copy)]
enum Message {
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
}

impl GHttpServer {
    pub fn new_server(grpc_broker: Arc<Mutex<GRpcBroker>>) -> HttpServer<GHttpServer> {
        HttpServer::new(GHttpServer { grpc_broker })
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

        //let channel: Channel = self.grpc_broker.dial_to_host_service(read_conn_id)
        //    .map_err(|e| e.into())?;

        Err(Status::unknown(""))
    }
}
