//! The sets' bonuses (docs/re/sets.md, #1727): what wearing pieces of an item set gives its
//! wearer, as the client's `0x571710` applies them in its switch over the set's number, each
//! bonus from a count of the set's pieces on, its value scaled by the difficulty's factor `D`.
//!
//! [`TABLE`] holds the bonuses that the switch writes into the stats; the bonuses that other code
//! applies by the counts (the strike, the luck, the death, docs/re/sets.md §4) follow in later
//! steps of #1727. [`told`] gives the lines of the equipment window's list of the sets (#1670)
//! from the same values that [`stats`] applies.

use std::collections::BTreeMap;

use crate::drops::Difficulty;
use crate::fight::KINDS;

/// The kinds of damage and resistance, in the stats' order.
const PHYSICAL: usize = 0;
const FIRE: usize = 1;
const MAGIC: usize = 2;
const POISON: usize = 3;

/// The difficulty's factor `D` that the sets' bonuses take (`0x51CEF0`, `0x55E070`,
/// `0x55E0F0`): 1.0, 1.34, 1.67, 2.0 and 2.34 from bronze to niobium.
pub fn factor(difficulty: Difficulty) -> f32 {
    match difficulty {
        Difficulty::Bronze => 1.0,
        Difficulty::Silver => 1.34,
        Difficulty::Gold => 1.67,
        Difficulty::Platinum => 2.0,
        Difficulty::Niobium => 2.34,
    }
}

/// One of the stats' three arrays of damage multipliers (docs/re/fight.md): the first (`+0x96`),
/// the weapons' second (`+0xA6`) and the spells' (`+0xB6`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Array {
    Damage,
    Weapon,
    Spells,
}

/// What a set's bonus gives, its number `c` scaled by `D`: whole, `⌊c·D⌋` (`0x55E070`), or a
/// float, `c·D` (`0x55E0F0`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Give {
    /// The attribute `a_i` (the stats' `+0x10 + 2i`), `⌊c·D⌋`.
    Attribute(usize, f32),
    /// The skill's bonus, `⌊c·D⌋`, in the first of the stats' slots that holds the skill, none
    /// where none does (`0x5827E0`, `0x5824A0`).
    Skill(u8, f32),
    /// Each of these arts' bonus byte, `⌊c·D⌋` (`0x581F80`), a bonus that would pass 255
    /// refused.
    Arts(&'static [u16], f32),
    /// Every art the wearer holds, `⌊c·D⌋`.
    AllArts(f32),
    /// A damage multiplier of these kinds in an array, `c·D`.
    Multiplier(Array, &'static [usize], f32),
    /// A kind's resistance factor (`+0xD6…`), `c·D`.
    ResistanceFactor(usize, f32),
    /// A kind's resistance (`+0x66…`), `⌊c·D⌋`.
    Resistance(usize, f32),
    /// The attack speed (`+0x3C`), `⌊c·D⌋`.
    AttackSpeed(f32),
    /// `A`'s and `D`'s multipliers (`+0xE6`, `+0xEA`), `c·D`.
    AttackTimes(f32),
    DefenceTimes(f32),
    /// The spells' regeneration speed (`+0xEE`), `c·D`.
    ArtsSpeed(f32),
    /// The pace (`+0x40`) times `c·D`, truncated (`0x55E0F0(1.15)`).
    PaceTimes(f32),
    /// A bit of the stats' flags `+0x148`, which other code reads (docs/re/sets.md §5).
    Flag(u32),
    /// A bit of the creature's `+0x1FC`.
    CreatureFlag(u32),
    /// The sets 33 to 38's: each kind's most damage the attribute `a0`, its least `⌊a0/2⌋`, and
    /// each resistance `a1`, set in place of what the items gave (`0x575CA3`, `0x575F32`).
    Festive,
    /// Set 24's: the physical least and most each `+⌊a0·⌊c·D⌋/100⌋`.
    StrengthPart(f32),
    /// Set 46's: the four resistance factors by the Resistance skill's (11) curves at the level
    /// `max(1, ⌊level/4⌋)`, over 100, and the pace by its pace's curve, truncated.
    ResistanceCurves,
    /// Set 48's: with a bow or a crossbow in hand (`0x54A2A0`, **hypothesis** of the weapon
    /// groups 7 and 13), `A`'s multiplier by the Ranged Combat skill's (6) curve at the level
    /// `max(1, ⌊level/3⌋)`, over 100, and the attack speed by its speed's curve, truncated.
    RangedCurves,
    /// A [`Give::Flag`] only while the set's three first sockets of one of its pieces each hold
    /// another piece of it (the stats' byte `+0x154 + set` at 7).
    FlagWhenFilled(u32),
    /// A [`Give::AttackSpeed`] only so.
    AttackSpeedWhenFilled(f32),
    /// A [`Give::ArtsSpeed`] only so.
    ArtsSpeedWhenFilled(f32),
}

/// A set's bonus from a count of its worn pieces on (each case tests `count ≥ pieces`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    pub set: u8,
    pub pieces: u8,
    pub gives: Give,
}

const fn at(set: u8, pieces: u8, gives: Give) -> Entry {
    Entry { set, pieces, gives }
}

use Array::{Damage, Spells, Weapon};
use Give::{
    AllArts, ArtsSpeed, ArtsSpeedWhenFilled, AttackSpeed, AttackSpeedWhenFilled, AttackTimes,
    Attribute, CreatureFlag, DefenceTimes, Festive, Flag, FlagWhenFilled, Multiplier, PaceTimes,
    RangedCurves, Resistance, ResistanceCurves, ResistanceFactor, Skill, StrengthPart,
};

const ALL_KINDS: &[usize] = &[PHYSICAL, FIRE, MAGIC, POISON];

