// Copied from: https://github.com/ava-labs/timestampvm/blob/main/timestampvm/block.go

use crate::error::LandslideError;
use crate::id::Id;
use serde::{Deserialize, Serialize};
use sled::Db;
use std::convert::TryInto;
use time::OffsetDateTime;

const LAST_ACCEPTED_BLOCK_ID_KEY: &[u8] = b"last_accepted_block_id";
const STATE_INITIALIZED_KEY: &[u8] = b"state_initialized";
const STATE_INITIALIZED_VALUE: &[u8] = b"state_has_infact_been_initialized";

// Block is a block on the chain.
// Each block contains:
// 1) ParentID
// 2) Height
// 3) Timestamp
// 4) A piece of data (a string)
#[derive(Serialize, Deserialize, Debug)]
pub struct Block {
    pub parent_id: Id,
    pub height: u64,
    pub timestamp: Vec<u8>,
    pub data: Vec<u8>,
}

impl Block {
    pub fn id(&self) -> Result<Id, LandslideError> {
        let block_bytes = serde_json::to_vec(&self)?;
        let block_id = Id::new(&block_bytes)?;
        Ok(block_id)
    }

    pub fn timestamp(&self) -> Result<OffsetDateTime, LandslideError> {
        let timestamp_bytes = self.timestamp.clone();

        // for now we only know what to do with the first 8 bytes (64 bits)
        if timestamp_bytes.len() < 8 {
            return Err(LandslideError::Generic(format!("There were {} bytes, which is less than the required 8 bytes in the timestamp field, since we interpret it as a 64-bit timestamp.", self.timestamp.len())));
        }

        let timestamp_little_endian: [u8; 8] =
            timestamp_bytes.try_into().unwrap_or_else(|v: Vec<u8>| {
                panic!("Expected a Vec of length {} but it was {}", 8, v.len())
            });

        Ok(OffsetDateTime::from_unix_timestamp(i64::from_le_bytes(
            timestamp_little_endian,
        ))?)
    }
}

#[derive(Serialize, Deserialize)]
pub struct StorageBlock {
    pub block: Block,
    pub status: Status,
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

    pub fn put_block(&self, sb: StorageBlock) -> Result<(), LandslideError> {
        let block_bytes = serde_json::to_vec(&sb)?;
        self.block_db.insert(sb.block.id()?, block_bytes)?;
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
