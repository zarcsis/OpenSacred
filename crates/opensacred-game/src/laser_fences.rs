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
//!
//! A save keeps each maker's timers and each segment flying ([`Saved`]), which its load puts back
//! onto the maker of the same point in place of its run's own (#3450).

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

/// The fences as a save keeps them (#3450; docs/re/effects.md, "The scripts' traps", "The
/// save"): each maker's point and its two timers, and each segment flying with its maker's point.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
// Named apart from another saved type of its name, which the saves' layout test traces by
// name (#2306); postcard writes no names.
#[serde(rename = "SavedFences")]
pub struct Saved {
    pub makers: Vec<(Vec3, script_traps::Fences)>,
    pub segments: Vec<(Vec3, Laser)>,
}

/// The fences that a loaded save kept, each maker's timers and its segments put back onto the
/// maker of its point as it runs, the segments that its run made since the load's start gone
/// ([`make`]): the original's load brings the maker's object and its segments back and runs no
/// `Enter` of the sectors it kept marked.
#[derive(Resource, Default)]
pub(crate) struct Again(pub Saved);

impl Again {
    /// The kept timers of the maker of the point `maker` and its segments, taken.
    fn take(&mut self, maker: Vec3) -> Option<(script_traps::Fences, Vec<Laser>)> {
        let index = self.0.makers.iter().position(|(at, _)| same(*at, maker))?;
        let (_, fences) = self.0.makers.swap_remove(index);
        let (segments, others) = std::mem::take(&mut self.0.segments)
            .into_iter()
            .partition(|(at, _)| same(*at, maker));
        self.0.segments = others;
        Some((
            fences,
            segments.into_iter().map(|(_, laser)| laser).collect(),
        ))
    }

    /// Whether it keeps a maker of the point `maker`.
    fn holds(&self, maker: Vec3) -> bool {
        self.0.makers.iter().any(|(at, _)| same(*at, maker))
    }
}

/// Whether two makers' points are one: a record's point placed again.
fn same(a: Vec3, b: Vec3) -> bool {
    (a - b).abs().max_element() < 1.0
}

/// The fences as they stand, to save (#3450): those that a load kept and that wait for their
/// makers, then each running maker of another point and the segments flying from it.
pub(crate) fn saved(world: &mut World) -> Saved {
    let mut saved = world
        .get_resource::<Again>()
        .map(|again| again.0.clone())
        .unwrap_or_default();
    let kept = Again(saved.clone());
    let mut makers = world.query_filtered::<(&Transform, &Making), With<Maker>>();
    saved.makers.extend(
        makers
            .iter(world)
            .filter(|(at, _)| !kept.holds(at.translation))
            .map(|(at, making)| (at.translation, making.0)),
    );
    let mut lasers = world.query::<&Flying>();
    saved.segments.extend(
        lasers
            .iter(world)
            .filter(|flying| !kept.holds(flying.maker))
            .map(|flying| (flying.maker, flying.laser)),
    );
    saved
}