/// The bonuses that `0x571710` writes into the stats, by set and pieces (docs/re/sets.md §3);
/// the switch's text-only rows, and the bonuses that other code applies by the counts, are not
/// here. Set 42's arts at 4 are 1030 to 1037, those of a wearer without the creature's `+0x14`
/// bit `0x2000` (**hypothesis** of what the bit tells; with it every art).
pub const TABLE: &[Entry] = &[
    at(1, 6, ResistanceFactor(POISON, 0.6)),
    at(2, 3, Attribute(1, 10.0)),
    at(2, 3, Attribute(3, 10.0)),
    at(2, 4, Skill(9, 3.0)),
    at(2, 4, Skill(11, 3.0)),
    at(2, 5, Skill(10, 2.0)),
    at(3, 3, Give::Arts(&[1014, 1015, 1016], 2.0)),
    at(3, 4, Flag(0x8000_0000)),
    at(4, 2, Skill(5, 4.0)),
    at(4, 3, Skill(2, 4.0)),
    at(5, 3, Multiplier(Damage, &[MAGIC], 0.5)),
    at(5, 5, ResistanceFactor(FIRE, 1.0)),
    at(6, 3, Attribute(4, 5.0)),
    at(6, 4, Attribute(3, 5.0)),
    at(6, 5, Attribute(0, 10.0)),
    at(6, 5, Attribute(2, 10.0)),
    at(6, 5, Attribute(3, 10.0)),
    at(6, 8, Give::Arts(&[15, 18, 20, 22], 2.0)),
    at(6, 9, Give::Arts(&[13, 17, 19, 21, 1019], 2.0)),
    at(7, 3, AttackSpeed(20.0)),
    at(7, 5, Skill(25, 3.0)),
    at(7, 5, AttackTimes(0.15)),
    at(8, 3, Attribute(0, 10.0)),
    at(8, 3, Attribute(1, 8.0)),
    at(8, 5, Skill(4, 3.0)),
    at(9, 4, Skill(2, 3.0)),
    at(9, 4, Skill(9, 3.0)),
    at(9, 4, Skill(10, 3.0)),
    at(10, 3, Skill(7, 3.0)),
    at(10, 3, Attribute(2, 10.0)),
    at(10, 5, Give::Arts(&[1004, 1005, 1006, 1007, 1008], 2.0)),
    at(11, 3, Skill(2, 3.0)),
    at(11, 3, Skill(3, 3.0)),
    at(11, 5, Attribute(0, 20.0)),
    at(11, 5, Attribute(1, 10.0)),
    at(12, 2, Skill(14, 3.0)),
    at(12, 4, Skill(19, 2.0)),
    at(12, 4, Attribute(4, 10.0)),
    at(13, 3, Attribute(2, 15.0)),
    at(13, 5, Skill(26, 2.0)),
    at(13, 5, Skill(27, 2.0)),
    at(13, 7, Give::Arts(&[1049, 1050, 1051, 1052, 1053], 2.0)),
    at(14, 3, Attribute(2, 10.0)),
    at(14, 4, Attribute(0, 10.0)),
    at(14, 5, Skill(25, 3.0)),
    at(14, 6, Skill(2, 3.0)),
    at(15, 3, Attribute(1, 10.0)),
    at(15, 3, Attribute(2, 10.0)),
    at(15, 5, Skill(8, 2.0)),
    at(15, 5, Skill(27, 2.0)),
    at(16, 3, Skill(12, 3.0)),
    at(16, 3, Skill(14, 3.0)),
    at(16, 5, ResistanceFactor(PHYSICAL, 0.15)),
    at(17, 2, Skill(2, 4.0)),
    at(17, 2, Skill(4, 4.0)),
    at(17, 4, AttackSpeed(30.0)),
    at(18, 3, Attribute(0, 10.0)),
    at(18, 5, Skill(2, 3.0)),
    at(18, 5, Skill(3, 3.0)),
    at(18, 7, Skill(11, 3.0)),
    at(19, 5, AllArts(2.0)),
    at(20, 3, Skill(9, 3.0)),
    at(20, 3, Skill(11, 3.0)),
    at(20, 7, AttackTimes(0.3)),
    at(20, 7, DefenceTimes(0.3)),
    at(21, 3, Skill(9, 3.0)),
    at(21, 3, Skill(20, 3.0)),
    at(22, 3, Resistance(FIRE, 30.0)),
    at(22, 5, Multiplier(Weapon, &[FIRE], 1.0)),
    at(22, 5, ResistanceFactor(FIRE, 1.0)),
    at(24, 3, Multiplier(Spells, ALL_KINDS, 0.3)),
    at(24, 5, StrengthPart(40.0)),
    at(24, 6, ArtsSpeed(0.2)),
    at(24, 6, Give::Arts(&[13], 10.0)),
    at(25, 3, Multiplier(Spells, ALL_KINDS, 0.2)),
    at(25, 4, ArtsSpeed(0.1)),
    at(25, 4, Give::Arts(&[17], 10.0)),
    at(26, 3, Skill(2, 5.0)),
    at(26, 4, DefenceTimes(0.3)),
    at(26, 5, Multiplier(Damage, &[POISON], 0.4)),
    at(27, 3, Give::Arts(&[1049], 6.0)),
    at(27, 4, Multiplier(Damage, &[FIRE], 0.5)),
    at(27, 6, CreatureFlag(0x4)),
    at(28, 3, Skill(9, 7.0)),
    at(28, 5, Multiplier(Damage, &[POISON], 0.4)),
    at(29, 2, Skill(7, 8.0)),
    at(29, 4, Multiplier(Damage, &[FIRE], 0.5)),
    at(30, 2, Skill(21, 10.0)),
    at(30, 3, Give::Arts(&[25], 4.0)),
    at(30, 4, Multiplier(Damage, &[MAGIC], 0.25)),
    at(31, 2, Skill(17, 6.0)),
    at(31, 3, Skill(15, 6.0)),
    at(31, 4, Multiplier(Damage, &[FIRE], 0.3)),
    at(31, 5, CreatureFlag(0x4)),
    at(32, 3, Skill(16, 6.0)),
    at(32, 4, Skill(18, 6.0)),
    at(32, 5, Multiplier(Damage, &[MAGIC], 0.4)),
    at(32, 7, CreatureFlag(0x8)),
    at(33, 3, Festive),
    at(34, 3, Festive),
    at(35, 3, Festive),
    at(36, 3, Festive),
    at(37, 3, Festive),
    at(38, 5, Festive),
    at(39, 2, Skill(14, 5.0)),
    at(39, 4, Attribute(4, 10.0)),
    at(39, 6, Flag(0x2)),
    at(40, 4, Flag(0x2000)),
    at(40, 6, Flag(0x8_0000)),
    at(41, 2, Skill(22, 5.0)),
    at(41, 5, Flag(0x10_0000)),
    at(41, 6, Flag(0x400)),
    at(42, 2, Skill(28, 5.0)),
    at(
        42,
        4,
        Give::Arts(&[1030, 1031, 1032, 1033, 1034, 1035, 1036, 1037], 3.0),
    ),
    at(42, 5, Flag(0x20_0000)),
    at(42, 6, Flag(0x40_0000)),
    at(43, 2, Skill(2, 5.0)),
    at(43, 5, Flag(0x80_0000)),
    at(43, 6, Flag(0x2_0000)),
    at(44, 2, Skill(5, 5.0)),
    at(44, 3, Flag(0x10_0000)),
    at(44, 5, Flag(0x40)),
    at(45, 2, DefenceTimes(0.3)),
    at(45, 3, Attribute(4, 10.0)),
    at(45, 5, Flag(0x200)),
    at(46, 2, Skill(2, 5.0)),
    at(46, 3, Attribute(0, 10.0)),
    at(46, 4, ResistanceCurves),
    at(46, 5, Flag(0x1)),
    at(47, 2, Skill(26, 5.0)),
    at(47, 3, Attribute(3, 10.0)),
    at(47, 4, AllArts(3.0)),
    at(47, 5, Flag(0x100)),
    at(48, 2, Skill(2, 5.0)),
    at(48, 3, Attribute(2, 10.0)),
    at(48, 6, RangedCurves),
    at(49, 2, Skill(2, 5.0)),
    at(49, 3, Attribute(0, 10.0)),
    at(49, 4, Flag(0x20_0000)),
    at(49, 6, Flag(0x80)),
    at(50, 2, Skill(5, 5.0)),
    at(50, 3, Give::Arts(&[1083, 1084], 2.0)),
    at(50, 6, Flag(0x20_0000)),
    at(51, 2, Skill(2, 5.0)),
    at(51, 3, AttackTimes(0.15)),
    at(51, 4, PaceTimes(1.15)),
    at(51, 6, Flag(0x1_0000)),
    at(52, 2, Skill(31, 5.0)),
    at(52, 3, Attribute(3, 10.0)),
    at(52, 6, Flag(0x800)),
    at(53, 2, Skill(33, 5.0)),
    at(53, 3, Give::Arts(&[55], 3.0)),
    at(54, 2, Skill(11, 5.0)),
    at(54, 3, Attribute(2, 10.0)),
    at(54, 6, ResistanceFactor(POISON, 0.25)),
    at(55, 2, Skill(2, 3.0)),
    at(55, 3, Skill(25, 5.0)),
    at(55, 4, AttackTimes(0.15)),
    at(55, 6, Attribute(0, 10.0)),
    at(55, 8, Flag(0x1000)),
    at(55, 10, Flag(0x4000_0000)),
    at(56, 2, Skill(25, 5.0)),
    at(56, 4, Attribute(3, 10.0)),
    at(56, 6, ResistanceFactor(POISON, 0.25)),
    at(56, 7, Flag(0x10)),
    at(57, 2, Skill(11, 5.0)),
    at(57, 3, DefenceTimes(0.15)),
    at(57, 4, Attribute(1, 10.0)),
    at(58, 2, Skill(32, 5.0)),
    at(58, 3, Skill(25, 5.0)),
    at(58, 4, Attribute(2, 10.0)),
    at(58, 6, Flag(0x4000)),
    at(59, 2, Skill(14, 5.0)),
    at(59, 3, Flag(0x10_0000)),
    at(59, 4, Attribute(4, 10.0)),
    at(59, 6, Flag(0x8000)),
    at(60, 2, Skill(4, 5.0)),
    at(60, 4, FlagWhenFilled(0x100_0000)),
    at(61, 2, Skill(5, 5.0)),
    at(61, 4, Flag(0x200_0000)),
    at(61, 4, FlagWhenFilled(0x400_0000)),
    at(62, 2, Attribute(2, 6.0)),
    at(62, 3, Multiplier(Weapon, &[PHYSICAL], 0.15)),
    at(62, 4, FlagWhenFilled(0x800_0000)),
    at(63, 2, Skill(7, 5.0)),
    at(63, 4, Flag(0x1000_0000)),
    at(63, 4, AttackSpeedWhenFilled(10.0)),
    at(64, 2, Skill(3, 5.0)),
    at(64, 3, Flag(0x20_0000)),
    at(64, 4, Flag(0x80_0000)),
    at(64, 4, FlagWhenFilled(0x2000_0000)),
    at(65, 2, Skill(14, 5.0)),
    at(65, 3, Flag(0x10_0000)),
    at(65, 4, Multiplier(Spells, ALL_KINDS, 0.15)),
    at(65, 4, ArtsSpeedWhenFilled(0.15)),
];

