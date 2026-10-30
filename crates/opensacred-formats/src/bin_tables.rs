//! The data tables of `bin/` (docs/02 §6.3), compiled by the original's built-in compiler from
//! texts that are not shipped: counted lists of words, read to the last byte. The meaning of most
//! fields waits for the code that uses them (M3); every word is kept. Confirmed by
//! `tools/analysis/formats/bin_tables_dump.py`.

use crate::error::{Result, field};
use crate::reader::Reader;

mod balance_keys;
pub use balance_keys::BALANCE_KEYS;

/// `Balance.bin`: `u32` the size in bytes of an image of the client's balance variables (at
/// `0xAD4490`), the image, then tables of their own (docs/02 §6.3). The balance text's keys and
/// `GoldProLevel`'s second value name 355 of the image's words ([`BALANCE_KEYS`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Balance {
    /// The image's words, 1,224: each an f32 or an i32, by its key where it has one.
    pub image: Vec<u32>,
    /// The tables after the image.
    pub tail: Tail,
}

/// The tables after the balance image, in the order of the client's reader `0x5E5C50`
/// (docs/02 §6.3, "The tail of `Balance.bin`"), each a `u32` count and its entries.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tail {
    /// 1: lists of records of three words by a key, 0 to 32, into the map at `0xAD57B0`; not
    /// followed.
    pub lists: Vec<(u32, Vec<[u32; 3]>)>,
    /// 2: the drops' weight of each quality key, 0 to 17, in the order of the keys.
    pub weights: Vec<(u32, u32)>,
    /// 3: the item maker's quality key and the value of the quality pass `0x5D2E40` it takes.
    pub qualities: Vec<(u32, u32)>,
    /// 4: rows of seven words by a key, 1 to 9, into `0xAD57D4`; not followed.
    pub rows: Vec<(u32, [u32; 7])>,
    /// 5: an effect and the parameters `a`, `b`, `c` of a chance by its value
    /// (`(1 − a/(b + v))·100 + 0.5` per cent at most `c`, `0x5F23A0`).
    pub chances: Vec<(u32, [f32; 3])>,
    /// 6: lists of item types by a key, into `0xAD57EC`; not followed.
    pub item_lists: Vec<(u32, Vec<u32>)>,
    /// 7: the regions (`0xAAAD28`), records of 0x40 bytes.
    pub regions: Vec<Region>,
    /// The words after the seventh table, to the end of the file: none in the file.
    pub rest: Vec<u32>,
}

/// A record of the regions' table (docs/02 §6.3): its 16 areas' numbers (0 unused), its name's
/// text key, its total of kills, and the record's own index at `+0x3C`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    pub areas: [u8; 16],
    pub name: String,
    pub total: u32,
    pub index: u8,
}

/// What a balance variable holds: the parser stores a float with `fstp`, an integer with `mov`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    F32,
    I32,
}

/// A key of the balance text, and the variable it names: its offset in the image, in bytes,
/// and its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BalanceKey {
    pub name: &'static str,
    pub offset: u16,
    pub kind: Kind,
}

impl Balance {
    /// The word that `key` names; `None` past the image.
    pub fn word(&self, key: &BalanceKey) -> Option<u32> {
        self.image.get(usize::from(key.offset / 4)).copied()
    }
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
    let size = r.u32()?;
    if !size.is_multiple_of(4) {
        return Err(r.error(field("balance image", size.into(), "whole words".into())));
    }
    let image = (0..size / 4).map(|_| r.u32()).collect::<Result<_>>()?;
    let tail = tail(&mut r)?;
    Ok(Balance { image, tail })
}

