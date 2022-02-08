//NOTE: I really don't understand protobufs. This code is clunky and I appreciate fixes/PRs.
// I've had a distaste for RPC since CORBA and SOAP didn't make it better.
mod handlers;
mod state;
mod static_handlers;

use crate::error::LandslideError;

use super::context::Context;
use super::proto;
use super::proto::vm_proto::*;
use semver::Version;
use state::{Block, State, Status as BlockStatus, BLOCK_DATA_LEN};
use std::collections::BTreeMap;
use tonic::{Request, Response};

use super::error::into_status;
use crate::id::{Id, ROOT_PARENT_ID};
use crate::proto::vm_proto::vm_server::Vm;
use anyhow::{anyhow, Context as AnyhowContext, Result};
use grr_plugin::GRpcBroker;
use grr_plugin::ServiceId;
use grr_plugin::Status;
use hyper::{Body, Request as HyperRequest, Response as HyperResponse};
use std::collections::HashMap;
use std::error::Error as StdError;
use std::sync::Arc;
use time::{Duration, OffsetDateTime};
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tonic::body::BoxBody;
use tonic::transport::Channel;
use tonic::transport::NamedService;
use tower::Service;

use super::proto::appsender::app_sender_client::*;
use super::proto::Message;

use super::proto::rpcdb::database_client::*;

use super::proto::messenger::messenger_client::*;
use super::proto::messenger::NotifyRequest;

use super::proto::gsubnetlookup::subnet_lookup_client::*;

use super::proto::gsharedmemory::shared_memory_client::*;

use super::proto::gkeystore::keystore_client::*;

use super::proto::galiasreader::alias_reader_client::*;

// Copied from: https://github.com/ava-labs/avalanchego/blob/master/snow/engine/common/http_handler.go#L11
// To get a u32 representation of this, just pick any one variant 'as u32'. For example:
//     lock: Lock::WriteLock as u32
pub enum Lock {
    #[allow(dead_code)]
    Write = 0,
    #[allow(dead_code)]
    Read,

    None,
}

// TimestampVM cannot mutably reference self on all its trait methods.
// Instead it stores an instance of TimestampVmInterior, which is mutable, and can be
// modified by the calls to TimestampVm's VM trait.
pub struct TimestampVmInterior {
    ctx: Option<Context>,
    version: Version,
    grpc_broker: Arc<Mutex<GRpcBroker>>,

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

    // blocks ready to propose
    mem_pool: Vec<[u8; BLOCK_DATA_LEN]>,
}

impl TimestampVmInterior {
    async fn mut_state_status(&mut self) -> Result<&mut State, Status> {
        self.mut_state().await.map_err(into_status)
    }

    async fn mut_state(&mut self) -> Result<&mut State, LandslideError> {
        self.state
            .as_mut()
            .ok_or(LandslideError::StateNotInitialized)
    }

    async fn init_genesis(&mut self, genesis_bytes: &[u8]) -> Result<(), LandslideError> {
        log::trace!("initialize genesis called");

        let state = self.mut_state().await?;

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

        let mut padded_genesis_vec = Vec::from(genesis_bytes);
        padded_genesis_vec.resize(BLOCK_DATA_LEN, 0);

        let padded_genesis_data: [u8; BLOCK_DATA_LEN] = padded_genesis_vec.as_slice().try_into()?;

        log::info!(
            "Genesis block created with length {} by padding up from data length {}",
            padded_genesis_data.len(),
            genesis_bytes.len()
        );
        let mut genesis_block = Block::new(
            ROOT_PARENT_ID,
            0,
            padded_genesis_data,
            OffsetDateTime::from_unix_timestamp(0)?,
            BlockStatus::Processing,
        )?;

        let genesis_block_id = genesis_block.generate_id()?.clone();

        log::info!(
            "Genesis storage block created with Id: {}",
            genesis_block_id
        );
        state.put_block(genesis_block.clone()).await?;
        log::info!(
            "Genesis storage block with Id {} put in database successfully.",
            genesis_block_id
        );
        self.accept_block(genesis_block).await?;
        log::info!(
            "Genesis storage block with Id {} was accepted by this node.",
            genesis_block_id
        );

        // reacquire state since we need to release writable_interior to pass into accept_block
        let state = self.mut_state_status().await?;
        state.set_state_initialized().await?;
        log::info!("State set to initialized, so it won't hapen again.");

        Ok(())
    }

