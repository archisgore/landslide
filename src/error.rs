use std::error::Error;
use std::fmt::{Display, Result as FmtResult, Formatter};

#[derive(Debug)]
pub enum LandslideError {
    NoTCPPortAvailable
}

impl Display for LandslideError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::NoTCPPortAvailable => write!(f, "No ports were available to bind the plugin's gRPC server to.")
        }
    }
}

impl Error for LandslideError {

}