/// The pieces of the sets that a wearer wears (`0x5796A0`'s item loop, `0x5711F0`): by set, the
/// distinct types of its pieces (the list `0xAD4224`), and the sets one of whose pieces holds
/// another piece of the set in each of its three first sockets.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Worn {
    types: BTreeMap<u8, Vec<u32>>,
    filled: Vec<u8>,
}

impl Worn {
    /// The pieces of `pieces`, each an item's type and its set's number (`Weapon.pak +0x23`, 0
    /// for none): the distinct types of each set, as the recomputation lists them once each.
    pub fn of(pieces: impl IntoIterator<Item = (u32, u8)>) -> Self {
        let mut types: BTreeMap<u8, Vec<u32>> = BTreeMap::new();
        for (item, set) in pieces.into_iter().filter(|&(_, set)| set != 0) {
            let of_set = types.entry(set).or_default();
            if !of_set.contains(&item) {
                of_set.push(item);
            }
        }
        Self {
            types,
            filled: Vec::new(),
        }
    }

    /// Whether a piece of the type `item` is worn, which the list colours (`0x571710`'s text).
    pub fn wears(&self, item: u32) -> bool {
        self.types.values().any(|types| types.contains(&item))
    }

    /// With the set `set` filled: a piece of it holds another piece of it in each of its three
    /// first sockets.
    pub fn filled(mut self, set: u8) -> Self {
        if !self.filled.contains(&set) {
            self.filled.push(set);
        }
        self
    }

