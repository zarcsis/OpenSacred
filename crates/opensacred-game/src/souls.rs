//! The souls of the bodies that vanish, and the Dark Elf's soulcatcher 1044 (#757; docs/re/arts.md,
//! "The Dark Elf's"): each body that vanishes as it has lain its time (`bodies`, `0x541491`) makes
//! its soul (780 by the FX event `0xC`), which homes on the Dark Elf where his effect 35 holds within
//! 550 of it (the object 869, `0x5FF7C0`), and else rises where it lay, homing on nothing (#3133;
//! the class 5, the fire ghoul and the bodies held lying never vanish so). A homing soul flies toward
//! him at `⌊10·t⌋` units a second, `t` its age, and as it
//! reaches him, nearer than his size and its own, heals him `10 + 30·L` up to his most health
//! with the effect 30 for 1 s (`opensacred_rules::arts::soul_speed`, `soul_heals`); a soul ends at
//! a cell that may not be walked on or without him. It flies from the creature's feet to his neck
//! (`0x44CBB0`), its look the creature's figure in sparks (`soul_sparks`, #1092).
//! A soul that reaches him, or that a cell stops, fades and, once its particles are gone, signals
//! his soulcatcher's object, which plays `DELF_SOULCATCHER_ABSORP` (#1173; [`ABSORBED_AFTER`]).

use bevy::prelude::*;
use opensacred_rules::arts::{SOUL_REACH, SOUL_SIZE, SOULCATCHER, soul_heals, soul_speed};

use crate::arts::HeroArts;
use crate::effects::Effects;
use crate::fight::{Experience, Health};
use crate::frame::{Playing, Step};
use crate::game::AppState;
use crate::hero::Body;
use crate::terrain::{Heights, Level};
use crate::{arrows, paths};

/// The bones a soul flies to (`0x44CBB0` of the bone 3): his neck, else his root.
const NECK: [&str; 2] = ["Bip01 Neck", "__Root"];

/// How long after a soul reaches him the soulcatcher's object plays its absorption, in seconds:
/// the soul's particles fade at 250 of their alpha's 255 a second (its state 2, `0x5AE42C`), and
/// once none is left it asks the object for its action 8 (`0x76CA78`, `0x5B5653`), which plays
/// `DELF_SOULCATCHER_ABSORP` once (`0x76A8F7`). **Hypothesis**: the request and the soul's end fall
/// in one update on another thread, so that the original may not always sound it.
const ABSORBED_AFTER: f32 = 255.0 / 250.0;
const ABSORBED: u32 = 2069;

/// A soul on its way to the Dark Elf: his entity, its level, its age in seconds and the health it
/// gives him.
#[derive(Component)]
struct Soul {
    to: Entity,
    level: Level,
    age: f32,
    heals: u32,
}

/// A soul that reached the Dark Elf, or that a cell stopped, fading: his entity, and the seconds
/// until his soulcatcher absorbs it.
#[derive(Component)]
struct Fading {
    to: Entity,
    left: f32,
}

/// A soul on its way to the Dark Elf as a save keeps it (#3809, the epic #3753): its place, him
/// by his name, its level, its age, the health it gives him, and its look; a load sends it on
/// again from there.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedSoul {
    at: Vec3,
    #[serde(with = "crate::names::required")]
    to: Entity,
    level: Level,
    age: f32,
    heals: u32,
    look: Option<crate::soul_sparks::SavedLook>,
}

impl bevy::ecs::entity::MapEntities for SavedSoul {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.to = mapper.get_mapped(self.to);
    }
}

/// A soul fading as a save keeps it (#3809): him by his name, and the seconds until his
/// soulcatcher absorbs it.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedFading {
    #[serde(with = "crate::names::required")]
    to: Entity,
    left: f32,
}

impl bevy::ecs::entity::MapEntities for SavedFading {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.to = mapper.get_mapped(self.to);
    }
}

/// The souls in `world` as a save keeps them (#3809): those on their way and those fading.
pub(crate) fn saved(world: &mut World) -> (Vec<SavedSoul>, Vec<SavedFading>) {
    let mut souls = world.query::<(Entity, &Soul, &Transform)>();
    let flying = souls
        .iter(world)
        .map(|(entity, soul, at)| SavedSoul {
            at: at.translation,
            to: soul.to,
            level: soul.level,
            age: soul.age,
            heals: soul.heals,
            look: crate::soul_sparks::saved_look(world, entity),
        })
        .collect();
    let mut fading = world.query::<&Fading>();
    let fading = fading
        .iter(world)
        .map(|soul| SavedFading {
            to: soul.to,
            left: soul.left,
        })
        .collect();
    (flying, fading)
}

