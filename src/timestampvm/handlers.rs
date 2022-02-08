use super::state::BLOCK_DATA_LEN;
use super::TimestampVmInterior;
use crate::encoding::{Checksum, Encoding};
use crate::error::into_jsonrpc_error;
use crate::id::Id;
use jsonrpc_core::{BoxFuture, Error as JsonRpcError, IoHandler, Result};
use jsonrpc_derive::rpc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

pub fn new(vm: Arc<RwLock<TimestampVmInterior>>) -> IoHandler {
    let mut io = IoHandler::new();
    let handlers = HandlersImpl { vm };

    io.extend_with(handlers.to_delegate());

    io
}

#[derive(Serialize, Deserialize)]
pub struct ProposeBlockArgs {
    data: String,
}

#[derive(Serialize, Deserialize)]
pub struct ProposeBlockReply {
    success: bool,
}

#[derive(Serialize, Deserialize)]
pub struct GetBlockArgs {
    id: Option<Vec<u8>>,
}

#[derive(Serialize, Deserialize)]
pub struct GetBlockReply {
    timestamp: u64,
    data: String,
    id: Vec<u8>,
    #[serde(rename = "parentID")]
    parent_id: Vec<u8>,
}

#[rpc(server)]
pub trait Handlers {
    #[rpc(name = "proposeBlock", alias("timestampvm.proposeBlock"))]
    fn propose_block(&self, args: ProposeBlockArgs) -> BoxFuture<Result<ProposeBlockReply>>;

    #[rpc(name = "getBlock", alias("timestampvm.getBlock"))]
    fn get_block(&self, args: GetBlockArgs) -> BoxFuture<Result<GetBlockReply>>;
}

pub struct HandlersImpl {
    vm: Arc<RwLock<TimestampVmInterior>>,
}

impl Handlers for HandlersImpl {
    fn propose_block(&self, args: ProposeBlockArgs) -> BoxFuture<Result<ProposeBlockReply>> {
        let vm = self.vm.clone();

        Box::pin(async move {
            let bytes = Encoding::Cb58
                .decode(args.data, Checksum::Yes)
                .map_err(into_jsonrpc_error)?;

            if bytes.len() != BLOCK_DATA_LEN {
                return Err(JsonRpcError::invalid_params(format!(
                    "Bad block data length. Expected length: {}, Provided length: {}",
                    BLOCK_DATA_LEN,
                    bytes.len()
                )));
            }

            vm.write()
                .await
                .propose_block(bytes.as_ref())
                .await
                .map_err(into_jsonrpc_error)?;

            Ok(ProposeBlockReply { success: true })
        })
    }

    fn get_block(&self, args: GetBlockArgs) -> BoxFuture<Result<GetBlockReply>> {
        let vm = self.vm.clone();

        Box::pin(async move {
            let mut mutable_vm = vm.write().await;
            let mutable_state = mutable_vm.mut_state().await.map_err(into_jsonrpc_error)?;

            // If an ID is given, parse its string representation to an ids.ID
            // If no ID is given, ID becomes the ID of last accepted block
            let id: Id = match args.id {
                None => mutable_state.get_last_accepted_block_id().await
                    .map_err(into_jsonrpc_error)?
                    .ok_or_else(|| JsonRpcError::invalid_params("No Id parameter provided, and last accepted block id could not be retrieved"))?,
                Some(idbytes) => {
                    Id::from_slice(&idbytes)
                        .map_err(|e| JsonRpcError::invalid_params(format!("Unable to convert provided Id bytes into a valid Id: {}", e)))?
                },
            };

            let mut block = mutable_state.get_block(&id).await
            .map_err(into_jsonrpc_error)?
            .ok_or_else(||JsonRpcError::invalid_params("Block with the provided id (or last accepted block with id) does not exist."))?;

            let bid = block.generate_id().map_err(into_jsonrpc_error)?.clone();

            let encoded_data = Encoding::Cb58
                .encode(block.data().as_ref(), Checksum::Yes)
                .map_err(into_jsonrpc_error)?;

            let timestamp_unix_i64 = block.timestamp().offsetdatetime().unix_timestamp();

            let timestamp_unix_u64 = u64::try_from(timestamp_unix_i64)
                .map_err(|e| e.into())
                .map_err(into_jsonrpc_error)?;

            Ok(GetBlockReply {
                id: Vec::from(bid.as_ref()),
                parent_id: Vec::from(block.parent_id().as_ref()),
                data: encoded_data,
                timestamp: timestamp_unix_u64,
            })
        })
    }
}
