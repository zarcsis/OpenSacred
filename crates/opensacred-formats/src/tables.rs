//! The fixed-record tables (docs/02 §6.2, §5.5, §4.1): `PAK/Creature.pak` (`"CIF"` v0),
//! `PAK/Weapon.pak` (`"WPN"` v8), `PAK/MOTIONS.PAK` (`"MHP"` v1) and `World/Triggers.PAK`
//! (`"TRG"` v1). A 0x100-byte header with the record count at `+4`, then the records back to back;
//! no directory but the one entry of `Triggers.PAK`. Fields whose meaning is not known yet are
//! kept as spans named by their offset, so that nothing of a record is lost. Confirmed on every
//! record by `tools/analysis/formats/tables_dump.py`.

use crate::error::{ErrorKind, Result, field, to_usize};
use crate::reader::Reader;

const HEADER: usize = 0x100;
const CREATURE_SIZE: usize = 86;
const WEAPON_SIZE: usize = 258;
const WEAPON_RESERVE: usize = 64;
const MOTION_SIZE: usize = 198;
const TRIGGER_SIZE: usize = 16;

/// A creature type, 86 bytes. The field map is the client's own dump of the table
/// (openheilig, `creature-pak.md`); the names in comments are its labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Creature {
    /// `+0x00`: also the `items.pak` slot of the creature's model.
    pub id: u32,
    /// `+0x04`: CLASS, 1–15.
    pub class: u8,
    /// `+0x06`: FLAGS: 0x01 flies, 0x02 big, 0x10 no shadow, 0x20 ghost, 0x40 and 0x80 (labels
    /// `BANANE`, `KURVE`).
    pub flags: u8,
    /// `+0x08`: EXP: experience `a + level × b`.
    pub experience: [u16; 2],
    /// `+0x0C`: BASE: the six attributes (labels STK, RES, GES, REPHY, REMAG, CHARISMA).
    pub base: [u8; 6],
    /// `+0x14`: SKILLS.
    pub skills: [u8; 2],
    /// `+0x16`: SKILLSX.
    pub skills_extra: [u8; 16],
    /// `+0x26`: SPEED, a pair (another source calls them walk and run).
    pub speed: [u16; 2],
    /// `+0x2A`: six bonuses (kind, target).
    pub bonus: [[u8; 2]; 6],
    /// `+0x36`.
    pub bonus_value: [u8; 6],
    /// `+0x3C`: the class each bonus applies to.
    pub bonus_class: [u8; 6],
    /// `+0x42`: four damping blocks (labels RP, RF, RM, RG).
    pub damping: [[u8; 5]; 4],
}

/// A weapon or armour type, 258 bytes; the table is followed by as many 64-byte reserves, all
/// zero.
#[derive(Debug, Clone, PartialEq)]
pub struct Weapon {
    pub floats_00: [f32; 5],
    pub bytes_14: [u8; 4],
    pub float_18: f32,
    pub bytes_1c: [u8; 8],
    /// `+0x24`: the item type it inherits from, or 0 (openheilig).
    pub parent: u32,
    /// `+0x28`: an internal name (Latin-1), padded with zeros to 88 bytes.
    pub name: String,
    /// `+0x80`: its item type; unique.
    pub id: u32,
    pub words_84: [u16; 63],
}

/// The parameters of a motion, 198 bytes: the block `MotionDesc[0x42:0x100]` of a motion of
/// `models.pak` (then `models03.pak`), whose own block is zero. The other bytes are zero.
#[derive(Debug, Clone, PartialEq)]
pub struct Motion {
    /// `+0x06`: 0 but in one motion.
    pub value_06: u16,
    pub floats_46: [f32; 4],
    pub floats_62: [f32; 9],
    /// `+0xBE`: 0 but in two motions.
    pub value_be: u16,
}

/// A trigger, 16 bytes `{u32 id; u16 16; u16 lo; u16 hi; u16 1; u32 0}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trigger {
    pub id: u32,
    /// `hi << 16 | lo`, 20 bits (**hypothesis**: an object of `Static.PAK`).
    pub target: u32,
}

/// The count at `+4` after the magic, and a reader at the records, which start `skip` bytes after
/// the header and, each with `tail` more bytes after the table, fill the file.
fn records<'a>(
    data: &'a [u8],
    magic: [u8; 3],
    version: u8,
    size: usize,
    tail: usize,
    skip: usize,
) -> Result<(usize, Reader<'a>)> {
    let file = Reader::new(data);
    let mut r = file.clone();
    let found: [u8; 3] = r.array("magic")?;
    let found_version = r.u8()?;
    if found != magic || found_version != version {
        return Err(file.error(ErrorKind::Magic {
            expected: magic,
            expected_version: version,
            found,
            found_version,
        }));
    }
    let count = to_usize(r.u32()?);
    let expected = count
        .checked_mul(size + tail)
        .and_then(|len| len.checked_add(HEADER + skip));
    if expected != Some(data.len()) {
        return Err(file.error_at(
            4,
            field(
                "record count",
                u64::try_from(count).unwrap_or(u64::MAX),
                format!("records of {size} bytes that fill the file"),
            ),
        ));
    }
    Ok((count, Reader::at(data, HEADER)))
}

pub(crate) fn u16s<const N: usize>(r: &mut Reader<'_>) -> Result<[u16; N]> {
    let mut out = [0; N];
    for value in &mut out {
        *value = r.u16()?;
    }
    Ok(out)
}

