//! The look of the lightning 13 (#1092; docs/re/effects.md, "The lightning"): at what it strikes,
//! `cParticleSystem_lightningStrike` draws (798, `0x77D000`) three bolts rising from it out of the
//! screen for 0.25 s, each of segments 250 high slanting at random and its strip of
//! `FX_LIGHTSTRIKE05` turned over at random at each draw, a glow for 0.1 s and a burst of sparks
//! (`opensacred_rules::particles::{LightningBolt, lightning_sparks}`, by `puffs`), all added
//! whole and untinted by the night, its light of 200 while its sparks fall; a creature's of
//! `FX_LIGHTSTRIKE06` in violet (`+0xA9`).
//!
//! Not followed: the bolts drawn over all else (its depth test off), and the glow at the
//! system's first point, which the game takes for its place on the ground.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, LightningBolt, lightning_sparks};
use opensacred_rules::spawning::Rng;

use crate::frame::Playing;
use crate::fx::{Effect, Quads, empty, particle_texture, repeated_texture, whole_material};
use crate::game::{AppState, Data};
use crate::{daynight, iso};

/// How many segments a bolt rises by: past the top of the screen at the farthest zoom, where the
/// original's stops (`0x6251E0`).
const SEGMENTS: u16 = 12;

/// How long a lightning lives: as long as its sparks (the life −250 a second).
const LIVES: f32 = 255.0 / 250.0;

/// A lightning to make at the place `at`, a hero's or a creature's.
#[derive(Component)]
struct Unmade {
    at: Vec3,
    hero: bool,
}

/// A lightning: its clock, its bolts, its kind (0 a hero's, 1 a creature's), its dice of the
/// strips' turns, its lamp, and its parts' meshes, the bolts' and the glow's.
#[derive(Component)]
struct Strike {
    clock: f32,
    bolts: [LightningBolt; particles::LIGHTNING_BOLTS],
    kind: usize,
    dice: Rng,
    lamp: Entity,
    parts: [Handle<Mesh>; 2],
}

/// The bolts' materials, a hero's and a creature's, and the glow's, made as the first lightning
/// strikes; none where a texture is missing.
#[derive(Resource, Default)]
struct Materials(Option<[Option<Handle<Effect>>; 3]>);

/// The lightnings' dice.
#[derive(Resource)]
struct Rolls(Rng);

impl Default for Rolls {
    fn default() -> Self {
        Self(crate::seed::dice(0x31E))
    }
}

/// A lightning striking at the place `at`, a hero's or a creature's (`spells`, `creature_casts`).
pub(crate) fn strike(commands: &mut Commands, at: Vec3, hero: bool) {
    commands.spawn((Unmade { at, hero }, DespawnOnExit(AppState::InGame)));
}

/// Makes each lightning asked for (`0x77D5C0`): its sparks, its bolts, its glow and its lamp.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    mut rolls: ResMut<Rolls>,
    (data, assets, mut made, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Materials>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (entity, &Unmade { at, hero }) in &unmade {
        let made = made
            .0
            .get_or_insert_with(|| {
                let mut warned = |image: Option<Handle<Image>>, name: &str| {
                    if image.is_none() {
                        warn!("lightning strikes: no texture {name}; its part is not drawn");
                    }
                    image.map(|image| materials.add(whole_material(image)))
                };
                let assets_dir = data.as_ref().map(|data| data.0.assets.clone());
                let [hero_bolt, creature_bolt] = particles::LIGHTNING_TEXTURES.map(|name| {
                    let image = assets_dir
                        .as_ref()
                        .and_then(|dir| repeated_texture(dir, &assets, name));
                    warned(image, name)
                });
                let glow = assets_dir.as_ref().and_then(|dir| {
                    particle_texture(dir, &assets, particles::LIGHTNING_GLOW_TEXTURE)
                });
                let glow = warned(glow, particles::LIGHTNING_GLOW_TEXTURE);
                [hero_bolt, creature_bolt, glow]
            })
            .clone();
        let kind = usize::from(!hero);
        crate::puffs::spawn(
            &mut commands,
            lightning_sparks(particles::LIGHTNING_COLOURS[kind], &mut rolls.0),
            at,
            None,
        );
        let lamp = commands
            .spawn((
                Transform::from_translation(at.with_y(0.0)),
                daynight::Light(particles::LIGHTNING_LIGHT),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let parts = [meshes.add(empty()), meshes.add(empty())];
        commands.entity(entity).despawn();
        let strike = commands
            .spawn((
                Transform::from_translation(at),
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        // Its Direct3D light at its place (`0x77CD70`, `0x77D000`).
        commands.spawn(crate::fx_lights::child(crate::fx_lights::PALE_BLUE, strike));
        for (mesh, material) in parts.iter().zip([made[kind].clone(), made[2].clone()]) {
            if let Some(material) = material {
                commands.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                    ChildOf(strike),
                ));
            }
        }
        let index = usize::try_from(strike.index_u32()).unwrap_or(0);
        commands.entity(strike).insert(Strike {
            clock: 0.0,
            bolts: LightningBolt::three(&mut rolls.0),
            kind,
            dice: crate::seed::dice(0x31E ^ index),
            lamp,
            parts,
        });
    }
}

/// Runs each lightning a frame on (`0x77CF70`, `0x77D000`): its bolts for 0.25 s, its strips
/// turned at random at each draw, its glow for 0.1 s; once its sparks are gone it goes, its lamp
/// with it.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut strikes: Query<(Entity, &mut Strike)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let s = iso::MODEL_SCALE;
    let (across, down) = iso::fx_axes();
    let at = |[x, y, z]: [f32; 3]| (across * x + down * y + Vec3::Y * z) * s;
    for (entity, mut strike) in &mut strikes {
        let strike = &mut *strike;
        let clock = strike.clock;
        strike.clock += time.delta_secs();
        if clock >= LIVES {
            commands.entity(strike.lamp).despawn();
            commands.entity(entity).despawn();
            continue;
        }
        let colour = particles::LIGHTNING_COLOURS[strike.kind];
        let mut bolts = Quads::default();
        if clock <= particles::LIGHTNING_BOLTS_SHOW {
            for bolt in strike.bolts {
                for k in 0..SEGMENTS {
                    let flip = strike.dice.below(2) == 1;
                    let corners = bolt.segment(k, flip);
                    bolts.push_corners(
                        corners.map(|(corner, _)| at(corner)),
                        corners.map(|(_, [u, v])| Vec2::new(u, v)),
                        [1.0; 4],
                    );
                }
            }
        }
        let mut glow = Quads::default();
        if clock <= particles::LIGHTNING_GLOW_SHOWS {
            glow.push(
                Vec3::ZERO,
                particles::LIGHTNING_GLOW_HALF * s,
                0.0,
                (Vec2::ZERO, 1.0),
                colour,
            );
        }
        for (handle, quads) in strike.parts.iter().zip([bolts, glow]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The lightnings' looks.
pub struct LightningStrikesPlugin;

impl Plugin for LightningStrikesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Materials>()
            .init_resource::<Rolls>()
            .add_systems(
                Update,
                (make, run)
                    .chain()
                    .in_set(Playing)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