/// Forgets the fences that a loaded save kept, as another game loads or the menu comes.
fn forget(mut again: ResMut<Again>) {
    again.0 = Saved::default();
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

/// Where a segment of the models' place `place` about its maker's point `maker` stands in the
/// world: on the ground as the flight's place, its height raised as a placed object's.
fn placed(maker: Vec3, place: [f32; 3]) -> Vec3 {
    maker
        + iso::flight([place[0], place[1]])
        + Vec3::Y * iso::height(place[2] * iso::PIXELS_PER_MODEL_HEIGHT)
}

/// Spawns a segment flying from the maker's point `maker`, its ribbon and glows of `looks`, a
/// fence's first with its pale blue light.
fn spawn_segment(
    commands: &mut Commands,
    laser: Laser,
    maker: Vec3,
    looks: &Looks,
    meshes: &mut Assets<Mesh>,
) {
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
    if laser.segment.index == 0 {
        commands.spawn(crate::fx_lights::child(
            crate::fx_lights::ICE_BLUE,
            crate::fx_lights::HIGH,
            flier,
        ));
    }
}

/// Sends each maker's fences as they come (`0x5AA82C`): eight segments each, a fence's first
/// with its pale blue light; a maker that a loaded save kept takes its timers and segments back
/// ([`Again`]).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn make(
    mut commands: Commands,
    time: Res<Time>,
    mut makers: Query<(Entity, &Transform, Option<&mut Making>), With<Maker>>,
    lasers: Query<(Entity, &Flying)>,
    (data, server, mut looks, mut effects): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Assets<Effect>>,
    ),
    (mut dice, mut meshes, mut again): (ResMut<Dice>, ResMut<Assets<Mesh>>, ResMut<Again>),
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
        if let Some((fences, segments)) = again.take(maker) {
            info!(
                "laser fences: the maker at {maker} as the save kept it, {} segments flying",
                segments.len()
            );
            for (laser, _) in lasers
                .iter()
                .filter(|(_, flying)| same(flying.maker, maker))
            {
                commands.entity(laser).despawn();
            }
            making.0 = fences;
            for laser in segments {
                spawn_segment(&mut commands, laser, maker, &looks, &mut meshes);
            }
        }
        for (fast, due) in [false, true].into_iter().zip(making.0.update(dt)) {
            if !due {
                continue;
            }
            debug!("laser fences: a fence, fast {fast}");
            for segment in script_traps::fence(fast, &mut dice.dice) {
                spawn_segment(
                    &mut commands,
                    Laser::new(segment),
                    maker,
                    &looks,
                    &mut meshes,
                );
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
    let end = iso::flight([ax * segment.length, ay * segment.length]);
    let across = iso::flight([-ay, ax]);
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
        let end = iso::flight([ax * segment.length, ay * segment.length]);
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
            .init_resource::<Again>()
            .add_systems(OnEnter(AppState::Loading), forget)
            .add_systems(OnEnter(AppState::Menu), forget)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A segment of a slow fence or a fast one.
    fn laser(fast: bool) -> Laser {
        Laser::new(script_traps::fence(fast, &mut crate::seed::dice(2))[0])
    }

    /// A maker's timers some time after it was made.
    fn timers(after: f32) -> script_traps::Fences {
        let mut fences = script_traps::Fences::default();
        fences.update(0.0);
        fences.update(after);
        fences
    }

    #[test]
    fn a_kept_maker_takes_its_timers_and_its_segments_alone() {
        let here = Vec3::new(10.0, 0.0, 20.0);
        let there = Vec3::new(500.0, 0.0, 20.0);
        let mut again = Again(Saved {
            makers: vec![(here, timers(1.0)), (there, timers(2.0))],
            segments: vec![
                (here, laser(false)),
                (there, laser(true)),
                (here, laser(true)),
            ],
        });
        assert!(again.take(Vec3::new(10.0, 0.0, 900.0)).is_none());
        let (fences, segments) = again.take(here + Vec3::splat(0.5)).unwrap();
        assert_eq!(fences, timers(1.0));
        assert_eq!(segments, vec![laser(false), laser(true)]);
        assert_eq!(again.0.makers, vec![(there, timers(2.0))]);
        assert_eq!(again.0.segments, vec![(there, laser(true))]);
        assert!(again.take(here).is_none());
        assert!(!again.holds(there + Vec3::X));
        assert!(again.holds(there));
    }

    #[test]
    fn a_save_keeps_the_makers_and_segments_and_those_a_load_kept() {
        let here = Vec3::new(10.0, 0.0, 20.0);
        let there = Vec3::new(500.0, 0.0, 20.0);
        let mut world = World::new();
        for (at, after) in [(here, 1.0), (there, 2.0)] {
            world.spawn((
                Maker,
                Transform::from_translation(at),
                Making(timers(after)),
            ));
            world.spawn(Flying {
                laser: laser(false),
                maker: at,
                meshes: [Handle::default(), Handle::default()],
            });
        }
        // A maker not yet run keeps nothing.
        world.spawn((
            Maker,
            Transform::from_translation(Vec3::new(0.0, 0.0, 900.0)),
        ));
        let all = saved(&mut world);
        assert_eq!(all.makers.len(), 2);
        assert!(all.makers.contains(&(here, timers(1.0))));
        assert!(all.makers.contains(&(there, timers(2.0))));
        assert_eq!(all.segments.len(), 2);
        // What a load kept and has not put back yet stands for its maker's run.
        world.insert_resource(Again(Saved {
            makers: vec![(here, timers(0.5))],
            segments: vec![(here, laser(true)), (here, laser(true))],
        }));
        let kept = saved(&mut world);
        assert_eq!(kept.makers.len(), 2);
        assert!(kept.makers.contains(&(here, timers(0.5))));
        assert!(kept.makers.contains(&(there, timers(2.0))));
        assert_eq!(kept.segments.len(), 3);
        assert!(kept.segments.contains(&(there, laser(false))));
        assert!(!kept.segments.contains(&(here, laser(false))));
    }
}
