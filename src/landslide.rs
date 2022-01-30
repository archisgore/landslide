//NOTE: I really don't understand protobufs. This code is clunky and I appreciate fixes/PRs.
// I've had a distaste for RPC since CORBA and SOAP didn't make it better.

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

// Allow others to use these types
pub use tonic::{transport::Server, Request, Response, Status};
pub use vm_proto::vm_server::{VmServer};

use vm_proto::vm_server::{Vm};
use vm_proto::{
    AppGossipMsg, AppRequestFailedMsg, AppRequestMsg, AppResponseMsg, BatchedParseBlockRequest,
    BatchedParseBlockResponse, BlockAcceptRequest, BlockRejectRequest, BlockVerifyRequest,
    BlockVerifyResponse, BuildBlockResponse, ConnectedRequest, CreateHandlersResponse,
    CreateStaticHandlersResponse, DisconnectedRequest, GatherResponse, GetAncestorsRequest,
    GetAncestorsResponse, GetBlockRequest, GetBlockResponse, HealthResponse, InitializeRequest,
    InitializeResponse, ParseBlockRequest, ParseBlockResponse, SetPreferenceRequest,
    VersionResponse,
};

#[derive(Debug, Default)]
pub struct Landslide {}

#[tonic::async_trait]
impl Vm for Landslide {
    async fn initialize(
        &self,
        request: Request<InitializeRequest>,
    ) -> Result<Response<InitializeResponse>, Status> {
        todo!()
    }

    async fn bootstrapping(&self, request: Request<()>) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn bootstrapped(&self, request: Request<()>) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn shutdown(&self, request: Request<()>) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn create_handlers(
        &self,
        request: Request<()>,
    ) -> Result<Response<CreateHandlersResponse>, Status> {
        todo!()
    }

    async fn create_static_handlers(
        &self,
        request: Request<()>,
    ) -> Result<Response<CreateStaticHandlersResponse>, Status> {
        todo!()
    }

    async fn connected(&self, request: Request<ConnectedRequest>) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn disconnected(
        &self,
        request: Request<DisconnectedRequest>,
    ) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn build_block(
        &self,
        request: Request<()>,
    ) -> Result<Response<BuildBlockResponse>, Status> {
        todo!()
    }

    async fn parse_block(
        &self,
        request: Request<ParseBlockRequest>,
    ) -> Result<Response<ParseBlockResponse>, Status> {
        todo!()
    }

    async fn get_block(
        &self,
        request: Request<GetBlockRequest>,
    ) -> Result<Response<GetBlockResponse>, Status> {
        todo!()
    }

    async fn set_preference(
        &self,
        request: Request<SetPreferenceRequest>,
    ) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn health(&self, request: Request<()>) -> Result<Response<HealthResponse>, Status> {
        todo!()
    }

    async fn version(&self, request: Request<()>) -> Result<Response<VersionResponse>, Status> {
        todo!()
    }

    async fn app_request(&self, request: Request<AppRequestMsg>) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn app_request_failed(
        &self,
        request: Request<AppRequestFailedMsg>,
    ) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn app_response(&self, request: Request<AppResponseMsg>) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn app_gossip(&self, request: Request<AppGossipMsg>) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn gather(&self, request: Request<()>) -> Result<Response<GatherResponse>, Status> {
        todo!()
    }

    async fn block_verify(
        &self,
        request: Request<BlockVerifyRequest>,
    ) -> Result<Response<BlockVerifyResponse>, Status> {
        todo!()
    }

    async fn block_accept(
        &self,
        request: Request<BlockAcceptRequest>,
    ) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn block_reject(
        &self,
        request: Request<BlockRejectRequest>,
    ) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn get_ancestors(
        &self,
        request: Request<GetAncestorsRequest>,
    ) -> Result<Response<GetAncestorsResponse>, Status> {
        todo!()
    }

    async fn batched_parse_block(
        &self,
        request: Request<BatchedParseBlockRequest>,
    ) -> Result<Response<BatchedParseBlockResponse>, Status> {
        todo!()
    }
}