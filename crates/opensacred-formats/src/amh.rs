//! The AMH container of the hero templates `Templates/Hero*.ptx` and the saves `save/hero*.pax`
//! (docs/02 §9): a 0x100-byte header, a table of 16 slots `{u32 tag; u32 offset; u32 size}` at
//! 0x100, and the sections of the used slots back to back to the end of the file, each stored or
//! zlib-compressed behind a 32-byte prefix. Confirmed on every section, and on the combat arts
//! of the creature section, by `tools/analysis/formats/amh_dump.py`.

use crate::error::{ErrorKind, Result, field, to_usize};
use crate::reader::Reader;
use crate::zlib::inflate;

const MAGIC: [u8; 4] = *b"AMH\x1b";
const HEADER: usize = 0x100;
const SLOTS: usize = 16;
const COMPRESSED: u32 = 0xBAAD_C0DE;
const PREFIX: usize = 32;
/// The creature section: the dump of the hero's creature, with its combat arts.
pub const CREATURE: u32 = 0xC7;
/// The summary section: level, class and name.
pub const SUMMARY: u32 = 0xC3;
/// The inventory section: a block for each item the hero carries.
pub const INVENTORY: u32 = 0xC8;
/// The backpack's section: an inventory container whose grid names its items by their ids.
pub const BACKPACK: u32 = 0xCA;
/// A stored section whose 64 bytes on disk stand for a table size of 556.
const MINI: u32 = 0xC4;
/// The marker of an item's block in the inventory section, after the item's type, and how far
/// from it the block holds the object's id (`u16`), the type again and the item's record.
const ITEM_MARK: [u8; 4] = 0xFEED_F00Du32.to_le_bytes();
const ITEM_ID: usize = 10;
const ITEM_TYPE: usize = 14;
const ITEM_RECORD: usize = 252;
/// The size of an item's record (docs/re/items.md), as `Weapon.pak +0x80…+0x101` holds one.
pub const RECORD: usize = 0x82;
/// The backpack's grid: after six `u16` and `0x400` bytes, 256 cells of 12 bytes, each an item's
/// id as a `u32` or 0.
const GRID: usize = 12 + 0x400;
const CELL: usize = 12;
const CELLS: usize = 256;
const MINI_ON_DISK: usize = 64;
/// Where the creature section holds the count of its combat arts (`u16`), the entries following
/// it (docs/re/arts.md, "The heroes' starting arts").
const ARTS: usize = 1227;
/// A combat art's entry, as a creature's stats hold it (docs/re/arts.md, "A creature's arts").
pub const ART: usize = 22;
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

/// A combat art of the creature section: an entry of the stats' list (docs/re/arts.md, "A
/// creature's arts").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Art {
    /// `+0x00`: 1 the first table, the spells; 2 the second, the weapon arts.
    pub table: u32,
    /// `+0x04`: the art's id.
    pub id: u16,
    /// `+0x06`.
    pub level: u8,
    /// `+0x07`: a bonus to the level, of the items and the sets.
    pub bonus: u8,
    /// `+0x08`: bit 0 set on an art a rune taught (`0x580750`, `0x581F80`), the high byte the
    /// shortcut's slot.
    pub flags: u16,
    /// `+0x0A`: its regeneration time, in seconds.
    pub time: f32,
    /// `+0x0E`: its regeneration's factor.
    pub factor: f32,
    /// `+0x12`: the regeneration still to run, in seconds.
    pub remaining: f32,
}