/// The tables after the image, in the order of the reader `0x5E5C50`.
fn tail(r: &mut Reader<'_>) -> Result<Tail> {
    // A list's least size: its key and its count.
    let n = count(r, 8)?;
    let lists = (0..n)
        .map(|_| {
            let key = r.u32()?;
            let m = count(r, 12)?;
            let records = (0..m).map(|_| words(r)).collect::<Result<_>>()?;
            Ok((key, records))
        })
        .collect::<Result<_>>()?;
    let mut pairs = || -> Result<Vec<(u32, u32)>> {
        let n = count(r, 8)?;
        (0..n).map(|_| Ok((r.u32()?, r.u32()?))).collect()
    };
    let weights = pairs()?;
    let qualities = pairs()?;
    let n = count(r, 32)?;
    let rows = (0..n)
        .map(|_| Ok((r.u32()?, words(r)?)))
        .collect::<Result<_>>()?;
    let n = count(r, 16)?;
    let chances = (0..n)
        .map(|_| Ok((r.u32()?, r.f32s()?)))
        .collect::<Result<_>>()?;
    let n = count(r, 8)?;
    let item_lists = (0..n)
        .map(|_| {
            let key = r.u32()?;
            let m = count(r, 4)?;
            let items = (0..m).map(|_| r.u32()).collect::<Result<_>>()?;
            Ok((key, items))
        })
        .collect::<Result<_>>()?;
    let n = count(r, REGION)?;
    let regions = (0..n).map(|_| region(r)).collect::<Result<_>>()?;
    if !r.rest().len().is_multiple_of(4) {
        return Err(r.error(field(
            "balance rest",
            u64::try_from(r.rest().len()).unwrap_or(u64::MAX),
            "whole words".into(),
        )));
    }
    let rest = (0..r.rest().len() / 4)
        .map(|_| r.u32())
        .collect::<Result<_>>()?;
    Ok(Tail {
        lists,
        weights,
        qualities,
        rows,
        chances,
        item_lists,
        regions,
        rest,
    })
}

/// A region's record's size.
const REGION: usize = 0x40;

/// A record of the regions' table: the areas at `+0`, the name's key at `+0x10` (32 bytes, to its
/// first zero), the total at `+0x30`, the warning's byte at `+0x34`, the remaining count at
/// `+0x38`, the index at `+0x3C` (`0x5E5C50`).
fn region(r: &mut Reader<'_>) -> Result<Region> {
    let areas = r.array::<16>("region areas")?;
    let name = r.bytes(32, "region name")?;
    let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());
    let name = String::from_utf8_lossy(&name[..end]).into_owned();
    let total = r.u32()?;
    let _warning = r.u32()?;
    let _remaining = r.u32()?;
    let index = r.u8()?;
    r.bytes(3, "region padding")?;
    Ok(Region {
        areas,
        name,
        total,
        index,
    })
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
        // An image of 8 bytes, two words; then the five tables, one entry each, and two words.
        let mut words = vec![8, 0x3F80_0000, u32::MAX];
        words.extend([1, 0, 1, u32::MAX, u32::MAX, 3]);
        words.extend([1, 16, 300]);
        words.extend([1, 8, 9]);
        words.extend([1, 1, 140, 5, 25, 5, 15, 55, 0]);
        words.extend([1, 868]);
        words.extend([500.0_f32, 502.0, 25.0].map(f32::to_bits));
        // A list of two items by the key 3, and one region of the areas 1 and 2, `AB`, 7,500.
        words.extend([1, 3, 2, 1758, 1715]);
        words.extend([1, 0x0201, 0, 0, 0, 0x4241]);
        words.extend([0; 7]);
        words.extend([7500, 0, 7500, 1]);
        words.extend([6, 3]);
        let b = balance(&le(&words)).unwrap();
        assert_eq!(
            b,
            Balance {
                image: vec![0x3F80_0000, u32::MAX],
                tail: Tail {
                    lists: vec![(0, vec![[u32::MAX, u32::MAX, 3]])],
                    weights: vec![(16, 300)],
                    qualities: vec![(8, 9)],
                    rows: vec![(1, [140, 5, 25, 5, 15, 55, 0])],
                    chances: vec![(868, [500.0, 502.0, 25.0])],
                    item_lists: vec![(3, vec![1758, 1715])],
                    regions: vec![Region {
                        areas: [1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                        name: "AB".to_owned(),
                        total: 7500,
                        index: 1,
                    }],
                    rest: vec![6, 3],
                }
            }
        );
        // The first key, BalanceDmg, names the image's first word, 1.0.
        assert_eq!(BALANCE_KEYS[0].name, "BalanceDmg");
        assert_eq!(b.word(&BALANCE_KEYS[0]), Some(0x3F80_0000));
        assert_eq!(b.word(BALANCE_KEYS.last().unwrap()), None);
        // A table that runs past the end is an error.
        assert!(
            balance(&le(&[0, 1, 0, 5]))
                .unwrap_err()
                .to_string()
                .contains("`count` is 5")
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
