//! `opensacred-rules`: the game's rules with no engine (ADR-04, PLAN.md §6.3), what the original's
//! client decides as docs/re/ reads it, written as plain Rust that the Bevy systems of
//! `opensacred-game` call, so that a switch of the engine keeps them (#328).
//!
//! It holds the spawning's rolls ([`spawning`]), the fight's formulas with the hero's levels
//! ([`fight`]), the ways with the cells in the way ([`ways`]), the creatures' decisions with
//! the hero's attack ([`minds`]) and the archers' arrows ([`arrows`]).

pub mod arrows;
pub mod fight;
pub mod minds;
pub mod spawning;
pub mod ways;

/// A global cell of the world: `x` and `y`, 64 to a sector.
pub type Cell = (i32, i32);
