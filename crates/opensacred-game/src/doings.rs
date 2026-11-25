//! What the creatures are doing, as a save keeps it (#3756, the epic #3753): a creature's record
//! of the original's save (`cCreature`'s `0x54B3D0`; docs/re/saves.md, "A creature's record"),
//! each creature's [`Doing`], which a load gives back to the creature that bears its name
//! (`names`) once the loaded scene is whole (`saves`' resume).
//!
//! A module gives and takes the state of its own components through [`Saves`], and [`Doing`]
//! lists them, a field each, given back in the order of the mind, the body and the action. It
//! holds so far the mind (`ai::Mind`, #3757) with its wake and its shooing, its walk and its
//! height aloft (#3758), the effects, whose sources go by their names, and the knock-downs'
//! marks; the next sub-issues of #3753 add the actions.
//!
//! The commands `doings` and `unsaved` show each named creature's doing and the components of
//! the game that neither a save keeps nor a load makes again ([`DERIVED`]).

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::prelude::*;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::effects::Effects;
use crate::fight::KnockMarks;
use crate::names::{Id, Named};

/// A component that a save keeps and a load gives back: its saved form, which keeps its entities
/// by their names (`names`) and its clocks of the real time from the save's time, and the
/// component again of that form.
pub trait Saves: Component + Sized {
    type Saved: Serialize + DeserializeOwned + MapEntities;

    /// Its saved form at the moment `at`, none where it keeps nothing worth a save.
    fn saved(&self, at: &Moment) -> Option<Self::Saved>;

    /// The component of its saved form at the moment `at`, none where it makes none.
    fn again(saved: Self::Saved, at: &Moment) -> Option<Self>;
}

/// The moment of a save or of a load: the real clock's seconds, from which a saved form keeps
/// its clocks of the real time and on which a load puts them (`Time<Real>`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Moment {
    pub now: f64,
}

impl Moment {
    /// The moment of `world` now.
    pub fn of(world: &World) -> Self {
        Self {
            now: world
                .get_resource::<Time<bevy::time::Real>>()
                .map_or(0.0, Time::<bevy::time::Real>::elapsed_secs_f64),
        }
    }
}

impl Saves for Effects {
    type Saved = Effects;

    fn saved(&self, _: &Moment) -> Option<Effects> {
        (!self.timed().is_empty()).then(|| self.clone())
    }

    fn again(mut saved: Effects, _: &Moment) -> Option<Self> {
        saved.forget_lost();
        Some(saved)
    }
}

impl Saves for KnockMarks {
    type Saved = Marks;

    fn saved(&self, _: &Moment) -> Option<Marks> {
        (self.0 != 0).then_some(Marks(self.0))
    }

    fn again(saved: Marks, _: &Moment) -> Option<Self> {
        Some(Self(saved.0))
    }
}

/// The knock-downs' marks as a save keeps them (`fight::KnockMarks`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marks(u8);

impl MapEntities for Marks {
    fn map_entities<E: EntityMapper>(&mut self, _: &mut E) {}
}

/// What a creature is doing, as a save keeps it: a field each of the components that make it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, MapEntities)]
pub struct Doing {
    /// Its mind, the creatures and doors it goes to by their names (#3757), whether a wake waits
    /// for its sight, and the child it shoos.
    #[entities]
    mind: Option<crate::ai::SavedMind>,
    woken: Option<crate::ai::Woken>,
    shoos: Option<crate::ai::Shoos>,
    /// Its body: where it walks and how (#3758), and its height aloft.
    walk: Option<crate::npcs::SavedWalk>,
    aloft: Option<crate::flight::Aloft>,
    /// Its effects, their sources by their names.
    #[entities]
    effects: Option<Effects>,
    marks: Option<Marks>,
}