    async fn set_preference(&mut self, preferred_block_id: Id) {
        log::trace!("setting preferred block id...");
        self.preferred_block_id = Some(preferred_block_id)
    }

    async fn open_connection(
        &mut self,
        service_id: ServiceId,
        target: &str,
    ) -> Result<Channel, Status> {
        log::trace!(
            "opening a new connection to host for service_id: {}",
            service_id
        );
        Ok(self
            .grpc_broker
            .lock()
            .await
            .dial_to_host_service(service_id)
            .await
            .with_context(|| {
                format!(
                    "Failed to dial a connection to the {} server {}",
                    target, service_id,
                )
            })
            .map_err(|e| e.into())
            .map_err(into_status)?)
    }

    pub async fn new_grpc_server<S>(&mut self, server: S) -> Result<ServiceId, Status>
    where
        S: Service<HyperRequest<Body>, Response = HyperResponse<BoxBody>>
            + NamedService
            + Clone
            + Send
            + 'static,
        <S as Service<HyperRequest<Body>>>::Future: Send + 'static,
        <S as Service<HyperRequest<Body>>>::Error: Into<Box<dyn StdError + Send + Sync>> + Send,
    {
        log::trace!("Opening a new gRPC server through the grpc broker...");
        self.grpc_broker
            .lock()
            .await
            .new_grpc_server(server)
            .await
            .context("Unable to create a new GHttp Server server for handlers")
            .map_err(|e| e.into())
            .map_err(into_status)
    }

    async fn version(&self) -> Result<Response<VersionResponse>, Status> {
        let version = self.version.to_string();
        log::info!("responding with version {}", version);
        Ok(Response::new(VersionResponse {
            version: self.version.to_string(),
        }))
    }

    async fn propose_block(&mut self, data: &[u8]) -> Result<(), LandslideError> {
        log::trace!("Proposing a new block...");
        let fixed_array: [u8; BLOCK_DATA_LEN] = data.try_into()?;
        self.mem_pool.push(fixed_array);

        self.notify_block_ready().await
    }

    async fn notify_block_ready(&mut self) -> Result<(), LandslideError> {
        log::trace!("Notifying engine that a new block is ready...");
        match self.engine_client.as_mut() {
            Some(engine_client) => {
                engine_client
                    .notify(NotifyRequest {
                        message: Message::PendingTransactions as u32,
                    })
                    .await?;
            }
            None => log::debug!("dropped message to consensus engine..."),
        }

        Ok(())
    }

    async fn accept_block(&mut self, mut block: Block) -> Result<(), LandslideError> {
        let state = self.mut_state().await?;

        block.status = BlockStatus::Accepted;
        let bid = block.generate_id()?.clone();
        log::info!("Accepting block with id: {}", bid);

        state.put_block(block).await?;
        log::info!("Put accepted block into database with id: {}", bid);

        state.set_last_accepted_block_id(&bid).await?;
        log::info!("Setting last accepted block id in database to: {}", bid);

        self.verified_blocks.remove(&bid);
        log::info!(
            "Removing from verified blocks, since it is now accepted, the block id: {}",
            bid
        );

        Ok(())
    }

