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

pub use vm_proto::vm_server::{Vm, VmServer};
pub use vm_proto::{
    AppGossipMsg, AppRequestFailedMsg, AppRequestMsg, AppResponseMsg, BatchedParseBlockRequest,
    BatchedParseBlockResponse, BlockAcceptRequest, BlockRejectRequest, BlockVerifyRequest,
    BlockVerifyResponse, BuildBlockResponse, ConnectedRequest, CreateHandlersResponse,
    CreateStaticHandlersResponse, DisconnectedRequest, GatherResponse, GetAncestorsRequest,
    GetAncestorsResponse, GetBlockRequest, GetBlockResponse, HealthResponse, InitializeRequest,
    InitializeResponse, ParseBlockRequest, ParseBlockResponse, SetPreferenceRequest,
    VersionResponse,
};