/// A name field of `len` bytes: the bytes before the NUL, Latin-1, only zeros after it.
pub(crate) fn name_field(r: &mut Reader<'_>, len: usize, what: &'static str) -> Result<String> {
    let at = r.offset();
    let bytes = r.bytes(len, what)?;
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(len);
    if bytes[end..].iter().any(|&b| b != 0) {
        return Err(r.error_at(at, field(what, 0, "padded with zeros".into())));
    }
    Ok(bytes[..end].iter().map(|&b| char::from(b)).collect())
}

/// Bytes that must be zero.
pub(crate) fn zeros(r: &mut Reader<'_>, len: usize, what: &'static str) -> Result<()> {
    let at = r.offset();
    if r.bytes(len, what)?.iter().any(|&b| b != 0) {
        return Err(r.error_at(at, field(what, 1, "zeros".into())));
    }
    Ok(())
}

pub fn creatures(data: &[u8]) -> Result<Vec<Creature>> {
    let (count, mut r) = records(data, *b"CIF", 0, CREATURE_SIZE, 0, 0)?;
    (0..count)
        .map(|_| {
            let id = r.u32()?;
            let class = r.u8()?;
            zeros(&mut r, 1, "creature byte 0x05")?;
            let flags = r.u8()?;
            zeros(&mut r, 1, "creature byte 0x07")?;
            let experience = u16s(&mut r)?;
            let base = r.array("creature base")?;
            zeros(&mut r, 2, "creature bytes 0x12")?;
            let skills = r.array("creature skills")?;
            let skills_extra = r.array("creature skills")?;
            let speed = u16s(&mut r)?;
            let mut bonus = [[0; 2]; 6];
            for pair in &mut bonus {
                *pair = r.array("creature bonus")?;
            }
            let bonus_value = r.array("creature bonus value")?;
            let bonus_class = r.array("creature bonus class")?;
            let mut damping = [[0; 5]; 4];
            for block in &mut damping {
                *block = r.array("creature damping")?;
            }
            Ok(Creature {
                id,
                class,
                flags,
                experience,
                base,
                skills,
                skills_extra,
                speed,
                bonus,
                bonus_value,
                bonus_class,
                damping,
            })
        })
        .collect()
}

pub fn weapons(data: &[u8]) -> Result<Vec<Weapon>> {
    let (count, mut r) = records(data, *b"WPN", 8, WEAPON_SIZE, WEAPON_RESERVE, 0)?;
    let weapons = (0..count)
        .map(|_| {
            let floats_00 = r.f32s()?;
            let bytes_14 = r.array("weapon bytes 0x14")?;
            let float_18 = r.f32()?;
            let bytes_1c = r.array("weapon bytes 0x1c")?;
            let parent = r.u32()?;
            let name = name_field(&mut r, 0x80 - 0x28, "weapon name")?;
            let id = r.u32()?;
            let words_84 = u16s(&mut r)?;
            Ok(Weapon {
                floats_00,
                bytes_14,
                float_18,
                bytes_1c,
                parent,
                name,
                id,
                words_84,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    zeros(&mut r, count * WEAPON_RESERVE, "weapon reserve")?;
    Ok(weapons)
}

pub fn motions(data: &[u8]) -> Result<Vec<Motion>> {
    let (count, mut r) = records(data, *b"MHP", 1, MOTION_SIZE, 0, 0)?;
    (0..count)
        .map(|index| {
            let at = r.offset();
            let [first, zero, second] = u16s(&mut r)?;
            if u16::try_from(index).ok() != Some(first) || zero != 0 || second != first {
                return Err(r.error_at(at, field("motion index", first.into(), index.to_string())));
            }
            let value_06 = r.u16()?;
            zeros(&mut r, 0x46 - 0x08, "motion bytes 0x08")?;
            let floats_46 = r.f32s()?;
            zeros(&mut r, 0x62 - 0x56, "motion bytes 0x56")?;
            let floats_62 = r.f32s()?;
            zeros(&mut r, 0xBE - 0x86, "motion bytes 0x86")?;
            let value_be = r.u16()?;
            zeros(&mut r, MOTION_SIZE - 0xC0, "motion bytes 0xc0")?;
            Ok(Motion {
                value_06,
                floats_46,
                floats_62,
                value_be,
            })
        })
        .collect()
}

/// The triggers by slot; `None` for an empty one.
pub fn triggers(data: &[u8]) -> Result<Vec<Option<Trigger>>> {
    // The one directory entry takes 12 bytes before the records.
    let (count, mut r) = records(data, *b"TRG", 1, TRIGGER_SIZE, 0, 12)?;
    let file = Reader::new(data);
    let entry = [r.u32()?, r.u32()?, r.u32()?];
    let expected =
        [136, HEADER + 12, count * TRIGGER_SIZE].map(|v| u32::try_from(v).unwrap_or(u32::MAX));
    if entry != expected {
        return Err(file.error_at(
            HEADER,
            field(
                "trigger directory",
                entry[0].into(),
                "{136, 0x10c, the records' size}".into(),
            ),
        ));
    }
    (0..count)
        .map(|_| {
            let at = r.offset();
            let id = r.u32()?;
            let [sixteen, lo, hi, one] = u16s(&mut r)?;
            let zero = r.u32()?;
            if [id, zero] == [0, 0] && [sixteen, lo, hi, one] == [0; 4] {
                return Ok(None);
            }
            if sixteen != 16 || hi >= 16 || one != 1 || zero != 0 {
                return Err(r.error_at(
                    at,
                    field(
                        "trigger",
                        id.into(),
                        "{id, 16, lo, hi < 16, 1, 0} or zeros".into(),
                    ),
                ));
            }
            Ok(Some(Trigger {
                id,
                target: u32::from(hi) << 16 | u32::from(lo),
            }))
        })
        .collect()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod game_tests;
