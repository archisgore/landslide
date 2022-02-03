//NOTE: I really don't understand protobufs. This code is clunky and I appreciate fixes/PRs.
// I've had a distaste for RPC since CORBA and SOAP didn't make it better.
mod block;
mod static_handlers;

use crate::error::LandslideError;
use crate::function;

use super::context::Context;
use super::vm::vm_proto::*;
use tonic::{Request, Response};

use block::{Block, State, Status as BlockStatus, StorageBlock};
use semver::Version;

use super::error::into_status;
use super::log_and_escalate;
use crate::id::Id;
use crate::vm::vm_proto::vm_server::Vm;
use grr_plugin::log_and_escalate_status;
use grr_plugin::JsonRpcBroker;
use grr_plugin::Status;
use std::collections::HashMap;
use time::{Duration, OffsetDateTime};
use tokio::sync::{RwLock, RwLockReadGuard};

const BLOCK_DATA_LEN: usize = 32;

// Copied from: https://github.com/ava-labs/avalanchego/blob/master/snow/engine/common/http_handler.go#L11
// To get a u32 representation of this, just pick any one variant 'as u32'. For example:
//     lock: Lock::WriteLock as u32
pub enum Lock {
    Write,
    Read,
    NoLock,
}

// TimestampVM cannot mutably reference self on all its trait methods.
// Instead it stores an instance of TimestampVmInterior, which is mutable, and can be
// modified by the calls to TimestampVm's VM trait.
struct TimestampVmInterior {
    ctx: Option<Context>,
    version: Version,
    state: Option<State>,
    verified_blocks: HashMap<Id, Block>,
    jsonrpc_broker: JsonRpcBroker,
}

pub struct TimestampVm {
    interior: RwLock<TimestampVmInterior>,
}

impl TimestampVm {
    pub fn new(jsonrpc_broker: JsonRpcBroker) -> Result<TimestampVm, LandslideError> {
        Ok(TimestampVm {
            interior: RwLock::new(TimestampVmInterior {
                ctx: None,
                version: Version::new(1, 2, 1),
                state: None,
                verified_blocks: HashMap::new(),
                jsonrpc_broker,
            }),
        })
    }

