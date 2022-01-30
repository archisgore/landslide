
use serde::{Serialize, Deserialize};
use hmac_sha256::Hash;
use super::error::LandslideError;
use std::str::FromStr;
use std::string::ToString;
use hex::ToHex;
use std::convert::TryInto;
use zerocopy::{AsBytes, FromBytes, Unaligned};

#[derive(Serialize, Deserialize, AsBytes, FromBytes, Unaligned)]
#[repr(transparent)]
pub struct Id([u8; 32]);

impl AsRef<[u8]> for Id {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

const BITS_PER_BYTE: usize = 8;

impl Id {
    // ToID attempt to convert a byte slice into an id
    pub fn new(bytes: &[u8]) -> Result<Id, LandslideError> {
        Ok(Id(Hash::hash(bytes)))
    }

       // Bit returns the bit value at the ith index of the byte array. Returns 0 or 1
       pub fn bit(&self, i: usize) -> bool {
        let byteIndex = i / BITS_PER_BYTE;
        let bitIndex = i % BITS_PER_BYTE;

        let b = self.0[byteIndex];

        // b = [7, 6, 5, 4, 3, 2, 1, 0]

        let b = b >> bitIndex;

        // b = [0, ..., bitIndex + 1, bitIndex]
        // 1 = [0, 0, 0, 0, 0, 0, 0, 1]

        let b = b & 1;

        // b = [0, 0, 0, 0, 0, 0, 0, bitIndex]

        match b {
            0 => false,
            _ => true,
        }
    }

}

impl FromStr for Id {
    type Err = LandslideError;

    // from_string is the inverse of ID.to_string()
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = hex::decode(s)?;
        
        let newid: [u8; 32] = match bytes.try_into() {
            Ok(n) => n,
            Err(e) => return Err(LandslideError::Generic(format!("Deserializing ID from string {}, resulted in {} bytes, when we expected exactly 32.", s, bytes.len())))
        };

        Ok(Id(newid))
    }
}

impl ToString for Id {
    fn to_string(&self) -> std::string::String {
        self.0.encode_hex()
    }
}


