//! A creature's combat arts (docs/re/arts.md): the list its stats hold, an art's level in play,
//! each entry's regeneration time by its record and its skills, the runes that teach the arts,
//! the hero's arts at a new game, and their regeneration: a use that starts it for every art of
//! its kind and the frames that run it down; and how the weapon arts `Attacke`, `Rundum` and
//! `Hardhit` strike, their factors and their targets (#508), whose chance and damage
//! `fight::art_strike` resolves. `opensacred-game` keeps the hero's list; the combos are not here.

use opensacred_data::heroes::HeroArt;
use opensacred_data::tables::{Arts as Tables, Balance, WeaponArt};

use crate::fight::curve;

/// The bit of an entry's flags that a rune sets: on every art it adds (`0x580750`) and on every
/// art it raises for good (`0x581F80`).
pub const TAUGHT: u16 = 1;
/// The Vampiress's arts by day and by night, in pairs (`0x580750`, `0x552370`).
const VAMPIRESS: [(u16, u16); 8] = [
    (1022, 1023),
    (1024, 1025),
    (1026, 1027),
    (1028, 1029),
    (1073, 1038),
    (1074, 1039),
    (1075, 1040),
    (1076, 1041),
];
/// The Daemon's arts in pairs (`0x891154`, read by `0x552370`).
const DAEMON: [(u16, u16); 5] = [
    (1094, 1091),
    (1097, 1092),
    (1099, 1101),
    (1098, 1102),
    (1100, 1103),
];
/// The arts that `0x581F80` raises with an art, where the creature holds them.
const RAISED_WITH: [(u16, u16); 19] = [
    (1022, 1023),
    (1024, 1025),
    (1026, 1027),
    (1028, 1029),
    (1000, 1038),
    (1001, 1039),
    (1002, 1040),
    (1009, 1041),
    (1073, 1038),
    (1074, 1039),
    (1075, 1040),
    (1076, 1041),
    (1025, 1024),
    (1027, 1026),
    (1029, 1028),
    (1038, 1073),
    (1039, 1074),
    (1040, 1075),
    (1041, 1076),
];
/// The skills that shorten the times of arts (`0x566070` through `0x565DC0`): each skill, the
/// balance's keys of its curve (`…off`, `…__s`, `…__w`) and the arts it shortens.
const SHORTENING: [(u8, &str, &[u16]); 14] = [
    (1, "HM", &[13, 15, 17, 18, 19, 20, 21, 22, 1019]),
    (15, "FM", &[5, 11, 23, 24]),
    (16, "WM", &[27, 4, 34, 35]),
    (17, "EM", &[2, 16, 25, 26]),
    (18, "LM", &[1, 28, 29, 33]),
    (19, "MM", &[3, 7, 36, 37, 38, 39, 40, 41, 42]),
    (
        20,
        "KO",
        &[
            1023, 1025, 1027, 1029, 1038, 1039, 1040, 1041, 1032, 43, 1030, 1031, 1033, 1034, 1035,
            1036, 1037,
        ],
    ),
    (
        25,
        "KO",
        &[
            1000, 1001, 1002, 1003, 1004, 1005, 1006, 1007, 1008, 1009, 1010, 1011, 1012, 1013,
            1014, 1015, 1016, 1017, 1018, 1020, 1057, 1058, 1059, 1060, 1061, 1062, 1063, 1064,
            1065, 1066, 1067, 1068, 1069, 1070, 1071, 1072, 1073, 1074, 1075, 1076, 1054, 1022,
            1024, 1026, 1028, 1042, 1043, 1044, 1045, 1046, 1091, 1094, 1092, 1097, 1102, 1098,
            1101, 1099, 1103, 1100, 1078, 1079, 1080, 1082, 1083, 1084, 1089,
        ],
    ),
    (26, "BA", &[1047, 1048, 1049, 1050, 1051, 1052, 1053]),
    (27, "KO", &[1047, 1048, 1049, 1050, 1051, 1052, 1053]),
    (
        28,
        "BL",
        &[1022, 43, 1032, 1030, 1031, 1033, 1034, 1035, 1036, 1037],
    ),
    (29, "WT", &[1087, 1086, 1085, 55, 56, 57, 58, 59]),
    (31, "KO", &[1087, 1086, 1085, 55, 56, 57, 58, 59]),
    (32, "HOM", &[60, 61, 62, 63, 64, 65, 66, 67]),
];
/// The original's curves of the skills that shorten the arts' times: `KO`'s, and every other's.
const KO: [f32; 3] = [12.0, 50.0, 300.0];
const SHORTENS: [f32; 3] = [6.0, 50.0, 100.0];
/// The groups of arts whose uses start the regeneration of their own (`0x562390`): the Dark Elf's
/// traps and the Vampiress's bites and bats.
const GROUPS: [&[u16]; 2] = [
    &[1051, 1052, 1053, 1048, 1049, 1047, 1050],
    &[1030, 1031, 1032, 1033, 1034, 1035, 1036, 1037],
];
/// The bits of the stats' `+0x148` that take the time a use adds to 0.8 of it (`0x562470`): for
/// the spells and for the weapon arts, set by the items' and the sets' effects (`0x571710`).
pub const QUICKER_SPELLS: u32 = 8;
pub const QUICKER_WEAPON_ARTS: u32 = 4;
/// What is still to run of an art that is ready, at most (`0x580DF0`).
const READY: f32 = 0.01;
/// The arts that `0x581D40` finds for the generic `Rundum`, `Hardhit`, `Attacke` and `Kick`
/// (1000, 1001, 1002, 1009): every class's own.
const OWN: [(u16, &[u16]); 4] = [
    (1000, &[1078, 1090, 1057, 1069, 1065, 1061, 1073, 1038]),
    (1001, &[1079, 1091, 1058, 1062, 1066, 1070, 1074, 1039]),
    (1002, &[1080, 1092, 1059, 1063, 1067, 1071, 1075, 1040]),
    (1009, &[1093, 1060, 1064, 1068, 1072, 1076, 1041]),
];

