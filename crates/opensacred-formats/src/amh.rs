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
/// The summary section: level, class and name, the items worn and the difficulties won.
pub const SUMMARY: u32 = 0xC3;
/// The equipment's slots of a summary: their count (`u32`, 19 in every file) and the types of
/// the items in them (`u32` each, 0 for none) after it (docs/02 §9).
const EQUIPMENT_COUNT: usize = 0x88;
pub const EQUIPMENT: usize = 19;
/// A summary's byte for each difficulty the hero won, bronze's first, six of them (docs/re/fight.md,
/// "What opens the levels").
const WON: usize = 0xED;
pub const WON_COUNT: usize = 6;
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
/// The key of the object's name (`+0x3C`), 51 bytes after the marker.
const ITEM_NAME: usize = 51;
const ITEM_RECORD: usize = 252;
/// The count of a block's socketed items' records (`u8`), which come before the item's own, and
/// the marker before it (docs/02 §9).
const ITEM_SOCKETS: usize = 251;
const SOCKETS_MARK: [u8; 4] = 0xBAAD_C0DEu32.to_le_bytes();
/// The size of an item's record (docs/re/items.md), as `Weapon.pak +0x80…+0x101` holds one.
pub const RECORD: usize = 0x82;
/// The backpack's grid: after six `u16`, the fifth its width, and `0x400` bytes, 256 cells of 12
/// bytes, row by row, the top left cell of each item holding its id as a `u32` (0 none), a byte 1,
/// its width and height in cells, a byte 0 and its type as a `u32`.
const GRID: usize = 12 + 0x400;
const WIDTH: usize = 8;
const CELL: usize = 12;
const CELLS: usize = 256;
const MINI_ON_DISK: usize = 64;
/// The weapon sets after the backpack's grid: five sets of two halves, the left hand's and the
/// right's, of 12 bytes each, a half's first `u32` its item's object id, 0 for none (docs/02 §9).
pub const WEAPON_SETS: usize = 5;
const HALF: usize = 12;
/// The bar's art slots after the weapon sets: five slots in two banks, a record of 20 bytes each,
/// the slot `s` of the bank `b` the record `2s + b`, its kind (0 none, 2 a spell, 3 a weapon art,
/// 4 a combo) and its art's id the first two `u32`; then the selections, `u16` each: the weapon
/// set selected and the art slot selected (the container's `+0xC90` and `+0xD58`, docs/02 §9).
pub const ART_SLOTS: usize = 5;
const ART_SLOT: usize = 20;
/// An art slot's two banks, each its kind and its art's id.
pub type ArtSlot = [(u32, u32); 2];
/// Where the creature section holds the count of its combat arts (`u16`), the entries following
/// it (docs/re/arts.md, "The heroes' starting arts").
const ARTS: usize = 1227;
/// A combat art's entry, as a creature's stats hold it (docs/re/arts.md, "A creature's arts").
pub const ART: usize = 22;
/// The creature section's markers at their places: the object's, the creature's and its stats'
/// (docs/02 §9, "The creature").
const CREATURE_MARKS: [(usize, u32); 3] =
    [(16, 0xFEED_F00D), (263, 0xC0DE_F00D), (961, 0xDEAD_BEEF)];
/// The equipment's slots, 19 object ids, in the creature section.
const EQUIPMENT_SLOTS: usize = 435;
/// The creature section's byte of the stats' `+0`: the stats' `+x` is at `STATS + x`.
const STATS: usize = 981;
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
    /// `+0x8C`: the type of the item in each of the hero's 19 equipment slots, 0 for none, what
    /// the choice of the hero's pedestal dresses its figure in (`0x6F4E60`).
    pub equipment: [u32; EQUIPMENT],
    /// `+0xED`: whether the hero won each difficulty, bronze's first (`0x6F4F20`).
    pub won: [bool; WON_COUNT],
}

