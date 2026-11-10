//! The undead that rise again (#1199; docs/re/fight.md, "The undead that rise again"): the magic
//! skeletons (48, 80, 81) and the undead goblins (142, 143), of the `Creature.pak` class 5, rise
//! once from a strike's death (`0x52E590`, at `0x532BD6`), 4 s after it, with a half, a third or a
//! quarter of their most health (`0x53ECA0`'s cases 3 and 5); the magic skeletons drain a lower
//! killer's health as they fall; at their last death they burn away. The skeleton warriors (32,
//! 112 to 117) change their type at a strike's death as they rise, an arm or the head lost each
//! time, until their last death bursts them; the other types of the class 5 burst or vanish at
//! their death (`0x532E3C`'s and `0x541653`'s switches, #1260). Any creature's body lies a time
//! by the options' detail and vanishes, and a hidden one is destroyed later (#2501).

/// The flags word's bit of an undead that has risen once, by which its next death is its last
/// (`+0x1F4` bit `0x200`); a boss or a champion takes it at its first fight.
pub const RISEN: u32 = 0x200;

/// How long an undead lies before it rises, in seconds (`0x89098C`).
pub const LIES: f32 = 4.0;

/// How long after its death a hidden corpse is destroyed, in seconds, at the options' detail
/// `detail` (`DETAILLEVEL`, `0x553DC0`): 25 at 2, 10 at 1 and 7 at 0.
pub fn destroyed_after(detail: u8) -> f32 {
    match detail {
        0 => 7.0,
        1 => 10.0,
        _ => 25.0,
    }
}

/// How long after its death any creature's body lies before it vanishes, in seconds, at the
/// options' detail `detail` (`DETAILLEVEL`, the byte `0x182EE78` read at `0x53FB9A`; #2501): 20
/// at 2, 5 at 1 and 2 at 0; then, unless something holds it, it vanishes in its soul's sparks
/// and is destroyed at [`destroyed_after`].
pub fn vanishes_after(detail: u8) -> f32 {
    match detail {
        0 => 2.0,
        1 => 5.0,
        _ => 20.0,
    }
}

/// The medium spider, whose body vanishes at its first tick of lying, with no effect, held or not
/// (`0x540037`–`0x540485`, #2504): it would split into small spiders 71 only on the machine of the
/// player who controls it (`+0x2B5`), which no creature of a game of one player names (**seen**:
/// 0 for the world's creatures, the local player's 1).
pub const MEDIUM_SPIDER: u32 = 75;

/// How long after its death a body's first tick of lying comes, in seconds: the dying action's
/// sub-mode 1 turns into 2 at the next tick of the world (`0x53FB11`; **seen** 0.23 s for a crow).
pub const FIRST_LYING_TICK: f32 = 0.25;

/// The flags word's bit of a creature whose body lies on (`+0x1F4` bit `0x2000`, the tag `0x45`
/// of `CreateNPC` and `SetNPCState`), which neither vanishes nor is destroyed (`0x53ECA0`'s
/// lying step, #2501).
pub const LIES_ON: u32 = 0x2000;

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

/// The skeleton warrior's head (`Skeleton_warrior_Head.GRN`), an item that goes into a socket,
/// which a warrior throws off as it loses its head (`0x426DF0` of the types 32 and 112 to 117).
pub const HEAD: u32 = 5100;

/// Whether a skeleton warrior that changes from the type `from` into `to` loses its head, which
/// it throws off (`0x550580`'s part 0): the whole one into the headless, and those of one arm
/// into 114 and 117; an arm lost is a bone (`0x427650`'s part 4 of the gib set 3, the types 926
/// to 936) that does not stay in the world (**seen**, #1202).
pub fn loses_head(from: u32, to: u32) -> bool {
    matches!((from, to), (WARRIOR, HEADLESS) | (113, 114) | (116, 117))
}

/// The gib set of an undead of the type `kind` (`items.pak +0x6F`, `0x427630`), which colours its
/// burst (`particles::burst_colour`): 3 the bones of the skeleton warriors, 1 green the zombies
/// and the ghouls (33, 34, 36, 50), 2 black the mummy 44, 0 red the others (read with #1202 off
/// the table; the byte is not converted).
pub fn gib_set(kind: u32) -> u8 {
    if warrior(kind) {
        3
    } else if BURSTING.contains(&kind) {
        1
    } else if kind == 44 {
        2
    } else {
        0
    }
}

/// The `items.pak` type of the body part `part` (0 to 5) of a burst of the gib set `set`
/// (`0x427650`; #1264, #1935): the set 0's 5094, 5093, 5089 to 5093, 5086 to 5089, 5083 to 5086
/// and 5065 to 5083; the set 1's 999, 998, 994 to 998, 991 to 994, 988 to 991 and 970 to 988;
/// the set 2's 969, 968, 964 to 968, 961 to 964, 958 to 961 and 940 to 958; the set 3's 937,
/// 936, 935, 925 and, for the parts 4 and 5, 926 to 936; none for another set.
pub fn part_type(set: u8, part: u8, dice: &mut impl crate::spawning::Dice) -> Option<u32> {
    let mut roll = |count: u32| dice.below(count);
    let (first, second, rolled): (u32, u32, [(u32, u32); 4]) = match set {
        0 => (5094, 5093, [(5089, 5), (5086, 4), (5083, 4), (5065, 19)]),
        1 => (999, 998, [(994, 5), (991, 4), (988, 4), (970, 19)]),
        2 => (969, 968, [(964, 5), (961, 4), (958, 4), (940, 19)]),
        3 => match part {
            0 => return Some(937),
            1 => return Some(936),
            2 => return Some(935),
            3 => return Some(925),
            4 | 5 => return Some(926 + roll(11)),
            _ => return None,
        },
        _ => return None,
    };
    match part {
        0 => Some(first),
        1 => Some(second),
        2..=5 => {
            let (base, count) = rolled[usize::from(part - 2)];
            Some(base + roll(count))
        }
        _ => None,
    }
}

