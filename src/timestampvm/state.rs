// Copied from: https://github.com/ava-labs/timestampvm/blob/main/timestampvm/block.go

use crate::error::LandslideError;
use crate::id::Id;
use crate::proto::rpcdb::database_client::*;
use crate::proto::rpcdb::*;
use serde::{Deserialize, Serialize};
use std::convert::AsRef;
use std::convert::TryInto;
use time::OffsetDateTime;
use tonic::transport::Channel;
use tonic::Response;

pub type Db = DatabaseClient<Channel>;

const LAST_ACCEPTED_BLOCK_ID_KEY: &[u8] = b"last_accepted_block_id";
const STATE_INITIALIZED_KEY: &[u8] = b"state_initialized";
const STATE_INITIALIZED_VALUE: &[u8] = b"state_has_infact_been_initialized";

const BLOCK_STATE_PREFIX: &[u8] = b"blockStatePrefix";
const SINGLETON_STATE_PREFIX: &[u8] = b"singleton";

const BLOCK_TIMESTAMP_BYTES: usize = 16;
pub const BLOCK_DATA_LEN: usize = 32;

#[derive(Debug)]
pub struct State {
    // block database
    db: Db,

    last_accepted_block_id_key: Vec<u8>,
    state_initialized_key: Vec<u8>,
}

impl State {
    pub fn new(db: Db) -> State {
        State {
            db,
            last_accepted_block_id_key: Self::prefix(
                BLOCK_STATE_PREFIX,
                LAST_ACCEPTED_BLOCK_ID_KEY,
            ),
            state_initialized_key: Self::prefix(SINGLETON_STATE_PREFIX, STATE_INITIALIZED_KEY),
        }
    }

    // Commit commits pending operations to baseDB
    pub async fn commit(&mut self) -> Result<(), LandslideError> {
        log::warn!("State::commit is a NOOP since underlying database has no commit method.");
        Ok(())
    }

    // Close closes the underlying base database
    pub async fn close(&mut self) -> Result<Response<CloseResponse>, LandslideError> {
        Ok(self.db.close(CloseRequest {}).await?)
    }

    pub async fn get_block(&mut self, block_id: &[u8]) -> Result<StorageBlock, LandslideError> {
        let key = Self::prefix(BLOCK_STATE_PREFIX, block_id);
        let get_response = self.db.get(GetRequest { key }).await?.into_inner();

        if get_response.err != 0 {
            return Err(LandslideError::Generic(format!(
                "DatabaseClient::get returned with error: {}",
                get_response.err
            )));
        }

        let sb: StorageBlock = serde_json::from_slice(&get_response.value)?;

        Ok(sb)
    }

    pub async fn put_block(&mut self, sb: StorageBlock) -> Result<(), LandslideError> {
        let value = serde_json::to_vec(&sb)?;
        let key = Self::prefix(BLOCK_STATE_PREFIX, sb.block.id()?.as_ref());
        let put_response = self.db.put(PutRequest { key, value }).await?.into_inner();
        if put_response.err != 0 {
            return Err(LandslideError::Generic(format!(
                "DatabaseClient::put returned with error: {}",
                put_response.err
            )));
        }
        Ok(())
    }

    pub async fn delete_block(&mut self, block_id: &Id) -> Result<(), LandslideError> {
        let key = Self::prefix(BLOCK_STATE_PREFIX, &block_id.as_ref());
        let delete_response = self.db.delete(DeleteRequest { key }).await?.into_inner();
        if delete_response.err != 0 {
            return Err(LandslideError::Generic(format!(
                "DatabaseClient::delete returned with error: {}",
                delete_response.err
            )));
        }
        Ok(())
    }

    pub async fn get_last_accepted_block_id(&mut self) -> Result<Id, LandslideError> {
        let get_response = self
            .db
            .get(GetRequest {
                key: self.last_accepted_block_id_key.clone(),
            })
            .await?
            .into_inner();
        if get_response.err != 0 {
            return Err(LandslideError::Generic(format!(
                "DatabaseClient::get returned with error: {}",
                get_response.err
            )));
        }

        Ok(Id::new(get_response.value.as_ref())?)
    }

