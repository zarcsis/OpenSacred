//! TEX textures: `PAK/texture.pak` and `PAK/texture03.pak` (docs/02 §3.1).
//!
//! A record is an 80-byte header followed by data: format 4 is ARGB4444 (`D3DFMT_A4R4G4B4`) in a
//! zlib stream, format 6 is uncompressed `D3DFMT_A8R8G8B8`, bytes B, G, R, A. There are no mip
//! levels; rows go top to bottom. The directory `size` excludes the record header. The decoder
//! produces RGBA8.

use crate::error::{ErrorKind, Result, field, to_usize};
use crate::pak::Pak;
use crate::reader::Reader;
use crate::zlib::inflate;

pub const MAGIC: [u8; 3] = *b"TEX";
pub const VERSION: u8 = 3;
/// Record header; data starts at +0x50.
pub const RECORD_HEADER_SIZE: usize = 0x50;
pub const NAME_LEN: usize = 32;

/// Pixel format of a record (byte +0x24).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    /// 4: 16-bit ARGB4444, u16 LE with alpha in the high nibble; zlib stream.
    Argb4444Zlib,
    /// 6: 32-bit, bytes B, G, R, A; uncompressed.
    Bgra8888,
}

impl Format {
    /// Format code in the file.
    pub fn code(self) -> u8 {
        match self {
            Self::Argb4444Zlib => 4,
            Self::Bgra8888 => 6,
        }
    }

    fn from_code(code: u8) -> Option<Self> {
        match code {
            4 => Some(Self::Argb4444Zlib),
            6 => Some(Self::Bgra8888),
            _ => None,
        }
    }
}

/// Record header (80 bytes, packed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordHeader {
    name: [u8; NAME_LEN],
    pub width: u16,
    pub height: u16,
    pub format: Format,
    /// Format 4: size of the zlib stream (the unpacked size for `MIX*.444` atlases, see
    /// [`TexPak::record`]); format 6: `w·h·4`. Equals the directory `size`.
    pub data_size: u32,
    /// `(offset << 8) | pakIndex`; zero in the pak, filled in only in `Texture.TMP`.
    pub packed_offset: u64,
}

impl RecordHeader {
    /// Name (`*.TGA` or `MIXnnnn.444`, any case, cp1252) up to the first NUL. A name without
    /// NUL fills all 32 bytes: `texture.pak` has seven such names, all truncated (`….T`).
    pub fn name(&self) -> &[u8] {
        self.name.split(|&b| b == 0).next().unwrap_or_default()
    }
}

/// Texture in RGBA8, rows top to bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// TEX container, version 3.
#[derive(Debug)]
pub struct TexPak<'a> {
    pak: Pak<'a>,
}

impl<'a> TexPak<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        Pak::parse_expecting(data, MAGIC, VERSION).map(|pak| Self { pak })
    }

    pub fn pak(&self) -> &Pak<'a> {
        &self.pak
    }

    /// Number of directory entries, empty ones included.
    pub fn len(&self) -> usize {
        self.pak.entries().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Record `index`: its header checked against the directory, and the bounds of its data.
    /// `None` for an empty directory entry.
    pub fn record(&self, index: usize) -> Result<Option<Record<'a>>> {
        let entry = self.pak.entry(index)?;
        if entry.is_empty() {
            return Ok(None);
        }
        let mut r = self.pak.record_reader(index, entry);
        let mut h = r.take(RECORD_HEADER_SIZE, "TEX record header")?;
        let name = h.array("name")?; // +0x00
        let width = h.u16()?; // +0x20
        let height = h.u16()?; // +0x22
        let format_at = h.offset();
        let code = h.u8()?; // +0x24
        let data_size_at = h.offset();
        let data_size = h.u32()?; // +0x25
        let packed_offset = h.u64()?; // +0x29; zeros up to +0x50

        let format = Format::from_code(code)
            .ok_or_else(|| h.error_at(format_at, field("format", code.into(), "4 or 6".into())))?;
        if entry.kind != u32::from(code) {
            return Err(h.error_at(
                self.pak.entry_offset(index),
                field(
                    "directory type",
                    entry.kind.into(),
                    format!("record format {code}"),
                ),
            ));
        }
        if data_size != entry.size {
            return Err(h.error_at(
                data_size_at,
                field(
                    "dataSize",
                    data_size.into(),
                    format!("directory size {}", entry.size),
                ),
            ));
        }

        let payload = match format {
            // A zlib stream knows its own length, so `dataSize` is only an upper bound for it:
            // `MIX*.444` atlases store the unpacked size 131 072 there while the stream is 3–5
            // times shorter, and the last three atlases of `texture.pak` run past the end of the
            // file with that size.
            Format::Argb4444Zlib => r.take_up_to(to_usize(data_size)),
            Format::Bgra8888 => {
                let expected = image_bytes(width, height, 4)
                    .ok_or_else(|| h.error(ErrorKind::ImageTooLarge { width, height }))?;
                if to_usize(data_size) != expected {
                    return Err(h.error_at(
                        data_size_at,
                        field("dataSize", data_size.into(), format!("{width}*{height}*4")),
                    ));
                }
                r.take(expected, "BGRA8888 pixels")?
            }
        };
        Ok(Some(Record {
            index,
            header: RecordHeader {
                name,
                width,
                height,
                format,
                data_size,
                packed_offset,
            },
            payload,
        }))
    }
}

