//! The Wood Elf's speed's sight (37, 846, `cParticleSystem_highspeed`; #1092; docs/re/effects.md,
//! "The Wood Elf's sights"): two systems at the bearer's root, riding with it for the spell's
//! seconds, each laying squares flat on the ground about its feet, turned at random and added
//! whole in the ramp of their life: green twirls flashing through yellow, cyan and blue, over a
//! dimmer layer of dot discs; each with its green light and its loop 2010, untinted by the night
//! (`opensacred_rules::particles::SpeedSight`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{SPEED_TEXTURES, SpeedSight};

use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::spawning::Rng;
use crate::{fx_lights, iso};

/// The loop of each of the two systems (`MAGE_PENTAGRAMM_LOOP`, `0x79AB70`).
const LOOP: u32 = 2010;

/// A system to make: its variant, its bearer and its seconds.
#[derive(Component)]
struct Unmade {
    variant: usize,
    bearer: Entity,
    seconds: f32,
}

/// A system that shows: its bearer, its look and its mesh.
#[derive(Component)]
struct Shown {
    bearer: Entity,
    look: SpeedSight,
    mesh: Handle<Mesh>,
}

/// The two systems' materials, added whole, by their variant; none where a texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<[Option<Handle<Effect>>; 2]>);

/// The systems' dice (`rand()`).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x846))
    }
}

/// Shows the speed's two systems about `bearer` for `seconds`, the spell's value 0 at its level.
pub(crate) fn show(commands: &mut Commands, bearer: Entity, seconds: f32) {
    for variant in 0..2 {
        commands.spawn((
            Unmade {
                variant,
                bearer,
                seconds,
            },
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// Gives each new system its look at its bearer's place, its mesh, its material, its light and
/// its loop (`0x79A750`, `0x79AB70`).
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    places: Query<&GlobalTransform>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, unmade) in &unmade {
        let Ok(at) = places.get(unmade.bearer) else {
            commands.entity(entity).despawn();
            continue;
        };
        let materials = looks
            .0
            .get_or_insert_with(|| {
                SPEED_TEXTURES.map(|name| {
                    let image = data
                        .as_ref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("speed sights: no texture {name}; its system is not drawn");
                    }
                    image.map(|image| effects.add(whole_material(image)))
                })
            })
            .clone();
        let mesh = meshes.add(empty());
        // Its green light at the bearer's root, which it moves at each draw.
        commands.spawn(fx_lights::child(fx_lights::SOUL_GREEN, entity));
        let mut made = commands.entity(entity);
        made.remove::<Unmade>().insert((
            Shown {
                bearer: unmade.bearer,
                look: SpeedSight::new(unmade.variant, unmade.seconds),
                mesh: mesh.clone(),
            },
            Transform::from_translation(at.translation()),
            Visibility::default(),
            crate::hums::Hum(LOOP),
        ));
        if let Some(material) = materials.get(unmade.variant).cloned().flatten() {
            made.with_child((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
            ));
        }
    }
}

/// Runs each system a frame on (`0x79A840`) and draws it (`0x79A8F0`): at its bearer's root, each
/// particle a square of its half side flat on the ground at its place about the root, turned
/// about the upright by its angle, in the ramp's colour of its life; it goes at once with its
/// seconds, or with its bearer.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut systems: Query<(Entity, &mut Shown, &mut Transform)>,
    places: Query<&GlobalTransform>,
    (mut meshes, mut dice): (ResMut<Assets<Mesh>>, ResMut<Dice>),
) {
    let dt = time.delta_secs();
    let s = iso::MODEL_SCALE;
    let (across, depth) = iso::fx_axes();
    let frame = |[x, y, z]: [f32; 3]| (across * x + depth * y + Vec3::Y * z) * s;
    for (entity, mut shown, mut transform) in &mut systems {
        let shown = &mut *shown;
        let Ok(place) = places.get(shown.bearer).map(GlobalTransform::translation) else {
            commands.entity(entity).despawn();
            continue;
        };
        shown.look.update(dt, &mut dice.0);
        if shown.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation = place;
        let mut quads = Quads::default();
        for particle in &shown.look.system.particles {
            let [px, py, pz] = particle.place;
            let (sin, cos) = particle.angle.sin_cos();
            let half = particle.size;
            let corner =
                |x: f32, y: f32| frame([px + x * cos + y * sin, py - x * sin + y * cos, pz]);
            quads.push_corners(
                [
                    corner(-half, -half),
                    corner(half, -half),
                    corner(half, half),
                    corner(-half, half),
                ],
                [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
                shown.look.colour(particle.life),
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&shown.mesh) {
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The speed's sight.
pub struct SpeedSightsPlugin;

impl Plugin for SpeedSightsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>()
            .init_resource::<Dice>()
            .add_systems(
                Update,
                (make, run)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
