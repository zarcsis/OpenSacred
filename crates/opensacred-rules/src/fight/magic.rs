//! The item maker's magic bonus (`0x5D4CE0`, docs/re/items.md, "The magic bonus"; #2265): one
//! bonus of a template's entries, the first whose chance passes its roll, its effect the entry's
//! or one of a list's, its value by the entry's least and most, the difficulty and the effect's
//! record, or for the spells and the skills by the level.

use super::{Bonus, Difficulty};
use crate::spawning::Dice;

/// The lists that an entry's effect is drawn from (`0x5E2310` for 1 to 13, the skills as the
/// effects `599 + skill`; `0x5E3800` for 14 to 34, the spells and the combat arts), in the
/// order the original builds them, which its draw `rand() % count` reads; any other list empty.
pub(crate) const LISTS: [&[u32]; 35] = [
    &[],
    &[
        601, 613, 624, 611, 626, 619, 600, 614, 616, 615, 617, 625, 618, 627, 604, 605, 623, 606,
        603, 602, 612, 609, 610, 607, 608, 622, 621, 620,
    ],
    &[601, 624, 622],
    &[609, 610, 607, 608],
    &[604, 605, 623, 606, 603, 602, 612],
    &[621, 620],
    &[601, 624, 609, 610, 607, 608],
    &[613, 611, 614, 616, 615, 617],
    &[601, 626, 625, 612, 607, 622],
    &[601, 624, 606, 613, 618, 607],
    &[601, 624, 605, 613, 600, 609],
    &[601, 624, 619, 627, 610, 608],
    &[601],
    &[601],
    &[
        36, 38, 39, 40, 41, 42, 37, 3, 1019, 21, 20, 19, 17, 18, 15, 13, 22, 5, 24, 23, 11, 2, 25,
        16, 26, 28, 29, 1, 33, 27, 34, 35, 4, 7, 30, 31, 32,
    ],
    &[1001, 1002, 1000, 1009],
    &[
        1003, 1013, 1012, 1010, 1014, 1015, 1016, 1001, 1002, 1000, 1009,
    ],
    &[1019, 21, 20, 19, 17, 18, 15, 13, 22],
    &[1001, 1002, 1009, 1020, 1054, 1017, 1069, 1018],
    &[36, 38, 39, 40, 41, 42, 37, 3],
    &[1006, 1007, 1008, 1004, 1005, 1000, 1001, 1002],
    &[1042, 1043, 1044, 1045, 1046, 1068, 1065, 1066, 1067],
    &[1051, 1052, 1053, 1048, 1049, 1047, 1050],
    &[1030, 1031, 1033, 1032, 1034, 1035, 1036, 1037],
    &[
        1022, 1024, 1026, 1028, 1025, 1027, 1029, 1000, 1001, 1002, 1009,
    ],
    &[
        5, 24, 23, 11, 2, 25, 16, 26, 28, 29, 1, 33, 27, 34, 35, 4, 7, 30, 31, 32,
    ],
    &[5, 24, 23, 11],
    &[2, 25, 16, 26],
    &[27, 34, 35, 4],
    &[28, 29, 1, 33],
    &[7, 30, 31, 32],
    &[1078, 1079, 1080, 1082, 1083, 1084, 1089],
    &[1087, 1086, 1085, 55, 56, 57, 58, 59],
    &[1094, 1097, 1098, 1099, 1100],
    &[60, 62, 63, 64, 65, 66, 67],
];

/// The hero type whose spell or combat art `art` is (`0x52DBA0`): the Seraphim's 1, the
/// Gladiator's 2, the Mage's 3, the Dark Elf's 4, the Wood Elf's 5, the Vampiress's 6 and 7 by
/// her forms, the Dwarf's 8 and the Daemon's 9; 0 for none, a skill's effect among them.
pub(crate) fn art_hero(art: u32) -> u8 {
    match art {
        1 | 2 | 4 | 5 | 7 | 11 | 16 | 23..=35 => 3,
        3 | 36..=42 | 1004..=1008 | 1061..=1063 => 5,
        13 | 15 | 17..=22 | 1017..=1020 | 1054 | 1069..=1072 => 1,
        55..=59 | 1078..=1080 | 1082..=1089 => 8,
        60..=67 | 1090..=1102 => 9,
        1003 | 1010 | 1012..=1016 | 1057..=1059 => 2,
        1022 | 1024 | 1026 | 1028 | 1073..=1076 => 6,
        1023 | 1025 | 1027 | 1029..=1041 => 7,
        1042..=1053 | 1065..=1068 => 4,
        _ => 0,
    }
}

