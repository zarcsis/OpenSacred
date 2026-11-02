//! A creature's combat arts (docs/re/arts.md): the list its stats hold, an art's level in play,
//! each entry's regeneration time by its record and its skills, the runes that teach the arts,
//! the hero's arts at a new game, and their regeneration: a use that starts it for every art of
//! its kind and the frames that run it down; and how the weapon arts `Attacke`, `Rundum` and
//! `Hardhit` strike, their factors and their targets (#508), whose chance and damage
//! `fight::art_strike` resolves. `opensacred-game` keeps the hero's list; the combos are not here.

use opensacred_data::heroes::HeroArt;
use opensacred_data::tables::{Arts as Tables, Balance, Spell, WeaponArt};

use crate::fight::{Effect, curve};
use crate::spawning::Dice;

/// The bit of an entry's flags that a rune sets: on every art it adds (`0x580750`) and on every
/// art it raises for good (`0x581F80`); the spells page's hide clears it on every art
/// (`0x5809B0`), a mark of the arts new to the page.
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

/// The generic art of a class's own `Rundum`, `Hardhit`, `Attacke` and `Kick` (`0x52DA80`), by
/// which an item's bonus finds the hero's own: 1000, 1001, 1002 or 1009; any other art itself.
pub fn generic(art: u16) -> u16 {
    match art {
        1057 | 1061 | 1065 | 1069 | 1073 | 1078 | 1090 | 1038 => 1000,
        1058 | 1062 | 1066 | 1070 | 1074 | 1079 | 1091 | 1094 | 1039 => 1001,
        1059 | 1063 | 1067 | 1071 | 1075 | 1080 | 1092 | 1097 | 1040 => 1002,
        1060 | 1064 | 1068 | 1072 | 1076 | 1093 | 1041 => 1009,
        other => other,
    }
}

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
    /// The Gladiator's 1013 (`GUI_MOVE_SCHLEUDERKLINGEN`, `0x4FF0B0` at `0x501CF5`): below
    /// [`THROW_LEFT`] of its clip the item in his hand leaves it and flies on an arc to beyond
    /// his target ([`throw_end`], [`throw_height`], [`throw_time`], [`throw_point`]), strikes the
    /// first creature it touches with its own damages and the art's factor, and falls there, or
    /// at its end, an item to pick up again; the art ends at the throw.
    Throw,
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
    /// As [`Self::Ward`], its effect the spells' deflection (`0x1A`) at its own clip: the Dark
    /// Elf's spellproof 1046 (`GUI_MOVE_SPELLPROOF`, `0x506770`), of the launch type 2.
    WardSpells,
    /// The Dark Elf's traps 1049 (`GUI_MOVE_BOMB`), 1050 (`GUI_MOVE_HELL`), 1051
    /// (`GUI_MOVE_CONFUSION`), 1052 (`GUI_MOVE_VENOM`) and 1053 (`GUI_MOVE_FOG`), of the launch type
    /// 0 (`0x506770`): once what is left of its clip is below [`TRAPS`] he throws a ball to the
    /// order's point ([`ball_rise`]), which makes its trap where it lands, of its spell's record
    /// ([`trap_spell`]).
    Trap,
    /// Its effects on its maker at a part of its clip, those of his other forms taken off first
    /// ([`form`]): the Daemon's forms 1097 to 1100 (`0x5192E0`), of the launch type 2, made on
    /// himself.
    Form,
    /// At once, without a clip: the effect of the multi-shot on its maker ([`volley`]), the Wood
    /// Elf's 1005 (`GUI_MOVE_MULTIPFEIL`, `0x50AC00`), of the launch type 0.
    Volley,
    /// With a bow or a crossbow, a shot at its target of the art's own arrows
    /// (`arrows::projectile`), its regeneration started; without one, nothing: the Wood Elf's
    /// dumdum 1004 (`GUI_MOVE_DUMDUM`), its strike of the art's factor ([`factor`]), club arrow
    /// 1007 (`GUI_MOVE_KEULEN`), which may knock its target back by [`club_knockback`]
    /// (`arrows::club_knocks`), and blades 1008 (`GUI_MOVE_KLINGEN`), which home on their target
    /// for a time by its level and strike what they pass through ([`blades`],
    /// `arrows::Arrow::home`; `0x50AC00`, `0x5BD2D0`, `0x5BB010`).
    Arrow,
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
    /// The Gladiator's backbreaker 1010 (`GUI_MOVE_RUECKENBRECHER`, `0x4FF0B0`): its target taken
    /// up and held ([`backbreaker_hold`]), let go at its point ([`backbreaker_lets_go`]) with a
    /// strike of [`backbreaker_factor`], and held on by a roll ([`backbreaker_holds`]).
    Backbreaker,
    /// The Vampiress's blood bite 1030 (`GUI_MOVE_BLUTBISS`, `0x50E6C0`), by night: once what is
    /// left of its clip is below [`BITES`] its target is held for [`BITE_HOLD`] with the effect
    /// 15, struck with a fixed physical damage of [`bite_drain`], and she takes as much health.
    Bite,
    /// The Vampiress's domination, 1024 (`GUI_MOVE_BEHERRSCHUNG_DAY`) and 1025
    /// (`GUI_MOVE_BEHERRSCHUNG`), `0x50E6C0`: once what is left of its clip is below
    /// [`DOMINATES`] its target, no boss nor champion, takes the effect [`DOMINATED`] for
    /// [`domination_seconds`], and does not fight her while it holds (seen).
    Dominate,
    /// The Vampiress's call of the wolf, 1026 (`GUI_MOVE_WOLFSRUF_DAY`) and 1027
    /// (`GUI_MOVE_WOLFSRUF`), of the launch type 3 (`0x50E6C0`): once what is left of its clip is
    /// below [`SUMMONS`] a summoning near its point ([`summoning_point`]) that makes her a wolf
    /// of [`wolf_level`] for her follower.
    Summon,
    /// The Vampiress's awakening 1034 (`GUI_MOVE_ERWECKUNG`), by night, of the launch type 4 (a
    /// dead creature, `0x50E6C0`): once what is left of its clip is below [`RAISES`] the dead
    /// creature rises as her follower for [`raised_seconds`], at [`raised_level`].
    Raise,
    /// The Vampiress's master's bite 1031 (`GUI_MOVE_MEISTERBISS`, `0x50E6C0`), by night: once
    /// what is left of its clip is below [`MASTER_BITES`] its target, if [`master_bites`] takes
    /// it, takes the effect 14, which turns its sides over, for [`master_bite_seconds`] (seen).
    MasterBite,
    /// The Vampiress's bats, 1035 (`GUI_MOVE_BAT_ATTACK`), 1036 (`GUI_MOVE_BAT_CONFUSE`) and 1037
    /// (`GUI_MOVE_BAT_DEFEND`), by night, of the launch type 0 (`0x50E6C0`): once what is left of
    /// its clip is below [`BATS`] a bat of its type ([`bat`]) beside her as her follower, sent at
    /// its target, her bats of that type beyond [`bats_most`] dismissed.
    Bats,
    /// The Vampiress's death's kiss 1033 (`GUI_MOVE_TODESKUSS`, `0x50E6C0`), by night: once what is
    /// left of its clip is below [`KISSES`] a skull leaves her neck and homes on its target
    /// ([`SKULL_SPEED`], [`skull_life`]); each hostile creature it touches is cursed, its attack's
    /// and defence's multipliers lowered by [`curse`] and bleeding ([`bleed_value`], [`bleed`]),
    /// for [`CURSE_SECONDS`] (the effects 1, 2 and 23; seen).
    Kiss,
    /// One strike on its target and the same on every hostile creature nearer that target than
    /// the reach of it and the maker, at a part of its clip ([`SMITE_LEFT`], [`smitten`]): the
    /// Gladiator's 1012 (`GUI_MOVE_HANDGOTTES`, `0x4FF0B0`).
    Smite,
    /// Effects on its maker at a part of its clip, made where it stands ([`boon`]): the Dark
    /// Elf's 1047 and 1048, the Dwarf's 1089 and the Seraphim's 1018, of the launch type 5
    /// (`ECF_ONMEMYSELFANDI`, whose target `0x51D840` makes the maker itself), and the
    /// Gladiator's 1015, of the type 2 (`ECF_ONFRIENDLY`), on a friend or on himself, whom the
    /// right button on him makes its target (seen).
    Boon,
    /// As a boon, at its own clip: the Dwarf's 1082 (`gui_moves_abschlag`, `0x514790`), the effect
    /// [`FRENZY`] on himself ([`boon`]), with which his strikes quicken his attack
    /// ([`frenzy_rises`], [`frenzy_falls`], [`quickened`]).
    Frenzy,
    /// As a boon, at its own clip: the Seraphim's 1019 (`GUI_SPELL_ENERGIEBLITZE`, `0x5038F0`),
    /// the effect [`RAILGUN`] on herself ([`boon`]), with which her shots are bolts ([`Bolt`]).
    Bolts,
    /// As a boon, at its own clip: the Gladiator's 1016 (`GUI_MOVE_KLINGENBLICK`, `0x4FF0B0`),
    /// the effect [`GLANCE`] on a friend or himself ([`glance`]), with which he strikes a
    /// hostile creature about him about once a second ([`glance_due`]).
    Glance,
    /// The Daemon's charge 1091 (`gui_moves_hardhit_demon`, `0x5192E0` at `0x519BD0`): at its
    /// run after its first he flies to his target ([`charge_flight`], [`jump_landing`]), and once
    /// landed he strikes it within the reach, makes his blast about him ([`Blast`]) and ends.
    Charge,
    /// The Daemon's 1101 (`gui_moves_giftring`, `0x5192E0` at `0x51A9B7`): below 0.3 of its clip
    /// his ring of poison burns every hostile creature within [`RING_REACH`] of him ([`Burn`],
    /// [`ring_poison`], [`daemon_ratio`]).
    Ring,
    /// The Daemon's 1103 (`0x5192E0` at `0x51AD88`): below 0.3 of its clip a fan of bolts toward
    /// his target ([`fan`], [`fan_magic`], [`EMPEROR_COURSE`]).
    Fan,
    /// The Dwarf's flamethrower 1085 (`gui_spells_flammenwerfer`, `0x514790` at `0x516D45`):
    /// while what is left of its clip lies within [`FLAMES`], every hostile creature along the
    /// [`FLAMES_LENGTH`] before him burns ([`Burn`], [`gun_damage`]).
    Flamer,
    /// His mortar 1086 (`gui_spells_moerser`): at its clip's first hit point a shell to his
    /// target's place at least [`MORTAR_LEAST`] off, which strikes about it [`MORTAR_DELAY`]
    /// later ([`mortar_point`]).
    Mortar,
    /// His cannon 1087 (`gui_spells_kanone`): a shot along the line to his target once what is
    /// left of its clip falls below [`cannon_left`], striking every hostile creature on it.
    Cannon,
    /// The Daemon's 1102 (`gui_moves_feuerhauch`, `0x5192E0` at `0x51AD2B`): below 0.3 of its
    /// clip his breath of fire leaves him toward his point, burning what it meets, and lays a
    /// wall of fire where it stops ([`breath_speed`], [`wall_spreads`]).
    Breath,
    /// The Dwarf's 1084 (`gui_moves_kriegsrausch`, `0x514790` at `0x517BB5`), with a two-handed
    /// weapon alone: once, at its clip's first hit point, its target and every hostile creature
    /// about it ([`sweep_reaches`]), those at a part of its factor ([`sweep_factor`]).
    Sweep,
    /// The Dwarf's 1083 (`gui_moves_wucht`, `0x514790` at `0x517EE2`), with a two-handed weapon
    /// alone: once, at its clip's first hit point, its target thrown along the line from him
    /// ([`hurl_speed`], [`Flight`]), held ([`hurl_hold`]) and struck ([`hurl_factor`]); as it
    /// flies it knocks over the creatures it meets ([`bowled`]).
    Hurl,
    /// A spell of the first table cast with the clip of its slot (`0x51D150`, `0x522030`;
    /// [`crate::spells`]): it acts once the share of its clip still to play falls below its own
    /// ([`crate::spells::share`]).
    Cast(u16),
}