/// How far beyond the two creatures' reach an `Attacke` strikes (`0x4FF0B0`, `0x5038F0`).
pub const ATTACKE_BEYOND: u32 = 45;
/// What is left of its clip, as a part of it, below which the Gladiator's `Rundum` looks for
/// its creatures (`0x4FF0B0` at `0x500CCC`): from 0.3 of the clip to its end.
const RUNDUM_LEFT: f32 = 0.7;
/// How near a bone a `Rundum`'s creature must be, whatever its radius (`0x4FF0B0`).
const RUNDUM_MOST: u32 = 1000;

/// How a weapon art strikes, by its executor (docs/re/arts.md, "An art in a strike").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// A strike at each hit point of its clip on its target, or on the next hostile creature,
    /// within the reach and [`ATTACKE_BEYOND`].
    Attacke,
    /// Strikes, while it looks ([`rundum_looks`]), the creatures nearest its weapons' and hands'
    /// bones within its radius ([`rundum_radius`]).
    Rundum,
    /// One strike on its target at its clip's hit point.
    Hardhit,
    /// One strike on its target at its class's point of its clip ([`kick_point`]), which may
    /// knock it back ([`knockback_chance`], [`knockback`]).
    Kick,
}

/// How the art `art` strikes: the classes' own `Rundum`, `Hardhit`, `Attacke` and `Kick`, which
/// `0x581D40` finds for the generic 1000, 1001, 1002 and 1009 (the Gladiator's 1057 to 1060 and
/// the Seraphim's 1069 to 1072 read in their executors, the others' `Attacke` by the same test of
/// the reach; **hypothesis** for the other classes' `Rundum`, `Hardhit` and `Kick`); none for
/// another.
pub fn style(art: u16) -> Option<Style> {
    OWN.iter()
        .zip([Style::Rundum, Style::Hardhit, Style::Attacke, Style::Kick])
        .find(|((_, own), _)| own.contains(&art))
        .map(|(_, style)| style)
}

/// The factor of a strike of the weapon art `record` at its level in play `level`, which its
/// executor sends with the strike: `L·[+0x54] + [+0x50]` at 53 bits, stored as a float.
pub fn factor(record: &WeaponArt, level: u32) -> f32 {
    let [base, step] = record.factor.map(f64::from);
    narrow(f64::from(level).mul_add(step, base))
}

/// The factor that a creature's strike event carries (`0x4EA3F0`): halved where it holds two
/// weapons or a weapon of kind 8, 0.5 for a factor of 0 or below.
pub fn held_factor(factor: f32, halved: bool) -> f32 {
    match (halved, factor <= 0.0) {
        (false, _) => factor,
        (true, true) => 0.5,
        (true, false) => factor * 0.5,
    }
}

/// A creature that an art may strike, as its executor finds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub id: u32,
    /// Its distance from the striker, or from the bone, in world units (`0x5370B0`).
    pub distance: u32,
    /// The reach of the striker and of it (`0x4DBC60`).
    pub reach: u32,
    /// Whether it lives and the striker may strike it (`0x549080`, `0x548F60`).
    pub hostile: bool,
}

/// The creature that an `Attacke` strikes at a hit point of its clip (`0x4FF0B0`, `0x5038F0`):
/// its target while that lives within the reach and [`ATTACKE_BEYOND`], else the first of the
/// world's creatures (`others`, in its list's order) other than it that is hostile within them,
/// which becomes its target; none ends the art.
pub fn attacke_target(target: Option<Candidate>, others: &[Candidate]) -> Option<u32> {
    let within = |c: &Candidate| c.hostile && c.distance <= c.reach + ATTACKE_BEYOND;
    if let Some(target) = target.filter(within) {
        return Some(target.id);
    }
    let current = target.map(|t| t.id);
    others
        .iter()
        .find(|c| Some(c.id) != current && within(c))
        .map(|c| c.id)
}

/// Whether the `Rundum` of a creature of type `kind`, `done` of its clip played, looks for the
/// creatures about its bones: the Gladiator's (2) while what is left of its clip is less than
/// [`RUNDUM_LEFT`] of it (`0x4FF0B0` at `0x500CCC`), the Seraphim's (`0x5038F0` at `0x504F37`),
/// the Dark Elf's and the Wood Elf's throughout (docs/re/arts.md, "A `Rundum`'s run", #644).
pub fn rundum_looks(kind: u32, done: f32) -> bool {
    kind != 2 || 1.0 - done < RUNDUM_LEFT
}

/// The radius about a bone within which a `Rundum` of `record` at its level in play `level`
/// strikes (`0x4FF0B0`): `[+0x60] + [+0x64]·L` per cent of the reach `reach`.
pub fn rundum_radius(record: &WeaponArt, level: u32, reach: u32) -> f32 {
    let [base, step] = record.value_60.map(i64::from);
    let percent = base + step * i64::from(level);
    #[expect(
        clippy::cast_precision_loss,
        reason = "a radius in per cent of a reach, a few hundred, as the original's float"
    )]
    let (reach, percent) = (reach as f32, percent as f32);
    reach * percent * 0.01
}

