//! The butterflies (#1073; docs/re/effects.md, "The butterflies", docs/re/weather.md): as a sector
//! of the butterflies' weather entries comes in, ten swarms an entry at walkable places at random
//! (the event of kind 10), one in 31 with a glow; each swarm of 3 to 6 butterflies wanders the
//! walkable ground at 20 to 40 units of the models a second, turning by its own rate and turning
//! back where it may not go, its butterflies fluttering about a point 50 units above it
//! (`cParticleSystem_butterfly`, `0x79B020`, the object's flag `0x8000`). By day they are drawn
//! as billboards of `PARTICLE_BUTTERFLY01` playing its 8 × 8 wing beats over their looping life of
//! 2.55 s, each its own colour at the alpha of its life; by night as fireflies, glows of
//! `PARTICLE_GLOW03` of the colour (`N`, `N`, `N`/2) by the night's level `N`; both at dusk and
//! dawn (`0x79B3E0`). They go with their sector.

use bevy::prelude::*;
use opensacred_rules::spawning::Rng;

use crate::daynight::{self, Clock};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::sectors::Streamer;
use crate::terrain::Heights;
use crate::weather::Weather;
use crate::{iso, paths};

/// The swarms an entry of the butterflies makes (the event of kind 10).
const SWARMS: u32 = 10;
/// One swarm in this many glows (the event's `+0x38` of 1).
const GLOWS_ONE_IN: u32 = 31;
/// The tries for a walkable place of a swarm in its sector (**estimate**).
const TRIES: u32 = 40;
/// A butterfly's size, its flutter's speed and pull toward the point above the swarm, that
/// point's height, and its life's fall a second; in the models' units.
const SIZE: f32 = 4.0;
const FLUTTER: f32 = 13.0;
const PULL: f32 = 8.0;
const ABOVE: f32 = 50.0;
const LIFE_FALL: f32 = 100.0;
/// The night's levels below which the butterflies show and above which the fireflies do, and
/// the glows' half sides.
const DAY_BELOW: f64 = 240.0;
const NIGHT_ABOVE: f64 = 16.0;
const SWARM_GLOW: f32 = 7.5;
const FIREFLY: f32 = 5.0;
/// The atlas of the wings: 8 × 8 frames.
const GRID: f32 = 8.0;
/// The seed of the butterflies' `rand()` (**estimate**: any).
const SEED: usize = 0x369;

/// A swarm: where it is, the way it goes and how fast it turns and goes, whether it glows, its
/// butterflies, and its meshes of the butterflies and of the glows.
#[derive(Component)]
struct Swarm {
    way: Vec2,
    turn: f32,
    speed: f32,
    glow: bool,
    flies: Vec<Fly>,
    meshes: [Handle<Mesh>; 2],
}

/// A butterfly: its offset from its swarm and velocity in the models' units, the seconds to its
/// next turn, its life and its colour.
struct Fly {
    offset: Vec3,
    velocity: Vec3,
    timer: f32,
    life: f32,
    colour: [f32; 4],
}

/// The swarms' materials and dice, and the sectors whose swarms are made.
#[derive(Resource)]
struct Butterflies {
    wings: Option<Handle<StandardMaterial>>,
    glow: Option<Handle<StandardMaterial>>,
    dice: Rng,
    made: Vec<(u32, u32)>,
}

impl Butterflies {
    /// A number from 0 to 1 at random, as the client's `rand()` / 32767.
    fn share(&mut self) -> f32 {
        f32::from(u16::try_from(self.dice.below(0x8000)).unwrap_or(0)) / 32_767.0
    }

    fn signed(&mut self) -> f32 {
        self.share() * 2.0 - 1.0
    }
}

fn begin(
    mut commands: Commands,
    data: Option<Res<Data>>,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut material = |name: &str, alpha_mode| {
        let data = data.as_ref()?;
        let image = particle_texture(&data.0.assets, &assets, name)?;
        Some(materials.add(particle_material(image, alpha_mode)))
    };
    let wings = material("particle_butterfly01", AlphaMode::Blend);
    let glow = material("particle_glow03", AlphaMode::Add);
    commands.insert_resource(Butterflies {
        wings,
        glow,
        dice: Rng::of(0, 0, SEED),
        made: Vec::new(),
    });
}