/// The stats of the creature section (docs/02 §9, "The creature"): the stats' `+0x08`…`+0x56`
/// that a hero's values are made of again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    /// `+0x08`: the class, the hero's creature type.
    pub class: u32,
    /// `+0x0C`.
    pub experience: u32,
    /// `+0x10`: the attributes as the stats made them, with the items'.
    pub attributes: [u16; 6],
    /// `+0x1C`: the attribute points spent on each.
    pub spent: [u8; 6],
    /// `+0x23`: the attribute points still to spend.
    pub attribute_points: u8,
    /// `+0x24`: the skill of each slot, 0 for none.
    pub skills: [u8; 8],
    /// `+0x2C`: each slot's base.
    pub bases: [u8; 8],
    /// `+0x34`: the items' bonus to each slot.
    pub bonuses: [u8; 8],
    /// `+0x42`: the skill points still to spend.
    pub skill_points: u16,
    /// `+0x44`: the skills still to learn.
    pub to_learn: u16,
    /// `+0x46`.
    pub gold: u32,
    /// `+0x4A`: the base attributes, the class's at level 1.
    pub base: [u16; 6],
    /// `+0x56`.
    pub level: u16,
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
    /// The records of the items in its sockets, [`RECORD`] bytes each, in their order.
    pub socketed: Vec<Vec<u8>>,
    /// The key of its name (the object's `+0x3C`): the hash of the name that the item maker
    /// composed for it with the bit 31 set, 0 for its type's name (docs/02 §9).
    pub name: u32,
}

