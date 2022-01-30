//NOTE: I really don't understand protobufs. This code is clunky and I appreciate fixes/PRs.
// I've had a distaste for RPC since CORBA and SOAP didn't make it better.
mod block;

use crate::error::LandslideError;
use crate::function;

use super::context::Context;
use super::vm::*;
use tonic::{Request, Response, Status};

use block::State;
use semver::Version;

use std::ops::{Deref, DerefMut};
use tokio::sync::RwLock;

// DRY on accessing a mutable reference to interior state
macro_rules! mutable_interior {
    ($self:ident, $interior:ident) => {
        let mut interior_write_guard = $self.interior.write().await;
        let $interior = interior_write_guard.deref_mut();
    };
}

macro_rules! immutable_interior {
    ($self:ident, $interior:ident) => {
        let interior_read_guard = $self.interior.read().await;
        let $interior = interior_read_guard.deref();
    };
}

macro_rules! err_status {
    ($e:expr, $m:expr) => {
        match $e {
            Ok(si) => si,
            Err(err) => {
                return Err(Status::unknown(format!(
                    "Unknown error when {}: {:?}",
                    $m, err
                )))
            }
        }
    };
}

const BLOCK_DATA_LEN: usize = 32;

// TimestampVM cannot mutably reference self on all its trait methods.
// Instead it stores an instance of TimestampVmInterior, which is mutable, and can be
// modified by the calls to TimestampVm's VM trait.
#[derive(Debug)]
struct TimestampVmInterior {
    ctx: Option<Context>,
    version: Version,
    state: State,
}

#[derive(Debug)]
pub struct TimestampVm {
    interior: RwLock<TimestampVmInterior>,
}

impl TimestampVm {
    pub fn new() -> Result<TimestampVm, LandslideError> {
        Ok(TimestampVm {
            interior: RwLock::new(TimestampVmInterior {
                ctx: None,
                version: Version::new(1, 2, 1),
                state: State::new(sled::open("block_store")?),
            }),
        })
    }

    async fn init_genessis(&self, genesis_bytes: &[u8]) -> Result<(), Status> {
        mutable_interior!(self, interior);

        if err_status!(
            interior.state.is_state_initialized(),
            "checking whether state is initialized"
        ) {
            return Ok(());
        }

        if genesis_bytes.len() != BLOCK_DATA_LEN {
            return Err(Status::unknown(format!(
                "Genesis data byte length {} mismatches the expected block byte length of {}",
                genesis_bytes.len(),
                BLOCK_DATA_LEN
            )));
        }

        Ok(())
    }
}