/// Makes the swarms of each sector that has come in, and forgets the sectors gone.
fn make(
    mut commands: Commands,
    mut butterflies: ResMut<Butterflies>,
    mut streamer: Option<ResMut<Streamer>>,
    weather: Option<Res<Weather>>,
    heights: Res<Heights>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (Some(streamer), Some(weather)) = (streamer.as_mut(), weather) else {
        return;
    };
    let loaded: Vec<(u32, u32)> = streamer.sectors().collect();
    butterflies.made.retain(|at| loaded.contains(at));
    let side = u32::try_from(opensacred_data::world::SIDE).unwrap_or(64);
    for at in loaded {
        if butterflies.made.contains(&at) {
            continue;
        }
        butterflies.made.push(at);
        let entries = weather.butterflies(at);
        for _ in 0..entries * SWARMS {
            let butterflies = &mut *butterflies;
            let mut place = None;
            for _ in 0..TRIES {
                let cell = (
                    at.0 * side + butterflies.dice.below(side),
                    at.1 * side + butterflies.dice.below(side),
                );
                let (Ok(cx), Ok(cy)) = (i32::try_from(cell.0), i32::try_from(cell.1)) else {
                    continue;
                };
                if heights.walkable((cx, cy)) {
                    place = Some(iso::ground(
                        f32::from(u16::try_from(cell.0).unwrap_or(0)) + 0.5,
                        f32::from(u16::try_from(cell.1).unwrap_or(0)) + 0.5,
                    ));
                    break;
                }
            }
            let Some(mut place) = place else {
                continue;
            };
            place.y = heights.standing(place.x, place.z, None).unwrap_or(0.0);
            let glow = butterflies.dice.below(GLOWS_ONE_IN) == 0;
            let way = Vec2::new(butterflies.signed(), butterflies.signed()).normalize_or(Vec2::X);
            let turn = butterflies.share() * 0.3;
            let speed = (20.0 * (1.0 + butterflies.share())).floor();
            let count = 3 + butterflies.dice.below(4);
            let flies = (0..count)
                .map(|_| Fly {
                    offset: Vec3::new(
                        5.0 * butterflies.signed(),
                        5.0 * butterflies.signed(),
                        20.0 + 12.0 * butterflies.signed(),
                    ),
                    velocity: Vec3::ZERO,
                    timer: 0.0,
                    life: 255.0,
                    colour: [
                        (128.0 + 127.0 * butterflies.signed()) / 255.0,
                        (128.0 + 127.0 * butterflies.signed()) / 255.0,
                        (128.0 + 127.0 * butterflies.signed()) / 255.0,
                        1.0,
                    ],
                })
                .collect();
            let wings = meshes.add(empty());
            let glows = meshes.add(empty());
            let mut swarm = commands.spawn((
                Swarm {
                    way,
                    turn,
                    speed,
                    glow,
                    flies,
                    meshes: [wings.clone(), glows.clone()],
                },
                Transform::from_translation(place),
                Visibility::default(),
            ));
            swarm.with_children(|layers| {
                for (mesh, material) in [
                    (wings, butterflies.wings.clone()),
                    (glows, butterflies.glow.clone()),
                ] {
                    if let Some(material) = material {
                        layers.spawn((
                            Mesh3d(mesh),
                            MeshMaterial3d(material),
                            Transform::default(),
                            bevy::camera::visibility::NoFrustumCulling,
                            // Untinted by the night: the system's flags lack `0x40` (#1191).
                            bevy::camera::visibility::RenderLayers::layer(crate::iso::UNTINTED),
                        ));
                    }
                }
            });
            let swarm = swarm.id();
            let (cx, cy) = paths::cell_of(place.x, place.z);
            debug!("butterflies: a swarm of {count} in {at:?} at {cx},{cy}");
            streamer.adopt(&mut commands, at, swarm);
        }
    }
}

