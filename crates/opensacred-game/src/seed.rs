//! The run's seed of the rolls (#1644). The original rolls by the C library's `rand()`, which it
//! seeds with the time as it starts (`srand(time())`, `0x849B33`: `initApp` `0x815FC7`, `cEngine`'s
//! constructor `0x60ACA9`, docs/re/items.md), so that every run rolls otherwise. The game's
//! generators that stand for that `rand()` take one seed of the run, the clock's unless `--seed`
//! gives one, which the log and the console's `seed` tell so that a run can be played again: each
//! mixes it into its own constant ([`dice`]), and a game's own rolls, made anew as it starts (the
//! scripts', the quest book's), the game's count too ([`Game`]), as the original's `rand()` runs on
//! from one game to the next. The rolls seeded on purpose keep their seeds: the sectors' fills
//! and `CreateNPC`'s levels by their sector and cell (docs/decisions/spawning.md), the containers'
//! by the original's own seed (`opensacred_rules::containers::seed`).
//!
//! Without a seed set, as in the tests, the run's seed is 0, which leaves each generator its
//! constant alone.

use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;
use opensacred_rules::spawning::Rng;

/// The run's seed, set once as the process starts.
static RUN: OnceLock<u64> = OnceLock::new();

/// Sets the run's seed, `given` or else the clock's, before the plugins make their generators;
/// a later call keeps the first's. The seed set.
pub fn init(given: Option<u64>) -> u64 {
    let seed = given.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| {
                since.as_secs() ^ u64::from(since.subsec_nanos()) << 32
            })
    });
    *RUN.get_or_init(|| seed)
}

/// The run's seed, 0 where none was set.
pub fn run() -> u64 {
    RUN.get().copied().unwrap_or(0)
}

/// The run's generator of the constant `constant`.
pub fn dice(constant: usize) -> Rng {
    Rng::of(0, 0, constant).mixed(run())
}

/// The seed of the game being played, which its own rolls start from: the run's, with the count
/// of the games before it in the run mixed in ([`Game::of`]), so that the run's first game takes
/// the run's own.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Game(pub u64);

impl Game {
    /// The seed of the run's game `count`, from 0.
    pub fn of(count: u64) -> Self {
        Self(run() ^ count.wrapping_mul(0x9E37_79B9_7F4A_7C15))
    }

    /// The game's generator of the constant `constant`.
    pub fn dice(self, constant: usize) -> Rng {
        Rng::of(0, 0, constant).mixed(self.0)
    }
}

/// Writes the run's seed into the log as the game starts.
fn tell() {
    info!("seed: the run's seed is {} (--seed gives it again)", run());
}

/// The run's seed.
pub struct SeedPlugin;

impl Plugin for SeedPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, tell);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_without_a_seed_leaves_each_generator_its_constant() {
        // The tests set none.
        let rolls = |mut rng: Rng| (0..8).map(|_| rng.below(1000)).collect::<Vec<_>>();
        assert_eq!(rolls(dice(415)), rolls(Rng::of(0, 0, 415)));
        assert_eq!(rolls(Game::of(0).dice(1560)), rolls(Rng::of(0, 0, 1560)));
        // A run's later games roll otherwise.
        assert_ne!(rolls(Game::of(1).dice(1560)), rolls(Game::of(0).dice(1560)));
        assert_ne!(rolls(Game::of(2).dice(1560)), rolls(Game::of(1).dice(1560)));
    }
}