    /// How many distinct pieces of the set `set` are worn.
    pub fn count(&self, set: u8) -> u8 {
        self.types
            .get(&set)
            .map_or(0, |types| u8::try_from(types.len()).unwrap_or(u8::MAX))
    }

    /// The sets worn, each with its count, by their numbers.
    pub fn sets(&self) -> impl Iterator<Item = (u8, u8)> + '_ {
        self.types.keys().map(|&set| (set, self.count(set)))
    }

    /// The entries of [`TABLE`] that the counts reach, those of a filled set's sockets only
    /// where it is filled.
    pub fn reached(&self) -> impl Iterator<Item = &'static Entry> + '_ {
        TABLE.iter().filter(move |entry| {
            let filled = matches!(
                entry.gives,
                FlagWhenFilled(_) | AttackSpeedWhenFilled(_) | ArtsSpeedWhenFilled(_)
            );
            self.count(entry.set) >= entry.pieces && (!filled || self.filled.contains(&entry.set))
        })
    }
}

/// The special bonuses that the wearer's own values, or its level, its skills' curves and what it
/// holds, turn into numbers ([`Give::Festive`], [`Give::StrengthPart`] at its `⌊c·D⌋`,
/// [`Give::ResistanceCurves`], [`Give::RangedCurves`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Special {
    Festive,
    StrengthPart(f32),
    ResistanceCurves,
    RangedCurves,
}

/// A set's bonus valued at the difficulty's factor as the switch values it: a [`Give`] whose
/// number is `⌊c·D⌋` or `c·D`, both [`stats`] and [`told`] taking it from [`valued`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Bonus {
    Attribute(usize, f32),
    Skill(u8, u32),
    Arts(&'static [u16], u32),
    AllArts(u32),
    Multiplier(Array, &'static [usize], f32),
    ResistanceFactor(usize, f32),
    Resistance(usize, f32),
    AttackSpeed(f32),
    AttackTimes(f32),
    DefenceTimes(f32),
    ArtsSpeed(f32),
    PaceTimes(f32),
    Flag(u32),
    CreatureFlag(u32),
    Special(Special),
}

/// What the sets worn give the stats, summed over the entries reached, each number scaled by
/// the difficulty's factor as the switch scales it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stats {
    /// The pieces worn, whose counts the readers of the sets test and whose list the equipment
    /// window's set icon shows (#1670).
    pub worn: Worn,
    /// The attributes `a0` to `a5`.
    pub attributes: [f32; 6],
    /// The skills' bonuses, by skill.
    pub skills: Vec<(u8, u32)>,
    /// The arts' bonuses, by art, and the bonus of every art held.
    pub arts: Vec<(u16, u32)>,
    pub all_arts: u32,
    /// The three arrays of damage multipliers.
    pub damage: [f32; KINDS],
    pub weapon: [f32; KINDS],
    pub spells: [f32; KINDS],
    /// The resistances' factors and the resistances.
    pub resistance_factors: [f32; KINDS],
    pub resistances: [f32; KINDS],
    /// The attack speed, added whole.
    pub attack_speed: f32,
    /// `A`'s and `D`'s multipliers.
    pub attack_times: f32,
    pub defence_times: f32,
    /// The spells' regeneration speed.
    pub arts_speed: f32,
    /// The pace's factor, where a set gives one.
    pub pace_times: Option<f32>,
    /// The bits of the stats' flags `+0x148` and of the creature's `+0x1FC`.
    pub flags: u32,
    pub creature_flags: u32,
    pub specials: Vec<Special>,
    /// The factor of a strike's whole damage, `D`, where a bit of [`WHOLE_DAMAGE`] is set (the
    /// strike's resolution `0x56B130`, at `0x56D0C9`; the maintainer's decision keeps it, #1727).
    pub whole_damage: Option<f32>,
    /// The survival bonus's part of the whole set 28, `10·D`, which its readers add to the
    /// fight's share of the curve (docs/re/sets.md §4, M12; #1761).
    pub survival: f32,
    /// The difficulty's factor `D` of these values, which the readers of the counts take
    /// ([`Self::luck`], [`Self::rising`], [`Self::experience`]).
    pub factor: f32,
}