/// The creature that a `Rundum` strikes by a bone of a weapon or a hand (`0x4FF0B0`): the nearest
/// of `candidates` (their distances from the bone) within 1,000 and its radius by its reach with
/// the striker (`radius`, [`rundum_radius`], as `0x4DBC60` gives it for each), unless it is the
/// one it struck last (`+0x190`) or none may be struck; candidates of the same distance keep the
/// first.
pub fn rundum_target(
    candidates: &[Candidate],
    radius: impl Fn(u32) -> f32,
    last: Option<u32>,
) -> Option<u32> {
    let mut nearest: Option<&Candidate> = None;
    for c in candidates {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a distance in world units, as the original's float"
        )]
        let within = (c.distance as f32) < radius(c.reach);
        if c.distance < RUNDUM_MOST && within && nearest.is_none_or(|n| c.distance < n.distance) {
            nearest = Some(c);
        }
    }
    nearest
        .filter(|c| c.hostile && Some(c.id) != last)
        .map(|c| c.id)
}

/// The clip of an art's action (`0x593320` from the executors): a motion slot of the creature's
/// model, or one of its attack's clips (the action 4, `0x5467A0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtClip {
    Slot(u16),
    Attack,
}

/// What a creature's art plays: its clip, and the rate it plays at as a factor of its attack's
/// clip speed (the attack speed over 100, `0x584FE0`), or, `fixed`, the rate itself (the kicks',
/// the Dwarf's and the Daemon's), and the slot it falls back to where the model lacks the clip's
/// (the Dwarf's).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArtAction {
    pub clip: ArtClip,
    pub rate: f32,
    pub fixed: bool,
    pub fallback: Option<u16>,
}

/// The Seraphim's `Attacke`'s rate with a weapon of the classes it plays the slot 211 for
/// (`0x891080`).
const SERAPHIMS_ATTACKE: f32 = 0.73;

/// What the art of `style` of a creature of type `kind` plays, holding what is of `class`
/// (`motions::class`, `0x546F80`), on foot (the Gladiator's `0x4FF0B0`, the Seraphim's
/// `0x5038F0`, the Dark Elf's `0x506770`, the Wood Elf's `0x50AC00`, the Vampiress's
/// `0x50E6C0`, the Dwarf's `0x514790`, the Daemon's `0x5192E0`; docs/re/arts.md, "The arts'
/// clips" and "The other classes' clips", #649): none with a bow or a crossbow (the classes 7
/// and 12, whose arts are shots, not here), nor for the Daemon's `Rundum` and charge, the other
/// classes' kicks and the Mage, not read. The Vampiress by night (7) plays her claws' column
/// (**hypothesis**: her night form is the one of the flag `+0x14` bit `0x2000`).
pub fn action(kind: u32, style: Style, class: u8) -> Option<ArtAction> {
    if matches!(class, 7 | 12) {
        return None;
    }
    let slot = slot_of;
    let kick = |slot| ArtAction {
        fixed: true,
        ..slot_of(slot)
    };
    // The Dwarf's and the Daemon's play at the rate 1.0, the Dwarf's falling back to `back`.
    let plain = |slot| ArtAction {
        fixed: true,
        ..slot_of(slot)
    };
    let dwarfs = |slot, back| ArtAction {
        fallback: Some(back),
        ..plain(slot)
    };
    let attack = ArtAction {
        clip: ArtClip::Attack,
        ..slot_of(0)
    };
    match (kind, style) {
        (1, Style::Attacke) => Some(match class {
            2 => slot(212),
            4 => slot(214),
            8 => slot(216),
            _ => ArtAction {
                rate: SERAPHIMS_ATTACKE,
                ..slot(211)
            },
        }),
        (1, Style::Rundum) => Some(slot(153)),
        (1, Style::Hardhit) => Some(attack),
        (1, Style::Kick) => Some(kick(81)),
        (2, Style::Kick) => Some(kick(148)),
        (2, Style::Attacke) => Some(slot(match class {
            0 => 210,
            2 => 212,
            3 => 213,
            8 => 216,
            _ => 211,
        })),
        (2, Style::Rundum) => Some(slot(match class {
            2 => 221,
            3 => 222,
            _ => 220,
        })),
        (2, Style::Hardhit) => Some(slot(match class {
            0 => 201,
            2 => 203,
            3 => 204,
            8 => 207,
            _ => 202,
        })),
        (4, Style::Attacke) => Some(slot(match class {
            0 => 210,
            2 => 212,
            8 => 216,
            _ => 211,
        })),
        (4, Style::Hardhit) => Some(match class {
            0 => slot(145),
            1 => slot(202),
            2 => slot(203),
            4 => slot(205),
            8 => slot(207),
            _ => attack,
        }),
        (4, Style::Rundum) => Some(slot(match class {
            0 => 219,
            2 => 221,
            4 => 223,
            8 => 225,
            _ => 220,
        })),
        (5, Style::Attacke) => Some(slot(match class {
            0 => 210,
            4 => 214,
            5 => 215,
            _ => 211,
        })),
        (5, Style::Hardhit) => Some(slot(match class {
            0 => 201,
            4 => 205,
            5 => 206,
            _ => 202,
        })),
        (5, Style::Rundum) => Some(slot(match class {
            0 => 219,
            4 => 223,
            5 => 224,
            _ => 220,
        })),
        (7, Style::Attacke) => Some(slot(210)),
        (7, Style::Hardhit) => Some(slot(201)),
        (7, Style::Rundum) => Some(slot(219)),
        (6, Style::Attacke) => Some(slot(match class {
            2 => 212,
            8 => 216,
            _ => 211,
        })),
        (6, Style::Hardhit) => Some(attack),
        (6, Style::Rundum) => Some(slot(if class == 8 { 225 } else { 221 })),
        (8, Style::Attacke) => Some(dwarfs(
            match class {
                0 => 210,
                2 => 212,
                3 => 213,
                8 => 216,
                13 | 14 => 248,
                _ => 211,
            },
            211,
        )),
        (8, Style::Hardhit) => Some(dwarfs(
            match class {
                0 => 201,
                2 => 203,
                3 => 204,
                8 => 207,
                13 => 247,
                14 => 148,
                _ => 202,
            },
            202,
        )),
        (8, Style::Rundum) if !matches!(class, 13 | 14) => Some(dwarfs(
            match class {
                0 => 219,
                2 => 221,
                3 => 222,
                _ => 220,
            },
            220,
        )),
        (9, Style::Attacke) => Some(plain(match class {
            0 => 210,
            2 => 212,
            3 => 213,
            8 => 216,
            _ => 211,
        })),
        _ => None,
    }
}

