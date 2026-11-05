//! The looks of Anducar's demon's rings (1163, `FX_ANDUCAR_DEMON`) and the demon prince's fan (1166,
//! `FX_DAEMONENFUERST`) (#1092; docs/re/effects.md, "The spikes"): each of their objects raises ten
//! spikes about its place as the wave comes to them, one of five models at random, turned and
//! sized at random, lit in their texture and their glow added over them, the prince's in the
//! colour of his type; each rises in half a second, stands and sinks back
//! (`opensacred_rules::particles::Spikes`).

use bevy::camera::visibility::{DynamicSkinnedMeshBounds, NoFrustumCulling};
use bevy::mesh::MeshTag;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use opensacred_rules::particles::{self, Spikes, SpikesKind};

use crate::blended::Layer;
use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::texture_path;
use crate::game::{AppState, Data};
use crate::iso;
use crate::models::{self as model_looks, Anchors, ModelMaterial, Pass, in_order};
use crate::npcs::Retexture;
use crate::spawning::Rng;

/// An object of the rings or the fan, which takes its look ([`raise`]): whose, its place on the
/// ground, the wave's origin and its glow's colour.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Raise {
    pub(crate) kind: SpikesKind,
    pub(crate) ground: Vec3,
    pub(crate) origin: Vec3,
    pub(crate) glow: [f32; 4],
}

/// The spikes' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x1163))
    }
}

/// What the looks take, found at the first object: the five models and the texture, relative to
/// the asset directory, and the glows' images, the demon's and the prince's.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    models: Vec<Option<String>>,
    texture: Option<String>,
    glows: [Option<Handle<Image>>; 2],
}

impl Looks {
    /// Finds the looks' files at the first look asked.
    fn make(&mut self, data: Option<&Data>, assets: &AssetServer) {
        if self.made {
            return;
        }
        self.made = true;
        let Some(data) = data else {
            return;
        };
        let shared = &data.0;
        let files = crate::npcs::model_files(&shared.assets).unwrap_or_default();
        self.models = particles::SPIKE_MODELS
            .iter()
            .map(|model| {
                let file = files.get(*model).cloned();
                if file.is_none() {
                    warn!("spikes: no {model}; its spikes are not drawn");
                }
                file
            })
            .collect();
        self.texture = texture_path(&shared.assets, particles::SPIKE_TEXTURE);
        self.glows = particles::SPIKE_GLOWS
            .map(|name| texture_path(&shared.assets, name).map(|path| assets.load(path)));
    }
}

/// An object's look: its spikes, their models and their glows' copies once made, its place, its
/// glow's image and colour.
#[derive(Component)]
struct Shown {
    spikes: Spikes,
    models: Vec<Option<Entity>>,
    glowing: Vec<bool>,
    place: Vec3,
    glow: Option<(Handle<Image>, [f32; 4])>,
}

