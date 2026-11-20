//! The scripts' fences of lightning (`FX_TRAP03_PARENT`, 894, with 895; #2916;
//! docs/re/effects.md, "The scripts' traps"): the maker that a sector's `Enter` makes (`npcs`),
//! in 69,9 by the door of main quest 61, sends a slow fence every 3 s outward along the world's
//! +x and a fast one every 5.5 s back toward it (`opensacred_rules::script_traps::Fences`), each
//! of eight segments end to end (`fence`) that fly through walls, stop, fade and end
//! (`Laser`), each striking every living creature within its reach of its start, the hero or
//! another, hostile or not, from nobody, 200 physical and 100 magic, at most once a second
//! (`fight::Burn`).
//!
//! Each segment is a ribbon of `FX_LIGHTNING01`, flat and upright, its joints swaying and its
//! texture jumping each frame by the manager's table of random vectors, grey by its fade, added
//! whole, with a glow of `PARTICLE_GLOW06` at each end of its fence, untinted by the night; it
//! lights the world about it by its fade (`daynight::Light`), and a fence's first segment the
//! models about it in pale blue (`fx_lights`). The segments' places in the models' frame go to
//! the ground as the flight's do (`0x5AA330` by `0x623AD0`): a unit across the screen
//! `MODEL_SCALE` world units, one up it `2/√5` of that.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::diagnostic::FrameCount;
use bevy::prelude::*;
use opensacred_rules::script_traps::{
    self, LASER_DAMAGES, LASER_GLOW_TEXTURE, LASER_HALF_WIDTH, LASER_SIZE, LASER_TEXTURE, Laser,
    Segment,
};
use opensacred_rules::spawning::Rng;

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, particle_texture, whole_material, wrapped_texture};
use crate::game::{AppState, Data};
use crate::iso;

/// A maker of fences that a script made, at its record's point on the plane of the height 0.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Maker;

/// A maker's two timers, once it runs.
#[derive(Component, Default)]
struct Making(script_traps::Fences);

/// A segment flying: its rules, its maker's point, and the meshes of its ribbon and its glows.
#[derive(Component)]
struct Flying {
    laser: Laser,
    maker: Vec3,
    meshes: [Handle<Mesh>; 2],
}

/// The segments' materials, the ribbon's and the glows', made as the first maker runs; none
/// where a texture is missing.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    ribbon: Option<Handle<Effect>>,
    glow: Option<Handle<Effect>>,
}

/// The fences' dice (`rand()`; **estimate**: any seed), and the systems' manager's table of 256
/// random vectors that their joints sway by.
#[derive(Resource)]
struct Dice {
    dice: Rng,
    table: [[f32; 3]; 256],
}

impl Default for Dice {
    fn default() -> Self {
        let mut dice = crate::seed::dice(0x37E);
        let table = script_traps::sway_table(&mut dice);
        Self { dice, table }
    }
}

/// The foreshortening of the models' frame's second axis on the ground, the camera's tilt.
const TILT: f32 = 0.894_427_2;

/// The world's vector on the ground of `[x, y]` in the models' frame, as the flight takes the
/// system's place to the ground (`0x5AA330` by `0x623AD0`; docs/re/effects.md, "Their flight").
fn ground([x, y]: [f32; 2]) -> Vec3 {
    let (across, down) = iso::fx_axes();
    (across * x + down * (y * TILT)) * iso::MODEL_SCALE
}

/// Where a segment of the models' place `place` about its maker's point `maker` stands in the
/// world: on the ground as the flight's place, its height raised as a placed object's.
fn placed(maker: Vec3, place: [f32; 3]) -> Vec3 {
    maker
        + ground([place[0], place[1]])
        + Vec3::Y * iso::height(place[2] * iso::PIXELS_PER_MODEL_HEIGHT)
}

/// Sends each maker's fences as they come (`0x5AA82C`): eight segments each, a fence's first
/// with its pale blue light.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn make(
    mut commands: Commands,
    time: Res<Time>,
    mut makers: Query<(Entity, &Transform, Option<&mut Making>), With<Maker>>,
    (data, server, mut looks, mut effects): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Assets<Effect>>,
    ),
    (mut dice, mut meshes): (ResMut<Dice>, ResMut<Assets<Mesh>>),
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, at, making) in &mut makers {
        let Some(mut making) = making else {
            commands.entity(entity).insert(Making::default());
            continue;
        };
        if !looks.made {
            let Some(data) = data.as_deref() else {
                return;
            };
            looks.made = true;
            let assets = &data.0.assets;
            looks.ribbon = wrapped_texture(assets, &server, LASER_TEXTURE)
                .map(|image| effects.add(whole_material(image)));
            looks.glow = particle_texture(assets, &server, LASER_GLOW_TEXTURE)
                .map(|image| effects.add(whole_material(image)));
            if looks.ribbon.is_none() || looks.glow.is_none() {
                warn!("laser fences: a texture is missing; its part is not drawn");
            }
        }
        let maker = at.translation;
        for (fast, due) in [false, true].into_iter().zip(making.0.update(dt)) {
            if !due {
                continue;
            }
            debug!("laser fences: a fence, fast {fast}");
            for segment in script_traps::fence(fast, &mut dice.dice) {
                let laser = Laser::new(segment);
                let mut flying = commands.spawn((
                    Transform::from_translation(placed(maker, laser.place)),
                    Visibility::default(),
                    crate::daynight::Light(laser.light()),
                    DespawnOnExit(AppState::InGame),
                ));
                let flier = flying.id();
                let made = [looks.ribbon.clone(), looks.glow.clone()].map(|material| {
                    let mesh = meshes.add(empty());
                    if let Some(material) = material {
                        flying.with_child((
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(material),
                            Transform::IDENTITY,
                            NoFrustumCulling,
                            RenderLayers::layer(iso::UNTINTED),
                        ));
                    }
                    mesh
                });
                flying.insert(Flying {
                    laser,
                    maker,
                    meshes: made,
                });
                if segment.index == 0 {
                    commands.spawn(crate::fx_lights::child(
                        crate::fx_lights::ICE_BLUE,
                        crate::fx_lights::HIGH,
                        flier,
                    ));
                }
            }
        }
    }
}

