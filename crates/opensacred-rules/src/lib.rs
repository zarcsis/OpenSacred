//! `opensacred-rules`: the game's rules with no engine (ADR-04, PLAN.md §6.3), what the original's
//! client decides as docs/re/ reads it, written as plain Rust that the Bevy systems of
//! `opensacred-game` call, so that a switch of the engine keeps them (#328).
//!
//! It holds the spawning's rolls ([`spawning`]) and the fight's formulas with the hero's levels
//! ([`fight`]) so far; the ways and the creatures' decisions follow (#361, #362).

pub mod fight;
pub mod spawning;

/// A global cell of the world: `x` and `y`, 64 to a sector.
pub type Cell = (i32, i32);
