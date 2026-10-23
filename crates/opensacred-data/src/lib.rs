//! Open-format types shared by the converter and the game: sector descriptions, item and
//! creature tables, sound profiles, the intermediate representation of scripts (PLAN.md §7).
//!
//! The game reads only these types and never the original files.

pub mod archive;
pub mod heroes;
pub mod netscript;
mod packed;
pub mod sound;
pub mod tables;
pub mod text;
pub mod video;
pub mod world;

pub use packed::DecodeError;