    pub async fn set_last_accepted_block_id(&mut self, id: &Id) -> Result<(), LandslideError> {
        let put_response = self
            .db
            .put(PutRequest {
                key: self.last_accepted_block_id_key.clone(),
                value: Vec::from(id.as_ref()),
            })
            .await?
            .into_inner();
        if put_response.err != 0 {
            return Err(LandslideError::Generic(format!(
                "DatabaseClient::put returned with error: {}",
                put_response.err
            )));
        }
        Ok(())
    }

    pub async fn is_state_initialized(&mut self) -> Result<bool, LandslideError> {
        let get_response = self
            .db
            .get(GetRequest {
                key: self.state_initialized_key.clone(),
            })
            .await?
            .into_inner();
        if get_response.err != 0 {
            return Err(LandslideError::Generic(format!(
                "DatabaseClient::get returned with error: {}",
                get_response.err
            )));
        }
        if get_response.value.len() > 0 {
            return Ok(true);
        }
        Ok(false)
    }

    pub async fn set_state_initialized(&mut self) -> Result<(), LandslideError> {
        let put_response = self
            .db
            .put(PutRequest {
                key: self.state_initialized_key.clone(),
                value: Vec::from(STATE_INITIALIZED_VALUE),
            })
            .await?
            .into_inner();
        if put_response.err != 0 {
            return Err(LandslideError::Generic(format!(
                "DatabaseClient::put returned with error: {}",
                put_response.err
            )));
        }
        Ok(())
    }

    fn prefix(prefix: &[u8], data: &[u8]) -> Vec<u8> {
        let mut result = Vec::with_capacity(prefix.len() + data.len());
        result.extend_from_slice(prefix);
        result.extend_from_slice(data);

        result
    }
}

// Block is a block on the chain.
// Each block contains:
// 1) ParentID
// 2) Height
// 3) Timestamp
// 4) A piece of data (a string)
#[derive(Serialize, Deserialize, Debug, Clone)]
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

    pub fn timestamp_as_offsetdatetime(&self) -> Result<OffsetDateTime, LandslideError> {
        let timestamp_bytes = self.timestamp.clone();
        Self::bytes_to_offsetdatetime(timestamp_bytes)
    }

    pub fn bytes_to_offsetdatetime(
        timestamp_bytes: Vec<u8>,
    ) -> Result<OffsetDateTime, LandslideError> {
        // for now we only know what to do with the first 16 bytes (128 bits)
        if timestamp_bytes.len() < BLOCK_TIMESTAMP_BYTES {
            return Err(LandslideError::Generic(format!("There were {} bytes, which is less than the required {} bytes in the timestamp field, since we interpret it as a {}-bit timestamp.", BLOCK_TIMESTAMP_BYTES, BLOCK_TIMESTAMP_BYTES * 8, timestamp_bytes.len())));
        }

        let timestamp_little_endian: [u8; BLOCK_TIMESTAMP_BYTES] =
            timestamp_bytes.try_into().unwrap_or_else(|v: Vec<u8>| {
                panic!(
                    "Expected a Vec of length {} but it was {}",
                    BLOCK_TIMESTAMP_BYTES,
                    v.len()
                )
            });

        Ok(OffsetDateTime::from_unix_timestamp_nanos(
            i128::from_le_bytes(timestamp_little_endian),
        )?)
    }

    pub fn offsetdatetime_to_bytes(dt: OffsetDateTime) -> Result<Vec<u8>, LandslideError> {
        let timestamp_val = dt.unix_timestamp_nanos();
        let timestamp_bytes = i128::to_le_bytes(timestamp_val);
        Ok(Vec::from(timestamp_bytes))
    }
}

// The Block structure stored in storage backend (i.e. Database)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageBlock {
    pub block: Block,
    pub status: Status,
}

impl StorageBlock {
    pub fn new(
        parent_id: Id,
        height: u64,
        data: Vec<u8>,
        timestamp: OffsetDateTime,
    ) -> Result<Self, LandslideError> {
        Ok(StorageBlock {
            block: Block {
                parent_id,
                height,
                data,
                timestamp: Block::offsetdatetime_to_bytes(timestamp)?,
            },
            status: Status::Processing,
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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
