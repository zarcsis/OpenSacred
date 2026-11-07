//! The look of the Dwarf's burrowing (57, 1152, `cParticleSystem_dwarf_shield`; #1092;
//! docs/re/effects.md, "The Dwarf's burrowing"): a dome over him for the spell's seconds, built
//! down from its cap in three phases (`fx_shield_phase1` to `3`), whole (`fx_shield_phase4`), and
//! taken back up; at 1.2 times its size, turning a radian a second, drawn twice, lit in
//! `Fx_dwarf_shield` laid over by its alpha and then unlit in `Fx_dwarf_shield2` added, both
//! textures scrolling, its brightness breathing; and a blue glow flat at his feet, added twice.
//! Untinted by the night and laid under his model, as the original draws its system before the
//! creature (`models::Pass::UnderBlended`, `Pass::Under`, `fx::Orders`, #2178)
//! (`opensacred_rules::particles::Burrow`).

use bevy::camera::visibility::{DynamicSkinnedMeshBounds, NoFrustumCulling, RenderLayers};
use bevy::math::Affine2;
use bevy::mesh::MeshTag;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use opensacred_rules::particles::{
    BURROW_GLOW_HALF, BURROW_MODELS, BURROW_SCALE, BURROW_TEXTURES, Burrow,
};

use crate::blended::Layer;
use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, particle_texture, whole_material, wrapped_texture};
use crate::game::{AppState, Data};
use crate::iso;
use crate::models::{self as model_looks, Anchors, ModelMaterial, Pass, in_order};

/// How far over the ground the glow lies, in the models' units, clear of its triangles.
const GROUND_LIFT: f32 = 0.5;

/// A model's mesh as its scene makes it, which the dome's passes dress and copy.
type ModelMesh = (
    Entity,
    &'static Mesh3d,
    &'static MeshMaterial3d<ModelMaterial>,
    Option<&'static SkinnedMesh>,
    &'static MeshTag,
    Has<DynamicSkinnedMeshBounds>,
    Has<NoFrustumCulling>,
);

/// The domes' models, relative to the asset directory, their passes' textures and their glows'
/// material, found at the first dome.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    models: [Option<String>; 4],
    textures: [Option<Handle<Image>>; 2],
    glow: Option<Handle<Effect>>,
}

/// A dome to make: its bearer and its seconds.
#[derive(Component)]
struct Unmade {
    bearer: Entity,
    seconds: f32,
}

/// A dome that shows: its bearer, its look, its phases' models, which of them its passes have
/// dressed, its passes' materials and its glow's mesh.
#[derive(Component)]
struct Shown {
    bearer: Entity,
    look: Burrow,
    models: [Option<Entity>; 4],
    dressed: [bool; 4],
    materials: Option<[Handle<ModelMaterial>; 2]>,
    glow: Handle<Mesh>,
}

/// Shows the burrowing's dome over `bearer` for `seconds`, the spell's value 0 at its level
/// (`0x5287B9`).
pub(crate) fn show(commands: &mut Commands, bearer: Entity, seconds: f32) {
    commands.spawn((Unmade { bearer, seconds }, DespawnOnExit(AppState::InGame)));
}

/// Gives each new dome its phases' models, hidden but its first, and its glow (`0x7B96E0`,
/// `0x7B9C80`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    places: Query<&GlobalTransform>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes, anchors, mut materials): (
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
        Res<Anchors>,
        ResMut<Assets<ModelMaterial>>,
    ),
) {
    for (entity, unmade) in &unmade {
        let Ok(at) = places.get(unmade.bearer).map(GlobalTransform::translation) else {
            commands.entity(entity).despawn();
            continue;
        };
        if !looks.made {
            looks.made = true;
            if let Some(data) = data.as_deref() {
                let files = crate::npcs::model_files(&data.0.assets).unwrap_or_default();
                looks.models = BURROW_MODELS.map(|name| {
                    let file = files.get(name).cloned();
                    if file.is_none() {
                        warn!("dwarf burrows: no {name}; its phase is not drawn");
                    }
                    file
                });
                let [lit, added, glow] = BURROW_TEXTURES;
                looks.textures = [lit, added].map(|name| {
                    let image = wrapped_texture(&data.0.assets, &assets, name);
                    if image.is_none() {
                        warn!("dwarf burrows: no texture {name}; its pass is not drawn");
                    }
                    image
                });
                looks.glow = particle_texture(&data.0.assets, &assets, glow)
                    .map(|image| effects.add(whole_material(image)));
            }
        }
        let look = Burrow::new(unmade.seconds);
        let models = std::array::from_fn(|phase| {
            looks.models[phase].as_ref().map(|model| {
                let visible = if phase == look.phase() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                commands
                    .spawn((
                        model_looks::scene(&assets, model),
                        Transform::from_translation(at)
                            .with_scale(Vec3::splat(BURROW_SCALE * iso::MODEL_SCALE)),
                        visible,
                        model_looks::Ordered::default(),
                        DespawnOnExit(AppState::InGame),
                    ))
                    .id()
            })
        });
        // The lit pass's material and the added pass's, which every phase's meshes share.
        let [lit, added] = looks.textures.clone();
        let made = [
            in_order(
                StandardMaterial {
                    base_color: Color::WHITE,
                    base_color_texture: lit,
                    unlit: false,
                    ..default()
                },
                &anchors,
                Pass::UnderBlended(Layer::Objects),
            ),
            in_order(
                StandardMaterial {
                    base_color: Color::WHITE,
                    base_color_texture: added,
                    unlit: true,
                    ..default()
                },
                &anchors,
                Pass::Under(Layer::Objects),
            ),
        ]
        .map(|material| materials.add(material));
        let glow = meshes.add(empty());
        let mut shown = commands.entity(entity);
        shown.remove::<Unmade>().insert((
            Shown {
                bearer: unmade.bearer,
                look,
                models,
                dressed: [false; 4],
                materials: Some(made),
                glow: glow.clone(),
            },
            Transform::from_translation(at),
            Visibility::default(),
        ));
        if let Some(material) = looks.glow.clone() {
            shown.with_child((
                Mesh3d(glow),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
            ));
        }
    }
}

