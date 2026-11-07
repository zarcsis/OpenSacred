//! The look of the Dwarf's war cry (1089, 1023, `cParticleSystem_kriegsschrei`; #1092;
//! docs/re/effects.md, "The Dwarf's war cry"): as the art starts, at its clip's rate, a blue glow
//! flat at his feet growing; three dwarf ghosts (`fx_dwar_ghost`) rising one after another over
//! him, facing as he does, growing, glowing up dark blue and dark again; a burst of sparks leaping
//! and bouncing where he stood, white through blue to black; the glow shrinking after it. Added
//! and unlit, untinted by the night; the ghosts laid under his model (`models::Pass::Under`), the
//! glow just behind it (`fx::Orders`, #2178) (`opensacred_rules::particles::WarCry`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{WAR_CRY_GLOW, WAR_CRY_MODEL, WAR_CRY_TEXTURES, WarCry};

use crate::blended::Layer;
use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;
use crate::making::Making;
use crate::models::{self as model_looks, Anchors, ModelMaterial, Pass, in_order};
use crate::spawning::Rng;

/// The war cry, whose start makes its look.
const WAR_CRY: u16 = 1089;
/// How far over the ground the glow lies, in the models' units, clear of its triangles.
const GROUND_LIFT: f32 = 0.5;

/// The cries' model, relative to the asset directory, the ghosts' texture, and the glow's and the
/// sparks' materials, found at the first cry.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    model: Option<String>,
    ghost: Option<Handle<Image>>,
    materials: [Option<Handle<Effect>>; 2],
}

/// The cries' dice (`rand()`).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x1023))
    }
}

/// A ghost of a cry: its model, its material, and whether its meshes wear it.
struct Ghost {
    model: Entity,
    material: Handle<ModelMaterial>,
    dressed: bool,
}

/// A cry that shows: its bearer, its look, the way he faced as it started, its ghosts, where its
/// burst lies, and the meshes of its glow and its sparks.
#[derive(Component)]
struct Shown {
    bearer: Entity,
    look: WarCry,
    facing: Quat,
    ghosts: Vec<Ghost>,
    burst_at: Option<Vec3>,
    meshes: [Handle<Mesh>; 2],
}

