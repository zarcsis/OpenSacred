//! The AMH container of the hero templates `Templates/Hero*.ptx` and the saves `save/hero*.pax`
//! (docs/02 §9): a 0x100-byte header, a table of 16 slots `{u32 tag; u32 offset; u32 size}` at
//! 0x100, and the sections of the used slots back to back to the end of the file, each stored or
//! zlib-compressed behind a 32-byte prefix. Confirmed on every section by
//! `tools/analysis/formats/amh_dump.py`.

use crate::error::{ErrorKind, Result, field, to_usize};
use crate::reader::Reader;
use crate::zlib::inflate;

const MAGIC: [u8; 4] = *b"AMH\x1b";
const HEADER: usize = 0x100;
const SLOTS: usize = 16;
const COMPRESSED: u32 = 0xBAAD_C0DE;
const PREFIX: usize = 32;
/// The summary section: level, class and name.
pub const SUMMARY: u32 = 0xC3;
/// A stored section whose 64 bytes on disk stand for a table size of 556.
const MINI: u32 = 0xC4;
const MINI_ON_DISK: usize = 64;
/// The header words that are not zero (offset, value), the same in every file.
const HEADER_WORDS: [(usize, u32); 6] = [
    (4, 0x10),
    (8, 0x22C),
    (0x1C, 1),
    (0x20, 9),
    (0x24, 2),
    (0x28, 20_000),
];

/// A hero file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Amh {
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// The slot of the table, 0–15.
    pub slot: usize,
    pub tag: u32,
    /// The size in the table: the inflated size of a compressed section.
    pub size: u32,
    pub compressed: bool,
    /// The inflated bytes, or the stored bytes on disk (64 for tag 0xC4).
    pub data: Vec<u8>,
}

/// The summary section, 556 bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub level: u32,
    /// 1–9 (docs/02 §6.1).
    pub class: u32,
    /// Empty in the templates.
    pub name: String,
}

