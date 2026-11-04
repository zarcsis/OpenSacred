//! The look of the Daemon's ring of poison 1101 (#1092; docs/re/effects.md, "The ring of poison"):
//! where he makes it, `cParticleSystem_ringofpoison` lies for 3 s (1008, `0x7AF550`): its burst of
//! sparks (`opensacred_rules::particles::ring_sparks`, by `puffs`) and a screen's quake of 30 at
//! its start, its green ring spreading over the ground for 0.75 s (`poison_ring`), its two twirls
//! turning on the ground and shrinking (`poison_twirls`), both added whole and untinted by the
//! night, and its light of 200 at it.

use std::f32::consts::TAU;

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, poison_ring, poison_twirls};

use crate::frame::Playing;
use crate::fx::{Effect, Quads, empty, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::{daynight, iso};

/// How far over the ground the ring and the twirls lie, in world units, so that the ground's own
/// faces do not cut them.
const GROUND_LIFT: f32 = 0.5;

/// A ring of poison: its clock, its place on the ground, its lamp, and its parts' meshes, the
/// ring's and the twirls'.
#[derive(Component)]
struct Ring {
    clock: f32,
    place: Vec3,
    lamp: Entity,
    parts: [Handle<Mesh>; 2],
}

/// The ring's and the twirls' materials, made as the first ring is; none where a texture is
/// missing.
#[derive(Resource, Default)]
struct Materials(Option<[Option<Handle<Effect>>; 2]>);

/// A ring of poison to make at the place on the ground `at` (`making`).
#[derive(Message)]
pub struct Made {
    pub at: Vec3,
}

/// The rings' dice of their sparks.
#[derive(Resource)]
struct Rolls(opensacred_rules::spawning::Rng);

impl Default for Rolls {
    fn default() -> Self {
        Self(crate::seed::dice(0x3F0))
    }
}

/// Makes each ring asked for: its sparks, its quake, its lamp and its parts (`0x7AF840`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn make(
    mut commands: Commands,
    mut made: MessageReader<Made>,
    mut rolls: ResMut<Rolls>,
    (data, assets, mut made_materials, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Materials>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for &Made { at } in made.read() {
        let parts_materials = made_materials
            .0
            .get_or_insert_with(|| {
                [
                    particles::POISON_RING_TEXTURE,
                    particles::POISON_TWIRL_TEXTURE,
                ]
                .map(|texture| {
                    let image = data
                        .as_ref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, texture));
                    if image.is_none() {
                        warn!("poison rings: no texture {texture}; its part is not drawn");
                    }
                    image.map(|image| materials.add(whole_material(image)))
                })
            })
            .clone();
        crate::puffs::spawn(
            &mut commands,
            particles::ring_sparks(&mut rolls.0),
            at,
            None,
        );
        commands.queue(|world: &mut World| {
            if let Some(mut weather) = world.get_resource_mut::<crate::weather::Weather>() {
                weather.set(opensacred_rules::weather::Event::Quake(
                    particles::POISON_RING_QUAKE,
                ));
            }
        });
        let lamp = commands
            .spawn((
                Transform::from_translation(at.with_y(0.0)),
                daynight::Light(particles::POISON_RING_LIGHT),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let parts = [meshes.add(empty()), meshes.add(empty())];
        let ring = commands
            .spawn((
                Transform::IDENTITY,
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        for (mesh, material) in parts.iter().zip(parts_materials) {
            if let Some(material) = material {
                commands.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                    ChildOf(ring),
                ));
            }
        }
        commands.entity(ring).insert(Ring {
            clock: 0.0,
            place: at,
            lamp,
            parts,
        });
        debug!("poison rings: {ring} at {:.0},{:.0}", at.x, at.z);
    }
}

/// Runs each ring a frame on (`0x7AF550`, `0x7AF650`): its ring and its twirls by its clock; at
/// 3 s it goes, its lamp with it.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut rings: Query<(Entity, &mut Ring)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let s = iso::MODEL_SCALE;
    for (entity, mut ring) in &mut rings {
        ring.clock += time.delta_secs();
        if ring.clock >= particles::POISON_RING_LIVES {
            commands.entity(ring.lamp).despawn();
            commands.entity(entity).despawn();
            continue;
        }
        let ground = ring.place + Vec3::Y * GROUND_LIFT;
        let mut band = Quads::default();
        if let Some((inner, outer, colour)) = poison_ring(ring.clock) {
            let segments = particles::POISON_RING_SEGMENTS;
            let edge = |k: u16, radius: f32| {
                let angle = TAU * f32::from(k) / f32::from(segments);
                ground + Vec3::new(angle.sin(), 0.0, angle.cos()) * radius * s
            };
            for k in 0..segments {
                band.push_corners(
                    [
                        edge(k, inner),
                        edge(k, outer),
                        edge(k + 1, outer),
                        edge(k + 1, inner),
                    ],
                    [Vec2::ZERO, Vec2::Y, Vec2::ONE, Vec2::X],
                    colour,
                );
            }
        }
        let mut twirls = Quads::default();
        for (half, angle) in poison_twirls(ring.clock).into_iter().flatten() {
            let (sin, cos) = angle.sin_cos();
            let corner = |x: f32, y: f32| {
                // The models' frame turned about its upright, its y the world's z.
                ground + Vec3::new(x * cos - y * sin, 0.0, x * sin + y * cos) * half * s
            };
            twirls.push_corners(
                [
                    corner(-1.0, -1.0),
                    corner(1.0, -1.0),
                    corner(1.0, 1.0),
                    corner(-1.0, 1.0),
                ],
                [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
                particles::POISON_TWIRL_COLOUR,
            );
        }
        for (handle, quads) in ring.parts.iter().zip([band, twirls]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The Daemon's rings of poison.
pub struct PoisonRingsPlugin;

impl Plugin for PoisonRingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Made>()
            .init_resource::<Materials>()
            .init_resource::<Rolls>()
            .add_systems(
                Update,
                (make, run)
                    .chain()
                    .in_set(Playing)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
