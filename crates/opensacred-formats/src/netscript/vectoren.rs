//! `bin/<tree>/Vectoren.bin` (docs/02 §7.1): three counted tables back to back to the end of the
//! file, read by the client at `0x46FDCA`. The functions name the blocks of `FunkCode.bin`; the
//! quests and the regions are kept as they are, their fields mostly not understood yet.

use std::ops::Range;

use super::Script;
use crate::error::{Result, field};
use crate::reader::Reader;

const NAME: usize = 64;
const QUEST_GAP: usize = 192;

/// The tables of a `Vectoren.bin`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vectoren {
    pub functions: Vec<Function>,
    pub quests: Vec<Quest>,
    pub regions: Vec<Region>,
}

/// A function block of `FunkCode.bin`: 84 bytes `{char name[64]; i32 offset; i32 length;
/// i32 quest; i32 region; i32 0}`. The blocks follow one another from offset 0 to the end of
/// `FunkCode.bin`, each starting and ending at a record; the first has no name and no length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    /// Byte offset of the first record in `FunkCode.bin`.
    pub offset: u32,
    /// Bytes of records.
    pub length: u32,
    /// The id of the quest the function belongs to, or −1.
    pub quest: i32,
    /// 0, or 1–23 in the Sacred campaign trees (**hypothesis**: the region of a dynamic quest).
    pub region: i32,
}

/// 292 bytes `{u32 id; char name[64]; 192 zero bytes; i32 values[8]}`; the values hold function
/// indices (**hypothesis**).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quest {
    pub id: u32,
    pub name: String,
    pub values: [i32; 8],
}

/// `{u32 id; char name[64]; i32 values[5]; 3 × (u32 n; u32 list[n]); u32 n; n × entry}`: 22 in
/// the Sacred campaign trees, none elsewhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    pub id: u32,
    pub name: String,
    pub values: [i32; 5],
    pub lists: [Vec<u32>; 3],
    pub entries: Vec<RegionEntry>,
}

/// `{u32 n; u32 list[n]; i32 values[4]; u8 tail[8]}` (the client's reader `0x4B2AA0`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionEntry {
    pub list: Vec<u32>,
    pub values: [i32; 4],
    pub tail: [u8; 8],
}

fn i32s<const N: usize>(r: &mut Reader<'_>) -> Result<[i32; N]> {
    let mut out = [0; N];
    for value in &mut out {
        *value = i32::from_le_bytes(r.array("value")?);
    }
    Ok(out)
}

