//! A quest's reward, the scripts' `Gewinn` (125, `0x4AC940`; docs/re/netscript.md, "The
//! rewards"): up to three numbers, its flags, a value and a step of 0 to 5. The flag 1 gives
//! gold, `(G[s]·v + 100)·(H/2 + 1)` for the hero's level `H`; the flag 2 experience, the middle
//! of the hero's sector's levels `m` making `⌊(m + 2)·(E[s]·v + 100)/2⌋` times the difficulty's
//! factor, 1, 2, 10, 20 or 30, times the decay of a high hero; the flags 4 to `0x40` an item, one
//! at most ([`Prize::item`], #1386). The end of a side quest gives a rune of the hero's class by
//! a chance of its own (`0x45FC60`, `drops::Rules::quest_rune_range`, [`class_rune`]).

use crate::drops::{self, Catalogue, Difficulty, Hero, Kind, Wanted};
use crate::fight;
use crate::spawning::Dice;

/// The reward's flags: gold and experience.
pub const GOLD: u32 = 0x1;
pub const EXPERIENCE: u32 = 0x2;
/// The flags of its items: weapons, rings and amulets, armour, a shield, the hero's own (`0x7C`
/// tested, `0x40` its class's item).
pub const ITEMS: u32 = 0x7C;

/// Gold's and experience's numbers by the reward's step (`0x4ACE79`…).
const GOLD_STEPS: [i64; 6] = [5, 8, 12, 20, 4, 25];
const EXPERIENCE_STEPS: [i64; 6] = [10, 8, 12, 8, 4, 15];
/// The experience's factor by the difficulty, bronze to niobium (`0x4ACDFB`).
const EXPERIENCE_FACTORS: [f64; 5] = [1.0, 2.0, 10.0, 20.0, 30.0];

/// A reward's numbers, as its record gives them (the tags `0x0B` and `0x9F`, the first three).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Prize {
    pub flags: u32,
    pub value: i32,
    pub step: i32,
}

impl Prize {
    /// The reward of the numbers `numbers`, those missing 0.
    pub fn of(numbers: &[i32]) -> Self {
        let at = |i: usize| numbers.get(i).copied().unwrap_or(0);
        Self {
            flags: u32::from_ne_bytes(at(0).to_ne_bytes()),
            value: at(1),
            step: at(2),
        }
    }

    /// Its step, held to 0 to 5.
    fn step(self) -> usize {
        usize::try_from(self.step.clamp(0, 5)).unwrap_or(0)
    }

    /// The gold it gives a hero of level `hero`, none without its flag 1:
    /// `(G[s]·v + 100)·(H/2 + 1)`.
    pub fn gold(self, hero: u32) -> Option<i64> {
        (self.flags & GOLD != 0).then(|| {
            (GOLD_STEPS[self.step()] * i64::from(self.value) + 100) * (i64::from(hero / 2) + 1)
        })
    }

    /// The experience it gives a hero of level `hero` in a sector whose levels' middle is
    /// `middle` (0 for a sector without levels), on `difficulty`, none without its flag 2:
    /// `⌊(m + 2)·(E[s]·v + 100)/2⌋` times the difficulty's factor, truncated, times the decay of
    /// a high hero (`0x57E050`), truncated; none below 0.
    pub fn experience(self, middle: i64, hero: u32, difficulty: Difficulty) -> Option<u64> {
        if self.flags & EXPERIENCE == 0 {
            return None;
        }
        let base = EXPERIENCE_STEPS[self.step()] * i64::from(self.value) + 100;
        let points = (middle + 2) * base / 2;
        let factor = EXPERIENCE_FACTORS[difficulty.index()];
        let scaled = whole(lossless(points) * factor);
        let decayed = whole(f64::from(fight::decay(hero)) * lossless(scaled));
        Some(u64::try_from(decayed).unwrap_or(0))
    }
}

