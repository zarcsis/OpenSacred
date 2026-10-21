//! The vector sets of the fight's formulas (docs/re/fight.md): the functions of `Sacred.exe` run
//! on sets of inputs, their outputs kept. The oracle only sets the inputs (the arguments, the
//! fields of a synthetic `this`) and reads the outputs; what the functions compute is the
//! original's machine code (docs/08 §3).

use anyhow::Result;
use serde::Serialize;

use crate::machine::{Machine, Stub};

/// The level's end `X(L)` (`0x57F6E0`, a `thiscall` on the stats with the level, a word; 0 for
/// the stats' own level `+0x56`).
const LEVEL_END: u32 = 0x57_F6E0;
/// The level of an experience (`0x57F8D0`, a `thiscall` on the stats, whose `+0x56` is the level
/// held, with the experience).
const LEVEL_OF: u32 = 0x57_F8D0;
/// The weight of `x` against `y` (`0x43EEE0`, `cdecl`): `x·(1 − 1/(1 + x/y)^k)`, `k` of the share
/// `p` and the base `c`, `k = −ln(1 − p)/ln(1 + c)`; its last argument, when not null, takes
/// `1 − 1/(1 + x/y)^k` as a float. The strike's resolution takes the hit chance and each kind's
/// damage against its resistance by it (`0x56B130`).
const WEIGHT: u32 = 0x43_EEE0;
/// The client's log (`cdecl`), which `0x57F8D0` writes to when it would call itself without end.
const LOG_PRINTF: u32 = 0x66_EF40;
/// A creature's most health (`0x5658F0`, a `fastcall` on the stats, docs/re/fight.md, "Health"):
/// it writes the stats' `+0x12C`, and the health `+0x130` and the value shown `+0x128` of a new
/// creature.
const MOST_HEALTH: u32 = 0x56_58F0;
/// The getter of the client's object (`cdecl`), whose `+0x14` the most health compares with the
/// creature's `+0x39`: it computes only the creatures of the local client (**hypothesis**).
const CLIENT: u32 = 0x7D_84A0;
/// Whether a creature type is a hero's (a `thiscall` on the creatures' table with the type), by
/// its class's record.
const IS_HERO: u32 = 0x42_6650;

/// The size of the synthetic stats (`+0x3A8` of a creature, docs/re/fight.md) and their fields:
/// the creature they are of, its type, the attributes `a_i` (words), the base attributes `b_i`
/// (words), the level (a word), the value of the health shown, the most health and the health.
const STATS_SIZE: u32 = 0x200;
const STATS_CREATURE: u32 = 0x04;
const STATS_TYPE: u32 = 0x08;
const STATS_ATTRIBUTES: u32 = 0x10;
const STATS_BASE: u32 = 0x4A;
const STATS_LEVEL: u32 = 0x56;
const STATS_SHOWN: u32 = 0x128;
const STATS_MOST: u32 = 0x12C;
const STATS_HEALTH: u32 = 0x130;
/// The size of a synthetic creature: its fields that the most health reads are all 0 (no boss
/// `+0x1F4`, no champion `+0x2B8`, no owner `+0x14` bit 2, of the client 0 `+0x39`).
const CREATURE_SIZE: u32 = 0x600;

/// The highest level (`0x57F8D0`).
const TOP_LEVEL: u16 = 216;

/// Makes the stubs the sets' functions need, and the client's object, kept.
pub fn prepare(machine: &mut Machine<'_>) -> Result<()> {
    machine.stub_function(
        LOG_PRINTF,
        Stub {
            name: "logPrintf".to_owned(),
            value: 0,
            pops: 0,
        },
    )?;
    // The client 0, the creatures' own.
    let client = machine.object(0x100)?;
    machine.keep_objects();
    machine.stub_function(
        CLIENT,
        Stub {
            name: "the client's object".to_owned(),
            value: client,
            pops: 0,
        },
    )?;
    machine.stub_function(
        IS_HERO,
        Stub {
            name: "a hero's type".to_owned(),
            value: 0,
            pops: 4,
        },
    )
}

/// A level's end.
#[derive(Serialize)]
pub struct LevelEnd {
    pub level: u16,
    /// The experience at which it ends.
    pub end: u32,
}

/// The ends of the levels 1 to 216 (`0x57F6E0`).
pub fn level_ends(machine: &mut Machine<'_>) -> Result<Vec<LevelEnd>> {
    machine.clear_objects();
    let stats = machine.object(STATS_SIZE)?;
    (1..=TOP_LEVEL)
        .map(|level| {
            let end = machine
                .call(LEVEL_END, Some(stats), &[u32::from(level)], false)?
                .eax;
            Ok(LevelEnd { level, end })
        })
        .collect()
}