/// Non-empty container record.
#[derive(Debug, Clone)]
pub struct Record<'a> {
    pub index: usize,
    pub header: RecordHeader,
    /// Record data from +0x50; for format 4, no longer than `dataSize` and the end of the file.
    payload: Reader<'a>,
}

impl Record<'_> {
    /// Unpacks the record data and converts it to RGBA8.
    pub fn decode(&self) -> Result<Texture> {
        let RecordHeader {
            width,
            height,
            format,
            ..
        } = self.header;
        let too_large = || {
            self.payload
                .error(ErrorKind::ImageTooLarge { width, height })
        };
        let rgba_len = image_bytes(width, height, 4).ok_or_else(too_large)?;
        let mut rgba = vec![0; rgba_len];
        let (out, _) = rgba.as_chunks_mut::<4>();
        match format {
            Format::Argb4444Zlib => {
                let expected = image_bytes(width, height, 2).ok_or_else(too_large)?;
                let raw = inflate(self.payload.rest(), expected)
                    .map_err(|kind| self.payload.error(kind))?;
                let (pixels, _) = raw.as_chunks::<2>();
                for (out, &[lo, hi]) in out.iter_mut().zip(pixels) {
                    // u16 LE: the high byte is A·16 + R, the low byte is G·16 + B.
                    *out = [
                        expand4(hi & 0xF),
                        expand4(lo >> 4),
                        expand4(lo & 0xF),
                        expand4(hi >> 4),
                    ];
                }
            }
            Format::Bgra8888 => {
                let (pixels, _) = self.payload.rest().as_chunks::<4>();
                for (out, &[b, g, r, a]) in out.iter_mut().zip(pixels) {
                    *out = [r, g, b, a];
                }
            }
        }
        Ok(Texture {
            width: width.into(),
            height: height.into(),
            rgba,
        })
    }
}

/// 4 bits → 8: `n·17 = (n << 4) | n`, as `dec4444` in `tools/analysis/formats/tex_decode.py`.
fn expand4(nibble: u8) -> u8 {
    nibble * 17
}

/// Image size in bytes; `None` if it does not fit in `usize`.
fn image_bytes(width: u16, height: u16, bytes_per_pixel: usize) -> Option<usize> {
    usize::from(width)
        .checked_mul(usize::from(height))?
        .checked_mul(bytes_per_pixel)
}

#[cfg(test)]
mod game_tests;

#[cfg(test)]
mod tests {
    use miniz_oxide::deflate::compress_to_vec_zlib;

    use super::*;
    use crate::pak::{DIR_ENTRY_SIZE, HEADER_SIZE};
    use crate::testing::{PakRecord, build_pak, tex_record};

    fn tex_pak(records: &[Option<PakRecord>]) -> Vec<u8> {
        build_pak(MAGIC, VERSION, records)
    }