/// The flags of `+0x148` under which a strike's whole damage is times the difficulty's factor:
/// the last bonuses of the sets 39, 41, 44, 45, 47 and 49.
pub const WHOLE_DAMAGE: u32 = 0x7C2;
/// The stats' flag `+0x148` bit `0x200000` (sets 42, 49, 50 and 64), by which the strike grows the
/// stun's value (M18, `0x56D4D6`; [`Stats::stun`]).
pub const STUNS: u32 = 0x20_0000;

/// The term by which a set of the flag [`STUNS`] grows the stun's value, times `D` (M18): set 64's
/// 0.9, sets 49's and 50's 0.15, any other's 0.05.
fn stun_term(set: u8) -> f32 {
    match set {
        64 => 0.9,
        49 | 50 => 0.15,
        _ => 0.05,
    }
}

/// A bonus that other code applies by the counts of the sets worn (docs/re/sets.md §4), its
/// number `c` scaled by `D`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reader {
    /// The luck `ℓ` becomes `⌊ℓ·(1 + c·D) + 0.5⌋`, each set in turn (M2, `0x564A60`).
    Luck(f32),
    /// The undead's rising chance `p` becomes `⌊p·(100 − 100c·D)·0.01⌋`, at least 0, each set in
    /// turn (M5, `0x532C6F`, `0x532CD6`).
    Exorcism(f32),
    /// A kill's experience times `1 + c·D`, after the levels' weighing (M6, `0x57DDE3`).
    Experience(f32),
    /// The survival bonus's part `c·D` (M12).
    Survival(f32),
    /// Shift with a left click on a backpack's item turns it into its worth in gold (the query
    /// `0x51`, `0x55DE75`; M19; docs/re/inventory.md, §3).
    IntoGold,
    /// The hero's strike at a creature of the `Creature.pak` class `class` (its byte `+4`: 5 the
    /// undead, 10 the dragons, 9 the demons; `0x426A80`, `0x426B70`, `0x426B20`) lets these kinds'
    /// damages through its resistances times `1 + c·D` (M1, `0x56CA85`…).
    Against(u8, &'static [usize], f32),
    /// A drop's call of no type makes a potion in one case of ten (M7, `0x559CA9`;
    /// `drops::Kill::potions`); the list tells it in no line, the game having no text of it.
    Potions,
    /// The hero's sight on the maps' lens grows by `c·D` world units, once a group of its
    /// readers, the first that holds (M13, `0x6CE9B0`, `0x6CEA26`; docs/re/maps.md).
    Sight(f32, u8),
    /// The struck hero's defence `D` (its attack record's `+0x54`) times `1 + c·D` against an
    /// attacker that holds a ranged weapon or shoots (M11, `0x56B6D1`).
    RangedDefence(f32),
    /// The hero's critical hit's value (the effect 859) grows by `c·D`, each in turn (M3,
    /// `0x56E8CC`…; `fight::critical`).
    Critical(f32),
    /// The wounds' damage's value (the effect 867, a per cent) grows by `c·D` (M10, `0x56E178`,
    /// `0x56E1F4`; `fight::wounded`).
    Wounds(f32),
    /// The struck hero's effect 866's value grows by `c·D` (M4, `0x56D83F`;
    /// `fight::gold_for_health`).
    GoldForHealth(f32),
    /// The effect 856's value of a hit grows by `c·D` (M14, `0x56F68F`; `fight::gold_pile`).
    GoldPile(f32),
    /// The killer's effect 863, the exorcism, grows by `c·D` at an undead's death (M20,
    /// `0x532D77`, `0x532D9F`; `undead::exorcised`): set 61's flag `0x2000000` first, then set 54.
    Exorcist(f32),
    /// A struck creature that lives on is wounded (the effect 23) where `R(0, 100)` comes at most
    /// to `⌊c·D⌋`, one roll for each `c` reached (M15, `0x535183`, `0x535261`;
    /// `fight::open_wound`).
    OpenWounds(f32),
}

/// The `Creature.pak` classes that the strike's readers test (M1).
pub const UNDEAD: u8 = 5;
pub const DEMONS: u8 = 9;
pub const DRAGONS: u8 = 10;

/// A reader of a set's count, from a count of its pieces on (each tests `count ≥ pieces`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Read {
    pub set: u8,
    pub pieces: u8,
    pub reads: Reader,
}

const fn read(set: u8, pieces: u8, reads: Reader) -> Read {
    Read { set, pieces, reads }
}

/// The readers of the counts that the game follows, each kind in the order its code reads them
/// (docs/re/sets.md §4); set 21's luck asks six of its five pieces and set 18's exorcism eight of
/// its seven, so that neither ever holds.
pub const READERS: &[Read] = &[
    read(1, 2, Reader::Against(UNDEAD, &[FIRE], 0.6)),
    read(1, 5, Reader::Against(UNDEAD, ALL_KINDS, 0.3)),
    read(5, 5, Reader::Against(DRAGONS, ALL_KINDS, 0.3)),
    read(54, 5, Reader::Against(UNDEAD, &[POISON], 0.5)),
    read(58, 5, Reader::Against(DEMONS, ALL_KINDS, 0.5)),
    read(3, 8, Reader::Luck(0.2)),
    read(9, 7, Reader::Luck(0.2)),
    read(21, 6, Reader::Luck(0.2)),
    read(53, 7, Reader::Luck(0.2)),
    read(1, 6, Reader::Exorcism(0.3)),
    read(18, 8, Reader::Exorcism(0.3)),
    read(19, 5, Reader::Experience(0.1)),
    read(22, 5, Reader::Potions),
    read(28, 4, Reader::RangedDefence(0.5)),
    read(4, 4, Reader::Critical(5.0)),
    read(44, 4, Reader::Critical(9.0)),
    read(50, 4, Reader::Critical(15.0)),
    read(56, 5, Reader::Critical(9.0)),
    read(61, 3, Reader::Critical(6.0)),
    read(63, 3, Reader::Critical(6.0)),
    read(7, 7, Reader::Critical(10.0)),
    read(20, 4, Reader::Critical(10.0)),
    read(27, 5, Reader::Critical(5.0)),
    read(29, 5, Reader::Critical(6.0)),
    read(26, 6, Reader::Wounds(0.1)),
    read(62, 4, Reader::Wounds(0.2)),
    read(16, 6, Reader::GoldForHealth(30.0)),
    read(30, 5, Reader::GoldPile(10.0)),
    read(61, 4, Reader::Exorcist(0.15)),
    read(54, 7, Reader::Exorcist(0.3)),
    read(31, 5, Reader::OpenWounds(10.0)),
    read(60, 3, Reader::OpenWounds(10.0)),
    read(40, 2, Reader::OpenWounds(6.0)),
    read(48, 4, Reader::OpenWounds(6.0)),
    read(29, 3, Reader::Sight(50.0, 0)),
    read(52, 4, Reader::Sight(100.0, 1)),
    read(57, 6, Reader::Sight(100.0, 1)),
    read(28, 6, Reader::Survival(10.0)),
    read(53, 7, Reader::IntoGold),
];

