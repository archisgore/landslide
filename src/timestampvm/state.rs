// Copied from: https://github.com/ava-labs/timestampvm/blob/main/timestampvm/block.go

use crate::error::LandslideError;
use crate::id::Id;
use crate::proto::rpcdb::database_client::*;
use crate::proto::rpcdb::*;
use crate::proto::DatabaseError;
use anyhow::{anyhow, Context, Result};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::BufMut;
use lazy_static::lazy_static;
use num::FromPrimitive;
use serde::{
    de::{Deserializer, Error},
    ser::Serializer,
    Deserialize, Serialize,
};
use std::convert::AsRef;
use std::io::Cursor;
use time::{format_description::well_known::Rfc3339, OffsetDateTime, UtcOffset};
use tonic::transport::Channel;
use tonic::Response;

pub type Db = DatabaseClient<Channel>;

const LAST_ACCEPTED_BLOCK_ID_KEY: &[u8] = b"last_accepted_block_id";
const STATE_INITIALIZED_KEY: &[u8] = b"state_initialized";
const STATE_INITIALIZED_VALUE: &[u8] = b"state_has_infact_been_initialized";

const BLOCK_STATE_PREFIX: &[u8] = b"blockStatePrefix";
const SINGLETON_STATE_PREFIX: &[u8] = b"singleton";

