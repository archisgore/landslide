// Copied from: https://github.com/ava-labs/timestampvm/blob/main/timestampvm/block.go

use crate::error::LandslideError;
use crate::id::{Id, BYTE_LENGTH as ID_BYTE_LENGTH};
use crate::proto::rpcdb::database_client::*;
use crate::proto::rpcdb::*;
use crate::proto::DatabaseError;
use num::FromPrimitive;
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

    pub async fn get(&mut self, key: Vec<u8>) -> Result<Option<Vec<u8>>, LandslideError> {
        let get_response = self.db.get(GetRequest { key }).await?.into_inner();

        let dberr = DatabaseError::from_u32(get_response.err);
        match dberr {
            Some(DatabaseError::Closed) => Err(LandslideError::Generic(format!(
                "DatabaseClient::get returned with error: {:?}",
                dberr
            ))),
            Some(DatabaseError::NotFound) => Ok(None),
            _ => Ok(Some(get_response.value)),
        }
    }

    pub async fn put(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<(), LandslideError> {
        let put_response = self.db.put(PutRequest { key, value }).await?.into_inner();

        let dberr = DatabaseError::from_u32(put_response.err);
        match dberr {
            Some(DatabaseError::None) => Ok(()),
            Some(DatabaseError::Closed) => Err(LandslideError::Generic(format!(
                "DatabaseClient::put returned with error: {:?}.",
                dberr
            ))),
            Some(DatabaseError::NotFound) => Err(LandslideError::Generic(format!(
                "DatabaseClient::put returned with error: {:?}.",
                dberr
            ))),
            _ => Err(LandslideError::Generic(format!(
                "DatabaseClient::put returned with unknown error: {}.",
                put_response.err
            ))),
        }
    }

    pub async fn delete(&mut self, key: Vec<u8>) -> Result<(), LandslideError> {
        let delete_response = self.db.delete(DeleteRequest { key }).await?.into_inner();

        let dberr = DatabaseError::from_u32(delete_response.err);
        match dberr {
            Some(DatabaseError::None) => Ok(()),
            Some(DatabaseError::Closed) => Err(LandslideError::Generic(format!(
                "DatabaseClient::delete returned with error: {:?}",
                dberr
            ))),
            Some(DatabaseError::NotFound) => Err(LandslideError::Generic(format!(
                "DatabaseClient::delete returned with error: {:?}",
                dberr
            ))),
            _ => Err(LandslideError::Generic(format!(
                "DatabaseClient::delete returned with unknown error: {}.",
                delete_response.err
            ))),
        }
    }

    pub async fn get_block(
        &mut self,
        block_id: &[u8],
    ) -> Result<Option<StorageBlock>, LandslideError> {
        let key = Self::prefix(BLOCK_STATE_PREFIX, block_id);
        let maybe_sb_bytes = self.get(key).await?;

        Ok(match maybe_sb_bytes {
            Some(sb_bytes) => serde_json::from_slice(&sb_bytes)?,
            None => None,
        })
    }

    pub async fn put_block(&mut self, sb: StorageBlock) -> Result<(), LandslideError> {
        let value = serde_json::to_vec(&sb)?;
        let key = Self::prefix(BLOCK_STATE_PREFIX, sb.block.id()?.as_ref());

        self.put(key, value).await
    }

    pub async fn delete_block(&mut self, block_id: &Id) -> Result<(), LandslideError> {
        let key = Self::prefix(BLOCK_STATE_PREFIX, block_id.as_ref());
        self.delete(key).await
    }

    pub async fn get_last_accepted_block_id(&mut self) -> Result<Option<Id>, LandslideError> {
        let maybe_block_id_bytes = self.get(self.last_accepted_block_id_key.clone()).await?;

        Ok(match maybe_block_id_bytes {
            Some(block_id_bytes) => {
                if block_id_bytes.len() != ID_BYTE_LENGTH {
                    let errmsg = format!("Id byte length was expected to be {}, but the database provided the last accepted id of length {}. The Id saved to the database is not the same structure as the one being retrieved into. This is a critical failure.", ID_BYTE_LENGTH, block_id_bytes.len());
                    log::error!("{}", errmsg);
                    return Err(LandslideError::Generic(errmsg));
                }
                let mut block_id_byte_array: [u8; ID_BYTE_LENGTH] = [0; ID_BYTE_LENGTH];
                for (i, b) in block_id_bytes.into_iter().enumerate() {
                    block_id_byte_array[i] = b;
                }

                log::info!("Getting last accepted block id bytes: {:?}", &block_id_byte_array);
                Some(Id::from_bytes(block_id_byte_array)?)
            },
            None => None,
        })
    }

    pub async fn set_last_accepted_block_id(&mut self, id: &Id) -> Result<(), LandslideError> {
        log::info!("Setting last accepted block id bytes: {:?}", id.as_ref());
        self.put(
            self.last_accepted_block_id_key.clone(),
            Vec::from(id.as_ref()),
        )
        .await
    }

    pub async fn is_state_initialized(&mut self) -> Result<bool, LandslideError> {
        let maybe_state_initialized_bytes = self.get(self.state_initialized_key.clone()).await?;

        Ok(match maybe_state_initialized_bytes {
            Some(state_initialized_bytes) => !state_initialized_bytes.is_empty(),
            None => false,
        })
    }

    pub async fn set_state_initialized(&mut self) -> Result<(), LandslideError> {
        self.put(
            self.state_initialized_key.clone(),
            Vec::from(STATE_INITIALIZED_VALUE),
        )
        .await
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
        let block_id = Id::generate(&block_bytes)?;
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
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
        matches!(self, Self::Rejected | Self::Accepted)
    }

    pub fn valid(&self) -> bool {
        !matches!(self, Self::Unknown)
    }
}