/// The arts that give their maker effects ([`Style::Boon`]).
const BOONS: [u16; 7] = [1015, 1018, 1044, 1047, 1048, 1089, 1094];

/// The Seraphim's effect of 1018 (`GUI_MOVE_BFG`, `0x10`): while it holds, her hands hold the
/// item [`BFG_ITEM`] alone, her own put away, which come back as it ends (the adder `0x582D50`
/// and the taker `0x586D80`, docs/re/arts.md, "The Seraphim's"; seen).
pub const BFG: u32 = 16;

/// The item that [`BFG`] puts in the Seraphim's right hand (`SeraBFG.GRN`, a fixed item of
/// `Weapon.pak`).
pub const BFG_ITEM: u32 = 3809;

/// The Seraphim's effect of 1019 (`GUI_SPELL_ENERGIEBLITZE`, `0x11`): while it holds, each of her
/// shots makes two bolts [`Bolt::Railgun`] (the adder `0x582D50`'s bit `0x400000`; docs/re/arts.md,
/// "Her shots with 1018's and 1019's effects").
pub const RAILGUN: u32 = 17;

/// The Gladiator's effect of 1016 (`GUI_MOVE_KLINGENBLICK`, `0x12`): while it holds on the
/// local player's creature, it strikes a creature of the hero's list of hostiles near it about
/// once a second with its whole value ([`glance_due`], [`glance_reach`]; docs/re/arts.md, "The
/// Gladiator's"). Its bit `0x800000` of `+0x200` has no reader.
pub const GLANCE: u32 = 18;

/// The milliseconds of [`GLANCE`]'s count above which its bearer strikes, at which a new effect
/// starts it (`0x562730` at `0x5630CE`; the adder `0x582D50`).
pub const GLANCE_EVERY: u32 = 1000;

/// Whether a bearer of [`GLANCE`] of the count `count` strikes at a frame of `seconds`, the hero's
/// list of hostiles holding any, `hostiles` (`0x562730` at `0x563048`): above [`GLANCE_EVERY`]
/// with hostiles the count starts again from 0 and it strikes; then the count takes the frame's
/// milliseconds, taken whole (`0x5633ED`).
pub fn glance_due(count: &mut u32, seconds: f32, hostiles: bool) -> bool {
    let due = *count > GLANCE_EVERY && hostiles;
    if due {
        *count = 0;
    }
    let milliseconds = u32::try_from(whole_units(f64::from(seconds) * 1000.0)).unwrap_or(0);
    *count = count.saturating_add(milliseconds.min(u32::from(u16::MAX)));
    due
}

/// How near its bearer of level `level` a creature is struck by [`GLANCE`]: `(l + 20)·10`
/// (`0x5370B0`).
pub fn glance_reach(level: u32) -> f32 {
    narrow((f64::from(level) + 20.0) * 10.0)
}

/// What the Gladiator's 1016 gives (`0x4FF0B0` at `0x500186`, [`Style::Glance`]): below 0.3 of
/// its clip the effect [`GLANCE`] of the whole value `L·[+0x54] + [+0x50]` (3 + 7·L) times
/// `factor` ([`crate::fight::Rules::glance_factor`]), taken whole, for `L·[+0x5C] + [+0x58]`
/// seconds (39.5 + 0.5·L), keyed by it (seen: 12 and 39 at the levels 1 and 4 of a Gladiator of
/// `a0` 33); the effect 34 of its time is left out, as 1015's. None for another art.
pub fn glance(record: &WeaponArt, level: u32, factor: f64) -> Option<Boon> {
    if record.id != 1016 {
        return None;
    }
    let l = f64::from(level);
    let by_level = |[base, step]: [f32; 2]| narrow(l.mul_add(f64::from(step), f64::from(base)));
    let whole = whole_units(f64::from(by_level(record.factor)) * factor);
    let effect = Effect {
        id: GLANCE,
        whole: i32::try_from(whole).unwrap_or(i32::MAX),
        value: 0.0,
        art: u32::from(record.id),
        level: 0,
    };
    Some(Boon {
        point: 0.7,
        effects: vec![(effect, by_level(record.value_58))],
        clears: Vec::new(),
        starts: Vec::new(),
        replaces: Vec::new(),
        ends: false,
    })
}

/// The Dark Elf's effect of 1044 (`GUI_MOVE_SOULCATCHER`, `0x23`), which his object 869 gives
/// him for its life: while it holds, each creature that dies within [`SOUL_REACH`] of him sends
/// him a soul that heals him ([`soul_speed`], [`soul_heals`]; docs/re/arts.md, "The Dark Elf's").
pub const SOULCATCHER: u32 = 35;

/// How near the Dark Elf a creature dies whose soul his 1044 catches: the sum of the sizes of the
/// objects 869 and 780, 500 and 50 (`0x5FF7C0`).
pub const SOUL_REACH: f32 = 550.0;

/// How much nearer than his size a soul reaches the Dark Elf (`0x5ABD80`'s case `0x30C`): its
/// own size, 50.
pub const SOUL_SIZE: f32 = 50.0;

/// The speed of a soul of the age `age` in seconds toward the Dark Elf, in units a second
/// (`0x5A7BB2`, `0x76C9D0`): `⌊10·t⌋`.
pub fn soul_speed(age: f32) -> f32 {
    (10.0 * age).floor()
}

/// What the Dark Elf's 1044 gives him (`0x506770` at `0x507E82`, the object 869's start
/// `0x76AA40`): at once the effect [`SOULCATCHER`] for the first table's record 47's value 0 at
/// the level `level`, `39 + L` at most 90 taken whole (`spell`), keyed by 1044; and it ends.
pub fn soulcatcher(spell: &Spell, level: u32) -> Boon {
    let [base, step] = spell.values[0].map(u32::from);
    let mut seconds = base + step * level;
    if spell.caps[0] > 0 {
        seconds = seconds.min(u32::from(spell.caps[0]));
    }
    let effect = Effect {
        id: SOULCATCHER,
        whole: 0,
        value: 0.0,
        art: 1044,
        level: 0,
    };
    Boon {
        point: 0.0,
        effects: vec![(effect, narrow(f64::from(seconds)))],
        clears: Vec::new(),
        starts: Vec::new(),
        replaces: Vec::new(),
        ends: true,
    }
}

/// The health a soul of the Dark Elf's 1044 gives him (`0x5ABD80`'s case `0x30C`, the event 6):
/// the record 47's value 1 at his art's level in play `level`, `10 + 30·L` (`spell`).
pub fn soul_heals(spell: &Spell, level: u32) -> u32 {
    let [base, step] = spell.values[1].map(u32::from);
    base + step * level
}

/// How far her target lies beyond which a creature with the effect [`BFG`] shoots (`0x53A170` at
/// `0x53AACE`).
pub const BFG_BEYOND: f32 = 200.0;

/// How long after a shot's first [`Bolt::Railgun`] its second leaves, in seconds (seen: two
/// objects 801 0.1 s apart; **hypothesis**: the same for every clip).
pub const RAILGUN_SECOND: f32 = 0.1;

/// A bolt of the Seraphim's shots, the objects 801 (`FX_RAILGUN`) and 875 (`FX_BFG`)
/// (docs/re/arts.md, "The Seraphim's bolts"): it leaves where she stands along the line to her
/// target, flies straight at its speed for its time at most (`0x5A6680`), ends at a cell that may
/// not be walked on, and hits each hostile creature alive nearer it than the sum of their sizes
/// (`0x5AB490`) with its damages ([`bolt_damages`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bolt {
    /// The effect [`RAILGUN`]'s, two a shot: it hits every creature it finds in a frame, once,
    /// and goes (`0x5ABD80` at `0x5AC24D`).
    Railgun,
    /// The effect [`BFG`]'s, one a shot: it hits the creatures of its first contact, once, and
    /// flies on harmless (`0x5ABD80` at `0x5AC485`).
    Bfg,
}

impl Bolt {
    /// The bolt of a shot of a creature with the effect [`BFG`] (`bfg`) or [`RAILGUN`]
    /// (`railgun`), its target at `distance`, or none: the BFG's beyond [`BFG_BEYOND`] or without
    /// a target, the railgun's always; none, a strike, otherwise (the bits `0x200000` and
    /// `0x400000` of `+0x200`, `0x53A170` at `0x53AACE`).
    pub fn of(bfg: bool, railgun: bool, distance: Option<f32>) -> Option<Self> {
        if bfg && distance.is_none_or(|distance| distance > BFG_BEYOND) {
            Some(Self::Bfg)
        } else if railgun {
            Some(Self::Railgun)
        } else {
            None
        }
    }

    /// The effect whose whole value is its level.
    pub fn effect(self) -> u32 {
        match self {
            Self::Railgun => RAILGUN,
            Self::Bfg => BFG,
        }
    }

    /// Its record of the first table, whose damages it deals (`0x4D2310`).
    pub fn spell(self) -> u16 {
        match self {
            Self::Railgun => 51,
            Self::Bfg => 8,
        }
    }

    /// How many a shot makes (`0x4DCB80`).
    pub fn count(self) -> u32 {
        match self {
            Self::Railgun => 2,
            Self::Bfg => 1,
        }
    }

    /// Its speed in world units a second and its most time in seconds, its particle system's
    /// (`0x5A0C50`).
    pub fn flight(self) -> (f32, f32) {
        match self {
            Self::Railgun => (400.0, 2.0),
            Self::Bfg => (200.0, 3.0),
        }
    }

    /// The rate its shot plays at (`0x4DCB80`): the railgun's slot 164 at 1.0, the BFG's slot 145
    /// at 1.5 times its own.
    pub fn rate(self) -> f32 {
        match self {
            Self::Railgun => 1.0,
            Self::Bfg => 1.5,
        }
    }

    /// Whether it goes as it hits (the railgun's), rather than flying on harmless.
    pub fn goes(self) -> bool {
        self == Self::Railgun
    }
}