impl Doing {
    /// The doing of `entity` as it stands, none where it does nothing that a save keeps.
    pub fn of(world: &World, entity: Entity) -> Option<Self> {
        let at = Moment::of(world);
        let doing = Self {
            mind: saved::<crate::ai::Mind>(world, entity, &at),
            woken: saved::<crate::ai::Woken>(world, entity, &at),
            shoos: saved::<crate::ai::Shoos>(world, entity, &at),
            walk: saved::<crate::npcs::Walk>(world, entity, &at),
            aloft: saved::<crate::flight::Aloft>(world, entity, &at),
            effects: saved::<Effects>(world, entity, &at),
            marks: saved::<KnockMarks>(world, entity, &at),
        };
        (doing != Self::default()).then_some(doing)
    }

    /// Gives `entity` this doing, in place of what it did since it was made: in the order of the
    /// mind, the body and the action.
    pub fn give(self, world: &mut World, entity: Entity) {
        let at = Moment::of(world);
        let Ok(mut creature) = world.get_entity_mut(entity) else {
            return;
        };
        put::<crate::ai::Mind>(&mut creature, self.mind, &at);
        put::<crate::ai::Woken>(&mut creature, self.woken, &at);
        put::<crate::ai::Shoos>(&mut creature, self.shoos, &at);
        put::<crate::npcs::Walk>(&mut creature, self.walk, &at);
        put::<crate::flight::Aloft>(&mut creature, self.aloft, &at);
        put::<Effects>(&mut creature, self.effects, &at);
        put::<KnockMarks>(&mut creature, self.marks, &at);
    }
}

/// The saved form of `entity`'s component `T`, if it bears one worth a save.
fn saved<T: Saves>(world: &World, entity: Entity, at: &Moment) -> Option<T::Saved> {
    world
        .get::<T>(entity)
        .and_then(|component| component.saved(at))
}

/// The named creatures' doings as a save keeps them, by their names.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Doings(pub Vec<(Id, Doing)>);

impl Doings {
    /// The names of the creatures whose doings these are.
    pub fn owners(&self) -> impl Iterator<Item = Id> + '_ {
        self.0.iter().map(|&(id, _)| id)
    }
}

impl MapEntities for Doings {
    fn map_entities<E: EntityMapper>(&mut self, mapper: &mut E) {
        for (_, doing) in &mut self.0 {
            doing.map_entities(mapper);
        }
    }
}

/// Puts the component of `saved` on `creature`, or takes away the one it has where the save kept
/// none.
fn put<T: Saves>(creature: &mut EntityWorldMut<'_>, saved: Option<T::Saved>, at: &Moment) {
    match saved.and_then(|saved| T::again(saved, at)) {
        Some(component) => {
            creature.insert(component);
        }
        None => {
            creature.remove::<T>();
        }
    }
}

/// Each named creature's doing, but the hero's, which its own part of the save keeps, by name,
/// in the order of their names (`names::Id`), its references kept by their names in `refs`.
pub fn gather(world: &mut World, refs: &mut crate::names::Refs) -> Doings {
    let mut named = world.query_filtered::<(Entity, &Named), With<crate::ai::Mind>>();
    let mut creatures: Vec<(Id, Entity)> = named
        .iter(world)
        .filter(|(_, named)| named.0 != Id::Hero)
        .map(|(entity, named)| (named.0, entity))
        .collect();
    creatures.sort_by_key(|&(id, _)| order(id));
    let world = &*world;
    let doings = creatures
        .into_iter()
        .filter_map(|(id, entity)| {
            let doing = Doing::of(world, entity)?;
            let mut saving = crate::names::Saving { world, refs };
            Some((id, crate::names::mapped(doing, &mut saving)))
        })
        .collect();
    Doings(doings)
}

/// A name's place in a save's order: the placements, then the creatures made in play.
fn order(id: Id) -> (u8, u32) {
    match id {
        Id::Hero => (0, 0),
        Id::Placed(index) => (1, index),
        Id::Born(serial) => (2, serial),
    }
}