    /// Offset of the first record in a container of `count` entries.
    fn first_record(count: usize) -> usize {
        HEADER_SIZE + count * DIR_ENTRY_SIZE
    }

    fn argb4444(name: &[u8], width: u16, height: u16, pixels: &[u16]) -> PakRecord {
        let raw: Vec<u8> = pixels.iter().flat_map(|p| p.to_le_bytes()).collect();
        let stream = compress_to_vec_zlib(&raw, 6);
        tex_record(
            name,
            width,
            height,
            4,
            u32::try_from(stream.len()).unwrap(),
            &stream,
        )
    }

    fn decode(data: &[u8], index: usize) -> Result<Texture> {
        TexPak::parse(data)?.record(index)?.unwrap().decode()
    }

    #[test]
    fn decodes_argb4444_with_alpha_in_the_high_nibble() {
        let data = tex_pak(&[
            None,
            Some(argb4444(b"A.TGA", 2, 2, &[0xF84C, 0x0000, 0xFFFF, 0x1234])),
        ]);
        let pak = TexPak::parse(&data).unwrap();
        assert_eq!(pak.len(), 2);
        assert!(pak.record(0).unwrap().is_none());

        let record = pak.record(1).unwrap().unwrap();
        assert_eq!(record.header.name(), b"A.TGA");
        assert_eq!(record.header.format, Format::Argb4444Zlib);
        assert_eq!(
            record.decode().unwrap(),
            Texture {
                width: 2,
                height: 2,
                rgba: vec![
                    136, 68, 204, 255, // 0xF84C
                    0, 0, 0, 0, // 0x0000
                    255, 255, 255, 255, // 0xFFFF
                    34, 51, 68, 17, // 0x1234
                ],
            }
        );
    }

    #[test]
    fn decodes_bgra8888_by_swapping_blue_and_red() {
        let data = tex_pak(&[Some(tex_record(
            b"UI.TGA",
            2,
            1,
            6,
            8,
            &[1, 2, 3, 4, 5, 6, 7, 8],
        ))]);
        let texture = decode(&data, 0).unwrap();
        assert_eq!((texture.width, texture.height), (2, 1));
        assert_eq!(texture.rgba, [3, 2, 1, 4, 7, 6, 5, 8]);
    }

    #[test]
    fn name_without_nul_fills_all_32_bytes() {
        let long = b"HORSE_BRIDLE_LEATHER_METAL01.TGA";
        assert_eq!(long.len(), NAME_LEN);
        let data = tex_pak(&[Some(argb4444(long, 1, 1, &[0]))]);
        let record = TexPak::parse(&data).unwrap().record(0).unwrap().unwrap();
        assert_eq!(record.header.name(), long);
    }

    #[test]
    fn stream_shorter_than_data_size_may_run_past_the_end_of_the_file() {
        // Like `MIX*.444` atlases: `dataSize` is the unpacked size, the stream is much shorter.
        // The first record overlaps the second by its size, the second runs past the end.
        let zeros = vec![0u16; 16 * 16];
        let atlas = |name: &[u8]| {
            let mut record = argb4444(name, 16, 16, &zeros);
            record.size = 16 * 16 * 2;
            record.bytes[0x25..0x29].copy_from_slice(&record.size.to_le_bytes());
            record
        };
        let data = tex_pak(&[Some(atlas(b"MIX0.444")), Some(atlas(b"MIX1.444"))]);
        assert!(data.len() < first_record(2) + RECORD_HEADER_SIZE + 512);
        for index in 0..2 {
            let texture = decode(&data, index).unwrap();
            assert_eq!(texture.rgba, vec![0; 16 * 16 * 4]);
        }
    }

    #[test]
    fn unknown_format_names_record_and_field_offset() {
        let data = tex_pak(&[None, Some(tex_record(b"X.TGA", 1, 1, 5, 0, &[]))]);
        let err = TexPak::parse(&data).unwrap().record(1).unwrap_err();
        assert_eq!(err.place.record, Some(1));
        assert_eq!(err.place.offset, first_record(2) + 0x24);
        assert_eq!(
            err.to_string(),
            format!(
                "record 1, offset {:#x}: field `format` is 5, expected 4 or 6",
                first_record(2) + 0x24
            )
        );
    }