/// Moves each swarm and its butterflies a frame on (`0x5A8885`, `0x79B020`).
fn wander(
    time: Res<Time>,
    mut butterflies: ResMut<Butterflies>,
    heights: Res<Heights>,
    mut swarms: Query<(&mut Swarm, &mut Transform)>,
) {
    let dt = time.delta_secs().min(0.1);
    for (mut swarm, mut transform) in &mut swarms {
        let swarm = &mut *swarm;
        let step = swarm.way * swarm.speed * iso::MODEL_SCALE * dt;
        let next = transform.translation + Vec3::new(step.x, 0.0, step.y);
        if heights.walkable(paths::cell_of(next.x, next.z)) {
            transform.translation = next;
            if let Some(y) = heights.standing(next.x, next.z, None) {
                transform.translation.y = y;
            }
        } else {
            swarm.way = -swarm.way;
            swarm.turn = butterflies.share() * 0.2;
        }
        swarm.way = Vec2::from_angle(swarm.turn * dt).rotate(swarm.way);
        let above = Vec3::new(0.0, 0.0, ABOVE);
        for fly in &mut swarm.flies {
            fly.life -= LIFE_FALL * dt;
            if fly.life < 0.0 {
                fly.life += 255.0;
            }
            fly.timer -= dt;
            if fly.timer <= 0.0 {
                fly.timer = 0.5 + 0.7 * butterflies.share();
                let wander = Vec3::new(
                    butterflies.signed(),
                    butterflies.signed(),
                    butterflies.signed(),
                )
                .normalize_or_zero();
                fly.velocity = wander * FLUTTER + (above - fly.offset).normalize_or_zero() * PULL;
            }
            fly.offset += fly.velocity * dt;
        }
    }
}

/// Lays each swarm's butterflies and glows out by the night's level (`0x79B3E0`).
fn draw(clock: Option<Res<Clock>>, swarms: Query<&Swarm>, mut meshes: ResMut<Assets<Mesh>>) {
    let night = clock.map_or(0.0, |clock| daynight::level(clock.minutes()));
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the night's level, 0 to 255"
    )]
    let n = (night / 255.0) as f32;
    for swarm in &swarms {
        let mut wings = Quads::default();
        let mut glows = Quads::default();
        // The models' frame, its third coordinate up, at the world's scale.
        let place = |offset: Vec3| Vec3::new(offset.x, offset.z, offset.y) * iso::MODEL_SCALE;
        if night < DAY_BELOW {
            for fly in &swarm.flies {
                // Its wing beat, a frame of 64 by its age (`⌊(255 − life) · 64 / 256⌋`).
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "a frame of 0 to 63"
                )]
                let frame = ((255.0 - fly.life) * 64.0 / 256.0).clamp(0.0, 63.0) as u8;
                let corner = Vec2::new(f32::from(frame % 8), f32::from(frame / 8)) / GRID;
                let [r, g, b, _] = fly.colour;
                wings.push(
                    place(fly.offset),
                    SIZE * iso::MODEL_SCALE,
                    0.0,
                    (corner, 1.0 / GRID),
                    [r, g, b, fly.life / 255.0],
                );
            }
            if swarm.glow
                && let Some(first) = swarm.flies.first()
            {
                glows.push(
                    place(first.offset),
                    SWARM_GLOW * iso::MODEL_SCALE,
                    0.0,
                    (Vec2::ZERO, 1.0),
                    first.colour,
                );
            }
        }
        if night > NIGHT_ABOVE {
            for fly in &swarm.flies {
                glows.push(
                    place(fly.offset),
                    FIREFLY * iso::MODEL_SCALE,
                    0.0,
                    (Vec2::ZERO, 1.0),
                    [n, n, n / 2.0, 1.0],
                );
            }
        }
        for (mesh, quads) in swarm.meshes.iter().zip([wings, glows]) {
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

fn end(mut commands: Commands) {
    commands.remove_resource::<Butterflies>();
}

/// The butterflies and the fireflies.
pub struct ButterfliesPlugin;

impl Plugin for ButterfliesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), begin)
            .add_systems(OnExit(AppState::InGame), end)
            .add_systems(
                Update,
                (make, wander, draw)
                    .chain()
                    .run_if(resource_exists::<Butterflies>)
                    .in_set(Playing)
                    .in_set(Unpaused),
            );
    }
}