impl Worn {
    /// The readers of [`READERS`] that the counts reach.
    pub fn reads(&self) -> impl Iterator<Item = &'static Read> + '_ {
        READERS
            .iter()
            .filter(move |read| self.count(read.set) >= read.pieces)
    }
}

/// Whether the wearer of `worn` answers the query `0x51` ([`Reader::IntoGold`]).
pub fn into_gold(worn: &Worn) -> bool {
    worn.reads().any(|read| read.reads == Reader::IntoGold)
}

impl Stats {
    /// The luck `luck` of the wearer, raised by its sets in turn ([`Reader::Luck`]).
    pub fn luck(&self, luck: f32) -> f32 {
        let d = self.factor;
        self.worn.reads().fold(luck, |luck, read| match read.reads {
            Reader::Luck(c) => (luck * (1.0 + c * d) + 0.5).floor(),
            _ => luck,
        })
    }

    /// The undead's rising chance `chance` in per cent where the wearer kills, cut by its sets in
    /// turn ([`Reader::Exorcism`]).
    pub fn rising(&self, chance: u32) -> u32 {
        let d = f64::from(self.factor);
        self.worn
            .reads()
            .fold(chance, |chance, read| match read.reads {
                Reader::Exorcism(c) => {
                    let cut = f64::from(chance) * (100.0 - 100.0 * f64::from(c) * d) * 0.01;
                    u32::try_from(whole_of(cut)).unwrap_or(0)
                }
                _ => chance,
            })
    }

    /// The factors of each kind's damage let through the resistances of a creature of the class
    /// `class` that the wearer strikes ([`Reader::Against`]), 1 where none.
    pub fn against(&self, class: u8) -> [f32; KINDS] {
        let mut times = [1.0; KINDS];
        for read in self.worn.reads() {
            if let Reader::Against(of, kinds, c) = read.reads
                && of == class
            {
                for &kind in kinds {
                    times[kind] *= 1.0 + c * self.factor;
                }
            }
        }
        times
    }

    /// The world units that the wearer's sight on the maps' lens grows by ([`Reader::Sight`]),
    /// each group's first reader that holds.
    pub fn sight(&self) -> f32 {
        let mut groups: Vec<u8> = Vec::new();
        let mut sight = 0.0;
        for read in self.worn.reads() {
            if let Reader::Sight(c, group) = read.reads
                && !groups.contains(&group)
            {
                groups.push(group);
                sight += c * self.factor;
            }
        }
        sight
    }

    /// The factor of the wearer's defence against a ranged attacker ([`Reader::RangedDefence`]),
    /// 1 where none.
    pub fn ranged_defence(&self) -> f32 {
        self.worn.reads().fold(1.0, |times, read| match read.reads {
            Reader::RangedDefence(c) => times * (1.0 + c * self.factor),
            _ => times,
        })
    }

    /// The critical hit's value `value` of the wearer's items, grown by its sets' terms in turn,
    /// each stored as a float ([`Reader::Critical`]).
    pub fn critical(&self, value: f32) -> f32 {
        self.worn
            .reads()
            .fold(value, |value, read| match read.reads {
                Reader::Critical(c) => value + c * self.factor,
                _ => value,
            })
    }

    /// The wounds' value `value` of the wearer's items, grown by its sets' terms
    /// ([`Reader::Wounds`]).
    pub fn wounds(&self, value: f32) -> f32 {
        self.worn
            .reads()
            .fold(value, |value, read| match read.reads {
                Reader::Wounds(c) => value + c * self.factor,
                _ => value,
            })
    }

    /// The effect 866's value `value` of the wearer's items, grown by its sets
    /// ([`Reader::GoldForHealth`]).
    pub fn gold_for_health(&self, value: f32) -> f32 {
        self.worn
            .reads()
            .fold(value, |value, read| match read.reads {
                Reader::GoldForHealth(c) => value + c * self.factor,
                _ => value,
            })
    }

    /// The effect 862's value `value` of the attacker's items, the stun, grown while its flag
    /// [`STUNS`] holds by one term alone (M18, `0x56D4D6`): `0.9·D` for set 64 at 3, else
    /// `0.15·D` for set 49 at 4 or set 50 at 5, else `0.05·D`.
    pub fn stun(&self, value: f32) -> f32 {
        if self.flags & STUNS == 0 {
            return value;
        }
        let term = if self.worn.count(64) >= 3 {
            stun_term(64)
        } else if self.worn.count(49) >= 4 || self.worn.count(50) >= 5 {
            stun_term(49)
        } else {
            stun_term(42)
        };
        value + term * self.factor
    }

