// Copied from: https://github.com/ava-labs/avalanchego/blob/master/utils/formatting/encoding.go
use super::error::LandslideError;
use anyhow::anyhow;
use hmac_sha256::Hash;
use num_derive::FromPrimitive;
use serde::{Deserialize, Serialize};

const I32_MAX: usize = std::i32::MAX as usize;

const HEX_PREFIX: &str = "0x";
const CHECKSUM_LEN: usize = 4;

// Just computed this cause who knows whether float operations across
// two languages are identical and stable
const MAX_CB58_ENCODE_SIZE: usize = 2147483647;
const MAX_CB58_DECODE_SIZE: usize = 2932728742;

#[derive(Debug, FromPrimitive, Clone, Copy, Serialize, Deserialize)]
pub enum Checksum {
    Yes,
    No,
}

#[derive(Debug, FromPrimitive, Clone, Copy, Serialize, Deserialize)]
pub enum Encoding {
    #[serde(rename = "cb58")]
    Cb58 = 0,
    #[serde(rename = "hex")]
    Hex,
    #[serde(rename = "json")]
    Json,
}

impl Encoding {
    pub fn encode(&self, bytes: &[u8], checksum: Checksum) -> Result<String, LandslideError> {
        let checked_bytes = match checksum {
            Checksum::No => Vec::from(bytes),
            Checksum::Yes => {
                if bytes.len() > I32_MAX - CHECKSUM_LEN {
                    return Err(LandslideError::Encoding(anyhow!("Length of bytes to encode {} is greater than the maximum supported length {}", bytes.len(), MAX_CB58_ENCODE_SIZE)));
                }

                let checked: Vec<u8> =
                    [bytes, Self::checksum(bytes, CHECKSUM_LEN)?.as_ref()].concat();

                checked
            }
        };

        let encoded_str = match self {
            Self::Hex => format!("0x{}", hex::encode(checked_bytes)),
            Self::Cb58 => bs58::encode(bytes).into_string(),
            Self::Json => {
                return Err(LandslideError::Encoding(anyhow!(
                    "JSON encoding is not yet supported (neither in upstream Avalanche)"
                )))
            }
        };

        Ok(encoded_str)
    }

    pub fn decode(
        &self,
        encoded_str: String,
        checksum: Checksum,
    ) -> Result<Vec<u8>, LandslideError> {
        if encoded_str.is_empty() {
            return Ok(Vec::new());
        }

        let decoded_bytes = match self {
            Self::Hex => {
                if !encoded_str.starts_with(HEX_PREFIX) {
                    return Err(LandslideError::Encoding(anyhow!("The hexadecimal prefix 0x was not found for this string intended to be decoded as hex: {}", encoded_str)));
                }

                let un_prefixed_str = encoded_str.trim_start_matches(HEX_PREFIX);
                hex::decode(un_prefixed_str)?
            }
            Self::Cb58 => {
                if encoded_str.len() > MAX_CB58_DECODE_SIZE {
                    return Err(LandslideError::Encoding(anyhow!("The length of the encoded string {} is greater than the maximum decode size supported {}. This string is bound to fail.", encoded_str.len(), MAX_CB58_DECODE_SIZE)));
                }

                bs58::decode(encoded_str).into_vec()?
            }
            Self::Json => {
                return Err(LandslideError::Encoding(anyhow!(
                    "JSON decoding is not yet supported (neither in upstream Avalanche)"
                )))
            }
        };

        match checksum {
            Checksum::No => Ok(decoded_bytes),
            Checksum::Yes => {
                if decoded_bytes.len() < CHECKSUM_LEN {
                    return Err(LandslideError::Encoding(anyhow!("Length of decoded bytes {} was less than the length of the checksum {}, but a checksum verification was requested during decoding.", decoded_bytes.len(), CHECKSUM_LEN)));
                }

                let decoded_bytes_len = decoded_bytes.len();
                let raw_bytes_len = decoded_bytes.len() - CHECKSUM_LEN;
                let raw_bytes = Vec::from(&decoded_bytes[0..raw_bytes_len - 1]);

                let checksum_bytes =
                    Vec::from(&decoded_bytes[raw_bytes_len..decoded_bytes_len - 1]);
                let generated_checksum_bytes = Self::checksum(raw_bytes.as_ref(), CHECKSUM_LEN)?;

                if generated_checksum_bytes != checksum_bytes {
                    return Err(LandslideError::Encoding(anyhow!(
                        "Decoded checksum {:?} did not match the generated checksum {:?}",
                        checksum_bytes,
                        generated_checksum_bytes
                    )));
                }

                Ok(raw_bytes)
            }
        }
    }

    pub fn checksum(bytes: &[u8], len: usize) -> Result<Vec<u8>, LandslideError> {
        let mut checksum = Vec::from(Hash::hash(bytes));
        if checksum.len() < len {
            return Err(LandslideError::Encoding(anyhow!(
                "SHA256 length was 32 but requested checkum length {} was larger than that.",
                len
            )));
        }
        // clip down to expected checksum length
        checksum.resize(CHECKSUM_LEN, 0);

        Ok(checksum)
    }
}
