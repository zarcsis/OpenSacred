//! A quest's reward, the scripts' `Gewinn` (125, `0x4AC940`; docs/re/netscript.md, "The
//! rewards"): up to three numbers, its flags, a value and a step of 0 to 5. The flag 1 gives
//! gold, `(G[s]·v + 100)·(H/2 + 1)` for the hero's level `H`; the flag 2 experience, the middle
//! of the hero's sector's levels `m` making `⌊(m + 2)·(E[s]·v + 100)/2⌋` times the difficulty's
//! factor, 1, 2, 10, 20 or 30, times the decay of a high hero; the flags 4 to `0x40` items (not
//! followed yet).

use crate::drops::Difficulty;
use crate::fight;

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