/// The level of an experience.
#[derive(Serialize)]
pub struct LevelOf {
    pub experience: u32,
    /// The level held, the stats' `+0x56`.
    pub held: u16,
    pub level: u32,
}

/// The levels of the experiences about each level's end (one less, the end, one more), and of
/// 0 and 1, for the levels held 1, 2, 10, 100, 215 and 216 (`0x57F8D0`).
pub fn levels_of(machine: &mut Machine<'_>, ends: &[LevelEnd]) -> Result<Vec<LevelOf>> {
    let mut experiences = vec![0, 1];
    for end in ends {
        experiences.extend([end.end - 1, end.end, end.end + 1]);
    }
    let mut vectors = Vec::new();
    for held in [1, 2, 10, 100, 215, TOP_LEVEL] {
        machine.clear_objects();
        let stats = machine.object(STATS_SIZE)?;
        machine.write(stats + STATS_LEVEL, &held.to_le_bytes())?;
        for &experience in &experiences {
            let level = machine
                .call(LEVEL_OF, Some(stats), &[experience], false)?
                .eax;
            vectors.push(LevelOf {
                experience,
                held,
                level,
            });
        }
    }
    Ok(vectors)
}

/// The weight `0x43EEE0` of `x` against `y` with the base `c` and the share `p`: what it returns,
/// and the float it writes.
fn weight(machine: &mut Machine<'_>, x: f32, y: f32, c: f32, p: f32) -> Result<(f64, f32)> {
    machine.clear_objects();
    let out = machine.object(4)?;
    let returned = machine.call(
        WEIGHT,
        None,
        &[x.to_bits(), y.to_bits(), c.to_bits(), p.to_bits(), out],
        true,
    )?;
    let share: [u8; 4] = machine.read(out, 4)?.try_into().unwrap_or_default();
    Ok((returned.st0.unwrap_or(f64::NAN), f32::from_le_bytes(share)))
}

/// A strike's hit chance: an attack `A` against a defence `D`.
#[derive(Serialize)]
pub struct Chance {
    pub attack: f32,
    pub defence: f32,
    /// The chance, the float that `0x43EEE0` writes for `A` against `D` with the base 1 and the
    /// share 0.5, which the resolution compares a roll of 0 to 1,000 over 1,000 with.
    pub chance: f32,
}

/// The attacks and defences of the sets: the bounds, the wolf's and the goblin warrior's of
/// docs/re/fight.md, and some others.
const ATTACKS: [f32; 10] = [0.0, 0.5, 1.0, 5.0, 13.0, 18.5, 20.6, 100.0, 250.25, 5000.0];
const DEFENCES: [f32; 10] = [
    0.0, 0.0005, 0.001, 0.0011, 1.0, 13.0, 16.0, 20.6, 100.0, 5000.0,
];

/// The hit chances of the attacks against the defences (`0x43EEE0` as `0x56B130` calls it).
pub fn chances(machine: &mut Machine<'_>) -> Result<Vec<Chance>> {
    let mut vectors = Vec::new();
    for attack in ATTACKS {
        for defence in DEFENCES {
            let (_, chance) = weight(machine, attack, defence, 1.0, 0.5)?;
            vectors.push(Chance {
                attack,
                defence,
                chance,
            });
        }
    }
    Ok(vectors)
}

/// A kind's damage against its resistance.
#[derive(Serialize)]
pub struct Dealt {
    pub damage: f32,
    pub resistance: f32,
    /// The levels of an attacker that is no player's above its defender's.
    pub above: u32,
    /// The share `p` of `0x43EEE0` that the resolution passes: `(1 + above/100)/2` in floats.
    pub p: f32,
    /// What `0x43EEE0` returns, with the base 1: the damage that the resistance lets through.
    pub dealt: f64,
}

const DAMAGES: [f32; 10] = [
    0.0, 0.01, 1.0, 3.33, 4.43, 12.04, 15.0, 21.67, 100.0, 1234.5,
];
const RESISTANCES: [f32; 9] = [0.0, 0.0005, 0.001, 0.002, 1.0, 4.48, 6.62, 30.0, 1000.0];
const ABOVE: [u32; 8] = [0, 1, 10, 50, 98, 99, 100, 150];

