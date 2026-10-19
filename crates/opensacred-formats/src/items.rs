//! `PAK/items.pak` (`"ITM"` v5): the object types that `Static.PAK`, scripts and saves name
//! (docs/02 §6.1); the overlay `PAK/Items03.pak` has the same layout and one used slot. Confirmed
//! on every record by `tools/analysis/formats/mixed_dump.py` and `tables_dump.py`.

use crate::error::{Result, field, to_usize};
use crate::pak::Pak;
use crate::reader::Reader;

pub const MAGIC: [u8; 3] = *b"ITM";
pub const VERSION: u8 = 5;
const RECORD_SIZE: usize = 128;
/// Directory type of a used slot; an empty one has 0.
const USED: u32 = 2;

/// The 32,768 slots, 17,408 of them used.
#[derive(Debug)]
pub struct Items<'a> {
    pak: Pak<'a>,
}

/// Object type: a creature, equipment, a world static, a tree, an interactive object, ….
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Item<'a> {
    /// `+0x00`: 2 creatures and equipment, 9 world statics, 12 miniatures, 13 interactive
    /// objects (doors, buttons, bridges), 25 trees and walls, 29 floors, 34 upgrades, ….
    pub kind: u8,
    /// `+0x01`, `+0x02`.
    pub class: u8,
    pub subclass: u8,
    /// `+0x00` as a u32: the kind, class, subclass and `+0x03`; the original's draw lists test
    /// its bits `0x4` and `0x800000` (docs/02 §6.1, openheilig).
    pub flags: u32,
    /// `+0x04`: for the miniature types 8 and 12 (`MiniObjTex*`), the `texture.pak` atlas that
    /// their statics cut squares from (openheilig).
    pub texture: u32,
    /// `+0x10`: the `mixed.pak` sprite of the type; 0 for none.
    pub sprite: u32,
    /// `+0x37`: the name (Latin-1): a `.GRN` file, or an editor label such as `Tree17`.
    pub name: &'a [u8],
    /// `+0x5B`…`+0x65`: the shadow that the client draws for a static of the type when its
    /// [`flags`](Item::flags) have bit `0x10000` (`0x631EB0`, docs/re/drawing.md).
    pub shadow: Shadow,
}

/// A type's shadow: a quad of the shadow atlas `SHADOW_TREE00` (16 × 16 cells of 32 × 32
/// texels) on the ground at the static's foot. Confirmed by code (`0x631EB0`) and on every record
/// by `tools/analysis/formats/mixed_dump.py`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Shadow {
    /// `+0x5B`: the silhouette's cell, `16 · row + column`.
    pub cell: u16,
    /// `+0x5D`, `+0x5F`: half the offset of the static's foot from its sprite's top-left
    /// corner, in pixels at zoom 1.
    pub half_foot: [i16; 2],
    /// `+0x63`: 0 a rectangle about the foot; otherwise its top edge is shifted right by its
    /// height, the shadow leaning up and to the right.
    pub shape: u8,
    /// `+0x64`: the quad's height in pixels at zoom 1; it is twice as wide, its bottom edge on
    /// the foot's line.
    pub size: u16,
}

/// Every field of a used slot, 128 bytes; spans whose meaning is not known yet are named by their
/// offset. The name is padded with zeros (`tools/analysis/formats/tables_dump.py`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub kind: u8,
    pub class: u8,
    pub subclass: u8,
    pub byte_03: u8,
    /// `+0x04`.
    pub texture: u32,
    pub words_08: [u32; 2],
    /// `+0x10`.
    pub sprite: u32,
    pub words_14: [u32; 8],
    pub bytes_34: [u8; 3],
    /// `+0x37`, 32 bytes, Latin-1.
    pub name: String,
    pub byte_57: u8,
    pub words_58: [u32; 10],
}