/// An item in the backpack's grid, at its top left cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Packed {
    /// The object's id, as the inventory section names it ([`Item::id`]).
    pub id: u32,
    /// Its top left cell, column and row.
    pub at: (u8, u8),
    /// Its width and height in cells.
    pub size: (u8, u8),
    /// Its type, as the inventory section has it ([`Item::item`]).
    pub item: u32,
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
    /// again 14 bytes after it, the marker `0xBAADC0DE` and the count of its socketed items 247
    /// and 251 bytes after it, then each socketed item's record and the item's own, which begins
    /// with the type too.
    pub fn items(&self) -> Result<Vec<Item>> {
        let data = &self.section(INVENTORY, "AMH inventory section")?.data;
        let file = Reader::new(data);
        let mut items = Vec::new();
        for at in (4..data.len().saturating_sub(3)).filter(|&at| data[at..].starts_with(&ITEM_MARK))
        {
            let item = Reader::at(data, at - 4).u32()?;
            let id = Reader::at(data, at + ITEM_ID).u16()?;
            let mark = Reader::at(data, at + ITEM_SOCKETS - 4).u32()?;
            if mark.to_le_bytes() != SOCKETS_MARK {
                return Err(file.error_at(
                    at + ITEM_SOCKETS - 4,
                    field(
                        "AMH item block's sockets' marker",
                        mark.into(),
                        "0xBAADC0DE".into(),
                    ),
                ));
            }
            let sockets = usize::from(*data.get(at + ITEM_SOCKETS).unwrap_or(&0));
            let own = ITEM_RECORD + sockets * RECORD;
            let mut record = file.window(at + own, RECORD, "AMH item record")?;
            // The record's own type is the item's in every template, but one item of the shipped
            // `hero07.pax` holds another's (docs/02 §9), which the hero keeps.
            let found = Reader::at(data, at + ITEM_TYPE).u32()?;
            if found != item {
                return Err(file.error_at(
                    at + ITEM_TYPE,
                    field("AMH item block's type", found.into(), item.to_string()),
                ));
            }
            let socketed = (0..sockets)
                .map(|k| {
                    file.window(at + ITEM_RECORD + k * RECORD, RECORD, "AMH socketed record")
                        .and_then(|mut r| {
                            r.bytes(RECORD, "AMH socketed record").map(<[u8]>::to_vec)
                        })
                })
                .collect::<Result<_>>()?;
            items.push(Item {
                id: id.into(),
                item,
                record: record.bytes(RECORD, "AMH item record")?.to_vec(),
                socketed,
                name: Reader::at(data, at + ITEM_NAME).u32()?,
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

    /// The weapon sets of the backpack's section: each set's halves, the left hand's and the
    /// right's, by their items' object ids, 0 for none.
    pub fn sets(&self) -> Result<[[u32; 2]; WEAPON_SETS]> {
        let data = &self.section(BACKPACK, "AMH backpack section")?.data;
        let mut sets = [[0; 2]; WEAPON_SETS];
        for (k, half) in sets.iter_mut().flatten().enumerate() {
            *half = Reader::at(data, GRID + CELL * CELLS + HALF * k).u32()?;
        }
        Ok(sets)
    }

    /// The bar's art slots of the backpack's section, each slot's two banks' kind and art, and the
    /// selections: the weapon set selected and the art slot selected.
    pub fn art_slots(&self) -> Result<([ArtSlot; ART_SLOTS], [u16; 2])> {
        let data = &self.section(BACKPACK, "AMH backpack section")?.data;
        let at = GRID + CELL * CELLS + HALF * 2 * WEAPON_SETS;
        let mut slots = [[(0, 0); 2]; ART_SLOTS];
        for (k, slot) in slots.iter_mut().flatten().enumerate() {
            let mut r = Reader::at(data, at + ART_SLOT * k);
            *slot = (r.u32()?, r.u32()?);
        }
        let mut r = Reader::at(data, at + ART_SLOT * 2 * ART_SLOTS);
        Ok((slots, [r.u16()?, r.u16()?]))
    }

    /// The creature section with its markers checked at their places.
    fn creature(&self) -> Result<&[u8]> {
        let data = &self.section(CREATURE, "AMH creature section")?.data;
        for (at, mark) in CREATURE_MARKS {
            let found = Reader::at(data, at).u32()?;
            if found != mark {
                return Err(Reader::at(data, at).error_at(
                    at,
                    field(
                        "AMH creature section's marker",
                        found.into(),
                        format!("{mark:#010X}"),
                    ),
                ));
            }
        }
        Ok(data)
    }

    /// The stats of the creature section, from its byte 989 (the stats' `+0x08`).
    pub fn stats(&self) -> Result<Stats> {
        let data = self.creature()?;
        let at = |x: usize| Reader::at(data, STATS + x);
        let words = |x: usize| -> Result<[u16; 6]> {
            let mut r = at(x);
            let mut words = [0; 6];
            for word in &mut words {
                *word = r.u16()?;
            }
            Ok(words)
        };
        Ok(Stats {
            class: at(0x08).u32()?,
            experience: at(0x0C).u32()?,
            attributes: words(0x10)?,
            spent: at(0x1C).array("AMH attribute points spent")?,
            attribute_points: at(0x23).u8()?,
            skills: at(0x24).array("AMH skills")?,
            bases: at(0x2C).array("AMH skills' bases")?,
            bonuses: at(0x34).array("AMH skills' bonuses")?,
            skill_points: at(0x42).u16()?,
            to_learn: at(0x44).u16()?,
            gold: at(0x46).u32()?,
            base: words(0x4A)?,
            level: at(0x56).u16()?,
        })
    }

    /// The equipment's 19 slots of the creature section, each slot's item by its object id in the
    /// inventory section, 0 for none (the creature's `+0x1A4`).
    pub fn equipment(&self) -> Result<[u32; 19]> {
        let data = self.creature()?;
        let mut r = Reader::at(data, EQUIPMENT_SLOTS);
        let mut slots = [0; 19];
        for slot in &mut slots {
            *slot = r.u32()?;
        }
        Ok(slots)
    }

    /// The items in the backpack's grid, cell by cell, row by row, each at its top left cell.
    pub fn backpack(&self) -> Result<Vec<Packed>> {
        let data = &self.section(BACKPACK, "AMH backpack section")?.data;
        // The 256 cells' width, a byte's worth at most.
        let width = u8::try_from(Reader::at(data, WIDTH).u16()?)
            .unwrap_or(u8::MAX)
            .max(1);
        let mut grid = Reader::new(data).window(GRID, CELL * CELLS, "AMH backpack grid")?;
        let mut packed = Vec::new();
        for index in 0..=u8::MAX {
            let mut cell = grid
                .take(CELL, "AMH backpack cell")?
                .with_record(usize::from(index));
            let id = cell.u32()?;
            if id == 0 {
                continue;
            }
            cell.u8()?;
            let size = (cell.u8()?, cell.u8()?);
            cell.u8()?;
            packed.push(Packed {
                id,
                at: (index % width, index / width),
                size,
                item: cell.u32()?,
            });
        }
        Ok(packed)
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
        let mut slots = Reader::at(&section.data, EQUIPMENT_COUNT);
        let count = slots.u32()?;
        if usize::try_from(count).ok() != Some(EQUIPMENT) {
            return Err(slots.error_at(
                EQUIPMENT_COUNT,
                field("AMH summary's equipment slots", count.into(), "19".into()),
            ));
        }
        let mut equipment = [0; EQUIPMENT];
        for slot in &mut equipment {
            *slot = slots.u32()?;
        }
        let mut won = [false; WON_COUNT];
        for (k, won) in won.iter_mut().enumerate() {
            *won = Reader::at(&section.data, WON + k).u8()? != 0;
        }
        Ok(Summary {
            level,
            class,
            name,
            equipment,
            won,
        })
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
        summary[0x88] = 19;
        summary[0x8C + 4 * 13..0x8C + 4 * 14].copy_from_slice(&1856u32.to_le_bytes());
        summary[0xEE] = 1;
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
                name: "Ilja".into(),
                equipment: std::array::from_fn(|slot| if slot == 13 { 1856 } else { 0 }),
                won: [false, true, false, false, false, false],
            }
        );
    }

    /// An inventory of three items' blocks, a backpack holding the second, as the templates
    /// have, the third with a socketed item's record before its own, as the saved heroes have.
    fn inventory() -> Amh {
        let mut inventory = 30u32.to_le_bytes().to_vec();
        for (id, item, least, socketed) in [
            (17u16, 7901u32, 21u16, None),
            (19, 5171, 0, None),
            (56, 7725, 9, Some(5560u32)),
        ] {
            let at = inventory.len() + 4;
            let sockets = usize::from(socketed.is_some());
            inventory.extend(item.to_le_bytes());
            inventory.extend(ITEM_MARK);
            inventory.resize(at + ITEM_RECORD + (sockets + 1) * RECORD + 60, 0);
            inventory[at + ITEM_ID..at + ITEM_ID + 2].copy_from_slice(&id.to_le_bytes());
            inventory[at + ITEM_TYPE..at + ITEM_TYPE + 4].copy_from_slice(&item.to_le_bytes());
            inventory[at + ITEM_SOCKETS - 4..at + ITEM_SOCKETS].copy_from_slice(&SOCKETS_MARK);
            inventory[at + ITEM_SOCKETS] = u8::from(socketed.is_some());
            if let Some(socketed) = socketed {
                let first = at + ITEM_RECORD;
                inventory[first..first + 4].copy_from_slice(&socketed.to_le_bytes());
            }
            let record = at + ITEM_RECORD + sockets * RECORD;
            inventory[record..record + 4].copy_from_slice(&item.to_le_bytes());
            inventory[record + 0x1C..record + 0x1E].copy_from_slice(&least.to_le_bytes());
        }
        let mut backpack = vec![0; 4444];
        backpack[WIDTH] = 20;
        let cell = GRID + 23 * CELL;
        backpack[cell] = 19;
        backpack[cell + 4..cell + 12].copy_from_slice(&[1, 1, 2, 0, 0x33, 0x14, 0, 0]);
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
        assert_eq!(
            found,
            [
                (17, 7901, RECORD, 21),
                (19, 5171, RECORD, 0),
                (56, 7725, RECORD, 9)
            ]
        );
        // The third's socketed item's record before its own.
        let socketed: Vec<_> = items[2]
            .socketed
            .iter()
            .map(|r| (r.len(), u32::from_le_bytes([r[0], r[1], r[2], r[3]])))
            .collect();
        assert_eq!(socketed, [(RECORD, 5560)]);
        assert!(items[0].socketed.is_empty());
        assert_eq!(
            amh.backpack().unwrap(),
            [Packed {
                id: 19,
                at: (3, 1),
                size: (1, 2),
                item: 5171
            }]
        );
        // A block whose type after the marker is another is an error, and so is one without the
        // sockets' marker.
        let mut broken = inventory();
        broken.sections[0].data[4 + 4 + ITEM_TYPE] = 0;
        assert!(
            broken
                .items()
                .unwrap_err()
                .to_string()
                .contains("`AMH item block's type`")
        );
        let mut unmarked = inventory();
        unmarked.sections[0].data[4 + 4 + ITEM_SOCKETS - 4] = 0;
        assert!(
            unmarked
                .items()
                .unwrap_err()
                .to_string()
                .contains("`AMH item block's sockets' marker`")
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
    fn the_creature_section_holds_the_stats_and_the_equipment() {
        let mut amh = creature(&[]);
        // Without its markers the section is refused.
        assert!(
            amh.stats().unwrap_err().to_string().contains("marker"),
            "{:?}",
            amh.stats()
        );
        let data = &mut amh.sections[0].data;
        for (at, mark) in CREATURE_MARKS {
            data[at..at + 4].copy_from_slice(&mark.to_le_bytes());
        }
        let put = |data: &mut Vec<u8>, at: usize, bytes: &[u8]| {
            data[at..at + bytes.len()].copy_from_slice(bytes);
        };
        put(data, STATS + 0x08, &8u32.to_le_bytes());
        put(data, STATS + 0x0C, &1_800_000u32.to_le_bytes());
        put(data, STATS + 0x10, &100u16.to_le_bytes());
        put(data, STATS + 0x1C + 2, &[14]);
        put(data, STATS + 0x24, &[2, 10]);
        put(data, STATS + 0x2C, &[23, 13]);
        put(data, STATS + 0x34, &[13]);
        put(data, STATS + 0x42, &9u16.to_le_bytes());
        put(data, STATS + 0x44, &1u16.to_le_bytes());
        put(data, STATS + 0x46, &977_384u32.to_le_bytes());
        put(data, STATS + 0x4A, &26u16.to_le_bytes());
        put(data, STATS + 0x56, &29u16.to_le_bytes());
        put(data, EQUIPMENT_SLOTS + 13 * 4, &74u32.to_le_bytes());
        let stats = amh.stats().unwrap();
        assert_eq!(
            stats,
            Stats {
                class: 8,
                experience: 1_800_000,
                attributes: [100, 0, 0, 0, 0, 0],
                spent: [0, 0, 14, 0, 0, 0],
                attribute_points: 0,
                skills: [2, 10, 0, 0, 0, 0, 0, 0],
                bases: [23, 13, 0, 0, 0, 0, 0, 0],
                bonuses: [13, 0, 0, 0, 0, 0, 0, 0],
                skill_points: 9,
                to_learn: 1,
                gold: 977_384,
                base: [26, 0, 0, 0, 0, 0],
                level: 29,
            }
        );
        let equipment = amh.equipment().unwrap();
        assert_eq!(
            (
                equipment[13],
                equipment.iter().filter(|&&id| id != 0).count()
            ),
            (74, 1)
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
