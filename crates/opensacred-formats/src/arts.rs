//! The combat arts' two tables compiled into `Sacred.exe` (docs/02 §6.5, docs/re/arts.md): the
//! spells, 70 records of `0x94` bytes, and the weapon arts, 94 records of `0x78` bytes, each
//! after an empty record 0 (id 0, the icon `INVALID.TGA`). They lie at fixed virtual addresses of
//! the GOG 2.28 build, which the caller checks against the manifest. Confirmed on every record by
//! `tools/analysis/formats/arts_dump.py`.

use crate::error::{ErrorKind, Result, field};
use crate::pe::Image;
use crate::reader::Reader;
use crate::tables::{name_field, u16s, zeros};

/// The first table, the spells (`xls_magictypes.h`): its record 0 in GOG 2.28.
pub const SPELLS_ADDRESS: u32 = 0x0095_2A38;
pub const SPELL_SIZE: usize = 0x94;
/// The spells after the record 0, which `0x4D2310` searches.
pub const SPELL_COUNT: usize = 70;
/// The second table, the weapon arts (`xls_smovetypes.h`): its record 0 in GOG 2.28.
pub const WEAPON_ARTS_ADDRESS: u32 = 0x0094_FDB0;
pub const WEAPON_ART_SIZE: usize = 0x78;
/// The weapon arts after the record 0, which `0x4D1AF0` searches.
pub const WEAPON_ART_COUNT: usize = 94;
/// The thirteen effects of a spell, each a base and a step by the level.
pub const EFFECTS: usize = 13;
/// The three values by the level of a spell, each with its cap.
pub const VALUES: usize = 3;
/// The icon's texture name: 32 bytes, zeros after the name.
const ICON: usize = 32;
/// The icon of the record 0 of either table.
const INVALID_ICON: &str = "INVALID.TGA";

/// The fields that open a record of either table, `+0x00` to `+0x3C`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    /// `+0x00`: the art's id; `+0x02` is 0.
    pub id: u16,
    /// `+0x04`: the name's text index: the `global.res` string named by it in decimal (docs/02
    /// §8).
    pub name: u32,
    /// `+0x08`: the long description's text index; 600 more is the short one's. 0 in eleven
    /// spells, which no string names.
    pub description: u32,
    /// `+0x0C`: the icon's texture name (`GUI_SPELL_IRRITATION.TGA`); `+0x2C`, its texture's
    /// handle at run time, is 0.
    pub icon: String,
    /// `+0x30`: three sprites of the interface's table for the icon, the last the one drawn.
    pub sprites: [u32; 3],
}

/// A spell, a record of the first table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spell {
    pub head: Head,
    /// `+0x3C`: the launch type: 1 on an enemy, 2 on a friend, 3 on the ground, 4 on the dead, 5
    /// on itself, 0 none.
    pub launch: u16,
    /// `+0x3E`: the regeneration time in seconds, `base + step·L`.
    pub regeneration: [u16; 2],
    /// `+0x42`: thirteen effects, `base + step·L` each: the physical, fire, magic and poison
    /// damage, the attack, the defence in percent, the physical, fire, magic and poison
    /// resistances, the speed, the mental and the physical regeneration.
    pub effects: [[u16; 2]; EFFECTS],
    /// `+0x76`: three values, `base + step·L` each; the value 0 is the duration in seconds for
    /// most.
    pub values: [[u16; 2]; VALUES],
    /// `+0x82`: each value's cap, 0 for none.
    pub caps: [u16; VALUES],
    /// `+0x88`: the element: 1 water, 2 air, 3 earth, 4 fire, 5 lost, 0 none; `+0x8A` is 0.
    pub element: u16,
    /// `+0x8C`: the classes that learn and use the art, a mask.
    pub classes: u32,
    /// `+0x90`: bit 0 not on horseback, bit 1 on horseback only with a bow, bit 2 with a
    /// two-handed weapon only; `+0x91` to `+0x93` are 0.
    pub flags: u8,
}

/// A weapon art, a record of the second table.
#[derive(Debug, Clone, PartialEq)]
pub struct WeaponArt {
    pub head: Head,
    /// `+0x3C`: a column of the balancing sheet, which no code is known to read.
    pub value_3c: u32,
    /// `+0x40`: the regeneration time in seconds, `base + step·L`.
    pub regeneration: [f32; 2],
    /// `+0x48`: the distance the art is made from: 101 to 250 the reach of the two creatures, 0
    /// where the creature stands, below 0 for the jumps.
    pub distance: i32,
    /// `+0x4C`: the launch type, as a spell's; `+0x4D` to `+0x4F` are 0.
    pub launch: u8,
    /// `+0x50`: for most arts the damage's factor of each strike, `base + step·L`; for the jumps
    /// 1003 and 1032 the radius they strike within as they land, for 1083 another value.
    pub factor: [f32; 2],
    /// `+0x58`: a second value by the level, of each art its own: the factor of the strikes of
    /// 1003 and 1083, the value that 1032 gives each creature it lands by, the kicks' own.
    pub value_58: [f32; 2],
    /// `+0x60`: a whole value by the level: for the `Rundum` arts the radius in percent of the
    /// reach.
    pub value_60: [i32; 2],
    /// `+0x68`: another, of each art its own.
    pub value_68: [i32; 2],
    /// `+0x70`: the classes, a mask.
    pub classes: u32,
    /// `+0x74`: the flags of a spell's `+0x90`; `+0x75` to `+0x77` are 0.
    pub flags: u8,
}

