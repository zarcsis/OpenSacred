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
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Heroes {
    pub heroes: Vec<HeroEntry>,
    /// The installation's heroes of the Underworld campaign's choice, as it ships them
    /// (`save/hero06.pax` and `hero07.pax`, docs/re/underworld.md), by their slots.
    #[serde(default)]
    pub saved: Vec<SavedHero>,
}

/// A saved hero of the choice's eight slots (`SAVE\HERO%.2d.PAX`): its slot, its name (the
/// summary's) and its hero, as [`HeroEntry`] indexes a template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct SavedHero {
    pub slot: u8,
    pub name: String,
    pub hero: HeroEntry,
    /// The items in its equipment's slots, by the slots, as its summary keeps them: what its
    /// figure on the pedestal wears (`0x6F4E60`; docs/02 §9).
    #[serde(default)]
    pub equipment: Vec<(u8, u32)>,
    /// Whether it won each difficulty, bronze's first, as its summary keeps them: the choice
    /// offers it the difficulties up to one past the highest (`0x6F4F20`; docs/re/fight.md).
    #[serde(default)]
    pub won: Vec<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
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
    /// Its creature section's values (docs/02 §9, "The creature"), which a game of a saved hero
    /// takes in place of its class's start (#2362).
    #[serde(default)]
    pub stats: HeroStats,
    /// The items in its equipment's slots, each slot's by its object id ([`Worn::id`]), as its
    /// creature section keeps them; the hands' are its selected weapon set's.
    #[serde(default)]
    pub equipment: Vec<(u8, u32)>,
    /// Its five weapon sets, each set's left hand's and right hand's item by its object id, 0 for
    /// none, as its backpack's section keeps them (docs/02 §9).
    #[serde(default)]
    pub sets: Vec<[u32; 2]>,
}

/// An item in a socket of another, as the other's block keeps it (docs/02 §9): its type, its
/// record's words from its `+4` as [`Worn::words`], and the tier of the smith who put it there,
/// its record's byte `+0x81`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Socketed {
    pub item: u32,
    pub words: Vec<u16>,
    pub tier: u8,
}

/// A hero's values of its creature section's stats (docs/02 §9, "The creature"): its
/// experience and gold, the attribute points spent on each attribute and those left, its eight
/// skill slots with their bases, and the skill points and skills left to it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct HeroStats {
    pub experience: u32,
    pub gold: u32,
    pub spent: [u8; 6],
    pub attribute_points: u8,
    /// The skill of each slot, 0 for none, and its base.
    pub skills: [u8; 8],
    pub bases: [u8; 8],
    pub skill_points: u16,
    pub to_learn: u16,
}

/// A combat art of a template: an entry of a creature's list of arts (docs/re/arts.md, "A
/// creature's arts").
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
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
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Worn {
    /// The object's id in the template.
    pub id: u32,
    /// Its type: its `items.pak` slot and `Weapon.pak` record.
    pub item: u32,
    /// Its record's `u16` words from its `+4`, as `tables::Weapon::words_84` holds a fixed
    /// item's from `Weapon.pak +0x84` (docs/re/items.md).
    pub words: Vec<u16>,
    /// The items in its sockets, in their order.
    #[serde(default)]
    pub socketed: Vec<Socketed>,
}

/// An item in a hero's backpack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
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
    /// The items in its sockets, in their order.
    #[serde(default)]
    pub socketed: Vec<Socketed>,
}

/// A hero: the sections of the original file in slot order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Hero {
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
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
