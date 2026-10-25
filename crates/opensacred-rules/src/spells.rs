//! The spells' cast and their objects (docs/re/arts.md, "The spells' cast", "The objects'
//! systems", "The Mage's spells"; #672, #872): the clip that a spell plays, the share of it at
//! which the spell acts, and the fireball's flight. A spell's damages are the bolts' of its
//! record ([`crate::arts::bolt_damages`]) by the caster's spells' multipliers
//! ([`crate::fight::Combat::spells`]), and its regeneration starts as it acts.

use crate::fight::to_f32;

/// The fireball, `FEUERBALL`, the Mage's first spell.
pub const FIREBALL: u16 = 5;

/// The spells that the game makes.
pub const MADE: [u16; 1] = [FIREBALL];

/// Whether the game makes the spell `spell`.
pub fn made(spell: u16) -> bool {
    MADE.contains(&spell)
}

/// The slot of the clip that a hero on foot plays for the spell `spell` (`0x522030`): 155, 161,
/// 157 or 163 by its list, else 154.
pub fn slot(spell: u16) -> u16 {
    match spell {
        5 | 29 | 33 | 37 | 41 | 58 | 59 | 61 | 64 | 65 => 155,
        7 | 12 | 36 => 161,
        20 | 21 | 25 | 30 | 31 | 32 | 34 => 157,
        1 | 2 | 4 | 16 | 18 | 27 | 35 => 163,
        _ => 154,
    }
}

/// The share of its clip still to play below which the spell `spell` acts (`0x521D60`): 0.66
/// for the fireball, 0.56 for the stone skin (2), 0.5 for the healing (7) and 0.35 for the
/// regeneration (32); none for the others, not read.
pub fn share(spell: u16) -> Option<f32> {
    match spell {
        5 => Some(0.66),
        2 => Some(0.56),
        7 => Some(0.5),
        32 => Some(0.35),
        _ => None,
    }
}

/// How much of its clip still to play at which a cast that has acted goes back to standing, in
/// seconds (`0x521D60`'s sub-state 4).
pub const STANDS_WITHIN: f32 = 0.3;

/// Whether the spell `spell` turns its caster to its target's place as it starts (`0x51E110`):
/// the fireball alone of those read.
pub fn turns(spell: u16) -> bool {
    spell == FIREBALL
}

/// The fireball's speed in units a second at its level `level` for a hero's cast (its particle
/// system's start, `0x76BE80`): 150 + 10·L, at most 500.
pub fn fireball_speed(level: u32) -> f32 {
    (150.0 + 10.0 * to_f32(level)).min(500.0)
}

/// The fireball's most time in seconds at its level `level` (`0x76BE80`): 2 + 0.5·L.
pub fn fireball_life(level: u32) -> f32 {
    2.0 + 0.5 * to_f32(level)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spells_clip_is_its_lists_slot() {
        assert_eq!(slot(FIREBALL), 155);
        assert_eq!((slot(7), slot(32), slot(2), slot(18)), (161, 157, 163, 163));
        assert_eq!(slot(13), 154);
    }

    #[test]
    fn the_fireball_acts_at_two_thirds_of_its_clip_left_and_flies_faster_by_its_level() {
        assert_eq!(share(FIREBALL), Some(0.66));
        assert_eq!(share(13), None);
        // At level 1, 160 a second for 2.5 s, some 400; held at 500 from level 35.
        assert_eq!((fireball_speed(1), fireball_life(1)), (160.0, 2.5));
        assert_eq!((fireball_speed(35), fireball_speed(60)), (500.0, 500.0));
        assert!(made(FIREBALL) && !made(13));
    }
}