/// Flies each segment a frame on (`0x5A6680`, `0x7A2C50`), its light by its fade, and strikes
/// from it, at most once a second, every living creature on the ground within its size and the
/// creature's of its start (`0x5AB490`, `0x5ABD80`); ends it as it has faded.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    mut lasers: Query<(
        Entity,
        &mut Flying,
        &mut Transform,
        &mut crate::daynight::Light,
    )>,
    mut dice: ResMut<Dice>,
    mut burns: MessageWriter<crate::fight::Burn>,
    creatures: Query<
        (
            Entity,
            &Transform,
            &crate::crowd::InTheWay,
            &crate::fight::Health,
            Option<&crate::arrows::Size>,
        ),
        (
            Without<crate::hero::Body>,
            Without<crate::npcs::Dies>,
            Without<Flying>,
        ),
    >,
    heroes: Query<
        (
            Entity,
            &Transform,
            &crate::hero::Body,
            &crate::fight::Health,
            Option<&crate::arrows::Size>,
        ),
        (Without<crate::npcs::Dies>, Without<Flying>),
    >,
) {
    let dt = time.delta_secs().min(0.1);
    let own = opensacred_rules::arrows::size(0);
    for (entity, mut flying, mut at, mut light) in &mut lasers {
        let roll = f32::from(u16::try_from(dice.dice.below(32_768)).unwrap_or(0)) / 32_767.0;
        if !flying.laser.update(dt, roll) {
            commands.entity(entity).despawn();
            continue;
        }
        at.translation = placed(flying.maker, flying.laser.place);
        light.0 = flying.laser.light();
        let here = at.translation.xz();
        let within = |place: &Transform, size: Option<&crate::arrows::Size>| {
            let reach = LASER_SIZE + size.map_or(own, |size| size.0);
            (place.translation.xz() - here).length() < reach
        };
        let creatures = creatures
            .iter()
            .filter(|(_, place, way, health, size)| {
                way.creature && way.level.is_none() && health.current > 0 && within(place, *size)
            })
            .map(|(target, ..)| target);
        let heroes = heroes
            .iter()
            .filter(|(_, place, body, health, size)| {
                body.level().is_none() && health.current > 0 && within(place, *size)
            })
            .map(|(target, ..)| target);
        let targets: Vec<Entity> = creatures.chain(heroes).collect();
        if flying.laser.strikes(!targets.is_empty()) {
            for target in targets {
                debug!("laser fences: a segment strikes {target}");
                burns.write(crate::fight::Burn {
                    target,
                    damages: LASER_DAMAGES,
                });
            }
        }
    }
}

/// The corners of a segment's flat quad and of its upright one about its start, its joints
/// swayed across and up by `sway`.
fn ribbon(segment: &Segment, sway: [[f32; 2]; 2]) -> [[Vec3; 4]; 2] {
    let [ax, ay] = segment.along();
    let end = ground([ax * segment.length, ay * segment.length]);
    let across = ground([-ay, ax]);
    let up = world([0.0, 0.0, 1.0]);
    let [near, far] = sway;
    let w = LASER_HALF_WIDTH;
    let start = across * near[0] + up * near[1];
    let end = end + across * far[0] + up * far[1];
    [across, up].map(|side| {
        [
            start - side * w,
            start + side * w,
            end + side * w,
            end - side * w,
        ]
    })
}

/// Draws each segment (`0x7A2CE0`): its ribbon grey by its fade, its texture's `u` and its
/// joints swayed by the frame; a glow at its fence's far end and at its maker's end.
fn draw(
    lasers: Query<(&Flying, &Transform)>,
    (dice, frame, orders): (Res<Dice>, Res<FrameCount>, Orders),
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let s = iso::MODEL_SCALE;
    for (flying, at) in &lasers {
        let laser = &flying.laser;
        let segment = &laser.segment;
        let shade = laser.shade();
        let grey = [shade / 2.0, shade / 2.0, shade / 2.0, 1.0];
        let (sway, shift) = segment.sway(&dice.table, frame.0);
        let mut strip = Quads::default();
        for (corners, [from, to]) in ribbon(segment, sway).into_iter().zip(segment.u()) {
            let [from, to] = [from + shift, to + shift];
            strip.push_corners(
                corners,
                [
                    Vec2::new(from, 0.0),
                    Vec2::new(from, 1.0),
                    Vec2::new(to, 1.0),
                    Vec2::new(to, 0.0),
                ],
                grey,
            );
        }
        let mut glows = Quads::default();
        let [ax, ay] = segment.along();
        let end = ground([ax * segment.length, ay * segment.length]);
        let white = [shade, shade, shade, 1.0];
        for (shows, centre) in segment.glows().into_iter().zip([Vec3::ZERO, end]) {
            if shows {
                glows.push_facing(centre, laser.glow * s, [white; 4]);
            }
        }
        let order = orders.at(at.translation);
        for (mut quads, mesh) in [strip, glows].into_iter().zip(&flying.meshes) {
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                quads.in_order(order, at.translation);
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

pub struct LaserFencesPlugin;

impl Plugin for LaserFencesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (make, fly, draw)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