/// Sends a loaded save's souls on again and fades its fading ones on (#3809); one whose Dark Elf
/// the load finds not is left out.
pub(crate) fn again(world: &mut World, (souls, fading): (Vec<SavedSoul>, Vec<SavedFading>)) {
    for saved in souls {
        if crate::names::found(Some(saved.to)).is_none() {
            continue;
        }
        let mut soul = world.spawn((
            Transform::from_translation(saved.at),
            Soul {
                to: saved.to,
                level: saved.level,
                age: saved.age,
                heals: saved.heals,
            },
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(look) = saved.look {
            crate::soul_sparks::look_again(&mut soul, look);
        }
    }
    for saved in fading {
        if crate::names::found(Some(saved.to)).is_none() {
            continue;
        }
        world.spawn((
            Fading {
                to: saved.to,
                left: saved.left,
            },
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// A body that vanishes as it has lain its time (`0x53ECA0`'s lying step, `0x541491`): the body,
/// whose figure its soul takes, and where it lies.
#[derive(Message, Clone, Copy, Debug)]
pub(crate) struct Vanished {
    pub(crate) body: Entity,
    pub(crate) at: Vec3,
}

/// Sends the soul of each body that vanishes: to the hero where his effect 35 holds within
/// [`SOUL_REACH`] of it, of the health that his 1044 at its level in play gives, else rising where
/// it lay, homing on nothing.
fn catch(
    mut commands: Commands,
    mut vanished: MessageReader<Vanished>,
    arts: Option<Res<HeroArts>>,
    experience: Res<Experience>,
    heroes: Query<(Entity, &Transform, &Body, &Effects)>,
) {
    let catcher = arts.as_deref().and_then(|arts| {
        let (hero, at, body, _) = heroes
            .iter()
            .find(|(.., effects)| effects.holds(SOULCATCHER))?;
        let level = arts.arts.level_in_play(
            1044,
            opensacred_rules::arts::Creature {
                kind: arts.kind,
                level: experience.level,
                player: true,
            },
        );
        let spell = arts.tables.spells.iter().find(|spell| spell.id == 47)?;
        let heals = soul_heals(spell, level.unwrap_or(1));
        Some((hero, at.translation, body.level(), heals))
    });
    for &Vanished { body: from, at } in vanished.read() {
        let home = catcher.filter(|&(_, to, ..)| (at - to).xz().length() < SOUL_REACH);
        // It leaves the body's place at its height (`0x5A46B3`).
        let mut soul = commands.spawn((
            Transform::from_translation(at),
            crate::soul_sparks::Sparks {
                from,
                rising: home.is_none(),
            },
            DespawnOnExit(AppState::InGame),
        ));
        if let Some((hero, _, level, heals)) = home {
            debug!("souls: a soul for {hero} of {heals} from {at:?}");
            soul.insert(Soul {
                to: hero,
                level,
                age: 0.0,
                heals,
            });
        }
    }
}

/// Flies each soul toward the hero's neck and heals him as it reaches him (`0x5A7BB2`,
/// `0x5ABD80`'s case `0x30C`); one that a cell stops fades without healing him (`0x5A7DA8`).
fn home(
    mut commands: Commands,
    time: Res<Time>,
    heights: Res<Heights>,
    mut souls: Query<(Entity, &mut Soul, &mut Transform), Without<Body>>,
    mut heroes: Query<(&Transform, &mut Health, Option<&mut Effects>), With<Body>>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
) {
    let dt = time.delta_secs();
    for (entity, mut soul, mut transform) in &mut souls {
        let Ok((at, mut health, effects)) = heroes.get_mut(soul.to) else {
            commands.entity(entity).despawn();
            continue;
        };
        soul.age += dt;
        let here = transform.translation.xz();
        let offset = at.translation.xz() - here;
        // He reaches it within his size and its own (the heroes' 50).
        if offset.length() < arrows::size(0) + SOUL_SIZE {
            health.current = (health.current + soul.heals).min(health.most);
            // Its effect 30 of the values 0 keyed by 100, a cure, adds nothing (#3139).
            if let Some(mut effects) = effects {
                effects.cure(crate::effects::HEAL_CURES);
            }
            debug!(
                "souls: {entity} heals {} by {}, health {}/{}",
                soul.to, soul.heals, health.current, health.most
            );
            commands.entity(entity).despawn();
            commands.spawn((
                Fading {
                    to: soul.to,
                    left: ABSORBED_AFTER,
                },
                DespawnOnExit(AppState::InGame),
            ));
            continue;
        }
        let neck = crate::making::bone(soul.to, &NECK, &children, (&bones, &worn))
            .unwrap_or(at.translation);
        let way = neck - transform.translation;
        let step = (soul_speed(soul.age) * dt).min(way.length());
        let next = transform.translation + way.normalize_or_zero() * step;
        if !heights.walkable_on(paths::cell_of(next.x, next.z), soul.level) {
            commands.entity(entity).despawn();
            commands.spawn((
                Fading {
                    to: soul.to,
                    left: ABSORBED_AFTER,
                },
                DespawnOnExit(AppState::InGame),
            ));
            continue;
        }
        transform.translation = next;
    }
}

/// The soulcatcher's absorption of each soul faded out, at the Dark Elf while its effect holds:
/// its sound, and its twirls' swelling (`soulcatchers`).
fn absorb(
    mut commands: Commands,
    time: Res<Time>,
    mut fading: Query<(Entity, &mut Fading)>,
    heroes: Query<&Effects, With<Body>>,
    (mut sounds, mut absorbed): (
        MessageWriter<crate::sfx::Play>,
        MessageWriter<crate::soulcatchers::Absorbs>,
    ),
) {
    for (entity, mut soul) in &mut fading {
        soul.left -= time.delta_secs();
        if soul.left > 0.0 {
            continue;
        }
        commands.entity(entity).despawn();
        if heroes
            .get(soul.to)
            .is_ok_and(|effects| effects.holds(SOULCATCHER))
        {
            sounds.write(crate::sfx::Play {
                sound: crate::sfx::Of::Id(ABSORBED),
                at: Some(soul.to),
                rate: (0.5, crate::sfx::Roll::None),
            });
            absorbed.write(crate::soulcatchers::Absorbs(soul.to));
        }
    }
}

/// The Dark Elf's souls.
pub struct SoulsPlugin;

impl Plugin for SoulsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Vanished>().add_systems(
            Update,
            (catch, home, absorb)
                .chain()
                .after(Step::Npcs)
                .in_set(Playing),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_soul_saved_goes_on_to_the_dark_elf_of_its_name_with_its_heals() {
        use crate::names::{Id, Loading, Named, Names, NamesPlugin, Refs, Saving, mapped};
        let mut app = App::new();
        app.add_plugins(NamesPlugin);
        let elf = app.world_mut().spawn(Named(Id::Hero)).id();
        app.world_mut().spawn((
            Soul {
                to: elf,
                level: None,
                age: 1.5,
                heals: 70,
            },
            Transform::from_xyz(10.0, 2.0, 30.0),
        ));
        app.world_mut().spawn(Fading { to: elf, left: 0.4 });
        let mut refs = Refs::default();
        let (souls, fading) = saved(app.world_mut());
        let mut saving = Saving {
            world: app.world(),
            refs: &mut refs,
        };
        let kept = (mapped(souls, &mut saving), mapped(fading, &mut saving));
        let bytes = postcard::to_stdvec(&(&refs, &kept)).unwrap();
        let (refs, kept): (Refs, (Vec<SavedSoul>, Vec<SavedFading>)) =
            postcard::from_bytes(&bytes).unwrap();
        let mut loaded = App::new();
        loaded.add_plugins(NamesPlugin);
        loaded.world_mut().spawn_batch((0..3).map(|_| ()));
        let elf = loaded.world_mut().spawn(Named(Id::Hero)).id();
        let mut loading = Loading {
            refs: &refs,
            names: loaded.world().resource::<Names>(),
            lost: 0,
        };
        let kept = (mapped(kept.0, &mut loading), mapped(kept.1, &mut loading));
        again(loaded.world_mut(), kept);
        let world = loaded.world_mut();
        let souls: Vec<(Entity, f32, u32, Vec3)> = world
            .query::<(&Soul, &Transform)>()
            .iter(world)
            .map(|(soul, at)| (soul.to, soul.age, soul.heals, at.translation))
            .collect();
        assert_eq!(souls, [(elf, 1.5, 70, Vec3::new(10.0, 2.0, 30.0))]);
        let fading: Vec<(Entity, f32)> = world
            .query::<&Fading>()
            .iter(world)
            .map(|soul| (soul.to, soul.left))
            .collect();
        assert_eq!(fading, [(elf, 0.4)]);
    }
}
