//NOTE: I really don't understand protobufs. This code is clunky and I appreciate fixes/PRs.
// I've had a distaste for RPC since CORBA and SOAP didn't make it better.
mod state;
mod static_handlers;

use crate::error::LandslideError;

use super::context::Context;
use super::proto::vm_proto::*;
use std::collections::BTreeMap;
use tonic::{Request, Response};

use semver::Version;
use state::{Block, State, Status as BlockStatus, StorageBlock, BLOCK_DATA_LEN};

use super::error::into_status;
use crate::id::{Id, ZERO_ID};
use crate::proto::vm_proto::vm_server::Vm;
use anyhow::{anyhow, Context as AnyhowContext, Result};
use grr_plugin::JsonRpcBroker;
use grr_plugin::Status;
use std::collections::HashMap;
use time::{Duration, OffsetDateTime};
use tokio::sync::RwLock;
use tonic::transport::Channel;

use super::proto::appsender::app_sender_client::*;

use super::proto::rpcdb::database_client::*;

use super::proto::messenger::messenger_client::*;

use super::proto::gsubnetlookup::subnet_lookup_client::*;

use super::proto::gsharedmemory::shared_memory_client::*;

use super::proto::gkeystore::keystore_client::*;

use super::proto::galiasreader::alias_reader_client::*;

const LOG_PREFIX: &str = "TimestampVm:: ";

// Copied from: https://github.com/ava-labs/avalanchego/blob/master/snow/engine/common/http_handler.go#L11
// To get a u32 representation of this, just pick any one variant 'as u32'. For example:
//     lock: Lock::WriteLock as u32
pub enum Lock {
    Write,
    Read,
    None,
}

// TimestampVM cannot mutably reference self on all its trait methods.
// Instead it stores an instance of TimestampVmInterior, which is mutable, and can be
// modified by the calls to TimestampVm's VM trait.
struct TimestampVmInterior {
    ctx: Option<Context>,
    version: Version,
    jsonrpc_broker: JsonRpcBroker,

    // These get initialized during the Initialize RPC call.
    state: Option<State>,
    versioned_db_clients: Option<BTreeMap<Version, DatabaseClient<Channel>>>,
    engine_client: Option<MessengerClient<Channel>>,
    keystore_client: Option<KeystoreClient<Channel>>,
    shared_memory_client: Option<SharedMemoryClient<Channel>>,
    bc_lookup_client: Option<AliasReaderClient<Channel>>,
    sn_lookup_client: Option<SubnetLookupClient<Channel>>,
    appsender_client: Option<AppSenderClient<Channel>>,

    // These are used throughout the function
    verified_blocks: HashMap<Id, Block>,
    preferred_block_id: Option<Id>,
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
                jsonrpc_broker,

                state: None,
                versioned_db_clients: None,
                engine_client: None,
                keystore_client: None,
                shared_memory_client: None,
                bc_lookup_client: None,
                sn_lookup_client: None,
                appsender_client: None,

