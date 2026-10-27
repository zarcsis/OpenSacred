//! The undead that rise again (#1199; docs/re/fight.md, "The undead that rise again"): the magic
//! skeletons (48, 80, 81) and the undead goblins (142, 143), of the `Creature.pak` class 5, rise
//! once from a strike's death (`0x52E590`, at `0x532BD6`), 4 s after it, with a half, a third or a
//! quarter of their most health (`0x53ECA0`'s cases 3 and 5); the magic skeletons drain a lower
//! killer's health as they fall; at their last death they burn away.

/// The flags word's bit of an undead that has risen once, by which its next death is its last
/// (`+0x1F4` bit `0x200`); a boss or a champion takes it at its first fight.
pub const RISEN: u32 = 0x200;

/// How long an undead lies before it rises, in seconds (`0x89098C`).
pub const LIES: f32 = 4.0;

/// The magic skeletons, which drain their killer.
const SKELETONS: [u32; 3] = [48, 80, 81];
/// The undead goblins.
const GOBLINS: [u32; 2] = [142, 143];

/// Whether a creature of the type `kind` may rise from its death.
pub fn rises(kind: u32) -> bool {
    SKELETONS.contains(&kind) || GOBLINS.contains(&kind)
}

/// Whether a creature of the type `kind` burns away at its last death (`0x53ECA0`'s cases 3, 4
/// and 5): the undead that rise and the fire ghoul.
pub fn burns_away(kind: u32) -> bool {
    rises(kind) || kind == 73
}

/// The chance in per cent that an undead rises from a strike's death (`0x52E590`): 90, or 45 for
/// a hit of another kind than a plain strike (the strike record's `+0x14`; **hypothesis**: an
/// art's, a shot's or a throw's); the halvings by the record's second damage and `+0x2B7`, the
/// killer's effect 33 and its set pieces are not followed (docs/re/fight.md).
pub fn chance(plain: bool) -> u32 {
    if plain { 90 } else { 45 }
}

/// Whether the roll `roll` of `R(0, 100)` (`0..=100`) makes an undead rise at the chance `chance`.
pub fn rolls(roll: u32, chance: u32) -> bool {
    roll < chance
}

/// The health that a magic skeleton of the level `level` dying at the hand of a killer of the
/// level `killer` and the health `health` takes from it (`0x52E590` at `0x532E8D`): none for a
/// killer of its level or above or of 10 health or less, else `⌊health · g⌋` with
/// `g = min((level + 20) / (killer + 20) − 1, 0.5)`; none for the other undead.
pub fn drained(kind: u32, level: u32, killer: u32, health: u32) -> u32 {
    if !SKELETONS.contains(&kind) || killer >= level || health <= 10 {
        return 0;
    }
    let share = ((f64::from(level) + 20.0) / (f64::from(killer) + 20.0) - 1.0).min(0.5);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a share of a health, whole and not negative, below the health"
    )]
    let drained = (f64::from(health) * share).floor() as u32;
    drained
}

/// The health an undead of the most health `most` rises with, the roll `roll` of `rand() % 3`:
/// its most over 2, 3 or 4.
pub fn risen_health(most: u32, roll: u32) -> u32 {
    most / (roll % 3 + 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_undead_rise_once_at_their_chance() {
        assert!(rises(48) && rises(143) && !rises(73) && !rises(32));
        assert!(burns_away(73) && burns_away(80));
        assert_eq!((chance(true), chance(false)), (90, 45));
        assert!(rolls(89, 90) && !rolls(90, 90));
        // Seen: 139 of most health rose with 34, 69 and 46.
        assert_eq!([0, 1, 2].map(|roll| risen_health(139, roll)), [69, 46, 34]);
    }

    #[test]
    fn a_magic_skeleton_drains_a_lower_killer() {
        // A hero of level 1 at 119 health, a skeleton of level 5: (25/21 − 1) of it.
        assert_eq!(drained(48, 5, 1, 119), 22);
        assert_eq!(drained(48, 5, 5, 119), 0);
        assert_eq!(drained(142, 5, 1, 119), 0);
        assert_eq!(drained(48, 100, 1, 119), 59);
        assert_eq!(drained(48, 5, 1, 10), 0);
    }
}
