//! The look of a meteor of the hail of meteors (#1092; docs/re/effects.md, "The hail of
//! meteors"): where it lands (`spells`), `cParticleSystem_meteor` (823, `0x786AC0`) waits, then
//! its rock (`GIBS01.GRN` in `C_LAVA00`, lit as the world's models are) falls from the side with
//! a glow at it and a trail of fire added by its alpha and smoke laid over by its alpha
//! (`opensacred_rules::particles::Meteor`); at the ground a quake and a burst of fire and smoke,
//! and its crater laid over the ground, darkened by the night as the ground is, which lies and
//! fades. The fire, the smoke and the glow are untinted by the night.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, Meteor};
use opensacred_rules::spawning::Rng;

use crate::frame::Playing;
use crate::fx::{
    Effect, Quads, empty, particle_material, particle_texture, texture_path, whole_material,
};
use crate::game::{AppState, Data};
use crate::npcs::Retexture;
use crate::terrain::{Heights, Level};
use crate::{iso, models, npcs};

/// How far over the ground the crater lies, in world units, so that the ground's own faces do not
/// cut it.
const GROUND_LIFT: f32 = 0.5;

/// A meteor to make at its entity's place on its level, falling after its wait in seconds.
#[derive(Component)]
struct Unmade {
    level: Level,
    wait: f32,
}

/// A meteor that lands at `at` on `level` after `wait` seconds and its fall (`spells`).
pub(crate) fn fall(commands: &mut Commands, at: Vec3, level: Level, wait: f32) {
    commands.spawn((
        Unmade { level, wait },
        Transform::from_translation(at),
        DespawnOnExit(AppState::InGame),
    ));
}

/// A meteor's look: its system, its dice, its rock while it falls, and its parts' meshes, the
/// fire's, the smoke's, the glow's and the crater's.
#[derive(Component)]
struct Shown {
    meteor: Meteor,
    dice: Rng,
    rock: Option<Entity>,
    parts: [Handle<Mesh>; 4],
}

/// The rock's model and texture, relative to the asset directory, and the materials: the fire's,
/// the smoke's, the glow's and the two craters'; made as the first meteor shows, none where a
/// texture is missing.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    model: Option<String>,
    texture: Option<String>,
    materials: [Option<Handle<Effect>>; 5],
}

/// The seed of the meteors' `rand()` (**estimate**: any).
const SEED: usize = 0x337;

/// Makes each meteor asked for (`0x786390`, `0x786EF0`): on the ground under its place, its parts
/// in their layers.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade, &Transform)>,
    heights: Res<Heights>,
    (data, assets, mut looks, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (entity, unmade, transform) in &unmade {
        commands.entity(entity).despawn();
        let Some(data) = data.as_deref() else {
            continue;
        };
        if !looks.made {
            let shared = &data.0;
            looks.made = true;
            looks.model = npcs::model_files(&shared.assets)
                .ok()
                .and_then(|files| files.get(particles::METEOR_MODEL).cloned());
            looks.texture = texture_path(&shared.assets, particles::METEOR_TEXTURE);
            if looks.model.is_none() {
                warn!(
                    "meteors: no {}; its rock is not drawn",
                    particles::METEOR_MODEL
                );
            }
            let mut load = |name: &str| {
                let image = particle_texture(&shared.assets, &assets, name);
                if image.is_none() {
                    warn!("meteors: no texture {name}; its part is not drawn");
                }
                image
            };
            let smoke = load(particles::METEOR_PARTICLES);
            let glow = load(particles::METEOR_GLOW_TEXTURE);
            let [even, odd] = particles::METEOR_CRATERS.map(&mut load);
            looks.materials = [
                smoke
                    .clone()
                    .map(|image| materials.add(particle_material(image, AlphaMode::Add))),
                smoke.map(|image| materials.add(particle_material(image, AlphaMode::Blend))),
                glow.map(|image| materials.add(whole_material(image))),
                even.map(|image| materials.add(particle_material(image, AlphaMode::Blend))),
                odd.map(|image| materials.add(particle_material(image, AlphaMode::Blend))),
            ];
        }
        let at = transform.translation;
        let ground = heights.standing(at.x, at.z, unmade.level).unwrap_or(at.y);
        let index = usize::try_from(entity.index_u32()).unwrap_or(0);
        let mut dice = crate::seed::dice(SEED ^ index);
        let meteor = Meteor::new(unmade.wait, &mut dice);
        let shown = commands
            .spawn((
                Transform::from_translation(at.with_y(ground)),
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let parts: [Handle<Mesh>; 4] = std::array::from_fn(|_| meshes.add(empty()));
        let [fire, smoke, glow, even, odd] = looks.materials.clone();
        let crater = if meteor.crater == 0 { even } else { odd };
        for (k, (mesh, material)) in parts.iter().zip([fire, smoke, glow, crater]).enumerate() {
            let Some(material) = material else {
                continue;
            };
            let mut part = commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                ChildOf(shown),
            ));
            // The crater takes the light at it (`+0xA0`), as the ground does; the rest is untinted.
            if k < 3 {
                part.insert(RenderLayers::layer(iso::UNTINTED));
            }
        }
        let rock = looks.model.as_ref().map(|model| {
            let mut rock = commands.spawn((
                models::scene(&assets, model),
                Transform::from_scale(Vec3::ZERO),
                models::Ordered::default(),
                DespawnOnExit(AppState::InGame),
            ));
            if let Some(texture) = &looks.texture {
                rock.insert(Retexture::load(&assets, texture));
            }
            rock.id()
        });
        commands.entity(shown).insert(Shown {
            meteor,
            dice,
            rock,
            parts,
        });
    }
}

