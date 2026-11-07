//! The burning bones (788, `FX_BURNING_BONE`, #2061; docs/re/effects.md, "The arrows'
//! elements"): a hero's burning arrow sets the head of what it hits burning for half a second
//! (`opensacred_rules::particles::BurningBone`), white flames rising from it, added whole and
//! untinted by the night, lighting it; one burns on a creature at a time.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, BurningBone};

use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::spawning::Rng;
use crate::{bolt_looks, daynight, iso};

/// The bone it keeps to (the FX event `0x14`'s bone 20).
const HEAD: [&str; 1] = ["Bip01 Head"];

/// The flames' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x788))
    }
}

/// The flames' material, made at the first burning; none where the texture is missing.
#[derive(Resource, Default)]
struct Material(Option<Option<Handle<Effect>>>);

/// A burning to start on its target.
#[derive(Component)]
struct Unmade(Entity);

/// A bone that burns: its flames in the frame of its entity's place, its creature and the bone,
/// where the bone was a frame ago, and the mesh its flames go into.
#[derive(Component)]
struct Burning {
    bone: BurningBone,
    target: Entity,
    head: Option<Entity>,
    last: Option<[f32; 3]>,
    mesh: Handle<Mesh>,
}

/// Sets `target`'s head burning, unless one burns there already (`0x5FA150`).
pub fn burn(commands: &mut Commands, target: Entity) {
    commands.spawn(Unmade(target));
}

/// Starts each burning asked for on a target that has none, at its place.
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    burning: Query<&Burning>,
    places: Query<&GlobalTransform>,
    (data, assets): (Option<Res<Data>>, Res<AssetServer>),
    (mut made, mut materials, mut meshes): (
        ResMut<Material>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (entity, &Unmade(target)) in &unmade {
        commands.entity(entity).despawn();
        let Ok(at) = places.get(target) else {
            continue;
        };
        if burning.iter().any(|burning| burning.target == target) {
            continue;
        }
        let material = made
            .0
            .get_or_insert_with(|| {
                let image = data.as_ref().and_then(|data| {
                    particle_texture(&data.0.assets, &assets, particles::BURNING_BONE_TEXTURE)
                });
                if image.is_none() {
                    warn!(
                        "burning bones: no texture {}; they are not drawn",
                        particles::BURNING_BONE_TEXTURE
                    );
                }
                image.map(|image| materials.add(whole_material(image)))
            })
            .clone();
        debug!("burning bones: {target} burns");
        let mesh = meshes.add(empty());
        let mut shown = commands.spawn((
            Transform::from_translation(at.translation()),
            Visibility::default(),
            daynight::Light(particles::BURNING_BONE_LIGHT),
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(material) = material {
            shown.with_children(|children| {
                children.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                ));
            });
        }
        shown.insert(Burning {
            bone: BurningBone::new(0.0, 0),
            target,
            head: None,
            last: None,
            mesh,
        });
    }
}

/// Burns each bone a frame on at its creature's head (`0x7754B0`), its flames rising in the world;
/// it goes with its last flame, or as its creature goes.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut burning: Query<(Entity, &mut Burning, &Transform)>,
    places: Query<&GlobalTransform>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (mut dice, mut meshes, orders): (ResMut<Dice>, ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let grid = particles::BURNING_BONE_GRID;
    for (entity, mut fire, place) in &mut burning {
        if places.get(fire.target).is_err() {
            commands.entity(entity).despawn();
            continue;
        }
        if fire.head.is_none() {
            fire.head = crate::making::bone_entity(fire.target, &HEAD, &children, (&bones, &worn));
        }
        let at = fire
            .head
            .and_then(|head| places.get(head).ok())
            .or_else(|| places.get(fire.target).ok())
            .map(GlobalTransform::translation)
            .unwrap_or(place.translation);
        let to = bolt_looks::models(at - place.translation);
        let from = fire.last.unwrap_or(to);
        let Burning { bone, mesh, .. } = &mut *fire;
        bone.update(dt, (from, to), &mut dice.0);
        if bone.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let grey = bone.grey;
        let mut quads = Quads::default();
        for particle in &bone.system.particles {
            let frame = particles::age_frame(particle.life, grid);
            let side = f32::from(grid);
            let corner = Vec2::new(f32::from(frame % grid), f32::from(frame / grid)) / side;
            quads.push(
                bolt_looks::world(particle.place),
                particle.size * s,
                particle.angle,
                (corner, 1.0 / side),
                [grey, grey, grey, 0.0],
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&*mesh) {
            // Just behind the creature it burns, as the original draws the system before the
            // creature (#2188).
            quads.in_order(orders.behind(fire.target), place.translation);
            quads.into_mesh(&mut mesh);
        }
        fire.last = Some(to);
    }
}

/// The burning bones.
pub struct BurningBonesPlugin;

impl Plugin for BurningBonesPlugin {
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
