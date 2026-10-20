//! The creatures in one another's way, as the client's test of a step has a creature in a cell
//! stop another's step into it (`0x4F7E10`, docs/re/movement.md, "The objects in the way"): the
//! cells the creatures stand on, at their levels, which the hero's and the walking NPCs' steps
//! and ways go round.
//!
//! A creature stops others only when it is alive, not big (its `Creature.pak` flags' bit
//! `0x02`) and not a critter (its flags word's bit `0x1000000`); and it stops the hero, of id 1,
//! only when its flags word has bit 0 or 1 (`CreateNPC`'s tags `0x12` and `0x13`: the monsters,
//! the start's captain), every other creature, of an id above 16, whatever its flags.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::frame::Step;
use crate::hero::Body;
use crate::paths::Cell;
use crate::terrain::Level;

/// The bits of a creature's flags word (`+0x1F4`) that stop the hero: a hostile creature, awake
/// (bit 0) or of a dormant group (bit 1).
const STOPS_THE_HERO: u32 = 0x3;
/// The bit of the flags word that the creature maker sets on a hostile creature (`0x480D40`).
pub const HOSTILE: u32 = 0x1;
/// The bit of a creature's flags word that keeps it from stopping others: the critters.
const CRITTER: u32 = 0x100_0000;
/// The bit of a `Creature.pak` record's flags of the big creatures, which stop no step by their
/// cell (`0x426CB0`).
pub const BIG: u8 = 0x02;
/// The bit of a `Creature.pak` record's flags whose creatures the constructor makes critters
/// (`0x52DE40`).
const CRITTER_RECORD: u8 = 0x20;

/// How a creature stands in the others' way, and the level it stands on.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct InTheWay {
    /// Whether it stops the others' steps: not big, not a critter.
    pub stops: bool,
    /// Whether it stops the hero's too: its flags word has bit 0 or 1.
    pub hostile: bool,
    /// Whether it is a creature, which crowds another in its cell (`0x54FCB0` counts the types
    /// of the categories 2, 3 and `0x20`, `0x426610`); an object is not.
    pub creature: bool,
    /// The level it stands on; the hero's is its `Body`'s.
    pub level: Level,
}

impl InTheWay {
    /// An object of category 4 (a chest, a barrel, a vase, a coffin, a cabinet, a book stand, a
    /// shrine) on `level`, which `0x4F7E10` has stop a creature of id above 16 as a creature
    /// does, not the hero.
    pub fn object(level: Level) -> Self {
        Self {
            stops: true,
            hostile: false,
            creature: false,
            level,
        }
    }

    /// A creature of type `creature` (its `items.pak` slot), whose `Creature.pak` record has
    /// `record_flags`, and whose flags word is `word`, standing on `level`.
    pub fn of(creature: u32, record_flags: u8, word: u32, level: Level) -> Self {
        let critter = word & CRITTER != 0 || critter(creature, record_flags);
        Self {
            stops: record_flags & BIG == 0 && !critter,
            hostile: word & STOPS_THE_HERO != 0,
            creature: true,
            level,
        }
    }
}

/// Whether the constructor makes a creature of type `creature` a critter (`0x52DE40`): some
/// types by their numbers, and the types whose record has [`CRITTER_RECORD`].
fn critter(creature: u32, record_flags: u8) -> bool {
    matches!(
        creature,
        0x47 | 0x4B
            | 0x201..=0x204
            | 0x24D
            | 0x24E
            | 0x250..=0x254
            | 0x230..=0x233
            | 0x235
            | 0x236
    ) || record_flags & CRITTER_RECORD != 0
}

