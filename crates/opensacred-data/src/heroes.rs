//! The converted heroes (docs/02 §9): the starting templates of the classes.
//!
//! `heroes.ron` holds the [`Heroes`] index; each hero is a [`Hero`] file, postcard-encoded in a
//! zlib stream ([`Hero::encode`]), with the sections of the original inflated. The sections are
//! kept whole until their layout is recovered (the creature dump, the inventory, the world state).

use serde::{Deserialize, Serialize};

use crate::packed::{DecodeError, decode, encode};

/// Version of the hero encoding; a file of another version does not load.
pub const VERSION: u32 = 1;
/// The index file under the hero directory.
pub const INDEX: &str = "heroes.ron";

/// The index, `heroes.ron`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Heroes {
    pub heroes: Vec<HeroEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeroEntry {
    /// The file in the installation, such as `Templates/Hero00.ptx`.
    pub source: String,
    /// The hero file, relative to `heroes.ron`.
    pub file: String,
    pub level: u32,
    /// 1–9.
    pub class: u32,
    /// The items the hero wears and holds at the start: those of its inventory that its backpack
    /// does not hold (docs/02 §9).
    #[serde(default)]
    pub worn: Vec<Worn>,
    /// The items in its backpack at the start, each at its top left cell (docs/02 §9).
    #[serde(default)]
    pub backpack: Vec<Carried>,
    /// The combat arts its template holds, as the creature section has them (docs/re/arts.md,
    /// "The heroes' starting arts").
    #[serde(default)]
    pub arts: Vec<HeroArt>,
}

/// A combat art of a template: an entry of a creature's list of arts (docs/re/arts.md, "A
/// creature's arts").
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HeroArt {
    /// 1 an art of the first table, a spell; 2 of the second, a weapon art.
    pub table: u32,
    pub id: u16,
    pub level: u8,
    /// A bonus to the level, of the items and the sets.
    pub bonus: u8,
    /// Bit 0 set on an art a rune taught, the high byte the shortcut's slot.
    pub flags: u16,
    /// Its regeneration time, in seconds.
    pub time: f32,
    /// Its regeneration's factor.
    pub factor: f32,
    /// The regeneration still to run, in seconds.
    pub remaining: f32,
}

/// An item a hero wears or holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Worn {
    /// The object's id in the template.
    pub id: u32,
    /// Its type: its `items.pak` slot and `Weapon.pak` record.
    pub item: u32,
    /// Its record's `u16` words from its `+4`, as `tables::Weapon::words_84` holds a fixed
    /// item's from `Weapon.pak +0x84` (docs/re/items.md).
    pub words: Vec<u16>,
}

/// An item in a hero's backpack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Carried {
    /// The object's id in the template.
    pub id: u32,
    /// Its type: its `items.pak` slot and `Weapon.pak` record.
    pub item: u32,
    /// Its top left cell, column and row.
    pub at: (u8, u8),
    /// Its width and height in cells.
    pub size: (u8, u8),
    /// Its record's words from its `+4`, as [`Worn::words`].
    pub words: Vec<u16>,
}

/// A hero: the sections of the original file in slot order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hero {
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section {
    /// The slot of the original table, 0–15.
    pub slot: u8,
    /// 0xC7 the creature, 0xCA the backpack, 0xCB a second inventory container, 0xC8 the
    /// inventory, 0xC3 the summary, 0xC4 a small block, 0xCD the world state, 0xCE zeros (docs/02
    /// §9).
    pub tag: u32,
    /// The size in the original table; the data of tag 0xC4 is 64 bytes for a size of 556.
    pub size: u32,
    pub data: Vec<u8>,
}

impl Hero {
    /// The hero file: the version and the hero, postcard-encoded, in a zlib stream.
    pub fn encode(&self) -> Result<Vec<u8>, postcard::Error> {
        encode(VERSION, self)
    }

    /// The hero of a file that [`Hero::encode`] wrote.
    pub fn decode(file: &[u8]) -> Result<Self, DecodeError> {
        decode(VERSION, file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heroes_round_trip() {
        let hero = Hero {
            sections: vec![Section {
                slot: 4,
                tag: 0xC3,
                size: 556,
                data: vec![1, 0, 0, 0, 2, 0, 0, 0],
            }],
        };
        assert_eq!(Hero::decode(&hero.encode().unwrap()).unwrap(), hero);
    }
}