/// The hit points of an art's clip, the fractions of it at which its strikes are due: its
/// motion's floats from `MotionDesc +0xA0` (`floats_62[1..]` of `tables/motions.ron`), the
/// `n`-th read for the `n`-th strike (`0x4135D0` at `8 + n`), up to the first that is not above
/// 0, which ends an `Attacke`.
pub fn hit_points(floats: &[f32]) -> Vec<f32> {
    floats
        .iter()
        .copied()
        .take_while(|&point| point > 0.0)
        .collect()
}

/// The point of a `Hardhit`'s one strike: its clip's first hit point, half the clip where that is
/// not within (0, 1] (`0x4FF0B0`, `0x5038F0`).
pub fn hardhit_point(points: &[f32]) -> f32 {
    points
        .first()
        .copied()
        .filter(|&point| point > 0.0 && point <= 1.0)
        .unwrap_or(0.5)
}

/// The clip of the slot `slot` at its attack's rate.
fn slot_of(slot: u16) -> ArtAction {
    ArtAction {
        clip: ArtClip::Slot(slot),
        rate: 1.0,
        fixed: false,
        fallback: None,
    }
}

/// The point of its clip at which a kick strikes, of a creature of type `kind` holding what is of
/// `class`: the Gladiator's (`0x4FF0B0` at `0x5019D6`) once what is left of the clip is less than
/// 0.4 of it, 0.45 with a weapon of class 2 or 3; the Seraphim's (`0x5038F0` at `0x50408E`) once
/// it is less than 0.6 (docs/re/arts.md, "The kicks", #650).
pub fn kick_point(kind: u32, class: u8) -> f32 {
    match (kind, class) {
        (1, _) => 0.4,
        (_, 2 | 3) => 0.55,
        _ => 0.6,
    }
}

/// The chance in per cent that a kick of the level in play `level` knocks back its target of
/// type `creature` and level `target` (`0x4FC9D0`): `100 · 5L/M`, 0 for a target of level 0 and
/// for the types that no kick moves, times 1/1.3 for a boss (`+0x1F4` bit `0x80000`) and 1/1.2
/// for a big type (`Creature.pak` flags bit `0x02`, `0x426CB0`); a roll of 0 to 99 below it
/// knocks the target back. `M` is the target's level as it stands for a creature (`+0x3FE`), the
/// difficulty's for a player's (`0x552230`, not here).
pub fn knockback_chance(level: u32, (creature, target): (u32, u32), boss: bool, big: bool) -> f32 {
    const UNMOVED: [u32; 28] = [
        52, 53, 55, 74, 88, 100, 101, 102, 103, 136, 137, 141, 144, 182, 183, 185, 237, 250, 251,
        252, 336, 348, 360, 363, 365, 367, 701, 1249,
    ];
    if target == 0 || UNMOVED.contains(&creature) {
        return 0.0;
    }
    let mut chance = narrow(f64::from(5 * level) / f64::from(target));
    if boss {
        chance *= 0.769_230_8;
    }
    if big {
        chance *= 0.833_333_3;
    }
    chance * 100.0
}

/// How far a kick of `record` at the level in play `level` knocks back its target of level
/// `target` (`0x4FF0B0`): `[+0x60] · min(1, 5L/M)` in whole units, none at 10 or less; the
/// target slides away from the kicker at [`KNOCKBACK_SPEED`] until it has gone that far
/// (`0x594D12`).
pub fn knockback(record: &WeaponArt, level: u32, target: u32) -> Option<u32> {
    let ratio = if target == 0 {
        1.0
    } else {
        (f64::from(5 * level) / f64::from(target)).min(1.0)
    };
    let distance = whole_units(f64::from(record.value_60[0]) * ratio);
    u32::try_from(distance)
        .ok()
        .filter(|&distance| distance > 10)
}

/// A whole number of units, as the original's `_ftol` truncates it.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a distance of a few thousand units at most, truncated as the original's _ftol"
)]
fn whole_units(value: f64) -> i64 {
    value
        .trunc()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i64
}

/// How fast a knocked-back creature slides away from its kicker, in units a second, and the most
/// of a frame's time that one step takes (`0x594D12`: `0x890D5C`, `0x8907E4`).
pub const KNOCKBACK_SPEED: f32 = 400.0;
pub const KNOCKBACK_STEP: f32 = 0.3;