/// The categories of a reward's items by their flags, in the order the reward tries them
/// (`0x4AD5CB`…): the flag 4 one of nine, 8 a weapon or one of 13, `0x10` one of ten, `0x20`
/// the rings' and amulets' 8.
const ITEM_CATEGORIES: [(u32, &[u8]); 4] = [
    (0x04, &[7, 8, 9, 15, 16, 20, 29, 30, 31]),
    (0x08, &[5, 13]),
    (0x10, &[6, 13, 17, 18, 19, 21, 22, 23, 24, 25]),
    (0x20, &[8]),
];
/// The flag of the hero's class's item, a rune of its class ([`class_rune`]).
const CLASS_ITEM: u32 = 0x40;
/// The steps' numbers of the items' quality (`0x4ACE79`…, the table read at `0x4AD47D`).
const QUALITY_STEPS: [f64; 6] = [50.0, 100.0, 80.0, 200.0, 500.0, 800.0];
/// The items' factor by the difficulty, bronze to niobium (`0x4ACDD6`…).
const ITEM_FACTORS: [f32; 5] = [1.0, 1.4, 1.9, 2.9, 4.2];

impl Prize {
    /// The quality that its items are asked of (`0x4AD461`): `S` its step's number, a key rolled
    /// on the drops' weights ([`drops::Rules::key`]) with the bonus `⌊S·F/10⌋`, `F` the
    /// difficulty's items' factor, and the factor `g` of the share `S/400`
    /// ([`drops::Rules::spread`]); `16 − key` for the keys 2 to 15, else 0.
    pub fn quality(
        self,
        difficulty: Difficulty,
        rules: &drops::Rules,
        dice: &mut impl Dice,
    ) -> i64 {
        let steps = QUALITY_STEPS[self.step()];
        let factor = f64::from(ITEM_FACTORS[difficulty.index()]);
        let bonus = u32::try_from(whole(steps * factor * f64::from(0.1_f32))).unwrap_or(0);
        let g = rules.spread(steps * f64::from(0.0025_f32));
        match rules.key(bonus, g, dice) {
            key @ 2..=15 => 16 - i64::from(key),
            _ => 0,
        }
    }

    /// Its item (`0x4AC940`; #1386), none without its flags 4 to `0x40`: for each of the flags
    /// 4, 8, `0x10` and `0x20`, a category of its own at random and a type of it
    /// ([`Catalogue::choose`]) at its value plus 0 to 3, of the hero's class, of the quality
    /// `quality` ([`Prize::quality`]), the last made the reward's; with the flag `0x40`, or none
    /// made, a rune of the hero's class ([`class_rune`]), which counts among the class's runes
    /// given (`0x424690`).
    pub fn item(
        self,
        catalogue: &Catalogue,
        hero: &Hero,
        quality: i64,
        dice: &mut impl Dice,
    ) -> Option<RewardItem> {
        if self.flags & ITEMS == 0 {
            return None;
        }
        let mut made = None;
        for (flag, categories) in ITEM_CATEGORIES {
            if self.flags & flag == 0 {
                continue;
            }
            let len = u32::try_from(categories.len()).unwrap_or(1);
            let category = categories[usize::try_from(dice.below(len)).unwrap_or(0)];
            let level = u32::try_from(self.value.max(0)).unwrap_or(0) + dice.below(4);
            let wanted = Wanted {
                category,
                level,
                class: hero.creature,
                loose: false,
                quality,
                kind: None,
            };
            made = catalogue.choose(wanted, hero, dice);
        }
        if self.flags & CLASS_ITEM != 0 || made.is_none() {
            return class_rune(hero.creature, catalogue, dice).map(|item| RewardItem {
                item,
                class_rune: true,
            });
        }
        made.map(|item| RewardItem {
            item,
            class_rune: false,
        })
    }
}

/// A reward's item: its type, and whether it is a rune of the hero's class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RewardItem {
    pub item: u32,
    pub class_rune: bool,
}

