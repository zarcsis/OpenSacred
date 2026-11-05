//! The blood fountains (908, `FX_BLOODFONTAIN`, #1964; docs/re/effects.md, "The blood
//! fountain"): under `[gameplay] violence`, the original's `VIOLENCE`, a creature that bleeds
//! spurts blood from one of five bones in one death of ten, with `BLEEDING01` or `02` at it
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
/// none with a dialogue (`+0x245`), none that follows the hero (its master the local player) and
/// none of the hero; in one case of ten (`0x53F38F`), at the bone of a part of six that its death
/// rolls (`+0x574`, which the original rolls as it makes the creature), the sixth none.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn spurt(
    mut commands: Commands,
    settings: Option<Res<Settings>>,
    gibs: Option<Res<Gibs>>,
    dying: Query<
        (Entity, &crate::ai::Mind, &GlobalTransform),
        (
            Added<Dies>,
            Without<Talk>,
            Without<crate::followers::Follower>,
            Without<crate::hero::Body>,
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
    for (entity, mind, at) in &dying {
        let set = gibs.as_ref().map_or(0, |gibs| gibs.of(mind.creature));
        if !particles::bleeds(mind.creature, mind.class, set)
            || !particles::spurts_at_death(&mut dice.0)
        {
            continue;
        }
        let part = dice.0.below(particles::BLOOD_PARTS);
        let Some(name) = usize::try_from(part)
            .ok()
            .and_then(|part| particles::BLOOD_BONES.get(part))
        else {
            continue;
        };
        let Some(bone) = crate::making::bone_entity(entity, &[name], &children, (&bones, &worn))
        else {
            continue;
        };
        debug!(
            "blood fountains: {entity}, a {} of the set {set}, bleeds from its {name}",
            mind.creature
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
            at: Some(entity),
            rate: (0.4, Roll::Share(0.2)),
        });
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
                image.map(|image| materials.add(particle_material(image, AlphaMode::Blend)))
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
    (mut dice, mut meshes): (ResMut<Dice>, ResMut<Assets<Mesh>>),
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
            quads.into_mesh(&mut mesh);
        }
        fountain.last = Some(to);
    }
}

/// The blood fountains of the creatures' deaths.
pub struct BloodFountainsPlugin;

impl Plugin for BloodFountainsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Material>()
            .add_systems(Startup, read)
            .add_systems(
                Update,
                (spurt, make, run)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
