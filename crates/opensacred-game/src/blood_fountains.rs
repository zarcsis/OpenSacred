//! The blood fountains (908, `FX_BLOODFONTAIN`, #1964; docs/re/effects.md, "The blood
//! fountain"): under `[gameplay] violence`, the original's `VIOLENCE`, a creature that bleeds
//! spurts blood from one of five bones in one death of ten, and a creature that a strike's open
//! wound's roll holds for spurts it from one of the bones' table's first 45 (the part 100,
//! [`spurt_at_hit`], #3159), with `BLEEDING01` or `02` at it
//! (`opensacred_rules::particles::BloodFountain`). Its drops are drawn as the fires' billboards
//! (`fx`), laid over the frame by their alpha, turned, and tinted by the night as the light at
//! them tints them; they stay in the world while the fountain keeps to its bone.

use std::collections::HashMap;

use bevy::prelude::*;
use opensacred_rules::particles::{self, BloodFountain};

use crate::frame::{Playing, Unpaused};
use crate::fx::{Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::npcs::{Dies, Talk};
use crate::settings::Settings;
use crate::sfx::{Of, Play, Roll};
use crate::spawning::Rng;
use crate::{bolt_looks, iso};

/// The creatures' gib sets by their type (`items.pak +0x6F`), read at the start.
#[derive(Resource, Default)]
pub(crate) struct Gibs(HashMap<u32, u8>);

impl Gibs {
    /// The gib set of a creature of the type `creature`, 0 (red) where the table has none.
    pub(crate) fn of(&self, creature: u32) -> u8 {
        self.0.get(&creature).copied().unwrap_or(0)
    }
}

/// The fountains' dice: the deaths' rolls, the parts, the sounds and the drops.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x908))
    }
}

