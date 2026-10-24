//! A creature's combat arts (docs/re/arts.md): the list its stats hold, an art's level in play,
//! each entry's regeneration time by its record and its skills, the runes that teach the arts,
//! the hero's arts at a new game, and their regeneration: a use that starts it for every art of
//! its kind and the frames that run it down; and how the weapon arts `Attacke`, `Rundum` and
//! `Hardhit` strike, their factors and their targets (#508), whose chance and damage
//! `fight::art_strike` resolves. `opensacred-game` keeps the hero's list; the combos are not here.

use opensacred_data::heroes::HeroArt;
use opensacred_data::tables::{Arts as Tables, Balance, WeaponArt};

use crate::fight::{Effect, curve};

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
    /// At each of its runs, a strike on the creature that the point of its socket of the item in
    /// its hand touches first ([`blade_touches`]), unless that is the one it struck last: the
    /// Seraphim's 1054 (`GUI_MOVE_SCHLEUDERKLINGEN`, `0x5038F0`).
    Blades,
    /// Her `Rundum`'s strikes, each creature struck knocked back by a kick's roll
    /// ([`knockback_chance`], [`whirl_knockback`]): the Seraphim's 1020
    /// (`GUI_MOVE_WIRBELSCHLAG`, `0x5038F0`, which runs it as her 1069).
    Whirl,
    /// A flight to its target and a landing ([`jump_landing`], [`jump_height`],
    /// [`jump_points`]), made where its maker stands ([`order_distance`]) at a rate by its
    /// distance ([`jump_rate`]): the Seraphim's 1017 (`GUI_MOVE_KAMPFSPRUNG`, `0x5038F0`) strikes
    /// its target as she lands, the Gladiator's 1003 (`0x4FF0B0`) every hostile about him and
    /// stuns some ([`jump_quake`], [`jump_stun`]).
    Jump,
    /// As its clip starts, every hostile creature about its maker within its radius
    /// ([`roar_radius`]) knocked back by a kick's roll ([`roar_knockback`]) and held
    /// ([`roar_hold`]): the Gladiator's 1014 (`GUI_MOVE_EHRFURCHT`, `0x4FF0B0`), of the launch
    /// type 2, made on himself.
    Roar,
    /// Once what is left of its clip is below [`VIPER_LEFT`], its target and every hostile of
    /// its maker's list near it ([`viper`]) stunned by a kick's roll, each with the effect 35:
    /// the Dark Elf's 1042 (`GUI_MOVE_NATTER`, `0x506770`).
    Viper,
    /// Its effect on its maker at a part of its clip ([`ward`]): the Dark Elf's arrowproof 1045
    /// (`GUI_MOVE_ARROWPROOF`, `0x506770`), of the launch type 2, made on himself.
    Ward,
    /// Its effects on its maker at a part of its clip, those of his other forms taken off first
    /// ([`form`]): the Daemon's forms 1097 to 1100 (`0x5192E0`), of the launch type 2, made on
    /// himself.
    Form,
    /// At once, without a clip: the effect of the multi-shot on its maker ([`volley`]), the Wood
    /// Elf's 1005 (`GUI_MOVE_MULTIPFEIL`, `0x50AC00`), of the launch type 0.
    Volley,
    /// A leap to a point on the ground at most its reach away ([`panther_reach`],
    /// [`panther_point`]), the Dark Elf's 1043 (`GUI_MOVE_PANTHER`, `0x506770`), of the launch
    /// type 3, at a rate by its reach ([`panther_rate`]).
    Panther,
    /// The Vampiress's change of form once what is left of its clip is below [`CHANGES`]: 1022
    /// (`GUI_MOVE_CHANGELING_DAY`) into her night's form for the effect 22's time
    /// ([`changeling_seconds`]), 1023 (`GUI_MOVE_CHANGELING`) back to her day's (`0x50E6C0`), of
    /// the launch type 0, made where she stands.
    Changeling,
    /// The Vampiress's dash, 1028 (`GUI_MOVE_SPEED_DAY`) and 1029 (`GUI_MOVE_SPEED`), of the
    /// launch type 0: once what is left of its clip is below [`DASHES`] she is put at its point,
    /// the action's at most its reach away ([`dash_reach`]) where she may stand (`0x50E6C0`).
    Dash,
    /// One strike on its target and the same on every hostile creature nearer that target than
    /// the reach of it and the maker, at a part of its clip ([`SMITE_LEFT`], [`smitten`]): the
    /// Gladiator's 1012 (`GUI_MOVE_HANDGOTTES`, `0x4FF0B0`).
    Smite,
    /// Effects on its maker at a part of its clip, made where it stands ([`boon`]): the Dark
    /// Elf's 1047 and 1048 and the Dwarf's 1089, of the launch type 5 (`ECF_ONMEMYSELFANDI`,
    /// whose target `0x51D840` makes the maker itself), and the Gladiator's 1015, of the type 2
    /// (`ECF_ONFRIENDLY`), on a friend or on himself, whom the right button on him makes its
    /// target (seen).
    Boon,
}