/// The flags word (`+0x1F4`) that `CreateNPC` (`0x482510`) makes from a record's operand-less
/// tags, in order (docs/02 §7.3): one mode in the low 16 bits, the last of the mode tags; a few
/// tags add bits above it.
pub fn flags_word(tags: impl IntoIterator<Item = u8>) -> u32 {
    let (mut mode, mut more) = (0u32, 0u32);
    for tag in tags {
        match tag {
            0x12 => mode = 0x1,
            0x13 => mode = 0x2,
            0x14 => mode = 0,
            0x2B => mode = 0x8,
            0x2E => mode = 0x10,
            0x30 => mode = 0x20,
            0x2F | 0x31 => mode = 0x40,
            0x32 => mode = 0x80,
            0x42 => mode = 0x400,
            0x43 => mode = 0x800,
            0x44 => mode = 0x1000,
            0x46 => mode = 0x4000,
            0x4C => mode = 0x8000,
            0x45 => more |= 0x2000,
            0x50 => more |= 0x1_0000,
            0x74 => more |= 0x800_0000,
            0x85 => {
                mode = 0;
                more |= 0x200_0000;
            }
            0x8A => {
                mode = 0;
                more |= 0x1000_0000;
            }
            _ => {}
        }
    }
    mode | more
}

/// The creatures standing in each cell at each level, this frame.
#[derive(Resource, Default)]
pub struct Crowd {
    cells: HashMap<(Cell, Level), Vec<(Entity, InTheWay)>>,
}

impl Crowd {
    /// Whether a creature stops `stepper`'s step into `cell` at `level`: for the hero (`hero`),
    /// a hostile one; for another, any that stops the others; never the stepper itself.
    pub fn stops(&self, cell: Cell, level: Level, stepper: Entity, hero: bool) -> bool {
        self.cells.get(&(cell, level)).is_some_and(|here| {
            here.iter()
                .any(|&(entity, way)| entity != stepper && way.stops && (way.hostile || !hero))
        })
    }

    /// Whether another creature than `entity` stands in `cell` at `level`, the hero among them
    /// (`0x54FCB0`).
    pub fn crowded(&self, cell: Cell, level: Level, entity: Entity) -> bool {
        self.cells.get(&(cell, level)).is_some_and(|here| {
            here.iter()
                .any(|&(other, way)| other != entity && way.creature)
        })
    }

    fn add(&mut self, cell: Cell, entity: Entity, way: InTheWay) {
        self.cells
            .entry((cell, way.level))
            .or_default()
            .push((entity, way));
    }
}

/// Takes the cells the creatures stand on, the hero's at its level of now.
pub fn gather(
    mut crowd: ResMut<Crowd>,
    creatures: Query<(Entity, &Transform, &InTheWay, Option<&Body>)>,
) {
    crowd.cells.clear();
    for (entity, transform, way, body) in &creatures {
        let mut way = *way;
        if let Some(body) = body {
            way.level = body.level();
        }
        let p = transform.translation;
        crowd.add(crate::paths::cell_of(p.x, p.z), entity, way);
    }
}

/// The distance in world units from the hero to its nearest hostile creature, as the client
/// keeps it for the fight stance (`0xAD3CE4`, filled by `0x547070`, docs/re/motions.md, "The
/// fight stance"): looked for every 0.3 s among the creatures of the 3 × 3 sectors about the hero
/// within [`RANGE`], and kept as it was while none is found; 0 before any is, as the client's is
/// when the process starts.
#[derive(Resource, Default)]
pub struct Nearest {
    pub distance: f32,
    /// Whether the last look found one: the hero's own stance asks it (`0x547070` sets the
    /// hero's bit `0x8000` only then).
    pub found: bool,
    /// The time since the last look.
    since: f32,
}

/// How often the hero looks for its nearest hostile, in seconds (`+0x204` of a player's hero).
const LOOK_EVERY: f32 = 0.3;
/// How far it looks, in world units: the client's `(S + 45) · 25`, `S` a smoothed rate of its
/// frames (**hypothesis**: about 1,900 to 2,600 units); taken at 30 frames a second.
const RANGE: f32 = 1_875.0;

