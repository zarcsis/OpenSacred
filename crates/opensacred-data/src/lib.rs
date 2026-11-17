//! Open-format types shared by the converter and the game: sector descriptions, item and
//! creature tables, the heroes' index, sound profiles, the intermediate representation of scripts
//! and the index and calls of their Luau translation, the texts' and the cutscenes' names
//! (PLAN.md §7). With them, the code that writes and reads two of the formats, the ZIP archives
//! of the game's assets ([`archive`]) and its KTX2 textures ([`texture`]), and the mark of the
//! assets converted for a player ([`converted`]).
//!
//! The game reads only these types and never the original files.

pub mod archive;
pub mod converted;
pub mod heroes;
pub mod luau;
pub mod netscript;
mod packed;
#[cfg(test)]
mod round_trips;
pub mod sound;
pub mod tables;
pub mod text;
pub mod texture;
pub mod video;
pub mod world;

pub use packed::DecodeError;
