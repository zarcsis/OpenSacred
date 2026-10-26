//! The creatures in one another's way, as the client's test of a step has a creature in a cell
//! stop another's step into it (`0x4F7E10`, docs/re/movement.md, "The objects in the way"): the
//! cells the creatures stand on, at their levels, which the hero's and the walking NPCs' steps
//! and ways go round. Who stops whose step is the rules' (`opensacred_rules::ways::InTheWay`).

use std::collections::HashMap;

use bevy::prelude::*;
pub use opensacred_rules::ways::{BIG, HOSTILE, constructed, flags_word};
use opensacred_rules::{minds, ways};

use crate::frame::{Step, WorldTick};
use crate::hero::Body;
use crate::npcs::{Dies, Object};
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

    /// A creature whose `Creature.pak` record has `record_flags` and whose flags word is `word`,
    /// standing on `level`.
    pub fn of(record_flags: u8, word: u32, level: Level) -> Self {
        Self {
            stands: ways::InTheWay::of(record_flags, word),
            level,
        }
    }
}

/// A creature standing in a cell: how it stands in the way, and the master whose steps it does
/// not stop, its follower's ([`ways::stops_its_master`]).
#[derive(Clone, Copy, Debug)]
struct Standing {
    entity: Entity,
    way: InTheWay,
    spares: Option<Entity>,
}

/// The creatures standing in each cell at each level, this frame.
#[derive(Resource, Default)]
pub struct Crowd {
    cells: HashMap<(Cell, Level), Vec<Standing>>,
}

impl Crowd {
    /// Whether a creature stops `stepper`'s step into `cell` at `level`: for the hero (`hero`),
    /// a hostile one; for another, any that stops the others; never the stepper itself, nor its
    /// followers but those that stop it all the same.
    pub fn stops(&self, cell: Cell, level: Level, stepper: Entity, hero: bool) -> bool {
        self.cells.get(&(cell, level)).is_some_and(|here| {
            here.iter().any(|standing| {
                standing.entity != stepper
                    && standing.spares != Some(stepper)
                    && standing.way.stops_step(hero)
            })
        })
    }

    /// Whether another creature than `entity` stands in `cell` at `level`, the hero among them
    /// (`0x54FCB0`).
    pub fn crowded(&self, cell: Cell, level: Level, entity: Entity) -> bool {
        self.cells.get(&(cell, level)).is_some_and(|here| {
            here.iter()
                .any(|standing| standing.entity != entity && standing.way.creature)
        })
    }

    #[cfg(test)]
    fn add(&mut self, cell: Cell, entity: Entity, way: InTheWay) {
        self.spare(cell, entity, way, None);
    }

    /// Adds a creature standing in `cell`, which does not stop `master`'s steps.
    fn spare(&mut self, cell: Cell, entity: Entity, way: InTheWay, master: Option<Entity>) {
        self.cells
            .entry((cell, way.level))
            .or_default()
            .push(Standing {
                entity,
                way,
                spares: master,
            });
    }
}

/// Takes the cells the creatures stand on, the hero's at its level of now, and an object's its
/// logical position's, its `0x04` cell, wherever its model is drawn (`npcs::Object`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
pub fn gather(
    mut crowd: ResMut<Crowd>,
    creatures: Query<(
        Entity,
        &Transform,
        &InTheWay,
        Option<&Body>,
        Option<&Object>,
        Option<(&crate::followers::Follower, &crate::ai::Mind)>,
    )>,
) {
    crowd.cells.clear();
    for (entity, transform, way, body, object, follower) in &creatures {
        let mut way = *way;
        if let Some(body) = body {
            way.level = body.level();
        }
        let p = object.map_or(transform.translation, |object| object.at);
        // A follower does not stop its master's steps, its master the hero, unless it is hostile
        // to it, a soldier or a guard.
        let spares = follower
            .filter(|(_, mind)| !ways::stops_its_master(mind.hostile(), mind.class, mind.flags))
            .map(|(follower, _)| follower.master);
        crowd.spare(crate::paths::cell_of(p.x, p.z), entity, way, spares);
    }
}

