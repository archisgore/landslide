use super::id::Id;
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

#[derive(Debug)]
pub enum LandslideError {
    NoParentBlock {
        block_id: Id,
        parent_block_id: Id,
    },
    NoTCPPortAvailable,
    GRPCHandshakeMagicCookieValueMismatch,
    StateNotInitialized,
    SledError(sled::Error),
    FromHexError(hex::FromHexError),
    SerdeJsonError(serde_json::Error),
    StdIoError(std::io::Error),
    SetLoggerError(log::SetLoggerError),
    Generic(String),
    TonicTransportError(tonic::transport::Error),
    ParentBlockHeightUnexpected {
        block_height: u64,
        parent_block_height: u64,
    },
    TimeErrorComponentRange(time::error::ComponentRange),
}

#[macro_export]
macro_rules! log_and_escalate {
    ($e:expr) => {
        match $e {
            Err(err) => {
                log::error!("{},({}:{}), {:?}", function!(), file!(), line!(), err);
                return Err(err.into());
            }
            Ok(o) => o,
        }
    };
}

#[macro_export]
macro_rules! function {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        &name[..name.len() - 3]
    }};
}

pub fn into_status(err: LandslideError) -> tonic::Status {
    tonic::Status::unknown(format!("landslide error: {:?}", err))
}


impl Display for LandslideError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::NoTCPPortAvailable => write!(
                f,
                "No ports were available to bind the plugin's gRPC server to."
            ),
            Self::GRPCHandshakeMagicCookieValueMismatch => write!(f, "This executable is meant to be a go-plugin to other processes. Do not run this directly. The Magic Handshake failed."),
            Self::StateNotInitialized => write!(f, "The VM has not yet been initialized, and it's internal state is empty."),
            Self::SledError(e) => write!(f, "An error occurred in the Sled database: {:?}", e),
            Self::FromHexError(e) => write!(f, "Unable to parse bytes from hexadecimal: {:?}", e),
            Self::SerdeJsonError(e) => write!(
                f,
                "An error occurred when serializing/deserializing JSON: {:?}",
                e
            ),
            Self::Generic(s) => write!(f, "{}", s),
            Self::StdIoError(e) => write!(f, "Error with IO: {:?}", e),
            Self::SetLoggerError(e) => write!(f, "Error setting logger: {:?}", e),
            Self::TonicTransportError(e) => write!(f, "Error with tonic (gRPC) transport: {:?}", e),
            Self::NoParentBlock{block_id, parent_block_id} => write!(
                f,
                "No parent block with id {} found for block with id {}. All blocks have parents (since the genesis block is bootstrapped especially for this purpose). This block is invalid.", parent_block_id, block_id
            ),
            Self::ParentBlockHeightUnexpected{block_height, parent_block_height} => write!(f, "Parent block height, {}, should have been exactly 1 greater than the block's height {}. This block is invalid.", parent_block_height, block_height),
            Self::TimeErrorComponentRange(e) => write!(f, "A time component range error occurred: {:?}", e),
        }
    }
}

impl Error for LandslideError {}

impl From<sled::Error> for LandslideError {
    fn from(err: sled::Error) -> Self {
        Self::SledError(err)
    }
}

impl From<hex::FromHexError> for LandslideError {
    fn from(err: hex::FromHexError) -> Self {
        Self::FromHexError(err)
    }
}

impl From<serde_json::Error> for LandslideError {
    fn from(err: serde_json::Error) -> Self {
        Self::SerdeJsonError(err)
    }
}

impl From<std::io::Error> for LandslideError {
    fn from(err: std::io::Error) -> Self {
        Self::StdIoError(err)
    }
}

impl From<log::SetLoggerError> for LandslideError {
    fn from(err: log::SetLoggerError) -> Self {
        Self::SetLoggerError(err)
    }
}

impl From<tonic::transport::Error> for LandslideError {
    fn from(err: tonic::transport::Error) -> Self {
        Self::TonicTransportError(err)
    }
}

impl From<time::error::ComponentRange> for LandslideError {
    fn from(err: time::error::ComponentRange) -> Self {
        Self::TimeErrorComponentRange(err)
    }
}