/// The reader of a table's records: its record 0 checked, the cursor at the record 1.
fn table<'a>(
    exe: &'a [u8],
    address: u32,
    size: usize,
    count: usize,
    what: &'static str,
) -> Result<Reader<'a>> {
    let image = Image::parse(exe)?;
    let mut r = image.at(address, size * (count + 1), what)?;
    let start = r.offset();
    let mut first = r.take(size, what)?;
    let head = head(&mut first)?;
    if head.id != 0 || head.icon != INVALID_ICON {
        return Err(r.error_at(start, ErrorKind::Signature { what }));
    }
    Ok(r)
}

fn head(r: &mut Reader<'_>) -> Result<Head> {
    let id = r.u16()?;
    zeros(r, 2, "art bytes 0x02")?;
    let name = r.u32()?;
    let description = r.u32()?;
    let icon = name_field(r, ICON, "art icon")?;
    let handle_at = r.offset();
    let handle = r.u32()?;
    if handle != 0 {
        return Err(r.error_at(
            handle_at,
            field("art icon handle", handle.into(), "0".into()),
        ));
    }
    let sprites = [r.u32()?, r.u32()?, r.u32()?];
    Ok(Head {
        id,
        name,
        description,
        icon,
        sprites,
    })
}

fn pairs<const N: usize>(r: &mut Reader<'_>) -> Result<[[u16; 2]; N]> {
    let mut out = [[0; 2]; N];
    for pair in &mut out {
        *pair = u16s(r)?;
    }
    Ok(out)
}

fn i32_pair(r: &mut Reader<'_>) -> Result<[i32; 2]> {
    Ok([r.u32()?, r.u32()?].map(|word| i32::from_le_bytes(word.to_le_bytes())))
}

/// The spells of the first table (VA `0x952A38` in GOG 2.28), in its order: the ids 1 to 72 but
/// 54 and 68, 13 and 14 and 47 and 48 swapped, from 54 on one or two ahead of the records.
pub fn spells(exe: &[u8]) -> Result<Vec<Spell>> {
    let mut r = table(
        exe,
        SPELLS_ADDRESS,
        SPELL_SIZE,
        SPELL_COUNT,
        "table of spells",
    )?;
    (1..=SPELL_COUNT)
        .map(|record| {
            let mut r = r.take(SPELL_SIZE, "spell")?.with_record(record);
            let head = head(&mut r)?;
            let launch = r.u16()?;
            let regeneration = u16s(&mut r)?;
            let effects = pairs(&mut r)?;
            let values = pairs(&mut r)?;
            let caps = u16s(&mut r)?;
            let element = r.u16()?;
            zeros(&mut r, 2, "spell bytes 0x8A")?;
            let classes = r.u32()?;
            let flags = r.u8()?;
            zeros(&mut r, 3, "spell bytes 0x91")?;
            Ok(Spell {
                head,
                launch,
                regeneration,
                effects,
                values,
                caps,
                element,
                classes,
                flags,
            })
        })
        .collect()
}

/// The weapon arts of the second table (VA `0x94FDB0` in GOG 2.28), in its order: the ids 1000
/// to 1103 but ten, mostly in order.
pub fn weapon_arts(exe: &[u8]) -> Result<Vec<WeaponArt>> {
    let mut r = table(
        exe,
        WEAPON_ARTS_ADDRESS,
        WEAPON_ART_SIZE,
        WEAPON_ART_COUNT,
        "table of weapon arts",
    )?;
    (1..=WEAPON_ART_COUNT)
        .map(|record| {
            let mut r = r.take(WEAPON_ART_SIZE, "weapon art")?.with_record(record);
            let head = head(&mut r)?;
            let value_3c = r.u32()?;
            let regeneration = r.f32s()?;
            let distance = i32::from_le_bytes(r.u32()?.to_le_bytes());
            let launch = r.u8()?;
            zeros(&mut r, 3, "weapon art bytes 0x4D")?;
            let factor = r.f32s()?;
            let value_58 = r.f32s()?;
            let value_60 = i32_pair(&mut r)?;
            let value_68 = i32_pair(&mut r)?;
            let classes = r.u32()?;
            let flags = r.u8()?;
            zeros(&mut r, 3, "weapon art bytes 0x75")?;
            Ok(WeaponArt {
                head,
                value_3c,
                regeneration,
                distance,
                launch,
                factor,
                value_58,
                value_60,
                value_68,
                classes,
                flags,
            })
        })
        .collect()
}

#[cfg(test)]
mod game_tests;

#[cfg(test)]
mod tests;
