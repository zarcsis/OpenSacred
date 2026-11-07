//! The look of the banishing circle's runes (#1092; docs/re/effects.md, "The banishing circle's
//! runes"): eight about the Mage for its seconds (`spells`), each `cParticleSystem_groundsymbol`
//! (805, `0x781DC0`), its rune `runeshield<n>` facing the camera laid over by its alpha, its two
//! glows added over it, light bars of `runeshield_gradient` rising above it in a warm colour, and
//! tiny sparks rising (`opensacred_rules::particles::{Rune, rune_colours}`), untinted by the night.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use opensacred_rules::particles::{self, Rune, rune_colours};
use opensacred_rules::spawning::Rng;

use crate::bolt_looks::world;
use crate::frame::Playing;
use crate::fx::{
    Effect, Quads, empty, particle_material, particle_texture, texture_path, whole_material,
};
use crate::game::{AppState, Data};
use crate::{daynight, iso};

/// A rune to make at the place `at`, of its index (0 to 7) and its seconds.
#[derive(Component)]
struct Unmade {
    at: Vec3,
    index: usize,
    seconds: f32,
}

/// A rune: its sparks, its dice, its clock, its lamp, and its parts' meshes: the rune, its first
/// glow, its second, its bar and rays, and its sparks.
#[derive(Component)]
struct Shown {
    rune: Rune,
    dice: Rng,
    clock: f32,
    lamp: Entity,
    parts: [Handle<Mesh>; 5],
}

/// The runes' materials by their index (the rune laid over, its two glows added whole), and the
/// bars' and the sparks', made as they are first asked; none where a texture is missing.
#[derive(Resource, Default)]
struct Materials {
    runes: Vec<Option<[Option<Handle<Effect>>; 3]>>,
    shared: Option<[Option<Handle<Effect>>; 2]>,
}

/// The seed of the runes' `rand()` (**estimate**: any).
const SEED: usize = 0x325;

