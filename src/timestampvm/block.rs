// Copied from: https://github.com/ava-labs/timestampvm/blob/main/timestampvm/block.go

use crate::error::LandslideError;
use crate::id::Id;
use serde::{Deserialize, Serialize};
use sled::Db;

const LAST_ACCEPTED_BLOCK_ID_KEY: &[u8] = b"last_accepted_block_id";
const STATE_INITIALIZED_KEY: &[u8] = b"state_initialized";
const STATE_INITIALIZED_VALUE: &[u8] = b"state_has_infact_been_initialized";

// Block is a block on the chain.
// Each block contains:
// 1) ParentID
// 2) Height
// 3) Timestamp
// 4) A piece of data (a string)
#[derive(Serialize, Deserialize)]
pub struct Block {
    pub parent_id: Id,
    pub height: u64,
    pub timestamp: Vec<u8>,
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
    Accepted,
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

#[derive(Debug)]
pub struct State {
    // block database
    block_db: Db,
}

impl State {
    pub fn new(block_db: Db) -> State {
        State { block_db }
    }

    pub async fn flush(&self) -> Result<(), LandslideError> {
        self.block_db.flush_async().await?;
        Ok(())
    }

    pub fn get_block(&self, block_id: &Id) -> Result<Option<StorageBlock>, LandslideError> {
        let maybe_storage_block_bytes = self.block_db.get(block_id)?;

        let storage_block_bytes = match maybe_storage_block_bytes {
            Some(s) => s,
            None => return Ok(None),
        };

        let sb: StorageBlock = serde_json::from_slice(&storage_block_bytes)?;

        Ok(Some(sb))
    }

    pub fn put_block(&self, block: StorageBlock) -> Result<(), LandslideError> {
        let block_bytes = serde_json::to_vec(&block)?;
        self.block_db.insert(block.id()?, block_bytes)?;
        Ok(())
    }

    pub fn delete_block(&self, block_id: &Id) -> Result<(), LandslideError> {
        self.block_db.remove(block_id)?;
        Ok(())
    }

    pub fn get_last_accepted_block_id(&mut self) -> Result<Option<Id>, LandslideError> {
        self.block_db.get(LAST_ACCEPTED_BLOCK_ID_KEY)?.map_or(
            Ok(None),
            |last_accepted_block_id_bytes| {
                Ok(Some(Id::new(last_accepted_block_id_bytes.as_ref())?))
            },
        )
    }

    pub fn set_last_accepted_block_id(&self, id: &Id) -> Result<(), LandslideError> {
        self.block_db
            .insert(LAST_ACCEPTED_BLOCK_ID_KEY, id.as_ref())?;

        Ok(())
    }

    pub fn is_state_initialized(&mut self) -> Result<bool, LandslideError> {
        self.block_db
            .get(STATE_INITIALIZED_KEY)?
            .map_or(Ok(false), |_| Ok(true))
    }

    pub fn set_state_initialized(&self) -> Result<(), LandslideError> {
        self.block_db
            .insert(STATE_INITIALIZED_KEY, STATE_INITIALIZED_VALUE)?;

        Ok(())
    }
}
