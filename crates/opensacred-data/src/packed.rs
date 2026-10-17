//! The binary files of the open formats: a version and a value, postcard-encoded, in a zlib
//! stream. A file of another version does not load: the converter writes it again.

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Why a binary file does not load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// The deflate stream is broken.
    Inflate(String),
    /// The file was written by another version of the encoding.
    Version { found: u32, expected: u32 },
    /// The postcard data is broken.
    Postcard(String),
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inflate(e) => write!(f, "deflate stream: {e}"),
            Self::Version { found, expected } => write!(
                f,
                "encoding version {found}, expected {expected}: convert again"
            ),
            Self::Postcard(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Most bytes a file may inflate to: a guard against a broken or hostile file.
const MAX_INFLATED: usize = 256 << 20;

/// `version` and `value`, postcard-encoded, in a zlib stream.
pub(crate) fn encode<T: Serialize>(version: u32, value: &T) -> Result<Vec<u8>, postcard::Error> {
    let mut bytes = postcard::to_allocvec(&version)?;
    bytes.extend(postcard::to_allocvec(value)?);
    Ok(miniz_oxide::deflate::compress_to_vec_zlib(&bytes, 6))
}

/// The value of a file that [`encode`] wrote with `version`.
pub(crate) fn decode<T: DeserializeOwned>(version: u32, file: &[u8]) -> Result<T, DecodeError> {
    let bytes = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(file, MAX_INFLATED)
        .map_err(|e| DecodeError::Inflate(e.to_string()))?;
    let (found, rest) = postcard::take_from_bytes::<u32>(&bytes)
        .map_err(|e| DecodeError::Postcard(e.to_string()))?;
    if found != version {
        return Err(DecodeError::Version {
            found,
            expected: version,
        });
    }
    postcard::from_bytes(rest).map_err(|e| DecodeError::Postcard(e.to_string()))
}
