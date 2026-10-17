//! Parse error: what is wrong and where — the container record and the offset from the start
//! of the file. The caller adds the file name: this crate parses slices and knows no paths.

use std::fmt;

/// Error while parsing an original format.
#[derive(Debug, thiserror::Error)]
#[error("{place}: {kind}")]
pub struct Error {
    pub place: Place,
    pub kind: ErrorKind,
}

/// Where in the file the error is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    /// Index of the container directory entry, if the error belongs to a record.
    pub record: Option<usize>,
    /// Offset from the start of the file.
    pub offset: usize,
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(record) = self.record {
            write!(f, "record {record}, ")?;
        }
        write!(f, "offset {:#x}", self.offset)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorKind {
    #[error("{what} of {len} bytes runs past the end of the data at {end:#x}")]
    OutOfBounds {
        what: &'static str,
        len: usize,
        end: usize,
    },
    #[error(
        "expected magic `{}` version {expected_version}, found `{}` version {found_version}",
        expected.escape_ascii(),
        found.escape_ascii()
    )]
    Magic {
        expected: [u8; 3],
        expected_version: u8,
        found: [u8; 3],
        found_version: u8,
    },
    #[error("{what} signature does not match")]
    Signature { what: &'static str },
    #[error("{what}: stored CRC-32 {stored:#010x}, computed {computed:#010x}")]
    Checksum {
        what: &'static str,
        stored: u32,
        computed: u32,
    },
    #[error("no such record, the directory has {count}")]
    NoRecord { count: usize },
    /// A tree format (GRN) holds a part some other number of times than once.
    #[error("{what}: {found} with tag {tag:#010x}, expected one")]
    TagCount {
        what: &'static str,
        tag: u32,
        found: usize,
    },
    /// A list in a tree format (GRN) holds a node of another kind.
    #[error("{what}: tag {found:#010x}, expected {expected:#010x}")]
    Tag {
        what: &'static str,
        found: u32,
        expected: u32,
    },
    #[error("{what}: no single value of property `{key}`")]
    NoProperty {
        what: &'static str,
        key: &'static str,
    },
    #[error("field `{field}` is {found}, expected {expected}")]
    Field {
        field: &'static str,
        found: u64,
        expected: String,
    },
    /// The times of an animation curve (GRN) do not rise strictly from 0.
    #[error("time of key {key} is {found}, expected {expected}")]
    Time {
        key: usize,
        found: f32,
        expected: String,
    },
    #[error("image {width}x{height} does not fit in memory")]
    ImageTooLarge { width: u16, height: u16 },
    /// Message of `miniz_oxide`; the partially unpacked data is dropped.
    #[error("zlib stream: {0}")]
    Zlib(String),
    #[error("zlib stream unpacks to more than {expected} bytes")]
    UnpackedTooLong { expected: usize },
    #[error("zlib stream unpacks to {actual} bytes, expected {expected}")]
    UnpackedTooShort { expected: usize, actual: usize },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// A field value that does not fit in `usize` necessarily points past the end of any slice,
/// so saturating is safe: such an offset or length fails the bounds check on read.
pub(crate) fn to_usize(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

/// [`ErrorKind::Field`]: a record field holds a value the format does not allow.
pub(crate) fn field(field: &'static str, found: u64, expected: String) -> ErrorKind {
    ErrorKind::Field {
        field,
        found,
        expected,
    }
}
