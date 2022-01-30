use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

#[derive(Debug)]
pub enum LandslideError {
    NoTCPPortAvailable,
    SledError(sled::Error),
    FromHexError(hex::FromHexError),
    SerdeJsonError(serde_json::Error),
    Generic(String),
}

impl Display for LandslideError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::NoTCPPortAvailable => write!(
                f,
                "No ports were available to bind the plugin's gRPC server to."
            ),
            Self::SledError(e) => write!(f, "An error occurred in the Sled database: {:?}", e),
            Self::FromHexError(e) => write!(f, "Unable to parse bytes from hexadecimal: {:?}", e),
            Self::SerdeJsonError(e) => write!(
                f,
                "An error occurred when serializing/deserializing JSON: {:?}",
                e
            ),
            Self::Generic(s) => write!(f, "{}", s),
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
