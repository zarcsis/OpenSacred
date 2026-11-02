//! The creatures' casts (docs/re/ai.md, "The creatures' spells"; #1852): which types cast which
//! spells by their marks (`0x43C540`, the fight's specials `0x4D4E50`), the band they keep from
//! their target, their deadline between two casts and the rolls of their decisions. A creature
//! does not choose from its arts' list, which `Creature.pak` leaves empty: its spells are its
//! type's. The mark 1: the mummies, the magic skeletons and the orc shaman; the mark 4's dragons
//! and big demons.

use crate::spells::{FIREBALL, WHIRLWIND};

/// `BLITZ_SERA`, the lightning of the mummy 03 (13).
pub const LIGHTNING: u16 = 13;
/// `METEORITENHAGEL`, the orc shaman's (26).
pub const METEORS: u16 = 26;
/// The flesh to stone (`FLEISCHSTEIN`, 25), which the mark 1's casters hold the hero with.
pub const STONE: u16 = 25;

/// What a caster of the mark 1 casts at its target (`0x4D51ED`'s path B): its spell and the
/// fireball's variant (`+0x13C`): the mummies 44, 64 and 65 `WIRBELWIND` (29), the mummy 03 (66)
/// the lightning (13), the magic skeletons 48 and 80 `FEUERBALL` (5) of the variants 1 and 2, the
/// orc shaman (67) `METEORITENHAGEL` (26); none for another type.
pub fn mark_1(creature: u32) -> Option<(u16, u8)> {
    Some(match creature {
        44 | 64 | 65 => (WHIRLWIND, 0),
        66 => (LIGHTNING, 0),
        48 => (FIREBALL, 1),
        80 => (FIREBALL, 2),
        67 => (METEORS, 0),
        _ => return None,
    })
}

/// Whether a creature of type `creature` is a dragon (`0x4D5B95`): of the mark 4 and of
/// `Creature.pak`'s class 10 (`0x426B70`), the types 52, 74, 88 and 100 to 103.
pub fn dragon(creature: u32) -> bool {
    matches!(creature, 52 | 74 | 88 | 100..=103)
}

/// How much farther than its reach a dragon casts its meteors from.
pub const DRAGON_BEYOND: f32 = 500.0;

/// Whether a dragon casts `METEORITENHAGEL` (26) at a decision (`0x4D5B95`): farther than its
/// reach `reach` and [`DRAGON_BEYOND`] from its target, by a roll `rand() % 2` of 0 (**hypothesis**
/// for the value: one decision of two), with no deadline and no test of the way.
pub fn dragon_casts(distance: f32, reach: f32, roll: u32) -> bool {
    distance > reach + DRAGON_BEYOND && roll.is_multiple_of(2)
}

/// Whether a creature of type `creature` is a big demon (`TYPE_NPC_BIG_DEMON`, 53 and 94), of the
/// mark 4.
pub fn big_demon(creature: u32) -> bool {
    matches!(creature, 53 | 94)
}

/// The big demons' own spells (docs/re/arts.md, "The creatures' own spells"): the eye lasers
/// (49), which land the demon's own strike on its target from afar where a straight line of
/// cells reaches it, and the demon's fire (50), a homing fire whose burst lands its strike on what
/// it touches; their records hold no damage.
pub const EYE_LASERS: u16 = 49;
pub const DEMON_FIRE: u16 = 50;

/// What a big demon casts at a decision of its melee (`0x4D5B95`), standing: in one decision of
/// three (a roll `rand() % 3` of 0; **hypothesis** for the value) with a straight way to its target
/// (`way`), the eye lasers in two cases of three (`choice`, a `rand() % 3` below 2) or the fire, with
/// no deadline; without the way, none: it runs about its target ([`big_demon_runs`]).
pub fn big_demon_casts(roll: u32, way: bool, choice: u32) -> Option<u16> {
    (roll.is_multiple_of(3) && way).then_some(if choice % 3 < 2 {
        EYE_LASERS
    } else {
        DEMON_FIRE
    })
}

/// Whether a big demon without a straight way runs about its target at a decision (`0x4D5B95`):
/// in the decision of three that would have cast.
pub fn big_demon_runs(roll: u32, way: bool) -> bool {
    roll.is_multiple_of(3) && !way
}