/// A point of the meteor's frame about its place, its x along the screen's horizontal on the
/// ground (docs/re/effects.md, "Their flight"), at the world's scale.
fn frame([x, y, z]: [f32; 3]) -> Vec3 {
    let (across, depth) = iso::fx_axes();
    (across * x + depth * y + Vec3::Y * z) * iso::MODEL_SCALE
}

/// Runs each meteor a frame on (`0x7866C0`, `0x786AC0`): its rock and its glow at its head while
/// it falls, its fire and smoke, its quake as it lands and its crater after; once its crater has
/// faded it goes.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &Transform)>,
    mut rocks: Query<&mut Transform, Without<Shown>>,
    (mut meshes, orders): (ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut look, place) in &mut shown {
        let look = &mut *look;
        let place = place.translation;
        if look.meteor.update(dt, &mut look.dice) {
            commands.queue(|world: &mut World| {
                if let Some(mut weather) = world.get_resource_mut::<crate::weather::Weather>() {
                    weather.set(opensacred_rules::weather::Event::Quake(
                        particles::METEOR_QUAKE,
                    ));
                }
            });
        }
        if look.meteor.done() {
            if let Some(rock) = look.rock {
                commands.entity(rock).despawn();
            }
            commands.entity(entity).despawn();
            continue;
        }
        let head = frame(look.meteor.head);
        let falling = look.meteor.falling_now();
        if let Some(mut rock) = look.rock.and_then(|rock| rocks.get_mut(rock).ok()) {
            let [a, b] = look.meteor.turns;
            *rock = Transform::from_translation(place + head)
                .with_rotation(Quat::from_rotation_z(b) * Quat::from_rotation_x(a))
                .with_scale(Vec3::splat(if falling {
                    particles::METEOR_SCALE * s
                } else {
                    0.0
                }));
        }
        let [ramp_fire, ramp_smoke] = Meteor::ramps();
        let mut quads: [Quads; 4] = std::array::from_fn(|_| Quads::default());
        for particle in &look.meteor.system.particles {
            let (part, ramp) = if particle.layer == 0 {
                (0, &ramp_fire)
            } else {
                (1, &ramp_smoke)
            };
            quads[part].push(
                frame(particle.place),
                particle.size * s,
                particle.angle,
                (Vec2::ZERO, 1.0),
                ramp.at(particle.life),
            );
        }
        if falling {
            quads[2].push(
                head,
                particles::METEOR_GLOW_HALF * s,
                0.0,
                (Vec2::ZERO, 1.0),
                particles::METEOR_GLOW_COLOUR,
            );
        }
        if let Some(alpha) = look.meteor.crater_alpha() {
            let (across, depth) = iso::fx_axes();
            let half = particles::METEOR_CRATER_HALF * s;
            let ground = Vec3::Y * GROUND_LIFT;
            quads[3].push_corners(
                [
                    ground + (-across - depth) * half,
                    ground + (across - depth) * half,
                    ground + (across + depth) * half,
                    ground + (-across + depth) * half,
                ],
                [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
                [1.0, 1.0, 1.0, alpha],
            );
        }
        // At its object's place in the order, as the original draws the system (#2188).
        let order = orders.at(place);
        for (handle, mut part) in look.parts.iter().zip(quads) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.in_order(order, place);
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The meteors' looks.
pub struct MeteorsPlugin;

impl Plugin for MeteorsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>().add_systems(
            Update,
            (make, run)
                .chain()
                .in_set(Playing)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
