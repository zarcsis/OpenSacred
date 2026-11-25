//! The names that a save refers to creatures and things by (#3755, the epic #3753): an entity
//! names a slot of the running world alone, which a load makes anew, so a save names each by an
//! [`Id`] that the load's world gives the same creature again, as the original's save keeps its
//! objects' ids, which stay valid as its whole object table comes back (docs/re/saves.md, "What a
//! load puts back").
//!
//! [`Named`] carries a creature's or a thing's name, inserted as it is made, and [`Names`] finds
//! the entity of a name, kept by observers as names come and go. A save's references map through
//! Bevy's `MapEntities` to indices of its own table of names ([`Refs`], [`Saving`]), and a load's
//! back to the entities that bear those names then ([`Loading`]), none where none does.

use std::collections::HashMap;

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// What a save calls a creature or a thing by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Id {
    /// The hero.
    Hero,
    /// The creature or the object of a placement of the scripts, by its index in the placements
    /// (`npcs::PlacementIndex`), which a loaded save's start makes again at the same index (#1958).
    Placed(u32),
    /// A creature made in play of no placement of its own: a sector's fill's, one a creature
    /// summoned, one the hero summoned, called or raised, one the command `spawn` made; by a
    /// serial of the game (`npcs::Kept`'s count), which a save keeps.
    Born(u32),
    /// An item lying or flying, by its place in a save's list of the drops (`drops::laid`),
    /// which its load lays again in that order (#3763).
    Drop(u32),
}

/// The name of a creature or a thing.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Named(pub Id);

/// The entity that bears each name.
#[derive(Resource, Default, Debug)]
pub struct Names(HashMap<Id, Entity>);

impl Names {
    /// The entity that bears `id`, if any.
    pub fn get(&self, id: Id) -> Option<Entity> {
        self.0.get(&id).copied()
    }
}

/// Notes the entity of a name as it is given.
fn given(given: On<Insert<Named>>, named: Query<&Named>, mut names: ResMut<Names>) {
    let Ok(&Named(id)) = named.get(given.entity) else {
        return;
    };
    if let Some(before) = names.0.insert(id, given.entity)
        && before != given.entity
    {
        debug!("names: {id:?} passes from {before} to {}", given.entity);
    }
}

/// Forgets the entity of a name as it is taken away or replaced (discarded), unless another bears
/// it now.
fn taken(taken: On<Discard<Named>>, named: Query<&Named>, mut names: ResMut<Names>) {
    let Ok(&Named(id)) = named.get(taken.entity) else {
        return;
    };
    if names.0.get(&id) == Some(&taken.entity) {
        names.0.remove(&id);
    }
}

/// A save's table of the names that its references point at: a reference is kept as the index
/// of its name in the table.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Refs(Vec<Id>);

impl Refs {
    /// The index of `id` in the table, added where it is not yet.
    fn index_of(&mut self, id: Id) -> u32 {
        let found = self.0.iter().position(|&other| other == id);
        let index = found.unwrap_or_else(|| {
            self.0.push(id);
            self.0.len() - 1
        });
        u32::try_from(index).unwrap_or(u32::MAX)
    }

    /// The name of the reference `index`.
    fn id(&self, index: u32) -> Option<Id> {
        self.0.get(usize::try_from(index).ok()?).copied()
    }

    /// The count of names.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

/// Maps a running world's entities to a save's references: an entity that bears a name to the
/// entity of its index in [`Refs`], one that bears none to [`Entity::PLACEHOLDER`], which a load
/// takes for none.
pub struct Saving<'a> {
    pub world: &'a World,
    pub refs: &'a mut Refs,
}

impl EntityMapper for Saving<'_> {
    fn get_mapped(&mut self, source: Entity) -> Entity {
        let Some(&Named(id)) = self.world.get::<Named>(source) else {
            return Entity::PLACEHOLDER;
        };
        Entity::from_raw_u32(self.refs.index_of(id)).unwrap_or(Entity::PLACEHOLDER)
    }

    fn set_mapped(&mut self, _source: Entity, _target: Entity) {}
}

/// Maps a save's references back to the loaded world's entities that bear their names, a name
/// that none bears to [`Entity::PLACEHOLDER`].
pub struct Loading<'a> {
    pub refs: &'a Refs,
    pub names: &'a Names,
    /// The references that found no entity.
    pub lost: usize,
}

impl EntityMapper for Loading<'_> {
    fn get_mapped(&mut self, source: Entity) -> Entity {
        if source == Entity::PLACEHOLDER {
            return source;
        }
        let found = self
            .refs
            .id(source.index_u32())
            .and_then(|id| self.names.get(id));
        found.unwrap_or_else(|| {
            self.lost += 1;
            Entity::PLACEHOLDER
        })
    }

    fn set_mapped(&mut self, _source: Entity, _target: Entity) {}
}