                verified_blocks: HashMap::new(),
                preferred_block_id: None,
            }),
        })
    }

    async fn accept_block(
        writable_interior: &mut TimestampVmInterior,
        mut sb: StorageBlock,
    ) -> Result<(), LandslideError> {
        let state = writable_interior
            .state
            .as_mut()
            .ok_or(LandslideError::StateNotInitialized)
            .map_err(into_status)?;

        sb.status = BlockStatus::Accepted;
        let bid = sb.block.id()?;
        log::info!("Accepting block with id: {}", bid);

        state.put_block(sb).await?;
        log::info!("Put accepted block into database with id: {}", bid);

        state.set_last_accepted_block_id(&bid).await?;
        log::info!("Setting last accepted block id in database to: {}", bid);

        writable_interior.verified_blocks.remove(&bid);
        log::info!(
            "Removing from verified blocks, since it is now accepted, the block id: {}",
            bid
        );

        Ok(())
    }

    async fn init_genesis(
        writable_interior: &mut TimestampVmInterior,
        genesis_bytes: &[u8],
    ) -> Result<(), LandslideError> {
        log::trace!("initialize called");

        let state = writable_interior
            .state
            .as_mut()
            .ok_or(LandslideError::StateNotInitialized)
            .map_err(into_status)?;

        if state.is_state_initialized().await? {
            // State is already initialized - no need to init genesis block
            log::info!("state is already initialized. No further work to do.");
            return Ok(());
        }

        if genesis_bytes.len() > BLOCK_DATA_LEN {
            return Err(LandslideError::Other(anyhow!(
                "Genesis data byte length {} is greater than the expected block byte length of {}. Genesis bytes: {:#?} as a string: {}",
                genesis_bytes.len(),
                BLOCK_DATA_LEN,
                genesis_bytes,
                String::from_utf8(Vec::from(genesis_bytes)).unwrap(),
            )));
        }
        let mut padded_genesis_data = Vec::with_capacity(BLOCK_DATA_LEN);
        padded_genesis_data.extend_from_slice(genesis_bytes);
        // resize to capacity with 0 filler bytes
        padded_genesis_data.resize(BLOCK_DATA_LEN, 0);

        log::info!(
            "Genesis block created with length {} by padding up from data length {}",
            padded_genesis_data.len(),
            genesis_bytes.len()
        );
        let genesis_storage_block = StorageBlock::new(
            ZERO_ID,
            0,
            padded_genesis_data,
            OffsetDateTime::from_unix_timestamp(0)?,
        )?;

        let genesis_block_id = genesis_storage_block.block.id()?;

        log::info!(
            "Genesis storage block created with Id: {}",
            genesis_block_id
        );
        state.put_block(genesis_storage_block.clone()).await?;
        log::info!(
            "Genesis storage block with Id {} put in database successfully.",
            genesis_block_id
        );
        Self::accept_block(writable_interior, genesis_storage_block).await?;
        log::info!(
            "Genesis storage block with Id {} was accepted by this node.",
            genesis_block_id
        );

        // reacquire state since we need to release writable_interior to pass into accept_block
        let state = writable_interior
            .state
            .as_mut()
            .ok_or(LandslideError::StateNotInitialized)
            .map_err(into_status)?;
        state.set_state_initialized().await?;
        log::info!("State set to initialized, so it won't hapen again.");

        Ok(())
    }

    async fn set_preference(writable_interior: &mut TimestampVmInterior, preferred_block_id: Id) {
        writable_interior.preferred_block_id = Some(preferred_block_id)
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
        let parent_sb = state.get_block(block.parent_id.as_ref()).await?.ok_or_else(||
            LandslideError::Other(anyhow!("TimestampVm::verify_block - Parent Block ID {} was not found in the database for Block being verified with Id {}", block.parent_id, bid)))?;

        // Ensure [b]'s height comes right after its parent's height
        if parent_sb.block.height + 1 != block.height {
            return Err(LandslideError::ParentBlockHeightUnexpected {
                block_height: block.height,
                parent_block_height: parent_sb.block.height,
            });
        }

        let bts = block.timestamp_as_offsetdatetime()?;
        let pbts = parent_sb.block.timestamp_as_offsetdatetime()?;
        // Ensure [b]'s timestamp is after its parent's timestamp.
        if bts < pbts {
            return Err(LandslideError::Other(anyhow!("The current block {}'s  timestamp {}, is before the parent block {}'s timestamp {}, which is invalid for a Blockchain.", bid, bts, block.parent_id, pbts)));
        }

        // Ensure [b]'s timestamp is not more than an hour
        // ahead of this node's time
        let now = OffsetDateTime::now_utc();
        let one_hour_from_now = match now.checked_add(Duration::hours(1)) {
            Some(t) => t,
            None => {
                return Err(LandslideError::Other(anyhow!(
                    "Unable to compute time 1 hour from now."
                )))
            }
        };

        if bts >= one_hour_from_now {
            return Err(LandslideError::Other(anyhow!("The current block {}'s  timestamp {}, is more than 1 hour in the future compared to this node's time {}", bid, bts, now)));
        }

        // Put that block to verified blocks in memory
        writable_interor.verified_blocks.insert(bid, block);

        Ok(())
    }

    // Reject sets this block's status to Rejected and saves the status in state
    // Recall that b.vm.DB.Commit() must be called to persist to the DB
    async fn reject_block(&self, block: Block) -> Result<(), LandslideError> {
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
        state.put_block(sb).await?;

        Ok(())
    }

    async fn version_on_readable_interior(
        readable_interior: &TimestampVmInterior,
        _request: Request<()>,
    ) -> Result<Response<VersionResponse>, Status> {
        let version = readable_interior.version.to_string();
        log::info!("responding with version {}", version);
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
        log::info!("{}Initialize called", LOG_PREFIX);
        let mut writable_interior = self.interior.write().await;

        log::info!("{}Initialize Calling Version...", LOG_PREFIX);

        let version =
            Self::version_on_readable_interior(&writable_interior, Request::new(())).await
            .context("Failed calling version on the mutable interior (but passed immutably) of the TimestampVm")
            .map_err(|e| e.into())
            .map_err(into_status)?;

        log::info!(
            "{}Initialize obtained VM version: {:?}",
            LOG_PREFIX,
            version
        );

        let ir = request.into_inner();
        log::trace!("{}, Full Request: {:?}", LOG_PREFIX, ir,);

        writable_interior.ctx = Some(Context {
            network_id: ir.network_id,
            subnet_id: ir.subnet_id,
            chain_id: ir.chain_id,
            node_id: ir.node_id,

            x_chain_id: ir.x_chain_id,
            avax_asset_id: ir.avax_asset_id,
        });
        log::info!("{}Initialize - setup context from genesis data", LOG_PREFIX);

        let mut versioned_db_clients: BTreeMap<Version, DatabaseClient<Channel>> = BTreeMap::new();
        for db_server in ir.db_servers.iter() {
            let ver_without_v = db_server.version.trim_start_matches('v');
            let version = Version::parse(ver_without_v)
                .with_context(|| format!("In initialize, failed to parse the semver::Version for a VersionedDatabase obtained from the host/client. Version provided by server: {}, with the leading 'v' removed: {}", db_server.version, ver_without_v))
                .map_err(|e| e.into())
                .map_err(into_status)?;

            let conn = writable_interior
                .jsonrpc_broker
                .dial_to_host_service(db_server.db_server)
                .await
                .with_context(|| {
                    format!(
                        "Failed to dial a connection to the VersionedDatabase server {}",
                        db_server.db_server
                    )
                })
                .map_err(|e| e.into())
                .map_err(into_status)?;

            let db_client = DatabaseClient::new(conn);
            versioned_db_clients.insert(version, db_client);
            log::info!(
                "{}Initialize - initialized versioned db client for server: {:?}",
                LOG_PREFIX,
                db_server
            );
        }
        writable_interior.versioned_db_clients = Some(versioned_db_clients);
        log::info!(
            "{}Initialize - initialized all versioned db clients",
            LOG_PREFIX
        );

        let conn = writable_interior
            .jsonrpc_broker
            .dial_to_host_service(ir.engine_server)
            .await
            .with_context(|| {
                format!(
                    "Failed to dial a connection to the engine_server {}",
                    ir.engine_server
                )
            })
            .map_err(|e| e.into())
            .map_err(into_status)?;
        writable_interior.engine_client = Some(MessengerClient::new(conn));
        log::info!(
            "{}Initialize - initialized messenger (engine server) client",
            LOG_PREFIX
        );

        let conn = writable_interior
            .jsonrpc_broker
            .dial_to_host_service(ir.keystore_server)
            .await
            .with_context(|| {
                format!(
                    "Failed to dial a connection to the keystore_server {}",
                    ir.keystore_server
                )
            })
            .map_err(|e| e.into())
            .map_err(into_status)?;
        writable_interior.keystore_client = Some(KeystoreClient::new(conn));
        log::info!("{}Initialize - initialized keystore client", LOG_PREFIX);

        let conn = writable_interior
            .jsonrpc_broker
            .dial_to_host_service(ir.shared_memory_server)
            .await
            .with_context(|| {
                format!(
                    "Failed to dial a connection to the shared_memory_server {}",
                    ir.shared_memory_server
                )
            })
            .map_err(|e| e.into())
            .map_err(into_status)?;
        writable_interior.shared_memory_client = Some(SharedMemoryClient::new(conn));
        log::info!(
            "{}Initialize - initialized shared memory client",
            LOG_PREFIX
        );

        let conn = writable_interior
            .jsonrpc_broker
            .dial_to_host_service(ir.bc_lookup_server)
            .await
            .with_context(|| {
                format!(
                    "Failed to dial a connection to the bc_lookup_server {}",
                    ir.bc_lookup_server
                )
            })
            .map_err(|e| e.into())
            .map_err(into_status)?;
        writable_interior.bc_lookup_client = Some(AliasReaderClient::new(conn));
        log::info!("{}Initialize - initialized alias reader client", LOG_PREFIX);

        let conn = writable_interior
            .jsonrpc_broker
            .dial_to_host_service(ir.sn_lookup_server)
            .await
            .with_context(|| {
                format!(
                    "Failed to dial a connection to the sn_lookup_server {}",
                    ir.sn_lookup_server
                )
            })
            .map_err(|e| e.into())
            .map_err(into_status)?;
        writable_interior.sn_lookup_client = Some(SubnetLookupClient::new(conn));
        log::info!(
            "{}Initialize - initialized subnet lookup client",
            LOG_PREFIX
        );

        let conn = writable_interior
            .jsonrpc_broker
            .dial_to_host_service(ir.app_sender_server)
            .await
            .with_context(|| {
                format!(
                    "Failed to dial a connection to the app_sender_server {}",
                    ir.app_sender_server
                )
            })
            .map_err(|e| e.into())
            .map_err(into_status)?;
        writable_interior.appsender_client = Some(AppSenderClient::new(conn));
        log::info!("{}Initialize - initialized app sender client", LOG_PREFIX);

        if let Some(versioned_db_clients) = writable_interior.versioned_db_clients.as_ref() {
            if versioned_db_clients.is_empty() {
                return Err(Status::unknown("zero versioned_db_clients were found. Unable to proceed without a versioned database."));
            }
            if let Some(db_client) = versioned_db_clients.values().rev().next() {
                let state = State::new(db_client.clone());
                writable_interior.state = Some(state);
            } else {
                return Err(Status::unknown("database client not found, when length was verified to be > 0 a little earlier."));
            }
        } else {
            return Err(Status::unknown("versioned_db_clients was None, when it was just set in this same method a little bit before."));
        }

        TimestampVm::init_genesis(&mut writable_interior, ir.genesis_bytes.as_ref())
            .await
            .context("Failed to initialize genesis block.")
            .map_err(|e| e.into())
            .map_err(into_status)?;

        log::info!("TimestampVm::Initialize genesis initialized");

        let state = writable_interior
            .state
            .as_mut()
            .ok_or(LandslideError::StateNotInitialized)
            .map_err(into_status)?;
        let labid = state
            .get_last_accepted_block_id()
            .await
            .context("Failed to get last accepted block id")
            .map_err(|e| e.into())
            .map_err(into_status)?
            .ok_or_else(||Status::unknown("TimestampVm::initialize - unable to find last accepted block id in the database. This is unusual since the init_genesis() call made within this function a bit earlier, should have initialized the genesis block at least.".to_string()))?;

        log::info!(
            "TimestampVm::Initialize obtained last accepted block id: {}",
            labid
        );

        let sb = state.get_block(labid.as_ref()).await
        .with_context(|| format!("Failed to get Block from database with id {}", labid))
        .map_err(|e| e.into())
        .map_err(into_status)?
        .ok_or_else(||Status::unknown(format!("The storage block with Id {} was not found in the database, which is unusual considering this id was obtained from the database as the last accepted block's id.", labid)))?;

        let u32status = sb.status as u32;

        log::info!(
            "TimestampVm::Initialize obtained last accepted block with status: {:?}(u32 value: {})",
            sb.status,
            u32status
        );

        Self::set_preference(&mut writable_interior, labid.clone()).await;

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
        log::info!("bootstrapping called");
        Ok(Response::new(()))
    }

    async fn bootstrapped(&self, _request: Request<()>) -> Result<Response<()>, Status> {
        log::info!("bootstrapped called");
        Ok(Response::new(()))
    }

    async fn shutdown(&self, _request: Request<()>) -> Result<Response<()>, Status> {
        log::info!("shutdown called");

        Ok(Response::new(()))
    }

    async fn create_handlers(
        &self,
        _request: Request<()>,
    ) -> Result<Response<CreateHandlersResponse>, Status> {
        log::info!("create_handlers called");
        let mut writable_interor = self.interior.write().await;

        log::debug!("Creating a new JSON-RPC 2.0 server for handlers...",);
        let server_id = writable_interor
            .jsonrpc_broker
            .new_server(static_handlers::new())
            .await
            .context("Unable to create a new JSON-RPC 2.0 server for handlers")
            .map_err(|e| e.into())
            .map_err(into_status)?;

        let vm_static_api_service = Handler {
            prefix: "".to_string(),
            lock_options: Lock::None as u32,
            server: server_id,
        };
        log::debug!(
            "Created a new JSON-RPC 2.0 server for handlers with server_id: {}",
            server_id
        );

        log::debug!("responding with API service.",);
        Ok(Response::new(CreateHandlersResponse {
            handlers: vec![vm_static_api_service],
        }))
    }

    // This is the code that we must meet: https://github.com/ava-labs/avalanchego/blob/master/vms/rpcchainvm/vm_client.go#L343
    async fn create_static_handlers(
        &self,
        _request: Request<()>,
    ) -> Result<Response<CreateStaticHandlersResponse>, Status> {
        log::info!("create_static_handlers called");
        let mut writable_interor = self.interior.write().await;

        log::debug!("Creating a new JSON-RPC 2.0 server for static handlers...",);
        let server_id = writable_interor
            .jsonrpc_broker
            .new_server(static_handlers::new())
            .await
            .context("Unable to create a new JSON-RPC 2.0 server for handlers")
            .map_err(|e| e.into())
            .map_err(into_status)?;

        let vm_static_api_service = Handler {
            prefix: "".to_string(),
            lock_options: Lock::None as u32,
            server: server_id,
        };
        log::debug!(
            "Created a new JSON-RPC 2.0 server for static handlers with server_id: {}",
            server_id
        );

        log::debug!("responding with static API service.",);
        Ok(Response::new(CreateStaticHandlersResponse {
            handlers: vec![vm_static_api_service],
        }))
    }

    async fn connected(&self, _request: Request<ConnectedRequest>) -> Result<Response<()>, Status> {
        log::info!("connected called");
        Ok(Response::new(()))
    }

    async fn disconnected(
        &self,
        _request: Request<DisconnectedRequest>,
    ) -> Result<Response<()>, Status> {
        log::info!("disconnected called");
        Ok(Response::new(()))
    }

    async fn build_block(
        &self,
        _request: Request<()>,
    ) -> Result<Response<BuildBlockResponse>, Status> {
        log::info!("build_block called");
        todo!()
    }

    async fn parse_block(
        &self,
        _request: Request<ParseBlockRequest>,
    ) -> Result<Response<ParseBlockResponse>, Status> {
        log::info!("parse_block called");
        todo!()
    }

    async fn get_block(
        &self,
        _request: Request<GetBlockRequest>,
    ) -> Result<Response<GetBlockResponse>, Status> {
        log::info!("get_block called");
        todo!()
    }

    async fn set_preference(
        &self,
        _request: Request<SetPreferenceRequest>,
    ) -> Result<Response<()>, Status> {
        log::info!("set_preference called");
        Ok(Response::new(()))
    }

    async fn health(&self, _request: Request<()>) -> Result<Response<HealthResponse>, Status> {
        log::info!("health called");
        log::info!("TimestampVM: Health endpoint pinged; reporting healthy...");
        Ok(Response::new(HealthResponse {
            details: "All is well.".to_string(),
        }))
    }

    async fn version(&self, request: Request<()>) -> Result<Response<VersionResponse>, Status> {
        log::info!("version called");
        let readable_interior = self.interior.read().await;
        Self::version_on_readable_interior(&readable_interior, request).await
    }

    async fn app_request(&self, _request: Request<AppRequestMsg>) -> Result<Response<()>, Status> {
        log::info!("app_request called");
        Ok(Response::new(()))
    }

    async fn app_request_failed(
        &self,
        _request: Request<AppRequestFailedMsg>,
    ) -> Result<Response<()>, Status> {
        log::info!("app_request_failed called");
        Ok(Response::new(()))
    }

    async fn app_response(
        &self,
        _request: Request<AppResponseMsg>,
    ) -> Result<Response<()>, Status> {
        log::info!("app_response called");
        Ok(Response::new(()))
    }

    async fn app_gossip(&self, _request: Request<AppGossipMsg>) -> Result<Response<()>, Status> {
        log::info!("app_gossip called");
        Ok(Response::new(()))
    }

    async fn gather(&self, _request: Request<()>) -> Result<Response<GatherResponse>, Status> {
        log::info!("gather called");
        todo!()
    }

    async fn block_verify(
        &self,
        _request: Request<BlockVerifyRequest>,
    ) -> Result<Response<BlockVerifyResponse>, Status> {
        log::info!("block_verify called");
        todo!()
    }

    async fn block_accept(
        &self,
        _request: Request<BlockAcceptRequest>,
    ) -> Result<Response<()>, Status> {
        log::info!("block_accept called");
        Ok(Response::new(()))
    }

    async fn block_reject(
        &self,
        _request: Request<BlockRejectRequest>,
    ) -> Result<Response<()>, Status> {
        log::info!("block_reject called");
        Ok(Response::new(()))
    }

    async fn get_ancestors(
        &self,
        _request: Request<GetAncestorsRequest>,
    ) -> Result<Response<GetAncestorsResponse>, Status> {
        log::info!("get_ancestors called");
        todo!()
    }

    async fn batched_parse_block(
        &self,
        _request: Request<BatchedParseBlockRequest>,
    ) -> Result<Response<BatchedParseBlockResponse>, Status> {
        log::info!("batched_parse_block called");
        todo!()
    }
}