/// Whether a strike of an art at the hit point `point` is due, `done` of its clip played: past
/// the point (the executors wait while what is left of the clip is at least `1 − point` of it).
pub fn due(point: f32, done: f32) -> bool {
    done > point
}

/// How far from its target a creature makes an art of the record's distance `distance`, and how
/// near that it must stand (`0x4FD0F0`): 101 to 250 the reach of the two creatures `reach`,
/// within 10 of it; 0 where it stands; none for another, the jumps' below 0 among them (not
/// here).
pub fn made_from(distance: i32, reach: f32) -> Option<(f32, f32)> {
    match distance {
        0 => Some((0.0, f32::INFINITY)),
        100..=250 => Some((reach, MADE_WITHIN)),
        _ => None,
    }
}

/// How near the distance an art is made from its creature must stand (`0x4FD0F0`; 120 for a
/// rider, not here).
const MADE_WITHIN: f32 = 10.0;

/// The table an art is of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Table {
    /// The first, the spells.
    Spells,
    /// The second, the weapon arts.
    WeaponArts,
}

impl Table {
    /// The stats' number of the table: 1 or 2.
    pub fn number(self) -> u32 {
        match self {
            Self::Spells => 1,
            Self::WeaponArts => 2,
        }
    }

    /// The table of the stats' number `number`.
    pub fn of_number(number: u32) -> Option<Self> {
        match number {
            1 => Some(Self::Spells),
            2 => Some(Self::WeaponArts),
            _ => None,
        }
    }
}

/// An entry of a creature's list of arts (the stats' `+0x102`, 22 bytes each).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    pub table: Table,
    pub art: u16,
    pub level: u8,
    /// A bonus to the level, of the items and the sets (`0x571710`).
    pub bonus: u8,
    /// [`TAUGHT`], and the shortcut's slot in the high byte (`0x580C80`, `0x580D00`).
    pub flags: u16,
    /// The regeneration time, in seconds.
    pub time: f32,
    /// The regeneration's factor, 1.0 (`0x571080`).
    pub factor: f32,
    /// The regeneration still to run, in seconds.
    pub remaining: f32,
}

impl Entry {
    /// A template's entry, as the hero's loader reads it (`0x585C60`); none of a table but the
    /// two.
    pub fn of_template(art: &HeroArt) -> Option<Self> {
        Some(Self {
            table: Table::of_number(art.table)?,
            art: art.id,
            level: art.level,
            bonus: art.bonus,
            flags: art.flags,
            time: art.time,
            factor: art.factor,
            remaining: art.remaining,
        })
    }

    /// Whether a rune taught it ([`TAUGHT`]).
    pub fn taught(&self) -> bool {
        self.flags & TAUGHT != 0
    }

    /// The shortcut's slot, 0 for none.
    pub fn slot(&self) -> u8 {
        self.flags.to_be_bytes()[0]
    }

    /// The level in play: the level and the bonus.
    pub fn in_play(&self) -> u32 {
        u32::from(self.level) + u32::from(self.bonus)
    }

    /// The kind whose uses start its regeneration (`0x562470`): its table's, the spells' for
    /// 1019 and 1085 to 1087.
    pub fn kind(&self) -> Table {
        if matches!(self.art, 1019 | 1085..=1087) {
            Table::Spells
        } else {
            self.table
        }
    }

    /// Whether it regenerates at the spells' speed (`0x562730`): an art of the first table, and
    /// 1019; 1085 to 1087, which start as spells, run at the weapon arts'.
    fn at_spells_speed(&self) -> bool {
        self.table == Table::Spells || self.art == 1019
    }
}

/// A creature as its arts take it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Creature {
    /// Its type (`+0x10`): a hero's is its class, 1 the Seraphim to 9 the Daemon (6 and 7 the
    /// Vampiress by day and by night).
    pub kind: u32,
    /// Its level (the stats' `+0x56`).
    pub level: u32,
    /// Whether it is a player's.
    pub player: bool,
}

/// The bit of a creature's class in the tables' masks (`0x4D1B20`, `0x4D24C0`); 0 for a creature
/// that is no hero.
pub fn class_mask(kind: u32) -> u32 {
    match kind {
        1 => 0x1,
        2 => 0x2,
        3 => 0x4,
        4 => 0x8,
        5 => 0x10,
        6 | 7 => 0x20,
        8 => 0x0040_0000,
        9 => 0x0080_0000,
        _ => 0,
    }
}

/// The class's own `Rundum` and `Hardhit` for the generic 1061 and 1062 (`0x580A10`): the Mage's
/// and the Wood Elf's stay 1061 and 1062, the Wood Elf's own.
pub fn own(art: u16, kind: u32) -> u16 {
    match (art, kind) {
        (1061, 1) => 1069,
        (1061, 2) => 1057,
        (1061, 4) => 1065,
        (1061, 6 | 7) => 1073,
        (1061, 8) => 1078,
        (1061, 9) => 1090,
        (1062, 1) => 1070,
        (1062, 2) => 1058,
        (1062, 4) => 1066,
        (1062, 6 | 7) => 1074,
        (1062, 8) => 1079,
        (1062, 9) => 1091,
        _ => art,
    }
}