    // Verify returns nil iff this block is valid.
    // To be valid, it must be that:
    // b.parent.Timestamp < b.Timestamp <= [local time] + 1 hour
    async fn verify_block(&mut self, mut block: Block) -> Result<(), LandslideError> {
        log::trace!("Verifying block...");
        let state = self.mut_state().await?;

        let bid = block.generate_id()?.clone();
        let parent_id = block.parent_id().clone();

        let parent_block = state.get_block(&parent_id).await?.ok_or_else(||
            LandslideError::Other(anyhow!("TimestampVm::verify_block - Parent Block ID {} was not found in the database for Block being verified with Id {}", parent_id, bid)))?;
        log::info!("retrieved parent block");

        // Ensure [b]'s height comes right after its parent's height
        if parent_block.height() + 1 != block.height() {
            let err = LandslideError::ParentBlockHeightUnexpected {
                block_height: block.height(),
                parent_block_height: parent_block.height(),
            };
            log::error!("{}", err);
            return Err(err);
        }

        let bts = *block.timestamp().offsetdatetime();
        let pbts = *parent_block.timestamp().offsetdatetime();
        // Ensure [b]'s timestamp is after its parent's timestamp.
        if bts < pbts {
            return Err(LandslideError::Other(anyhow!("The current block {}'s  timestamp {}, is before the parent block {}'s timestamp {}, which is invalid for a Blockchain.", bid, bts, parent_id, pbts)));
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

        log::info!("Adding block to list of verified blocks: {:?}", bid);
        // Put that block to verified blocks in memory
        self.verified_blocks.insert(bid, block);

        Ok(())
    }

    // Reject sets this block's status to Rejected and saves the status in state
    // Recall that b.vm.DB.Commit() must be called to persist to the DB
    #[allow(dead_code)]
    async fn reject_block(&mut self, mut block: Block) -> Result<(), LandslideError> {
        let state = self.mut_state().await?;

        block.status = BlockStatus::Rejected;

        let block_id = block.generate_id()?.clone();

        state.put_block(block).await?;

        self.verified_blocks.remove(&block_id);

        Ok(())
    }
}

pub struct TimestampVm {
    interior: Arc<RwLock<TimestampVmInterior>>,
}

impl TimestampVm {
    pub fn new(grpc_broker: Arc<Mutex<GRpcBroker>>) -> Result<TimestampVm, LandslideError> {
        Ok(TimestampVm {
            interior: Arc::new(RwLock::new(TimestampVmInterior {
                ctx: None,
                version: Version::new(0, 1, 0),
                grpc_broker,

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
                mem_pool: Vec::new(),
            })),
        })
    }
}

#[tonic::async_trait]
impl Vm for TimestampVm {
    async fn initialize(
        &self,
        request: Request<InitializeRequest>,
    ) -> Result<Response<InitializeResponse>, Status> {
        log::trace!("called");
        let mut writable_interior = self.interior.write().await;

        log::trace!("Initialize Calling Version...");

        let version =
            writable_interior.version().await
            .context("Failed calling version on the mutable interior (but passed immutably) of the TimestampVm")
            .map_err(|e| e.into())
            .map_err(into_status)?;

        log::trace!("Initialize obtained VM version: {:?}", version);

        let ir = request.into_inner();
        log::trace!("Full Request: {:?}", ir,);

        writable_interior.ctx = Some(Context {
            network_id: ir.network_id,
            subnet_id: ir.subnet_id,
            chain_id: ir.chain_id,
            node_id: ir.node_id,

            x_chain_id: ir.x_chain_id,
            avax_asset_id: ir.avax_asset_id,
        });
        log::trace!("setup context from genesis data");

        let mut versioned_db_clients: BTreeMap<Version, DatabaseClient<Channel>> = BTreeMap::new();
        for db_server in ir.db_servers.iter() {
            let ver_without_v = db_server.version.trim_start_matches('v');
            let version = Version::parse(ver_without_v)
                .with_context(|| format!("In initialize, failed to parse the semver::Version for a VersionedDatabase obtained from the host/client. Version provided by server: {}, with the leading 'v' removed: {}", db_server.version, ver_without_v))
                .map_err(|e| e.into())
                .map_err(into_status)?;

            let conn = writable_interior
                .open_connection(db_server.db_server, "VersionedDatabase")
                .await?;

            let db_client = DatabaseClient::new(conn);
            versioned_db_clients.insert(version, db_client);
            log::info!(
                "initialized versioned db client for server: {:?}",
                db_server
            );
        }
        writable_interior.versioned_db_clients = Some(versioned_db_clients);
        log::trace!("initialized all versioned db clients",);

        let conn = writable_interior
            .open_connection(ir.engine_server, "engine_server")
            .await?;
        writable_interior.engine_client = Some(MessengerClient::new(conn));
        log::trace!("initialized messenger (engine server) client",);

        let conn = writable_interior
            .open_connection(ir.keystore_server, "keystore_server")
            .await?;
        writable_interior.keystore_client = Some(KeystoreClient::new(conn));
        log::trace!("initialized keystore client");

        let conn = writable_interior
            .open_connection(ir.shared_memory_server, "shared_memory_server")
            .await?;
        writable_interior.shared_memory_client = Some(SharedMemoryClient::new(conn));
        log::trace!("initialized shared memory client",);

        let conn = writable_interior
            .open_connection(ir.bc_lookup_server, "bc_lookup_server")
            .await?;
        writable_interior.bc_lookup_client = Some(AliasReaderClient::new(conn));
        log::trace!("initialized alias reader client");

        let conn = writable_interior
            .open_connection(ir.sn_lookup_server, "sn_lookup_server")
            .await?;
        writable_interior.sn_lookup_client = Some(SubnetLookupClient::new(conn));
        log::trace!("initialized subnet lookup client",);

        let conn = writable_interior
            .open_connection(ir.app_sender_server, "app_sender_server")
            .await?;
        writable_interior.appsender_client = Some(AppSenderClient::new(conn));
        log::trace!("initialized app sender client");

        if let Some(versioned_db_clients) = writable_interior.versioned_db_clients.as_ref() {
            if versioned_db_clients.is_empty() {
                return Err(Status::unknown("zero versioned_db_clients were found. Unable to proceed without a versioned database."));
            }
            if let Some(db_client) = versioned_db_clients.values().rev().next() {
                log::info!("Initialized state for this VM");
                let state = State::new(db_client.clone());
                writable_interior.state = Some(state);
            } else {
                return Err(Status::unknown("database client not found, when length was verified to be > 0 a little earlier."));
            }
        } else {
            return Err(Status::unknown("versioned_db_clients was None, when it was just set in this same method a little bit before."));
        }

        writable_interior
            .init_genesis(ir.genesis_bytes.as_ref())
            .await
            .context("Failed to initialize genesis block.")
            .map_err(|e| e.into())
            .map_err(into_status)?;

        log::trace!("TimestampVm::Initialize genesis initialized");

        log::info!("Using state for this VM");
        let state = writable_interior.mut_state_status().await?;

        let labid = state
            .get_last_accepted_block_id()
            .await
            .context("Failed to get last accepted block id")
            .map_err(|e| e.into())
            .map_err(into_status)?
            .ok_or_else(||Status::unknown("TimestampVm::initialize - unable to find last accepted block id in the database. This is unusual since the init_genesis() call made within this function a bit earlier, should have initialized the genesis block at least.".to_string()))?;

        log::trace!(
            "TimestampVm::Initialize obtained last accepted block id: {}",
            labid
        );

        let block = state.get_block(&labid).await
        .with_context(|| format!("Failed to get Block from database with id {}", labid))
        .map_err(|e| e.into())
        .map_err(into_status)?
        .ok_or_else(||Status::unknown(format!("The storage block with Id {} was not found in the database, which is unusual considering this id was obtained from the database as the last accepted block's id.", labid)))?;

        let u32status = block.status as u32;

        log::trace!(
            "TimestampVm::Initialize obtained last accepted block with status: {:?}(u32 value: {})",
            block.status,
            u32status
        );

        writable_interior.set_preference(labid.clone()).await;

        Ok(Response::new(InitializeResponse {
            last_accepted_id: Vec::from(labid.as_ref()),
            last_accepted_parent_id: Vec::from(block.parent_id().as_ref()),
            bytes: Vec::from(block.data()),
            height: block.height(),
            timestamp: Vec::from(block.timestamp().bytes()),
            status: u32status,
        }))
    }