/// The damages that a bolt of the first table's record `spell` deals, its shooter's effect of the
/// whole value `level` and its shooter's damage multipliers `multipliers`
/// (`fight::Combat::multipliers`, `0x4D2310`): each kind's `base + step·L` times its multiplier,
/// truncated and held at 65,534; the shooter's spell factor, 1 for a hero (`0x56A800`), is left
/// out.
pub fn bolt_damages(spell: &Spell, level: i32, multipliers: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|kind| {
        let [base, step] = spell.effects[kind].map(f64::from);
        let value = f64::from(level).mul_add(step, base) * f64::from(multipliers[kind]);
        narrow(value.trunc().clamp(0.0, 65_534.0))
    })
}

/// The key of the Daemon's forms' effects of attack speed and defence (`0x4321`), beside the
/// arts' own keys.
pub const DAEMON_FORM: u32 = 0x4321;

/// The Daemon's art of his form, 1094, and its effect, keyed by it (`0x584B16`, docs/re/arts.md,
/// "The effects", 50).
pub const FORM: u16 = 1094;
pub const FORMED: u32 = 50;
/// The effect that strikes the Daemon whose form ends where he may not stand, keyed by
/// [`DAEMON_FORM`] (`0x5881A0`): 55.
pub const DROWNS: u32 = 55;

/// Whether the Daemon in his form of 1094 may step onto a cell of the ground that the client's
/// test of a cell refuses, by its ground byte (`+0x1F`) and its first corner heights (`+0x10` to
/// `+0x13`), as `0x4EBCA0` lets a creature of an effect keyed by 1094 (at `0x4EBD62`) or a mounted
/// one: a cell of the low nibble 2, those an arrow flies over, the ponds, the fences and the walls,
/// with a corner below 2, signed (**hypothesis**: the shallow water and the low fences).
pub fn wades(ground: u8, heights: [i8; 4]) -> bool {
    ground & 0xF == 2 && heights.iter().any(|&height| height < 2)
}

/// The seconds of his form within which a number counts them down over the Daemon (`0x5635BB`).
pub const COUNTS_DOWN: f32 = 5.5;

/// The number that counts the Daemon's form down over him at a tick that took its seconds from
/// `before` to `after`, and its colour (ARGB), where he stands on a cell he may walk on or not
/// (`walkable`): at each whole second crossed below 5.5 s, the seconds rounded (`0x5635BB`, the
/// engine's event 6, as the townsfolk's lines), in [`countdown_colour`].
pub fn countdown(before: f32, after: f32, walkable: bool) -> Option<(u32, u32)> {
    let crossed = f64::from(before).trunc() > f64::from(after);
    if !crossed || after >= COUNTS_DOWN || after < 0.0 {
        return None;
    }
    let shown = u32::try_from(crate::fight::truncated(f64::from(after) + 0.5)).ok()?;
    Some((shown, countdown_colour(after, walkable)))
}

/// The countdown's colour at `left` seconds of the form (ARGB, `0x5635BB`): yellow, `0xFFF2FF00`,
/// where he may walk on his cell; else red, `0xFFFF0000`, from 1 s, and above it orange, its red
/// `0xF2` and `⌊13/s⌋`, its green `0xFF` less `⌊255/s⌋`, toward red as the seconds go.
pub fn countdown_colour(left: f32, walkable: bool) -> u32 {
    if walkable {
        return 0xFFF2_FF00;
    }
    if left <= 1.0 {
        return 0xFFFF_0000;
    }
    let left = f64::from(left);
    let red = crate::fight::truncated(-13.0 / left);
    let green = crate::fight::truncated(-255.0 / left);
    let rgb = 0xFF_F2FF + green - (red << 8);
    u32::try_from((rgb << 8) & 0xFFFF_FFFF).unwrap_or(0xFFFF_0000)
}

/// How far about the Daemon, in cells, his form's end on a cell he may not stand on looks for one
/// he may (`0x5881A0`).
pub const LANDS_WITHIN: i32 = 5;

/// The cell the Daemon is put on as his form ends on `at`, a cell he may not stand on: one at
/// random of the first ring about him, 1 to 5 cells off, that holds one he may (`walkable`,
/// `0x636C10`, `0x5FC8B0` in `0x5881A0`; **hypothesis**: the rings square, as the ways' window);
/// none where none does, and he drowns ([`drowning`]).
pub fn landing(
    (x, y): (i32, i32),
    walkable: impl Fn((i32, i32)) -> bool,
    dice: &mut impl crate::spawning::Dice,
) -> Option<(i32, i32)> {
    (1..=LANDS_WITHIN).find_map(|ring| {
        let cells: Vec<(i32, i32)> = (-ring..=ring)
            .flat_map(|dy| (-ring..=ring).map(move |dx| (dx, dy)))
            .filter(|&(dx, dy)| dx.abs().max(dy.abs()) == ring)
            .map(|(dx, dy)| (x + dx, y + dy))
            .filter(|&cell| walkable(cell))
            .collect();
        let count = u32::try_from(cells.len()).ok().filter(|&n| n > 0)?;
        cells.get(usize::try_from(dice.below(count)).ok()?).copied()
    })
}

/// How long the drowning's effect 55 holds, renewed while the Daemon lives, and how often it
/// strikes him (`0x5881A0`).
pub const DROWNING_SECONDS: f32 = 5.0;
pub const DROWNING_EVERY: f32 = 0.5;

/// The strike of the drowning's effect 55 that the Daemon's form gives him as it ends where he
/// may not stand and none can take him, by his health: a tenth of it and 1, its whole value, which
/// strikes him from himself every 0.5 s (`0x5881A0`, the hit's kind `0x11`).
pub fn drowning(health: u32) -> u32 {
    health / 10 + 1
}