/// A count of `size`-byte items, bounded by the bytes left so that a broken count fails before
/// it allocates.
fn count(r: &mut Reader<'_>, size: usize) -> Result<usize> {
    let at = r.offset();
    let n = r.u32()?;
    let n = usize::try_from(n).unwrap_or(usize::MAX);
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

/// A 64-byte name field: the bytes before the NUL, Latin-1, and only zeros after it.
fn name(r: &mut Reader<'_>, what: &'static str) -> Result<String> {
    let at = r.offset();
    let bytes = r.bytes(NAME, what)?;
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(NAME);
    if bytes[len..].iter().any(|&b| b != 0) {
        return Err(r.error_at(at, field(what, 0, "padded with zeros".into())));
    }
    Ok(bytes[..len].iter().map(|&b| char::from(b)).collect())
}

impl Vectoren {
    /// The records of each function: index ranges into `funk`, the script of `FunkCode.bin`;
    /// `None` when a function does not start and end at a record.
    pub fn record_ranges(&self, funk: &Script) -> Option<Vec<Range<usize>>> {
        let index = |offset: u32| {
            let offset = usize::try_from(offset).ok()?;
            match funk.records.binary_search_by_key(&offset, |r| r.offset) {
                Ok(i) => Some(i),
                Err(i) if i == funk.records.len() && offset == funk.size => Some(i),
                Err(_) => None,
            }
        };
        self.functions
            .iter()
            .map(|f| Some(index(f.offset)?..index(f.offset.checked_add(f.length)?)?))
            .collect()
    }

    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let n = count(&mut r, 84)?;
        let mut functions = Vec::with_capacity(n);
        for _ in 0..n {
            let at = r.offset();
            let name = name(&mut r, "function name")?;
            let offset = r.u32()?;
            let length = r.u32()?;
            let [quest, region, zero] = i32s(&mut r)?;
            if zero != 0 {
                return Err(r.error_at(
                    at + 80,
                    field("function tail", u64::from(zero.unsigned_abs()), "0".into()),
                ));
            }
            functions.push(Function {
                name,
                offset,
                length,
                quest,
                region,
            });
        }
        let n = count(&mut r, 292)?;
        let mut quests = Vec::with_capacity(n);
        for _ in 0..n {
            let id = r.u32()?;
            let name = name(&mut r, "quest name")?;
            let at = r.offset();
            if r.bytes(QUEST_GAP, "quest gap")?.iter().any(|&b| b != 0) {
                return Err(r.error_at(at, field("quest gap", 0, "zeros".into())));
            }
            quests.push(Quest {
                id,
                name,
                values: i32s(&mut r)?,
            });
        }
        let n = count(&mut r, 4 + NAME + 20 + 16)?;
        let mut regions = Vec::with_capacity(n);
        for _ in 0..n {
            let id = r.u32()?;
            let name = name(&mut r, "region name")?;
            let values = i32s(&mut r)?;
            let lists = [list(&mut r)?, list(&mut r)?, list(&mut r)?];
            let n = count(&mut r, 28)?;
            let mut entries = Vec::with_capacity(n);
            for _ in 0..n {
                entries.push(RegionEntry {
                    list: list(&mut r)?,
                    values: i32s(&mut r)?,
                    tail: r.array("region entry tail")?,
                });
            }
            regions.push(Region {
                id,
                name,
                values,
                lists,
                entries,
            });
        }
        if !r.rest().is_empty() {
            let left = u64::try_from(r.rest().len()).unwrap_or(u64::MAX);
            return Err(r.error(field("bytes after the regions", left, "0".into())));
        }
        Ok(Self {
            functions,
            quests,
            regions,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name_field(name: &str) -> Vec<u8> {
        let mut field = name.as_bytes().to_vec();
        field.resize(NAME, 0);
        field
    }

    fn file() -> Vec<u8> {
        let le = |v: u32| v.to_le_bytes();
        let mut d = le(2).to_vec();
        d.extend(name_field(""));
        d.extend([0u8; 20]);
        d.extend(name_field("dq_reward"));
        for v in [0i32, 410, -1, 0, 0] {
            d.extend(v.to_le_bytes());
        }
        d.extend(le(1));
        d.extend(le(99));
        d.extend(name_field("MP"));
        d.extend([0; QUEST_GAP]);
        for v in [0i32, 0, 6, 8, 0, 9, 0, 0] {
            d.extend(v.to_le_bytes());
        }
        d.extend(le(1));
        d.extend(le(1));
        d.extend(name_field("Region1"));
        for v in [0i32, 0, 3, 0, 0] {
            d.extend(v.to_le_bytes());
        }
        for list in [&[0u32, 6143][..], &[], &[7]] {
            d.extend(le(u32::try_from(list.len()).unwrap()));
            list.iter().for_each(|&v| d.extend(le(v)));
        }
        d.extend(le(1));
        d.extend(le(2));
        d.extend(le(36));
        d.extend(le(37));
        for v in [1i32, 0, 1, 5] {
            d.extend(v.to_le_bytes());
        }
        d.extend(b"1\0\0\0\0\0\0\0");
        d
    }

    #[test]
    fn three_tables_to_the_end_of_the_file() {
        let v = Vectoren::parse(&file()).unwrap();
        assert_eq!(
            v.functions[1],
            Function {
                name: "dq_reward".into(),
                offset: 0,
                length: 410,
                quest: -1,
                region: 0,
            }
        );
        assert_eq!(v.quests[0].values, [0, 0, 6, 8, 0, 9, 0, 0]);
        let region = &v.regions[0];
        assert_eq!(region.lists, [vec![0, 6143], vec![], vec![7]]);
        assert_eq!(
            region.entries,
            [RegionEntry {
                list: vec![36, 37],
                values: [1, 0, 1, 5],
                tail: *b"1\0\0\0\0\0\0\0",
            }]
        );
    }

    #[test]
    fn broken_files_are_errors() {
        let mut data = file();
        data.push(0);
        assert!(
            Vectoren::parse(&data)
                .unwrap_err()
                .to_string()
                .contains("`bytes after the regions` is 1")
        );
        let mut data = file();
        data[0] = 0xFF;
        assert!(
            Vectoren::parse(&data)
                .unwrap_err()
                .to_string()
                .contains("`count` is 255")
        );
        let mut data = file();
        data[4 + 84 + 20] = b'x';
        assert!(
            Vectoren::parse(&data)
                .unwrap_err()
                .to_string()
                .contains("`function name`")
        );
    }
}
