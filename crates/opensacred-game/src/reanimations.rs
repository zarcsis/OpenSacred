//! The ring about a creature that a caster raises (924, `FX_REANIMATE`, #1092; docs/re/effects.md,
//! "The raised creature's ring"): a point turning once about its place leaves pale blue lines
//! standing on the ground, which fade over 2.55 s (`opensacred_rules::particles::Reanimate`),
//! added by their alpha and untinted by the night, lighting it by 200.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, Reanimate};

use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::spawning::Rng;
use crate::{bolt_looks, daynight, iso};

/// The lines' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x924))
    }
}

/// The lines' material, made at the first ring; none where the texture is missing.
#[derive(Resource, Default)]
struct Material(Option<Option<Handle<Effect>>>);

/// A ring to start at its place.
#[derive(Component)]
struct Unmade;

/// A ring: its system in the frame of its entity's place, and the mesh its lines go into.
#[derive(Component)]
struct Ring {
    ring: Reanimate,
    mesh: Handle<Mesh>,
}

/// Starts a ring at `at`, a raised creature's place (the FX event `0x84`).
pub fn ring_in(world: &mut World, at: Vec3) {
    world.spawn((
        Unmade,
        Transform::from_translation(at),
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
}

/// Gives each new ring its mesh, its material and its light.
fn make(
    mut commands: Commands,
    unmade: Query<Entity, With<Unmade>>,
    (data, assets): (Option<Res<Data>>, Res<AssetServer>),
    (mut made, mut materials, mut meshes): (
        ResMut<Material>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for entity in &unmade {
        let material = made
            .0
            .get_or_insert_with(|| {
                let image = data.as_ref().and_then(|data| {
                    particle_texture(&data.0.assets, &assets, particles::REANIMATE_TEXTURE)
                });
                if image.is_none() {
                    warn!(
                        "reanimations: no texture {}; the rings are not drawn",
                        particles::REANIMATE_TEXTURE
                    );
                }
                image.map(|image| materials.add(particle_material(image, AlphaMode::Add)))
            })
            .clone();
        let mesh = meshes.add(empty());
        let mut ring = commands.entity(entity);
        ring.remove::<Unmade>().insert((
            daynight::Light(particles::REANIMATE_LIGHT),
            Ring {
                ring: Reanimate::new(),
                mesh: mesh.clone(),
            },
        ));
        if let Some(material) = material {
            ring.with_children(|children| {
                children.spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                ));
            });
        }
    }
}

/// Runs each ring a frame on (`0x7AB590`, `0x7AB680`): its lines standing on their feet at an
/// eighth of their size, in the ramp's colour; it goes with its last line.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut rings: Query<(Entity, &mut Ring, &GlobalTransform)>,
    (mut dice, mut meshes, orders): (ResMut<Dice>, ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let ramp = Reanimate::ramp();
    for (entity, mut look, at) in &mut rings {
        let Ring { ring, mesh } = &mut *look;
        ring.update(dt, &mut dice.0);
        if ring.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut quads = Quads::default();
        for line in &ring.system.particles {
            quads.push_footed(
                bolt_looks::world(line.place),
                line.size * particles::REANIMATE_SIZE * s,
                0.0,
                (Vec2::ZERO, 1.0),
                ramp.at(line.life),
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&*mesh) {
            // At its object's place in the order, as the original draws the system (#2188).
            quads.in_order(orders.at(at.translation()), at.translation());
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The raised creatures' rings.
pub struct ReanimationsPlugin;

impl Plugin for ReanimationsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Material>()
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
