use super::id::Id;
use thiserror::Error as ThisError;
use tonic::Status;

pub fn into_status(err: LandslideError) -> tonic::Status {
    tonic::Status::unknown(format!("{:?}", err))
}

#[derive(Debug, ThisError)]
pub enum LandslideError {
    #[error("No parent block with id {parent_block_id} found for block with id {block_id}. All blocks have parents (since the genesis block is bootstrapped especially for this purpose). This block is invalid.")]
    NoParentBlock { block_id: Id, parent_block_id: Id },
    #[error("No ports were available to bind the plugin's gRPC server to.")]
    NoTCPPortAvailable,
    #[error("This executable is meant to be a go-plugin to other processes. Do not run this directly. The Magic Handshake failed.")]
    GRPCHandshakeMagicCookieValueMismatch,
    #[error("The VM has not yet been initialized, and it's internal state is empty.")]
    StateNotInitialized,
    #[error("Unable to parse bytes from hexadecimal: {0}")]
    FromHexError(#[from] hex::FromHexError),
    #[error("An error occurred when serializing/deserializing JSON: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error(transparent)]
    StdIoError(#[from] std::io::Error),
    #[error("Unable to set logger: {0}")]
    SetLoggerError(#[from] log::SetLoggerError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
    #[error("Error with tonic (gRPC) transport: {0}")]
    TonicTransportError(#[from] tonic::transport::Error),
    #[error(transparent)]
    Status(#[from] Status),
    #[error("Parent block height, {parent_block_height}, should have been exactly 1 greater than the block's height {block_height}. This block is invalid.")]
    ParentBlockHeightUnexpected {
        block_height: u64,
        parent_block_height: u64,
    },
    #[error("Error occurred parsing the time components: {0}")]
    TimeErrorComponentRange(#[from] time::error::ComponentRange),
}