    /// The effect 863's value `value` of the killer's items, grown by its sets in turn
    /// ([`Reader::Exorcist`]).
    pub fn exorcist(&self, value: f32) -> f32 {
        self.worn
            .reads()
            .fold(value, |value, read| match read.reads {
                Reader::Exorcist(c) => value + c * self.factor,
                _ => value,
            })
    }

    /// The rolls of the open wounds that the wearer's strikes give (M15, [`Reader::OpenWounds`]):
    /// a bound's factor for each of the two blocks reached, `10` by sets 31 and 60, `6` by sets
    /// 40 and 48, in that order, each block one roll however many of its sets hold.
    pub fn open_wounds(&self) -> [Option<f32>; 2] {
        let reached = |c: f32| {
            self.worn
                .reads()
                .any(|read| matches!(read.reads, Reader::OpenWounds(of) if of == c))
                .then_some(c)
        };
        [reached(10.0), reached(6.0)]
    }

    /// The effect 856's value `value` of the wearer's items, grown by its sets
    /// ([`Reader::GoldPile`]).
    pub fn gold_pile(&self, value: f32) -> f32 {
        self.worn
            .reads()
            .fold(value, |value, read| match read.reads {
                Reader::GoldPile(c) => value + c * self.factor,
                _ => value,
            })
    }

    /// Whether the wearer's kills drop potions ([`Reader::Potions`]).
    pub fn potions(&self) -> bool {
        self.worn.reads().any(|read| read.reads == Reader::Potions)
    }

    /// The share of a kill's experience that the wearer earns more ([`Reader::Experience`]).
    pub fn experience(&self) -> f32 {
        self.worn
            .reads()
            .map(|read| match read.reads {
                Reader::Experience(c) => c * self.factor,
                _ => 0.0,
            })
            .sum()
    }
}

/// `x` truncated toward zero and held at 0 and above.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a chance in per cent, well within an i64"
)]
fn whole_of(x: f64) -> i64 {
    x.trunc().max(0.0) as i64
}

/// `⌊c·D⌋`, the float product truncated (`0x55E070`).
fn whole(c: f32, d: f32) -> f32 {
    (c * d).trunc()
}

/// A whole bonus as a count, 0 for none.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a bonus of a few tens, whole and not negative"
)]
fn count(value: f32) -> u32 {
    value.max(0.0) as u32
}

/// The bonus that `gives` gives at the difficulty's factor `d`.
pub fn valued(gives: Give, d: f32) -> Bonus {
    match gives {
        Attribute(i, c) => Bonus::Attribute(i, whole(c, d)),
        Skill(skill, c) => Bonus::Skill(skill, count(whole(c, d))),
        Give::Arts(arts, c) => Bonus::Arts(arts, count(whole(c, d))),
        AllArts(c) => Bonus::AllArts(count(whole(c, d))),
        Multiplier(array, kinds, c) => Bonus::Multiplier(array, kinds, c * d),
        ResistanceFactor(kind, c) => Bonus::ResistanceFactor(kind, c * d),
        Resistance(kind, c) => Bonus::Resistance(kind, whole(c, d)),
        AttackSpeed(c) | AttackSpeedWhenFilled(c) => Bonus::AttackSpeed(whole(c, d)),
        AttackTimes(c) => Bonus::AttackTimes(c * d),
        DefenceTimes(c) => Bonus::DefenceTimes(c * d),
        ArtsSpeed(c) | ArtsSpeedWhenFilled(c) => Bonus::ArtsSpeed(c * d),
        PaceTimes(c) => Bonus::PaceTimes(c * d),
        Flag(bit) | FlagWhenFilled(bit) => Bonus::Flag(bit),
        CreatureFlag(bit) => Bonus::CreatureFlag(bit),
        Festive => Bonus::Special(Special::Festive),
        StrengthPart(c) => Bonus::Special(Special::StrengthPart(whole(c, d))),
        ResistanceCurves => Bonus::Special(Special::ResistanceCurves),
        RangedCurves => Bonus::Special(Special::RangedCurves),
    }
}

/// The survival bonus's part that the sets of `worn` add at the factor `d` ([`Reader::Survival`]).
fn survival(worn: &Worn, d: f32) -> f32 {
    worn.reads()
        .map(|read| match read.reads {
            Reader::Survival(c) => c * d,
            _ => 0.0,
        })
        .sum()
}

/// What the sets of `worn` give on the difficulty `difficulty` (`0x571710` over the sets worn).
pub fn stats(worn: &Worn, difficulty: Difficulty) -> Stats {
    let d = factor(difficulty);
    let mut stats = Stats {
        worn: worn.clone(),
        factor: d,
        ..Stats::default()
    };
    let add_skill = |stats: &mut Stats, skill: u8, value: u32| match stats
        .skills
        .iter_mut()
        .find(|(id, _)| *id == skill)
    {
        Some((_, bonus)) => *bonus += value,
        None => stats.skills.push((skill, value)),
    };
    for entry in worn.reached() {
        match valued(entry.gives, d) {
            Bonus::Attribute(i, value) => stats.attributes[i] += value,
            Bonus::Skill(skill, value) => add_skill(&mut stats, skill, value),
            Bonus::Arts(arts, value) => {
                for &art in arts {
                    match stats.arts.iter_mut().find(|(id, _)| *id == art) {
                        Some((_, bonus)) => *bonus += value,
                        None => stats.arts.push((art, value)),
                    }
                }
            }
            Bonus::AllArts(value) => stats.all_arts += value,
            Bonus::Multiplier(array, kinds, value) => {
                let to = match array {
                    Damage => &mut stats.damage,
                    Weapon => &mut stats.weapon,
                    Spells => &mut stats.spells,
                };
                for &kind in kinds {
                    to[kind] += value;
                }
            }
            Bonus::ResistanceFactor(kind, value) => stats.resistance_factors[kind] += value,
            Bonus::Resistance(kind, value) => stats.resistances[kind] += value,
            Bonus::AttackSpeed(value) => stats.attack_speed += value,
            Bonus::AttackTimes(value) => stats.attack_times += value,
            Bonus::DefenceTimes(value) => stats.defence_times += value,
            Bonus::ArtsSpeed(value) => stats.arts_speed += value,
            Bonus::PaceTimes(times) => stats.pace_times = Some(times),
            Bonus::Flag(bit) => stats.flags |= bit,
            Bonus::CreatureFlag(bit) => stats.creature_flags |= bit,
            Bonus::Special(special) => stats.specials.push(special),
        }
    }
    if stats.flags & WHOLE_DAMAGE != 0 {
        stats.whole_damage = Some(d);
    }
    stats.survival = survival(worn, d);
    stats
}