/// The damages that the resistances let through, by the levels above (`0x43EEE0` as `0x56B130`
/// calls it for each kind).
pub fn dealt(machine: &mut Machine<'_>) -> Result<Vec<Dealt>> {
    let mut vectors = Vec::new();
    for above in ABOVE {
        let p = share(above);
        for damage in DAMAGES {
            for resistance in RESISTANCES {
                let (dealt, _) = weight(machine, damage, resistance, 1.0, p)?;
                vectors.push(Dealt {
                    damage,
                    resistance,
                    above,
                    p,
                    dealt,
                });
            }
        }
    }
    Ok(vectors)
}

/// The share that the resolution passes for an attacker `above` levels above its defender
/// (docs/re/fight.md, "A strike's resolution"): 1 plus a hundredth of each level, kept as a
/// float, then half of it.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the resolution keeps the factor as a float"
)]
fn share(above: u32) -> f32 {
    let factor = (f64::from(above) * f64::from(0.01_f32) + 1.0) as f32;
    factor * 0.5
}

/// A creature's most health.
#[derive(Serialize)]
pub struct MostHealth {
    /// `Creature.pak`'s six base attributes, the stats' `b_i`.
    pub base: [u16; 6],
    pub level: u16,
    /// A hero's type, which takes no level's factor.
    pub hero: bool,
    /// The attributes `a_i` as the recomputation `0x5796A0` makes them of a creature without
    /// extra points or items: `b_i + ⌊b_i·(L − 1)/10⌋`.
    pub attributes: [u16; 6],
    /// The most health, and the health and the value shown that a new creature takes.
    pub most: i32,
    pub health: i32,
    pub shown: i32,
}

/// The base attributes of the set: the wolf's of docs/re/fight.md (512), the example of
/// `opensacred-rules`' test, nothing, a strong one past the exponent's bound of 60, and others.
const BASES: [[u16; 6]; 8] = [
    [8, 10, 18, 13, 0, 1],
    [14, 12, 20, 16, 2, 4],
    [20, 0, 10, 15, 0, 0],
    [0; 6],
    [40, 10, 10, 30, 5, 5],
    [1, 1, 1, 1, 1, 1],
    [25, 25, 25, 25, 25, 25],
    [12, 30, 5, 22, 8, 3],
];
const LEVELS: [u16; 9] = [1, 2, 5, 10, 30, 60, 100, 150, 216];

/// The most health of the bases at the levels, of a hero's type and not (`0x5658F0`), on the
/// first difficulty, with the balance's level factor.
pub fn most_health(machine: &mut Machine<'_>) -> Result<Vec<MostHealth>> {
    let mut vectors = Vec::new();
    for hero in [true, false] {
        machine.restub(IS_HERO, u32::from(hero))?;
        for base in BASES {
            for level in LEVELS {
                machine.clear_objects();
                let creature = machine.object(CREATURE_SIZE)?;
                let stats = machine.object(STATS_SIZE)?;
                let attributes = base.map(|b| {
                    let grown = u32::from(b) * (u32::from(level) - 1) / 10;
                    b.saturating_add(u16::try_from(grown).unwrap_or(u16::MAX))
                });
                let words = |values: [u16; 6]| -> Vec<u8> {
                    values.iter().flat_map(|v| v.to_le_bytes()).collect()
                };
                machine.write(stats + STATS_CREATURE, &creature.to_le_bytes())?;
                machine.write(stats + STATS_TYPE, &1_u32.to_le_bytes())?;
                machine.write(stats + STATS_ATTRIBUTES, &words(attributes))?;
                machine.write(stats + STATS_BASE, &words(base))?;
                machine.write(stats + STATS_LEVEL, &level.to_le_bytes())?;
                // A new creature's health and value shown, which the most health fills.
                machine.write(stats + STATS_SHOWN, &100_i32.to_le_bytes())?;
                machine.write(stats + STATS_HEALTH, &100_i32.to_le_bytes())?;
                machine.call(MOST_HEALTH, Some(stats), &[], false)?;
                let int = |machine: &Machine<'_>, at: u32| -> Result<i32> {
                    let bytes: [u8; 4] =
                        machine.read(stats + at, 4)?.try_into().unwrap_or_default();
                    Ok(i32::from_le_bytes(bytes))
                };
                vectors.push(MostHealth {
                    base,
                    level,
                    hero,
                    attributes,
                    most: int(machine, STATS_MOST)?,
                    health: int(machine, STATS_HEALTH)?,
                    shown: int(machine, STATS_SHOWN)?,
                });
            }
        }
    }
    Ok(vectors)
}