    async fn init_genessis(&self, genesis_bytes: &[u8]) -> Result<(), Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());

        let mut writable_interor = self.interior.write().await;
        let state = writable_interor
            .state
            .as_mut()
            .ok_or(LandslideError::StateNotInitialized)
            .map_err(into_status)?;

        if state.is_state_initialized().map_err(into_status)? {
            // State is already initialized - no need to init genessis block
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

    // Verify returns nil iff this block is valid.
    // To be valid, it must be that:
    // b.parent.Timestamp < b.Timestamp <= [local time] + 1 hour
    async fn verify_block(&mut self, block: Block) -> Result<(), LandslideError> {
        let mut writable_interor = self.interior.write().await;
        let state = writable_interor
            .state
            .as_mut()
            .ok_or(LandslideError::StateNotInitialized)?;

        let bid = block.id()?;
        let parent_sb = match state.get_block(&block.parent_id)? {
            Some(pb) => pb,
            None => {
                return Err(LandslideError::NoParentBlock {
                    block_id: bid,
                    parent_block_id: block.parent_id,
                })
            }
        };

        // Ensure [b]'s height comes right after its parent's height
        if parent_sb.block.height + 1 != block.height {
            return Err(LandslideError::ParentBlockHeightUnexpected {
                block_height: block.height,
                parent_block_height: parent_sb.block.height,
            });
        }

        let bts = block.timestamp()?;
        let pbts = parent_sb.block.timestamp()?;
        // Ensure [b]'s timestamp is after its parent's timestamp.
        if block.timestamp()? < parent_sb.block.timestamp()? {
            return Err(LandslideError::Generic(format!("The current block {}'s  timestamp {}, is before the parent block {}'s timestamp {}, which is invalid for a Blockchain.", bid, bts, block.parent_id, pbts)));
        }

        // Ensure [b]'s timestamp is not more than an hour
        // ahead of this node's time
        let now = OffsetDateTime::now_utc();
        let one_hour_from_now = match now.checked_add(Duration::hours(1)) {
            Some(t) => t,
            None => {
                return Err(LandslideError::Generic(
                    "Unable to compute time 1 hour from now.".to_string(),
                ))
            }
        };

        if bts >= one_hour_from_now {
            return Err(LandslideError::Generic(format!("The current block {}'s  timestamp {}, is more than 1 hour in the future compared to this node's time {}", bid, bts, now)));
        }

        // Put that block to verified blocks in memory
        writable_interor.verified_blocks.insert(bid, block);

        Ok(())
    }

    // Accept sets this block's status to Accepted and sets lastAccepted to this
    // block's ID and saves this info to b.vm.DB
    async fn accept(&self, block: Block) -> Result<(), LandslideError> {
        let mut writable_interor = self.interior.write().await;
        let state = writable_interor
            .state
            .as_mut()
            .ok_or(LandslideError::StateNotInitialized)?;

        let sb = StorageBlock {
            block,
            status: BlockStatus::Accepted,
        };

        let block_id = sb.block.id()?;

        // Persist data
        state.put_block(sb)?;

        state.set_last_accepted_block_id(&block_id)?;

        Ok(())
    }

    // Reject sets this block's status to Rejected and saves the status in state
    // Recall that b.vm.DB.Commit() must be called to persist to the DB
    async fn reject(&self, block: Block) -> Result<(), LandslideError> {
        let mut writable_interor = self.interior.write().await;
        let state = writable_interor
            .state
            .as_mut()
            .ok_or(LandslideError::StateNotInitialized)?;

        let sb = StorageBlock {
            block,
            status: BlockStatus::Rejected,
        };

        let _block_id = sb.block.id()?;

        // Persist data
        state.put_block(sb)?;

        Ok(())
    }

    async fn version_on_readable_interior(
        readable_interior: &TimestampVmInterior,
        _request: Request<()>,
    ) -> Result<Response<VersionResponse>, Status> {
        let version = readable_interior.version.to_string();
        log::info!(
            "{}, ({},{}) - responding with version {}",
            function!(),
            file!(),
            line!(),
            version
        );
        Ok(Response::new(VersionResponse {
            version: readable_interior.version.to_string(),
        }))
    }
}

#[tonic::async_trait]
impl Vm for TimestampVm {
    async fn initialize(
        &self,
        request: Request<InitializeRequest>,
    ) -> Result<Response<InitializeResponse>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        let mut writable_interior = self.interior.write().await;

        log::info!("TimestampVm::Initialize Calling Version...");

        let version = log_and_escalate!(
            Self::version_on_readable_interior(&writable_interior, Request::new(())).await
        );

        log::info!("TimestampVm::Initialize obtained VM version: {:?}", version);

        let ir = request.into_inner();
        log::info!(
            "{}, ({},{}) - Request: {:?}",
            function!(),
            file!(),
            line!(),
            ir
        );

        writable_interior.ctx = Some(Context {
            network_id: ir.network_id,
            subnet_id: ir.subnet_id,
            chain_id: ir.chain_id,
            node_id: ir.node_id,

            x_chain_id: ir.x_chain_id,
            avax_asset_id: ir.avax_asset_id,
        });

        log::info!("TimestampVm::Initialize setup context from genesis data");

        log_and_escalate!(self.init_genessis(ir.genesis_bytes.as_ref()).await);

        log::info!("TimestampVm::Initialize genesis initialized");

        let state = writable_interior
            .state
            .as_mut()
            .ok_or(LandslideError::StateNotInitialized)
            .map_err(into_status)?;
        let labid = match log_and_escalate!(state.get_last_accepted_block_id().map_err(into_status))
        {
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

        let sb = match state.get_block(&labid).map_err(into_status)? {
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
        Ok(Response::new(()))
    }

    async fn bootstrapped(&self, _request: Request<()>) -> Result<Response<()>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        Ok(Response::new(()))
    }

