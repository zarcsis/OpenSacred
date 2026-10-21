//! The creatures in one another's way, as the client's test of a step has a creature in a cell
//! stop another's step into it (`0x4F7E10`, docs/re/movement.md, "The objects in the way"): the
//! cells the creatures stand on, at their levels, which the hero's and the walking NPCs' steps
//! and ways go round. Who stops whose step is the rules' (`opensacred_rules::ways::InTheWay`).

use std::collections::HashMap;

use bevy::prelude::*;
use opensacred_rules::ways;
pub use opensacred_rules::ways::{BIG, HOSTILE, flags_word};

use crate::frame::Step;
use crate::hero::Body;
use crate::paths::Cell;
use crate::terrain::Level;

/// How a creature stands in the others' way (`opensacred_rules::ways::InTheWay`), and the level
/// it stands on.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Deref)]
pub struct InTheWay {
    /// Whether it stops the others' steps, whether it stops the hero's too, and whether it
    /// crowds another in its cell.
    #[deref]
    pub stands: ways::InTheWay,
    /// The level it stands on; the hero's is its `Body`'s.
    pub level: Level,
}

impl InTheWay {
    /// An object of category 4 (a chest, a barrel, a vase, a coffin, a cabinet, a book stand, a
    /// shrine) on `level`, which `0x4F7E10` has stop a creature of id above 16 as a creature
    /// does, not the hero.
    pub fn object(level: Level) -> Self {
        Self {
            stands: ways::InTheWay::object(),
            level,
        }
    }

    /// A creature of type `creature` (its `items.pak` slot), whose `Creature.pak` record has
    /// `record_flags`, and whose flags word is `word`, standing on `level`.
    pub fn of(creature: u32, record_flags: u8, word: u32, level: Level) -> Self {
        Self {
            stands: ways::InTheWay::of(creature, record_flags, word),
            level,
        }
    }
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
                .any(|&(entity, way)| entity != stepper && way.stops_step(hero))
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