/// How many decisions a big demon stands facing its target after its run about it (`+0x129`).
pub const DEMON_STANDS: u8 = 50;

/// The distances of a big demon's casts as it comes on (`0x4E1F73`): from [`APPROACH_FROM`], the
/// eye lasers beyond [`LASERS_BEYOND`].
pub const APPROACH_FROM: f32 = 371.0;
pub const LASERS_BEYOND: f32 = 500.0;

/// What a big demon casts as it comes on to its target on its level (`0x4E0300`'s sub-state 1 at
/// `0x4E1F73`), at each tick: from [`APPROACH_FROM`] away, by a roll `rand() % 101` below 5, the
/// eye lasers beyond [`LASERS_BEYOND`], else the fire in four cases of five (`choice`, a
/// `rand() % 5` below 4) and the lasers in one.
pub fn big_demon_approaches(distance: f32, roll: u32, choice: u32) -> Option<u16> {
    (distance >= APPROACH_FROM && roll % 101 < 5).then_some(
        if distance > LASERS_BEYOND || choice % 5 >= 4 {
            EYE_LASERS
        } else {
            DEMON_FIRE
        },
    )
}

/// The homing fires (docs/re/arts.md, "The creatures' own spells"; the object update's flag
/// `0x10000`): held at the caster's hand for their charge, then flying at [`HOMING_SPEED`] a second
/// toward their target for [`HOMING_FLIGHT`] seconds, turning by at most [`HOMING_TURN`] radians a
/// second; the demon's fire charges [`DEMON_FIRE_CHARGE`] seconds and is of the size
/// [`DEMON_FIRE_SIZE`].
pub const HOMING_SPEED: f32 = 250.0;
pub const HOMING_FLIGHT: f32 = 5.0;
pub const HOMING_TURN: f32 = 3.0;
pub const DEMON_FIRE_CHARGE: f32 = 2.5;
pub const DEMON_FIRE_SIZE: f32 = 70.0;

/// The clip's slot that a creature of type `creature` casts the spell `spell` with, on foot
/// (`0x522030`): by the spell, a few types their own, the dragons (`dragon`, `Creature.pak`'s
/// class 10) always 81, Anducar (182) and his demon (183) always 154.
pub fn clip(spell: u16, creature: u32, dragon: bool) -> u16 {
    if dragon {
        return 81;
    }
    if matches!(creature, 182 | 183) {
        return 154;
    }
    let ice = matches!(creature, 140 | 146);
    match spell {
        1 | 2 | 4 | 16 | 18 | 27 | 35 => 163,
        3 | 8 | 14 | 42 => 160,
        5 | 29 | 33 | 37 | 41 | 58 | 59 | 61 | 64 | 65 => 155,
        6 => 164,
        7 | 12 | 36 => 161,
        11 | 55 => 156,
        13 | 15 | 22 if ice => 147,
        13 | 28 | 40 | 56 | 66 => 158,
        17 if creature == 141 => 154,
        26 if creature == 183 => 154,
        17 | 20 | 21 | 22 | 25 | 26 | 30..=32 | 34 => 157,
        19 | 39 => 159,
        24 | 38 => 162,
        49 => 144,
        50 => 145,
        53 => 84,
        _ => 154,
    }
}

/// The band a caster of the mark 1 keeps from its target (`0x4E0300`'s sub-state 1): it walks to
/// [`BAND_POINT`] from its target where it stands nearer than [`BAND_NEAR`] after it was ordered
/// a cast, or farther than [`BAND_FAR`], while its decisions on its casting path stay below
/// [`BAND_MOVES`] (`+0x12A`).
pub const BAND_NEAR: f32 = 240.0;
pub const BAND_POINT: f32 = 270.0;
pub const BAND_FAR: f32 = 300.0;
pub const BAND_MOVES: u8 = 25;

/// Whether a caster at `distance` from its target walks back into its band, `ordered` a cast
/// before, `moves` its decisions on its casting path in this fight, this one counted.
pub fn leaves_band(distance: f32, ordered: bool, moves: u8) -> bool {
    moves < BAND_MOVES && ((distance < BAND_NEAR && ordered) || distance > BAND_FAR)
}