/// An art's counterpart in the creature's other form, as a rune takes it (`0x552370` with its
/// flag set): the Daemon's pairs both ways, the Vampiress's night art for her day art by day (6)
/// and her day art for her night art by night (7); the art itself for any other.
pub fn counterpart(art: u16, kind: u32) -> u16 {
    match kind {
        9 => DAEMON
            .iter()
            .find_map(|&(a, b)| {
                if a == art {
                    Some(b)
                } else if b == art {
                    Some(a)
                } else {
                    None
                }
            })
            .unwrap_or(art),
        6 => VAMPIRESS
            .iter()
            .find(|&&(_, night)| night == art)
            .map_or(art, |&(day, _)| day),
        7 => VAMPIRESS
            .iter()
            .find(|&&(day, _)| day == art)
            .map_or(art, |&(_, night)| night),
        _ => art,
    }
}

/// An art's regeneration time at the level `level`, from its record: a spell's `base + step·L`
/// in whole seconds (`0x4D25C0`), a weapon art's in floats (`0x4D1C00`); the first table is
/// searched first (`0x571080`), and none holds an art of neither.
pub fn time(tables: &Tables, art: u16, level: u32) -> Option<f64> {
    if let Some(spell) = tables.spells.iter().find(|s| s.id == art) {
        let [base, step] = spell.regeneration.map(u32::from);
        return Some(f64::from(base + step * level));
    }
    let record = tables.weapon_arts.iter().find(|a| a.id == art)?;
    let [base, step] = record.regeneration.map(f64::from);
    Some(f64::from(level).mul_add(step, base))
}

/// A float of the original's stores to an `f32`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a regeneration time, a few hundred seconds, rounded to the f32 the original stores"
)]
fn narrow(value: f64) -> f32 {
    value as f32
}

/// The speeds the arts regenerate at: the stats' `+0xEE` for the spells and `+0xF2` for the
/// weapon arts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Speeds {
    pub spells: f32,
    pub weapon_arts: f32,
}

impl Default for Speeds {
    fn default() -> Self {
        Self {
            spells: 1.0,
            weapon_arts: 1.0,
        }
    }
}

impl Speeds {
    /// The speeds of a creature of type `kind` (`0x5796A0`): the spells'
    /// `(1 + bonuses[0] + effect) · (1 + magic / 100)` and the weapon arts'
    /// `(1 + bonuses[1] + effect) · (1 + physical / 100)`, none below 0, the Dwarf's spells at
    /// the weapon arts' speed. `bonuses` are its items' bonuses 813 and 814 (`0x32D`, `0x32E`),
    /// each a hundredth of its value; `effect` the value of its effect `0xB`; `magic` and
    /// `physical` its attributes `MagicRegen` (4) and `PhysicalRegen` (3) with its items'.
    pub fn of(bonuses: [f32; 2], effect: f32, magic: f32, physical: f32, kind: u32) -> Self {
        let speed = |bonus: f32, attribute: f32| {
            let base = 1.0 + bonus + effect;
            narrow((f64::from(attribute) * 0.01 + 1.0) * f64::from(base)).max(0.0)
        };
        let weapon_arts = speed(bonuses[1], physical);
        let spells = if kind == 8 {
            weapon_arts
        } else {
            speed(bonuses[0], magic)
        };
        Self {
            spells,
            weapon_arts,
        }
    }

    /// The speed of an entry's kind.
    fn of_entry(self, entry: &Entry) -> f32 {
        if entry.at_spells_speed() {
            self.spells
        } else {
            self.weapon_arts
        }
    }
}

/// The curves of the skills that shorten the arts' times, the balance's `HM`, `FM`, `WM`, `EM`,
/// `LM`, `MM`, `KO`, `BA`, `BL`, `WT` and `HOM` (`0x566070`).
#[derive(Clone, Debug, PartialEq)]
pub struct Shortening {
    curves: Vec<(u8, [f32; 3], &'static [u16])>,
}

impl Default for Shortening {
    /// The original's curves.
    fn default() -> Self {
        Self {
            curves: SHORTENING
                .iter()
                .map(|&(skill, keys, arts)| (skill, if keys == "KO" { KO } else { SHORTENS }, arts))
                .collect(),
        }
    }
}

impl Shortening {
    /// The curves of `balance`, the original's where it lacks them.
    pub fn of(balance: &Balance) -> Self {
        let mut shortening = Self::default();
        for ((_, triple, _), &(_, keys, _)) in shortening.curves.iter_mut().zip(&SHORTENING) {
            for (value, suffix) in triple.iter_mut().zip(["off", "__s", "__w"]) {
                *value = balance.f32(&format!("{keys}{suffix}")).unwrap_or(*value);
            }
        }
        shortening
    }
}

/// What the stats' recomputation (`0x5796A0`) makes the arts' times of: the arts' tables, the
/// creature's type, its skills with their levels in their slots' order, and the curves of the
/// skills that shorten the times.
#[derive(Clone, Copy, Debug)]
pub struct Context<'a> {
    pub tables: &'a Tables,
    pub kind: u32,
    pub skills: &'a [(u8, u32)],
    pub shortening: &'a Shortening,
}

/// Whether a use of the art `group` starts the regeneration of `art` (`0x562390`): the art
/// itself, and else where both or neither are of the groups ([`GROUPS`]), whichever.
fn shares(art: u16, group: u16) -> bool {
    if art == group {
        return true;
    }
    let count = GROUPS
        .iter()
        .flat_map(|arts| arts.iter())
        .filter(|&&a| a == art || a == group)
        .count();
    count == 0 || count == 2
}

/// Why a rune does not teach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// It names the art 0.
    NoArt,
    /// Neither table holds its art for the reader's class.
    NotForClass,
    /// Its art's level is at 255 already.
    Highest,
}

/// A creature's list of arts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Arts {
    pub entries: Vec<Entry>,
}

