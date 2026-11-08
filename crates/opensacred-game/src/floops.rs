//! A trampling horse's floops (#2329; docs/re/horses.md, "Fighting from horseback"): as its trample
//! starts, the effect 24's adder makes an object 842 at the horse, of the sight 1077, standing at
//! the horse at each frame (`opensacred_rules::particles::HorseFloops`): red tongues spread flat on
//! the ground about its feet while the trample runs, added and untinted, as the Dwarf's mines'
//! sign spreads its blue ones (`mines`). Its light of 200 is not made.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{HorseFloops, MINE_SIGN_TEXTURE};

use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::spawning::Rng;
use crate::trample::Trampling;
use crate::{bolt_looks, iso};

/// How far over the ground the tongues lie, in world units.
const GROUND_LIFT: f32 = 0.5;

/// The floops' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x1077))
    }
}

/// The tongues' material, made at the first floops, none where its texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<Option<Handle<Effect>>>);

/// A horse's floops, on an entity riding with it: its horse, its system and the mesh its tongues
/// go into.
#[derive(Component)]
struct Floops {
    horse: Entity,
    look: HorseFloops,
    mesh: Handle<Mesh>,
}

/// Gives each horse whose trample starts its floops, riding with it.
fn start(
    mut commands: Commands,
    horses: Query<Entity, Added<Trampling>>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for horse in &horses {
        let material = looks
            .0
            .get_or_insert_with(|| {
                let image = data
                    .as_deref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, MINE_SIGN_TEXTURE));
                if image.is_none() {
                    warn!("floops: no texture {MINE_SIGN_TEXTURE}; the floops are not drawn");
                }
                image.map(|image| effects.add(particle_material(image, AlphaMode::Add)))
            })
            .clone();
        let mesh = meshes.add(empty());
        let mut floops = commands.spawn((
            Transform::IDENTITY,
            Visibility::default(),
            Floops {
                horse,
                look: HorseFloops::new(),
                mesh: mesh.clone(),
            },
            ChildOf(horse),
        ));
        if let Some(material) = material {
            floops.with_child((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
            ));
        }
    }
}

/// Runs each horse's floops a frame on (`0x78EA70`, `0x78ED70`): its tongues lie flat on the
/// ground, each standing on its foot on the circle and pointing out by its turn; new ones come
/// while the horse tramples, and the floops go once the last tongue dies.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    tramplers: Query<(), With<Trampling>>,
    mut floops: Query<(Entity, &mut Floops)>,
    (mut dice, mut meshes): (ResMut<Dice>, ResMut<Assets<Mesh>>),
) {
    let dt = time.delta_secs().min(0.1);
    let ramp = HorseFloops::ramp();
    let lift = Vec3::Y * GROUND_LIFT;
    for (entity, mut floops) in &mut floops {
        let on = tramplers.contains(floops.horse);
        let Floops { look, mesh, .. } = &mut *floops;
        look.update(dt, on, &mut dice.0);
        if !on && look.system.particles.is_empty() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut quads = Quads::default();
        for tongue in &look.system.particles {
            let (sin, cos) = tongue.angle.sin_cos();
            let [x, y, _] = tongue.place;
            let size = tongue.size;
            let at = |across: f32, out: f32| {
                bolt_looks::world([
                    x + cos * across + sin * out,
                    y - sin * across + cos * out,
                    0.0,
                ]) + lift
            };
            quads.push_corners(
                [
                    at(-size, 0.0),
                    at(size, 0.0),
                    at(size, 2.0 * size),
                    at(-size, 2.0 * size),
                ],
                [Vec2::Y, Vec2::ONE, Vec2::X, Vec2::ZERO],
                ramp.at(tongue.life),
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&*mesh) {
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The horses' floops.
pub struct FloopsPlugin;

impl Plugin for FloopsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (start, run)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