/// An item of the inventory section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// The object's id, which the backpack's grid names it by.
    pub id: u32,
    /// Its type: its `items.pak` slot and `Weapon.pak` record.
    pub item: u32,
    /// Its record, [`RECORD`] bytes from its type.
    pub record: Vec<u8>,
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

    fn section(&self, tag: u32, what: &'static str) -> Result<&Section> {
        self.sections
            .iter()
            .find(|s| s.tag == tag)
            .ok_or_else(|| Reader::new(&[]).error(field(what, 0, "one".into())))
    }

    /// The items of the inventory section, in its order: each block holds the item's type as a
    /// `u32`, the marker `0xFEEDF00D`, the object's id 10 bytes after the marker and the type
    /// again 14 bytes after it, and the item's record 252 bytes after it, which begins with the
    /// type too.
    pub fn items(&self) -> Result<Vec<Item>> {
        let data = &self.section(INVENTORY, "AMH inventory section")?.data;
        let file = Reader::new(data);
        let mut items = Vec::new();
        for at in (4..data.len().saturating_sub(3)).filter(|&at| data[at..].starts_with(&ITEM_MARK))
        {
            let item = Reader::at(data, at - 4).u32()?;
            let id = Reader::at(data, at + ITEM_ID).u16()?;
            let mut record = file.window(at + ITEM_RECORD, RECORD, "AMH item record")?;
            for (offset, found) in [
                (ITEM_TYPE, Reader::at(data, at + ITEM_TYPE).u32()?),
                (ITEM_RECORD, record.clone().u32()?),
            ] {
                if found != item {
                    return Err(file.error_at(
                        at + offset,
                        field("AMH item block's type", found.into(), item.to_string()),
                    ));
                }
            }
            items.push(Item {
                id: id.into(),
                item,
                record: record.bytes(RECORD, "AMH item record")?.to_vec(),
            });
        }
        Ok(items)
    }

    /// The combat arts of the creature section: a `u16` count at its byte 1,227, then the
    /// entries of 22 bytes; the table of each is 1 or 2.
    pub fn arts(&self) -> Result<Vec<Art>> {
        let data = &self.section(CREATURE, "AMH creature section")?.data;
        let mut r = Reader::at(data, ARTS);
        let count = r.u16()?;
        (0..count)
            .map(|k| {
                let mut entry = r.take(ART, "AMH combat art")?.with_record(usize::from(k));
                let at = entry.offset();
                let table = entry.u32()?;
                if !(1..=2).contains(&table) {
                    return Err(entry.error_at(
                        at,
                        field("AMH combat art's table", table.into(), "1 or 2".into()),
                    ));
                }
                Ok(Art {
                    table,
                    id: entry.u16()?,
                    level: entry.u8()?,
                    bonus: entry.u8()?,
                    flags: entry.u16()?,
                    time: entry.f32()?,
                    factor: entry.f32()?,
                    remaining: entry.f32()?,
                })
            })
            .collect()
    }

    /// The ids of the items in the backpack's grid, cell by cell.
    pub fn backpack(&self) -> Result<Vec<u32>> {
        let data = &self.section(BACKPACK, "AMH backpack section")?.data;
        let mut grid = Reader::new(data).window(GRID, CELL * CELLS, "AMH backpack grid")?;
        let mut ids = Vec::new();
        for _ in 0..CELLS {
            let mut cell = grid.take(CELL, "AMH backpack cell")?;
            match cell.u32()? {
                0 => {}
                id => ids.push(id),
            }
        }
        Ok(ids)
    }

    /// The decoded summary section.
    pub fn summary(&self) -> Result<Summary> {
        let section = self.section(SUMMARY, "AMH summary section")?;
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

    /// An inventory of two items' blocks, a backpack holding the second, as the templates have.
    fn inventory() -> Amh {
        let mut inventory = 30u32.to_le_bytes().to_vec();
        for (id, item, least) in [(17u16, 7901u32, 21u16), (19, 5171, 0)] {
            let at = inventory.len() + 4;
            inventory.extend(item.to_le_bytes());
            inventory.extend(ITEM_MARK);
            inventory.resize(at + ITEM_RECORD + RECORD + 60, 0);
            inventory[at + ITEM_ID..at + ITEM_ID + 2].copy_from_slice(&id.to_le_bytes());
            inventory[at + ITEM_TYPE..at + ITEM_TYPE + 4].copy_from_slice(&item.to_le_bytes());
            let record = at + ITEM_RECORD;
            inventory[record..record + 4].copy_from_slice(&item.to_le_bytes());
            inventory[record + 0x1C..record + 0x1E].copy_from_slice(&least.to_le_bytes());
        }
        let mut backpack = vec![0; 4444];
        backpack[GRID + 3 * CELL] = 19;
        let section = |tag, data: Vec<u8>| Section {
            slot: 3,
            tag,
            size: u32::try_from(data.len()).unwrap(),
            compressed: true,
            data,
        };
        Amh {
            sections: vec![section(INVENTORY, inventory), section(BACKPACK, backpack)],
        }
    }

    #[test]
    fn the_inventory_holds_the_items_and_the_backpack_names_them() {
        let amh = inventory();
        let items = amh.items().unwrap();
        let found: Vec<_> = items
            .iter()
            .map(|i| (i.id, i.item, i.record.len(), i.record[0x1C]))
            .collect();
        assert_eq!(found, [(17, 7901, RECORD, 21), (19, 5171, RECORD, 0)]);
        assert_eq!(amh.backpack().unwrap(), [19]);
        // A block whose record is of another type is an error.
        let mut broken = inventory();
        broken.sections[0].data[4 + 4 + ITEM_RECORD] = 0;
        assert!(
            broken
                .items()
                .unwrap_err()
                .to_string()
                .contains("`AMH item block's type`")
        );
    }

    /// A creature section of the Seraphim's template's size, with her two arts.
    fn creature(arts: &[(u32, u16, u16, f32)]) -> Amh {
        let mut data = vec![0; ARTS];
        data.extend(u16::try_from(arts.len()).unwrap().to_le_bytes());
        for &(table, id, flags, time) in arts {
            data.extend(table.to_le_bytes());
            data.extend(id.to_le_bytes());
            data.extend([1, 0]);
            data.extend(flags.to_le_bytes());
            for value in [time, 1.0, 0.0] {
                data.extend(value.to_le_bytes());
            }
        }
        data.resize(1881, 0);
        Amh {
            sections: vec![Section {
                slot: 0,
                tag: CREATURE,
                size: 1881,
                compressed: true,
                data,
            }],
        }
    }

    #[test]
    fn the_creature_section_holds_the_combat_arts() {
        let amh = creature(&[(2, 1071, 0, 15.0), (1, 18, 1, 24.0)]);
        let arts = amh.arts().unwrap();
        assert_eq!(
            arts[1],
            Art {
                table: 1,
                id: 18,
                level: 1,
                bonus: 0,
                flags: 1,
                time: 24.0,
                factor: 1.0,
                remaining: 0.0,
            }
        );
        assert_eq!((arts.len(), arts[0].id, arts[0].table), (2, 1071, 2));
        let error = creature(&[(3, 1071, 0, 15.0)]).arts().unwrap_err();
        assert_eq!(error.place.record, Some(0));
        assert!(error.to_string().contains("table"), "{error}");
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
