//! zlib streams of the containers (TEX textures, world sectors).

use miniz_oxide::inflate::{TINFLStatus, decompress_to_vec_zlib_with_limit};

use crate::error::ErrorKind;

/// Unpacks a zlib stream into exactly `expected` bytes; bytes after the end of the stream are
/// not read.
pub(crate) fn inflate(stream: &[u8], expected: usize) -> Result<Vec<u8>, ErrorKind> {
    match decompress_to_vec_zlib_with_limit(stream, expected) {
        Ok(raw) if raw.len() == expected => Ok(raw),
        Ok(raw) => Err(ErrorKind::UnpackedTooShort {
            expected,
            actual: raw.len(),
        }),
        Err(e) if e.status == TINFLStatus::HasMoreOutput => {
            Err(ErrorKind::UnpackedTooLong { expected })
        }
        Err(e) => Err(ErrorKind::Zlib(e.to_string())),
    }
}
