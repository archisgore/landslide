//NOTE: I really don't understand protobufs. This code is clunky and I appreciate fixes/PRs.
// I've had a distaste for RPC since CORBA and SOAP didn't make it better.
mod state;
mod block;

use super::context::Context;
use super::vm::*;
use semver::Version;
use sled::Db;

use std::ops::{Deref, DerefMut};
use tokio::sync::RwLock;

// DRY on accessing a mutable reference to interior state
macro_rules! mutable_interior {
    ($self:ident, $interior:ident) => {
        let mut interior_write_guard = $self.interior.write().await;
        let mut $interior = interior_write_guard.deref_mut();
    };
}

macro_rules! immutable_interior {
    ($self:ident, $interior:ident) => {
        let interior_read_guard = $self.interior.read().await;
        let $interior = interior_read_guard.deref();
    };
}

const SINGLETON_STATE_PREFIX: &str = "singleton";
const BLOCK_STATE_PREFIX: &str = "block";

// TimestampVM cannot mutably reference self on all its trait methods.
// Instead it stores an instance of TimestampVmInterior, which is mutable, and can be
// modified by the calls to TimestampVm's VM trait.
#[derive(Debug)]
struct TimestampVmInterior {
    ctx: Option<Context>,
    version: Version,
    state: Option<State>,
}

#[derive(Debug)]
pub struct TimestampVm {
    interior: RwLock<TimestampVmInterior>,
}

impl TimestampVm {
    pub fn new() -> TimestampVm {
        TimestampVm {
            interior: RwLock::new(TimestampVmInterior {
                ctx: None,
                version: Version::new(0, 1, 0),
            }),
        }
    }

    fn init_genessis(_genesis_bytes: &[u8]) -> Result<(), Status> {

        Ok(())
    }
}

#[tonic::async_trait]
impl Vm for TimestampVm {
    async fn initialize(
        &self,
        request: Request<InitializeRequest>,
    ) -> Result<Response<InitializeResponse>, Status> {
        mutable_interior!(self, interior);

        let _version = match self.version(Request::new(())).await {
            Ok(v) => v,
            Err(e) => return Err(Status::unknown(format!("Unable to initialize the Timestamp VM. Unable to fetch self version. Error: {:?}", e))),
        };

        let ir = request.into_inner();
        interior.ctx = Some(Context::from(ir));

        Err(Status::ok("Initialized Timestamp VM"))
    }

    async fn bootstrapping(&self, _request: Request<()>) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn bootstrapped(&self, _request: Request<()>) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn shutdown(&self, _request: Request<()>) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn create_handlers(
        &self,
        _request: Request<()>,
    ) -> Result<Response<CreateHandlersResponse>, Status> {
        todo!()
    }

    async fn create_static_handlers(
        &self,
        _request: Request<()>,
    ) -> Result<Response<CreateStaticHandlersResponse>, Status> {
        todo!()
    }

    async fn connected(&self, _request: Request<ConnectedRequest>) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn disconnected(
        &self,
        _request: Request<DisconnectedRequest>,
    ) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn build_block(
        &self,
        _request: Request<()>,
    ) -> Result<Response<BuildBlockResponse>, Status> {
        todo!()
    }

    async fn parse_block(
        &self,
        _request: Request<ParseBlockRequest>,
    ) -> Result<Response<ParseBlockResponse>, Status> {
        todo!()
    }

    async fn get_block(
        &self,
        _request: Request<GetBlockRequest>,
    ) -> Result<Response<GetBlockResponse>, Status> {
        todo!()
    }

    async fn set_preference(
        &self,
        _request: Request<SetPreferenceRequest>,
    ) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn health(&self, _request: Request<()>) -> Result<Response<HealthResponse>, Status> {
        todo!()
    }

    async fn version(&self, _request: Request<()>) -> Result<Response<VersionResponse>, Status> {
        immutable_interior!(self, interior);
        Ok(Response::new(VersionResponse {
            version: interior.version.to_string(),
        }))
    }

    async fn app_request(&self, _request: Request<AppRequestMsg>) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn app_request_failed(
        &self,
        _request: Request<AppRequestFailedMsg>,
    ) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn app_response(
        &self,
        _request: Request<AppResponseMsg>,
    ) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn app_gossip(&self, _request: Request<AppGossipMsg>) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn gather(&self, _request: Request<()>) -> Result<Response<GatherResponse>, Status> {
        todo!()
    }

    async fn block_verify(
        &self,
        _request: Request<BlockVerifyRequest>,
    ) -> Result<Response<BlockVerifyResponse>, Status> {
        todo!()
    }

    async fn block_accept(
        &self,
        _request: Request<BlockAcceptRequest>,
    ) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn block_reject(
        &self,
        _request: Request<BlockRejectRequest>,
    ) -> Result<Response<()>, Status> {
        todo!()
    }

    async fn get_ancestors(
        &self,
        _request: Request<GetAncestorsRequest>,
    ) -> Result<Response<GetAncestorsResponse>, Status> {
        todo!()
    }

    async fn batched_parse_block(
        &self,
        _request: Request<BatchedParseBlockRequest>,
    ) -> Result<Response<BatchedParseBlockResponse>, Status> {
        todo!()
    }
}
