//! The blasts (794, `FX_EXPLOSION`, #1092; docs/re/effects.md, "The Dwarf's mines"): a Dwarf's
//! mine's and the Dark Elf's bomb trap's (`opensacred_rules::particles::Explosion`). For 0.8 s
//! flames rise and play their frames, the mine's of `PARTICLE_FIRE01`, the trap's of `FIRE04`,
//! added and untinted by the night; a scorch lies under them on the ground for 10 s and fades
//! over 5; the trap's ring spreads out from its place.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, Explosion};

use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::spawning::Rng;
use crate::{bolt_looks, iso};

/// How far over the ground the scorch and the ring lie, in world units, so that the ground's own
/// faces do not cut them.
const GROUND_LIFT: f32 = 0.5;

/// The index of the scorch among a blast's parts.
const SCORCH: usize = 1;

/// The blasts' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x794))
    }
}

/// The parts' materials by their textures, each made at the first blast that takes it: the two
/// flames', the two scorches' and the ring's; none where a texture is missing.
#[derive(Resource, Default)]
struct Materials(Option<[Option<Handle<Effect>>; 5]>);

/// A blast to start: the bomb trap's where true.
#[derive(Component)]
struct Unmade(bool);

/// A blast: its system in the frame of its place, and the meshes of its flames, its scorch and its
/// ring.
#[derive(Component)]
struct Blast {
    blast: Explosion,
    meshes: [Handle<Mesh>; 3],
}

/// Starts a blast at `at`, a mine's place on the ground, or the bomb trap's where `trap` (the
/// action 1 for the type 794, `0x5B2895`), its maker shaking the screen by 21 as it makes it (the
/// mine's run `0x7BAD70`, the trap's ball `0x79E730`).
pub fn blast(commands: &mut Commands, at: Vec3, trap: bool) {
    commands.queue(|world: &mut World| {
        if let Some(mut weather) = world.get_resource_mut::<crate::weather::Weather>() {
            weather.set(opensacred_rules::weather::Event::Quake(
                particles::MINE_QUAKE,
            ));
        }
    });
    commands.spawn((
        Unmade(trap),
        Transform::from_translation(at),
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
}

/// Gives each new blast its parts.
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    (data, assets): (Option<Res<Data>>, Res<AssetServer>),
    mut dice: ResMut<Dice>,
    (mut made, mut materials, mut meshes): (
        ResMut<Materials>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (entity, &Unmade(trap)) in &unmade {
        let made = made
            .0
            .get_or_insert_with(|| {
                let image = |name: &str| {
                    let image = data
                        .as_ref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("explosions: no texture {name}; its part is not drawn");
                    }
                    image
                };
                let [fire, trap_fire] = particles::EXPLOSION_FLAMES;
                let [scorch, other_scorch] = particles::EXPLOSION_SCORCHES;
                let mut added = |name| {
                    image(name).map(|image| materials.add(particle_material(image, AlphaMode::Add)))
                };
                let flames = [added(fire), added(trap_fire)];
                let mut laid = |name| {
                    image(name)
                        .map(|image| materials.add(particle_material(image, AlphaMode::Blend)))
                };
                let scorches = [laid(scorch), laid(other_scorch)];
                let ring = image(particles::EXPLOSION_RING)
                    .map(|image| materials.add(whole_material(image)));
                let [f0, f1] = flames;
                let [s0, s1] = scorches;
                [f0, f1, s0, s1, ring]
            })
            .clone();
        let blast = Explosion::new(trap, &mut dice.0);
        let parts: [Handle<Mesh>; 3] = std::array::from_fn(|_| meshes.add(empty()));
        let chosen = [
            made[blast.flames()].clone(),
            made[2 + blast.scorch].clone(),
            made[4].clone(),
        ];
        let mut shown = commands.entity(entity);
        shown.remove::<Unmade>();
        shown.with_children(|children| {
            for (k, (mesh, material)) in parts.iter().zip(chosen).enumerate() {
                let Some(material) = material else {
                    continue;
                };
                let mut part = children.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                ));
                // The scorch lies on the ground with the world, as the meteors' craters; the
                // flames and the ring are untinted.
                if k != SCORCH {
                    part.insert(RenderLayers::layer(iso::UNTINTED));
                }
            }
        });
        shown.insert(Blast {
            blast,
            meshes: parts,
        });
    }
}

/// Runs each blast a frame on (`0x779A50`, `0x779B60`); it goes with its scorch.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut blasts: Query<(Entity, &mut Blast, &GlobalTransform)>,
    (mut dice, mut meshes, orders): (ResMut<Dice>, ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let grid = particles::EXPLOSION_GRID;
    let side = f32::from(grid);
    let (across, down) = iso::fx_axes();
    let ground = Vec3::Y * GROUND_LIFT;
    for (entity, mut look, at) in &mut blasts {
        let Blast {
            blast,
            meshes: parts,
        } = &mut *look;
        blast.update(dt, &mut dice.0);
        if blast.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut flames = Quads::default();
        for flame in &blast.system.particles {
            let frame = particles::age_frame(flame.life, grid);
            let corner = Vec2::new(f32::from(frame % grid), f32::from(frame / grid)) / side;
            flames.push(
                bolt_looks::world(flame.place),
                flame.size * s,
                flame.angle,
                (corner, 1.0 / side),
                [1.0, 1.0, 1.0, flame.life.clamp(0.0, 255.0) / 255.0],
            );
        }
        let mut scorch = Quads::default();
        let (sin, cos) = blast.scorch_turn.sin_cos();
        let half = particles::EXPLOSION_SCORCH_HALF * s;
        let (a, b) = (
            (across * cos + down * sin) * half,
            (down * cos - across * sin) * half,
        );
        scorch.push_corners(
            [
                ground - a - b,
                ground + a - b,
                ground + a + b,
                ground - a + b,
            ],
            [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
            [1.0, 1.0, 1.0, blast.fade],
        );
        let mut ring = Quads::default();
        if let Some(inner) = blast.ring() {
            ring.push_ring(
                ground,
                (inner * s, (inner + particles::EXPLOSION_RING_WIDTH) * s),
                particles::EXPLOSION_RING_SEGMENTS,
                [1.0; 4],
            );
        }
        // At its object's place in the order, as the original draws the system (#2188).
        let origin = at.translation();
        let order = orders.at(origin);
        for (handle, mut part) in parts.iter().zip([flames, scorch, ring]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.in_order(order, origin);
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The blasts.
pub struct ExplosionsPlugin;

impl Plugin for ExplosionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Materials>()
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