/// Makes the war cry's look as the art starts (`0x518ABA`, `0x7B7DD0`, `0x7B8300`): at the
/// Dwarf, at his clip's rate, facing as he does.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn start(
    mut commands: Commands,
    started: Query<
        (Entity, &Making, &crate::making::Playing, &GlobalTransform),
        Added<crate::making::Playing>,
    >,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes, anchors, mut materials): (
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
        Res<Anchors>,
        ResMut<Assets<ModelMaterial>>,
    ),
) {
    for (bearer, making, playing, at) in &started {
        if making.art != WAR_CRY {
            continue;
        }
        if !looks.made {
            looks.made = true;
            if let Some(data) = data.as_deref() {
                looks.model = crate::npcs::model_files(&data.0.assets)
                    .ok()
                    .and_then(|files| files.get(WAR_CRY_MODEL).cloned());
                if looks.model.is_none() {
                    warn!("war cries: no {WAR_CRY_MODEL}; their ghosts are not drawn");
                }
                let [ghost, spark, glow] = WAR_CRY_TEXTURES.map(|name| {
                    let image = particle_texture(&data.0.assets, &assets, name);
                    if image.is_none() {
                        warn!("war cries: no texture {name}; its part is not drawn");
                    }
                    image
                });
                looks.ghost = ghost;
                looks.materials = [glow, spark]
                    .map(|image| image.map(|image| effects.add(whole_material(image))));
            }
        }
        let place = at.translation();
        let facing = at.rotation();
        let ghosts = looks
            .model
            .as_ref()
            .map(|model| {
                (0..3)
                    .map(|_| Ghost {
                        model: commands
                            .spawn((
                                model_looks::scene(&assets, model),
                                Transform::from_translation(place).with_rotation(facing),
                                Visibility::Hidden,
                                model_looks::Ordered::default(),
                                DespawnOnExit(AppState::InGame),
                            ))
                            .id(),
                        material: materials.add(in_order(
                            StandardMaterial {
                                base_color: Color::BLACK,
                                base_color_texture: looks.ghost.clone(),
                                unlit: true,
                                ..default()
                            },
                            &anchors,
                            Pass::Under(Layer::Objects),
                        )),
                        dressed: false,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let parts: [Handle<Mesh>; 2] = std::array::from_fn(|_| meshes.add(empty()));
        let mut shown = commands.spawn((
            Shown {
                bearer,
                look: WarCry::new(playing.rate()),
                facing,
                ghosts,
                burst_at: None,
                meshes: parts.clone(),
            },
            Transform::from_translation(place),
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        for (mesh, material) in parts.into_iter().zip(looks.materials.clone()) {
            if let Some(material) = material {
                shown.with_child((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                ));
            }
        }
    }
}

/// Dresses each ghost's meshes in its material once they are there (`0x40D310`: unlit, added,
/// its depth tested and not written).
fn dress(
    mut commands: Commands,
    mut shown: Query<&mut Shown>,
    (children, meshes): (
        Query<&Children>,
        Query<Entity, With<MeshMaterial3d<ModelMaterial>>>,
    ),
) {
    for mut cry in &mut shown {
        for ghost in cry.ghosts.iter_mut().filter(|ghost| !ghost.dressed) {
            let own: Vec<Entity> = children
                .iter_descendants(ghost.model)
                .filter(|&entity| meshes.contains(entity))
                .collect();
            if own.is_empty() {
                continue;
            }
            ghost.dressed = true;
            for entity in own {
                commands
                    .entity(entity)
                    .insert(MeshMaterial3d(ghost.material.clone()));
            }
        }
    }
}

/// Runs each cry a frame on (`0x7B7F10`) and draws it (`0x7B80F0`) at its bearer's root: its glow
/// flat; its rising ghosts at their heights over him, at their scales, facing as he faced, in
/// their colours; its sparks where it burst; it goes, with its ghosts, once its glow is gone, or
/// with its bearer.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &mut Transform)>,
    places: Query<&GlobalTransform>,
    mut models: Query<(&mut Transform, &mut Visibility), Without<Shown>>,
    (mut meshes, mut materials, mut dice): (
        ResMut<Assets<Mesh>>,
        ResMut<Assets<ModelMaterial>>,
        ResMut<Dice>,
    ),
    orders: Orders,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let whole = (Vec2::ZERO, 1.0);
    for (entity, mut cry, mut transform) in &mut shown {
        let cry = &mut *cry;
        let place = places
            .get(cry.bearer)
            .map(GlobalTransform::translation)
            .ok();
        cry.look.update(dt, &mut dice.0);
        let Some(place) = place.filter(|_| !cry.look.done()) else {
            for ghost in &cry.ghosts {
                commands.entity(ghost.model).try_despawn();
            }
            commands.entity(entity).despawn();
            continue;
        };
        transform.translation = place;
        let rising = cry.look.ghosts();
        for (i, ghost) in cry.ghosts.iter().enumerate() {
            let Ok((mut at, mut visible)) = models.get_mut(ghost.model) else {
                continue;
            };
            let Some(&(_, height, scale, colour)) = rising.iter().find(|g| g.0 == i) else {
                *visible = Visibility::Hidden;
                continue;
            };
            *visible = Visibility::Inherited;
            *at = Transform::from_translation(place + Vec3::Y * height * s)
                .with_rotation(cry.facing)
                .with_scale(Vec3::splat(scale * s));
            if let Some(mut material) = materials.get_mut(&ghost.material) {
                let [r, g, b] = colour;
                material.base.base_color = Color::srgb(r, g, b);
            }
        }
        let half = cry.look.glow_half();
        let [r, g, b] = WAR_CRY_GLOW;
        let mut glow = Quads::default();
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
        if cry.look.burst() && cry.burst_at.is_none() {
            cry.burst_at = Some(place);
        }
        // Its sparks stay where it burst.
        let shift = cry.burst_at.map_or(Vec3::ZERO, |at| at - place);
        let mut sparks = Quads::default();
        for spark in &cry.look.sparks.particles {
            sparks.push(
                world(spark.place) + shift,
                spark.size * s,
                0.0,
                whole,
                cry.look.colour(spark.life),
            );
        }
        let order = orders.behind(cry.bearer);
        for (handle, mut part) in cry.meshes.iter().zip([glow, sparks]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.in_order(order, place);
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The Dwarf's war cries' looks.
pub struct WarCriesPlugin;

impl Plugin for WarCriesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>()
            .init_resource::<Dice>()
            .add_systems(
                Update,
                (start, dress, run)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