/// Gives each saved doing, its references mapped to the loaded world's entities already
/// (`names::Loading`), to the creature that bears its name; the count of those given and of those
/// whose creature is not in the world.
pub fn give(world: &mut World, doings: Doings) -> (usize, usize) {
    let (mut given, mut missing) = (0, 0);
    for (id, doing) in doings.0 {
        let entity = world
            .get_resource::<crate::names::Names>()
            .and_then(|names| names.get(id));
        let Some(entity) = entity else {
            debug!("doings: {id:?} is not in the world");
            missing += 1;
            continue;
        };
        doing.give(world, entity);
        given += 1;
    }
    (given, missing)
}

/// The components of the game that a load makes again of what a save keeps otherwise, or that
/// last a frame, by their types' paths: the command `unsaved` leaves them out.
pub const DERIVED: &[&str] = &[
    "opensacred_game::names::Named",
    "opensacred_game::npcs::Placed",
    "opensacred_game::npcs::PlacementIndex",
    "opensacred_game::npcs::Npc",
    "opensacred_game::npcs::Arms",
    "opensacred_game::fight::Health",
    "opensacred_game::fight::Combat",
    "opensacred_game::fight::Regenerates",
    "opensacred_game::crowd::InTheWay",
    "opensacred_game::models::Ordered",
    "opensacred_game::models::Source",
    "opensacred_game::shadows::Casts",
    "opensacred_game::arrows::Size",
    "opensacred_game::buildings::Layered",
    "opensacred_game::sectors::InSector",
    "opensacred_game::fight::Ranked",
    "opensacred_game::fight::Worth",
    "opensacred_game::icons::Icon",
    "opensacred_game::npcs::ScriptName",
    "opensacred_game::npcs::Talk",
    "opensacred_game::npcs::RisesAgain",
    "opensacred_game::steps::Stepped",
    "opensacred_game::voices::Speaking",
];

/// The components that a creature's [`Doing`] keeps, by their types' paths.
pub const SAVED: &[&str] = &[
    "opensacred_game::ai::Mind",
    "opensacred_game::ai::Woken",
    "opensacred_game::ai::Shoos",
    "opensacred_game::npcs::Walk",
    "opensacred_game::flight::Aloft",
    "opensacred_game::effects::Effects",
    "opensacred_game::fight::KnockMarks",
];

/// The named creatures within `radius` cells of the hero, the nearest first, each with what it
/// is doing as a save keeps it (the command `doings`).
pub fn dump(world: &mut World, radius: u32) -> Vec<String> {
    near(world, radius)
        .into_iter()
        .map(|(id, entity, cells)| {
            let doing = Doing::of(world, entity);
            format!("{entity} {id:?} at {cells} cells: {doing:?}")
        })
        .collect()
}

/// The named creatures within `radius` cells of the hero, the nearest first, each with the
/// components of the game that it bears and that neither its doing keeps nor [`DERIVED`] names
/// (the command `unsaved`).
pub fn unsaved(world: &mut World, radius: u32) -> Vec<String> {
    near(world, radius)
        .into_iter()
        .map(|(id, entity, _)| {
            let mut names: Vec<String> = world
                .inspect_entity(entity)
                .map(|infos| {
                    infos
                        .map(|(_, info)| info.name().to_string())
                        .filter(|name| name.starts_with("opensacred_game::"))
                        .filter(|name| !SAVED.contains(&name.as_str()))
                        .filter(|name| !DERIVED.contains(&name.as_str()))
                        .collect()
                })
                .unwrap_or_default();
            names.sort();
            format!("{entity} {id:?}: {}", names.join(", "))
        })
        .collect()
}