/// The hero's list of its hostile creatures, as the client keeps it for the fight stance and the
/// places about the hero (`0xAD3CE0`, filled by `0x547070`, docs/re/motions.md, "The fight
/// stance"; docs/re/fight.md, "Melee: the AI's state 4"): looked for every 0.3 s of the world's
/// ticks (`minds::Look`, #617) among the creatures of the 3 × 3 sectors about the hero within
/// [`RANGE`], at most 55, nearest first.
/// The distance to the nearest is kept as it was while none is found, 0 before any is, as the
/// client's is when the process starts.
#[derive(Resource, Default)]
pub struct Nearest {
    pub distance: f32,
    /// Whether the last look found one: the hero's own stance asks it (`0x547070` sets the
    /// hero's bit `0x8000` only then).
    pub found: bool,
    /// The hostiles that the last look found, nearest first: none when it found none, as the
    /// list's count (`0xAD3BC8`) is then 0.
    pub hostiles: Vec<Entity>,
    /// Whether the hero looked at this tick: the creatures' thought asks it too, as the same
    /// look wakes them (`ai`).
    pub looked: bool,
    /// The look's timer.
    look: minds::Look,
}
/// How far it looks, in world units: the client's `(S + 45) · 25`, `S` a smoothed rate of its
/// frames (**hypothesis**: about 1,900 to 2,600 units); taken at 30 frames a second.
const RANGE: f32 = 1_875.0;
/// How many hostiles the list keeps.
const LISTED: usize = 55;

/// At a tick of the world whose time takes the look's timer above 0.3 s (`0x5375C0`, #617), looks
/// for the hero's hostile creatures: those alive that are hostile to the hero by their sides and
/// types (`0x548F30`, `ai::Mind::hostile`), in the 3 × 3 sectors about the hero and within
/// [`RANGE`]; not by their flags word, whose bits 0 and 1 a summoned follower has too (#1013).
pub(crate) fn look(
    tick: Res<WorldTick>,
    mut nearest: ResMut<Nearest>,
    hero: Query<&Transform, With<Body>>,
    creatures: Query<(Entity, &Transform, &crate::ai::Mind), Without<Dies>>,
) {
    let nearest = &mut *nearest;
    nearest.looked = nearest.look.looks(tick.seconds.unwrap_or_default());
    if !nearest.looked {
        return;
    }
    let Some(hero) = hero.iter().next() else {
        return;
    };
    let sector = |p: Vec3| {
        let side = crate::iso::CELL * 64.0;
        ((p.x / side).floor(), (p.z / side).floor())
    };
    let (sx, sy) = sector(hero.translation);
    let mut found: Vec<(f32, Entity)> = creatures
        .iter()
        .filter(|(_, _, mind)| mind.hostile())
        .filter(|(_, t, _)| {
            let (x, y) = sector(t.translation);
            (x - sx).abs() <= 1.0 && (y - sy).abs() <= 1.0
        })
        .map(|(entity, t, _)| ((t.translation - hero.translation).xz().length(), entity))
        .filter(|&(d, _)| d < RANGE)
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.truncate(LISTED);
    nearest.found = !found.is_empty();
    if let Some(&(distance, _)) = found.first() {
        nearest.distance = distance;
    }
    nearest.hostiles = found.into_iter().map(|(_, entity)| entity).collect();
}

/// The cells the creatures stand on, taken before the hero and the NPCs move, and the hero's
/// list of hostiles.
pub struct CrowdPlugin;

impl Plugin for CrowdPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Crowd>()
            .init_resource::<Nearest>()
            .add_systems(
                Update,
                (gather, look.run_if(crate::frame::ticked)).in_set(Step::Crowd),
            );
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
            InTheWay::of(0, flags_word([0x0E, 0x46]), None),
        );
        crowd.add(
            (2, 2),
            captain,
            InTheWay::of(0, flags_word([0x0E, 0x46, 0x13]), None),
        );
        // A fill's critter, by its constructor's bit, and a big creature, stop no one.
        crowd.add((3, 3), rabbit, InTheWay::of(0, constructed(516, 0), None));
        crowd.add((4, 4), troll, InTheWay::of(BIG, flags_word([0x12]), None));
        crowd.add((5, 5), hero, InTheWay::of(0, 0, None));
        // A summoned wolf, of bit 0, stops others, not its master.
        let wolf = world.spawn_empty().id();
        crowd.spare((7, 7), wolf, InTheWay::of(0, 0x101, None), Some(hero));
        assert!(!crowd.stops((7, 7), None, hero, true));
        assert!(crowd.stops((7, 7), None, guard, false));
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

    #[test]
    fn an_object_takes_its_cell_wherever_its_model_is_drawn() {
        let mut app = App::new();
        app.init_resource::<Crowd>().add_systems(Update, gather);
        // A chest of cell (6, 6) whose model is drawn in cell (9, 6).
        app.world_mut().spawn((
            Transform::from_translation(crate::iso::ground(9.5, 6.5)),
            InTheWay::object(None),
            Object {
                at: crate::iso::ground(6.5, 6.5),
            },
        ));
        let walker = app.world_mut().spawn_empty().id();
        app.update();
        let crowd = app.world().resource::<Crowd>();
        assert!(crowd.stops((6, 6), None, walker, false));
        assert!(!crowd.stops((9, 6), None, walker, false));
    }
}