// Golang's Zero time is January 1, year 1, 00:00:00.000000000 UTC
// https://cs.opensource.google/go/go/+/refs/tags/go1.17.6:src/time/time.go;l=97
const GOLANG_ZERO_DATETIME_STR: &str = "0001-01-01T00:00:00Z";

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

    // Close closes the underlying base database
    pub async fn close(&mut self) -> Result<Response<CloseResponse>, LandslideError> {
        Ok(self.db.close(CloseRequest {}).await?)
    }

    pub async fn get(&mut self, key: Vec<u8>) -> Result<Option<Vec<u8>>, LandslideError> {
        let get_response = self.db.get(GetRequest { key }).await?.into_inner();

        let dberr = DatabaseError::from_u32(get_response.err);
        match dberr {
            Some(DatabaseError::Closed) => Err(LandslideError::Other(anyhow!(
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
            Some(DatabaseError::Closed) => Err(LandslideError::Other(anyhow!(
                "DatabaseClient::put returned with error: {:?}.",
                dberr
            ))),
            Some(DatabaseError::NotFound) => Err(LandslideError::Other(anyhow!(
                "DatabaseClient::put returned with error: {:?}.",
                dberr
            ))),
            _ => Err(LandslideError::Other(anyhow!(
                "DatabaseClient::put returned with unknown error: {}.",
                put_response.err
            ))),
        }
    }

    #[allow(dead_code)]
    pub async fn delete(&mut self, key: Vec<u8>) -> Result<(), LandslideError> {
        let delete_response = self.db.delete(DeleteRequest { key }).await?.into_inner();

        let dberr = DatabaseError::from_u32(delete_response.err);
        match dberr {
            Some(DatabaseError::None) => Ok(()),
            Some(DatabaseError::Closed) => Err(LandslideError::Other(anyhow!(
                "DatabaseClient::delete returned with error: {:?}",
                dberr
            ))),
            Some(DatabaseError::NotFound) => Err(LandslideError::Other(anyhow!(
                "DatabaseClient::delete returned with error: {:?}",
                dberr
            ))),
            _ => Err(LandslideError::Other(anyhow!(
                "DatabaseClient::delete returned with unknown error: {}.",
                delete_response.err
            ))),
        }
    }

    pub async fn get_block(&mut self, block_id: &Id) -> Result<Option<Block>, LandslideError> {
        let key = Self::prefix(BLOCK_STATE_PREFIX, block_id.as_ref());
        let maybe_sb_bytes = self.get(key).await?;

        Ok(match maybe_sb_bytes {
            Some(sb_bytes) => serde_json::from_slice(&sb_bytes)?,
            None => None,
        })
    }

    pub async fn put_block(&mut self, mut block: Block) -> Result<(), LandslideError> {
        let value = serde_json::to_vec(&block)?;
        let key = Self::prefix(BLOCK_STATE_PREFIX, block.generate_id()?.as_ref());

        self.put(key, value).await
    }

    #[allow(dead_code)]
    pub async fn delete_block(&mut self, block_id: &Id) -> Result<(), LandslideError> {
        let key = Self::prefix(BLOCK_STATE_PREFIX, block_id.as_ref());
        self.delete(key).await
    }

    pub async fn get_last_accepted_block_id(&mut self) -> Result<Option<Id>, LandslideError> {
        match self.get(self.last_accepted_block_id_key.clone()).await? {
            Some(block_id_bytes) => Ok(Some(Id::from_slice(&block_id_bytes)?)),
            None => Ok(None),
        }
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
    parent_id: Id,
    height: u64,
    timestamp: Timestamp,
    data: [u8; BLOCK_DATA_LEN],

    pub status: Status,

    // Id should be generated, not serialized or deserialized
    #[serde(skip)]
    id: Option<Id>,
}

impl Block {
    pub fn new(
        parent_id: Id,
        height: u64,
        data: [u8; BLOCK_DATA_LEN],
        timestamp: OffsetDateTime,
        status: Status,
    ) -> Result<Self, LandslideError> {
        Ok(Block {
            parent_id,
            height,
            timestamp: Timestamp::from_offsetdatetime(timestamp)?,
            data,

            id: None,
            status,
        })
    }

    pub fn parent_id(&self) -> &Id {
        &self.parent_id
    }

    pub fn height(&self) -> u64 {
        self.height
    }

    pub fn timestamp(&self) -> &Timestamp {
        &self.timestamp
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn generate_id(&mut self) -> Result<&Id, LandslideError> {
        if self.id.is_none() {
            //generate bytes only for the stuff that makes an identity of the block
            let mut writer = Vec::new().writer();
            serde_json::to_writer(&mut writer, &self.parent_id())?;
            serde_json::to_writer(&mut writer, &self.height())?;
            serde_json::to_writer(&mut writer, &self.timestamp().bytes())?;
            serde_json::to_writer(&mut writer, &self.data())?;

            let buf = writer.into_inner();
            let block_id = Id::generate(&buf);
            self.id = Some(block_id);
        }

        Ok(self.id.as_ref().expect("in Block::id, the id was just set to Some(_) above and yet is still None. This is next to impossible."))
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
    #[allow(dead_code)]
    pub fn fetched(&self) -> bool {
        match self {
            Self::Processing => true,
            _ => self.decided(),
        }
    }

    #[allow(dead_code)]
    pub fn decided(&self) -> bool {
        matches!(self, Self::Rejected | Self::Accepted)
    }

    #[allow(dead_code)]
    pub fn valid(&self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

// Represents Timestamp as a binary-marshalled array of bytes,
// or as a Rust-native OffsetDateTime.
#[derive(Debug, Clone)]
pub struct Timestamp {
    bytes: Vec<u8>,

    // only serialize the bytes which are already serialized
    dt: OffsetDateTime,
}

impl Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(self.bytes())
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = Deserialize::deserialize(deserializer)?;
        Timestamp::from_bytes(v).map_err(D::Error::custom)
    }
}

impl Timestamp {
    pub fn from_offsetdatetime(dt: OffsetDateTime) -> Result<Self, LandslideError> {
        Ok(Timestamp {
            dt,
            bytes: Self::offsetdatetime_to_golang_binary_marshal_bytes(dt)?,
        })
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, LandslideError> {
        Ok(Timestamp {
            dt: Self::golang_binary_marshal_bytes_to_offsetdatetime(bytes.clone())?,
            bytes,
        })
    }

    pub fn offsetdatetime(&self) -> &OffsetDateTime {
        &self.dt
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    // Adapted from: https://cs.opensource.google/go/go/+/refs/tags/go1.17.6:src/time/time.go;l=1169
    // This is HIGHLY UNSTABLE and at the mercy of random go developers whims
    fn golang_binary_marshal_bytes_to_offsetdatetime(
        timestamp_bytes: Vec<u8>,
    ) -> Result<OffsetDateTime, LandslideError> {
        let mut bytes_reader = Cursor::new(timestamp_bytes);

        let version = bytes_reader.read_u8()
            .context("When conveting from Golang's Binary Marshal'd format to an OffsetDateTime, unable to read the format byte u8 from the timestamp byte vec.")?;

        if version != 1 {
            return Err(LandslideError::Other(anyhow!("When conveting from Golang's Binary Marshal'd format to an OffsetDateTime, did not recognize version {}. We only parse Version 1 of the format.", version)));
        }

        let golang_secs = bytes_reader.read_i64::<BigEndian>()
            .context("When conveting from Golang's Binary Marshal'd format to an OffsetDateTime, unable to read the BigEndian 64-bit integer seconds from the timestamp byte vec.")?;

        let golang_nanos = bytes_reader.read_i32::<BigEndian>()
            .context("When conveting from Golang's Binary Marshal'd format to an OffsetDateTime, unable to read the BigEndian 32-bit integer nanos from the timestamp byte vec.")?;

        let offset_mins_raw = bytes_reader.read_i16::<BigEndian>()
            .context("When conveting from Golang's Binary Marshal'd format to an OffsetDateTime, unable to read the BigEndian 16-bit integer offset minutes from the timestamp byte vec.")?;

        let offset_mins = match offset_mins_raw {
            -1 => 0, // if -1 (golang UTC) then convert to 0 (UTF for sane people)
            of => of,
        };

        let golang_nanos_whole: i128 = (golang_secs as i128) * 1000000000 + (golang_nanos as i128);

        let unix_timestamp_nanos = Self::nanos_from_unix_epoch(golang_nanos_whole);

        let offset = UtcOffset::from_whole_seconds((offset_mins as i32) * 60)
            .with_context(|| format!("When conveting from Golang's Binary Marshal'd format to an OffsetDateTime, unable to create an offset from minutes: {}", offset_mins))?;

        let dt_without_original_offset = OffsetDateTime::from_unix_timestamp_nanos(unix_timestamp_nanos)
        .with_context(|| format!("When conveting from Golang's Binary Marshal'd format to an OffsetDateTime, unable to convert unix timestamp nanoseconds {} into an OffsetDateTime", unix_timestamp_nanos))?;

        let dt_with_original_offset = dt_without_original_offset.to_offset(offset);

        Ok(dt_with_original_offset)
    }

    fn offsetdatetime_to_golang_binary_marshal_bytes(
        dt: OffsetDateTime,
    ) -> Result<Vec<u8>, LandslideError> {
        let offset_secs: i32 = dt.offset().whole_seconds();
        if offset_secs % 60 != 0 {
            return Err(LandslideError::Other(anyhow!("When converting OffsetDateTime to a Golang Binary Marshal'd format, offset had fractional minutes which is unsupported.")));
        }

        let offset_min: i16 = match offset_secs/60 {
            -1 => return Err(LandslideError::Other(anyhow!("When converting OffsetDateTime to a Golang Binary Marshal'd format, offset of -1 minutes is invalid for Golang Binary Marshaling since it is reserved for UTC. See: https://cs.opensource.google/go/go/+/refs/tags/go1.17.6:src/time/time.go;l=1170"))),
            0 => -1, // if 0 (sane-people UTC), then set -1 (golang UTC)
            of => i16::try_from(of)
                .with_context(|| format!("When converting OffsetDateTime to a Golang Binary Marshal'd format, unable to downcast i32 integer {} (the timezone offset in whole seconds) into an i16 integer.", of))?, // Keep the rest as-is
        };

        let golang_whole_nanos: i128 = Self::nanos_from_golang_zero(dt.unix_timestamp_nanos());

        // remove nanoseconds and cast to i64 as per golang
        let golang_secs = i64::try_from(golang_whole_nanos/1000000000)
            .with_context(||format!("When converting OffsetDateTime to a Golang Binary Marshal'd format, unable to downcast i128 integer {} (the seconds part of the whole nanos {}) into an i64 integer.", golang_whole_nanos/1000000000, golang_whole_nanos))?;

        // remove nanoseconds and cast to i32 as per golang
        let golang_nanos = i32::try_from(golang_whole_nanos%1000000000)
            .with_context(||format!("When converting OffsetDateTime to a Golang Binary Marshal'd format, unable to downcast i128 integer {} (the nanoseconds part of the whole nanos {}) into an i32 integer.", golang_whole_nanos%1000000000, golang_whole_nanos))?;

        // reserve 15 bytes for now  - 15 lines in the golang link above
        let mut bytes = Vec::with_capacity(15);
        bytes.push(1); // byte 0 is version: 1

        // All of this seems to be a bizarre hand-written Big-Endian encoding: https://cs.opensource.google/go/go/+/refs/tags/go1.17.6:src/time/time.go;l=1190
        bytes.write_i64::<BigEndian>(golang_secs)
            .with_context(|| format!("When converting OffsetDateTime to a Golang Binary Marshal'd format, unable to write 64-bit integer seconds {} to BigEndian", golang_secs))?;

        bytes.write_i32::<BigEndian>(golang_nanos)
            .with_context(|| format!("When converting OffsetDateTime to a Golang Binary Marshal'd format, unable to write 32-bit integer nanos {} to BigEndian", golang_nanos))?;

        bytes.write_i16::<BigEndian>(offset_min)
            .with_context(|| format!("When converting OffsetDateTime to a Golang Binary Marshal'd format, unable to write 16-bit integer offset minutes {} to BigEndian", offset_min))?;

        Ok(bytes)
    }

    fn nanos_from_golang_zero(nanos: i128) -> i128 {
        lazy_static! {
            // Golang's Zero time is January 1, year 1, 00:00:00.000000000 UTC
            // https://cs.opensource.google/go/go/+/refs/tags/go1.17.6:src/time/time.go;l=97
            // init that as an OffsetDateTime for future use
            static ref GOLANG_ZERO_DATETIME_NANOS_ABS: i128 = OffsetDateTime::parse(GOLANG_ZERO_DATETIME_STR, &Rfc3339)
                .with_context(|| format!("Unable to parse Golang's Zero DateTime, {}, into a rust OffsetDateTime", GOLANG_ZERO_DATETIME_STR)).unwrap().unix_timestamp_nanos().abs();
        }

        // Here's the logic.
        // 1. Incoming nanos are against a Unix Epoch of 0.
        // 2. Suppose Golang epoch is -10 compared to Unix Epoch.
        // 3. Suppose incoming nanos are 3, meaning Unix Epoch + 3
        // 4. We need to conver them into Golang Epoch + <something>
        // 5. Golang Epoch + <something> = Unix Epoch + 3
        //     Therefore, <something> = Unix Epoch + 3 - Golang Epoch
        //      Since we know Golang Epoch is negative (comes before Unix Epoch), and since we know Unix Epoch is "0",
        //      something = 3 + abs(golang epoch)

        *GOLANG_ZERO_DATETIME_NANOS_ABS + nanos
    }

    fn nanos_from_unix_epoch(nanos: i128) -> i128 {
        lazy_static! {
            // Golang's Zero time is January 1, year 1, 00:00:00.000000000 UTC
            // https://cs.opensource.google/go/go/+/refs/tags/go1.17.6:src/time/time.go;l=97
            // init that as an OffsetDateTime for future use
            static ref GOLANG_ZERO_DATETIME_NANOS_ABS: i128 = OffsetDateTime::parse(GOLANG_ZERO_DATETIME_STR, &Rfc3339)
                .with_context(|| format!("Unable to parse Golang's Zero DateTime, {}, into a rust OffsetDateTime", GOLANG_ZERO_DATETIME_STR)).unwrap().unix_timestamp_nanos().abs();
        }

        // Here's the logic.
        // 1. Incoming nanos are against a Golang Epoch of 0.
        // 2. Suppose Unix epoch is +10 compared to Golang Epoch.
        // 3. Suppose incoming nanos are 3, meaning Golang Epoch + 3
        // 4. We need to conver them into Unix Epoch + <something>
        // 5. Unix Epoch + <something> = Golang Epoch + 3
        //     Therefore, <something> = Golang Epoch + 3 - Unix Epoch
        //      Since we know Golang Epoch is negative (comes before Unix Epoch), and since we know Unix Epoch is "0",
        //      something = 3 + abs(golang epoch)

        nanos - *GOLANG_ZERO_DATETIME_NANOS_ABS
    }

    // This function should be used if/when Avalanche resolves this issue:
    // https://github.com/ava-labs/avalanchego/issues/1003
    #[allow(dead_code)]
    pub fn utc8_rfc3339_bytes_to_offsetdatetime(
        timestamp_bytes: Vec<u8>,
    ) -> Result<OffsetDateTime, LandslideError> {
        let rfc_str = String::from_utf8(timestamp_bytes).context(
            "Unable to parse timestamp as a UTF8 string, which is what the spec expects.",
        )?;

        Ok(OffsetDateTime::parse(&rfc_str, &Rfc3339)
            .with_context(|| format!("Failed to parse, what was expected to be an RFC3339 string, into a valid OffsetDateTime: {}", rfc_str))?)
    }

    // This function should be used if/when Avalanche resolves this issue:
    // https://github.com/ava-labs/avalanchego/issues/1003
    #[allow(dead_code)]
    pub fn offsetdatetime_to_utc8_rfc3339_bytes(
        dt: OffsetDateTime,
    ) -> Result<Vec<u8>, LandslideError> {
        let rfc_str = dt.format(&Rfc3339).with_context(|| {
            format!(
                "Failed to format the OffsetDateTime into an RFC3339 string: {}",
                dt
            )
        })?;
        Ok(rfc_str.into_bytes())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_epoch_conversions() {
        let unix_nanos = 500;
        let nanos = Timestamp::nanos_from_unix_epoch(Timestamp::nanos_from_golang_zero(unix_nanos));
        assert_eq!(unix_nanos, nanos);
    }

    #[tokio::test]
    async fn test_dt_conversions() {
        let dt = OffsetDateTime::now_utc();
        let newdt = Timestamp::golang_binary_marshal_bytes_to_offsetdatetime(
            Timestamp::offsetdatetime_to_golang_binary_marshal_bytes(dt).unwrap(),
        )
        .unwrap();
        assert_eq!(dt, newdt);
    }

    #[tokio::test]
    async fn test_dt_offset_conversions() {
        let dt = OffsetDateTime::now_utc().to_offset(UtcOffset::from_whole_seconds(300).unwrap());
        let newdt = Timestamp::golang_binary_marshal_bytes_to_offsetdatetime(
            Timestamp::offsetdatetime_to_golang_binary_marshal_bytes(dt).unwrap(),
        )
        .unwrap();
        assert_eq!(dt, newdt);
    }
}