/// A line of a set's list in the equipment window's tooltip (#1670): a bonus that its pieces
/// worn reach, told as the game applies it (the rule of #1737; docs/re/sets.md §8).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Told {
    /// A bonus of the stats, valued.
    Bonus(Bonus),
    /// A strike's whole damage's share more, `D − 1`.
    WholeDamage(f32),
    /// The survival bonus's part, in per cent.
    Survival(f32),
    /// The backpack's items turned into gold ([`into_gold`]).
    IntoGold,
    /// The luck's share more, `c·D` ([`Reader::Luck`]).
    Luck(f32),
    /// The share cut off the undead's rising chance, `c·D` ([`Reader::Exorcism`]).
    Exorcism(f32),
    /// A kill's experience's share more, `c·D` ([`Reader::Experience`]).
    Experience(f32),
    /// These kinds' damages' share more, `c·D`, against a creature of the class
    /// ([`Reader::Against`]).
    Against(u8, &'static [usize], f32),
    /// The sight on the maps' lens's world units more, `c·D` ([`Reader::Sight`]).
    Sight(f32),
    /// The defence's share more against a ranged attacker, `c·D` ([`Reader::RangedDefence`]).
    RangedDefence(f32),
    /// The critical hit's value more, `c·D` ([`Reader::Critical`]).
    Critical(f32),
    /// The wounds' value's per cent more, `c·D` ([`Reader::Wounds`]).
    Wounds(f32),
    /// The effect 866's value more, `c·D` held at the difficulty's most of it
    /// ([`Reader::GoldForHealth`], `fight::gold_for_health_cap`): the per cent chance that a hit
    /// is paid from the gold.
    GoldForHealth(f32),
    /// The effect 856's value more, `c·D` ([`Reader::GoldPile`]): the per cent chance that a hit
    /// makes its defender drop a gold pile.
    GoldPile(f32),
    /// The chance, a share of 1, that a strike wounds the struck one ([`Reader::OpenWounds`]):
    /// `(⌊c·D⌋ + 1)/101`.
    OpenWounds(f32),
    /// The effect 863's value more, `c·D` ([`Reader::Exorcist`]).
    Exorcist(f32),
    /// The stun's value more, the set's own term times `D` ([`Stats::stun`]); one set's term
    /// alone holds where several are worn.
    Stun(f32),
}

/// The lines of the set `set` of `worn` on the difficulty `difficulty`, in the order of the pieces
/// they ask, a count's bonuses before its readers': each bonus of [`TABLE`] reached, valued as
/// [`stats`] values it; the six sets' flag of the whole damage as its share more, none on bronze,
/// where `D` changes nothing; no line of a flag whose readers the game does not follow; and each
/// reader of [`READERS`] reached.
pub fn told(set: u8, worn: &Worn, difficulty: Difficulty) -> Vec<Told> {
    let d = factor(difficulty);
    let bonuses = worn
        .reached()
        .filter(|entry| entry.set == set)
        .filter_map(|entry| {
            let line = match valued(entry.gives, d) {
                Bonus::Flag(bit) if bit & WHOLE_DAMAGE != 0 => {
                    (d > 1.0).then_some(Told::WholeDamage(d - 1.0))?
                }
                Bonus::Flag(STUNS) => Told::Stun(stun_term(entry.set) * d),
                Bonus::Flag(_) | Bonus::CreatureFlag(_) => return None,
                bonus => Told::Bonus(bonus),
            };
            Some((entry.pieces, line))
        });
    let readers = worn
        .reads()
        .filter(|read| read.set == set)
        .filter_map(|read| {
            let line = match read.reads {
                Reader::Luck(c) => Told::Luck(c * d),
                Reader::Exorcism(c) => Told::Exorcism(c * d),
                Reader::Experience(c) => Told::Experience(c * d),
                Reader::Survival(c) => Told::Survival(c * d),
                Reader::IntoGold => Told::IntoGold,
                Reader::Against(class, kinds, c) => Told::Against(class, kinds, c * d),
                Reader::Potions => return None,
                Reader::Sight(c, _) => Told::Sight(c * d),
                Reader::RangedDefence(c) => Told::RangedDefence(c * d),
                Reader::Critical(c) => Told::Critical(c * d),
                Reader::Wounds(c) => Told::Wounds(c * d),
                Reader::GoldForHealth(c) => {
                    Told::GoldForHealth((c * d).min(crate::fight::gold_for_health_cap(difficulty)))
                }
                Reader::GoldPile(c) => Told::GoldPile(c * d),
                Reader::Exorcist(c) => Told::Exorcist(c * d),
                Reader::OpenWounds(c) => Told::OpenWounds(crate::fight::wound_chance(c, d)),
            };
            Some((read.pieces, line))
        });
    let mut lines: Vec<(u8, Told)> = bonuses.chain(readers).collect();
    lines.sort_by_key(|&(pieces, _)| pieces);
    lines.into_iter().map(|(_, line)| line).collect()
}

#[cfg(test)]
mod tests;
