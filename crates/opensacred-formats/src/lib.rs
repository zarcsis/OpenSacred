//! Readers for the original Sacred Gold 2.28 (GOG) formats: PAK containers, TEX textures, GRN
//! models, world sectors, NetScript, `global.res`, AMH templates, and the tables compiled into
//! `Sacred.exe`.
//!
//! The crate only parses data and writes nothing; reading never panics, errors name the record
//! and the file offset. Format descriptions are in `docs/02-data-formats.md`, reference Python
//! prototypes in `tools/analysis/formats/`.

pub mod amh;
pub mod arts;
pub mod bin_tables;
pub mod bmp;
pub mod error;
pub mod grn;
pub mod items;
pub mod mdl;
pub mod mixed;
pub mod netscript;
pub mod pak;
pub mod pe;
pub mod portraits;
pub mod reader;
pub mod sound;
pub mod tables;
pub mod tex;
pub mod text;
pub mod voices;
pub mod world;
mod zlib;

#[cfg(test)]
mod testing;

pub use error::{Error, ErrorKind, Place, Result};