/// The arts that `0x51D840` refuses while an effect of theirs holds on their maker (`0x582B00`):
/// 1031, 1042, 1044 to 1048, 1082 and 1089, and 1018 and 1019 while either's does.
pub fn refused_while(art: u16) -> &'static [u16] {
    if art < 1000 {
        return crate::spells::locked_by(art);
    }
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
    if crate::spells::made(art) {
        return Some(Style::Cast(crate::spells::slot(art)));
    }
    if BOONS.contains(&art) {
        return Some(Style::Boon);
    }
    if art == 1082 {
        return Some(Style::Frenzy);
    }
    if art == 1019 {
        return Some(Style::Bolts);
    }
    if art == 1016 {
        return Some(Style::Glance);
    }
    if art == 1091 {
        return Some(Style::Charge);
    }
    if art == 1101 {
        return Some(Style::Ring);
    }
    if art == 1085 {
        return Some(Style::Flamer);
    }
    if art == 1086 {
        return Some(Style::Mortar);
    }
    if art == 1087 {
        return Some(Style::Cannon);
    }
    if art == 1103 {
        return Some(Style::Fan);
    }
    if art == 1102 {
        return Some(Style::Breath);
    }
    if art == 1084 {
        return Some(Style::Sweep);
    }
    if art == 1083 {
        return Some(Style::Hurl);
    }
    if art == 1012 {
        return Some(Style::Smite);
    }
    if art == 1054 {
        return Some(Style::Blades);
    }
    if art == 1013 {
        return Some(Style::Throw);
    }
    if art == 1020 {
        return Some(Style::Whirl);
    }
    if art == 1017 || art == 1003 || art == 1032 {
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
    if art == 1046 {
        return Some(Style::WardSpells);
    }
    if (1049..=1053).contains(&art) {
        return Some(Style::Trap);
    }
    if (1097..=1100).contains(&art) {
        return Some(Style::Form);
    }
    if art == 1005 {
        return Some(Style::Volley);
    }
    if matches!(art, 1004 | 1006 | 1007 | 1008) {
        return Some(Style::Arrow);
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
    if art == 1010 {
        return Some(Style::Backbreaker);
    }
    if art == 1030 {
        return Some(Style::Bite);
    }
    if art == 1024 || art == 1025 {
        return Some(Style::Dominate);
    }
    if art == 1026 || art == 1027 {
        return Some(Style::Summon);
    }
    if art == 1034 {
        return Some(Style::Raise);
    }
    if art == 1031 {
        return Some(Style::MasterBite);
    }
    if (1035..=1037).contains(&art) {
        return Some(Style::Bats);
    }
    if art == 1033 {
        return Some(Style::Kiss);
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
    // A spell's clip is its slot's whatever the class and the hand, at the rate of the casts'
    // speed (`0x51E110`).
    if let Style::Cast(slot) = style {
        return Some(ArtAction {
            fixed: true,
            ..slot_of(slot)
        });
    }
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
        // The hurled blades 1013 the slot 147 at the blend 0.5 and the rate 1.0 (`0x50325C`),
        // which need an item in a hand (`0x5031C6`; **hypothesis**: the class 0 holds none).
        (2, Style::Throw) if class != 0 => Some(plain(147)),
        // The backbreaker: his `Hardhit`'s slot at the rate 1.0 (seen: 202 with a one-handed
        // weapon; the others' **hypothesis**).
        (2, Style::Backbreaker) => Some(plain(match class {
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
        // The Vampiress's claw jump by night: the slot 153 (seen at the rate 3).
        (7, Style::Jump) => Some(plain(153)),
        // Her blood bite by night: the slot 144 at the rate 1.0 (seen).
        (7, Style::Bite) => Some(plain(144)),
        // Her domination: the slot 149 at the rate 1.0 (seen by day; by night a **hypothesis**).
        (6 | 7, Style::Dominate) => Some(plain(149)),
        // Her call of the wolf: the slot 147 at the blend 0.5 and the rate 1.0, by day and by
        // night alike (`0x513C6D`; **hypothesis**: the night's model has its own 147).
        (6 | 7, Style::Summon) => Some(plain(147)),
        // Her awakening by night: the slot 150 at the blend 0.5 and the rate 1.0 (`0x513868`).
        (7, Style::Raise) => Some(plain(150)),
        // Her master's bite by night: the slot 154 at the rate 1.0 (seen).
        (7, Style::MasterBite) => Some(plain(154)),
        // Her bats by night: the slot 145 at the blend 0.5 and the rate 1.0 (seen, 1036).
        (7, Style::Bats) => Some(plain(145)),
        // Her death's kiss by night: the slot 148 at the rate 1.0 (seen).
        (7, Style::Kiss) => Some(plain(148)),
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
        // His spellproof 1046 the slot 147 at the blend 0.5 and the rate 1.0 (`0x509E62`).
        (4, Style::WardSpells) => Some(plain(147)),
        // His traps the first of the slots 109, 84 and 79 that his model has, at the blend 0.5 and
        // the rate 1.0 (`0x50A0FC`); 79 is not tried.
        (4, Style::Trap) => Some(ArtAction {
            fallback: Some(84),
            ..plain(109)
        }),
        (1, Style::Blades) => Some(ArtAction {
            rate: 1.6,
            ..plain(148)
        }),
        (8, Style::Boon) => Some(plain(154)),
        // His guns the slots 147 (1085), 145 (1086) and 146 (1087) at the blend 0.5 and the rate
        // [`GUN_RATE`] (`0x518CF6`, `0x518C65`, `0x518D87`).
        (8, Style::Flamer) => Some(ArtAction {
            rate: GUN_RATE,
            ..plain(147)
        }),
        (8, Style::Mortar) => Some(ArtAction {
            rate: GUN_RATE,
            ..plain(145)
        }),
        (8, Style::Cannon) => Some(ArtAction {
            rate: GUN_RATE,
            ..plain(146)
        }),
        // His 1084 the slot 144 at the blend 0.5 and the rate 1.0 (`0x518BF5`), which the order
        // refuses unless what his right hand holds is of the class 2 or 3 (`0x51D840`).
        (8, Style::Sweep) if matches!(class, 2 | 3) => Some(plain(144)),
        // His 1083 the slot 201 so (`0x518E07`), refused so too.
        (8, Style::Hurl) if matches!(class, 2 | 3) => Some(plain(201)),
        // His 1082 the slot 157 at the rate 1.0 (`0x514790` at `0x518600`).
        (8, Style::Frenzy) => Some(plain(157)),
        // The Daemon's 1094 the slot 150 at the blend 0.5 and the rate 1.0 (`0x5192E0` at
        // `0x51BC63`), and his charge 1091 the slot 148 so (seen).
        (9, Style::Boon) => Some(plain(150)),
        (9, Style::Charge) => Some(plain(148)),
        // His 1101 the slot 155 and his 1103 the slot 144 so (`0x51B46E`).
        (9, Style::Ring) => Some(plain(155)),
        (9, Style::Fan) => Some(plain(144)),
        // His 1102 the slot 157 so (197 mounted).
        (9, Style::Breath) => Some(plain(157)),
        // The Daemon's forms the slot 160 at the blend 0.2 and the rate 1.0 on himself
        // (`0x5192E0` at `0x51BEF5`; 197 at the attack's rate otherwise, not followed).
        (9, Style::Form) => Some(plain(160)),
        // The Gladiator's 1015 the slot 151 (`0x4FF0B0` at `0x502C5A`; 169 mounted), and his
        // 1016 the slot 150 (at `0x502C85`).
        (2, Style::Boon) => Some(plain(151)),
        (2, Style::Glance) => Some(plain(150)),
        // The Seraphim's 1018 the slot 149 at the blend 0.5 and the rate 1.0 (`0x5038F0` at
        // `0x506017`; 154 mounted), and her 1019 the slot 152 so (at `0x506137`).
        (1, Style::Boon) => Some(plain(149)),
        (1, Style::Bolts) => Some(plain(152)),
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
    /// Whether it ends as it gives them, the rest of its clip cut (the Seraphim's 1018).
    pub ends: bool,
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
/// - the Seraphim's 1018 below 0.5 the effect [`BFG`] of the whole value `L` for `b` seconds,
///   and it ends (`0x5038F0`; seen: 30 s at the level 1), and her 1019 below 0.35 the effect
///   [`RAILGUN`] so (its object 844 left out);
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
            ends: false,
        });
    }
    // The Seraphim's 1018 and 1019 their effects of the whole value `L`, and they end
    // (`0x5038F0`).
    if let Some((id, point)) = match record.id {
        1018 => Some((BFG, 0.5)),
        1019 => Some((RAILGUN, 0.65)),
        _ => None,
    } {
        let given = Effect {
            id,
            whole: i32::try_from(level).unwrap_or(i32::MAX),
            value: 0.0,
            art: u32::from(record.id),
            level: 0,
        };
        return Some(Boon {
            point,
            effects: vec![(given, b)],
            clears: Vec::new(),
            starts: Vec::new(),
            replaces: Vec::new(),
            ends: true,
        });
    }
    // The Dwarf's 1082 below 0.9 its effect 47 of the whole value `b` and the value `[+0x60]`
    // for `a` seconds, keyed by it (`0x514790` at `0x516040`).
    if record.id == 1082 {
        let frenzy = Effect {
            id: FRENZY,
            whole: i32::try_from(whole_units(f64::from(b))).unwrap_or(0),
            value: narrow(f64::from(record.value_60[0])),
            art: u32::from(record.id),
            level: 1,
        };
        return Some(Boon {
            point: 0.1,
            effects: vec![(frenzy, a)],
            clears: Vec::new(),
            starts: Vec::new(),
            replaces: Vec::new(),
            ends: false,
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
        ends: false,
    })
}

/// The effect of the Dwarf's 1082 (`0x2F`): while it holds, his attack speed is his stats' and
/// his frenzy's (the creature's float `+0x584`; [`quickened`], `0x584FE0`), which his strikes
/// raise toward its whole value ([`frenzy_rises`]) and which falls by its value a second
/// ([`frenzy_falls`]).
pub const FRENZY: u32 = 47;

/// The Dwarf's frenzy `frenzy` after a strike of his lands, his effect [`FRENZY`] of the whole
/// value `whole` holding (`0x56B130` at `0x56DC31`): a third of the way to the whole value, held
/// at it.
pub fn frenzy_rises(frenzy: f32, whole: i32) -> f32 {
    let whole = narrow(f64::from(whole));
    if frenzy < whole {
        frenzy + (whole - frenzy) * 0.333_333_34
    } else {
        whole
    }
}

/// The Dwarf's frenzy `frenzy` after a frame of `dt` seconds, his effect [`FRENZY`] of the value
/// `value` holding (`0x537E57`): less the value a second, a fifth of it while he fights a foe
/// (his query `0x11`, seen: 1 a second while he struck a goblin, 5 once it was dead), and 0 once
/// it comes to about 0 (`0x890C9C`).
pub fn frenzy_falls(frenzy: f32, value: f32, dt: f32, fighting: bool) -> f32 {
    let value = if fighting { value * 0.2 } else { value };
    let frenzy = frenzy - value * dt;
    if frenzy <= 1.192_092_9e-7 {
        0.0
    } else {
        frenzy
    }
}

/// The Vampiress's bloodlust `lust` (the creature's float `+0x584`, as the Dwarf's frenzy) after
/// a strike of hers lands in her night form (`+0x14` bit `0x2000`; `0x56B130` at `0x56DB08`):
/// `(lust + 10) · 1.1` while it is at most 90, else as it is.
pub fn bloodlust_rises(lust: f32) -> f32 {
    if lust <= 90.0 {
        narrow((f64::from(lust) + 10.0) * f64::from(1.1_f32))
    } else {
        lust
    }
}

/// The health that a strike of the Vampiress in her night form takes, `damage` before her
/// bloodlust `lust` (risen by the strike, [`bloodlust_rises`]) adds to it (`0x56DB80`): with a
/// bloodlust above 0 and below 150, `⌊lust · damage⌋ / 65` more, the division truncated.
pub fn bloodlust_damage(damage: u32, lust: f32) -> u32 {
    if lust > 0.0 && lust < 150.0 {
        let more = (f64::from(lust) * f64::from(damage)).trunc();
        damage.saturating_add(u32::try_from(whole_units(more) / 65).unwrap_or(0))
    } else {
        damage
    }
}

/// How fast the Vampiress's bloodlust falls, a second (`0x537DEC`: 5, or 1 while she fights a
/// foe, as [`frenzy_falls`] of this value).
pub const BLOODLUST_FALLS: f32 = 5.0;

/// The attack speed of a creature whose stats give `speed`, its frenzy `frenzy` (`0x584FE0`): their
/// sum truncated, at most 220.
pub fn quickened(speed: f32, frenzy: f32) -> f32 {
    (speed + frenzy).trunc().min(220.0)
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
        // The backbreaker's 80 (**hypothesis**: the distance itself; seen: he came within a
        // cell of the goblin warrior).
        1..=99 => Some((narrow(f64::from(distance)), MADE_WITHIN)),
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
/// (`0x5038F0` at `0x504570`), the Gladiator's 0.8 and 0.6 (`0x4FF0B0` at `0x50057C`), the
/// Vampiress's by night 0.8 and 0.35 (`0x50E6C0`; seen: her flight 0.2 of her clip in).
pub fn jump_points(kind: u32) -> [f32; 2] {
    match kind {
        2 => [0.8, 0.6],
        7 => [0.8, 0.35],
        _ => [0.75, 0.4],
    }
}

/// The part of its clip at which the death's kiss's curse lands on its target (seen: 1.84 s into
/// the 3.17 s of the slot 148 on a goblin two cells off; its missile, made below 0.6 left, and
/// its flight are left out), and how long the curse holds (seen: 5 s at the levels 1 and 9).
pub const KISSES: f32 = 0.6;
pub const CURSE_SECONDS: f32 = 5.0;
/// The kiss's skull 905 (`FX_FLYINGSKULL`, `0x7A6370`, `0x5A6680`): its speed, how fast it turns
/// toward its aim in radians a second, how near it touches a creature with the creature's size,
/// how far it looks for a creature to home on (a squared 100,000) and for its next one (a squared
/// 1,000,000).
pub const SKULL_SPEED: f32 = 150.0;
pub const SKULL_TURNS: f32 = 3.0;
pub const SKULL_REACH: f32 = 20.0;
pub const SKULL_LOOKS: f32 = 316.2;
pub const SKULL_NEXT: f32 = 1_000.0;
/// The effect of the kiss's bleeding (`0x560340`).
pub const BLEEDS: u32 = 23;

/// How long the kiss of `record` at the level in play `level` keeps its skull flying
/// (`0x7A6370`): `[+0x58] + [+0x5C]·L` seconds, 8 + 0.5·L.
pub fn skull_life(record: &WeaponArt, level: u32) -> f32 {
    let [base, step] = record.value_58.map(f64::from);
    narrow(f64::from(level).mul_add(step, base))
}

/// The value of the effect 23 that the kiss of `record` at the level in play `level` gives
/// (`0x5ABD80` at `0x5AF091`): ten times its factor, `10·(1 + 0.1·L)`, 10 + L (seen: 11 at the
/// level 1, 19 at 9).
pub fn bleed_value(record: &WeaponArt, level: u32) -> f32 {
    let [base, step] = record.factor.map(f64::from);
    narrow(10.0 * f64::from(level).mul_add(step, base))
}

/// A step of `dt` seconds of the bleeding of the value `value` whose gathered whole is `whole`
/// (`0x560340`): `whole` grows by `⌊100·v·dt⌋`, and once it is 100 or more and above `99·v` it
/// strikes for `⌊whole/100⌋` physical and keeps the rest of 100; the new whole and the strike.
pub fn bleed(whole: i32, value: f32, dt: f32) -> (i32, Option<u32>) {
    let grown = f64::from(whole) + (100.0 * f64::from(value) * f64::from(dt)).trunc();
    let whole = i32::try_from(whole_units(grown)).unwrap_or(i32::MAX);
    if whole >= 100 && f64::from(whole) > 99.0 * f64::from(value) {
        (whole % 100, u32::try_from(whole / 100).ok())
    } else {
        (whole, None)
    }
}

/// The value of the effects 1 and 2 that the death's kiss of `record` at the level in play
/// `level` gives its target (`0x50E6C0`): `−1/(L·[+0x54] + [+0x50])`, `−1/(1 + 0.1·L)` (seen:
/// −0.909 at level 1 and −0.526 at level 9), its attack and defence multiplied by
/// `1 − 1/(1 + 0.1·L)`.
pub fn curse(record: &WeaponArt, level: u32) -> f32 {
    let [base, step] = record.factor.map(f64::from);
    narrow(-1.0 / f64::from(level).mul_add(step, base).max(f64::EPSILON))
}

/// What is left of its clip, as a part of it, below which the domination takes its target
/// (`0x50E6C0`; seen: 1.77 s into the 4.29 s of the slot 149), and the effect it gives it, which
/// sets its `+0x1F4` bit `0x20000` (`0x582D50`): while it holds the target does not fight her.
pub const DOMINATES: f32 = 0.6;
pub const DOMINATED: u32 = 13;

/// How long the domination of `record` at the level in play `level` holds a target of the level
/// `target` (`0x50E6C0`): its factor `L·[+0x54] + [+0x50]` in seconds, 17 + 3·L by day and
/// 21 + 4·L by night, times `5L/M` where the target's level `M` is above `5L` (seen: 20 s on a
/// goblin warrior of level 1 at level 1).
pub fn domination_seconds(record: &WeaponArt, level: u32, target: u32) -> f32 {
    let [base, step] = record.factor.map(f64::from);
    let seconds = f64::from(level).mul_add(step, base);
    let most = 5 * level;
    if target > most {
        narrow(seconds * f64::from(most) / f64::from(target))
    } else {
        narrow(seconds)
    }
}

/// What is left of its clip, as a part of it, below which a trap is thrown (`0x506770`).
pub const TRAPS: f32 = 0.7;
/// The trap's ball (867, `0x5A55B2`, `0x5A706C`): how high its arc rises at its middle, and how
/// fast its course's part grows a second, so that it flies 2/3 s at any distance.
pub const BALL_RISE: f32 = 60.0;
pub const BALL_RATE: f32 = 1.5;
/// The traps' objects' reaches, their sizes in `items.pak` (`0x5AB490`): the explosion 794, the
/// pentagram 782 (its size 0 counting 50), the confusion 871 and the ground's poison 792; the
/// pentagram's time unarmed (`0x772C40`); the fog's sights 868 about its centre along each axis.
pub const EXPLOSION_REACH: f32 = 80.0;
pub const PENTAGRAM_REACH: f32 = 50.0;
pub const PENTAGRAM_ARMS: f32 = 2.0;
pub const CONFUSION_REACH: f32 = 160.0;
pub const POISON_REACH: f32 = 80.0;
pub const FOG_FROM: f32 = 60.0;

/// The spell whose record the trap `art` makes its trap of (`0x506770`, `0x4D2310`): 45 for the
/// bomb 1049, 6 for the pentagram 1050, 48 for the confusion 1051, 12 for the poison 1052 and 46
/// for the fog 1053.
pub fn trap_spell(art: u16) -> u16 {
    match art {
        1049 => 45,
        1050 => 6,
        1051 => 48,
        1052 => 12,
        _ => 46,
    }
}

/// How high over its straight course the trap's ball flies at the part `s` of it (`0x5A706C`):
/// `60·(1 − 4(s − 0.5)²)`.
pub fn ball_rise(s: f32) -> f32 {
    BALL_RISE * (1.0 - 4.0 * (s - 0.5) * (s - 0.5))
}

/// How long the confusion at the level `level` holds a creature of the level `creature`
/// (`0x5AD8D0`): `3·min(1, 5·L/M)` seconds.
pub fn confusion_seconds(level: u32, creature: u32) -> f32 {
    let ratio = 5.0 * crate::fight::to_f32(level) / crate::fight::to_f32(creature.max(1));
    3.0 * ratio.min(1.0)
}

/// Whether the fog at the level `level` takes a creature of the level `creature`, a boss or not
/// (the zone's kind 4, `0x59F960` at `0x59FDDD`): no boss, its level at most `5·L + 3`, as the
/// banishing circle's.
pub fn fog_takes(level: u32, creature: u32, boss: bool) -> bool {
    crate::spells::banishes(level, creature, boss)
}

/// What is left of its clip, as a part of it, below which the bats come (`0x50E6C0`), the most
/// level they come at for each level of the art, and their flags word's bits of a critter that
/// passes through the creatures (`0x1200000`).
pub const BATS: f32 = 0.87;
const BAT_LEVELS: u32 = 5;
pub const BATS_FLAGS: u32 = 0x120_0000;

/// The type of the bat that `art` calls (`0x50E6C0`): 594 for 1035, 595 for 1036, 596 for 1037.
pub fn bat(art: u16) -> u32 {
    match art {
        1035 => 594,
        1036 => 595,
        _ => 596,
    }
}

/// How many bats of its type the art `record` keeps at the level in play `level` (`0x50E6C0`):
/// its factor, `[+0x50] + [+0x54]·L`, at least 1 (5·L for 1035, 2 for 1036 and 1037).
pub fn bats_most(record: &WeaponArt, level: u32) -> u32 {
    u32::try_from(whole_units(f64::from(factor(record, level))))
        .unwrap_or(1)
        .max(1)
}

/// The level of a bat that the Vampiress of level `hero` calls at the level in play `level` in a
/// sector whose range's middle is `middle` (`sdk_HIRE`, `0x564D60`): the greater of hers and the
/// middle, at most `5·L`.
pub fn bat_level(hero: u32, middle: u32, level: u32) -> u32 {
    hero.max(middle).min(BAT_LEVELS * level).max(1)
}

/// What is left of its clip, as a part of it, below which the master's bite takes its target
/// (`0x50E6C0`; seen: 0.85 s into the slot 154), and a boss's level's factor in its test.
pub const MASTER_BITES: f32 = 0.6;
const MASTER_BITE_BOSS: f64 = 1.3;

/// Whether the master's bite at the level in play `level` of the Vampiress of level `hero` takes
/// a creature of `Creature.pak` class `class`, of a big type or not, of the level `creature`, a
/// boss or not (`0x50E6C0`): of none of the classes 10 and 9 nor a big type (`0x426B70`,
/// `0x426B20`, `0x426CB0`), its level, a boss's times 1.3, below `0.4·l + 5·L`.
pub fn master_bites(
    class: u8,
    big: bool,
    creature: u32,
    boss: bool,
    hero: u32,
    level: u32,
) -> bool {
    let creature = if boss {
        f64::from(creature) * MASTER_BITE_BOSS
    } else {
        f64::from(creature)
    };
    !matches!(class, 9 | 10)
        && !big
        && creature < 0.4f64.mul_add(f64::from(hero), 5.0 * f64::from(level))
}

/// How long the master's bite of `record` at the level in play `level` turns its target over:
/// `[+0x58] + [+0x5C]·L` seconds, 12 + 3·L (seen: 15 s at level 1, the effect 14 of the whole
/// value 1 keyed by 1031 on the target).
pub fn master_bite_seconds(record: &WeaponArt, level: u32) -> f32 {
    let [base, step] = record.value_58.map(f64::from);
    narrow(f64::from(level).mul_add(step, base))
}

/// What is left of its clip, as a part of it, below which the awakening raises its target
/// (`0x511088`), and how far from her a target may lie, with a straight way to it (`0x513868`).
pub const RAISES: f32 = 0.6;
pub const RAISES_WITHIN: f32 = 400.0;

/// Whether the awakening takes a dead creature of `Creature.pak` class `class` and type
/// `creature`, with a dialogue or not, a follower or not (`0x51D840`, case 4): no follower, no
/// dialogue, not of the class 10 nor of the type 55.
pub fn raises(class: u8, creature: u32, dialogue: bool, follower: bool) -> bool {
    !follower && !dialogue && class != 10 && creature != 55
}

/// The level of the creature that the Vampiress of level `hero` raises at the level in play
/// `level` (`0x511088`): `⌊3·L + 0.4·l⌋`.
pub fn raised_level(hero: u32, level: u32) -> u32 {
    u32::try_from(whole_units(
        0.4f64.mul_add(f64::from(hero), 3.0 * f64::from(level)),
    ))
    .unwrap_or(0)
}

/// How long the creature that the awakening of `record` raises at the level in play `level`
/// lives, the effect 29's time (`0x511088`): twice its factor, `2·(27 + 3·L)`.
pub fn raised_seconds(record: &WeaponArt, level: u32) -> f32 {
    2.0 * factor(record, level)
}

/// The spiders that the spider arrow 1006 makes at its first hit (`0x5BB010`): their type, their
/// side, a friend of the heroes', their flags word, the guards' (`0x4000`, docs/re/ai.md), and how
/// long they live, the effect 29's time.
pub const SPIDER: u32 = 71;
pub const SPIDERS_SIDE: u8 = 7;
pub const SPIDERS_FLAGS: u32 = 0x4000;
pub const SPIDERS_LIVE: f32 = 15.0;

/// How many spiders the spider arrow of `record` at the level in play `level` makes, and their
/// level, the local hero of the level `hero` (`0x5BB010`): `⌊([+0x60] + [+0x64]·L) / ([+0x68] +
/// [+0x6C]·L)⌋`, `⌊(10 + 130·L) / (50 + 20·L)⌋` in GOG 2.28, at least 1, each of the level
/// `min(5·L, M)` (`0x564D60`).
pub fn spiders(record: &WeaponArt, level: u32, hero: u32) -> (u32, u32) {
    let value = |[base, step]: [i32; 2]| i64::from(base) + i64::from(step) * i64::from(level);
    let below = value(record.value_68).max(1);
    let count = u32::try_from(value(record.value_60) / below)
        .unwrap_or(0)
        .max(1);
    (count, (5 * level).min(hero).max(1))
}

/// What is left of its clip, as a part of it, below which the call of the wolf makes its
/// summoning, and its regeneration starts whether it does or not (`0x512A13`, `0x512AEC`).
pub const SUMMONS: f32 = 0.6;
/// The wolf's level's factors of the call's level in play, by day and by night (`0x8910A8`,
/// `0x890768`).
const WOLF_BY_DAY: f64 = 2.5;
const WOLF_BY_NIGHT: f64 = 3.0;
/// The steps of the radius about the call's point that its summoning is tried at (`0x512A13`).
const SUMMONING_STEP: u32 = 20;
const SUMMONING_MOST: u32 = 200;

/// The level of the wolf that the Vampiress of level `hero` calls at the level in play `level`
/// (`0x512A13`): `⌊0.4·l + ⌊k·L⌋⌋`, `k` 2.5 by day and 3.0 by night (the renderer's day test
/// `0x41BA60`, not her form).
pub fn wolf_level(hero: u32, level: u32, night: bool) -> u32 {
    let k = if night { WOLF_BY_NIGHT } else { WOLF_BY_DAY };
    let level = (k * f64::from(level)).floor();
    u32::try_from(whole_units(0.4f64.mul_add(f64::from(hero), level))).unwrap_or(0)
}

/// Where the call of the wolf at `point` makes its summoning (`0x512A13`): for `r` 20, 40, … 200
/// one point `(x − r/2 + U[0, r], y − r/2 + U[0, r])`, the first that a straight way from her
/// reaches (`reaches`, `0x4FCC80`); none where none does.
pub fn summoning_point(
    [x, y]: [f32; 2],
    rng: &mut impl Dice,
    mut reaches: impl FnMut([f32; 2]) -> bool,
) -> Option<[f32; 2]> {
    (1..=SUMMONING_MOST / SUMMONING_STEP)
        .map(|i| i * SUMMONING_STEP)
        .map(|r| {
            let mut off = || {
                let u = rng.below(r + 1);
                crate::fight::to_f32(u) - crate::fight::to_f32(r) / 2.0
            };
            [x + off(), y + off()]
        })
        .find(|&at| reaches(at))
}

/// What is left of its clip, as a part of it, below which the blood bite bites (`0x50E6C0`), and
/// how long its target is held, with the effect 15 keyed by 1030 (seen: 0.44 s into the clip, and
/// the target frozen about 4.4 s).
pub const BITES: f32 = 0.9;
pub const BITE_HOLD: f32 = 4.4;

/// What the blood bite of `record` at the level in play `level` drains (`0x50E6C0`): its factor
/// `L·[+0x54] + [+0x50]`, 10 + 20·L, whole and at most 65,534, a fixed physical damage to its
/// target (the event `0x406`) and as much health to her (seen: at level 1 a goblin warrior lost
/// 25 of it through its resistance, and she took 30).
pub fn bite_drain(record: &WeaponArt, level: u32) -> u32 {
    let [base, step] = record.factor.map(f64::from);
    let drain = whole_units(f64::from(level).mul_add(step, base)).clamp(0, 65_534);
    u32::try_from(drain).unwrap_or(0)
}

/// The rate of the Vampiress's claw jump's clip (seen: 3, where the others' rule by the distance
/// gives 4; how she takes it, not read).
pub const CLAW_JUMP_RATE: f32 = 3.0;

/// How long the claw jump holds a creature that its roll takes as she lands (`0x50E6C0` at
/// `0x513005`: stopped, its timer set back by the float 3.0 at `0x890768`; seen: about 3 s).
pub const CLAW_HOLD: f32 = 3.0;

/// The radius about the Vampiress beyond the reach of the two and the fixed physical damage of the
/// strike her claw jump of `record` sends each creature in it as she lands, at the level in play
/// `level` (`0x50E6C0` at `0x512F6F`, the event 5 with the damage in its record's first fixed
/// damage): `L·[+0x54] + [+0x50]`, 190 + 10·L, and `L·[+0x5C] + [+0x58]`, 5 + 10·L, whole and at
/// most 65,534 (seen: each goblin warrior of level 1 about her lost 10 of it).
pub fn claw_quake(record: &WeaponArt, level: u32) -> (f32, f32) {
    let radius = {
        let [base, step] = record.factor.map(f64::from);
        narrow(f64::from(level).mul_add(step, base))
    };
    let damage = {
        let [base, step] = record.value_58.map(f64::from);
        whole_units(f64::from(level).mul_add(step, base)).clamp(0, 65_534)
    };
    (
        radius,
        narrow(f64::from(u32::try_from(damage).unwrap_or(0))),
    )
}

/// The screen's quake that the Gladiator's jump sends as he lands, an event of the weather of the
/// kind 7 (`0x4FF0B0` at `0x500765`; docs/re/weather.md): a shake of 15 pixels for 1.5 s.
pub const JUMP_SHAKES: u32 = 45;

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
/// from 495 to 500 and holds its clip (docs/decisions/arts.md, "Seraphim's jump rate at least 1").
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
        ends: false,
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

/// How long the blades of the Wood Elf's 1008 `record` at the level in play `level` fly, in
/// seconds, and the factor of their strikes (`0x5BD2D0`): `[+0x50] + [+0x54]·L`, 5.66 + 0.35·L,
/// and `[+0x58] + [+0x5C]·L`, 0.7 + 0.05·L.
pub fn blades(record: &WeaponArt, level: u32) -> (f32, f32) {
    let l = f64::from(level);
    let by_level = |[base, step]: [f32; 2]| narrow(l.mul_add(f64::from(step), f64::from(base)));
    (by_level(record.factor), by_level(record.value_58))
}

/// How far the Wood Elf's club arrow `record` at the level in play `level` knocks back the
/// creature it knocks (`0x5BB010`): the event 3 of the whole value `[+0x60] + [+0x64]·L`, 15.
pub fn club_knockback(record: &WeaponArt, level: u32) -> u32 {
    let [base, step] = record.value_60;
    u32::try_from(base.saturating_add(step.saturating_mul(i32::try_from(level).unwrap_or(0))))
        .unwrap_or(0)
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

/// What is left of its clip, as a part of it, below which the backbreaker lets its target go: 0.7
/// with a weapon of the classes 2 and 3, else 0.5 (`0x4FF0B0`; seen: 0.88 s into the 1.98 s of
/// `GLAD_SPECIAL_HHIT_1H` at 1.25 with a one-handed weapon).
pub fn backbreaker_lets_go(class: u8) -> f32 {
    if matches!(class, 2 | 3) { 0.7 } else { 0.5 }
}

/// How long the backbreaker holds its target of the level `target` that a Gladiator of the level
/// `maker` takes up (`0x4FF0B0`): `2·L' + 3 − M` seconds, from 5 to 10 (seen: about 5.4 s on a
/// goblin warrior of level 1 by the Gladiator of level 1).
pub fn backbreaker_hold(maker: u32, target: u32) -> f32 {
    let seconds = (2 * i64::from(maker) + 3 - i64::from(target)).clamp(5, 10);
    f32::from(u8::try_from(seconds).unwrap_or(10))
}

/// The factor of the backbreaker's strike at its level in play `level` (`0x4FF0B0`; `0x4D1EB0`
/// gives the same): `1 + 0.1·L`, its record's `[+0x50]` and `[+0x54]` not read.
pub fn backbreaker_factor(level: u32) -> f32 {
    narrow(f64::from(level).mul_add(0.1, 1.0))
}

/// Whether the backbreaker's target of the level `target` stays held once let go, by a roll
/// `roll` of 0 to 99 (`0x4FF0B0`): below `100·L/M`, its level in play `L` and `M` the target's,
/// times 0.769 for a boss; else its timer comes back within [`LET_GO`].
pub fn backbreaker_holds(level: u32, target: u32, boss: bool, roll: u32) -> bool {
    let mut chance = 100.0 * f64::from(level) / f64::from(target.max(1));
    if boss {
        chance *= 0.769;
    }
    f64::from(roll) < chance
}
pub const LET_GO: f32 = 0.5;

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

/// What the arrowproof or the spellproof `record` gives at the level in play `level`
/// ([`Style::Ward`], [`Style::WardSpells`], `0x506770` at `0x508561`), as a [`Boon`]: below
/// [`WARD_LEFT`] of its clip, on its target where it is of a hero's class (`0x426610`), else on
/// its maker, the effect `0x19`, the arrows' deflection (`arrows::deflection_chance`), or for
/// 1046 the effect `0x1A`, the spells' (`crate::spells::TURNS_SPELLS`), of
/// `[+0x58] + [+0x5C]·L` (1 + 0.05·L) for `[+0x50] + [+0x54]·L` seconds (44 + L), keyed by the
/// art and with its level; its whole value, the object 883 or 870 it makes, is left out.
pub fn ward(record: &WeaponArt, level: u32) -> Option<Boon> {
    let id = match record.id {
        1045 => crate::arrows::DEFLECTION,
        1046 => crate::spells::TURNS_SPELLS,
        _ => return None,
    };
    let l = f64::from(level);
    let by_level = |[base, step]: [f32; 2]| narrow(l.mul_add(f64::from(step), f64::from(base)));
    let effect = Effect {
        id,
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
        ends: false,
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
/// models' height (`0x44DED0`, `0x44DFE0`): `distance / 12` at its middle ([`arc`]).
pub fn jump_height(distance: f32, t: f32) -> f32 {
    arc(distance / 12.0, t)
}

/// How high a flight whose middle is `peak` high flies at the part `t` of it, in units of the
/// models' height (`0x44DFE0`): on a parabola from the ground to the ground.
pub fn arc(peak: f32, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    peak * (1.0 - 4.0 * (t - 0.5) * (t - 0.5))
}

/// The flight of the Daemon's charge to a target `distance` away (`0x5192E0` at `0x519CA6`): its
/// time in seconds, the distance times the float 1/1400 (`0x8910DC`), and how high its middle
/// flies ([`arc`]), a tenth of it (`0x890D9C`; seen: 0.096 s and 13.4 for 134).
pub fn charge_flight(distance: f32) -> (f32, f32) {
    (distance * f32::from_bits(0x3A3B_3EE7), distance * 0.1)
}

/// Whether a creature `offset` from the target of the Dwarf's 1084 `record` lies within its sweep
/// (`0x600730`): its squared whole distance at most `⌊[+0x58]⌋` (26,000, a radius of about 161).
pub fn sweep_reaches(record: &WeaponArt, [dx, dy]: [f32; 2]) -> bool {
    let [dx, dy] = [dx, dy].map(|d| f64::from(d.trunc()));
    dx.mul_add(dx, dy * dy) <= f64::from(record.value_58[0]).trunc()
}

/// The factor of the Dwarf's 1084's strikes at the creatures about its target, of `record` at
/// the level in play `level`: its own factor ([`factor`]) times `[+0x5C]` (0.6).
pub fn sweep_factor(record: &WeaponArt, level: u32) -> f32 {
    narrow(f64::from(factor(record, level)) * f64::from(record.value_58[1]))
}

/// A burn of the effect 62, which the caster keeps keyed by its target (the adder `0x584DDB`;
/// docs/re/arts.md, "The Daemon's"): its damage in its kind for its seconds, a blow whenever its
/// count of seconds passes 1.0, at once at its first update ([`Burn::step`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Burn {
    pub damage: f32,
    pub kind: usize,
    pub left: f32,
    count: f32,
}

impl Burn {
    /// A burn of `damage` in the damage kind `kind` (1 fire, 3 poison) for `seconds`.
    pub fn new(damage: f32, kind: usize, seconds: f32) -> Self {
        Self {
            damage,
            kind,
            left: seconds,
            count: 1.0,
        }
    }

    /// Steps it by `dt` seconds (`0x562730` at `0x563786`): its count takes them, and above 1.0 it
    /// strikes and starts again from 0; whether it strikes now.
    pub fn step(&mut self, dt: f32) -> bool {
        self.left -= dt;
        self.count += dt;
        let due = self.count > 1.0;
        if due {
            self.count = 0.0;
        }
        due
    }

    /// Whether its seconds are out.
    pub fn over(&self) -> bool {
        self.left <= 0.0
    }

    /// Its damages by kind, for the blow it strikes (held at 65,534).
    pub fn damages(&self) -> [f32; 4] {
        let mut damages = [0.0; 4];
        if let Some(damage) = damages.get_mut(self.kind) {
            *damage = self.damage.min(65_534.0);
        }
        damages
    }
}

/// What is left of the clip of the Daemon's 1101, 1102 and 1103 below which each acts
/// (`0x8907E4`).
pub const DAEMON_ACTS: f32 = 0.3;

/// How far from the Daemon his ring of poison reaches, besides a creature's size: its type's size
/// (`items.pak +0x14` of 1008).
pub const RING_REACH: f32 = 350.0;

/// The Daemon's 1101's, 1102's and 1103's ratio `⌊([+0x60] + [+0x64]·L)/([+0x68] + [+0x6C]·L)⌋`
/// of `record` at the level in play `level`, 0 for a divisor not above 2⁻²³: the seconds of the
/// burns of 1101 and 1102 (3 at the level 1), the count of 1103's bolts on each side of its aim.
pub fn daemon_ratio(record: &WeaponArt, level: u32) -> u32 {
    let l = f64::from(level);
    let [a, b] = record.value_60.map(f64::from);
    let [c, d] = record.value_68.map(f64::from);
    let divisor = l.mul_add(d, c);
    if divisor <= f64::from(f32::EPSILON) / 2.0 {
        return 0;
    }
    u32::try_from(whole_units(l.mul_add(b, a) / divisor)).unwrap_or(0)
}

/// The poison of each blow of the Daemon's 1101 (and the fire of 1102's) of `record` at the
/// level in play `level`, his multiplier of the kind `multiplier` (1.0 for another caster):
/// `m·([+0x50] + [+0x54]·L)` taken whole (10 + 10·L).
pub fn ring_poison(record: &WeaponArt, level: u32, multiplier: f32) -> f32 {
    let [base, step] = record.factor.map(f64::from);
    let value = f64::from(level).mul_add(step, base) * f64::from(multiplier);
    narrow(f64::from(i32::try_from(whole_units(value)).unwrap_or(0)))
}

/// The magic of each bolt of the Daemon's 1103 of `record` at the level in play `level`, his
/// magic multiplier `multiplier`: `m·([+0x58] + [+0x5C]·L)` taken whole (10 + 10·L).
pub fn fan_magic(record: &WeaponArt, level: u32, multiplier: f32) -> f32 {
    let [base, step] = record.value_58.map(f64::from);
    let value = f64::from(level).mul_add(step, base) * f64::from(multiplier);
    narrow(f64::from(i32::try_from(whole_units(value)).unwrap_or(0)))
}

/// The directions on the ground of the `2n + 1` bolts of the Daemon's 1103 toward `aim`, the
/// line to its target or point (`0x51B170`): over `(2n + 1)·25/d` radians about it, `d` its
/// length held to 50 to 300, in even steps, so that neighbours lie about 25 apart at `d`.
pub fn fan(aim: [f32; 2], n: u32) -> Vec<[f32; 2]> {
    let length = aim[0].hypot(aim[1]);
    let d = f64::from(length.clamp(50.0, 300.0));
    let base = f64::from(aim[1]).atan2(f64::from(aim[0]));
    let count = 2 * n + 1;
    let total = f64::from(count) * 25.0 / d;
    let step = if n == 0 {
        0.0
    } else {
        total / f64::from(2 * n)
    };
    (0..count)
        .map(|i| {
            let angle = base - total / 2.0 + step * f64::from(i);
            let angle = if n == 0 { base } else { angle };
            [narrow(angle.cos()), narrow(angle.sin())]
        })
        .collect()
}

/// How far the Daemon's breath of fire flies at most (`cParticleSystem_walloffire`).
pub const BREATH_MOST: f32 = 650.0;

/// The speed of the Daemon's breath of fire at its time `t` in seconds: `150·(1 − t/10)`, not
/// below 0.
pub fn breath_speed(t: f32) -> f32 {
    (150.0 * (1.0 - t / 10.0)).max(0.0)
}

/// How high over the ground the breath bounces at its time `t`, its start `rise` over the ground:
/// `(1 − t/10)·rise·|cos 3t|`.
pub fn breath_height(t: f32, rise: f32) -> f32 {
    ((1.0 - t / 10.0) * rise * (3.0 * t).cos().abs()).max(0.0)
}

/// How often the breath, stopped, lays its wall's fires, and how long each burns
/// (`FX_WALLOFFIRE_LINE`, its system's 10 s).
pub const WALL_EVERY: f32 = 0.15;
pub const WALL_BURNS: f32 = 10.0;

/// The spreads across its line at which the stopped breath lays its wall's fires in pairs, one on
/// each side: at each [`WALL_EVERY`] the spread grown at 200 a second, up to 200 (**estimate**: six
/// pairs, 30 to 180).
pub fn wall_spreads() -> Vec<f32> {
    (1..)
        .map(|k: u16| 200.0 * WALL_EVERY * f32::from(k))
        .take_while(|&spread| spread < 200.0)
        .collect()
}

/// The course of a bolt of the Daemon's 1103 (`cParticleSystem_emperor`): its speed, 500, less 300
/// a second, for 1.14 s, some 375 units.
pub const EMPEROR_COURSE: (f32, f32, f32) = (500.0, 300.0, 1.14);

/// The rate of the Dwarf's guns' clips: the stats' `+0x3E` over 100, 1.2 for the 120 a hero has
/// at the start (**hypothesis**: nothing raises it yet; docs/re/fight.md, "The values").
pub const GUN_RATE: f32 = 1.2;

/// The damage of a kind of the Dwarf's guns, its record's pair `[base, step]` at the level in play
/// `level`, his multiplier of the kind `multiplier` and the gun's factor `gun` (`0x4D21C0`,
/// [`crate::fight::Rules::glance_factor`] of his `a2`): `⌊⌊(base + step·L)·m⌋·g⌋`.
pub fn gun_damage([base, step]: [f32; 2], level: u32, multiplier: f32, gun: f64) -> f32 {
    let value = f64::from(level).mul_add(f64::from(step), f64::from(base));
    let first = whole_units(value * f64::from(multiplier));
    narrow(f64::from(
        i32::try_from(whole_units(
            f64::from(i32::try_from(first).unwrap_or(0)) * gun,
        ))
        .unwrap_or(0),
    ))
}

/// What is left of the flamethrower's clip within which it burns (`0x516D45`): from 0.65 to 0.2.
pub const FLAMES: [f32; 2] = [0.65, 0.2];
/// How far before the Dwarf his flames reach (`0x44CDF0`).
pub const FLAMES_LENGTH: f32 = 300.0;

/// The seconds of the flamethrower's burns, of `record` at the level in play `level`:
/// `max(2, ⌊([+0x60] + [+0x64]·L)/([+0x68] + [+0x6C]·L)⌋)` (3 at the level 1).
pub fn flame_seconds(record: &WeaponArt, level: u32) -> u32 {
    daemon_ratio(record, level).max(2)
}

/// How near the Dwarf a mortar's shell lands at least, how long after it leaves it strikes, and
/// how far about its point (`0x5B40CD`, `0x5B05E3`; the impact 1020's size).
pub const MORTAR_LEAST: f32 = 200.0;
pub const MORTAR_DELAY: f32 = 0.775;
pub const MORTAR_REACH: f32 = 150.0;

/// Where the Dwarf's mortar shell lands, from `here` toward the point `aim` (his target's place,
/// else the action's point): the point itself, or where it lies nearer than [`MORTAR_LEAST`],
/// that far along the line to it, truncated; along `facing` where the point is within 20.
pub fn mortar_point(here: [f32; 2], aim: [f32; 2], facing: [f32; 2]) -> [f32; 2] {
    let [dx, dy] = [aim[0] - here[0], aim[1] - here[1]];
    let distance = dx.hypot(dy);
    if distance >= MORTAR_LEAST {
        return aim;
    }
    let [ux, uy] = if distance > 20.0 {
        [dx / distance, dy / distance]
    } else {
        facing
    };
    [
        (here[0] + ux * MORTAR_LEAST).trunc(),
        (here[1] + uy * MORTAR_LEAST).trunc(),
    ]
}

/// What is left of the cannon's clip below which it shoots, its target `distance` off
/// (`0x5174E7`): `0.5 − 0.0002·d`, `d` held to 100 to 1,000, later the farther.
pub fn cannon_left(distance: f32) -> f32 {
    0.5 - 0.0002 * distance.clamp(100.0, 1000.0)
}

/// The cannon's line: steps of 40 to the first cell that stops it, at most 25.
pub const CANNON_STEP: f32 = 40.0;
pub const CANNON_STEPS: u32 = 25;
/// The part of each damage that the cannon's shot deals a creature other than its target, at
/// least 1 each; and how far its knockback slides a creature.
pub const CANNON_OTHERS: f32 = 0.1;
pub const CANNON_KNOCK: f32 = 100.0;

/// The damages of the cannon's shot of `record` at the level in play `level`, the Dwarf's
/// multipliers `multipliers` and gun factor `gun`, on its target (`target`) or another: the
/// physical of `[+0x50] + [+0x54]·L` times his fire multiplier, the fire of `[+0x58] +
/// [+0x5C]·L` times his physical one (**hypothesis**: swapped by a defect, as the code reads),
/// another creature a tenth of each, at least 1 each.
pub fn cannon_damages(
    record: &WeaponArt,
    level: u32,
    multipliers: [f32; 4],
    gun: f64,
    target: bool,
) -> [f32; 4] {
    let physical = gun_damage(record.factor, level, multipliers[1], gun);
    let fire = gun_damage(record.value_58, level, multipliers[0], gun);
    let part = |d: f32| {
        if target {
            d
        } else {
            (d * CANNON_OTHERS).trunc().max(1.0)
        }
    };
    [part(physical), part(fire), 0.0, 0.0]
}

/// The part of its velocity by which a thrown object moves, and by which its velocity falls, each
/// second (`0x44DFE0`).
pub const THROW_DRAG: f32 = 0.1;
/// How fast a thrown object falls faster each second (`0x44DFE0`).
pub const THROW_GRAVITY: f32 = 290.81;
/// How fast the Dwarf's 1083 throws its target up (`0x44DED0`'s third argument).
pub const THROW_RISE: f32 = 80.0;
/// The part of a flyer's speed that a creature it knocks over flies on at, and that it keeps
/// (`0x44DFE0`'s blows).
pub const BOWL_KEEPS: f32 = 0.9;
/// How long a creature that a flyer knocks over is held, in seconds.
pub const BOWL_HOLD: f32 = 2.0;

/// The flight of a thrown object (`0x44DED0` in its throw mode, its step `0x44DFE0`): its
/// velocity on the ground, its rise and its height over the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flight {
    pub velocity: [f32; 2],
    pub rise: f32,
    pub height: f32,
}

impl Flight {
    /// A flight of the velocity `velocity` on the ground and the rise `rise`, from the ground.
    pub fn new(velocity: [f32; 2], rise: f32) -> Self {
        Self {
            velocity,
            rise,
            height: 0.0,
        }
    }

    /// Moves it on by `dt` seconds: its place on the ground by [`THROW_DRAG`] of its velocity, its
    /// height by its rise; its velocity falls by that part of itself, its rise by
    /// [`THROW_GRAVITY`]; the step on the ground it took.
    pub fn step(&mut self, dt: f32) -> [f32; 2] {
        let moved = self.velocity.map(|v| THROW_DRAG * v * dt);
        self.velocity = self.velocity.map(|v| v - THROW_DRAG * v * dt);
        self.height += self.rise * dt;
        self.rise -= THROW_GRAVITY * dt;
        moved
    }

    /// Whether it has come down to the ground, its flight over.
    pub fn landed(&self) -> bool {
        self.height <= 0.0 && self.rise < 0.0
    }

    /// Its speed on the ground.
    pub fn speed(&self) -> f32 {
        self.velocity[0].hypot(self.velocity[1])
    }
}

/// The speed at which the Dwarf's 1083 `record` at the level in play `level` throws a creature of
/// type `creature` and level `target` (`0x514790`, `0x4D2160`): `35·s`, `s` its `[+0x50]` (400)
/// times `min(1, L/M)`, `[+0x50]·[+0x54]` (120) for a big type (`big`, `0x426CB0`) and 0 for the
/// type 52.
pub fn hurl_speed(
    record: &WeaponArt,
    level: u32,
    (creature, target): (u32, u32),
    big: bool,
) -> f32 {
    let strength = f64::from(record.factor[0]);
    let s = if creature == 52 {
        0.0
    } else if big {
        strength * f64::from(record.factor[1])
    } else {
        strength * (f64::from(level) / f64::from(target.max(1))).min(1.0)
    };
    narrow(35.0 * s)
}

/// How long the Dwarf's 1083 at the level in play `level` holds its target of level `target`, in
/// seconds: `max(2, 2·L + 3 − M)`.
pub fn hurl_hold(level: u32, target: u32) -> f32 {
    narrow((2.0 * f64::from(level) + 3.0 - f64::from(target)).max(2.0))
}

/// The factor of the Dwarf's 1083's strike at its target, `record` at the level in play
/// `level`: `L·[+0x5C] + [+0x58]` (0.9 + 0.1·L).
pub fn hurl_factor(record: &WeaponArt, level: u32) -> f32 {
    let [base, step] = record.value_58.map(f64::from);
    narrow(f64::from(level).mul_add(step, base))
}

/// What is left of the Gladiator's 1013's clip as the item leaves his hand (`0x501D39`).
pub const THROW_LEFT: f32 = 0.6;

/// How far beyond its target a hurled item flies, and how far one of a creature that is no player
/// flies in all (`0x501D39`).
const THROWN_BEYOND: f32 = 200.0;
const THROWN_FAR: f32 = 750.0;

/// Where an item that a thrower at `from` hurls at a target at `to` ends (`0x501D39`): `d + 200`
/// along the line from it, `d` the ground distance (at least 1), or 750 for a thrower that is no
/// player; and `d`.
pub fn throw_end(from: [f32; 2], to: [f32; 2], player: bool) -> ([f32; 2], f32) {
    let line = [to[0] - from[0], to[1] - from[1]];
    let d = line[0].hypot(line[1]).max(1.0);
    let far = if player {
        d + THROWN_BEYOND
    } else {
        THROWN_FAR
    };
    let end = [from[0] + line[0] / d * far, from[1] + line[1] / d * far];
    (end, d)
}

/// The height of a hurled item's arc: 80 for a player's (an id up to 16), 55 for another's
/// (`0x501F7F`).
pub fn throw_height(player: bool) -> f32 {
    if player { 80.0 } else { 55.0 }
}

/// The seconds a hurled item flies, the ground distance `d` to its target over 1,000
/// (`0x501D39`).
pub fn throw_time(d: f32) -> f32 {
    d / 1000.0
}

/// The point of a hurled item's arc from `from` to `to` at the share `s` of its flight
/// (`0x44DFE0`'s arc branch): along the line, and its height above the ground,
/// `height·(1 − 4(s − 0.5)²)`.
pub fn throw_point(from: [f32; 2], to: [f32; 2], s: f32, height: f32) -> ([f32; 2], f32) {
    let s = s.clamp(0.0, 1.0);
    let at = [
        from[0] + (to[0] - from[0]) * s,
        from[1] + (to[1] - from[1]) * s,
    ];
    let rise = (s - 0.5).powi(2).mul_add(-4.0, 1.0) * height;
    (at, rise)
}

/// The damages of an item that the Gladiator's 1013 hurls (`0x56B130` at `0x56BC81`,
/// `0x56ACD0`): each kind's least and most (`damage`), rolled at the hundredth, times `times`,
/// `1 + c(s₂)/100 + a0/BalanceGeschick` as 1016's (`crate::fight::Rules::glance_factor`).
pub fn thrown_damage(
    damage: &[[f32; 2]; crate::fight::KINDS],
    times: f64,
    dice: &mut impl crate::spawning::Dice,
) -> [f32; crate::fight::KINDS] {
    damage.map(|[least, most]| {
        let span = hundredths(most - least);
        let rolled = f64::from(least) + f64::from(dice.below(span + 1)) / 100.0;
        narrow(rolled * times)
    })
}

/// A span of damage in whole hundredths, none below 0.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a span of an item's damage, a few hundred at most, in hundredths"
)]
fn hundredths(span: f32) -> u32 {
    (span.max(0.0) * 100.0).round() as u32
}

/// The flight of a creature that a flyer of the velocity `velocity` knocks over, the line from the
/// flyer to it `to_it` (`0x44DFE0`'s blows): along the sum of the flyer's heading and that line,
/// made a unit, at [`BOWL_KEEPS`] of the flyer's speed, its rise `0.9·|v|·90/3500`.
pub fn bowled(velocity: [f32; 2], to_it: [f32; 2]) -> Flight {
    let unit = |[x, y]: [f32; 2]| {
        let length = x.hypot(y);
        if length > f32::EPSILON {
            [x / length, y / length]
        } else {
            [0.0, 0.0]
        }
    };
    let speed = velocity[0].hypot(velocity[1]);
    let [hx, hy] = unit(velocity);
    let [lx, ly] = unit(to_it);
    let [dx, dy] = unit([hx + lx, hy + ly]);
    let keeps = BOWL_KEEPS * speed;
    Flight::new([dx * keeps, dy * keeps], keeps * 90.0 / 3500.0)
}

/// How long the Daemon's blast lives, in seconds, and until when its radius grows (its particle
/// system's `0x7C1790` and `0x7C1890`).
pub const BLAST_LIFE: f32 = 3.0;
pub const BLAST_GROWS: f32 = 0.75;

/// The Daemon's blast as his charge lands, the object 1168 (`FX_KAMIKAZE`; docs/re/arts.md, "The
/// objects 1168 and 842"): its most radius `R`, `[+0x58] + [+0x5C]·L` at most `[+0x60]` taken
/// whole (150 + 2·L at most 400), the factor of its strikes, `[+0x50] + [+0x54]·L` ([`factor`],
/// 1.35 + 0.15·L), and its knockback's strength `[+0x64]` (20; the FX event `0x92`, `0x51A231`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blast {
    pub most: f32,
    pub factor: f32,
    pub strength: f32,
}

impl Blast {
    /// The blast of the charge `record` at the level in play `level`.
    pub fn of(record: &WeaponArt, level: u32) -> Self {
        let [base, step] = record.value_58.map(f64::from);
        let most = f64::from(level)
            .mul_add(step, base)
            .min(f64::from(record.value_60[0]));
        Self {
            most: narrow(f64::from(
                i32::try_from(whole_units(most)).unwrap_or(i32::MAX),
            )),
            factor: factor(record, level),
            strength: narrow(f64::from(record.value_60[1])),
        }
    }

    /// Its radius at its time `t` in seconds, its last `last` (set as it is drawn, `0x7C1890`):
    /// `R·t` taken whole below [`BLAST_GROWS`], then three quarters of the last taken whole, so
    /// that it falls to nothing within a few frames.
    pub fn radius(&self, t: f32, last: f32) -> f32 {
        let radius = if t < BLAST_GROWS {
            f64::from(self.most) * f64::from(t)
        } else {
            f64::from(last) * 0.75
        };
        narrow(f64::from(i32::try_from(whole_units(radius)).unwrap_or(0)))
    }

    /// The share of the Daemon's strike at a creature `distance` from him (`0x5ABD80` at
    /// `0x5B1EB1`): `(R − d)/R · max(1, f/2)`; none beyond `R`, where the original takes `R − d`
    /// as an unsigned number, a factor of some four thousand million over `R`
    /// (docs/decisions/arts.md, "Daemon's blast edge and level").
    pub fn share(&self, distance: u32) -> Option<f32> {
        let most = f64::from(self.most);
        let distance = f64::from(distance);
        (distance <= most && most > 0.0)
            .then(|| narrow((most - distance) / most * f64::from((self.factor * 0.5).max(1.0))))
    }
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

/// The art that the right button makes for the selected `art` of a hero of type `kind`
/// (`0x552370` with its flag clear, as the mouse's handler `0x6172C0` asks it): the Vampiress's of
/// her form ([`counterpart`]); the Daemon's second art of a pair of `0x891154` while an effect
/// keyed by its first holds (`holds`, `0x582B00`), and its first otherwise (seen: 1091 selected
/// made 1094); the art itself for any other.
pub fn right_button(art: u16, kind: u32, holds: impl Fn(u16) -> bool) -> u16 {
    if kind != 9 {
        return counterpart(art, kind);
    }
    for (first, second) in DAEMON {
        if art == first && holds(first) {
            return second;
        }
        if art == second && !holds(first) {
            return first;
        }
    }
    art
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

    /// Gives the entries their bonuses of the items' bonuses `bonuses` (each an art's id and a
    /// value, [`crate::fight::art_bonuses`]), the old ones cleared (`0x5796A0` at `0x57AC6E`):
    /// each art taken to its generic one (`0x52DA80`, [`generic`]) and found as `0x581D40` finds
    /// it ([`Arts::index`]), its value added to that entry's bonus not for good
    /// (`0x581F80(index, v, 0, 0)`), refused where the bonus would pass 255; then `every`, the
    /// sets' bonus of every art held (`0x571710`, sets 19 and 47), to each entry so.
    pub fn set_bonuses(&mut self, bonuses: &[(u16, u16)], every: u16) {
        for entry in &mut self.entries {
            entry.bonus = 0;
        }
        let raise = |entry: &mut Entry, value: u16| {
            let raised = u32::from(entry.bonus) + u32::from(value);
            if let Ok(bonus) = u8::try_from(raised) {
                entry.bonus = bonus;
            }
        };
        for &(art, value) in bonuses {
            let Some(index) = self.index(generic(art)) else {
                continue;
            };
            raise(&mut self.entries[index], value);
        }
        if every > 0 {
            for entry in &mut self.entries {
                raise(entry, every);
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

    /// Binds the shortcut's slot `slot` (the key F1 to F6 by its number) to `art` (`0x580D00`):
    /// every entry of that slot keeps only the four low bits of its word, and the art's word
    /// becomes the slot alone in its high byte, its mark of [`TAUGHT`] cleared with the rest.
    pub fn bind(&mut self, art: u16, slot: u8) {
        for entry in &mut self.entries {
            if entry.slot() == slot {
                entry.flags &= 0xF;
            }
            if entry.art == art {
                entry.flags = u16::from(slot) << 8;
            }
        }
    }

    /// Takes the art `art`'s shortcut off, as [`Self::bind`] takes one off the other entries of
    /// its slot: its word keeps only its four low bits. The original's spells page takes the key
    /// off its list alone and keeps the entry's, which its next showing puts back
    /// (docs/decisions/arts.md, "Spells page's key cleared with its binding").
    pub fn unbind(&mut self, art: u16) {
        for entry in self.entries.iter_mut().filter(|entry| entry.art == art) {
            entry.flags &= 0xF;
        }
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