/// The eight runes of a banishing circle about `centre` for `seconds` (`0x522DE3`): the rune `i`
/// at its angle, 200 off, only where a creature may stand, its cell one that may be walked on
/// (`0x636890`, #2052).
pub(crate) fn circle(
    commands: &mut Commands,
    heights: &crate::terrain::Heights,
    (centre, level): (Vec3, crate::terrain::Level),
    seconds: f32,
) {
    for index in 0..8u8 {
        let angle = particles::RUNES_START + particles::RUNES_APART * f32::from(index);
        let at = centre
            + Vec3::new(angle.sin(), 0.0, angle.cos()) * opensacred_rules::spells::RUNES_FROM;
        let cell = crate::paths::cell_of(at.x, at.z);
        let Some(ground) = heights
            .standing(at.x, at.z, level)
            .filter(|_| heights.walkable_at(cell, level))
        else {
            continue;
        };
        commands.spawn((
            Unmade {
                at: at.with_y(ground),
                index: usize::from(index),
                seconds,
            },
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// A rune's texture of `name`, repeated along its width where `repeat` (the second ray's).
fn texture(data: &Data, assets: &AssetServer, name: &str, repeat: bool) -> Option<Handle<Image>> {
    if !repeat {
        return particle_texture(&data.0.assets, assets, name);
    }
    let path = texture_path(&data.0.assets, name)?;
    Some(
        assets
            .load_builder()
            .with_settings(|settings: &mut ImageLoaderSettings| {
                settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                    lod_max_clamp: 0.0,
                    address_mode_u: ImageAddressMode::Repeat,
                    ..ImageSamplerDescriptor::linear()
                });
            })
            .load::<Image>(path),
    )
}

/// Makes each rune asked for (`0x7825E0`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    (data, assets, mut made, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Materials>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (entity, unmade) in &unmade {
        commands.entity(entity).despawn();
        let Some(data) = data.as_deref() else {
            continue;
        };
        if made.runes.len() < 8 {
            made.runes.resize(8, None);
        }
        let mut load = |name: String, over: bool| {
            let image = texture(data, &assets, &name, false);
            if image.is_none() {
                warn!("banishing runes: no texture {name}; its part is not drawn");
            }
            image.map(|image| {
                materials.add(if over {
                    particle_material(image, AlphaMode::Blend)
                } else {
                    whole_material(image)
                })
            })
        };
        let n = unmade.index + 1;
        let rune = made.runes[unmade.index]
            .get_or_insert_with(|| {
                [
                    load(format!("runeshield{n}"), true),
                    load(format!("runeshield{n}_glow1"), false),
                    load(format!("runeshield{n}_glow2"), false),
                ]
            })
            .clone();
        let shared = made
            .shared
            .get_or_insert_with(|| {
                let bar = texture(data, &assets, "runeshield_gradient", true)
                    .map(|image| materials.add(whole_material(image)));
                let sparks = particle_texture(&data.0.assets, &assets, "particle_glow03")
                    .map(|image| materials.add(whole_material(image)));
                [bar, sparks]
            })
            .clone();
        let lamp = commands
            .spawn((
                Transform::from_translation(unmade.at.with_y(0.0)),
                daynight::Light(particles::RUNE_LIGHT),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let parts: [Handle<Mesh>; 5] = std::array::from_fn(|_| meshes.add(empty()));
        let shown = commands
            .spawn((
                Transform::from_translation(unmade.at),
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let [rune_over, glow1, glow2] = rune;
        let [bar, sparks] = shared;
        for (mesh, material) in parts.iter().zip([rune_over, glow1, glow2, bar, sparks]) {
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
        let index = usize::try_from(shown.index_u32()).unwrap_or(0);
        commands.entity(shown).insert(Shown {
            rune: Rune::new(unmade.seconds),
            dice: crate::seed::dice(SEED ^ index),
            clock: 0.0,
            lamp,
            parts,
        });
    }
}

/// A quad facing the camera of `[left, right, bottom, top]` in the screen's right and up, the
/// texture's width `across` times across it, in `colour`.
fn facing(quads: &mut Quads, [left, right, bottom, top]: [f32; 4], across: f32, colour: [f32; 4]) {
    let s = iso::MODEL_SCALE;
    let (r, u) = (iso::right() * s, iso::up() * s);
    let at = |x: f32, y: f32| r * x + u * y;
    quads.push_corners(
        [
            at(left, bottom),
            at(right, bottom),
            at(right, top),
            at(left, top),
        ],
        [
            Vec2::new(0.0, 1.0),
            Vec2::new(across, 1.0),
            Vec2::new(across, 0.0),
            Vec2::ZERO,
        ],
        colour,
    );
}

/// Runs each rune a frame on (`0x781D60`, `0x781DC0`); past its seconds it goes, its lamp with it.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &GlobalTransform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    orders: crate::fx::Orders,
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut rune, at) in &mut shown {
        let rune = &mut *rune;
        rune.clock += dt;
        rune.rune.update(dt, &mut rune.dice);
        if rune.rune.done() {
            commands.entity(rune.lamp).despawn();
            commands.entity(entity).despawn();
            continue;
        }
        let (grey, bar, rays) = rune_colours(rune.clock);
        let mut quads: [Quads; 5] = std::array::from_fn(|_| Quads::default());
        facing(&mut quads[0], particles::RUNE_QUAD, 1.0, [1.0; 4]);
        facing(&mut quads[1], particles::RUNE_QUAD, 1.0, grey);
        facing(&mut quads[2], particles::RUNE_QUAD, 1.0, [1.0; 4]);
        facing(&mut quads[3], particles::RUNE_BAR, 1.0, bar);
        for ((sides, across), colour) in particles::RUNE_RAYS.iter().zip(rays) {
            facing(&mut quads[3], *sides, *across, colour);
        }
        for spark in &rune.rune.sparks.particles {
            quads[4].push(
                world(spark.place),
                spark.size * iso::MODEL_SCALE,
                0.0,
                (Vec2::ZERO, 1.0),
                particles::RUNE_SPARK_COLOUR,
            );
        }
        // At its object's place in the order, as the original draws the system (#2188).
        let origin = at.translation();
        let order = orders.at(origin);
        for (handle, mut part) in rune.parts.iter().zip(quads) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.in_order(order, origin);
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The banishing circle's runes.
pub struct BanishingRunesPlugin;

impl Plugin for BanishingRunesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Materials>().add_systems(
            Update,
            (make, run)
                .chain()
                .in_set(Playing)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
