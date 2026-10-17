//! The data tables of `bin/` (docs/02 §6.3), compiled by the original's built-in compiler from
//! texts that are not shipped: counted lists of words, read to the last byte. The meaning of most
//! fields waits for the code that uses them (M3); every word is kept. Confirmed by
//! `tools/analysis/formats/bin_tables_dump.py`.

use crate::error::{Result, field};
use crate::reader::Reader;

/// `Balance.bin`: the balance constants, read by key from fixed offsets; the key names are in
/// the client (openheilig, `balance-bin.md`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Balance {
    /// 4,896 words, each an f32 or an i32 by its key; 0xFFFFFFFF is "not set".
    pub values: Vec<u32>,
    /// The 1,185 words after them.
    pub tail: Vec<u32>,
}

/// An item set of `sets.bin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemSet {
    pub items: [u32; 10],
    pub hash: u32,
    pub bytes: [u8; 4],
}

/// A weapon modifier of `wpmod.bin` (the client's loader `0x42F8CE`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponModifier {
    /// The fixed 212 bytes.
    pub words: [u32; 53],
    pub entries: Vec<[u32; 6]>,
}

fn words<const N: usize>(r: &mut Reader<'_>) -> Result<[u32; N]> {
    let mut out = [0; N];
    for value in &mut out {
        *value = r.u32()?;
    }
    Ok(out)
}

/// A count of `size`-byte items, bounded by the bytes left.
fn count(r: &mut Reader<'_>, size: usize) -> Result<usize> {
    let at = r.offset();
    let n = usize::try_from(r.u32()?).unwrap_or(usize::MAX);
    if n.checked_mul(size).is_none_or(|len| len > r.rest().len()) {
        return Err(r.error_at(
            at,
            field(
                "count",
                u64::try_from(n).unwrap_or(u64::MAX),
                "items within the file".into(),
            ),
        ));
    }
    Ok(n)
}

fn list(r: &mut Reader<'_>) -> Result<Vec<u32>> {
    let n = count(r, 4)?;
    (0..n).map(|_| r.u32()).collect()
}

/// The end of the file must follow.
fn end(r: &Reader<'_>, what: &'static str) -> Result<()> {
    let left = r.rest().len();
    if left != 0 {
        return Err(r.error(field(
            what,
            u64::try_from(left).unwrap_or(u64::MAX),
            "0 bytes after the last record".into(),
        )));
    }
    Ok(())
}

pub fn balance(data: &[u8]) -> Result<Balance> {
    let mut r = Reader::new(data);
    let n = count(&mut r, 4)?;
    let values = (0..n).map(|_| r.u32()).collect::<Result<_>>()?;
    if !r.rest().len().is_multiple_of(4) {
        return Err(r.error(field(
            "balance tail",
            u64::try_from(r.rest().len()).unwrap_or(u64::MAX),
            "whole words".into(),
        )));
    }
    let tail = (0..r.rest().len() / 4)
        .map(|_| r.u32())
        .collect::<Result<_>>()?;
    Ok(Balance { values, tail })
}

/// `World.bin`: `u32 n` and n rows of three i32.
pub fn world_index(data: &[u8]) -> Result<Vec<[i32; 3]>> {
    let mut r = Reader::new(data);
    let n = count(&mut r, 12)?;
    let rows = (0..n)
        .map(|_| Ok(words::<3>(&mut r)?.map(|w| i32::from_le_bytes(w.to_le_bytes()))))
        .collect::<Result<_>>()?;
    end(&r, "World.bin")?;
    Ok(rows)
}

/// `World2.bin`: `u32 32768` and as many bytes (a 256 × 128 map, hypothesis).
pub fn world_map(data: &[u8]) -> Result<Vec<u8>> {
    let mut r = Reader::new(data);
    let n = count(&mut r, 1)?;
    let cells = r.bytes(n, "World2.bin")?.to_vec();
    end(&r, "World2.bin")?;
    Ok(cells)
}

/// `sets.bin`: `u32 n` and n records of 112 bytes; the first is all 0xCC, `None`.
pub fn item_sets(data: &[u8]) -> Result<Vec<Option<ItemSet>>> {
    let mut r = Reader::new(data);
    let n = count(&mut r, 112)?;
    let sets = (0..n)
        .map(|i| {
            let at = r.offset();
            let bytes = r.bytes(112, "item set")?;
            if i == 0 && bytes.iter().all(|&b| b == 0xCC) {
                return Ok(None);
            }
            let mut s = Reader::at(data, at);
            let items = words(&mut s)?;
            let hash = s.u32()?;
            let set_bytes = s.array("item set")?;
            if s.bytes(64, "item set")?.iter().any(|&b| b != 0) {
                return Err(s.error_at(at + 0x30, field("item set tail", 1, "zeros".into())));
            }
            Ok(Some(ItemSet {
                items,
                hash,
                bytes: set_bytes,
            }))
        })
        .collect::<Result<_>>()?;
    end(&r, "sets.bin")?;
    Ok(sets)
}

/// `wpmod.bin`: `u32 n` and n records `{u32 words[53]; u32 k; k × u32[6]}`.
pub fn weapon_modifiers(data: &[u8]) -> Result<Vec<WeaponModifier>> {
    let mut r = Reader::new(data);
    let n = count(&mut r, 216)?;
    let modifiers = (0..n)
        .map(|_| {
            let words = words(&mut r)?;
            let k = count(&mut r, 24)?;
            let entries = (0..k).map(|_| self::words(&mut r)).collect::<Result<_>>()?;
            Ok(WeaponModifier { words, entries })
        })
        .collect::<Result<_>>()?;
    end(&r, "wpmod.bin")?;
    Ok(modifiers)
}