    async fn shutdown(&self, _request: Request<()>) -> Result<Response<()>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());

        Ok(Response::new(()))
    }

    async fn create_handlers(
        &self,
        _request: Request<()>,
    ) -> Result<Response<CreateHandlersResponse>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        let mut writable_interor = self.interior.write().await;

        log::debug!(
            "{}, ({},{}) - Creating a new JSON-RPC 2.0 server for handlers...",
            function!(),
            file!(),
            line!()
        );
        let server_id = log_and_escalate_status!(
            writable_interor
                .jsonrpc_broker
                .new_server(static_handlers::new())
                .await
        );
        let vm_static_api_service = Handler {
            prefix: "/rusty_dynamic".to_string(),
            lock_options: Lock::NoLock as u32,
            server: server_id,
        };
        log::debug!(
            "{}, ({},{}) - Created a new JSON-RPC 2.0 server for handlers with server_id: {}",
            function!(),
            file!(),
            line!(),
            server_id
        );

        log::debug!(
            "{}, ({},{}) - called - responding with API service.",
            function!(),
            file!(),
            line!()
        );
        Ok(Response::new(CreateHandlersResponse {
            handlers: vec![vm_static_api_service],
        }))
    }

    // This is the code that we must meet: https://github.com/ava-labs/avalanchego/blob/master/vms/rpcchainvm/vm_client.go#L343
    async fn create_static_handlers(
        &self,
        _request: Request<()>,
    ) -> Result<Response<CreateStaticHandlersResponse>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        let mut writable_interor = self.interior.write().await;

        log::debug!(
            "{}, ({},{}) - Creating a new JSON-RPC 2.0 server for static handlers...",
            function!(),
            file!(),
            line!()
        );
        let server_id = log_and_escalate_status!(
            writable_interor
                .jsonrpc_broker
                .new_server(static_handlers::new())
                .await
        );
        let vm_static_api_service = Handler {
            prefix: "/rusty_static".to_string(),
            lock_options: Lock::NoLock as u32,
            server: server_id,
        };
        log::debug!("{}, ({},{}) - Created a new JSON-RPC 2.0 server for static handlers with server_id: {}", function!(), file!(), line!(), server_id);

        log::debug!(
            "{}, ({},{}) - called - responding with static API service.",
            function!(),
            file!(),
            line!()
        );
        Ok(Response::new(CreateStaticHandlersResponse {
            handlers: vec![vm_static_api_service],
        }))
    }

    async fn connected(&self, _request: Request<ConnectedRequest>) -> Result<Response<()>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        Ok(Response::new(()))
    }

    async fn disconnected(
        &self,
        _request: Request<DisconnectedRequest>,
    ) -> Result<Response<()>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        Ok(Response::new(()))
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
        Ok(Response::new(()))
    }

    async fn health(&self, _request: Request<()>) -> Result<Response<HealthResponse>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        log::info!("TimestampVM: Health endpoint pinged; reporting healthy...");
        Ok(Response::new(HealthResponse {
            details: "All is well.".to_string(),
        }))
    }

    async fn version(&self, request: Request<()>) -> Result<Response<VersionResponse>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        let readable_interior = self.interior.read().await;
        Self::version_on_readable_interior(&readable_interior, request).await
    }

    async fn app_request(&self, _request: Request<AppRequestMsg>) -> Result<Response<()>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        Ok(Response::new(()))
    }

    async fn app_request_failed(
        &self,
        _request: Request<AppRequestFailedMsg>,
    ) -> Result<Response<()>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        Ok(Response::new(()))
    }

    async fn app_response(
        &self,
        _request: Request<AppResponseMsg>,
    ) -> Result<Response<()>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        Ok(Response::new(()))
    }

    async fn app_gossip(&self, _request: Request<AppGossipMsg>) -> Result<Response<()>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        Ok(Response::new(()))
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
        Ok(Response::new(()))
    }

    async fn block_reject(
        &self,
        _request: Request<BlockRejectRequest>,
    ) -> Result<Response<()>, Status> {
        log::info!("{}, ({},{}) - called", function!(), file!(), line!());
        Ok(Response::new(()))
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