/// The arts that give their maker effects ([`Style::Boon`]).
const BOONS: [u16; 5] = [1015, 1047, 1048, 1089, 1094];

/// The key of the Daemon's forms' effects of attack speed and defence (`0x4321`), beside the
/// arts' own keys.
pub const DAEMON_FORM: u32 = 0x4321;

/// The arts that `0x51D840` refuses while an effect of theirs holds on their maker (`0x582B00`):
/// 1031, 1042, 1044 to 1048, 1082 and 1089, and 1018 and 1019 while either's does.
pub fn refused_while(art: u16) -> &'static [u16] {
    match art {
        1018 | 1019 => &[1018, 1019],
        1031 => &[1031],
        1042 => &[1042],
        1044 => &[1044],
        1045 => &[1045],
        1046 => &[1046],
        1047 => &[1047],
        1048 => &[1048],
        1082 => &[1082],
        1089 => &[1089],
        _ => &[],
    }
}

/// How the art `art` strikes: the classes' own `Rundum`, `Hardhit`, `Attacke` and `Kick`, which
/// `0x581D40` finds for the generic 1000, 1001, 1002 and 1009 (the Gladiator's 1057 to 1060 and
/// the Seraphim's 1069 to 1072 read in their executors, the others' `Attacke` by the same test of
/// the reach; **hypothesis** for the other classes' `Rundum`, `Hardhit` and `Kick`), and the
/// boons ([`Style::Boon`]); none for another.
pub fn style(art: u16) -> Option<Style> {
    if BOONS.contains(&art) {
        return Some(Style::Boon);
    }
    if art == 1012 {
        return Some(Style::Smite);
    }
    if art == 1054 {
        return Some(Style::Blades);
    }
    if art == 1020 {
        return Some(Style::Whirl);
    }
    if art == 1017 || art == 1003 {
        return Some(Style::Jump);
    }
    if art == 1014 {
        return Some(Style::Roar);
    }
    if art == 1042 {
        return Some(Style::Viper);
    }
    if art == 1045 {
        return Some(Style::Ward);
    }
    if (1097..=1100).contains(&art) {
        return Some(Style::Form);
    }
    if art == 1005 {
        return Some(Style::Volley);
    }
    if art == 1043 {
        return Some(Style::Panther);
    }
    if art == 1022 || art == 1023 {
        return Some(Style::Changeling);
    }
    if art == 1028 || art == 1029 {
        return Some(Style::Dash);
    }
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

/// Whether the blade of a [`Style::Blades`], its point `distance` from a creature of the size
/// `size` (`items.pak +0x14`, `arrows::size`), touches it, the item in the hand of the size
/// `blade` (`0x600340`: nearer than the sum of the two types' sizes, `0x428CE0`).
pub fn blade_touches(distance: f32, size: f32, blade: f32) -> bool {
    distance < size + blade
}

/// What is left of its clip, as a part of it, below which a [`Style::Smite`] strikes
/// (`0x4FF0B0`).
pub const SMITE_LEFT: f32 = 0.35;

/// The creatures that a [`Style::Smite`] strikes, by their ids: its target, if any, wherever it
/// stands, and every other of the hero's list of hostiles (`0xAD3CE0`, `others`, each its
/// distance from the target) nearer the target than the reach of its maker and the target,
/// `reach` (`0x5370B0` and `0x4DBC60` of the target).
pub fn smitten(target: Option<u32>, others: &[Candidate], reach: u32) -> Vec<u32> {
    target
        .into_iter()
        .chain(
            others
                .iter()
                .filter(|c| Some(c.id) != target && c.distance < reach)
                .map(|c| c.id),
        )
        .collect()
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
        // The boons, on foot, at the blend 0.5 and the rate 1.0: the Dark Elf's the slot 180
        // (`0x506770` at `0x50A09F`; mounted, 194 at 0.2, not followed), the Dwarf's 1089 the
        // slot 154 (`0x514790`).
        (4, Style::Boon) => Some(plain(180)),
        // The Gladiator's 1012 the slot 146 at the blend 0.5 and the rate 1.0 (`0x4FF0B0` at
        // `0x502C10`).
        (2, Style::Smite) => Some(plain(146)),
        // The Seraphim's 1054 the slot 148 at the blend 0.5 and the rate 1.6 (`0x5038F0` at
        // `0x505D44`).
        // The Seraphim's 1020 the slot 149 at the blend 0.5 and the rate 1.0 (`0x5038F0` at
        // `0x50619A`).
        (1, Style::Whirl) => Some(plain(149)),
        // The Seraphim's 1017 the slot 147 at the blend 0.15 (`0x5038F0` at `0x505C99`), its
        // rate by its distance ([`jump_rate`]).
        (1, Style::Jump) => Some(plain(147)),
        // The Gladiator's 1003 the slot 144 at the blend 0.15 (`0x4FF0B0` at `0x502619`), at the
        // same rate.
        (2, Style::Jump) => Some(plain(144)),
        // The Gladiator's 1014 the slot 149 at the blend 0.5 and the rate 1.0 (`0x4FF0B0` at
        // `0x502C2F`; 169 mounted).
        (2, Style::Roar) => Some(plain(149)),
        // The Dark Elf's 1042 the slot 148 at the blend 0.5 and the rate 1.0 (`0x506770` at
        // `0x509DEE`).
        (4, Style::Viper) => Some(plain(148)),
        // The Dark Elf's 1043 the slot 182 at the blend 0.5 (`0x506770` at `0x50A04D`), its rate
        // by its reach ([`panther_rate`]).
        (4, Style::Panther) => Some(plain(182)),
        // The changeling in either form: the slot 144 at the rate 0.7 (seen).
        (6 | 7, Style::Changeling) => Some(ArtAction {
            rate: CHANGELING_RATE,
            ..plain(144)
        }),
        // The dash: the slot 144 at the rate 5 (seen by day).
        (6 | 7, Style::Dash) => Some(ArtAction {
            rate: DASH_RATE,
            ..plain(144)
        }),
        // The Dark Elf's 1045 the slot 146 at the blend 0.5 and the rate 1.0 (`0x506770` at
        // `0x509E28`; 194 at the blend 0.2 mounted).
        (4, Style::Ward) => Some(plain(146)),
        (1, Style::Blades) => Some(ArtAction {
            rate: 1.6,
            ..plain(148)
        }),
        (8, Style::Boon) => Some(plain(154)),
        // The Daemon's 1094 the slot 150 at the blend 0.5 and the rate 1.0 (`0x5192E0` at
        // `0x51BC63`).
        (9, Style::Boon) => Some(plain(150)),
        // The Daemon's forms the slot 160 at the blend 0.2 and the rate 1.0 on himself
        // (`0x5192E0` at `0x51BEF5`; 197 at the attack's rate otherwise, not followed).
        (9, Style::Form) => Some(plain(160)),
        // The Gladiator's 1015 the slot 151 (`0x4FF0B0` at `0x502C5A`; 169 mounted).
        (2, Style::Boon) => Some(plain(151)),
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

/// What a boon art gives its maker ([`Style::Boon`]): the part of its clip at which it gives
/// it, and each effect with its seconds; and as it starts, the keys whose effects it takes off
/// (`0x5888F0`) and the effects it gives then.
#[derive(Clone, Debug, PartialEq)]
pub struct Boon {
    pub point: f32,
    pub effects: Vec<(Effect, f32)>,
    pub clears: Vec<u32>,
    pub starts: Vec<(Effect, f32)>,
    /// The keys whose effects it takes off at its point, before it gives its own.
    pub replaces: Vec<u32>,
}

/// The boon of the art `record` at its level in play `level` (docs/re/arts.md, "The special
/// weapon arts"), each effect keyed by the art, the values `L·[+0x54] + [+0x50]` (`a`) and
/// `L·[+0x5C] + [+0x58]` (`b`):
///
/// - the Dark Elf's 1047 once what is left of its clip is below 0.3, the effects 1 (`A`'s
///   multiplier) of `a` and 6 (the poison damage's) of `b`, and 1048 below 0.2 the effects 2
///   (`D`'s multiplier) of `a` and 12 (the regeneration) of `b`, each for `L·[+0x64] + [+0x60]`
///   seconds (`0x506770`);
/// - the Gladiator's 1015 below 0.3 the effects 1 and 2 of `a` for `b` seconds (`0x4FF0B0`);
/// - the Dwarf's 1089 below 0.2 the effects 1 of `a` and 2 of `b` for `L·[+0x64] + [+0x60]`
///   hundredths of a second (`0x514790`; to the hero, whom `0x51D840` refuses it while they
///   hold; a friend's effects of it raised by `0x5904E0`, not followed);
/// - the Daemon's 1094 (`0x5192E0`): as it starts, the effects keyed by 1094, 1097 to 1100 and
///   [`DAEMON_FORM`] off and the effect 50 for 10 s keyed by 1094 (`0x51BBC5`); below 0.3 the
///   effect 50 keyed by 1094, and the effects 2 of `b` and 52 (the attack speed and the pace) of
///   `(L·[+0x64] + [+0x60]) / 100` keyed by [`DAEMON_FORM`], each for `a` seconds at most 180
///   (times `1 + 0.4·f` for a Daemon of `+0x4F0` bit `0x8000`, not followed);
///
/// none for another.
pub fn boon(record: &WeaponArt, level: u32) -> Option<Boon> {
    let l = f64::from(level);
    let by_level = |[base, step]: [f32; 2]| narrow(l.mul_add(f64::from(step), f64::from(base)));
    let whole = |[base, step]: [i32; 2]| l.mul_add(f64::from(step), f64::from(base));
    let (a, b) = (by_level(record.factor), by_level(record.value_58));
    let keyed = |id, value, art| Effect {
        id,
        whole: 0,
        value,
        art,
        level: 0,
    };
    if record.id == 1094 {
        let art = u32::from(record.id);
        let seconds = a.min(180.0);
        let quickens = narrow(whole(record.value_60) * 0.01);
        return Some(Boon {
            point: 0.7,
            effects: vec![
                (keyed(50, 0.0, art), seconds),
                (keyed(2, b, DAEMON_FORM), seconds),
                (keyed(52, quickens, DAEMON_FORM), seconds),
            ],
            clears: vec![1094, 1097, 1098, 1099, 1100, DAEMON_FORM],
            starts: vec![(keyed(50, 0.0, art), 10.0)],
            replaces: Vec::new(),
        });
    }
    let (point, effects, seconds) = match record.id {
        1047 => (0.7, [(1, a), (6, b)], narrow(whole(record.value_60))),
        1048 => (0.8, [(2, a), (12, b)], narrow(whole(record.value_60))),
        1015 => (0.7, [(1, a), (2, a)], b),
        1089 => (0.8, [(1, a), (2, b)], narrow(whole(record.value_60) * 0.01)),
        _ => return None,
    };
    let effect = |(id, value)| (keyed(id, value, u32::from(record.id)), seconds);
    Some(Boon {
        point,
        effects: effects.into_iter().map(effect).collect(),
        clears: Vec::new(),
        starts: Vec::new(),
        replaces: Vec::new(),
    })
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

/// How far a creature that a [`Style::Whirl`] of `record` at the level in play `level` knocks
/// back slides (`0x5038F0` at `0x50522A`): the event of kind 3 with the whole value
/// `[+0x58] + [+0x5C]·L`, 170 + 5·L for 1020, whatever the target's level.
pub fn whirl_knockback(record: &WeaponArt, level: u32) -> u32 {
    let [base, step] = record.value_58.map(f64::from);
    u32::try_from(whole_units(f64::from(level).mul_add(step, base))).unwrap_or(0)
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

/// The arts that the order makes where their creature stands, whatever their record's distance,
/// once a straight way leads to the target (`0x4FD0F0`, `0x4FCC80`): the jumps 1003 and 1017,
/// and 1032.
const MADE_WHERE_IT_STANDS: [u16; 3] = [1003, 1017, 1032];

/// The distance the order makes the art `art` of the record's distance `distance` from
/// ([`made_from`]): 0 for [`MADE_WHERE_IT_STANDS`], else the record's.
pub fn order_distance(art: u16, distance: i32) -> i32 {
    if MADE_WHERE_IT_STANDS.contains(&art) {
        0
    } else {
        distance
    }
}

/// What is left of its clip, as a part of it, below which a [`Style::Jump`] of a hero of the
/// kind `kind` flies to its target, and below which it lands: the Seraphim's 0.75 and 0.4
/// (`0x5038F0` at `0x504570`), the Gladiator's 0.8 and 0.6 (`0x4FF0B0` at `0x50057C`).
pub fn jump_points(kind: u32) -> [f32; 2] {
    if kind == 2 { [0.8, 0.6] } else { [0.75, 0.4] }
}

/// The radius about the Gladiator and the factor of the strikes of his jump of `record` as he
/// lands, at the level in play `level` (`0x4FF0B0` at `0x500824`): `L·[+0x54] + [+0x50]`,
/// 190 + 10·L for 1003, a creature of the hero's list nearer him than that struck with
/// `L·[+0x5C] + [+0x58]`, 1.8 + 0.2·L.
pub fn jump_quake(record: &WeaponArt, level: u32) -> (f32, f32) {
    let radius = {
        let [base, step] = record.factor.map(f64::from);
        narrow(f64::from(level).mul_add(step, base))
    };
    let factor = {
        let [base, step] = record.value_58.map(f64::from);
        narrow(f64::from(level).mul_add(step, base))
    };
    (radius, factor)
}

/// How long the Gladiator's jump stuns a creature of the level `target` that its roll takes
/// (`0x4FF0B0` at `0x50097D`): its level and 3 seconds, at least 2.
pub fn jump_stun(target: u32) -> f32 {
    (f32::from(u16::try_from(target).unwrap_or(u16::MAX)) + 3.0).max(2.0)
}
/// The part of its clip's time that a jump's flight takes (`0x44DED0`'s time, the clip's times
/// the float 0.25 at `0x8907C4`).
pub const JUMP_FLIGHT: f32 = 0.25;

/// The rate of a jump's clip, its maker `distance` from its target (`0x5038F0` at `0x505C4C`):
/// 5 within 10, 1 beyond 500, else `5 − ⌊d/99⌋`, at least 1, where the original's falls to 0
/// from 495 to 500 and holds its clip (docs/decisions/2026-10-24-jump-rate.md).
pub fn jump_rate(distance: u32) -> f32 {
    let rate = match distance {
        0..10 => 5,
        10..=500 => 5_u32.saturating_sub(distance / 99).max(1),
        _ => 1,
    };
    f32::from(u8::try_from(rate).unwrap_or(1))
}

/// Where a jump from `from` to its target at `to` lands (`0x5038F0` at `0x504570`): the target's
/// place brought nearer by half the reach of the two (`reach`, `0x4DBC60`, halved as a whole
/// number), along the line from the jumper; its point itself without a target (`None`).
pub fn jump_landing(from: [f32; 2], to: [f32; 2], reach: Option<u32>) -> [f32; 2] {
    let Some(reach) = reach else {
        return to;
    };
    let [dx, dy] = [to[0] - from[0], to[1] - from[1]];
    let distance = dx.hypot(dy);
    if distance <= f32::EPSILON {
        return to;
    }
    let half = f32::from(u16::try_from(reach / 2).unwrap_or(u16::MAX));
    [to[0] - half * dx / distance, to[1] - half * dy / distance]
}

/// What a form of the Daemon's `record` gives at the level in play `level` ([`Style::Form`],
/// `0x5192E0`), as a [`Boon`]: below 0.3 left of its clip, on its target of a hero's class, else on
/// him, the effects keyed by 1094, 1097 to 1100 and [`DAEMON_FORM`] taken off (`0x5888F0`), then
/// for `[+0x50] + [+0x54]·L` seconds (44 + L) at most 180:
///
/// - 1097: the effects 3 (the physical damage's multiplier) of `([+0x60] + [+0x64]·L) / 100`
///   and 1 (`A`'s) of `([+0x68] + [+0x6C]·L) / 100`, 0.15 + 0.05·L each, and 7 (the first
///   resistance) of `[+0x58] + [+0x5C]·L` (10 + 20·L);
/// - 1098, 1099 and 1100: the effect 53 (`fight::effects::TURNS`) of the whole value 1, the value
///   `[+0x6C] / 1000` (0.871) and the level `0x100`, `0x300` or `0x200`, turning 0.129 of the
///   physical damage into fire, poison or magic, and the effect 8, 10 or 9 (that kind's
///   resistance) of 10 + 20·L;
///
/// each keyed by the art, with the effect 60 keyed by [`DAEMON_FORM`] (its whole value, an id, not
/// kept).
pub fn form(record: &WeaponArt, level: u32) -> Option<Boon> {
    let l = f64::from(level);
    let by_level = |[base, step]: [f32; 2]| narrow(l.mul_add(f64::from(step), f64::from(base)));
    let whole = |[base, step]: [i32; 2]| l.mul_add(f64::from(step), f64::from(base));
    let art = u32::from(record.id);
    let keyed = |id, whole, value, art| Effect {
        id,
        whole,
        value,
        art,
        level: 0,
    };
    let seconds = by_level(record.factor).min(180.0);
    let resistance = by_level(record.value_58);
    let aura = narrow(f64::from(record.value_68[1]) / 1000.0);
    // The effect 53 of the whole value 1 and the kinds from the physical to the form's.
    let turns = |kinds| Effect {
        level: kinds,
        ..keyed(53, 1, aura, art)
    };
    let mut effects = match record.id {
        1097 => vec![
            keyed(3, 0, narrow(whole(record.value_60) * 0.01), art),
            keyed(1, 0, narrow(whole(record.value_68) * 0.01), art),
            keyed(7, 0, resistance, art),
        ],
        1098 => vec![turns(0x100), keyed(8, 0, resistance, art)],
        1099 => vec![turns(0x300), keyed(10, 0, resistance, art)],
        1100 => vec![turns(0x200), keyed(9, 0, resistance, art)],
        _ => return None,
    };
    effects.push(keyed(60, 0, 0.0, DAEMON_FORM));
    Some(Boon {
        point: 0.7,
        effects: effects
            .into_iter()
            .map(|effect| (effect, seconds))
            .collect(),
        clears: Vec::new(),
        starts: Vec::new(),
        replaces: vec![1094, 1097, 1098, 1099, 1100, DAEMON_FORM],
    })
}

/// What the Wood Elf's multi-shot `record` gives her at the level in play `level` as it starts
/// ([`Style::Volley`], `0x50AC00` at `0x50C585`): the effect `0x15` (`arrows::MULTI_SHOT`) of
/// the whole value `⌊(L·[+0x54] + [+0x50]) / (L·[+0x5C] + [+0x58])⌋` arrows,
/// `⌊(50 + 144·L) / (77 + 20·L)⌋`, for `L·[+0x64] + [+0x60]` seconds (20), keyed by 1005.
pub fn volley(record: &WeaponArt, level: u32) -> (Effect, f32) {
    let l = f64::from(level);
    let by_level = |[base, step]: [f32; 2]| l.mul_add(f64::from(step), f64::from(base));
    let whole = by_level(record.factor) / by_level(record.value_58);
    let arrows = i32::try_from(whole_units(whole)).unwrap_or(1);
    let seconds = l.mul_add(f64::from(record.value_60[1]), f64::from(record.value_60[0]));
    let effect = Effect {
        id: crate::arrows::MULTI_SHOT,
        whole: arrows,
        value: 0.0,
        art: u32::from(record.id),
        level: 0,
    };
    (effect, narrow(seconds))
}

/// The Vampiress's types by day and by night (docs/02 §6.2), and the effect that her night's form
/// lasts for (`0x50E6C0`, the adder `0x582D50` with no art's key).
pub const VAMPIRESS_BY_DAY: u32 = 6;
pub const VAMPIRESS_BY_NIGHT: u32 = 7;
pub const NIGHT_FORM: u32 = 22;

/// What is left of its clip, as a part of it, below which a [`Style::Changeling`] changes its
/// maker's form, and the rate of its clip (seen: the slot 144 at 0.7 in either form).
pub const CHANGES: f32 = 0.5;
pub const CHANGELING_RATE: f32 = 0.7;

/// How long the night's form of the changeling `record` at the level in play `level` lasts
/// (`0x50E6C0`, `0x51CC20`): `[+0x50] + [+0x54]·L` seconds, 24 + 6·L, doubled but by day (`day`,
/// `0x41BA60`, from 05:20 to 20:20).
pub fn changeling_seconds(record: &WeaponArt, level: u32, day: bool) -> f32 {
    let [base, step] = record.factor.map(f64::from);
    let seconds = narrow(f64::from(level).mul_add(step, base));
    if day { seconds } else { 2.0 * seconds }
}

/// What is left of its clip, as a part of it, below which a [`Style::Dash`] puts its maker at its
/// point, and the rate of its clip (seen: the slot 144 at 5, and the Vampiress at her point 0.36 s
/// after the click).
pub const DASHES: f32 = 0.8;
pub const DASH_RATE: f32 = 5.0;

/// How far the Vampiress's dash `record` goes at most at the level in play `level` (`0x50E6C0`):
/// `[+0x60] + [+0x64]·L`, 400 + 25·L by day (1028), 500 + 50·L by night (1029); seen: a click 12
/// cells off put her 8 cells off at level 1.
pub fn dash_reach(record: &WeaponArt, level: u32) -> u32 {
    let [base, step] = record.value_60.map(i64::from);
    u32::try_from(base + step * i64::from(level)).unwrap_or(0)
}

/// Whether a Vampiress of type `kind` may make `art` in her form (`0x51D840`): a day art of her
/// pairs by day alone, a night art by night alone; any other art in either.
pub fn in_form(art: u16, kind: u32) -> bool {
    if VAMPIRESS.iter().any(|&(day, _)| day == art) {
        return kind != VAMPIRESS_BY_NIGHT;
    }
    if VAMPIRESS.iter().any(|&(_, night)| night == art) {
        return kind != VAMPIRESS_BY_DAY;
    }
    true
}

/// What is left of its clip, as a part of it, below which a [`Style::Panther`] leaps, and the
/// part of its clip's time that its flight takes (`0x506770` at `0x507DAA`: the double 0.7 at
/// `0x891068`, the float 0.5 at `0x890670`).
pub const PANTHER_LEAPS: f32 = 0.7;
pub const PANTHER_FLIGHT: f32 = 0.5;

/// How far the panther `record` leaps at most at the level in play `level` (`0x509F0F`):
/// `L·[+0x54] + [+0x50]` truncated, 400 + 50·L.
pub fn panther_reach(record: &WeaponArt, level: u32) -> u32 {
    let [base, step] = record.factor.map(f64::from);
    u32::try_from(whole_units(f64::from(level).mul_add(step, base))).unwrap_or(0)
}

/// The rate of the panther's clip by its reach `reach` (`0x509FF5`): 5 within 10, 1 beyond 500,
/// else `6 − 0.0101·reach` (the floats at `0x890954` and `0x891090`).
pub fn panther_rate(reach: u32) -> f32 {
    match reach {
        0..10 => 5.0,
        10..=500 => {
            let reach = f64::from(reach) * f64::from(f32::from_bits(0x3C25_7EB5));
            narrow(6.0 - reach)
        }
        _ => 1.0,
    }
}

/// Where the panther from `from`, aimed at the point `to` on the ground, leaps (`0x509E7B`): `to`
/// itself within its reach `reach`, else the point at that reach along the line, and at that reach
/// too where the middle button is held (`far`), moved by `(to − from) · reach / d` in whole units,
/// `d` the whole distance, 1 at least.
pub fn panther_point(from: [f32; 2], to: [f32; 2], reach: u32, far: bool) -> [f32; 2] {
    let delta = [0, 1].map(|i| f64::from(to[i] - from[i]).trunc());
    let d = delta[0].hypot(delta[1]).trunc().max(1.0);
    let reach = f64::from(reach);
    if reach >= d && !far {
        return to;
    }
    [0, 1].map(|i| narrow(f64::from(from[i]) + (delta[i] * reach / d).trunc()))
}

/// What is left of its clip, as a part of it, below which a [`Style::Ward`] gives its effect
/// (`0x506770` at `0x508561`: the double 0.2 at `0x890DB0`).
pub const WARD_LEFT: f32 = 0.2;

/// What the arrowproof `record` gives at the level in play `level` ([`Style::Ward`], `0x506770`
/// at `0x508561`), as a [`Boon`]: below [`WARD_LEFT`] of its clip, on its target where it is of a
/// hero's class (`0x426610`), else on its maker, the effect `0x19`, the arrows' deflection
/// (`arrows::deflection_chance`), of `[+0x58] + [+0x5C]·L` (1 + 0.05·L) for
/// `[+0x50] + [+0x54]·L` seconds (44 + L), keyed by 1045 and with its level; its whole value, the
/// object 883 it makes, is left out.
pub fn ward(record: &WeaponArt, level: u32) -> Option<Boon> {
    if record.id != 1045 {
        return None;
    }
    let l = f64::from(level);
    let by_level = |[base, step]: [f32; 2]| narrow(l.mul_add(f64::from(step), f64::from(base)));
    let effect = Effect {
        id: crate::arrows::DEFLECTION,
        whole: 0,
        value: by_level(record.value_58),
        art: u32::from(record.id),
        level: u16::try_from(level).unwrap_or(u16::MAX),
    };
    Some(Boon {
        point: 1.0 - WARD_LEFT,
        effects: vec![(effect, by_level(record.factor))],
        clears: Vec::new(),
        starts: Vec::new(),
        replaces: Vec::new(),
    })
}

/// What is left of its clip, as a part of it, below which a [`Style::Viper`] stuns
/// (`0x506770` at `0x508C3B`: the double 0.2 at `0x890DB0`).
pub const VIPER_LEFT: f32 = 0.2;

/// The radius about its target and the seconds of a [`Style::Viper`] of `record` at the level in
/// play `level` (`0x506770` at `0x508E5B`): `[+0x50] + [+0x54]·L`, 148 + 2·L for 1042, a creature
/// of the hero's list nearer the target than that stunned too; each stunned for
/// `[+0x58] + [+0x5C]·L` seconds, 19.5 + 0.5·L, its timer set back so far and the effect 35 for
/// that time.
pub fn viper(record: &WeaponArt, level: u32) -> (f32, f32) {
    let radius = {
        let [base, step] = record.factor.map(f64::from);
        narrow(f64::from(level).mul_add(step, base))
    };
    let seconds = {
        let [base, step] = record.value_58.map(f64::from);
        narrow(f64::from(level).mul_add(step, base))
    };
    (radius, seconds)
}

/// The radius of a [`Style::Roar`] of `record` at the level in play `level` (`0x4FF0B0` at
/// `0x4FFC15`): `L·[+0x54] + [+0x50]` truncated, 150 + 10·L for 1014; a creature nearer its maker
/// than that is roared at.
pub fn roar_radius(record: &WeaponArt, level: u32) -> u32 {
    let [base, step] = record.factor.map(f64::from);
    u32::try_from(whole_units(f64::from(level).mul_add(step, base))).unwrap_or(0)
}

/// How far a [`Style::Roar`] of `record` at the level in play `level` knocks a creature back
/// (`0x4FF0B0` at `0x4FFCA2`): `[+0x60] + [+0x68] − [+0x6C] + ([+0x64] + 1)·L`, 52 + 4·L for
/// 1014, at least 10, taken by the creature's `+0x24D` and sent with the event of kind 3.
pub fn roar_knockback(record: &WeaponArt, level: u32) -> u32 {
    let level = i64::from(level);
    let [base, step] = record.value_60.map(i64::from);
    let [plus, minus] = record.value_68.map(i64::from);
    let distance = base + plus - minus + (step + 1) * level;
    u32::try_from(distance.max(10)).unwrap_or(10)
}

/// How long a creature that a [`Style::Roar`] of `record` at the level in play `level` knocks
/// back is held at least, in seconds (`0x4FF0B0` at `0x4FFDCA`): `[+0x58] + [+0x5C]·L`,
/// 3 + 0.35·L for 1014, its timer set back to fire no sooner.
pub fn roar_hold(record: &WeaponArt, level: u32) -> f32 {
    let [base, step] = record.value_58.map(f64::from);
    narrow(f64::from(level).mul_add(step, base))
}

/// How high a jump of the length `distance` flies at the part `t` of its flight, in units of the
/// models' height (`0x44DED0`, `0x44DFE0`): `distance / 12` at its middle, on a parabola from the
/// ground to the ground.
pub fn jump_height(distance: f32, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    distance / 12.0 * (1.0 - 4.0 * (t - 0.5) * (t - 0.5))
}

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
