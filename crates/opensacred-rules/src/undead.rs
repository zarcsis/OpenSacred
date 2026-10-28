//! The undead that rise again (#1199; docs/re/fight.md, "The undead that rise again"): the magic
//! skeletons (48, 80, 81) and the undead goblins (142, 143), of the `Creature.pak` class 5, rise
//! once from a strike's death (`0x52E590`, at `0x532BD6`), 4 s after it, with a half, a third or a
//! quarter of their most health (`0x53ECA0`'s cases 3 and 5); the magic skeletons drain a lower
//! killer's health as they fall; at their last death they burn away. The skeleton warriors (32,
//! 112 to 117) change their type at a strike's death as they rise, an arm or the head lost each
//! time, until their last death bursts them; the other types of the class 5 burst or vanish at
//! their death (`0x532E3C`'s and `0x541653`'s switches, #1260).

/// The flags word's bit of an undead that has risen once, by which its next death is its last
/// (`+0x1F4` bit `0x200`); a boss or a champion takes it at its first fight.
pub const RISEN: u32 = 0x200;

/// How long an undead lies before it rises, in seconds (`0x89098C`).
pub const LIES: f32 = 4.0;

/// The magic skeletons, which drain their killer.
const SKELETONS: [u32; 3] = [48, 80, 81];
/// The undead goblins.
const GOBLINS: [u32; 2] = [142, 143];

/// The `Creature.pak` class of the undead, whose deaths these are (`0x52E590`'s switch).
pub const CLASS: u8 = 5;

/// The skeleton warrior, whole (`SKEL_WARRIOR.GRN`), and its types without a part: the head
/// (112), the right arm (113), the right arm and the head (114), both arms and the head (115),
/// the left arm (116), the left arm and the head (117).
const WARRIOR: u32 = 32;
const HEADLESS: u32 = 112;

/// The types that burst at their last death (`0x541653`'s cases 0 to 2): the skeleton warriors,
/// the zombies 33 and 34, and the ghouls 36 and 50.
const BURSTING: [u32; 4] = [33, 34, 36, 50];

/// The fire ghoul, which burns away once its dying clip has played (`0x541653`'s case 4); not of
/// the class 5.
pub const FIRE_GHOUL: u32 = 73;

/// What becomes of a creature of the class 5 at its last death (`0x541653`'s cases, #1260): it
/// plays no dying clip, its most health set to 0 (`0x53F31D`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// It bursts at once (`0x4E8DA0`), with `CHAR_SPLASH`.
    Bursts,
    /// It burns away a moment later (the cases 3 and 5).
    Burns,
    /// It vanishes at once (the case 6, a 781 of the variant 4).
    Vanishes,
}

/// What becomes of a creature of the class 5 and the type `kind` at its last death.
pub fn end(kind: u32) -> End {
    if rises(kind) {
        End::Burns
    } else if warrior(kind) || BURSTING.contains(&kind) {
        End::Bursts
    } else {
        End::Vanishes
    }
}

/// Whether the type `kind` is a skeleton warrior, whole or without a part.
pub fn warrior(kind: u32) -> bool {
    kind == WARRIOR || (HEADLESS..=117).contains(&kind)
}

/// The type that a skeleton warrior of the type `kind` changes into as it rises from a strike's
/// death (`0x532E3C`'s cases 0 and 2 to 5), by two draws of `rand() % 3`, `draws`: the whole
/// warrior loses its right arm (113) at a first draw of 0, else its left (116) at a second of 0,
/// else its head (112), a third, two ninths and four ninths of the time; the headless its right
/// or its left arm at even odds, the first draw's odd or even (114, 117); one armless its head
/// (114, 117); one of an arm and no head its other arm (115); none for the last, 115, whose death
/// is its last (**hypothesis** for how the original draws: the shares are read, `0x53305B`,
/// `0x53317B`).
pub fn changes(kind: u32, draws: [u32; 2]) -> Option<u32> {
    match kind {
        WARRIOR if draws[0] == 0 => Some(113),
        WARRIOR if draws[1] == 0 => Some(116),
        WARRIOR => Some(HEADLESS),
        HEADLESS if draws[0].is_multiple_of(2) => Some(114),
        HEADLESS => Some(117),
        113 => Some(114),
        116 => Some(117),
        114 | 117 => Some(115),
        _ => None,
    }
}

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
    fn a_skeleton_warrior_loses_a_part_at_each_death_until_its_last() {
        // The whole warrior: its right arm, its left, or its head.
        assert_eq!(changes(32, [0, 2]), Some(113));
        assert_eq!(changes(32, [1, 0]), Some(116));
        assert_eq!(changes(32, [2, 1]), Some(112));
        // The headless an arm, an armless its head, then the other arm, and 115's is the last.
        assert_eq!(changes(112, [0, 0]), Some(114));
        assert_eq!(changes(112, [1, 0]), Some(117));
        assert_eq!(changes(113, [2, 2]), Some(114));
        assert_eq!(changes(116, [2, 2]), Some(117));
        assert_eq!(changes(114, [0, 0]), Some(115));
        assert_eq!(changes(117, [0, 0]), Some(115));
        assert_eq!(changes(115, [0, 0]), None);
        assert_eq!(changes(48, [0, 0]), None);
        assert!(warrior(32) && warrior(115) && !warrior(131) && !warrior(118));
    }

    #[test]
    fn the_class_5_bursts_burns_or_vanishes_at_its_last_death() {
        assert_eq!(end(32), End::Bursts);
        assert_eq!(end(115), End::Bursts);
        assert_eq!(end(36), End::Bursts);
        assert_eq!(end(33), End::Bursts);
        assert_eq!(end(80), End::Burns);
        assert_eq!(end(143), End::Burns);
        // A mummy, a pirate and the skeleton warrior 131 of another model vanish.
        assert_eq!(end(44), End::Vanishes);
        assert_eq!(end(178), End::Vanishes);
        assert_eq!(end(131), End::Vanishes);
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