#[tonic::async_trait]
impl Vm for TimestampVm {
    async fn initialize(
        &self,
        request: Request<InitializeRequest>,
    ) -> Result<Response<InitializeResponse>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());

        mutable_interior!(self, interior);

        let version = match self.version(Request::new(())).await {
            Ok(v) => v,
            Err(e) => {
                return Err(Status::unknown(format!(
                "Unable to initialize the Timestamp VM. Unable to fetch self version. Error: {:?}",
                e
            )))
            }
        };

        log::info!("TimestampVm::Initialize obtained VM version: {:?}", version);

        let ir = request.into_inner();
        interior.ctx = Some(Context {
            network_id: ir.network_id,
            subnet_id: ir.subnet_id,
            chain_id: ir.chain_id,
            node_id: ir.node_id,

            x_chain_id: ir.x_chain_id,
            avax_asset_id: ir.avax_asset_id,
        });

        log::info!("TimestampVm::Initialize setup context from genesis data");

        self.init_genessis(ir.genesis_bytes.as_ref()).await?;

        log::info!("TimestampVm::Initialize genesis initialized");

        let labid = match err_status!(
            interior.state.get_last_accepted_block_id(),
            "obtaining last accepted block id"
        ) {
            Some(l) => l,
            None => {
                return Err(Status::unknown(
                    "No last accepted block id found. This was not expected.",
                ))
            }
        };

        log::info!(
            "TimestampVm::Initialize obtained last accepted block id: {}",
            labid
        );

        let sb = match err_status!(
            interior.state.get_block(&labid),
            format!("getting last accepted block from id {}", labid)
        ) {
            Some(sb) => sb,
            None => {
                return Err(Status::unknown(format!(
                    "No block found for id {}. This was not expected.",
                    labid
                )))
            }
        };

        let u32status = sb.status as u32;

        log::info!(
            "TimestampVm::Initialize obtained last accepted block with status: {}",
            u32status
        );

        Ok(Response::new(InitializeResponse {
            last_accepted_id: Vec::from(labid.as_ref()),
            last_accepted_parent_id: Vec::from(sb.block.parent_id.as_ref()),
            bytes: sb.block.data,
            height: sb.block.height,
            timestamp: sb.block.timestamp,
            status: u32status,
        }))
    }

    async fn bootstrapping(&self, _request: Request<()>) -> Result<Response<()>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        todo!()
    }

    async fn bootstrapped(&self, _request: Request<()>) -> Result<Response<()>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        todo!()
    }

    async fn shutdown(&self, _request: Request<()>) -> Result<Response<()>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        todo!()
    }

    async fn create_handlers(
        &self,
        _request: Request<()>,
    ) -> Result<Response<CreateHandlersResponse>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        todo!()
    }

    async fn create_static_handlers(
        &self,
        _request: Request<()>,
    ) -> Result<Response<CreateStaticHandlersResponse>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        todo!()
    }

    async fn connected(&self, _request: Request<ConnectedRequest>) -> Result<Response<()>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        todo!()
    }

    async fn disconnected(
        &self,
        _request: Request<DisconnectedRequest>,
    ) -> Result<Response<()>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        todo!()
    }

    async fn build_block(
        &self,
        _request: Request<()>,
    ) -> Result<Response<BuildBlockResponse>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        todo!()
    }

    async fn parse_block(
        &self,
        _request: Request<ParseBlockRequest>,
    ) -> Result<Response<ParseBlockResponse>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        todo!()
    }

    async fn get_block(
        &self,
        _request: Request<GetBlockRequest>,
    ) -> Result<Response<GetBlockResponse>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        todo!()
    }

    async fn set_preference(
        &self,
        _request: Request<SetPreferenceRequest>,
    ) -> Result<Response<()>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        todo!()
    }

    async fn health(&self, _request: Request<()>) -> Result<Response<HealthResponse>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        log::info!("TimestampVM: Health endpoint pinged; reporting healthy...");
        Ok(Response::new(HealthResponse {
            details: "All is well.".to_string(),
        }))
    }

    async fn version(&self, _request: Request<()>) -> Result<Response<VersionResponse>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        immutable_interior!(self, interior);

        let version = interior.version.to_string();
        log::info!("{}, ({},{}) - responding with version {}", function!(), file!(), line!(), version);
        Ok(Response::new(VersionResponse {
            version: interior.version.to_string(),
        }))
    }

    async fn app_request(&self, _request: Request<AppRequestMsg>) -> Result<Response<()>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        todo!()
    }

    async fn app_request_failed(
        &self,
        _request: Request<AppRequestFailedMsg>,
    ) -> Result<Response<()>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        todo!()
    }

    async fn app_response(
        &self,
        _request: Request<AppResponseMsg>,
    ) -> Result<Response<()>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        todo!()
    }

    async fn app_gossip(&self, _request: Request<AppGossipMsg>) -> Result<Response<()>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        todo!()
    }

    async fn gather(&self, _request: Request<()>) -> Result<Response<GatherResponse>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        todo!()
    }

    async fn block_verify(
        &self,
        _request: Request<BlockVerifyRequest>,
    ) -> Result<Response<BlockVerifyResponse>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        todo!()
    }

    async fn block_accept(
        &self,
        _request: Request<BlockAcceptRequest>,
    ) -> Result<Response<()>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        todo!()
    }

    async fn block_reject(
        &self,
        _request: Request<BlockRejectRequest>,
    ) -> Result<Response<()>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        todo!()
    }

    async fn get_ancestors(
        &self,
        _request: Request<GetAncestorsRequest>,
    ) -> Result<Response<GetAncestorsResponse>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        todo!()
    }

    async fn batched_parse_block(
        &self,
        _request: Request<BatchedParseBlockRequest>,
    ) -> Result<Response<BatchedParseBlockResponse>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        todo!()
    }
}
