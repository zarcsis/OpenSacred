//! Reader of little-endian fields from a file held in memory. Every read checks bounds; an error
//! names the record and the offset from the start of the file instead of panicking.
//!
//! On-disk structures are packed (`#pragma pack(1)`, docs/02 §1), so fields are read in order
//! without alignment: a record layout is a sequence of reads.

use crate::error::{Error, ErrorKind, Place, Result};

/// Window `[pos, end)` into the file data with the cursor at the start of the window.
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    /// The whole file: error offsets are counted from its start.
    data: &'a [u8],
    pos: usize,
    end: usize,
    record: Option<usize>,
}

impl<'a> Reader<'a> {
    /// Window over the whole file.
    pub fn new(data: &'a [u8]) -> Self {
        Self::at(data, 0)
    }

    /// Window from `offset` to the end of the file. An offset past the end fails on first read.
    pub fn at(data: &'a [u8], offset: usize) -> Self {
        Self {
            data,
            pos: offset,
            end: data.len(),
            record: None,
        }
    }

    /// Tags errors from this window with a container record index.
    pub fn with_record(self, record: usize) -> Self {
        Self {
            record: Some(record),
            ..self
        }
    }

    /// Cursor offset from the start of the file.
    pub fn offset(&self) -> usize {
        self.pos
    }