/// The runes of each hero type's class that `0x45F410` draws from, the types 1 to 9, the
/// Vampiress's twice.
const CLASS_RUNES: [&[u32]; 9] = [
    &[
        5387, 5391, 5393, 5394, 5395, 5398, 5409, 5569, 5570, 5571, 5572, 5573, 5611, 5612, 5613,
        5614,
    ],
    &[
        5553, 5554, 5555, 5556, 5615, 5616, 5617, 5618, 5619, 5621, 5622, 5628,
    ],
    &[
        5375, 5376, 5377, 5378, 5379, 5381, 5383, 5385, 5392, 5397, 5399, 5400, 5401, 5402, 5404,
        5405, 5406, 5407, 5408, 5410, 5411,
    ],
    &[
        5417, 5565, 5566, 5567, 5568, 5576, 5577, 5578, 5579, 5580, 5581, 5582, 5583, 5584, 5585,
        5586, 5587,
    ],
    &[
        5403, 5412, 5413, 5414, 5415, 5416, 5418, 5561, 5562, 5563, 5564, 5623, 5624, 5625, 5626,
        5627,
    ],
    &[
        5592, 5593, 5594, 5595, 5596, 5597, 5598, 5600, 5602, 5604, 5606, 5609, 5557, 5558, 5559,
        5560,
    ],
    &[
        5592, 5593, 5594, 5595, 5596, 5597, 5598, 5600, 5602, 5604, 5606, 5609, 5557, 5558, 5559,
        5560,
    ],
    &[
        5767, 5768, 5769, 5770, 5771, 5772, 5773, 5774, 5775, 5776, 5777, 5778, 5779, 5780, 5781,
        5782,
    ],
    &[
        5634, 5635, 5636, 5637, 5638, 5639, 5640, 5641, 5642, 5643, 5644, 5645, 5646,
    ],
];
/// The draws `0x45F410` makes at most before it takes the last.
const CLASS_RUNE_DRAWS: u32 = 5;

/// A rune of the class of the hero type `class` (`0x45F410`): one of its class's at random,
/// drawn again while it is no rune of the catalogue, five draws at most; none for another type.
pub fn class_rune(class: u32, catalogue: &Catalogue, dice: &mut impl Dice) -> Option<u32> {
    let runes = CLASS_RUNES.get(usize::try_from(class.checked_sub(1)?).ok()?)?;
    let len = u32::try_from(runes.len()).ok()?;
    let mut rune = None;
    for _ in 0..CLASS_RUNE_DRAWS {
        let drawn = runes[usize::try_from(dice.below(len)).ok()?];
        rune = Some(drawn);
        if catalogue.kind(drawn) == Some(Kind::Rune) {
            break;
        }
    }
    rune
}

/// A whole number of points as a double, held within an `i32` as the original's are.
fn lossless(points: i64) -> f64 {
    f64::from(i32::try_from(points).unwrap_or(if points < 0 { i32::MIN } else { i32::MAX }))
}

