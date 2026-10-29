//! The fight's formulas of `Sacred.exe` against `opensacred-rules`' on random inputs (#1431):
//! both run in this process on each input, the original's in the emulator, and proptest's runner
//! shrinks a mismatch to the smallest input that still shows it, running both again on each
//! smaller one. The inputs keep to the game's ranges with a bias toward their bounds and the
//! band of ±0.001 of `0x43EEE0`; an input on which the emulator faults is rejected and counted.
//! The integers compare exactly, the floats within `tests/oracle.rs`'s tolerances: bit-exact
//! results are not asked for (docs/01 §7).

use std::cell::{Cell, RefCell};
use std::fmt::Debug;

use anyhow::{Context, Result};
use opensacred_data::tables::{Balance, BalanceValue, BalanceVariable};
use opensacred_formats::bin_tables::{self, BALANCE_KEYS, Kind};
use opensacred_rules::fight::{self, Rules};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestError, TestRng, TestRunner};

use crate::Set;
use crate::fight as original;
use crate::machine::Machine;

/// The fight's rules of the balance `balance`, the whole of `bin/Balance.bin`, its variables
/// named as `opensacred-convert tables` names them.
pub fn rules(balance: &[u8]) -> Result<Rules> {
    let read = bin_tables::balance(balance).context("bin/Balance.bin")?;
    let variables = BALANCE_KEYS
        .iter()
        .map(|key| {
            let word = read
                .word(key)
                .with_context(|| format!("bin/Balance.bin: no word for {}", key.name))?;
            Ok(BalanceVariable {
                name: key.name.to_owned(),
                offset: key.offset,
                value: match key.kind {
                    Kind::F32 => BalanceValue::F32(f32::from_bits(word)),
                    Kind::I32 => BalanceValue::I32(i32::from_le_bytes(word.to_le_bytes())),
                },
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let balance = Balance {
        variables,
        image: read.image,
        ..Balance::default()
    };
    Ok(Rules::of(Some(&balance)))
}

/// Runs the set `set` on `cases` inputs from the seed `seed` and tells what it found.
pub fn report(
    machine: &mut Machine<'_>,
    rules: &Rules,
    set: Set,
    (cases, seed): (u32, u64),
) -> String {
    let runs = (cases, seed);
    let (rejected, mismatch) = match set {
        Set::HitChance => {
            let found = hit_chances(machine, runs, fight::hit_chance);
            let mismatch = found
                .mismatch
                .map(|(input, why)| format!("{input:?}: {why}"));
            (found.rejected, mismatch)
        }
        Set::Dealt => {
            let found = dealt(machine, runs);
            let mismatch = found
                .mismatch
                .map(|(input, why)| format!("{input:?}: {why}"));
            (found.rejected, mismatch)
        }
        Set::MostHealth => {
            let found = most_health(machine, runs, rules);
            let mismatch = found
                .mismatch
                .map(|(input, why)| format!("{input:?}: {why}"));
            (found.rejected, mismatch)
        }
    };
    let found = mismatch.map_or_else(
        || "no mismatch".to_owned(),
        |mismatch| format!("a mismatch, shrunk: {mismatch}"),
    );
    format!("{set:?}: {cases} cases from the seed {seed}: {found}; {rejected} inputs rejected")
}

/// What a run found: the inputs rejected, and the first mismatch shrunk, with why.
#[derive(Debug)]
pub struct Found<T> {
    pub rejected: u32,
    pub mismatch: Option<(T, String)>,
}

/// The largest attack, defence, damage or resistance taken: twice the vectors' largest.
const MOST: f32 = 10_000.0;
/// The tolerances of `tests/oracle.rs`: a chance within 10⁻⁶, a damage within 10⁻⁵ of itself.
const CHANCE_WITHIN: f32 = 1e-6;
const DAMAGE_WITHIN: f64 = 1e-5;

/// Runs `cases` inputs of `inputs` from the seed `seed`, each compared by `compare`: none where
/// both agree, why they part where they do, an error where the emulator faults (the input
/// rejected).
fn run<T: Clone + Debug>(
    machine: &mut Machine<'_>,
    (cases, seed): (u32, u64),
    inputs: impl Strategy<Value = T>,
    compare: impl Fn(&mut Machine<'_>, &T) -> Result<Option<String>>,
) -> Found<T> {
    let machine = RefCell::new(machine);
    let rejected = Cell::new(0_u32);
    let config = Config {
        cases,
        max_global_rejects: cases,
        failure_persistence: None,
        ..Config::default()
    };
    let mut bytes = [0_u8; 32];
    bytes[..8].copy_from_slice(&seed.to_le_bytes());
    let mut runner =
        TestRunner::new_with_rng(config, TestRng::from_seed(RngAlgorithm::ChaCha, &bytes));
    let result = runner.run(&inputs, |input| {
        match compare(&mut machine.borrow_mut(), &input) {
            Ok(None) => Ok(()),
            Ok(Some(why)) => Err(TestCaseError::fail(why)),
            Err(error) => {
                rejected.set(rejected.get() + 1);
                Err(TestCaseError::reject(error.to_string()))
            }
        }
    });
    let mismatch = match result {
        Ok(()) => None,
        Err(TestError::Fail(why, input)) => Some((input, why.to_string())),
        Err(TestError::Abort(why)) => {
            eprintln!("fuzz: stopped: {why}");
            None
        }
    };
    Found {
        rejected: rejected.get(),
        mismatch,
    }
}

/// An attack, a defence, a damage or a resistance: its bounds, the band of ±0.001 and just
/// above it the more often.
fn amount() -> impl Strategy<Value = f32> + Clone {
    prop_oneof![
        1 => Just(0.0_f32),
        1 => Just(0.001_f32),
        2 => 0.0..0.001_f32,
        2 => 0.001..0.002_f32,
        1 => Just(MOST),
        8 => 0.0..=MOST,
    ]
}

/// The hit chances of the original and of `ours`, an attack against a defence.
pub fn hit_chances(
    machine: &mut Machine<'_>,
    runs: (u32, u64),
    ours: impl Fn(f32, f32) -> f32,
) -> Found<(f32, f32)> {
    run(
        machine,
        runs,
        (amount(), amount()),
        |machine, &(attack, defence)| {
            let theirs = original::chance(machine, attack, defence)?;
            let ours = ours(attack, defence);
            Ok(((ours - theirs).abs() > CHANCE_WITHIN)
                .then(|| format!("ours {ours}, the original's {theirs}")))
        },
    )
}

/// The damages let through of the original and of the rules, a damage against a resistance, by
/// the levels above.
pub fn dealt(machine: &mut Machine<'_>, runs: (u32, u64)) -> Found<(f32, f32, u32)> {
    let above = prop_oneof![1 => Just(98_u32), 1 => Just(99), 1 => Just(100), 6 => 0..=215_u32];
    run(
        machine,
        runs,
        (amount(), amount(), above),
        |machine, &(damage, resistance, above)| {
            let theirs = original::let_through(machine, damage, resistance, above)?;
            let ours = f64::from(fight::dealt(damage, resistance, fight::exponent(above)));
            Ok(
                ((ours - theirs).abs() > DAMAGE_WITHIN * theirs.abs().max(1.0))
                    .then(|| format!("ours {ours}, the original's {theirs}")),
            )
        },
    )
}

/// The most health of the original and of the rules `rules`, of base attributes at a level, of
/// a hero's type or not.
pub fn most_health(
    machine: &mut Machine<'_>,
    runs: (u32, u64),
    rules: &Rules,
) -> Found<([u8; 6], u16, bool)> {
    let attribute =
        prop_oneof![1 => Just(0_u8), 1 => Just(u8::MAX), 6 => 0..=60_u8, 2 => any::<u8>()];
    let level = prop_oneof![1 => Just(1_u16), 1 => Just(216), 6 => 1..=216_u16];
    run(
        machine,
        runs,
        (prop::array::uniform6(attribute), level, any::<bool>()),
        |machine, &(base, level, hero)| {
            let theirs = original::health(machine, base.map(u16::from), level, hero)?.most;
            let ours = if hero {
                fight::hero_health(base, u32::from(level), &[], rules)
            } else {
                fight::most_health(base, u32::from(level), &[], 1.0, rules)
            };
            Ok((i64::from(ours) != i64::from(theirs))
                .then(|| format!("ours {ours}, the original's {theirs}")))
        },
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::pe::Image;
    use crate::{machine, vectors};

    /// The hit chance as `opensacred-rules` has it, with the band of `0x43EEE0` moved from 0.001
    /// to 0.002: a defence within that is taken as dividing nothing.
    fn wrong_hit_chance(attack: f32, defence: f32) -> f32 {
        let ratio = if f64::from(defence).abs() <= 0.002 {
            10_000.0
        } else {
            attack / defence
        };
        1.0 - 1.0 / (1.0 + ratio)
    }

    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn a_wrong_hit_chance_is_found_and_shrunk_to_its_band() {
        let game = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../OriginalGame");
        let exe = vectors::checked(&game, "Sacred.exe").unwrap_or_else(|e| panic!("{e:#}"));
        let image = Image::parse(&exe).unwrap_or_else(|e| panic!("{e:#}"));
        let control = machine::control_word(53).unwrap_or_else(|e| panic!("{e:#}"));
        let mut machine = Machine::new(&image, control).unwrap_or_else(|e| panic!("{e:#}"));
        original::prepare(&mut machine).unwrap_or_else(|e| panic!("{e:#}"));
        // The rules' own agree; the wrong copy parts within the band it moved, and the mismatch
        // shrinks to a defence there, just above the original's 0.001.
        let right = hit_chances(&mut machine, (2_000, 1), fight::hit_chance);
        assert!(right.mismatch.is_none(), "{:?}", right.mismatch);
        let wrong = hit_chances(&mut machine, (2_000, 1), wrong_hit_chance);
        let ((_, defence), why) = wrong
            .mismatch
            .unwrap_or_else(|| panic!("no mismatch found"));
        assert!(
            f64::from(defence) > 0.001 && f64::from(defence) <= 0.002,
            "shrunk to the defence {defence}: {why}"
        );
    }
}
