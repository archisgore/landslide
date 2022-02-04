use super::error::LandslideError;
use hex::ToHex;
use hmac_sha256::Hash;
use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::str::FromStr;
use zerocopy::{AsBytes, FromBytes, Unaligned};

pub const BYTE_LENGTH: usize = 32;
pub const ZERO_ID: Id = Id([0; BYTE_LENGTH]);

#[derive(
    Debug, Serialize, Deserialize, AsBytes, FromBytes, Unaligned, Hash, PartialEq, Eq, Clone,
)]
#[repr(transparent)]
pub struct Id([u8; BYTE_LENGTH]);

impl AsRef<[u8]> for Id {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

const BITS_PER_BYTE: usize = 8;

impl Id {
    // Create an Id wrapping over an array if bytes
    pub fn from_bytes(bytes: [u8; BYTE_LENGTH]) -> Result<Id, LandslideError> {
        Ok(Id(bytes))
    }

    // Generate an Id for an arbitrary set of bytes.
    pub fn generate(bytes: &[u8]) -> Result<Id, LandslideError> {
        Ok(Id::from_bytes(Hash::hash(bytes))?)
    }

    // Bit returns the bit value at the ith index of the byte array. Returns 0 or 1
    pub fn bit(&self, i: usize) -> bool {
        let byte_index = i / BITS_PER_BYTE;
        let bit_index = i % BITS_PER_BYTE;

        let b = self.0[byte_index];

        // b = [7, 6, 5, 4, 3, 2, 1, 0]

        let b = b >> bit_index;

        // b = [0, ..., bitIndex + 1, bitIndex]
        // 1 = [0, 0, 0, 0, 0, 0, 0, 1]

        let b = b & 1;

        // b = [0, 0, 0, 0, 0, 0, 0, bitIndex]

        !matches!(b, 0)
    }
}

impl FromStr for Id {
    type Err = LandslideError;

    // from_string is the inverse of ID.to_string()
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = hex::decode(s)?;

        let newid: [u8; BYTE_LENGTH] = match bytes.try_into() {
            Ok(n) => n,
            Err(err) => {
                return Err(LandslideError::Generic(format!(
                    "Error when deserializing ID from string {}. Error: {:?}",
                    s, err
                )))
            }
        };

        Ok(Id(newid))
    }
}

impl Display for Id {
    fn fmt(&self, w: &mut Formatter) -> FmtResult {
        write!(w, "{}", self.0.encode_hex::<String>())
    }
}
