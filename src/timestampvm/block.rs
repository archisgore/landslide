// Copied from: https://github.com/ava-labs/timestampvm/blob/main/timestampvm/block.go

use crate::id::Id;
use crate::error::LandslideError;
use serde::{Serialize, Deserialize};

// Block is a block on the chain.
// Each block contains:
// 1) ParentID
// 2) Height
// 3) Timestamp
// 4) A piece of data (a string)
#[derive(Serialize, Deserialize)]
pub struct Block {
	pub parent_id: Id,
	pub height:   u64,
	pub timestamp: i64,
	pub data: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct StorageBlock {
    pub block: Block,
    pub status: Status,
}

impl StorageBlock {
    pub fn id(&self) -> Result<Id, LandslideError> {
        let block_bytes = serde_json::to_vec(&self.block)?;
        let block_id = Id::new(&block_bytes)?;
        Ok(block_id)
    }
}

#[derive(Serialize, Deserialize)]
pub enum Status {
    Unknown,
    Processing,
    Rejected,
    Accepted
}

impl Status {
    pub fn fetched(&self) -> bool {
        match self {
            Self::Processing => true,
            _ => self.decided(),
        }
    }

    pub fn decided(&self) -> bool {
        match self {
            Self::Rejected | Self::Accepted => true,
            _ => false,
        }
    }

    pub fn valid(&self) -> bool {
        match self {
            Self::Unknown => false,
            _ => true,
        }
    }
}