/// The named creatures within `radius` cells of the hero, the nearest first, with their
/// distances in cells.
fn near(world: &mut World, radius: u32) -> Vec<(Id, Entity, u32)> {
    let mut heroes = world.query_filtered::<&Transform, With<crate::hero::Body>>();
    let Some(hero) = heroes.iter(world).next().map(|at| at.translation) else {
        return Vec::new();
    };
    let (hx, hy) = crate::paths::cell_of(hero.x, hero.z);
    let mut named = world.query_filtered::<(Entity, &Named, &Transform), With<crate::ai::Mind>>();
    let mut found: Vec<(Id, Entity, u32)> = named
        .iter(world)
        .map(|(entity, named, at)| {
            let (x, y) = crate::paths::cell_of(at.translation.x, at.translation.z);
            (named.0, entity, x.abs_diff(hx).max(y.abs_diff(hy)))
        })
        .filter(|&(.., cells)| cells <= radius)
        .collect();
    found.sort_by_key(|&(.., cells)| cells);
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::names::{Loading, Names, NamesPlugin, Refs};

    /// A creature's mind, which the doings' creatures bear.
    fn mind() -> crate::ai::Mind {
        crate::ai::Mind::new(Vec3::ZERO, 0, (false, false), None, [70.0, 180.0], (76, 2))
    }

    #[test]
    fn a_creatures_doing_goes_back_to_the_creature_of_its_name() {
        let mut saved = App::new();
        saved.add_plugins(NamesPlugin);
        let spider = saved.world_mut().spawn(Named(Id::Born(4))).id();
        let mut effects = Effects::default();
        let poison = opensacred_rules::fight::Poison {
            whole: 4,
            value: 0.4,
            seconds: 12.0,
            gathered: 3,
        };
        effects.poison(poison, spider);
        saved.world_mut().spawn((
            Named(Id::Placed(30)),
            mind(),
            effects,
            KnockMarks(KnockMarks::SECOND),
        ));
        // One doing nothing that a save keeps, and the hero, which its own part keeps.
        saved.world_mut().spawn((Named(Id::Born(9)), mind()));
        saved
            .world_mut()
            .spawn((Named(Id::Hero), mind(), KnockMarks(KnockMarks::SECOND)));
        let mut refs = Refs::default();
        let doings = gather(saved.world_mut(), &mut refs);
        // Each creature's mind, the hero's apart.
        assert_eq!(
            doings.owners().collect::<Vec<_>>(),
            [Id::Placed(30), Id::Born(9)]
        );
        let bytes = postcard::to_stdvec(&(&refs, &doings)).unwrap();
        let (refs, doings): (Refs, Doings) = postcard::from_bytes(&bytes).unwrap();
        // The loaded world makes them anew at other slots, the wolf with a mark of its own.
        let mut loaded = App::new();
        loaded.add_plugins(NamesPlugin);
        loaded.world_mut().spawn_batch((0..4).map(|_| ()));
        let spider = loaded.world_mut().spawn(Named(Id::Born(4))).id();
        let wolf = loaded
            .world_mut()
            .spawn((Named(Id::Placed(30)), mind()))
            .id();
        let doings = crate::names::mapped(
            doings,
            &mut Loading {
                refs: &refs,
                names: loaded.world().resource::<Names>(),
                lost: 0,
            },
        );
        assert_eq!(give(loaded.world_mut(), doings), (1, 1));
        let world = loaded.world();
        assert_eq!(
            world.get::<Effects>(wolf).unwrap().timed()[0].source,
            Some(spider)
        );
        assert_eq!(
            world.get::<KnockMarks>(wolf).map(|marks| marks.0),
            Some(KnockMarks::SECOND)
        );
    }

    #[test]
    fn a_doing_takes_away_what_the_creature_did_since_it_was_made() {
        let mut world = World::new();
        let wolf = world.spawn(KnockMarks(KnockMarks::SECOND)).id();
        let mut effects = Effects::default();
        effects.add(opensacred_rules::fight::Effect::new(1, 0.5), 30.0);
        world.entity_mut(wolf).insert(effects);
        Doing::default().give(&mut world, wolf);
        assert!(world.get::<KnockMarks>(wolf).is_none());
        assert!(world.get::<Effects>(wolf).is_none());
    }
}