/// The float 0.01 of the value by the level (`0x890CB0`, a little below a hundredth).
const HUNDREDTH: f32 = 0.01;

/// The least value whose record's third float makes the value by the level (`0x890C9C`).
const BY_LEVEL: f32 = 1.192_092_9e-7;

/// The bonus of the entries `entries` of an item's template made at `level` on `difficulty`
/// (`0x5D4CE0`), the effects' records `norms` its caps, the class byte for an item of every
/// class (`every`): the entries tried in order, each by its roll `R(0, 99)` below its chance; the
/// first that holds gives its condition and its effect, or the effect of its list drawn by
/// `rand() % count` (none for a list of one), an art's hero type into the condition's bits 8 to
/// 15 where `every`; its value `R(least, most)` times the difficulty's factor, held by the
/// effect's record ([`value`]), the effect 820 of the value 1 for an entry of no least or most
/// and no divisors; then an entry whose own effect is a skill's (600 to 798) takes the level's
/// `⌊(level + 9)/d₂⌋` and one of a spell's or none (up to 200, the lists' among them)
/// `⌊(level + 9)/d₁⌋`, at least 1, its divisors 10 and 20 where 0. None where no entry holds.
pub(super) fn bonus(
    entries: &[[u32; 6]],
    norms: &[[f32; 6]],
    (level, difficulty, every): (u32, Difficulty, bool),
    rng: &mut impl Dice,
) -> Option<Bonus> {
    let entry = entries
        .iter()
        .find(|entry| rng.below(100) < entry[0] & 0xFFFF)?;
    let [head, own, ends, list, first, second] = *entry;
    let mut condition = u16::try_from(head >> 16).unwrap_or(0);
    let mut effect = own;
    if list != 0 {
        let drawn = usize::try_from(list)
            .ok()
            .and_then(|list| LISTS.get(list))
            .copied()
            .unwrap_or(&[]);
        effect = match drawn.len() {
            0 => 0,
            1 => drawn[0],
            count => {
                drawn[usize::try_from(rng.below(u32::try_from(count).unwrap_or(1))).unwrap_or(0)]
            }
        };
        let hero = art_hero(effect);
        if every && hero != 0 {
            condition |= u16::from(hero) << 8;
        }
    }
    let [least, most] = [ends & 0xFFFF, ends >> 16]
        .map(|half| i16::from_le_bytes(u16::try_from(half).unwrap_or(0).to_le_bytes()));
    let rolled = rolled(least, most, rng);
    let factor = crate::set_bonuses::factor(difficulty);
    let record = crate::drops::norm(norms, effect & 0xFFFF).copied();
    let plain = (least == 0 || most == 0) && first == 0 && second == 0;
    let (effect, v) = if plain {
        (820, 1.0)
    } else {
        (effect, single(rolled * f64::from(factor)))
    };
    let mut whole = value(effect, v, (record, factor), level);
    let own = own & 0xFFFF;
    let by_level = |divisor: u32, default: u32| {
        let divisor = if divisor == 0 { default } else { divisor };
        ((level + 9) / divisor).max(1)
    };
    if (600..=798).contains(&own) {
        whole = by_level(second, 10);
    } else if own <= 200 {
        whole = by_level(first, 20);
    }
    Some(Bonus {
        condition,
        effect,
        value: i16::from_le_bytes(u16::try_from(whole & 0xFFFF).unwrap_or(0).to_le_bytes()),
    })
}

/// The entry's roll of its value (`0x5D4E8D`): its ends as they are where equal, else a negative
/// least 0 and a negative most 1, then `rand() % (high − low + 1) + low` where they still differ.
fn rolled(least: i16, most: i16, rng: &mut impl Dice) -> f64 {
    if least == most {
        return f64::from(least);
    }
    let (a, b) = (least.max(0), if most < 0 { 1 } else { most });
    if a == b {
        return f64::from(b);
    }
    let (low, high) = (a.min(b), a.max(b));
    let span = u32::try_from(i32::from(high) - i32::from(low) + 1).unwrap_or(1);
    f64::from(i32::from(low) + i32::try_from(rng.below(span)).unwrap_or(0))
}