/// A double truncated toward zero, as the C library's `__ftol` gives it.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a reward's points, held within an i32 before"
)]
fn whole(value: f64) -> i64 {
    value.trunc() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gold_grows_with_the_value_the_step_and_the_heros_level() {
        // `gewinn { _0B = { 1, 1, 3 } }`: (20 + 100) · (5/2 + 1) at level 5.
        assert_eq!(Prize::of(&[1, 1, 3]).gold(5), Some(360));
        // The step held to 5; no gold without the flag 1.
        assert_eq!(Prize::of(&[1, 2, 9]).gold(1), Some(150));
        assert_eq!(Prize::of(&[2, 1, 3]).gold(5), None);
        assert_eq!(Prize::of(&[]).gold(5), None);
    }

    /// Dice that give the numbers of `rolls` in turn, 0 after them.
    struct Rolls(Vec<u32>);

    impl Dice for Rolls {
        fn below(&mut self, n: u32) -> u32 {
            if self.0.is_empty() {
                0
            } else {
                self.0.remove(0).min(n.saturating_sub(1))
            }
        }
    }

    #[test]
    fn a_reward_without_its_items_flags_makes_none_and_its_class_item_is_a_class_rune() {
        let catalogue = Catalogue::default();
        let hero = Hero {
            creature: 1,
            level: 1,
            ..Hero::default()
        };
        assert_eq!(
            Prize::of(&[3, 5, 1]).item(&catalogue, &hero, 0, &mut Rolls(vec![])),
            None
        );
        // The flag 0x40 alone: the Seraphim's runes, the fourth drawn (none is a rune of an empty
        // catalogue, so the fifth draw stands).
        let rune =
            Prize::of(&[0x40, 5, 1]).item(&catalogue, &hero, 0, &mut Rolls(vec![0, 0, 0, 0, 3]));
        assert_eq!(
            rune,
            Some(RewardItem {
                item: 5394,
                class_rune: true
            })
        );
        // An item flag whose category has no type falls back on the class's rune.
        let fallback = Prize::of(&[0x20, 5, 1]).item(&catalogue, &hero, 0, &mut Rolls(vec![]));
        assert_eq!(
            fallback,
            Some(RewardItem {
                item: 5387,
                class_rune: true
            })
        );
        // A type that is no hero's has no class runes.
        assert_eq!(class_rune(10, &catalogue, &mut Rolls(vec![])), None);
        assert_eq!(class_rune(0, &catalogue, &mut Rolls(vec![])), None);
    }

    #[test]
    fn every_class_draws_from_runes_of_its_own() {
        for (class, runes) in CLASS_RUNES.iter().enumerate() {
            assert!(!runes.is_empty());
            assert!(
                runes.iter().all(|&rune| (5375..=5782).contains(&rune)),
                "{class}"
            );
        }
        // The Vampiress draws the same by day and by night.
        assert_eq!(CLASS_RUNES[5], CLASS_RUNES[6]);
    }

    #[test]
    fn a_rewards_quality_is_a_key_of_the_drops_weights_by_its_step_and_the_difficulty() {
        let rules = drops::Rules::default();
        // The roll 1 of 10,000 takes the first key whose running sum passes it.
        let prize = Prize::of(&[0x20, 9, 3]);
        let key = |roll| prize.quality(Difficulty::Bronze, &rules, &mut Rolls(vec![roll]));
        let first = key(0);
        let last = key(9_999);
        assert!((0..=14).contains(&first));
        assert!((0..=14).contains(&last));
        // A higher roll never asks a better quality of the same reward.
        assert!(last <= first || last == 0);
    }

    #[test]
    fn experience_grows_with_the_sector_and_the_difficulty() {
        let prize = Prize::of(&[2, 10, 3]);
        // (5 + 2) · (8·10 + 100) / 2 = 630 on bronze, 1,260 on silver, 18,900 on niobium.
        assert_eq!(prize.experience(5, 10, Difficulty::Bronze), Some(630));
        assert_eq!(prize.experience(5, 10, Difficulty::Silver), Some(1_260));
        assert_eq!(prize.experience(5, 10, Difficulty::Niobium), Some(18_900));
        // An odd product halved and truncated: (1 + 2) · (15 + 100) / 2.
        assert_eq!(
            Prize::of(&[2, 1, 5]).experience(1, 1, Difficulty::Bronze),
            Some(172)
        );
        // A high hero's decay: 0.985^11 of 630 at level 110.
        let decayed = prize.experience(5, 110, Difficulty::Bronze).unwrap();
        let expected = whole(f64::from(fight::decay(110)) * 630.0);
        assert_eq!(i64::try_from(decayed).unwrap(), expected);
        assert!(decayed < 630);
        assert_eq!(
            Prize::of(&[1, 10, 3]).experience(5, 10, Difficulty::Bronze),
            None
        );
    }
}