impl Amh {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let file = Reader::new(data);
        let mut r = file.clone();
        let magic: [u8; 4] = r.array("AMH magic")?;
        if magic != MAGIC {
            return Err(file.error(ErrorKind::Signature { what: "AMH" }));
        }
        for at in (4..HEADER).step_by(4) {
            let word = Reader::at(data, at).u32()?;
            let expected = HEADER_WORDS
                .iter()
                .find(|&&(o, _)| o == at)
                .map_or(0, |&(_, v)| v);
            if word != expected {
                return Err(file.error_at(
                    at,
                    field("AMH header word", word.into(), expected.to_string()),
                ));
            }
        }
        let mut table = Reader::at(data, HEADER);
        let mut slots = Vec::new();
        for slot in 0..SLOTS {
            let at = table.offset();
            let entry = [table.u32()?, table.u32()?, table.u32()?];
            if entry[1] != 0 {
                slots.push((slot, entry, at));
            } else if entry != [0; 3] {
                return Err(file.error_at(
                    at,
                    field("unused AMH slot", entry[0].into(), "zeros".into()),
                ));
            }
        }
        let mut expected_at = HEADER + 12 * SLOTS;
        let mut sections = Vec::with_capacity(slots.len());
        for (k, &(slot, [tag, offset, size], at)) in slots.iter().enumerate() {
            let offset = to_usize(offset);
            if offset != expected_at {
                return Err(file.error_at(
                    at + 4,
                    field(
                        "AMH section offset",
                        u64::try_from(offset).unwrap_or(u64::MAX),
                        format!("{expected_at:#x}, after the previous section"),
                    ),
                ));
            }
            let end = slots
                .get(k + 1)
                .map_or(data.len(), |&(_, [_, next, _], _)| to_usize(next));
            let len = end.checked_sub(offset).unwrap_or(usize::MAX);
            let block = file.window(offset, len, "AMH section")?;
            let bytes = block.rest();
            let compressed = bytes.get(..4) == Some(&COMPRESSED.to_le_bytes());
            let data = if compressed {
                let mut b = block.clone();
                b.u32()?;
                let csize = to_usize(b.u32()?);
                if bytes[8..PREFIX.min(bytes.len())].iter().any(|&x| x != 0)
                    || PREFIX.checked_add(csize) != Some(len)
                {
                    return Err(block.error(field(
                        "AMH compressed section",
                        u64::try_from(csize).unwrap_or(u64::MAX),
                        "a 32-byte prefix and a stream that fill the section".into(),
                    )));
                }
                inflate(&bytes[PREFIX..], to_usize(size))
                    .map_err(|kind| block.error_at(offset + PREFIX, kind))?
            } else {
                let on_disk = if tag == MINI {
                    MINI_ON_DISK
                } else {
                    to_usize(size)
                };
                if len != on_disk {
                    return Err(block.error(field(
                        "AMH stored section",
                        u64::try_from(len).unwrap_or(u64::MAX),
                        format!("{on_disk} bytes"),
                    )));
                }
                bytes.to_vec()
            };
            sections.push(Section {
                slot,
                tag,
                size,
                compressed,
                data,
            });
            expected_at = end;
        }
        Ok(Self { sections })
    }

    /// The decoded summary section.
    pub fn summary(&self) -> Result<Summary> {
        let section = self
            .sections
            .iter()
            .find(|s| s.tag == SUMMARY)
            .ok_or_else(|| Reader::new(&[]).error(field("AMH summary section", 0, "one".into())))?;
        let mut r = Reader::new(&section.data);
        let level = r.u32()?;
        let class = r.u32()?;
        let units: Vec<u16> = r
            .bytes(64, "hero name")?
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&c| u16::from_le_bytes(c))
            .take_while(|&u| u != 0)
            .collect();
        let name = String::from_utf16(&units)
            .map_err(|_| r.error_at(8, field("hero name", 0, "UTF-16".into())))?;
        Ok(Summary { level, class, name })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file with a compressed section in slot 0, the summary in slot 4 and the 64-byte 0xC4.
    fn file() -> Vec<u8> {
        let mut d = vec![0; HEADER + 12 * SLOTS];
        d[..4].copy_from_slice(&MAGIC);
        for (at, value) in HEADER_WORDS {
            d[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        let raw = vec![7u8; 1881];
        let stream = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6);
        let mut compressed = COMPRESSED.to_le_bytes().to_vec();
        compressed.extend(u32::try_from(stream.len()).unwrap().to_le_bytes());
        compressed.extend([0; 24]);
        compressed.extend(stream);
        let mut summary = vec![0; 556];
        summary[0] = 29;
        summary[4] = 8;
        for (i, unit) in "Ilja".encode_utf16().enumerate() {
            summary[8 + 2 * i..10 + 2 * i].copy_from_slice(&unit.to_le_bytes());
        }
        let sections: [(usize, u32, u32, Vec<u8>); 3] = [
            (0, 0xC7, 1881, compressed),
            (4, SUMMARY, 556, summary),
            (5, MINI, 556, vec![1; 64]),
        ];
        for (slot, tag, size, bytes) in sections {
            let entry = HEADER + 12 * slot;
            let offset = u32::try_from(d.len()).unwrap();
            for (i, v) in [tag, offset, size].into_iter().enumerate() {
                d[entry + 4 * i..entry + 4 * i + 4].copy_from_slice(&v.to_le_bytes());
            }
            d.extend(bytes);
        }
        d
    }

    #[test]
    fn sections_inflate_and_the_summary_decodes() {
        let amh = Amh::parse(&file()).unwrap();
        let s: Vec<_> = amh
            .sections
            .iter()
            .map(|s| (s.slot, s.tag, s.compressed, s.data.len()))
            .collect();
        assert_eq!(
            s,
            [
                (0, 0xC7, true, 1881),
                (4, SUMMARY, false, 556),
                (5, MINI, false, 64)
            ]
        );
        assert_eq!(
            amh.summary().unwrap(),
            Summary {
                level: 29,
                class: 8,
                name: "Ilja".into()
            }
        );
    }

    #[test]
    fn broken_files_are_errors() {
        let error = |d: &[u8]| Amh::parse(d).unwrap_err().to_string();
        let mut d = file();
        d[0x30] = 1;
        assert!(error(&d).contains("`AMH header word` is 1"));
        let mut d = file();
        d.pop();
        assert!(error(&d).contains("`AMH stored section` is 63"));
        let mut d = file();
        d[HEADER + 12 * 7] = 5;
        assert!(error(&d).contains("`unused AMH slot` is 5"));
    }
}

#[cfg(test)]
mod game_tests;
