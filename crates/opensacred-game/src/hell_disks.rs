//! The look of the Daemon's hell disk (1005, `FX_HELLDISK`, #1092; docs/re/effects.md, "The hell
//! disk"): each disk `spells` sends shows its model at 0.08 of its size, its rings whirling in its
//! clip's loop, in the lava's frames, lit and again added over itself; a dull orange glow pulses
//! about it and a trail of orange glows runs behind it, both added and untinted by the night
//! (`opensacred_rules::particles::HellDisk`). Stopped past its seconds, it stays where it is until
//! its trail is gone.

use bevy::camera::visibility::{DynamicSkinnedMeshBounds, NoFrustumCulling, RenderLayers};
use bevy::gltf::Gltf;
use bevy::mesh::MeshTag;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use opensacred_rules::particles::{self, HellDisk};

use crate::blended::Layer;
use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;
use crate::models::{self as model_looks, Anchors, ModelMaterial, Pass, in_order};
use crate::spawning::Rng;

/// The disks' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x1005))
    }
}

/// What the looks take, found at the first disk: its model, relative to the asset directory,
/// the lava's frames, and the materials of its glow and of its trail.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    model: Option<String>,
    frames: Vec<Handle<Image>>,
    glow: Option<Handle<Effect>>,
    trail: Option<Handle<Effect>>,
}

/// A disk's look, which outlives its disk while its trail fades: its disk while it flies, its
/// system, its model, its place and the place before, its meshes of the trail and of the glow,
/// its model's materials once its meshes are dressed in them, and whether its clip plays.
#[derive(Component)]
struct Shown {
    disk: Option<Entity>,
    look: HellDisk,
    model: Option<Entity>,
    place: Vec3,
    before: Option<Vec3>,
    meshes: [Handle<Mesh>; 2],
    materials: Option<[Handle<ModelMaterial>; 2]>,
    playing: bool,
}

/// A mesh of a model: its mesh, its joints, its tag and its bounds.
type ModelMesh = (
    Entity,
    &'static Mesh3d,
    &'static MeshMaterial3d<ModelMaterial>,
    Option<&'static SkinnedMesh>,
    &'static MeshTag,
    Has<DynamicSkinnedMeshBounds>,
    Has<NoFrustumCulling>,
);

/// Gives each new disk its look (`0x7AD8C0`, `0x7AE040`): its model, its glow and its trail.
fn show(
    mut commands: Commands,
    disks: Query<(Entity, &Transform), Added<crate::spells::HellDisk>>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, place) in &disks {
        if !looks.made {
            looks.made = true;
            if let Some(data) = data.as_deref() {
                looks.model = crate::npcs::model_files(&data.0.assets)
                    .ok()
                    .and_then(|files| files.get(particles::HELL_DISK_MODEL).cloned());
                if looks.model.is_none() {
                    warn!(
                        "hell disks: no {}; their disks are not drawn",
                        particles::HELL_DISK_MODEL
                    );
                }
                looks.frames = (0..particles::LAVA_FRAMES)
                    .filter_map(|frame| {
                        particle_texture(
                            &data.0.assets,
                            &assets,
                            &particles::lava_frame_name(frame),
                        )
                    })
                    .collect();
                if looks.frames.len() < usize::from(particles::LAVA_FRAMES) {
                    warn!("hell disks: a frame of the lava is missing; it does not flow");
                    looks.frames.clear();
                }
                let image = particle_texture(&data.0.assets, &assets, particles::HELL_DISK_GLOW);
                if image.is_none() {
                    warn!(
                        "hell disks: no texture {}; their glows are not drawn",
                        particles::HELL_DISK_GLOW
                    );
                }
                looks.glow = image
                    .clone()
                    .map(|image| effects.add(whole_material(image)));
                looks.trail =
                    image.map(|image| effects.add(particle_material(image, AlphaMode::Add)));
            }
        }
        let model = looks.model.as_ref().map(|model| {
            commands
                .spawn((
                    model_looks::scene(&assets, model),
                    Transform::from_translation(place.translation)
                        .with_scale(Vec3::splat(particles::HELL_DISK_SCALE * iso::MODEL_SCALE)),
                    model_looks::Ordered::default(),
                    DespawnOnExit(AppState::InGame),
                ))
                .id()
        });
        let parts = [meshes.add(empty()), meshes.add(empty())];
        let mut shown = commands.spawn((
            Transform::IDENTITY,
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        for (mesh, material) in parts.iter().zip([looks.trail.clone(), looks.glow.clone()]) {
            if let Some(material) = material {
                shown.with_child((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                ));
            }
        }
        shown.insert(Shown {
            disk: Some(entity),
            look: HellDisk::new(),
            model,
            place: place.translation,
            before: None,
            meshes: parts,
            materials: None,
            playing: false,
        });
    }
}

/// Dresses each disk's model in the lava once its meshes are there, lit in its own material and
/// again added over itself, white and unlit (`0x405C90`, `0x40D310`), and plays its clip's loop
/// once its player is there.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn dress(
    mut commands: Commands,
    mut shown: Query<&mut Shown>,
    (children, meshes, sources): (
        Query<&Children>,
        Query<ModelMesh>,
        Query<&model_looks::Source>,
    ),
    (mut players, gltfs, mut graphs): (
        Query<&mut AnimationPlayer, Without<AnimationGraphHandle>>,
        Res<Assets<Gltf>>,
        ResMut<Assets<AnimationGraph>>,
    ),
    (looks, anchors, mut materials): (Res<Looks>, Res<Anchors>, ResMut<Assets<ModelMaterial>>),
) {
    for mut look in &mut shown {
        let Some(model) = look.model else {
            continue;
        };
        if !look.playing {
            let clip = sources
                .get(model)
                .ok()
                .and_then(|source| gltfs.get(&source.0))
                .and_then(|gltf| gltf.animations.first().cloned());
            if let Some(clip) = clip {
                for entity in children.iter_descendants(model) {
                    if let Ok(mut player) = players.get_mut(entity) {
                        let (graph, node) = AnimationGraph::from_clip(clip.clone());
                        player.play(node).repeat();
                        commands
                            .entity(entity)
                            .insert(AnimationGraphHandle(graphs.add(graph)));
                        look.playing = true;
                    }
                }
            }
        }
        if look.materials.is_some() {
            continue;
        }
        let Some(first) = looks.frames.first().cloned() else {
            continue;
        };
        let own: Vec<_> = children
            .iter_descendants(model)
            .filter_map(|entity| meshes.get(entity).ok())
            .collect();
        let Some(mut lit) = own
            .first()
            .and_then(|(_, _, material, ..)| materials.get(&material.0).cloned())
        else {
            continue;
        };
        lit.base.base_color_texture = Some(first.clone());
        let added = in_order(
            StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: Some(first),
                unlit: true,
                ..default()
            },
            &anchors,
            Pass::Added(Layer::Objects),
        );
        let made = [materials.add(lit), materials.add(added)];
        for (entity, mesh, _, skinned, tag, dynamic, unculled) in own {
            commands
                .entity(entity)
                .insert(MeshMaterial3d(made[0].clone()));
            let mut copy = commands.spawn((
                Mesh3d(mesh.0.clone()),
                MeshMaterial3d(made[1].clone()),
                tag.clone(),
                ChildOf(entity),
            ));
            if let Some(skinned) = skinned {
                copy.insert(skinned.clone());
            }
            if dynamic {
                copy.insert(DynamicSkinnedMeshBounds);
            }
            if unculled {
                copy.insert(NoFrustumCulling);
            }
        }
        look.materials = Some(made);
    }
}

