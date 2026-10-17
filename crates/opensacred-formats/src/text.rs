//! `scripts/<language>/global.res` (docs/02 §8): the game's strings by the hash of their name.
//! `"SZ\0\0"`, then 23,123 records of 16 bytes `{u32 id, u32 offset, u32 flags, u32 length}`
//! sorted by id, then the UTF-16LE strings without terminators, one after another. An offset
//! counts from byte 4. Confirmed on every record by `tools/analysis/formats/global_res_dump.py`.

use crate::error::{ErrorKind, Result, field, to_usize};
use crate::reader::Reader;

const MAGIC: [u8; 4] = *b"SZ\0\0";
const RECORD_SIZE: usize = 16;

/// The string table.
#[derive(Debug, Clone)]
pub struct GlobalRes<'a> {
    entries: Vec<Entry<'a>>,
}

/// A string of the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry<'a> {
    /// [`sacred_hash`] of the string's name.
    pub id: u32,
    /// 0 but in 4 records (1, 1, 3, 5); meaning unknown.
    pub flags: u32,
    /// The UTF-16LE bytes.
    utf16: &'a [u8],
}

impl Entry<'_> {
    /// The string, decoded; the table holds no unpaired surrogate.
    pub fn text(&self) -> Option<String> {
        let units: Vec<u16> = self
            .utf16
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&c| u16::from_le_bytes(c))
            .collect();
        String::from_utf16(&units).ok()
    }
}

impl<'a> GlobalRes<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        let file = Reader::new(data);
        let mut r = file.clone();
        let magic: [u8; 4] = r.array("global.res magic")?;
        if magic != MAGIC {
            return Err(file.error(ErrorKind::Signature { what: "global.res" }));
        }
        // The records end where the first string starts.
        let first_at = r.offset() + 4;
        let first = to_usize(Reader::at(data, first_at).u32()?);
        if !first.is_multiple_of(RECORD_SIZE) || first == 0 {
            return Err(file.error_at(
                first_at,
                field(
                    "first offset",
                    as_u64(first),
                    "a multiple of 16, after the records".into(),
                ),
            ));
        }
        let count = first / RECORD_SIZE;
        let mut records = r.take(first, "global.res records")?;
        let mut entries: Vec<Entry<'a>> = Vec::with_capacity(count);
        let mut previous: Option<u32> = None;
        let mut expected = first;
        for index in 0..count {
            let at = records.offset();
            let id = records.u32()?;
            let offset = to_usize(records.u32()?);
            let flags = records.u32()?;
            let len = to_usize(records.u32()?);
            if previous.is_some_and(|p| id <= p) {
                return Err(records.error_at(
                    at,
                    field("string id", id.into(), "above the previous one".into()),
                ));
            }
            previous = Some(id);
            if offset != expected || !len.is_multiple_of(2) {
                return Err(records.error_at(
                    at + 4,
                    field(
                        "string offset",
                        as_u64(offset),
                        format!("{expected}, right after string {}", index.saturating_sub(1)),
                    ),
                ));
            }
            expected = offset + len;
            let utf16 = file.window(4 + offset, len, "string")?.rest();
            entries.push(Entry { id, flags, utf16 });
        }
        if 4 + expected != data.len() {
            return Err(file.error_at(
                data.len(),
                field(
                    "file size",
                    as_u64(data.len()),
                    format!("{}, the end of the last string", 4 + expected),
                ),
            ));
        }
        Ok(Self { entries })
    }

    /// The strings in id order.
    pub fn entries(&self) -> &[Entry<'a>] {
        &self.entries
    }

    /// The string with `id`.
    pub fn get(&self, id: u32) -> Option<&Entry<'a>> {
        let index = self.entries.binary_search_by_key(&id, |e| e.id).ok()?;
        self.entries.get(index)
    }
}

fn as_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

/// The id of the string named `name` (`FUN_0080E780`, per sacred-sdk, MIT): a hash over the
/// bytes, ASCII letters upper-cased, with the original's 32-bit overflow and signed remainder
/// (docs/02 §8). A numeric reference `res:N` hashes the text of `N`.
pub fn sacred_hash(name: &[u8]) -> u32 {
    let mut h: u32 = 0;
    for &b in name {
        let c = u32::from(b.to_ascii_uppercase());
        let v = h.wrapping_mul(113).wrapping_add(c).cast_signed() % 999_999_991;
        h = v.cast_unsigned();
    }
    h & 0x7FFF_FFFF
}

#[cfg(test)]
mod game_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn table(strings: &[(u32, &str)]) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        let mut pool = Vec::new();
        let first = strings.len() * RECORD_SIZE;
        for &(id, text) in strings {
            let bytes: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
            for v in [
                id,
                u32::try_from(first + pool.len()).unwrap(),
                0,
                u32::try_from(bytes.len()).unwrap(),
            ] {
                out.extend_from_slice(&v.to_le_bytes());
            }
            pool.extend(bytes);
        }
        out.extend(pool);
        out
    }

    #[test]
    fn reads_strings_by_id() {
        let data = table(&[(49, "Seraphim"), (50, "Gladiator"), (60, " Brücke ")]);
        let res = GlobalRes::parse(&data).unwrap();
        assert_eq!(res.entries().len(), 3);
        assert_eq!(res.get(50).unwrap().text().unwrap(), "Gladiator");
        assert_eq!(res.get(60).unwrap().text().unwrap(), " Brücke ");
        assert!(res.get(51).is_none());
    }

    #[test]
    fn ids_rise_and_strings_follow_each_other() {
        let mut data = table(&[(50, "a"), (49, "b")]);
        let err = GlobalRes::parse(&data).unwrap_err();
        assert!(
            err.to_string()
                .ends_with("field `string id` is 49, expected above the previous one"),
            "{err}"
        );
        data = table(&[(49, "a"), (50, "b")]);
        data.push(0);
        assert!(GlobalRes::parse(&data).is_err());
    }

    #[test]
    fn the_hash_matches_the_game() {
        // docs/02 §8: `res:1` and `res:1024`; case does not matter.
        assert_eq!(sacred_hash(b"1"), 49);
        assert_eq!(sacred_hash(b"1024"), 71_320_567);
        assert_eq!(sacred_hash(b"seraphim"), sacred_hash(b"SERAPHIM"));
    }
}
