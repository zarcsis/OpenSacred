//! Common reader of PAK containers (docs/02 §1): a 0x100-byte header, a directory of
//! `count × 12` bytes `(type, offset, size)` with absolute offsets, then the records. Record
//! semantics belong to the format module (for example, [`crate::tex`]).

use crate::error::{ErrorKind, Result, to_usize};
use crate::reader::Reader;

/// Size of the container header.
pub const HEADER_SIZE: usize = 0x100;
/// Size of a directory entry.
pub const DIR_ENTRY_SIZE: usize = 12;
/// Number of u32 header fields after `magic` and `version`; field 0 is the directory length.
pub const HEADER_FIELDS: usize = 63;

/// Directory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirEntry {
    /// Record type; for TEX it equals the texture format.
    pub kind: u32,
    /// Offset of the record from the start of the file; 0 marks an empty entry.
    pub offset: u32,
    /// Record size; the format defines what it covers (TEX excludes the record header).
    pub size: u32,
}

impl DirEntry {
    /// Empty entry: "no object" (`INVALID_*`), usually entry 0.
    pub fn is_empty(&self) -> bool {
        self.offset == 0
    }
}

/// Parsed container: header and directory; record data stays in the source slice.
#[derive(Debug)]
pub struct Pak<'a> {
    data: &'a [u8],
    magic: [u8; 3],
    version: u8,
    fields: [u32; HEADER_FIELDS],
    entries: Vec<DirEntry>,
}

impl<'a> Pak<'a> {
    /// Parses the header and directory of any container without checking the magic.
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let mut header = r.take(HEADER_SIZE, "PAK header")?;
        let magic = header.array("magic")?;
        let version = header.u8()?;
        let mut fields = [0; HEADER_FIELDS];
        for field in &mut fields {
            *field = header.u32()?;
        }

        let count = to_usize(fields[0]);
        // Bounds of the whole directory come first, so that a count read from the file does not
        // decide how much memory gets allocated.
        let dir_len = count.saturating_mul(DIR_ENTRY_SIZE);
        let mut dir = r.take(dir_len, "PAK directory")?;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            entries.push(DirEntry {
                kind: dir.u32()?,
                offset: dir.u32()?,
                size: dir.u32()?,
            });
        }
        Ok(Self {
            data,
            magic,
            version,
            fields,
            entries,
        })
    }

    /// Parses the container and checks its magic and version.
    pub fn parse_expecting(data: &'a [u8], magic: [u8; 3], version: u8) -> Result<Self> {
        let pak = Self::parse(data)?;
        if pak.magic != magic || pak.version != version {
            return Err(Reader::new(data).error(ErrorKind::Magic {
                expected: magic,
                expected_version: version,
                found: pak.magic,
                found_version: pak.version,
            }));
        }
        Ok(pak)
    }

    /// The whole file.
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    pub fn magic(&self) -> [u8; 3] {
        self.magic
    }

    pub fn version(&self) -> u8 {
        self.version
    }

    /// u32 header fields after `magic` and `version`, starting at offset 4.
    pub fn header_fields(&self) -> &[u32; HEADER_FIELDS] {
        &self.fields
    }

    pub fn entries(&self) -> &[DirEntry] {
        &self.entries
    }

    /// Directory entry `index`.
    pub fn entry(&self, index: usize) -> Result<DirEntry> {
        self.entries.get(index).copied().ok_or_else(|| {
            Reader::new(self.data).with_record(index).error_at(
                self.entry_offset(index),
                ErrorKind::NoRecord {
                    count: self.entries.len(),
                },
            )
        })
    }

    /// Offset of directory entry `index` from the start of the file.
    pub fn entry_offset(&self, index: usize) -> usize {
        index
            .saturating_mul(DIR_ENTRY_SIZE)
            .saturating_add(HEADER_SIZE)
    }

    /// Reader from the start of record `index` to the end of the file; errors carry the index.
    pub fn record_reader(&self, index: usize, entry: DirEntry) -> Reader<'a> {
        Reader::at(self.data, to_usize(entry.offset)).with_record(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{PakRecord, build_pak};

    #[test]
    fn parses_header_and_directory() {
        let data = build_pak(
            *b"TST",
            7,
            &[
                None,
                Some(PakRecord::new(5, b"abc")),
                Some(PakRecord::new(9, b"defg")),
            ],
        );
        let pak = Pak::parse_expecting(&data, *b"TST", 7).unwrap();
        assert_eq!(pak.header_fields()[0], 3);
        let entries = pak.entries();
        assert_eq!(entries.len(), 3);
        assert!(entries[0].is_empty());
        let first = HEADER_SIZE + 3 * DIR_ENTRY_SIZE;
        assert_eq!(
            entries[1],
            DirEntry {
                kind: 5,
                offset: u32::try_from(first).unwrap(),
                size: 3
            }
        );
        assert_eq!(entries[2].offset, entries[1].offset + 3);
        let mut r = pak.record_reader(2, entries[2]);
        assert_eq!(r.bytes(4, "record").unwrap(), b"defg");
    }

    #[test]
    fn short_header_is_an_error() {
        let err = Pak::parse(&[0; HEADER_SIZE - 1]).unwrap_err();
        assert_eq!(err.place.offset, 0);
        assert!(matches!(
            err.kind,
            ErrorKind::OutOfBounds {
                what: "PAK header",
                ..
            }
        ));
    }

    #[test]
    fn directory_past_the_end_is_an_error() {
        let mut data = build_pak(*b"TST", 1, &[Some(PakRecord::new(1, b"x"))]);
        // More entries than the file can hold.
        data[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        let err = Pak::parse(&data).unwrap_err();
        assert_eq!(err.place.offset, HEADER_SIZE);
        assert!(matches!(
            err.kind,
            ErrorKind::OutOfBounds {
                what: "PAK directory",
                ..
            }
        ));
    }

    #[test]
    fn wrong_magic_or_version_is_an_error() {
        let data = build_pak(*b"TEX", 2, &[]);
        let err = Pak::parse_expecting(&data, *b"TEX", 3).unwrap_err();
        assert_eq!(
            err.to_string(),
            "offset 0x0: expected magic `TEX` version 3, found `TEX` version 2"
        );
        assert!(Pak::parse_expecting(&data, *b"MDL", 2).is_err());
    }

    #[test]
    fn missing_record_is_an_error() {
        let data = build_pak(*b"TST", 1, &[None]);
        let pak = Pak::parse(&data).unwrap();
        let err = pak.entry(1).unwrap_err();
        assert_eq!(err.place.record, Some(1));
        assert_eq!(err.place.offset, HEADER_SIZE + DIR_ENTRY_SIZE);
        assert!(matches!(err.kind, ErrorKind::NoRecord { count: 1 }));
    }
}
