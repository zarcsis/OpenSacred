//! The mortar shells' impacts (1020, `FX_GRENADEIMPACT`, #2160; docs/re/effects.md, "The mortar's
//! impact"): where the Dwarf's shell lands (`guns`), the screen shakes, a flash added and a dark
//! cloud laid over grow as they fade, 75 tongues of fire flare up and out, sparks burst up and
//! fall bouncing, all for about a second, and a crater lies under them on the ground for 10 s and
//! fades over 4 (`opensacred_rules::particles::GrenadeImpact`). The crater and the tongues are
//! tinted by the night, the flash, the cloud and the sparks not.

use std::collections::HashMap;

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    GRENADE_CRATER_HALF, GRENADE_CRATERS, GRENADE_EXPLOSIONS, GRENADE_QUAKE, GrenadeImpact,
};

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;

/// How far over the ground the flat parts lie, in world units.
const GROUND_LIFT: f32 = 0.5;

/// Its parts, each a mesh: the crater, the flash, the cloud, the tongues, the sparks added and the
/// sparks laid over.
const PARTS: usize = 6;

/// An impact to make at its entity's place ([`make`]).
#[derive(Component)]
struct Unmade;

/// An impact: its look and the meshes of its parts.
#[derive(Component)]
struct Impact {
    look: GrenadeImpact,
    meshes: [Handle<Mesh>; PARTS],
}

/// The impacts' materials by their texture and whether laid over; none where a texture is missing.
#[derive(Resource, Default)]
struct Looks(HashMap<(&'static str, bool), Option<Handle<Effect>>>);

/// The impacts' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x1020))
    }
}

/// Starts an impact at `at`, the shell's point on the ground, the screen shaking by 8 pixels as it
/// starts (`0x7B70A0`).
pub(crate) fn land(commands: &mut Commands, at: Vec3) {
    commands.queue(|world: &mut World| {
        if let Some(mut weather) = world.get_resource_mut::<crate::weather::Weather>() {
            weather.set(opensacred_rules::weather::Event::Quake(GRENADE_QUAKE));
        }
    });
    commands.spawn((
        Unmade,
        Transform::from_translation(at),
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
}

/// Gives each new impact its look, its meshes and their materials (`0x7B67E0`).
fn make(
    mut commands: Commands,
    unmade: Query<Entity, With<Unmade>>,
    (data, assets, mut looks, mut dice): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Dice>,
    ),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for entity in &unmade {
        let look = GrenadeImpact::new(&mut dice.0);
        let mut material = |name: &'static str, over: bool| {
            looks
                .0
                .entry((name, over))
                .or_insert_with(|| {
                    let image = data
                        .as_ref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("grenade impacts: no texture {name}; its part is not drawn");
                    }
                    let mode = if over {
                        AlphaMode::Blend
                    } else {
                        AlphaMode::Add
                    };
                    image.map(|image| effects.add(particle_material(image, mode)))
                })
                .clone()
        };
        let [cloud, flash, sparks, tongues] = GRENADE_EXPLOSIONS;
        let crater = GRENADE_CRATERS[look.crater.min(GRENADE_CRATERS.len() - 1)];
        // The crater and the tongues lie with the world, tinted by the night.
        let parts = [
            (material(crater, true), true),
            (material(flash, false), false),
            (material(cloud, true), false),
            (material(tongues, true), true),
            (material(sparks, false), false),
            (material(sparks, true), false),
        ];
        let meshes_of: [Handle<Mesh>; PARTS] = std::array::from_fn(|_| meshes.add(empty()));
        let mut made = commands.entity(entity);
        made.remove::<Unmade>().insert(Impact {
            look,
            meshes: meshes_of.clone(),
        });
        for (mesh, (material, tinted)) in meshes_of.into_iter().zip(parts) {
            let Some(material) = material else {
                continue;
            };
            made.with_child((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
            ))
            .insert_if(RenderLayers::layer(iso::UNTINTED), || !tinted);
        }
    }
}

/// A square flat on the ground about the impact's place of the half side `half` in the models'
/// units, the whole texture, in `colour`.
fn flat(quads: &mut Quads, half: f32, colour: [f32; 4]) {
    let (across, down) = iso::fx_axes();
    let (a, d) = (
        across * half * iso::MODEL_SCALE,
        down * half * iso::MODEL_SCALE,
    );
    let ground = Vec3::Y * GROUND_LIFT;
    quads.push_corners(
        [
            ground - a - d,
            ground + a - d,
            ground + a + d,
            ground - a + d,
        ],
        [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
        colour,
    );
}

/// Runs each impact a frame on (`0x7B6BE0`, `0x7B6DD0`): its crater, and while it bursts its
/// flash, its cloud, its tongues scaled about its place and its sparks facing the camera, each of
/// its layer's frame of the 2 × 2 grid, the first layer's added and the others laid over; it goes
/// once its crater has faded.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut impacts: Query<(Entity, &mut Impact, &GlobalTransform)>,
    (mut meshes, mut dice, orders): (ResMut<Assets<Mesh>>, ResMut<Dice>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let ramps = GrenadeImpact::ramps();
    for (entity, mut impact, at) in &mut impacts {
        let impact = &mut *impact;
        impact.look.update(dt, &mut dice.0);
        if impact.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut parts: [Quads; PARTS] = std::array::from_fn(|_| Quads::default());
        let [crater, flash, cloud, tongues, added, over] = &mut parts;
        flat(
            crater,
            GRENADE_CRATER_HALF,
            [1.0, 1.0, 1.0, impact.look.crater_alpha()],
        );
        if impact.look.bursting() {
            if let Some((half, alpha)) = impact.look.flash() {
                flat(flash, half, [1.0, 1.0, 1.0, alpha]);
            }
            if let Some((half, alpha)) = impact.look.cloud() {
                flat(cloud, half, [1.0, 1.0, 1.0, alpha]);
            }
            let (scale, alpha) = impact.look.tongue();
            for tongue in &impact.look.tongues {
                let corners = tongue.corners.map(|[x, y, z]| {
                    world([x * scale, y * scale, z * scale]) + Vec3::Y * GROUND_LIFT
                });
                tongues.push_corners(corners, tongue.uvs.map(Vec2::from), [1.0, 1.0, 1.0, alpha]);
            }
            for particle in &impact.look.system.particles {
                let Some(ramp) = ramps.get(usize::from(particle.layer)) else {
                    continue;
                };
                let cell = Vec2::new(
                    f32::from(particle.frame % 2),
                    f32::from(particle.frame / 2 % 2),
                ) * 0.5;
                let quads = if particle.layer == 0 {
                    &mut *added
                } else {
                    &mut *over
                };
                quads.push(
                    world(particle.place),
                    particle.size * s,
                    particle.angle,
                    (cell, 0.5),
                    ramp.at(particle.life),
                );
            }
        }
        // At its object's place in the order, as the original draws the system (#2188).
        let origin = at.translation();
        let order = orders.at(origin);
        for (handle, mut quads) in impact.meshes.iter().zip(parts) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                quads.in_order(order, origin);
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The mortar shells' impacts.
pub struct GrenadeImpactsPlugin;

impl Plugin for GrenadeImpactsPlugin {
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
