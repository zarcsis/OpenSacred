//! The creatures' casts (docs/re/ai.md, "The creatures' spells"; #1852): which types cast which
//! spells by their marks (`0x43C540`, the fight's specials `0x4D4E50`), the band they keep from
//! their target, their deadline between two casts and the rolls of their decisions. A creature
//! does not choose from its arts' list, which `Creature.pak` leaves empty: its spells are its
//! type's. The mark 1 first: the mummies, the magic skeletons and the orc shaman.

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

/// The band a caster of the mark 1 keeps from its target (`0x4E0300`'s sub-state 1): it walks to
/// [`BAND_POINT`] from its target where it stands nearer than [`BAND_NEAR`] after it was ordered
/// a cast, or farther than [`BAND_FAR`], at most [`BAND_MOVES`] times.
pub const BAND_NEAR: f32 = 240.0;
pub const BAND_POINT: f32 = 270.0;
pub const BAND_FAR: f32 = 300.0;
pub const BAND_MOVES: u8 = 25;

/// Whether a caster at `distance` from its target walks back into its band, `ordered` a cast
/// before, `moves` the moves it made toward the band.
pub fn leaves_band(distance: f32, ordered: bool, moves: u8) -> bool {
    moves < BAND_MOVES && ((distance < BAND_NEAR && ordered) || distance > BAND_FAR)
}

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
    fn the_mark_1s_casters_cast_their_types_spells() {
        assert_eq!(mark_1(48), Some((FIREBALL, 1)));
        assert_eq!(mark_1(80), Some((FIREBALL, 2)));
        assert_eq!(mark_1(64), Some((WHIRLWIND, 0)));
        assert_eq!(mark_1(66), Some((LIGHTNING, 0)));
        assert_eq!(mark_1(67), Some((METEORS, 0)));
        assert_eq!(mark_1(33), None);
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
