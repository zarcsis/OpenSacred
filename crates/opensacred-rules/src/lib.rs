//! `opensacred-rules`: the game's rules with no engine (ADR-04, PLAN.md §6.3), what the original's
//! client decides as docs/re/ reads it, written as plain Rust that the Bevy systems of
//! `opensacred-game` call, so that a switch of the engine keeps them (#328).
//!
//! It holds the spawning's rolls ([`spawning`]), the fight's formulas with the hero's levels
//! ([`fight`]), the ways with the cells in the way ([`ways`]), the creatures' decisions with
//! the hero's attack ([`minds`]), the archers' arrows ([`arrows`]), what a killed creature
//! drops ([`drops`]), what a container gives as it opens ([`containers`]), a creature's combat
//! arts with the runes that teach them ([`arts`]), a hero's combos ([`combos`]), the combat
//! master's exchange of runes ([`exchange`]), the spells' cast ([`spells`]), the creatures'
//! casts ([`casts`]), the scripts' state: the quests, the variables and the dialogues
//! ([`quests`]), the regions' kills ([`regions`]), a quest's reward ([`rewards`]), the weather
//! ([`weather`]), the peace stream's music and the fights' ([`music`]), the undead that rise
//! again ([`undead`]), the particle systems ([`particles`]) with a held weapon's effects
//! ([`weapon_effects`]), the scripts' Pong ([`pong`]) and
//! traps ([`script_traps`]) with the sword's guard ([`zhadum`]), the
//! backpack's grid ([`inventory`]), the equipment's slots and who may use an item
//! ([`equipment`]), the land explored ([`explored`]), the compass's ways across the dungeons'
//! mouths ([`mouths`]), the potions and the belt ([`potions`]),
//! the weapon sets ([`sets`]), the item sets' bonuses ([`set_bonuses`]), the prices and the
//! merchants' stock ([`trade`]), the blacksmith ([`smith`]), the horses ([`horses`]) and the last
//! fight's slaves with Shaddar's ward ([`shaddar`]).

pub mod arrows;
pub mod arts;
pub mod casts;
pub mod combos;
pub mod containers;
pub mod drops;
pub mod equipment;
pub mod exchange;
pub mod explored;
pub mod fight;
pub mod horses;
pub mod inventory;
pub mod minds;
pub mod mouths;
pub mod music;
pub mod particles;
pub mod pick;
pub mod pong;
pub mod potions;
pub mod quests;
pub mod regions;
pub mod rewards;
pub mod script_traps;
pub mod set_bonuses;
pub mod sets;
pub mod shaddar;
pub mod smith;
pub mod spawning;
pub mod spells;
pub mod trade;
pub mod undead;
pub mod ways;
pub mod weapon_effects;
pub mod weather;
pub mod zhadum;

/// A global cell of the world: `x` and `y`, 64 to a sector.
pub type Cell = (i32, i32);
