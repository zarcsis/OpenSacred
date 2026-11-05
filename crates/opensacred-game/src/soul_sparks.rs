//! The look of the souls that fly to the Dark Elf's soulcatcher (780, `FX_PUZZLE_SOLVED`, #1092;
//! docs/re/effects.md, "The souls"): the dying creature's figure in pale green sparks, one at
//! each of its vertices taken as it dies, rising and streaming ever faster to its soul as it
//! flies to him, added whole and untinted; where it arrives or a cell stops it its sparks stop
//! and fade out (`opensacred_rules::particles::SoulSparks`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::mesh::VertexAttributeValues;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::prelude::*;
use opensacred_rules::particles::{self, SOUL_SPARKS, SoulSparks};

use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;

/// A soul, which takes its look ([`show`]): the creature whose figure it takes.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Sparks {
    pub(crate) from: Entity,
}

/// The sparks' material, made at the first soul; none where its texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<Option<Handle<Effect>>>);

/// A soul's look, which outlives its soul while its sparks fade: its soul while it flies, its
/// system, its place and its mesh.
#[derive(Component)]
struct Shown {
    soul: Option<Entity>,
    look: SoulSparks,
    place: Vec3,
    mesh: Handle<Mesh>,
}

/// A mesh of a creature's model: its mesh, its joints and its place.
type ModelMesh = (
    &'static Mesh3d,
    Option<&'static SkinnedMesh>,
    &'static GlobalTransform,
);

/// The places of the vertices of `root`'s meshes but its worn items', skinned to its pose, every
/// `n`-th of each mesh for `n` the count of all over [`SOUL_SPARKS`] (`0x767FB0`, `0x44FC30`).
fn figure(
    root: Entity,
    children: &Query<&Children>,
    (parts, joints, worn): (
        &Query<ModelMesh>,
        &Query<&GlobalTransform>,
        &crate::making::Worn,
    ),
    (meshes, poses): (&Assets<Mesh>, &Assets<SkinnedMeshInverseBindposes>),
) -> Vec<Vec3> {
    let mut found = Vec::new();
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if entity != root && worn.contains(entity) {
            continue;
        }
        if let Ok((mesh, skin, place)) = parts.get(entity)
            && let Some(mesh) = meshes.get(&mesh.0)
            && let Some(VertexAttributeValues::Float32x3(positions)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        {
            found.push((mesh, positions, skin, place));
        }
        if let Ok(kids) = children.get(entity) {
            stack.extend(kids.iter());
        }
    }
    let total: usize = found.iter().map(|(_, positions, ..)| positions.len()).sum();
    let every = (total / SOUL_SPARKS).max(1);
    let mut points = Vec::new();
    for (mesh, positions, skin, place) in found {
        let skinning = skin.and_then(|skin| {
            let poses = poses.get(&skin.inverse_bindposes)?;
            let matrices: Vec<Mat4> = skin
                .joints
                .iter()
                .zip(poses.iter())
                .map(|(joint, pose)| {
                    joints
                        .get(*joint)
                        .map_or(Mat4::IDENTITY, |joint| joint.to_matrix() * *pose)
                })
                .collect();
            let Some(VertexAttributeValues::Uint16x4(indices)) =
                mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX)
            else {
                return None;
            };
            let Some(VertexAttributeValues::Float32x4(weights)) =
                mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT)
            else {
                return None;
            };
            Some((matrices, indices, weights))
        });
        for k in (0..positions.len()).step_by(every) {
            let local = Vec3::from_array(positions[k]);
            let at = match &skinning {
                Some((matrices, indices, weights)) => {
                    let (Some(index), Some(weight)) = (indices.get(k), weights.get(k)) else {
                        continue;
                    };
                    index
                        .iter()
                        .zip(weight)
                        .filter_map(|(&joint, &weight)| {
                            matrices
                                .get(usize::from(joint))
                                .map(|matrix| matrix.transform_point3(local) * weight)
                        })
                        .sum()
                }
                None => place.transform_point(local),
            };
            points.push(at);
        }
    }
    points
}

/// Gives each new soul its look (`0x5A46B3`, `0x76CD50`): the sparks of its creature's figure.
fn show(
    mut commands: Commands,
    souls: Query<(Entity, &Sparks, &Transform), Added<Sparks>>,
    (children, parts, joints, worn): (
        Query<&Children>,
        Query<ModelMesh>,
        Query<&GlobalTransform>,
        crate::making::Worn,
    ),
    poses: Res<Assets<SkinnedMeshInverseBindposes>>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (soul, sparks, place) in &souls {
        let material = looks
            .0
            .get_or_insert_with(|| {
                let image = data.as_deref().and_then(|data| {
                    particle_texture(&data.0.assets, &assets, particles::SOUL_SPARK_TEXTURE)
                });
                if image.is_none() {
                    warn!(
                        "soul sparks: no texture {}; the souls are not drawn",
                        particles::SOUL_SPARK_TEXTURE
                    );
                }
                image.map(|image| effects.add(whole_material(image)))
            })
            .clone();
        let points = figure(
            sparks.from,
            &children,
            (&parts, &joints, &worn),
            (&meshes, &poses),
        );
        debug!(
            "soul sparks: {soul} takes {} sparks of {}",
            points.len(),
            sparks.from
        );
        let look = SoulSparks::new(points.into_iter().map(models));
        let mesh = meshes.add(empty());
        let mut shown = commands.spawn((
            Transform::IDENTITY,
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(material) = material {
            shown.with_child((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
            ));
        }
        shown.insert(Shown {
            soul: Some(soul),
            look,
            place: place.translation,
            mesh,
        });
    }
}

/// Runs each soul's look a frame on (`0x76C9D0`, `0x76CB10`): its sparks drawn to its soul as it
/// flies, their stop where it ends, and its end with its last spark.
fn stream(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown)>,
    souls: Query<&Transform, With<Sparks>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut look) in &mut shown {
        let look = &mut *look;
        match look.soul.map(|soul| souls.get(soul)) {
            Some(Ok(place)) => look.place = place.translation,
            _ => {
                if look.soul.take().is_some() {
                    look.look.arrive();
                }
            }
        }
        look.look.update(dt, models(look.place));
        if look.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut quads = Quads::default();
        let [r, g, b, _] = particles::SOUL_COLOUR;
        for spark in &look.look.system.particles {
            quads.push(
                world(spark.place),
                spark.size * s,
                spark.angle,
                (Vec2::ZERO, 1.0),
                [r, g, b, spark.life / 255.0],
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&look.mesh) {
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The souls' looks.
pub struct SoulSparksPlugin;

impl Plugin for SoulSparksPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>().add_systems(
            Update,
            (show, stream)
                .chain()
                .in_set(Playing)
                .in_set(Unpaused)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