/// Looks for the hero's nearest hostile creature: one whose flags word has bit 0 or 1, in the
/// 3 × 3 sectors about the hero and within [`RANGE`] (**hypothesis**: those bits for the client's
/// hostility to the hero, `0x548F30`).
fn look(
    time: Res<Time>,
    mut nearest: ResMut<Nearest>,
    hero: Query<&Transform, With<Body>>,
    creatures: Query<(&Transform, &InTheWay), Without<Body>>,
) {
    nearest.since += time.delta_secs();
    if nearest.since < LOOK_EVERY {
        return;
    }
    nearest.since = 0.0;
    let Some(hero) = hero.iter().next() else {
        return;
    };
    let sector = |p: Vec3| {
        let side = crate::iso::CELL * 64.0;
        ((p.x / side).floor(), (p.z / side).floor())
    };
    let (sx, sy) = sector(hero.translation);
    let found = creatures
        .iter()
        .filter(|(_, way)| way.hostile)
        .filter(|(t, _)| {
            let (x, y) = sector(t.translation);
            (x - sx).abs() <= 1.0 && (y - sy).abs() <= 1.0
        })
        .map(|(t, _)| (t.translation - hero.translation).xz().length())
        .filter(|&d| d < RANGE)
        .fold(None, |min: Option<f32>, d| {
            Some(min.map_or(d, |m| m.min(d)))
        });
    nearest.found = found.is_some();
    if let Some(distance) = found {
        nearest.distance = distance;
    }
}

/// The cells the creatures stand on, taken before the hero and the NPCs move, and the hero's
/// nearest hostile.
pub struct CrowdPlugin;

impl Plugin for CrowdPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Crowd>()
            .init_resource::<Nearest>()
            .add_systems(Update, (gather, look).in_set(Step::Crowd));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_npc_tags_make_the_flags_word() {
        // The guards (`0E 46`), the captain (`0E 46 13`), the monsters (`08 12`).
        assert_eq!(flags_word([0x0E, 0x46]), 0x4000);
        assert_eq!(flags_word([0x0E, 0x46, 0x13]), 0x2);
        assert_eq!(flags_word([0x08, 0x12]), 0x1);
        assert_eq!(flags_word([0x12, 0x45, 0x85]), 0x200_2000);
    }

    #[test]
    fn the_hero_is_stopped_by_the_hostile_alone_and_the_others_by_all() {
        let mut world = World::new();
        let (hero, guard, captain, rabbit, troll) = (
            world.spawn_empty().id(),
            world.spawn_empty().id(),
            world.spawn_empty().id(),
            world.spawn_empty().id(),
            world.spawn_empty().id(),
        );
        let mut crowd = Crowd::default();
        crowd.add(
            (1, 1),
            guard,
            InTheWay::of(295, 0, flags_word([0x0E, 0x46]), None),
        );
        crowd.add(
            (2, 2),
            captain,
            InTheWay::of(295, 0, flags_word([0x0E, 0x46, 0x13]), None),
        );
        // A critter by its type, and a big creature, stop no one.
        crowd.add(
            (3, 3),
            rabbit,
            InTheWay::of(0x4B, 0, flags_word([0x12]), None),
        );
        crowd.add(
            (4, 4),
            troll,
            InTheWay::of(300, BIG, flags_word([0x12]), None),
        );
        crowd.add((5, 5), hero, InTheWay::of(1, 0, 0, None));
        // The hero walks through the guard, not through the captain.
        assert!(!crowd.stops((1, 1), None, hero, true));
        assert!(crowd.stops((2, 2), None, hero, true));
        // An NPC is stopped by the guard, the captain and the hero, not by itself.
        assert!(crowd.stops((1, 1), None, captain, false));
        assert!(crowd.stops((5, 5), None, guard, false));
        assert!(!crowd.stops((1, 1), None, guard, false));
        // Nobody by the rabbit or the troll; nothing on another level.
        assert!(!crowd.stops((3, 3), None, guard, false) && !crowd.stops((4, 4), None, hero, true));
        assert!(!crowd.stops((1, 1), Some((0, 0, 1)), captain, false));
        // The hero's cell is crowded for another, the rabbit's too, not a chest's.
        assert!(crowd.crowded((5, 5), None, guard) && !crowd.crowded((5, 5), None, hero));
        assert!(crowd.crowded((3, 3), None, troll));
        let chest = world.spawn_empty().id();
        crowd.add((6, 6), chest, InTheWay::object(None));
        assert!(!crowd.crowded((6, 6), None, guard));
    }
}