impl<'a> Items<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        Ok(Self {
            pak: Pak::parse_expecting(data, MAGIC, VERSION)?,
        })
    }

    pub fn len(&self) -> usize {
        self.pak.entries().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Slot `index` as stored.
    pub fn bytes(&self, index: usize) -> Result<&'a [u8]> {
        let entry = self.pak.entry(index)?;
        if to_usize(entry.size) != RECORD_SIZE {
            return Err(Reader::new(self.pak.data()).error_at(
                self.pak.entry_offset(index) + 8,
                field("item size", entry.size.into(), RECORD_SIZE.to_string()),
            ));
        }
        self.pak
            .record_reader(index, entry)
            .bytes(RECORD_SIZE, "item")
    }

    /// The type in slot `index`; `None` for an empty slot.
    pub fn get(&self, index: usize) -> Result<Option<Item<'a>>> {
        if self.pak.entry(index)?.kind != USED {
            return Ok(None);
        }
        let bytes = self.bytes(index)?;
        let mut r = Reader::new(bytes).with_record(index);
        let kind = r.u8()?;
        let class = r.u8()?;
        let subclass = r.u8()?;
        let byte_03 = r.u8()?;
        let texture = r.u32()?;
        r.bytes(0x10 - 8, "item")?;
        let sprite = r.u32()?;
        r.bytes(0x37 - 0x14, "item")?;
        let name = r.bytes(32, "item name")?;
        r.bytes(0x5B - 0x57, "item")?;
        let cell = r.u16()?;
        let half_foot = [
            i16::from_le_bytes(r.array("item shadow")?),
            i16::from_le_bytes(r.array("item shadow")?),
        ];
        r.bytes(0x63 - 0x61, "item")?;
        let shape = r.u8()?;
        let size = r.u16()?;
        Ok(Some(Item {
            kind,
            class,
            subclass,
            flags: u32::from_le_bytes([kind, class, subclass, byte_03]),
            texture,
            sprite,
            name: name.split(|&b| b == 0).next().unwrap_or_default(),
            shadow: Shadow {
                cell,
                half_foot,
                shape,
                size,
            },
        }))
    }

    /// Every field of slot `index`; `None` for an empty slot.
    pub fn record(&self, index: usize) -> Result<Option<Record>> {
        if self.pak.entry(index)?.kind != USED {
            return Ok(None);
        }
        let bytes = self.bytes(index)?;
        let mut r = Reader::new(bytes).with_record(index);
        let u32s = |r: &mut Reader<'_>, out: &mut [u32]| -> Result<()> {
            for value in out {
                *value = r.u32()?;
            }
            Ok(())
        };
        let [kind, class, subclass, byte_03] = r.array("item")?;
        let texture = r.u32()?;
        let mut words_08 = [0; 2];
        u32s(&mut r, &mut words_08)?;
        let sprite = r.u32()?;
        let mut words_14 = [0; 8];
        u32s(&mut r, &mut words_14)?;
        let bytes_34 = r.array("item")?;
        let name = crate::tables::name_field(&mut r, 32, "item name")?;
        let byte_57 = r.u8()?;
        let mut words_58 = [0; 10];
        u32s(&mut r, &mut words_58)?;
        Ok(Some(Record {
            kind,
            class,
            subclass,
            byte_03,
            texture,
            words_08,
            sprite,
            words_14,
            bytes_34,
            name,
            byte_57,
            words_58,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{PakRecord, build_pak};

    fn record(kind: u8, sprite: u32, name: &[u8]) -> Vec<u8> {
        let mut r = vec![0; RECORD_SIZE];
        r[0] = kind;
        r[1] = 3;
        r[0x10..0x14].copy_from_slice(&sprite.to_le_bytes());
        r[0x37..0x37 + name.len()].copy_from_slice(name);
        r
    }

    #[test]
    fn reads_a_types_shadow() {
        // A tree's: cell 2, its foot at (190, 440), 240 tall, leaning.
        let mut tree = record(25, 812, b"Tree17");
        tree[2] = 1;
        tree[0x5B..0x5D].copy_from_slice(&2u16.to_le_bytes());
        tree[0x5D..0x5F].copy_from_slice(&95i16.to_le_bytes());
        tree[0x5F..0x61].copy_from_slice(&220i16.to_le_bytes());
        tree[0x63] = 1;
        tree[0x64..0x66].copy_from_slice(&240u16.to_le_bytes());
        let data = build_pak(MAGIC, VERSION, &[Some(PakRecord::new(USED, &tree))]);
        let items = Items::parse(&data).unwrap();
        let item = items.get(0).unwrap().unwrap();
        assert_eq!(item.flags & 0x10000, 0x10000);
        assert_eq!(
            item.shadow,
            Shadow {
                cell: 2,
                half_foot: [95, 220],
                shape: 1,
                size: 240,
            }
        );
    }

    #[test]
    fn reads_used_slots_and_skips_empty_ones() {
        let used = |bytes: Vec<u8>| Some(PakRecord::new(USED, &bytes));
        let data = build_pak(
            MAGIC,
            VERSION,
            &[
                Some(PakRecord::new(0, &[0; RECORD_SIZE])),
                used(record(25, 812, b"Tree17")),
                used(record(9, 0, b"Br\xFCcke A 01")),
            ],
        );
        let items = Items::parse(&data).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items.get(0).unwrap(), None);
        let tree = items.get(1).unwrap().unwrap();
        assert_eq!(
            (tree.kind, tree.class, tree.sprite, tree.name),
            (25, 3, 812, b"Tree17".as_slice())
        );
        assert_eq!(items.get(2).unwrap().unwrap().name, b"Br\xFCcke A 01");
        assert!(items.get(3).is_err());
        let record = items.record(2).unwrap().unwrap();
        assert_eq!(
            (record.kind, record.class, record.name.as_str()),
            (9, 3, "Br\u{fc}cke A 01")
        );
        assert_eq!(items.record(0).unwrap(), None);
    }
}