    #[test]
    fn directory_type_must_match_format() {
        let mut record = argb4444(b"X.TGA", 1, 1, &[0]);
        record.kind = 6;
        let data = tex_pak(&[Some(record)]);
        let err = TexPak::parse(&data).unwrap().record(0).unwrap_err();
        assert_eq!(err.place.offset, HEADER_SIZE);
        assert!(matches!(
            err.kind,
            ErrorKind::Field {
                field: "directory type",
                found: 6,
                ..
            }
        ));
    }

    #[test]
    fn data_size_must_match_directory_size() {
        let mut record = argb4444(b"X.TGA", 1, 1, &[0]);
        record.size += 1;
        let data = tex_pak(&[Some(record)]);
        let err = TexPak::parse(&data).unwrap().record(0).unwrap_err();
        assert_eq!(err.place.offset, first_record(1) + 0x25);
        assert!(matches!(
            err.kind,
            ErrorKind::Field {
                field: "dataSize",
                ..
            }
        ));
    }

    #[test]
    fn bgra8888_data_size_must_be_width_height_4() {
        let data = tex_pak(&[Some(tex_record(b"X.TGA", 2, 2, 6, 4, &[0; 4]))]);
        let err = TexPak::parse(&data).unwrap().record(0).unwrap_err();
        assert!(matches!(
            err.kind,
            ErrorKind::Field {
                field: "dataSize",
                found: 4,
                ..
            }
        ));
    }

    #[test]
    fn bgra8888_pixels_past_the_end_are_an_error() {
        let mut data = tex_pak(&[Some(tex_record(b"X.TGA", 2, 2, 6, 16, &[0; 16]))]);
        data.truncate(data.len() - 1);
        let err = TexPak::parse(&data).unwrap().record(0).unwrap_err();
        assert_eq!(err.place.offset, first_record(1) + RECORD_HEADER_SIZE);
        assert!(matches!(
            err.kind,
            ErrorKind::OutOfBounds {
                what: "BGRA8888 pixels",
                len: 16,
                ..
            }
        ));
    }

    #[test]
    fn record_header_past_the_end_is_an_error() {
        let mut data = tex_pak(&[Some(argb4444(b"X.TGA", 1, 1, &[0]))]);
        data.truncate(first_record(1) + RECORD_HEADER_SIZE - 1);
        let err = TexPak::parse(&data).unwrap().record(0).unwrap_err();
        assert_eq!(err.place.offset, first_record(1));
        assert!(matches!(
            err.kind,
            ErrorKind::OutOfBounds {
                what: "TEX record header",
                ..
            }
        ));
    }

    #[test]
    fn truncated_stream_is_an_error() {
        let pixels: Vec<u16> = (0..64).map(|i| i * 1021).collect();
        let mut data = tex_pak(&[Some(argb4444(b"X.TGA", 8, 8, &pixels))]);
        data.truncate(data.len() - 8);
        let err = decode(&data, 0).unwrap_err();
        assert_eq!(err.place.offset, first_record(1) + RECORD_HEADER_SIZE);
        assert!(matches!(err.kind, ErrorKind::Zlib(_)), "{err}");
    }

    #[test]
    fn corrupt_stream_is_an_error() {
        let data = tex_pak(&[Some(tex_record(b"X.TGA", 1, 1, 4, 4, b"junk"))]);
        let err = decode(&data, 0).unwrap_err();
        assert!(matches!(err.kind, ErrorKind::Zlib(_)), "{err}");
    }

    #[test]
    fn stream_must_unpack_to_exactly_width_height_2() {
        let longer = tex_pak(&[Some(argb4444(b"X.TGA", 1, 1, &[0, 0]))]);
        assert!(matches!(
            decode(&longer, 0).unwrap_err().kind,
            ErrorKind::UnpackedTooLong { expected: 2 }
        ));
        let shorter = tex_pak(&[Some(argb4444(b"X.TGA", 2, 1, &[0]))]);
        assert!(matches!(
            decode(&shorter, 0).unwrap_err().kind,
            ErrorKind::UnpackedTooShort {
                expected: 4,
                actual: 2
            }
        ));
    }
}
