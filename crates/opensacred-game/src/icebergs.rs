//! The look of the ring of frost's objects (#1092; docs/re/effects.md, "The ring of frost"): at
//! each of its eight objects (`spells`), `cParticleSystem_circleofice` (840, `0x78DD00`) strikes
//! for 1.5 s a flickering strike of light facing the camera, added by its alpha, over a glow on
//! the ground added by its alpha; then it bursts sparks of `PARTICLE_SPARK02`, added whole, that
//! fall and bounce, and stands its iceberg (`ICEBERG0N.GRN` in `ICEBERG`, lit as the world's
//! models are) over a wider glow; its light of 150 with it
//! (`opensacred_rules::particles::Iceberg`). The strike, the glows and the sparks are untinted by
//! the night.
//!
//! Not followed: the strike over all else (its depth test off, `0x78DED0`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, Iceberg};
use opensacred_rules::spawning::Rng;

use crate::frame::Playing;
use crate::fx::{
    Effect, Quads, empty, particle_material, particle_texture, texture_path, whole_material,
};
use crate::game::{AppState, Data};
use crate::npcs::Retexture;
use crate::{daynight, iso, models, npcs};

/// How far over the ground the glow lies, in world units, so that the ground's own faces do not
/// cut it.
const GROUND_LIFT: f32 = 0.5;

/// An object of the ring of frost to give its look, for its seconds (`spells`).
#[derive(Component)]
pub(crate) struct Look(pub f32);

/// An object's look: its object, its system, its dice, its iceberg and its lamp, and its parts'
/// meshes: the strike's by its four textures, the glow's and the sparks'.
#[derive(Component)]
struct Shown {
    object: Entity,
    ice: Iceberg,
    dice: Rng,
    berg: Option<Entity>,
    lamp: Entity,
    parts: [Handle<Mesh>; 6],
}

/// The icebergs' models and their texture, relative to the asset directory, and the materials:
/// the strikes' by their textures, the glow's and the sparks'; made as the first object shows,
/// none where a texture is missing.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    models: [Option<String>; 4],
    texture: Option<String>,
    materials: [Option<Handle<Effect>>; 6],
}

/// The seed of the objects' `rand()` (**estimate**: any).
const SEED: usize = 0x348;

