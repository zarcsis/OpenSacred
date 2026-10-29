//! The game's rolls (#1433): the functions of `Sacred.exe` that roll with the C library's `rand()`
//! run with its stub answering a script of values, as `opensacred-rules`' `Script` dice answer
//! them, so that both sides take the same values and must agree on each roll and on how many
//! values they take. The oracle sets the inputs and the script and reads the outputs; what the
//! functions compute is the original's machine code (docs/08 §3).

use anyhow::Result;
use serde::Serialize;

use crate::machine::Machine;

/// The C library's `rand()` (`cdecl`), linked into the executable: the thread's LCG of docs/01 §7,
/// 0 to 32,767.
const RAND: u32 = 0x84_9B40;
/// The game's roll of a whole number between two (`R`, `cdecl`, docs/re/items.md).
const BETWEEN: u32 = 0x44_3B20;

/// The largest value of `rand()`.
pub const RAND_MOST: u32 = 0x7FFF;

/// Makes `rand()` answer the scripts.
pub fn prepare(machine: &mut Machine<'_>) -> Result<()> {
    machine.stub_rand(RAND)
}

/// A roll between two bounds.
#[derive(Serialize)]
pub struct Between {
    pub least: i32,
    pub most: i32,
    /// The values that `rand()` answers in turn.
    pub rolls: Vec<u32>,
    pub value: i32,
    /// How many values the roll took.
    pub taken: usize,
}

/// The roll `R(least, most)` with `rand()` answering `rolls` in turn: its value, and how many
/// values it took.
pub fn between(
    machine: &mut Machine<'_>,
    least: i32,
    most: i32,
    rolls: &[u32],
) -> Result<(i32, usize)> {
    machine.script_rand(rolls);
    let word = |value: i32| u32::from_le_bytes(value.to_le_bytes());
    let eax = machine
        .call(BETWEEN, None, &[word(least), word(most)], false)?
        .eax;
    Ok((i32::from_le_bytes(eax.to_le_bytes()), machine.rand_taken()))
}

/// The bounds and the scripts of the set: negative bounds, equal ones, reversed ones, a span past
/// `rand()`'s largest value and the widest.
const LEASTS: [i32; 5] = [-5, 0, 1, 5, 100];
const MOSTS: [i32; 9] = [-1, 0, 1, 5, 99, 1000, 32_767, 40_000, i32::MAX];
const SCRIPTS: [u32; 4] = [0, 1, 12_345, RAND_MOST];

/// The rolls between the bounds, each with a script of one value.
pub fn betweens(machine: &mut Machine<'_>) -> Result<Vec<Between>> {
    let mut vectors = Vec::new();
    for least in LEASTS {
        for most in MOSTS {
            for value in SCRIPTS {
                let (rolled, taken) = between(machine, least, most, &[value])?;
                vectors.push(Between {
                    least,
                    most,
                    rolls: vec![value],
                    value: rolled,
                    taken,
                });
            }
        }
    }
    Ok(vectors)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::pe::Image;
    use crate::{machine, vectors};

    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn a_script_reaches_the_originals_roll_between_two() {
        let game = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../OriginalGame");
        let exe = vectors::checked(&game, "Sacred.exe").unwrap_or_else(|e| panic!("{e:#}"));
        let image = Image::parse(&exe).unwrap_or_else(|e| panic!("{e:#}"));
        let control = machine::control_word(53).unwrap_or_else(|e| panic!("{e:#}"));
        let mut machine = Machine::new(&image, control).unwrap_or_else(|e| panic!("{e:#}"));
        prepare(&mut machine).unwrap_or_else(|e| panic!("{e:#}"));
        let mut roll = |least, most, rolls: &[u32]| {
            between(&mut machine, least, most, rolls).unwrap_or_else(|e| panic!("{e:#}"))
        };
        // The same bounds with two scripts: 10 and the value modulo the 11 values of 10 to 20.
        assert_eq!(roll(10, 20, &[3, 8]), (13, 1));
        assert_eq!(roll(10, 20, &[8, 3]), (18, 1));
        assert_eq!(roll(20, 10, &[25]), (13, 1));
        // Equal bounds take no value.
        assert_eq!(roll(7, 7, &[5]), (7, 0));
    }
}