/// One of the six blood textures of the gib set `set` at random (`0x4271E0`): `FX_BLOOD01` to
/// `06` for the set 0, with `_G` for the set 1 and `_B` for the set 2; none for the sets 3 and 4.
pub fn blood_texture(set: u8, dice: &mut impl crate::spawning::Dice) -> Option<&'static str> {
    const RED: [&str; 6] = [
        "FX_BLOOD01",
        "FX_BLOOD02",
        "FX_BLOOD03",
        "FX_BLOOD04",
        "FX_BLOOD05",
        "FX_BLOOD06",
    ];
    const GREEN: [&str; 6] = [
        "FX_BLOOD01_G",
        "FX_BLOOD02_G",
        "FX_BLOOD03_G",
        "FX_BLOOD04_G",
        "FX_BLOOD05_G",
        "FX_BLOOD06_G",
    ];
    const BLACK: [&str; 6] = [
        "FX_BLOOD01_B",
        "FX_BLOOD02_B",
        "FX_BLOOD03_B",
        "FX_BLOOD04_B",
        "FX_BLOOD05_B",
        "FX_BLOOD06_B",
    ];
    let set = match set {
        0 => RED,
        1 => GREEN,
        2 => BLACK,
        _ => return None,
    };
    set.get(usize::try_from(dice.below(6)).unwrap_or(0))
        .copied()
}

/// The blood decal of a burst, and of each bounce of its body parts: its seconds before it fades
/// and its half side (`0x4E8DA0`'s `0x3C002D`; docs/re/effects.md, "A soul's mark").
pub const BURST_BLOOD: (f32, f32) = (60.0, 45.0);
pub const PART_BLOOD: (f32, f32) = (30.0, 25.0);

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
/// art's, a shot's or a throw's); 0 where the killer is `banished`, under a purple potion's effect
/// (the effect 33 of the whole value 4, its `+0xB0` bit `0x80000`, at `0x532C48`;
/// `potions::banishes`). The killer's set pieces cut it after (`set_bonuses::Stats::rising`,
/// #1727); the halvings by the record's second damage and `+0x2B7` are not followed (docs/re/
/// fight.md).
pub fn chance(plain: bool, banished: bool) -> u32 {
    match (banished, plain) {
        (true, _) => 0,
        (false, true) => 90,
        (false, false) => 45,
    }
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

/// The effect of the exorcism (`0x35F`), whose value the killer's items and sets give.
pub const EXORCISM: u32 = 863;
/// The faces of the exorcism's roll, `R(0, 99)` ([`exorcised`]).
pub const EXORCISM_FACES: u32 = 100;

/// Whether the killer's exorcism of the value `value` (its items' effect 863 and its sets',
/// `set_bonuses::Stats::exorcist`) lays an undead to rest for good at its death, of the balance's
/// parameters `params` (`0x532DEE`): `R(0, 99)`, `roll`, below its chance
/// (`fight::rare_chance`, `0x590570`); then it neither rises nor changes its type.
pub fn exorcised(value: f32, params: [f32; 3], roll: u32) -> bool {
    f64::from(roll) < crate::fight::rare_chance(value, params)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_exorcism_lays_the_undead_to_rest_below_its_chance() {
        // A value of 10 of (100, 100, 98): 100·10/110 + 0.5, the rolls 0 to 9.
        let params = [100.0, 100.0, 98.0];
        assert!(exorcised(10.0, params, 9) && !exorcised(10.0, params, 10));
        assert!(!exorcised(0.0, params, 0));
    }

    #[test]
    fn the_undead_rise_once_at_their_chance() {
        assert!(rises(48) && rises(143) && !rises(73) && !rises(32));
        assert!(burns_away(73) && burns_away(80));
        assert_eq!((chance(true, false), chance(false, false)), (90, 45));
        assert!(rolls(89, 90) && !rolls(90, 90));
        // A killer under a purple potion: none rises, whatever the roll.
        assert_eq!((chance(true, true), chance(false, true)), (0, 0));
        assert!(!rolls(0, chance(true, true)));
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
        // The changes that take the head throw it off; the arms' do not.
        assert!(loses_head(32, 112) && loses_head(113, 114) && loses_head(116, 117));
        assert!(!loses_head(32, 113) && !loses_head(112, 114) && !loses_head(114, 115));
    }

    #[test]
    fn the_class_5_bursts_burns_or_vanishes_at_its_last_death() {
        // The burst's colour by the gib set: the warriors' bones, the zombies' green.
        assert_eq!([32, 115, 36, 44, 48].map(gib_set), [3, 3, 1, 2, 0]);
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
    fn a_body_lies_and_is_destroyed_by_the_detail() {
        // Seen at the detail 2: the goblins', the thieves' and the wolves' bodies vanished at
        // 20.1 s and were destroyed at 25.0 s (#2501).
        assert_eq!([0, 1, 2].map(vanishes_after), [2.0, 5.0, 20.0]);
        assert_eq!([0, 1, 2].map(destroyed_after), [7.0, 10.0, 25.0]);
        assert!((0..=2).all(|detail| vanishes_after(detail) < destroyed_after(detail)));
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