impl Arts {
    /// The hero's arts at a new game, of its template's: the entries that no rune taught, their
    /// times recomputed ([`Self::recompute`]); a new game drops the arts its template marks
    /// [`TAUGHT`] (seen in the original 2026-10-23, #506: the Seraphim starts with 1071 alone,
    /// the Gladiator with 1059 alone, the Daemon with 1097 and 1092, the Vampiress with 1022 and
    /// 1023).
    pub fn new_game(template: &[HeroArt], context: &Context<'_>) -> Self {
        let mut arts = Self {
            entries: template
                .iter()
                .filter_map(Entry::of_template)
                .filter(|entry| !entry.taught())
                .collect(),
        };
        arts.recompute(context);
        arts
    }

    /// The arts' part of the stats' recomputation (`0x5796A0`): every time by its record
    /// ([`Self::times`]), then shortened by the skills ([`Self::shorten`]).
    pub fn recompute(&mut self, context: &Context<'_>) {
        self.times(context.tables);
        self.shorten(context.skills, context.shortening);
    }

    /// The skills' shortening of the times (`0x566070` through `0x565DC0`): for each skill in its
    /// slot's order that shortens arts, each art of its list that the creature holds has its time
    /// divided by `1 + v/100`, `v` the skill's curve at its level, held at 1 s at least, and what
    /// is still to run held to it.
    pub fn shorten(&mut self, skills: &[(u8, u32)], shortening: &Shortening) {
        for &(skill, level) in skills.iter().filter(|&&(_, level)| level > 0) {
            for (_, triple, arts) in shortening.curves.iter().filter(|(s, ..)| *s == skill) {
                let v = curve(*triple, level);
                for entry in self.entries.iter_mut().filter(|e| arts.contains(&e.art)) {
                    let time = narrow(f64::from(entry.time) / (f64::from(v) * 0.01 + 1.0));
                    entry.time = time.max(1.0);
                    entry.remaining = entry.remaining.min(entry.time);
                }
            }
        }
    }

    /// The index of the entry of `art` (`0x581D40`): for the generic `Rundum`, `Hardhit`,
    /// `Attacke` and `Kick` (1000, 1001, 1002, 1009) the first entry of any class's own, or of the
    /// art itself.
    pub fn index(&self, art: u16) -> Option<usize> {
        let own = OWN
            .iter()
            .find(|&&(generic, _)| generic == art)
            .map_or(&[][..], |&(_, own)| own);
        self.entries
            .iter()
            .position(|e| e.art == art || own.contains(&e.art))
    }

    /// The entry of `art` itself.
    pub fn get(&self, art: u16) -> Option<&Entry> {
        self.entries.iter().find(|e| e.art == art)
    }

    /// The level that `creature` makes `art` at (`0x580A10`): its entry's level and bonus, the
    /// class's own for the generic 1061 and 1062; a creature that is no player's and lacks the
    /// art makes it at `⌊its level / 5⌋ + 1`, a player's not at all.
    pub fn level_in_play(&self, art: u16, creature: Creature) -> Option<u32> {
        match self.get(own(art, creature.kind)) {
            Some(entry) => Some(entry.in_play()),
            None if !creature.player => Some(creature.level / 5 + 1),
            None => None,
        }
    }

    /// The sum of the levels of the arts without their bonuses, the drops' `a` for a rune
    /// (`0x5598B0`, docs/re/items.md).
    pub fn levels(&self) -> u32 {
        self.entries.iter().map(|e| u32::from(e.level)).sum()
    }

    /// Every entry's time by its record (`0x571080`): the time at its level, and with its bonus
    /// half of what the bonus adds where that is more, `v(L) + (v(L + b) − v(L))/2`; the factor
    /// 1.0. An art that neither table holds keeps its time.
    pub fn times(&mut self, tables: &Tables) {
        for entry in &mut self.entries {
            entry.factor = 1.0;
            let Some(at_level) = time(tables, entry.art, u32::from(entry.level)) else {
                continue;
            };
            entry.time = narrow(at_level);
            if let Some(with_bonus) = time(tables, entry.art, entry.in_play()) {
                let stored = f64::from(entry.time);
                if stored < with_bonus {
                    entry.time = narrow((with_bonus - stored).mul_add(0.5, stored));
                }
            }
        }
    }

    /// Adds `art` of `table` at level 0, marked [`TAUGHT`], unless the creature holds it
    /// (`0x580750`): with the Vampiress's other art of its pair, and with the art's counterpart
    /// ([`counterpart`]) where it is another, whether held or not; then every time
    /// ([`Self::recompute`]).
    pub fn add(&mut self, table: Table, art: u16, context: &Context<'_>) {
        let kind = context.kind;
        if self.get(art).is_some() {
            return;
        }
        let entry = |art| Entry {
            table,
            art,
            level: 0,
            bonus: 0,
            flags: TAUGHT,
            time: 0.0,
            factor: 0.0,
            remaining: 0.0,
        };
        self.entries.push(entry(art));
        if matches!(kind, 6 | 7)
            && let Some(&(_, night)) = VAMPIRESS.iter().find(|&&(day, _)| day == art)
        {
            self.entries.push(entry(night));
        }
        let other = counterpart(art, kind);
        if other != 0 && other != art {
            self.entries.push(entry(other));
        }
        self.recompute(context);
    }

