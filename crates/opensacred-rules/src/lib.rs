//! `opensacred-rules`: the game's rules with no engine (ADR-04, PLAN.md §6.3), what the original's
//! client decides as docs/re/ reads it, written as plain Rust that the Bevy systems of
//! `opensacred-game` call, so that a switch of the engine keeps them (#328).
//!
//! It holds the spawning's rolls ([`spawning`]), the fight's formulas with the hero's levels
//! ([`fight`]), the ways with the cells in the way ([`ways`]), the creatures' decisions with
//! the hero's attack ([`minds`]), the archers' arrows ([`arrows`]), what a killed creature
//! drops ([`drops`]), a creature's combat arts with the runes that teach them ([`arts`]), the
//! spells' cast ([`spells`]), the scripts' state: the quests, the variables and the dialogues
//! ([`quests`]), a quest's reward ([`rewards`]), and the weather ([`weather`]).

pub mod arrows;
pub mod arts;
pub mod drops;
pub mod fight;
pub mod minds;
pub mod quests;
pub mod rewards;
pub mod spawning;
pub mod spells;
pub mod ways;
pub mod weather;

/// A global cell of the world: `x` and `y`, 64 to a sector.
pub type Cell = (i32, i32);