/// Dresses each phase's model once its meshes are there: its meshes in the lit pass, laid over by
/// their alpha (`0x40D060`), and a copy of each in the added pass (`0x40D310`).
fn dress(
    mut commands: Commands,
    mut shown: Query<&mut Shown>,
    (children, meshes): (Query<&Children>, Query<ModelMesh>),
) {
    for mut dome in &mut shown {
        let dome = &mut *dome;
        let Some(made) = dome.materials.clone() else {
            continue;
        };
        for (model, dressed) in dome.models.iter().zip(dome.dressed.iter_mut()) {
            let Some(model) = *model else {
                continue;
            };
            if *dressed {
                continue;
            }
            let own: Vec<_> = children
                .iter_descendants(model)
                .filter_map(|entity| meshes.get(entity).ok())
                .collect();
            if own.is_empty() {
                continue;
            }
            *dressed = true;
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
        }
    }
}

/// Runs each dome a frame on (`0x7B98C0`) and draws it (`0x7B99D0`) at its bearer's root: its glow
/// flat, twice; its phase's model alone shown, scaled, turned about the upright, its passes in
/// their brightness and their textures' offsets; it goes, with its models, once it has ended, or
/// with its bearer.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &mut Transform)>,
    places: Query<&GlobalTransform>,
    mut models: Query<(&mut Transform, &mut Visibility), Without<Shown>>,
    (mut meshes, mut materials): (ResMut<Assets<Mesh>>, ResMut<Assets<ModelMaterial>>),
    orders: Orders,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut dome, mut transform) in &mut shown {
        let dome = &mut *dome;
        let place = places
            .get(dome.bearer)
            .map(GlobalTransform::translation)
            .ok();
        dome.look.update(dt);
        let Some(place) = place.filter(|_| !dome.look.done()) else {
            for model in dome.models.iter().flatten() {
                commands.entity(*model).try_despawn();
            }
            commands.entity(entity).despawn();
            continue;
        };
        transform.translation = place;
        for (phase, model) in dome.models.iter().enumerate() {
            let Some((mut at, mut visible)) = model.and_then(|model| models.get_mut(model).ok())
            else {
                continue;
            };
            *at = Transform::from_translation(place)
                .with_rotation(Quat::from_rotation_y(dome.look.turn()))
                .with_scale(Vec3::splat(BURROW_SCALE * s));
            *visible = if phase == dome.look.phase() {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
        let [lit_offset, added_offset] = dome.look.offsets().map(Vec2::from);
        if let Some([lit, added]) = dome.materials.as_ref() {
            if let Some(mut material) = materials.get_mut(lit) {
                material.base.base_color = Color::srgba(1.0, 1.0, 1.0, dome.look.lit_alpha());
                material.base.uv_transform = Affine2::from_translation(lit_offset);
            }
            if let Some(mut material) = materials.get_mut(added) {
                let grey = dome.look.added_grey();
                material.base.base_color = Color::srgb(grey, grey, grey);
                material.base.uv_transform = Affine2::from_translation(added_offset);
            }
        }
        let [r, g, b] = dome.look.glow_colour();
        let half = BURROW_GLOW_HALF;
        let mut glow = Quads::default();
        for _ in 0..2 {
            glow.push_corners(
                [
                    world([-half, -half, GROUND_LIFT]),
                    world([half, -half, GROUND_LIFT]),
                    world([half, half, GROUND_LIFT]),
                    world([-half, half, GROUND_LIFT]),
                ],
                [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
                [r, g, b, 1.0],
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&dome.glow) {
            glow.in_order(orders.behind(dome.bearer), place);
            glow.into_mesh(&mut mesh);
        }
    }
}

/// The Dwarf's burrowing's domes.
pub struct DwarfBurrowsPlugin;

impl Plugin for DwarfBurrowsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>().add_systems(
            Update,
            (make, dress, run)
                .chain()
                .in_set(Playing)
                .in_set(Unpaused)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