/// `entity` where a load found the creature of its name, none where it found none
/// ([`Entity::PLACEHOLDER`], [`Loading`]).
pub fn found(entity: Option<Entity>) -> Option<Entity> {
    entity.filter(|&entity| entity != Entity::PLACEHOLDER)
}

/// `value` with its references mapped by `mapper`.
pub fn mapped<T: MapEntities>(mut value: T, mapper: &mut impl EntityMapper) -> T {
    value.map_entities(mapper);
    value
}

/// A saved reference, kept as its entity's bits (`#[serde(with)]`): bits that name no entity
/// read as [`Entity::PLACEHOLDER`], none's mark.
pub mod required {
    use bevy::prelude::Entity;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(entity: &Entity, to: S) -> Result<S::Ok, S::Error> {
        entity.to_bits().serialize(to)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(from: D) -> Result<Entity, D::Error> {
        let bits = u64::deserialize(from)?;
        Ok(Entity::try_from_bits(bits).unwrap_or(Entity::PLACEHOLDER))
    }
}

/// A saved reference that may be none, kept as its entity's bits (`#[serde(with)]`): bits that
/// name no entity read as [`Entity::PLACEHOLDER`], none's mark, rather than failing the save's
/// read.
pub mod optional {
    use bevy::prelude::Entity;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(entity: &Option<Entity>, to: S) -> Result<S::Ok, S::Error> {
        entity.map(Entity::to_bits).serialize(to)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(from: D) -> Result<Option<Entity>, D::Error> {
        let bits = Option::<u64>::deserialize(from)?;
        Ok(bits.map(|bits| Entity::try_from_bits(bits).unwrap_or(Entity::PLACEHOLDER)))
    }
}

/// Gives each name its entity.
pub struct NamesPlugin;

impl Plugin for NamesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Names>()
            .add_observer(given)
            .add_observer(taken);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(NamesPlugin);
        app
    }

    #[test]
    fn a_name_finds_its_entity_while_it_bears_it() {
        let mut app = app();
        let wolf = app.world_mut().spawn(Named(Id::Born(7))).id();
        let names = app.world().resource::<Names>();
        assert_eq!(names.get(Id::Born(7)), Some(wolf));
        assert_eq!(names.get(Id::Born(8)), None);
        app.world_mut().despawn(wolf);
        assert_eq!(app.world().resource::<Names>().get(Id::Born(7)), None);
    }

    #[test]
    fn a_name_given_again_passes_to_the_newer_and_stays_with_it() {
        let mut app = app();
        let old = app.world_mut().spawn(Named(Id::Placed(3))).id();
        let new = app.world_mut().spawn(Named(Id::Placed(3))).id();
        assert_eq!(
            app.world().resource::<Names>().get(Id::Placed(3)),
            Some(new)
        );
        // The older's going leaves the newer's.
        app.world_mut().despawn(old);
        assert_eq!(
            app.world().resource::<Names>().get(Id::Placed(3)),
            Some(new)
        );
        // A name replaced by another frees it.
        app.world_mut().entity_mut(new).insert(Named(Id::Born(1)));
        let names = app.world().resource::<Names>();
        assert_eq!(names.get(Id::Placed(3)), None);
        assert_eq!(names.get(Id::Born(1)), Some(new));
    }

    #[test]
    fn a_reference_goes_by_its_name_to_the_loaded_worlds_entity() {
        let mut saved = app();
        let hero = saved.world_mut().spawn(Named(Id::Hero)).id();
        let goblin = saved.world_mut().spawn(Named(Id::Placed(12))).id();
        let nameless = saved.world_mut().spawn_empty().id();
        let mut refs = Refs::default();
        let kept = {
            let mut saving = Saving {
                world: saved.world(),
                refs: &mut refs,
            };
            mapped(vec![goblin, hero, nameless, goblin], &mut saving)
        };
        assert_eq!(refs.len(), 2);
        assert_eq!(kept[2], Entity::PLACEHOLDER);
        assert_eq!(kept[0], kept[3]);
        let bytes = postcard::to_stdvec(&(&refs, &kept)).unwrap();
        let (refs, kept): (Refs, Vec<Entity>) = postcard::from_bytes(&bytes).unwrap();
        // The loaded world makes them anew, at other slots, the goblin not yet.
        let mut loaded = app();
        loaded.world_mut().spawn_batch((0..5).map(|_| ()));
        let hero = loaded.world_mut().spawn(Named(Id::Hero)).id();
        let names = loaded.world().resource::<Names>();
        let mut loading = Loading {
            refs: &refs,
            names,
            lost: 0,
        };
        let back = mapped(kept, &mut loading);
        assert_eq!(back[1], hero);
        assert_eq!(back[0], Entity::PLACEHOLDER);
        assert_eq!(back[2], Entity::PLACEHOLDER);
        assert_eq!(loading.lost, 2);
    }
}