/// How far about its target a caster runs to where it neither casts nor keeps its band.
pub const STROLL: f32 = 280.0;

/// How many world's ticks a caster of the mark 1 stands facing its target after a decision
/// (`+0x129`), before it decides again.
pub const STANDS: u8 = 10;

/// The seconds of the real clock until a caster may cast again (`0x4D4670(1)`, the deadline
/// `+0x24D`): `⌊(6 + roll + hostiles) · (1 − level / 300) · b⌋`, `roll` a `rand() % 3`,
/// `hostiles` the hero's hostiles of the marks 8 and `0x200` fighting hand to hand, `b` 0.8 for a
/// boss or a champion (`elite`), else 1.
pub fn deadline(level: u32, elite: bool, hostiles: u32, roll: u32) -> u32 {
    let base = f64::from(6 + roll % 3 + hostiles);
    let level = 1.0 - f64::from(level) / 300.0;
    let elite = if elite { 0.8 } else { 1.0 };
    whole((base * level * elite).floor())
}

/// Whether a caster of the mark 1 casts at a decision (`0x4D51ED`): a roll `rand() % 3` of 2, a
/// straight way to its target, and its deadline passed.
pub fn casts(roll: u32, way: bool, deadline_passed: bool) -> bool {
    roll % 3 == 2 && way && deadline_passed
}

/// Whether a caster of the mark 1 takes its casting path at a decision rather than fighting hand
/// to hand (`0x4D51ED`): where the hero has more than one hostile and it is not the nearest, or
/// else in two cases of three (`roll` a `rand() % 3` not 0).
pub fn casting(crowded: bool, nearest: bool, roll: u32) -> bool {
    (crowded && !nearest) || !roll.is_multiple_of(3)
}

/// The distances from which a caster of the mark 1 or 8 holds the hero (`0x4D51ED`'s path A).
pub const HOLD_FROM: f32 = 400.0;
pub const HOLD_TO: f32 = 1_500.0;