    /// Unread bytes of the window.
    pub fn rest(&self) -> &'a [u8] {
        self.data.get(self.pos..self.end).unwrap_or_default()
    }

    /// Splits the next `len` bytes off into a window of their own and moves the cursor past them.
    pub fn take(&mut self, len: usize, what: &'static str) -> Result<Reader<'a>> {
        // The cursor may sit past the end of the window (`Reader::at`): fail even for `len == 0`.
        if self
            .end
            .checked_sub(self.pos)
            .is_none_or(|available| len > available)
        {
            return Err(self.out_of_bounds(what, len));
        }
        let window = Self {
            end: self.pos + len,
            ..self.clone()
        };
        self.pos += len;
        Ok(window)
    }

    /// Window of `len` bytes that starts `offset` bytes after the cursor; the cursor stays. For
    /// formats that address their parts by offsets (GRN sections and nodes).
    pub fn window(&self, offset: usize, len: usize, what: &'static str) -> Result<Reader<'a>> {
        Self {
            pos: self.pos.saturating_add(offset),
            ..self.clone()
        }
        .take(len, what)
    }

    /// Like [`Reader::take`], but stops at the end of the window. Only for streams that know
    /// their own length (zlib), where `len` is an upper bound rather than the exact size.
    pub fn take_up_to(&mut self, len: usize) -> Reader<'a> {
        let len = len.min(self.rest().len());
        let window = Self {
            end: self.pos + len,
            ..self.clone()
        };
        self.pos += len;
        window
    }

    pub fn bytes(&mut self, len: usize, what: &'static str) -> Result<&'a [u8]> {
        Ok(self.take(len, what)?.rest())
    }

    pub fn array<const N: usize>(&mut self, what: &'static str) -> Result<[u8; N]> {
        match self.rest().first_chunk::<N>() {
            Some(&array) => {
                self.pos += N;
                Ok(array)
            }
            None => Err(self.out_of_bounds(what, N)),
        }
    }

    pub fn u8(&mut self) -> Result<u8> {
        self.array::<1>("u8 field").map(|[b]| b)
    }

    pub fn u16(&mut self) -> Result<u16> {
        self.array("u16 field").map(u16::from_le_bytes)
    }

    pub fn u32(&mut self) -> Result<u32> {
        self.array("u32 field").map(u32::from_le_bytes)
    }

    pub fn u64(&mut self) -> Result<u64> {
        self.array("u64 field").map(u64::from_le_bytes)
    }

    pub fn f32(&mut self) -> Result<f32> {
        self.array("f32 field").map(f32::from_le_bytes)
    }

    /// `N` consecutive `f32` fields: a vector, a quaternion, a matrix.
    pub fn f32s<const N: usize>(&mut self) -> Result<[f32; N]> {
        let mut values = [0.0; N];
        for value in &mut values {
            *value = self.f32()?;
        }
        Ok(values)
    }

    /// Error at the cursor.
    pub fn error(&self, kind: ErrorKind) -> Error {
        self.error_at(self.pos, kind)
    }

    /// Error at `offset` from the start of the file, tagged with this window's record.
    pub fn error_at(&self, offset: usize, kind: ErrorKind) -> Error {
        Error {
            place: Place {
                record: self.record,
                offset,
            },
            kind,
        }
    }

    fn out_of_bounds(&self, what: &'static str, len: usize) -> Error {
        self.error(ErrorKind::OutOfBounds {
            what,
            len,
            end: self.end,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_little_endian_fields_in_order() {
        let data = [
            1, 0x34, 0x12, 0x78, 0x56, 0x34, 0x12, 8, 7, 6, 5, 4, 3, 2, 1, 0xAA,
        ];
        let mut r = Reader::new(&data);
        assert_eq!(r.u8().unwrap(), 1);
        assert_eq!(r.u16().unwrap(), 0x1234);
        assert_eq!(r.u32().unwrap(), 0x1234_5678);
        assert_eq!(r.u64().unwrap(), 0x0102_0304_0506_0708);
        assert_eq!(r.offset(), 15);
        assert_eq!(r.rest(), [0xAA]);
    }

    #[test]
    fn reads_f32_fields() {
        let data: Vec<u8> = [1.0f32, -0.5, 2.25]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let mut r = Reader::new(&data);
        assert_eq!(r.f32().unwrap(), 1.0);
        assert_eq!(r.f32s::<2>().unwrap(), [-0.5, 2.25]);
        assert!(r.f32s::<1>().is_err());
    }

    #[test]
    fn short_read_names_offset_record_and_end() {
        let data = [0; 10];
        let mut r = Reader::at(&data, 8).with_record(3);
        let err = r.u32().unwrap_err();
        assert_eq!(
            err.place,
            Place {
                record: Some(3),
                offset: 8
            }
        );
        assert!(matches!(
            err.kind,
            ErrorKind::OutOfBounds {
                len: 4,
                end: 10,
                ..
            }
        ));
        assert_eq!(
            err.to_string(),
            "record 3, offset 0x8: u32 field of 4 bytes runs past the end of the data at 0xa"
        );
        // A failed read does not move the cursor.
        assert_eq!(r.offset(), 8);
        assert_eq!(r.u16().unwrap(), 0);
    }

    #[test]
    fn offset_past_the_end_fails_on_first_read() {
        let data = [0; 4];
        let mut r = Reader::at(&data, usize::MAX);
        assert!(r.rest().is_empty());
        assert!(r.u8().is_err());
        assert!(r.take(0, "nothing").is_err());
        assert!(r.take_up_to(10).rest().is_empty());
    }

    #[test]
    fn take_limits_the_window() {
        let data = [1, 2, 3, 4, 5];
        let mut r = Reader::new(&data);
        let mut head = r.take(2, "head").unwrap();
        assert_eq!(head.u16().unwrap(), 0x0201);
        let err = head.u8().unwrap_err();
        assert!(matches!(err.kind, ErrorKind::OutOfBounds { end: 2, .. }));
        assert_eq!(r.offset(), 2);
        assert!(r.take(4, "tail").is_err());
        assert_eq!(r.take_up_to(100).rest(), [3, 4, 5]);
        assert!(r.rest().is_empty());
    }

    #[test]
    fn window_is_relative_to_the_cursor_and_keeps_it() {
        let data = [0, 1, 2, 3, 4, 5];
        let mut r = Reader::new(&data);
        r.u8().unwrap();
        let mut w = r.window(2, 3, "part").unwrap();
        assert_eq!((w.offset(), w.rest()), (3, [3, 4, 5].as_slice()));
        assert!(w.window(0, 4, "past").is_err());
        assert_eq!(r.offset(), 1);
        let err = r.window(usize::MAX, 1, "far").unwrap_err();
        assert_eq!(err.place.offset, usize::MAX);
        assert_eq!(w.u8().unwrap(), 3);
    }

    #[test]
    fn huge_length_does_not_overflow() {
        let data = [0; 4];
        let mut r = Reader::at(&data, 2);
        assert!(r.bytes(usize::MAX, "huge").is_err());
        assert_eq!(r.take_up_to(usize::MAX).rest(), [0, 0]);
    }
}