/// `wea.bin`: `{u32 k; u32 ids[k]}` lists to the end of the file.
pub fn weapon_groups(data: &[u8]) -> Result<Vec<Vec<u32>>> {
    let mut r = Reader::new(data);
    let mut groups = Vec::new();
    while !r.rest().is_empty() {
        groups.push(list(&mut r)?);
    }
    Ok(groups)
}

/// `Rust.bin`: `u32 n` and n records `{u32 k; k × (u32, u32)}`.
pub fn armour_switches(data: &[u8]) -> Result<Vec<Vec<[u32; 2]>>> {
    let mut r = Reader::new(data);
    let n = count(&mut r, 4)?;
    let switches = (0..n)
        .map(|_| {
            let k = count(&mut r, 8)?;
            (0..k).map(|_| words(&mut r)).collect::<Result<_>>()
        })
        .collect::<Result<_>>()?;
    end(&r, "Rust.bin")?;
    Ok(switches)
}

/// Rows of four u32 to the end of the file (`treppe.bin`, `merc.bin`, `MultiStart.bin`).
pub fn rows(data: &[u8]) -> Result<Vec<[u32; 4]>> {
    let mut r = Reader::new(data);
    if !data.len().is_multiple_of(16) {
        return Err(r.error(field(
            "file size",
            u64::try_from(data.len()).unwrap_or(u64::MAX),
            "rows of 16 bytes".into(),
        )));
    }
    (0..data.len() / 16).map(|_| words(&mut r)).collect()
}

/// `static10_18.bin`: `"map\0"`, `u32 n`, zeros to 0x100, n rows of four u32: two (old, new)
/// static id pairs, the second 0xCCCCCCCC twice when unset.
pub fn static_remap(data: &[u8]) -> Result<Vec<[u32; 4]>> {
    let file = Reader::new(data);
    let mut r = file.clone();
    let magic: [u8; 4] = r.array("static remap magic")?;
    if &magic != b"map\0" {
        return Err(file.error(crate::ErrorKind::Signature {
            what: "static10_18.bin",
        }));
    }
    let n = usize::try_from(r.u32()?).unwrap_or(usize::MAX);
    let at = r.offset();
    if r.bytes(0x100 - 8, "static remap header")?
        .iter()
        .any(|&b| b != 0)
    {
        return Err(file.error_at(at, field("static remap header", 1, "zeros".into())));
    }
    let rows = (0..n).map(|_| words(&mut r)).collect::<Result<_>>()?;
    end(&r, "static10_18.bin")?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|w| w.to_le_bytes()).collect()
    }

    #[test]
    fn counted_lists_read_to_the_end() {
        let data = le(&[2, 7, 8, 0, 1, 9]);
        assert_eq!(weapon_groups(&data).unwrap(), [vec![7, 8], vec![], vec![9]]);
        let data = le(&[2, 1, 316, 3080, 0]);
        assert_eq!(armour_switches(&data).unwrap(), [vec![[316, 3080]], vec![]]);
        let mut data = le(&[1]);
        data.extend(le(&[5; 53]));
        data.extend(le(&[1, 1, 2, 3, 4, 5, 6]));
        assert_eq!(
            weapon_modifiers(&data).unwrap(),
            [WeaponModifier {
                words: [5; 53],
                entries: vec![[1, 2, 3, 4, 5, 6]]
            }]
        );
        let data = le(&[2, 0x3F80_0000, u32::MAX, 849]);
        assert_eq!(
            balance(&data).unwrap(),
            Balance {
                values: vec![0x3F80_0000, u32::MAX],
                tail: vec![849]
            }
        );
        let data = le(&[2, 1, 3392, 0, 2, 3456, 0]);
        assert_eq!(world_index(&data).unwrap(), [[1, 3392, 0], [2, 3456, 0]]);
    }

    #[test]
    fn sets_and_the_remap_check_their_fill() {
        let mut data = le(&[2]);
        data.extend([0xCC; 112]);
        let mut set = le(&[4053, 7583, 0, 0, 0, 0, 0, 0, 0, 0, 0x9821_F1CE]);
        set.extend([6, 1, 0, 0]);
        set.resize(112, 0);
        data.extend(&set);
        let sets = item_sets(&data).unwrap();
        assert_eq!(sets[0], None);
        assert_eq!(sets[1].unwrap().bytes, [6, 1, 0, 0]);
        data[4 + 112 + 0x40] = 1;
        assert!(
            item_sets(&data)
                .unwrap_err()
                .to_string()
                .contains("item set tail")
        );

        let mut data = b"map\0".to_vec();
        data.extend(1u32.to_le_bytes());
        data.resize(0x100, 0);
        data.extend(le(&[928, 928, 0xCCCC_CCCC, 0xCCCC_CCCC]));
        assert_eq!(
            static_remap(&data).unwrap(),
            [[928, 928, 0xCCCC_CCCC, 0xCCCC_CCCC]]
        );
        data.push(0);
        assert!(static_remap(&data).is_err());
    }

    #[test]
    fn broken_counts_are_errors() {
        assert!(
            weapon_groups(&le(&[5, 1]))
                .unwrap_err()
                .to_string()
                .contains("`count` is 5")
        );
        assert!(
            rows(&[0; 15])
                .unwrap_err()
                .to_string()
                .contains("`file size` is 15")
        );
    }
}

#[cfg(test)]
mod game_tests;
