//! The sets' bonuses (docs/re/sets.md, #1727): what wearing pieces of an item set gives its
//! wearer, as the client's `0x571710` applies them in its switch over the set's number, each
//! bonus from a count of the set's pieces on, its value scaled by the difficulty's factor `D`.
//!
//! [`TABLE`] holds the bonuses that the switch writes into the stats; the bonuses that other code
//! applies by the counts (the strike, the luck, the death, docs/re/sets.md §4) and the lines of
//! the set's tooltip follow in later steps of #1727 and #1670.

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
/// count of the distinct types of its pieces, and the sets one of whose pieces holds another
/// piece of the set in each of its three first sockets.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Worn {
    counts: BTreeMap<u8, u8>,
    filled: Vec<u8>,
}

impl Worn {
    /// The counts of `pieces`, each an item's type and its set's number (`Weapon.pak +0x23`, 0
    /// for none): the distinct types of each set, as the recomputation lists them once each.
    pub fn of(pieces: impl IntoIterator<Item = (u32, u8)>) -> Self {
        let mut types: BTreeMap<u8, Vec<u32>> = BTreeMap::new();
        for (item, set) in pieces.into_iter().filter(|&(_, set)| set != 0) {
            let of_set = types.entry(set).or_default();
            if !of_set.contains(&item) {
                of_set.push(item);
            }
        }
        let counts = types
            .into_iter()
            .map(|(set, items)| (set, u8::try_from(items.len()).unwrap_or(u8::MAX)))
            .collect();
        Self {
            counts,
            filled: Vec::new(),
        }
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
        self.counts.get(&set).copied().unwrap_or(0)
    }

    /// The sets worn, each with its count.
    pub fn sets(&self) -> impl Iterator<Item = (u8, u8)> + '_ {
        self.counts.iter().map(|(&set, &count)| (set, count))
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

/// What the sets worn give the stats, summed over the entries reached, each number scaled by
/// the difficulty's factor as the switch scales it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stats {
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

/// What the sets of `worn` give on the difficulty `difficulty` (`0x571710` over the sets worn).
pub fn stats(worn: &Worn, difficulty: Difficulty) -> Stats {
    let d = factor(difficulty);
    let mut stats = Stats::default();
    let add_skill = |stats: &mut Stats, skill: u8, value: u32| match stats
        .skills
        .iter_mut()
        .find(|(id, _)| *id == skill)
    {
        Some((_, bonus)) => *bonus += value,
        None => stats.skills.push((skill, value)),
    };
    for entry in worn.reached() {
        match entry.gives {
            Attribute(i, c) => stats.attributes[i] += whole(c, d),
            Skill(skill, c) => add_skill(&mut stats, skill, count(whole(c, d))),
            Give::Arts(arts, c) => {
                for &art in arts {
                    match stats.arts.iter_mut().find(|(id, _)| *id == art) {
                        Some((_, bonus)) => *bonus += count(whole(c, d)),
                        None => stats.arts.push((art, count(whole(c, d)))),
                    }
                }
            }
            AllArts(c) => stats.all_arts += count(whole(c, d)),
            Multiplier(array, kinds, c) => {
                let to = match array {
                    Damage => &mut stats.damage,
                    Weapon => &mut stats.weapon,
                    Spells => &mut stats.spells,
                };
                for &kind in kinds {
                    to[kind] += c * d;
                }
            }
            ResistanceFactor(kind, c) => stats.resistance_factors[kind] += c * d,
            Resistance(kind, c) => stats.resistances[kind] += whole(c, d),
            AttackSpeed(c) | AttackSpeedWhenFilled(c) => stats.attack_speed += whole(c, d),
            AttackTimes(c) => stats.attack_times += c * d,
            DefenceTimes(c) => stats.defence_times += c * d,
            ArtsSpeed(c) | ArtsSpeedWhenFilled(c) => stats.arts_speed += c * d,
            PaceTimes(c) => stats.pace_times = Some(c * d),
            Flag(bit) | FlagWhenFilled(bit) => stats.flags |= bit,
            CreatureFlag(bit) => stats.creature_flags |= bit,
            Festive => stats.specials.push(Special::Festive),
            StrengthPart(c) => stats.specials.push(Special::StrengthPart(whole(c, d))),
            ResistanceCurves => stats.specials.push(Special::ResistanceCurves),
            RangedCurves => stats.specials.push(Special::RangedCurves),
        }
    }
    stats
}

#[cfg(test)]
mod tests;
