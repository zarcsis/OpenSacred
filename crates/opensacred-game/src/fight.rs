//! The fight's rules as far as the game follows them (docs/re/fight.md): the creatures' levels
//! and their health.

use bevy::prelude::*;
use opensacred_data::tables::Balance;

use crate::spawning::{Rng, SpawnValues};

/// The hero's level, which the creatures' levels follow: the hero gains none yet.
pub const HERO_LEVEL: u32 = 1;
/// The balance's row of a difficulty's values that the game plays: bronze, the row's sixth
/// value (docs/re/fight.md).
const BRONZE: usize = 5;

/// A creature's health, its most, and its level.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Health {
    pub current: u32,
    pub most: u32,
    pub level: u32,
}

/// The balance's values that a creature's health takes: the points of the level's factor of a
/// creature not a hero (`HP_s1`…`HP_w5`) and the difficulty's `ProzHP`.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct HealthRules {
    points: Vec<(f32, f32)>,
    factor: f32,
}

impl Default for HealthRules {
    /// Without the balance, no factor.
    fn default() -> Self {
        Self {
            points: Vec::new(),
            factor: 1.0,
        }
    }
}

impl HealthRules {
    /// The rules of the balance `balance`, the factors that it lacks 1.
    pub fn of(balance: Option<&Balance>) -> Self {
        let points = balance
            .and_then(|b| {
                (1..=5)
                    .map(|i| Some((b.f32(&format!("HP_s{i}"))?, b.f32(&format!("HP_w{i}"))?)))
                    .collect::<Option<Vec<_>>>()
            })
            .unwrap_or_default();
        let factor = balance
            .and_then(|b| b.row_f32("ProzHP", BRONZE))
            .unwrap_or(1.0);
        Self { points, factor }
    }

    /// The factor of a creature's level `level` (`0x5658F0`): between two of the points about
    /// it, the first's weight plus the level times the slope, the level taken whole rather than
    /// from the first point (the original's own); 1 outside the points.
    fn level_factor(&self, level: f32) -> f32 {
        self.points
            .windows(2)
            .find(|pair| pair[0].0 <= level && level <= pair[1].0)
            .map_or(1.0, |pair| {
                let ((s0, w0), (s1, w1)) = (pair[0], pair[1]);
                w0 + level * (w1 - w0) / (s1 - s0)
            })
    }
}

/// A creature's level (docs/re/spawning.md): the hero's held within its sector's lowest and
/// highest, and for one of a fill, `jitter`, moved by 0 to 1 below 6, −1 to 2 below 30, −2 to 3
/// below 90, −3 to 6 below 250; at least 1.
pub fn level(values: &SpawnValues, jitter: Option<&mut Rng>) -> u32 {
    let level = HERO_LEVEL.clamp(values.lowest.min(values.highest), values.highest);
    let Some(rng) = jitter else {
        return level.max(1);
    };
    let (down, up) = match level {
        0..6 => (0, 1),
        6..30 => (1, 2),
        30..90 => (2, 3),
        _ => (3, 6),
    };
    let moved = level + rng.below(down + up + 1);
    moved.saturating_sub(down).max(1)
}

/// The most health of a creature of `Creature.pak`'s base bytes `base` and level `level`
/// (`0x5658F0`): with `a` and `b` the bytes 0 and 3, `S = a + b + ⌊(L − 1)a/10⌋ + ⌊(L − 1)b/10⌋`
/// and `g = 1.5 + 0.004 · max(0, 60 − a − b)`, `⌊(b0 + b2) · S^(g − 1) · 10/31⌋`, 100 for 0,
/// times the level's factor of a creature not a hero and the difficulty's (`rules`). No items,
/// skills, bosses or champions yet (docs/decisions/2026-10-20-health.md).
pub fn most_health(base: [u8; 6], level: u32, rules: &HealthRules) -> u32 {
    let (a, b) = (f64::from(base[0]), f64::from(base[3]));
    let l = f64::from(level.max(1) - 1);
    let s = a + b + (l * a / 10.0).floor() + (l * b / 10.0).floor();
    let g = 1.5 + 0.004 * (60.0 - a - b).max(0.0);
    let most = ((a + f64::from(base[2])) * s.powf(g - 1.0) * 10.0 / 31.0).floor();
    let most = if most <= 0.0 { 100.0 } else { most };
    let level = f32::from(u16::try_from(level).unwrap_or(u16::MAX));
    let factor = f64::from(rules.level_factor(level) * rules.factor);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a health, whole and positive, far below the integers' range"
    )]
    let most = (most * factor).floor().max(1.0) as u32;
    most
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_most_health_grows_with_the_level_by_the_originals_formula() {
        let rules = HealthRules::default();
        // b0 20, b2 10, b3 15: S = 35, g = 1.5 + 0.004 · 25 = 1.6; ⌊30 · 35^0.6 · 10/31⌋.
        let base = [20, 0, 10, 15, 0, 0];
        let expected = (30.0_f64 * 35.0_f64.powf(0.6) * 10.0 / 31.0).floor();
        assert_eq!(f64::from(most_health(base, 1, &rules)), expected);
        assert!(most_health(base, 10, &rules) > most_health(base, 1, &rules));
        // A base that gives nothing gives 100.
        assert_eq!(most_health([0; 6], 1, &rules), 100);
    }

    #[test]
    fn the_level_factor_takes_the_level_whole_between_its_points() {
        let rules = HealthRules {
            points: vec![
                (1.0, 0.75),
                (4.0, 0.8),
                (10.0, 0.9),
                (20.0, 1.0),
                (35.0, 1.1),
            ],
            factor: 1.0,
        };
        // Between 1 and 4: 0.75 + L · 0.05/3, the level whole.
        assert!((rules.level_factor(1.0) - (0.75 + 0.05 / 3.0)).abs() < 1e-6);
        assert!((rules.level_factor(20.0) - (0.9 + 20.0 * 0.1 / 10.0)).abs() < 1e-6);
        assert!((rules.level_factor(40.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_creature_takes_the_heros_level_within_its_sectors() {
        let start = SpawnValues {
            density: 50,
            lowest: 1,
            highest: 4,
        };
        let west = SpawnValues {
            density: 50,
            lowest: 25,
            highest: 38,
        };
        assert_eq!(level(&start, None), 1);
        assert_eq!(level(&west, None), 25);
        let mut rng = Rng::of(50, 39, 0);
        assert!((0..200).all(|_| (1..=2).contains(&level(&start, Some(&mut rng)))));
        assert!((0..200).all(|_| (24..=27).contains(&level(&west, Some(&mut rng)))));
    }
}