    /// Raises the entry at `index` by `levels` (`0x581F80`): for good its level, marked
    /// [`TAUGHT`], with every time recomputed, else its bonus; refused where that passes 255. A
    /// time that grows adds as much to the regeneration still to run. With `linked`, the arts
    /// raised with it (`RAISED_WITH`) that the creature holds rise too, or else its
    /// [`counterpart`]. Whether it rose.
    pub fn raise(
        &mut self,
        index: usize,
        levels: u8,
        for_good: bool,
        linked: bool,
        context: &Context<'_>,
    ) -> bool {
        let Some(entry) = self.entries.get_mut(index) else {
            return false;
        };
        let value = if for_good {
            &mut entry.level
        } else {
            &mut entry.bonus
        };
        // At most 255.
        let Ok(raised) = u8::try_from(u32::from(*value) + u32::from(levels)) else {
            return false;
        };
        *value = raised;
        if for_good {
            entry.flags |= TAUGHT;
        }
        let (art, before) = (entry.art, entry.time);
        if for_good {
            self.recompute(context);
        }
        if let Some(entry) = self.entries.get_mut(index)
            && before < entry.time
        {
            entry.remaining += entry.time - before;
        }
        if linked {
            let with = RAISED_WITH
                .iter()
                .find(|&&(raised, _)| raised == art)
                .and_then(|&(_, with)| self.index(with));
            match with {
                Some(with) => {
                    self.raise(with, levels, for_good, false, context);
                }
                None => {
                    let other = counterpart(art, context.kind);
                    if other != art
                        && let Some(other) = self.index(other)
                    {
                        self.raise(other, levels, for_good, false, context);
                    }
                }
            }
        }
        true
    }

    /// Teaches `art` of `table` by `levels`, as that many runes would (`0x580750`, `0x581D40`,
    /// `0x581F80` for good with the linked arts), whatever the creature's class; whether it rose.
    pub fn teach(&mut self, table: Table, art: u16, levels: u8, context: &Context<'_>) -> bool {
        self.add(table, art, context);
        self.index(art)
            .is_some_and(|index| self.raise(index, levels, true, true, context))
    }

    /// A rune of `art` read by a creature of the type of `context` (`cItemUpgrade`, `0x5CD100`),
    /// its requirements met (`0x54ABE0`, the caller's): the art of the second table where that
    /// holds it for the class, else of the first, else refused; added at level 0 if lacking and
    /// raised by 1 for good. A refused rune is kept, a read one used up (its flag `0x8000000`).
    pub fn read_rune(&mut self, art: u16, context: &Context<'_>) -> Result<(), Refusal> {
        if art == 0 {
            return Err(Refusal::NoArt);
        }
        let tables = context.tables;
        let class = class_mask(context.kind);
        let table = if tables
            .weapon_arts
            .iter()
            .any(|a| a.id == art && a.classes & class != 0)
        {
            Table::WeaponArts
        } else if tables
            .spells
            .iter()
            .any(|s| s.id == art && s.classes & class != 0)
        {
            Table::Spells
        } else {
            return Err(Refusal::NotForClass);
        };
        if self.teach(table, art, 1, context) {
            Ok(())
        } else {
            Err(Refusal::Highest)
        }
    }

    /// Whether `art` is ready (`0x580DF0`): at most 0.01 s of its regeneration still to run; an
    /// art the creature lacks is.
    pub fn ready(&self, art: u16) -> bool {
        self.get(art).is_none_or(|entry| entry.remaining <= READY)
    }

    /// The time that a use of `art` adds (`0x582850`): its entry's, else its record's at level 0
    /// (`0x582A20`).
    pub fn time_of(&self, art: u16, tables: &Tables) -> f32 {
        match self.get(art) {
            Some(entry) => entry.time,
            None => time(tables, art, 0).map_or(0.0, narrow),
        }
    }

    /// A use of an art of `kind` (`0x562470`, which its executor calls once, at its first strike
    /// or its cast): `time` is added to what is still to run of every art of the kind
    /// ([`Entry::kind`]), 0.8 of it where `flags` (the stats' `+0x148`) has the kind's
    /// [`QUICKER_SPELLS`] or [`QUICKER_WEAPON_ARTS`], each held within 0 and its time; with
    /// `group`, an art its executor names, only the arts that share its start ([`shares`]).
    /// While a combo runs the original runs the combos' instead, which are not here.
    pub fn start(&mut self, time: f32, kind: Table, group: Option<u16>, flags: u32) {
        let quicker = match kind {
            Table::Spells => QUICKER_SPELLS,
            Table::WeaponArts => QUICKER_WEAPON_ARTS,
        };
        let added = if flags & quicker != 0 {
            time * 0.8
        } else {
            time
        };
        for entry in &mut self.entries {
            if entry.kind() != kind || group.is_some_and(|group| !shares(entry.art, group)) {
                continue;
            }
            entry.remaining = (entry.remaining + added).min(entry.time).max(0.0);
        }
    }

    /// `seconds` of the regeneration (`0x562730`, which the original runs on its world's tick of
    /// about 0.1 s, `0x5375C0`; the local player's hero's alone, not
    /// while its `+0x200` has the bit `0x1000000`): each art with time still to run loses
    /// `seconds · factor · speed`, the spells' speed for an art of the first table and 1019 and
    /// the weapon arts' for the others, held within 0 and its time.
    pub fn regenerate(&mut self, seconds: f32, speeds: Speeds) {
        for entry in &mut self.entries {
            if entry.remaining <= 0.0 {
                continue;
            }
            let rate = entry.factor * speeds.of_entry(entry);
            let left = f64::from(entry.remaining) - f64::from(rate) * f64::from(seconds);
            entry.remaining = narrow(left).max(0.0).min(entry.time);
        }
    }
}

#[cfg(test)]
mod tests;