/// Gives each new object its spikes (`0x7C04B0`, `0x7C0CC0`).
fn raise(
    mut commands: Commands,
    objects: Query<&Raise, Added<Raise>>,
    (data, assets, mut looks, mut dice): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Dice>,
    ),
) {
    for raise in &objects {
        looks.make(data.as_deref(), &assets);
        debug!("spikes: {:?} at {}", raise.kind, raise.ground);
        let spikes = Spikes::new(raise.kind, models(raise.ground - raise.origin), &mut dice.0);
        let models = spikes
            .spikes
            .iter()
            .map(|spike| {
                let model = looks.models.get(usize::from(spike.model))?.as_ref()?;
                let mut made = commands.spawn((
                    model_looks::scene(&assets, model),
                    Transform::from_translation(raise.ground + world(spike.offset))
                        .with_scale(Vec3::ZERO),
                    Visibility::Hidden,
                    model_looks::Ordered::default(),
                    DespawnOnExit(AppState::InGame),
                ));
                if let Some(texture) = &looks.texture {
                    made.insert(Retexture::load(&assets, texture));
                }
                Some(made.id())
            })
            .collect();
        let glow = match raise.kind {
            SpikesKind::Demon => &looks.glows[0],
            SpikesKind::Prince => &looks.glows[1],
        };
        commands.spawn((
            Shown {
                glowing: vec![false; spikes.spikes.len()],
                spikes,
                models,
                place: raise.ground,
                glow: glow.clone().map(|image| (image, raise.glow)),
            },
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// A mesh of a model, its material converted: its mesh, its joints, its tag and its bounds.
type ModelMesh = (
    &'static Mesh3d,
    Option<&'static SkinnedMesh>,
    &'static MeshTag,
    Has<DynamicSkinnedMeshBounds>,
    Has<NoFrustumCulling>,
);

/// Gives each spike's model, once its meshes are converted (`models`), the copies that add its
/// glow over it: the same mesh in the glow's texture and colour, unlit, added whole in the
/// objects' layer of the blended things (`0x40D310`).
fn glows(
    mut commands: Commands,
    mut shown: Query<&mut Shown>,
    (children, meshes): (
        Query<&Children>,
        Query<ModelMesh, With<MeshMaterial3d<ModelMaterial>>>,
    ),
    (anchors, mut materials): (Res<Anchors>, ResMut<Assets<ModelMaterial>>),
) {
    for mut look in &mut shown {
        let look = &mut *look;
        let Some((image, colour)) = look.glow.clone() else {
            continue;
        };
        let mut material = None;
        for (k, model) in look.models.iter().enumerate() {
            let Some(model) = *model else {
                continue;
            };
            if look.glowing.get(k).copied().unwrap_or(true) {
                continue;
            }
            let own: Vec<Entity> = children
                .iter_descendants(model)
                .filter(|&entity| meshes.contains(entity))
                .collect();
            if own.is_empty() {
                continue;
            }
            let material = material
                .get_or_insert_with(|| {
                    let [r, g, b, a] = colour;
                    let added = StandardMaterial {
                        base_color: Color::srgba(r, g, b, a),
                        base_color_texture: Some(image.clone()),
                        unlit: true,
                        ..default()
                    };
                    materials.add(in_order(added, &anchors, Pass::Added(Layer::Objects)))
                })
                .clone();
            for entity in own {
                let Ok((mesh, skinned, tag, dynamic, unculled)) = meshes.get(entity) else {
                    continue;
                };
                let mut copy = commands.spawn((
                    Mesh3d(mesh.0.clone()),
                    MeshMaterial3d(material.clone()),
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
            if let Some(glowing) = look.glowing.get_mut(k) {
                *glowing = true;
            }
        }
    }
}

/// Runs each object's spikes a frame on (`0x7C0150`, `0x7C02B0`): each grown and turned at its
/// place, hidden while it waits and once it is gone; the object ends with its last.
fn grow(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown)>,
    mut transforms: Query<(&mut Transform, &mut Visibility)>,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut look) in &mut shown {
        let look = &mut *look;
        look.spikes.update(dt);
        if look.spikes.done() {
            for model in look.models.iter().flatten() {
                commands.entity(*model).despawn();
            }
            commands.entity(entity).despawn();
            continue;
        }
        for (spike, model) in look.spikes.spikes.iter().zip(&look.models) {
            let Some(Ok((mut transform, mut visibility))) =
                model.map(|model| transforms.get_mut(model))
            else {
                continue;
            };
            // The models' frame's scale across and up: the glTF's x and z across, its y up.
            let [across, _, up] = spike.scale();
            *transform = Transform::from_translation(look.place + world(spike.offset))
                .with_rotation(Quat::from_rotation_y(spike.turn))
                .with_scale(Vec3::new(across, up, across) * s);
            *visibility = if spike.shown() {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
}

/// The spikes' looks.
pub struct SpikesPlugin;

impl Plugin for SpikesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (raise, glows, grow)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
