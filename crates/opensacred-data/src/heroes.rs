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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Heroes {
    pub heroes: Vec<HeroEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeroEntry {
    /// The file in the installation, such as `Templates/Hero00.ptx`.
    pub source: String,
    /// The hero file, relative to `heroes.ron`.
    pub file: String,
    pub level: u32,
    /// 1–9.
    pub class: u32,
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