/// The bone a creature's fountain spurted from (`+0xB0` bits `0x20` to `0x200`, `0x4E87E0`), over
/// which its body's puddle lies (`bodies`, #2516).
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct Bled(pub(crate) &'static str);

/// The fountains of the part 100 that a strike's open wounds' rolls call at `creature`
/// (`fight::wound`), to start with the violence on ([`spurt_hits`]).
#[derive(Component)]
struct HitSpurts {
    creature: Entity,
    count: u8,
}

/// Asks `count` fountains of the part 100 at `creature`, which a strike's open wounds' rolls
/// call (`0x4E87E0(100, 1)`, docs/re/effects.md, "The blood fountain").
pub(crate) fn spurt_at_hit(commands: &mut Commands, creature: Entity, count: u8) {
    commands.spawn((
        HitSpurts { creature, count },
        DespawnOnExit(AppState::InGame),
    ));
}

/// The drops' material, made at the first fountain; none where the texture is missing.
#[derive(Resource, Default)]
struct Material(Option<Option<Handle<crate::fx::Effect>>>);

/// A fountain that spurts: its drops in the frame of its entity's place, where its creature fell,
/// the bone it keeps to, where that bone was a frame ago and the mesh its drops go into.
#[derive(Component)]
struct Fountain {
    fountain: BloodFountain,
    bone: Entity,
    last: Option<[f32; 3]>,
    mesh: Handle<Mesh>,
}

/// Reads the gib sets off the assets' tables at the start; without them every creature bleeds
/// red.
fn read(mut commands: Commands, data: Option<Res<Data>>) {
    let Some(data) = data else {
        return;
    };
    match data.0.tables.items() {
        Ok(items) => commands.insert_resource(Gibs(
            items
                .iter()
                .map(|item| (item.slot, item.gib_set()))
                .collect(),
        )),
        Err(e) => warn!("blood fountains: no gib sets: {e:#}"),
    }
}

/// Starts a fountain at each creature that dies and bleeds, with the violence on (`0x53F37C`):
/// none with a dialogue (`+0x245`), none that follows the hero (its master the local player); the
/// hero as any creature, of its type, the class 1 and its gib set, so that the heroes' types 1 to
/// 7 bleed none (`0x4281F0`) and the Dwarf (8) and the Daemon (9) spurt (#3401); in one case of
/// ten (`0x53F38F`), three of ten for one of the second knock-down's mark, which a death by a
/// combat art's strike gives it (`fight::KnockMarks`, #3673), at the bone of a part of six that its death rolls (`+0x574`, which the
/// original rolls as it makes the creature), the sixth none. A hero keeps its first fountain's
/// bone ([`Bled`]), its rise clearing none of `+0xB0`'s bits, so that its later fountains spurt
/// from it and its later puddles lie under it (`bodies`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn spurt(
    mut commands: Commands,
    settings: Option<Res<Settings>>,
    (gibs, hero): (Option<Res<Gibs>>, Option<Res<crate::hero::Hero>>),
    dying: Query<
        (
            Entity,
            Option<&crate::ai::Mind>,
            &GlobalTransform,
            (
                Has<crate::hero::Body>,
                Option<&Bled>,
                Has<crate::fight::Downed>,
                Option<&crate::fight::KnockMarks>,
            ),
        ),
        (
            Added<Dies>,
            Without<Talk>,
            Without<crate::followers::Follower>,
        ),
    >,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (mut dice, mut sounds, mut meshes): (ResMut<Dice>, MessageWriter<Play>, ResMut<Assets<Mesh>>),
) {
    if !settings.is_some_and(|settings| settings.gameplay.violence) {
        return;
    }
    for (entity, mind, at, (is_hero, bled, downed, marks)) in &dying {
        // Dead in its knock-down's fall, it is asked no clip and its fountain is not rolled
        // (`0x53EF2A`, #3465).
        if downed {
            continue;
        }
        let (creature, class) = match (mind, is_hero) {
            (_, true) => (
                hero.as_deref().map_or(0, |hero| hero.creature),
                opensacred_rules::minds::HEROES,
            ),
            (Some(mind), false) => (mind.creature, mind.class),
            (None, false) => continue,
        };
        let set = gibs.as_ref().map_or(0, |gibs| gibs.of(creature));
        let marked = marks.is_some_and(|marks| marks.second());
        if !particles::bleeds(creature, class, set)
            || !particles::spurts_at_death(&mut dice.0, marked)
        {
            continue;
        }
        // A hero's later fountains spurt from its first's bone.
        let name = match bled {
            Some(bled) => bled.0,
            None => {
                let part = dice.0.below(particles::BLOOD_PARTS);
                let Some(&name) = usize::try_from(part)
                    .ok()
                    .and_then(|part| particles::BLOOD_BONES.get(part))
                else {
                    continue;
                };
                name
            }
        };
        let Some(bone) = crate::making::bone_entity(entity, &[name], &children, (&bones, &worn))
        else {
            continue;
        };
        debug!("blood fountains: {entity}, a {creature} of the set {set}, bleeds from its {name}");
        commands.entity(entity).insert(Bled(name));
        commands.spawn((
            Fountain {
                fountain: BloodFountain::new(set),
                bone,
                last: None,
                mesh: meshes.add(empty()),
            },
            Transform::from_translation(at.translation()),
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        sounds.write(Play {
            sound: Of::Id(particles::blood_sound(&mut dice.0)),
            at: Some(entity),
            rate: (0.4, Roll::Share(0.2)),
        });
    }
}

/// Starts the fountains of the part 100 that the strikes call, with the violence on
/// (`0x4E8C54`–`0x4E8CFA`): each at the creature's place, at a bone of the bones' table's first 45
/// rolled anew (`rand() % 45`), in its gib set's colour, with no test of who bleeds; none once the
/// creature's head has spurted ([`Bled`] of the head, `+0xB0` bit `0x20`), and none where its
/// model lacks the bone rolled (**hypothesis**: the original's lies off the screen there).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn spurt_hits(
    mut commands: Commands,
    asked: Query<(Entity, &HitSpurts)>,
    settings: Option<Res<Settings>>,
    (gibs, hero): (Option<Res<Gibs>>, Option<Res<crate::hero::Hero>>),
    creatures: Query<(
        &GlobalTransform,
        Option<&crate::ai::Mind>,
        Has<crate::hero::Body>,
        Option<&Bled>,
    )>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (mut dice, mut sounds, mut meshes): (ResMut<Dice>, MessageWriter<Play>, ResMut<Assets<Mesh>>),
) {
    let violence = settings.is_some_and(|settings| settings.gameplay.violence);
    for (marker, &HitSpurts { creature, count }) in &asked {
        commands.entity(marker).despawn();
        if !violence {
            continue;
        }
        let Ok((at, mind, is_hero, bled)) = creatures.get(creature) else {
            continue;
        };
        if bled.is_some_and(|bled| bled.0 == particles::BLOOD_BONES[0]) {
            continue;
        }
        let kind = match (mind, is_hero) {
            (_, true) => hero.as_deref().map_or(0, |hero| hero.creature),
            (Some(mind), false) => mind.creature,
            (None, false) => continue,
        };
        let set = gibs.as_ref().map_or(0, |gibs| gibs.of(kind));
        for _ in 0..count {
            let roll = dice.0.below(particles::HIT_BLOOD_BONES);
            let Some(names) = usize::try_from(roll)
                .ok()
                .and_then(|index| crate::beams::BONES.get(index..=index))
            else {
                continue;
            };
            let Some(bone) =
                crate::making::bone_entity(creature, names, &children, (&bones, &worn))
            else {
                continue;
            };
            debug!(
                "blood fountains: {creature} spurts at a hit from its {}",
                names[0]
            );
            commands.spawn((
                Fountain {
                    fountain: BloodFountain::new(set),
                    bone,
                    last: None,
                    mesh: meshes.add(empty()),
                },
                Transform::from_translation(at.translation()),
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
            ));
            sounds.write(Play {
                sound: Of::Id(particles::blood_sound(&mut dice.0)),
                at: Some(creature),
                rate: (0.4, Roll::Share(0.2)),
            });
        }
    }
}

/// Gives each new fountain the drops' material, in the world's camera, which the night tints.
fn make(
    mut commands: Commands,
    fountains: Query<(Entity, &Fountain), Without<Children>>,
    data: Option<Res<Data>>,
    assets: Res<AssetServer>,
    (mut material, mut materials): (ResMut<Material>, ResMut<Assets<crate::fx::Effect>>),
) {
    for (entity, fountain) in &fountains {
        let made = material
            .0
            .get_or_insert_with(|| {
                let image = data.as_ref().and_then(|data| {
                    particle_texture(&data.0.assets, &assets, particles::BLOOD_TEXTURE)
                });
                if image.is_none() {
                    warn!(
                        "blood fountains: no texture {}; they are not drawn",
                        particles::BLOOD_TEXTURE
                    );
                }
                image.map(|image| {
                    materials.add(crate::fx::among_objects(particle_material(
                        image,
                        AlphaMode::Blend,
                    )))
                })
            })
            .clone();
        let Some(made) = made else {
            continue;
        };
        commands.entity(entity).with_children(|parts| {
            parts.spawn((
                Mesh3d(fountain.mesh.clone()),
                MeshMaterial3d(made),
                Transform::default(),
                bevy::camera::visibility::NoFrustumCulling,
            ));
        });
    }
}

/// Runs each fountain a frame on at its bone (`0x77BA90`): its drops leave along the bone's
/// axis, its own `x`, and die on the ground where the creature fell; a fountain ends as its
/// strength runs out, or as its bone goes.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut fountains: Query<(Entity, &mut Fountain, &Transform)>,
    places: Query<&GlobalTransform>,
    (mut dice, mut meshes, orders): (ResMut<Dice>, ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut fountain, transform) in &mut fountains {
        let Ok(bone) = places.get(fountain.bone) else {
            commands.entity(entity).despawn();
            continue;
        };
        let origin = transform.translation;
        let to = bolt_looks::models(bone.translation() - origin);
        let from = fountain.last.unwrap_or(to);
        let axis = Vec3::from(bolt_looks::models(bone.affine().transform_vector3(Vec3::X)))
            .normalize_or_zero();
        let Fountain {
            fountain: system,
            mesh,
            ..
        } = &mut *fountain;
        system.update(dt, (from, to), (axis.to_array(), 0.0), &mut dice.0);
        if system.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut quads = Quads::default();
        for particle in &system.system.particles {
            quads.push(
                bolt_looks::world(particle.place),
                particle.size * iso::MODEL_SCALE,
                particle.angle,
                (Vec2::ZERO, 1.0),
                system.ramp.at(particle.life),
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&*mesh) {
            // At its object's place in the order, as the original draws the system (#2188).
            quads.in_order(orders.at(origin), origin);
            quads.into_mesh(&mut mesh);
        }
        fountain.last = Some(to);
    }
}

/// The blood fountains of the creatures' deaths and of the strikes' open wounds.
pub struct BloodFountainsPlugin;

impl Plugin for BloodFountainsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Material>()
            .add_systems(Startup, read)
            .add_systems(
                Update,
                (spurt, spurt_hits, make, run)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
