//! The look of the Daemon's demon fire (1015, `FX_DEMONFIRE`, #1092; docs/re/effects.md, "The
//! demon fire"): over its point a ball of lava bobs, two spheres in the lava's frames, the inner
//! lit and the outer added over it, turning against each other's pace, and a starburst of thin
//! orange rays about it, added and untinted by the night
//! (`opensacred_rules::particles::DemonFire`); it grows in over its first two seconds and shrinks
//! away over its last two, making no rays once it strikes no more.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, DemonFire};

use crate::blended::Layer;
use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;
use crate::models::{self, Anchors, ModelMaterial, Pass, in_order};
use crate::spawning::Rng;

/// The fires' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x1015))
    }
}

/// What the looks take, found at the first fire: the sphere's model, relative to the asset
/// directory, the lava's frames and the rays' material.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    model: Option<String>,
    frames: Vec<Handle<Image>>,
    rays: Option<Handle<Effect>>,
}

/// A fire's look: its system, its two spheres (the inner and the outer), their materials once
/// their meshes are dressed in them, and its rays' mesh.
#[derive(Component)]
struct Shown {
    fire: DemonFire,
    spheres: [Option<Entity>; 2],
    materials: Option<[Handle<ModelMaterial>; 2]>,
    mesh: Handle<Mesh>,
}

/// A sphere of a fire, which goes with it.
#[derive(Component)]
struct SphereOf(Entity);

/// Gives each new fire its look (`0x7B2B00`, `0x7B3110`): its two spheres and its rays.
fn show(
    mut commands: Commands,
    fires: Query<(Entity, &crate::spells::DemonFire, &Transform), Added<crate::spells::DemonFire>>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, fire, place) in &fires {
        if !looks.made {
            looks.made = true;
            if let Some(data) = data.as_deref() {
                looks.model = crate::npcs::model_files(&data.0.assets)
                    .ok()
                    .and_then(|files| files.get(particles::DEMON_FIRE_MODEL).cloned());
                if looks.model.is_none() {
                    warn!(
                        "demon fires: no {}; their balls are not drawn",
                        particles::DEMON_FIRE_MODEL
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
                    warn!("demon fires: a frame of the lava is missing; it does not flow");
                    looks.frames.clear();
                }
                let rays = particle_texture(&data.0.assets, &assets, particles::DEMON_FIRE_RAYS);
                if rays.is_none() {
                    warn!(
                        "demon fires: no texture {}; their rays are not drawn",
                        particles::DEMON_FIRE_RAYS
                    );
                }
                looks.rays =
                    rays.map(|image| effects.add(particle_material(image, AlphaMode::Add)));
            }
        }
        let mut sphere = || {
            looks.model.as_ref().map(|model| {
                commands
                    .spawn((
                        models::scene(&assets, model),
                        Transform::from_translation(place.translation).with_scale(Vec3::ZERO),
                        models::Ordered::default(),
                        SphereOf(entity),
                        DespawnOnExit(AppState::InGame),
                    ))
                    .id()
            })
        };
        let spheres = [sphere(), sphere()];
        let mesh = meshes.add(empty());
        // Its Direct3D light 45 over its place (`0x7B2B00`, `0x7B2DF0`; its wobble of 3 left out).
        commands.spawn(crate::fx_lights::light(
            crate::fx_lights::HELL,
            crate::fx_lights::LOW,
            place.translation,
            entity,
            (Some(entity), Vec3::Y * 45.0 * iso::MODEL_SCALE),
        ));
        let mut shown = commands.entity(entity);
        if let Some(material) = looks.rays.clone() {
            shown.with_child((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
            ));
        }
        shown.insert(Shown {
            fire: DemonFire::new(fire.times),
            spheres,
            materials: None,
            mesh,
        });
    }
}

/// Dresses each fire's spheres in the lava once their meshes are there: the inner in its own
/// material retextured, the outer added over it, white and unlit (`0x405C90`, `0x40D310`).
fn dress(
    mut commands: Commands,
    mut fires: Query<&mut Shown>,
    (children, meshes): (Query<&Children>, Query<&MeshMaterial3d<ModelMaterial>>),
    (looks, anchors, mut materials): (Res<Looks>, Res<Anchors>, ResMut<Assets<ModelMaterial>>),
) {
    for mut shown in &mut fires {
        if shown.materials.is_some() {
            continue;
        }
        let Some(first) = looks.frames.first().cloned() else {
            continue;
        };
        let parts = shown.spheres.map(|sphere| {
            sphere
                .map(|sphere| {
                    children
                        .iter_descendants(sphere)
                        .filter(|&entity| meshes.contains(entity))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        });
        let [inner, outer] = &parts;
        let Some(own) = inner.first().and_then(|&mesh| meshes.get(mesh).ok()) else {
            continue;
        };
        if outer.is_empty() {
            continue;
        }
        let Some(mut lit) = materials.get(&own.0).cloned() else {
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
        for (meshes, material) in parts.iter().zip(&made) {
            for &mesh in meshes {
                commands
                    .entity(mesh)
                    .insert(MeshMaterial3d(material.clone()));
            }
        }
        shown.materials = Some(made);
    }
}

/// Runs each fire's look a frame on (`0x7B2CA0`, `0x7B2DF0`): its spheres at its ball's centre,
/// sized by its growth and turned, in the lava's frame; its rays standing out from their feet.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn run(
    time: Res<Time>,
    mut fires: Query<(&mut Shown, &Transform)>,
    mut spheres: Query<&mut Transform, (With<SphereOf>, Without<Shown>)>,
    (looks, mut dice, mut meshes, mut materials): (
        Res<Looks>,
        ResMut<Dice>,
        ResMut<Assets<Mesh>>,
        ResMut<Assets<ModelMaterial>>,
    ),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let ramp = DemonFire::ramp();
    for (mut shown, place) in &mut fires {
        let shown = &mut *shown;
        shown.fire.update(dt, &mut dice.0);
        let centre = place.translation + world(shown.fire.centre());
        for (sphere, (size, turn)) in shown.spheres.iter().zip(shown.fire.spheres()) {
            if let Some(mut at) = sphere.and_then(|sphere| spheres.get_mut(sphere).ok()) {
                *at = Transform::from_translation(centre)
                    .with_rotation(Quat::from_rotation_y(turn))
                    .with_scale(Vec3::splat(size * s));
            }
        }
        let frame = looks.frames.get(usize::from(shown.fire.frame())).cloned();
        for material in shown.materials.iter().flatten() {
            if let Some(mut material) = materials.get_mut(material) {
                material.base.base_color_texture = frame.clone();
            }
        }
        // The inner sphere writes the depth, which hides the rays' feet within it.
        let [(inner, _), _] = shown.fire.spheres();
        let ball = particles::SPHERE_RADIUS * inner * s;
        let mut quads = Quads::default();
        for ray in &shown.fire.system.particles {
            quads.push_ray(
                world(ray.place),
                (0.125 * ray.size * s, 2.0 * ray.size * s, ball),
                ray.angle,
                ramp.at(ray.life),
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&shown.mesh) {
            quads.into_mesh(&mut mesh);
        }
    }
}

/// Takes away each sphere whose fire is gone.
fn gone(
    mut commands: Commands,
    spheres: Query<(Entity, &SphereOf)>,
    fires: Query<(), With<Shown>>,
) {
    for (sphere, of) in &spheres {
        if !fires.contains(of.0) {
            commands.entity(sphere).despawn();
        }
    }
}

/// The demon fires' looks.
pub struct DemonFiresPlugin;

impl Plugin for DemonFiresPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (show, dress, run, gone)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