/// Whether a caster of the mark 1 casts the stone (25) at the hero (`0x4D51ED`'s path A): from
/// [`HOLD_FROM`] to [`HOLD_TO`] away, a roll `rand() % (n + 1)` of 2 with `n` 3 and the hero's
/// hostiles of the marks 8 and `0x200` fighting hand to hand (`0x4D4670(2)`), its level above 5
/// and at least the hero's, the hero not held already and a straight way to it.
pub fn holds(
    distance: f32,
    (roll, hostiles): (u32, u32),
    (level, hero_level): (u32, u32),
    (held, way): (bool, bool),
) -> bool {
    (HOLD_FROM..=HOLD_TO).contains(&distance)
        && roll % (4 + hostiles) == 2
        && level > 5
        && level >= hero_level
        && !held
        && way
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a count of seconds, not negative, far below the integers' range"
)]
fn whole(value: f64) -> u32 {
    value.max(0.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fight::{Rank, Rules};

    #[test]
    fn a_dragon_casts_its_meteors_from_beyond_its_reach_and_500_in_one_decision_of_two() {
        assert!(dragon(52) && dragon(101) && !dragon(53) && !dragon(94));
        assert!(dragon_casts(800.0, 250.0, 0));
        assert!(!dragon_casts(800.0, 250.0, 1));
        assert!(!dragon_casts(750.0, 250.0, 0));
    }

    #[test]
    fn a_big_demon_casts_its_lasers_or_its_fire_standing_and_coming_on() {
        assert!(big_demon(53) && big_demon(94) && !big_demon(52));
        // Standing: one decision of three with a way, the lasers in two cases of three.
        assert_eq!(big_demon_casts(0, true, 1), Some(EYE_LASERS));
        assert_eq!(big_demon_casts(3, true, 2), Some(DEMON_FIRE));
        assert_eq!(big_demon_casts(1, true, 0), None);
        assert_eq!(big_demon_casts(0, false, 0), None);
        assert!(big_demon_runs(0, false) && !big_demon_runs(0, true) && !big_demon_runs(2, false));
        // Coming on: from 371, by a roll below 5 of 101; beyond 500 the lasers, nearer the fire
        // in four cases of five.
        assert_eq!(big_demon_approaches(370.0, 0, 0), None);
        assert_eq!(big_demon_approaches(400.0, 5, 0), None);
        assert_eq!(big_demon_approaches(400.0, 4, 3), Some(DEMON_FIRE));
        assert_eq!(big_demon_approaches(400.0, 4, 4), Some(EYE_LASERS));
        assert_eq!(big_demon_approaches(600.0, 0, 0), Some(EYE_LASERS));
    }

    #[test]
    fn the_mark_1s_casters_cast_their_types_spells() {
        assert_eq!(mark_1(48), Some((FIREBALL, 1)));
        assert_eq!(mark_1(80), Some((FIREBALL, 2)));
        assert_eq!(mark_1(64), Some((WHIRLWIND, 0)));
        assert_eq!(mark_1(66), Some((LIGHTNING, 0)));
        assert_eq!(mark_1(67), Some((METEORS, 0)));
        assert_eq!(mark_1(33), None);
    }

    #[test]
    fn a_creature_casts_with_its_spells_clip() {
        assert_eq!(clip(FIREBALL, 48, false), 155);
        assert_eq!(clip(WHIRLWIND, 64, false), 155);
        assert_eq!(clip(LIGHTNING, 66, false), 158);
        assert_eq!(clip(LIGHTNING, 140, false), 147);
        assert_eq!(clip(STONE, 44, false), 157);
        assert_eq!(clip(METEORS, 67, false), 157);
        assert_eq!(clip(METEORS, 52, true), 81);
        assert_eq!(clip(5, 182, false), 154);
        assert_eq!(clip(53, 127, false), 84);
    }

    #[test]
    fn a_caster_waits_five_to_seven_seconds_shorter_as_it_rises() {
        // 6 to 8 at the level 1, five to seven seconds; a boss four to five.
        assert_eq!(deadline(1, false, 0, 0), 5);
        assert_eq!(deadline(1, false, 0, 2), 7);
        assert_eq!(deadline(1, true, 0, 2), 6);
        assert_eq!(deadline(60, false, 0, 0), 4);
        // Each hostile of the magicians in the melee a second more.
        assert_eq!(deadline(1, false, 2, 0), 7);
    }

    #[test]
    fn a_caster_keeps_its_band_and_casts_by_its_rolls() {
        assert!(leaves_band(200.0, true, 0) && !leaves_band(200.0, false, 0));
        assert!(leaves_band(400.0, false, 0) && !leaves_band(400.0, false, BAND_MOVES));
        assert!(!leaves_band(270.0, true, 0));
        assert!(casts(2, true, true) && !casts(1, true, true) && !casts(2, false, true));
        assert!(!casts(2, true, false));
        // Alone or the nearest, it fights hand to hand in one decision of three.
        assert!(!casting(false, true, 0) && casting(false, true, 1));
        assert!(casting(true, false, 0));
        // The stone from 400 to 1,500, its level above 5 and at least the hero's.
        assert!(holds(800.0, (2, 0), (6, 6), (false, true)));
        assert!(!holds(300.0, (2, 0), (6, 6), (false, true)));
        assert!(!holds(800.0, (2, 0), (5, 1), (false, true)));
        assert!(!holds(800.0, (2, 0), (6, 7), (false, true)));
        assert!(!holds(800.0, (2, 0), (6, 6), (true, true)));
    }

    #[test]
    fn a_creatures_spell_is_of_its_fifth_level_and_its_damage_of_magie_all() {
        let rules = Rules::default();
        assert_eq!(rules.creature_spell_level(9, Rank::Common), 2);
        assert_eq!(rules.creature_spell_level(9, Rank::Boss), 2);
        // The balance's 1.4 a float a little below it: 5 · 1.4 falls short of 7 (a quirk).
        assert_eq!(rules.creature_spell_level(20, Rank::Boss), 6);
        assert_eq!(rules.creature_spell_level(25, Rank::Boss), 8);
        assert_eq!(rules.creature_spell_level(20, Rank::Champion), 6);
        let damage = |level| (rules.creature_spell_damage(level) * 1000.0).round();
        assert_eq!(
            [1, 2, 3, 4, 5, 6, 7].map(damage),
            [597.0, 642.0, 707.0, 690.0, 790.0, 771.0, 767.0]
        );
    }
}