/// Gives each new object its look (`0x78D870`, `0x78E070`): an entity of its own at its place,
/// its lamp, its iceberg hidden until it stands, and its parts.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn show(
    mut commands: Commands,
    objects: Query<(Entity, &Look, &GlobalTransform), Added<Look>>,
    (data, assets, mut looks, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (object, &Look(seconds), place) in &objects {
        let Some(data) = data.as_deref() else {
            continue;
        };
        if !looks.made {
            let shared = &data.0;
            looks.made = true;
            let files = npcs::model_files(&shared.assets).unwrap_or_default();
            looks.models = particles::ICE_MODELS.map(|name| files.get(name).cloned());
            looks.texture = texture_path(&shared.assets, particles::ICE_TEXTURES[0]);
            let mut load = |name: &str, whole: bool| {
                let image = particle_texture(&shared.assets, &assets, name);
                if image.is_none() {
                    warn!("icebergs: no texture {name}; its part is not drawn");
                }
                image.map(|image| {
                    materials.add(if whole {
                        whole_material(image)
                    } else {
                        particle_material(image, AlphaMode::Add)
                    })
                })
            };
            let [a, b, c, d] = particles::ICE_STRIKES.map(|name| load(name, false));
            let glow = load(particles::ICE_GLOW_TEXTURE, false);
            let sparks = load(particles::ICE_SPARK_TEXTURE, true);
            looks.materials = [a, b, c, d, glow, sparks];
        }
        let place = place.translation();
        let index = usize::try_from(object.index_u32()).unwrap_or(0);
        let mut dice = crate::seed::dice(SEED ^ index);
        let ice = Iceberg::new(seconds, &mut dice);
        let lamp = commands
            .spawn((
                Transform::from_translation(place.with_y(0.0)),
                daynight::Light(particles::ICE_LIGHT),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let shown = commands
            .spawn((
                Transform::from_translation(place),
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let parts: [Handle<Mesh>; 6] = std::array::from_fn(|_| meshes.add(empty()));
        for (mesh, material) in parts.iter().zip(looks.materials.clone()) {
            if let Some(material) = material {
                commands.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                    ChildOf(shown),
                ));
            }
        }
        let berg = looks.models[ice.model].as_ref().map(|model| {
            let mut berg = commands.spawn((
                models::scene(&assets, model),
                Transform::from_translation(place)
                    .with_rotation(Quat::from_rotation_y(ice.turn))
                    .with_scale(Vec3::splat(iso::MODEL_SCALE)),
                Visibility::Hidden,
                models::Ordered::default(),
                DespawnOnExit(AppState::InGame),
            ));
            if let Some(texture) = &looks.texture {
                berg.insert(Retexture::load(&assets, texture));
            }
            berg.id()
        });
        commands.entity(shown).insert(Shown {
            object,
            ice,
            dice,
            berg,
            lamp,
            parts,
        });
    }
}

/// Runs each object's look a frame on (`0x78DC10`, `0x78DD00`); once it ends or its object goes,
/// it goes, its iceberg and its lamp with it.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown)>,
    objects: Query<(), With<Look>>,
    mut visibilities: Query<&mut Visibility, Without<Shown>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let (across, depth) = iso::fx_axes();
    let frame = |[x, y, z]: [f32; 3]| (across * x + depth * y + Vec3::Y * z) * s;
    let colour = particles::ICE_COLOURS[0];
    for (entity, mut look) in &mut shown {
        let look = &mut *look;
        look.ice.update(dt, &mut look.dice);
        if look.ice.done() || !objects.contains(look.object) {
            if let Some(berg) = look.berg {
                commands.entity(berg).despawn();
            }
            commands.entity(look.lamp).despawn();
            commands.entity(entity).despawn();
            continue;
        }
        let striking = look.ice.striking();
        if let Some(mut visibility) = look.berg.and_then(|berg| visibilities.get_mut(berg).ok()) {
            visibility.set_if_neq(if striking {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            });
        }
        let mut quads: [Quads; 6] = std::array::from_fn(|_| Quads::default());
        if striking {
            // Facing the camera in the systems' frame, its texture's `u` up it.
            let (texture, [left, right]) = look.ice.strike();
            let [x0, x1, z0, z1] = particles::ICE_STRIKE_QUAD;
            if let Some(strike) = quads.get_mut(texture) {
                strike.push_corners(
                    [
                        frame([x0, 0.0, z0]),
                        frame([x1, 0.0, z0]),
                        frame([x1, 0.0, z1]),
                        frame([x0, 0.0, z1]),
                    ],
                    [
                        Vec2::new(0.0, left),
                        Vec2::new(0.0, right),
                        Vec2::new(1.0, right),
                        Vec2::new(1.0, left),
                    ],
                    colour,
                );
            }
        }
        let half = particles::ICE_GLOW_HALVES[usize::from(!striking)] * s;
        let ground = Vec3::Y * GROUND_LIFT;
        quads[4].push_corners(
            [
                ground + (-across - depth) * half,
                ground + (across - depth) * half,
                ground + (across + depth) * half,
                ground + (-across + depth) * half,
            ],
            [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
            colour,
        );
        let ramp = Iceberg::ramp();
        for spark in &look.ice.sparks.particles {
            quads[5].push(
                frame(spark.place),
                spark.size * s,
                0.0,
                (Vec2::ZERO, 1.0),
                ramp.at(spark.life),
            );
        }
        for (handle, part) in look.parts.iter().zip(quads) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The ring of frost's objects' looks.
pub struct IcebergsPlugin;

impl Plugin for IcebergsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>().add_systems(
            Update,
            (show, run)
                .chain()
                .in_set(Playing)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