    async fn bootstrapping(&self, _request: Request<()>) -> Result<Response<()>, Status> {
        log::trace!("bootstrapping called");
        Ok(Response::new(()))
    }

    async fn bootstrapped(&self, _request: Request<()>) -> Result<Response<()>, Status> {
        log::trace!("bootstrapped called");
        Ok(Response::new(()))
    }

    async fn shutdown(&self, _request: Request<()>) -> Result<Response<()>, Status> {
        log::trace!("shutdown called");
        let mut writable_interior = self.interior.write().await;
        if let Some(state) = writable_interior.state.as_mut() {
            state.close().await.map_err(into_status)?;
        }

        Ok(Response::new(()))
    }

    async fn create_handlers(
        &self,
        _request: Request<()>,
    ) -> Result<Response<CreateHandlersResponse>, Status> {
        log::trace!("create_handlers called");
        let mut writable_interor = self.interior.write().await;

        let ghttp_server = proto::GHttpServer::new_server(
            writable_interor.grpc_broker.clone(),
            handlers::new(self.interior.clone()),
        );
        log::debug!("Creating a new JSON-RPC 2.0 server for API handlers...",);
        let server_id = writable_interor.new_grpc_server(ghttp_server).await?;

        let vm_api_service = Handler {
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
            handlers: vec![vm_api_service],
        }))
    }

    // This is the code that we must meet: https://github.com/ava-labs/avalanchego/blob/master/vms/rpcchainvm/vm_client.go#L343
    async fn create_static_handlers(
        &self,
        _request: Request<()>,
    ) -> Result<Response<CreateStaticHandlersResponse>, Status> {
        log::trace!("create_static_handlers called");
        let mut writable_interior = self.interior.write().await;

        let ghttp_server = proto::GHttpServer::new_server(
            writable_interior.grpc_broker.clone(),
            static_handlers::new(),
        );
        log::debug!("Creating a new JSON-RPC 2.0 server for static handlers...",);
        let server_id = writable_interior.new_grpc_server(ghttp_server).await?;

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
        log::trace!("connected called");
        Ok(Response::new(()))
    }

    async fn disconnected(
        &self,
        _request: Request<DisconnectedRequest>,
    ) -> Result<Response<()>, Status> {
        log::trace!("disconnected called");
        Ok(Response::new(()))
    }

    async fn build_block(
        &self,
        _request: Request<()>,
    ) -> Result<Response<BuildBlockResponse>, Status> {
        log::trace!("build_block called");

        let mut writable_interior = self.interior.write().await;

        // Get the value to put in the new block
        let block_data = writable_interior
            .mem_pool
            .pop()
            .ok_or_else(|| Status::ok("No blocks to be built."))?;

        let preferred_block_id = match writable_interior.preferred_block_id.take() {
            None => return Err(Status::ok("No preferred block id to be built.")),
            Some(preferred_block_id) => preferred_block_id,
        };

        // Gets Preferred Block
        let preferred_block = writable_interior.mut_state().await.map_err(into_status)?
            .get_block(&preferred_block_id).await.map_err(into_status)?
            .ok_or_else(||Status::unknown("Preferred block couldn't be retrieved from database, despite having a preferred block id."))?;
        let preferred_height = preferred_block.height();

        // Build the block with preferred height
        let mut block = Block::new(
            preferred_block_id,
            preferred_height + 1,
            block_data,
            OffsetDateTime::now_utc(),
            BlockStatus::Processing,
        )
        .map_err(into_status)?;
        writable_interior
            .verify_block(block.clone())
            .await
            .map_err(into_status)?;

        // Notify consensus engine that there are more pending data for blocks
        // (if that is the case) when done building this block
        if !writable_interior.mem_pool.is_empty() {
            writable_interior
                .notify_block_ready()
                .await
                .map_err(into_status)?;
        }

        Ok(Response::new(BuildBlockResponse {
            id: block.generate_id().map_err(into_status)?.to_vec(),
            bytes: Vec::from(block.data()),
            height: block.height(),
            parent_id: block.parent_id().to_vec(),
            timestamp: Vec::from(block.timestamp().bytes()),
        }))
    }

    async fn parse_block(
        &self,
        request: Request<ParseBlockRequest>,
    ) -> Result<Response<ParseBlockResponse>, Status> {
        log::trace!("parse_block called");
        let pbr = request.into_inner();

        let mut block: Block = serde_json::from_slice(pbr.bytes.as_ref())
            .map_err(|e| e.into())
            .map_err(into_status)?;

        block.status = BlockStatus::Processing;

        let mut writable_interior = self.interior.write().await;
        let state = writable_interior.mut_state().await.map_err(into_status)?;
        let mut ret_block = if let Some(existing_block) = state
            .get_block(block.generate_id().map_err(into_status)?)
            .await
            .map_err(into_status)?
        {
            // if we already have this block, return that
            existing_block
        } else {
            block
        };

        Ok(Response::new(ParseBlockResponse {
            id: ret_block.generate_id().map_err(into_status)?.to_vec(),
            parent_id: ret_block.parent_id().to_vec(),
            status: ret_block.status as u32,
            height: ret_block.height(),
            timestamp: Vec::from(ret_block.timestamp().bytes()),
        }))
    }

    async fn get_block(
        &self,
        _request: Request<GetBlockRequest>,
    ) -> Result<Response<GetBlockResponse>, Status> {
        log::trace!("get_block called");
        todo!()
    }

    async fn set_preference(
        &self,
        request: Request<SetPreferenceRequest>,
    ) -> Result<Response<()>, Status> {
        log::trace!("set_preference called");
        let spr = request.into_inner();

        let mut writable_interior = self.interior.write().await;
        writable_interior
            .set_preference(Id::from_slice(&spr.id).map_err(into_status)?)
            .await;

        Ok(Response::new(()))
    }

    async fn health(&self, _request: Request<()>) -> Result<Response<HealthResponse>, Status> {
        log::trace!("health called");
        log::debug!("TimestampVM: Health endpoint pinged; reporting healthy...");
        Ok(Response::new(HealthResponse {
            details: "All is well.".to_string(),
        }))
    }

    async fn version(&self, _request: Request<()>) -> Result<Response<VersionResponse>, Status> {
        log::trace!("version called");
        let readable_interior = self.interior.read().await;
        readable_interior.version().await
    }

    async fn app_request(&self, _request: Request<AppRequestMsg>) -> Result<Response<()>, Status> {
        log::trace!("app_request called");
        Ok(Response::new(()))
    }

    async fn app_request_failed(
        &self,
        _request: Request<AppRequestFailedMsg>,
    ) -> Result<Response<()>, Status> {
        log::trace!("app_request_failed called");
        Ok(Response::new(()))
    }

    async fn app_response(
        &self,
        _request: Request<AppResponseMsg>,
    ) -> Result<Response<()>, Status> {
        log::trace!("app_response called");
        Ok(Response::new(()))
    }

    async fn app_gossip(&self, _request: Request<AppGossipMsg>) -> Result<Response<()>, Status> {
        log::trace!("app_gossip called");
        Ok(Response::new(()))
    }

    async fn gather(&self, _request: Request<()>) -> Result<Response<GatherResponse>, Status> {
        log::trace!("gather called");
        todo!()
    }

    async fn block_verify(
        &self,
        _request: Request<BlockVerifyRequest>,
    ) -> Result<Response<BlockVerifyResponse>, Status> {
        log::trace!("block_verify called");
        todo!()
    }

    async fn block_accept(
        &self,
        _request: Request<BlockAcceptRequest>,
    ) -> Result<Response<()>, Status> {
        log::trace!("block_accept called");
        Ok(Response::new(()))
    }

    async fn block_reject(
        &self,
        _request: Request<BlockRejectRequest>,
    ) -> Result<Response<()>, Status> {
        log::trace!("block_reject called");
        Ok(Response::new(()))
    }

    async fn get_ancestors(
        &self,
        _request: Request<GetAncestorsRequest>,
    ) -> Result<Response<GetAncestorsResponse>, Status> {
        log::trace!("get_ancestors called");
        todo!()
    }

    async fn batched_parse_block(
        &self,
        _request: Request<BatchedParseBlockRequest>,
    ) -> Result<Response<BatchedParseBlockResponse>, Status> {
        log::trace!("batched_parse_block called");
        todo!()
    }
}