/// `x` stored as a float, as the x87 unit stores it.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the original stores the product as a float"
)]
fn single(x: f64) -> f32 {
    x as f32
}

/// The value of the effect `effect` of the rolled `v` (`0x5D4EE3`–`0x5D4FE1`), by the record of
/// the entry's effect and the difficulty's `factor`: an effect word with a high half (an
/// attribute that scales it) at most the record's first float times the factor; without, where
/// the record's third float is below `2⁻²³`, at most its fourth float times the factor, else
/// `(level + 9)·factor·v·0.01`; at least 1, truncated. Without a record, `v` itself.
fn value(effect: u32, v: f32, (record, factor): (Option<[f32; 6]>, f32), level: u32) -> u32 {
    let factor = f64::from(factor);
    let v64 = f64::from(v);
    let x = match record {
        None => v64,
        Some(record) if effect & 0xFFFF_0000 != 0 => {
            let cap = f64::from(record[0]) * factor;
            if v64 > cap {
                f64::from(single(cap))
            } else {
                v64
            }
        }
        Some(record) if record[2] < BY_LEVEL => (f64::from(record[3]) * factor).min(v64),
        Some(_) => f64::from(level + 9) * factor * v64 * f64::from(HUNDREDTH),
    };
    whole(x.max(1.0))
}

/// `x` truncated toward zero (`_ftol`), held within a `u32`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a bonus's value, at least 1 and within a short"
)]
fn whole(x: f64) -> u32 {
    x.clamp(0.0, f64::from(u32::MAX)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spawning::Rng;

    #[test]
    fn the_arts_lists_nest_as_the_originals() {
        let joined = |lists: &[usize]| -> Vec<u32> {
            lists
                .iter()
                .flat_map(|&list| LISTS[list].iter().copied())
                .collect()
        };
        assert_eq!(LISTS[14], joined(&[19, 17, 25]).as_slice());
        assert_eq!(LISTS[25], joined(&[26, 27, 29, 28, 30]).as_slice());
        let mut skills: Vec<u32> = LISTS[1].to_vec();
        skills.sort_unstable();
        assert_eq!(skills, (600..628).collect::<Vec<_>>());
        assert_eq!((art_hero(1043), art_hero(1000), art_hero(612)), (4, 0, 0));
    }

    #[test]
    fn an_entry_gives_its_bonus_by_its_chance_and_its_effects_record() {
        // An entry of 801 scaled by the attribute 2, 2 to 5, a chance of 100; the record's first
        // float 3 holds it at 3 on bronze and at 3 times 2.34 on niobium.
        let mut norms = vec![[0.0; 6]; 74];
        norms[1] = [3.0, 0.0, 1.0, 0.0, 0.0, 0.0];
        let entry = [100, 801 | 2 << 16, 2 | 5 << 16, 0, 20, 10];
        let mut rng = Rng::of(1, 2, 3);
        for _ in 0..50 {
            let made = bonus(&[entry], &norms, (10, Difficulty::Bronze, false), &mut rng).unwrap();
            assert_eq!((made.condition, made.effect), (0, 801 | 2 << 16));
            assert!((2..=3).contains(&made.value), "{made:?}");
        }
        let niobium = (0..50)
            .map(|_| bonus(&[entry], &norms, (10, Difficulty::Niobium, false), &mut rng).unwrap())
            .map(|made| made.value)
            .max();
        assert_eq!(niobium, Some(7));
        // A chance of 0 never holds; a skill's list gives `(level + 9)/20`; an art of a list
        // takes its hero's type into the condition for an item of every class.
        assert_eq!(
            bonus(
                &[[0, 801, 2 | 5 << 16, 0, 0, 0]],
                &norms,
                (10, Difficulty::Bronze, false),
                &mut rng
            ),
            None
        );
        let skill = bonus(
            &[[100, 0, 1 | 3 << 16, 2, 20, 10]],
            &norms,
            (31, Difficulty::Bronze, false),
            &mut rng,
        )
        .unwrap();
        assert!(
            [601, 624, 622].contains(&skill.effect) && skill.value == 2,
            "{skill:?}"
        );
        let art = bonus(
            &[[100, 0, 1 | 3 << 16, 21, 20, 10]],
            &norms,
            (11, Difficulty::Bronze, true),
            &mut rng,
        )
        .unwrap();
        assert_eq!((art.condition >> 8, art.value), (4, 1));
    }
}
