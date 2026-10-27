//! The types' names that `Sacred.exe` holds (`0x8EC328`), entries of 0x44 bytes: a type's number
//! and its name of up to 64 bytes (`TYPE_NPC_GOBLIN_WARRIOR`), which `0x43CED0` gives a type's
//! number by and which the client's code tests for parts of (`0x68EB90` the footsteps, `0x695800`
//! the doors' sounds; docs/re/sound.md). A fixed virtual address of the GOG 2.28 build, which the
//! caller checks against the manifest.

use crate::error::Result;
use crate::pe::Image;

/// The table's address and the size of an entry.
pub const ADDRESS: u32 = 0x008E_C328;
const ENTRY: u32 = 0x44;
/// The most entries the table is read for, past its end.
const MOST: u32 = 16_384;

/// The types' numbers and names in the table's order, to the first entry whose name does not
/// start with `TYPE_` (5,624 in GOG 2.28).
pub fn type_names(exe: &[u8]) -> Result<Vec<(u32, String)>> {
    let image = Image::parse(exe)?;
    let mut names = Vec::new();
    for index in 0..MOST {
        let mut r = image.at(ADDRESS + index * ENTRY, 0x44, "type's name")?;
        let entry = r.bytes(0x44, "type's name")?;
        let kind = u32::from_le_bytes(entry[..4].try_into().unwrap_or_default());
        let name = &entry[4..];
        let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());
        if !name[..end].starts_with(b"TYPE_") {
            break;
        }
        names.push((kind, name[..end].iter().map(|&b| char::from(b)).collect()));
    }
    Ok(names)
}