/// Runs each disk's look a frame on (`0x7ADB10`, `0x7ADBC0`): its model at its place in the
/// lava's frames, its glow about it and its trail; it goes with its last glow.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown)>,
    disks: Query<&Transform, With<crate::spells::HellDisk>>,
    mut transforms: Query<&mut Transform, (Without<crate::spells::HellDisk>, Without<Shown>)>,
    (looks, mut dice, mut meshes, mut materials): (
        Res<Looks>,
        ResMut<Dice>,
        ResMut<Assets<Mesh>>,
        ResMut<Assets<ModelMaterial>>,
    ),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let ramp = HellDisk::ramp();
    for (entity, mut look) in &mut shown {
        let look = &mut *look;
        match look.disk.map(|disk| disks.get(disk)) {
            Some(Ok(place)) => look.place = place.translation,
            _ => {
                if look.disk.take().is_some() {
                    look.look.end();
                }
            }
        }
        let before = look.before.unwrap_or(look.place);
        look.look
            .update(dt, (models(before), models(look.place)), &mut dice.0);
        look.before = Some(look.place);
        if look.look.done() {
            if let Some(model) = look.model {
                commands.entity(model).try_despawn();
            }
            commands.entity(entity).despawn();
            continue;
        }
        if let Some(mut at) = look.model.and_then(|model| transforms.get_mut(model).ok()) {
            at.translation = look.place;
        }
        for (material, frame) in look.materials.iter().flatten().zip(look.look.frames()) {
            if let Some(mut material) = materials.get_mut(material) {
                material.base.base_color_texture = looks.frames.get(usize::from(frame)).cloned();
            }
        }
        let mut trail = Quads::default();
        for glow in &look.look.system.particles {
            trail.push(
                world(glow.place),
                glow.size * s,
                0.0,
                (Vec2::ZERO, 1.0),
                ramp.at(glow.life),
            );
        }
        let mut glow = Quads::default();
        glow.push(
            look.place,
            look.look.glow() * s,
            0.0,
            (Vec2::ZERO, 1.0),
            particles::HELL_DISK_GLOW_COLOUR,
        );
        for (mesh, quads) in look.meshes.iter().zip([trail, glow]) {
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The hell disks' looks.
pub struct HellDisksPlugin;

impl Plugin for HellDisksPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (show, dress, fly)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
