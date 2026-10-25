//! The Daemon's breath of fire 1102 (#760; docs/re/arts.md, "The Daemon's"): it leaves him toward
//! his point at `150·(1 − t/10)` units a second, bouncing over the ground, turning back from a cell
//! that may not be walked on, and burns each hostile creature it meets once
//! (`opensacred_rules::arts::Burn`, kept on him, `burns`); as far as the point, at most 650, it
//! stops, lays a pair of fires every 0.15 s across its line (`wall_spreads`), and goes; each fire
//! burns for 10 s each creature it meets once. A ball and flames of one colour stand in for their
//! particles (docs/decisions/2026-10-24-bolts-look.md).

use bevy::prelude::*;
use opensacred_rules::arts::{
    BREATH_MOST, Burn, WALL_BURNS, WALL_EVERY, breath_height, breath_speed, wall_spreads,
};

use crate::ai::Mind;
use crate::burns::Burns;
use crate::crowd::InTheWay;
use crate::fight::Health;
use crate::frame::{Playing, Step};
use crate::game::AppState;
use crate::hero::Body;
use crate::npcs::Dies;
use crate::terrain::{Heights, Level};
use crate::{arrows, paths};

/// How high the breath rises over the ground at its start (`+0xAC`, its maker's height and 80).
const RISE: f32 = 80.0;

/// What a breath and its fires burn with: its maker, its fire a blow and its seconds.
#[derive(Clone, Copy, Debug)]
pub struct Fire {
    pub by: Entity,
    pub damage: f32,
    pub seconds: f32,
}

/// A breath on its way: its fire, where it left, its direction, how far it flies, its time, its
/// level, the creatures it has burned, and once stopped how many pairs of fires it has laid.
#[derive(Component)]
pub struct Breath {
    fire: Fire,
    from: Vec2,
    direction: Vec2,
    distance: f32,
    time: f32,
    level: Level,
    touched: Vec<Entity>,
    laid: Option<usize>,
}

impl Breath {
    /// A breath of `fire` from `from` toward `to`, on `level`.
    pub fn new(fire: Fire, from: Vec2, to: Vec2, level: Level) -> Self {
        let line = to - from;
        Self {
            fire,
            from,
            direction: line.normalize_or(Vec2::X),
            distance: line.length().min(BREATH_MOST),
            time: 0.0,
            level,
            touched: Vec::new(),
            laid: None,
        }
    }
}

/// A fire of a breath's wall: its fire, its place, its level, how long it burns on, and the
/// creatures it has burned.
#[derive(Component)]
struct WallFire {
    fire: Fire,
    at: Vec2,
    level: Level,
    left: f32,
    touched: Vec<Entity>,
}

/// The looks of the breath and of a wall's fire.
#[derive(Resource)]
struct Looks {
    breath: (Handle<Mesh>, Handle<StandardMaterial>),
    fire: (Handle<Mesh>, Handle<StandardMaterial>),
}

/// Makes the looks: an orange ball, and a translucent orange flame (**estimate**).
fn make_looks(
    mut commands: Commands,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        return;
    };
    commands.insert_resource(Looks {
        breath: (
            meshes.add(Sphere::new(20.0)),
            materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.55, 0.1),
                unlit: true,
                ..default()
            }),
        ),
        fire: (
            meshes.add(Cylinder::new(22.0, 60.0)),
            materials.add(StandardMaterial {
                base_color: Color::srgba(1.0, 0.45, 0.05, 0.5),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..default()
            }),
        ),
    });
}

/// The creatures on `level` alive and hostile to the hero, not in `touched`, nearer `at` than 50
/// and their sizes.
fn met<'a>(
    creatures: impl Iterator<
        Item = (
            Entity,
            &'a Transform,
            &'a InTheWay,
            &'a Mind,
            &'a Health,
            Option<&'a arrows::Size>,
        ),
    >,
    at: Vec2,
    level: Level,
    touched: &[Entity],
) -> Vec<Entity> {
    creatures
        .filter(|(entity, place, way, mind, health, size)| {
            way.creature
                && way.level == level
                && mind.hostile()
                && health.current > 0
                && !touched.contains(entity)
                && (place.translation.xz() - at).length()
                    < arrows::size(0) + size.map_or(arrows::size(0), |size| size.0)
        })
        .map(|(entity, ..)| entity)
        .collect()
}

/// Burns `targets` by `fire`, kept on its maker.
fn burn(commands: &mut Commands, fire: Fire, targets: Vec<Entity>) {
    if targets.is_empty() {
        return;
    }
    commands
        .entity(fire.by)
        .entry::<Burns>()
        .or_default()
        .and_modify(move |mut burns| {
            for target in targets {
                burns.add(target, Burn::new(fire.damage, 1, fire.seconds));
            }
        });
}

/// Gives each new breath its look.
fn show(
    mut commands: Commands,
    looks: Option<Res<Looks>>,
    breaths: Query<(Entity, &Breath), Added<Breath>>,
    heights: Res<Heights>,
) {
    let Some(looks) = looks else {
        return;
    };
    for (entity, breath) in &breaths {
        let ground = heights
            .standing(breath.from.x, breath.from.y, breath.level)
            .unwrap_or(0.0);
        commands.entity(entity).insert((
            Mesh3d(looks.breath.0.clone()),
            MeshMaterial3d(looks.breath.1.clone()),
            Transform::from_xyz(breath.from.x, ground + RISE, breath.from.y),
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// Flies each breath a frame on, burns what it meets, and lays its wall where it stops
/// (`0x5A6680`, `0x5AFB64`, `0x5B3F1C`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn breathe(
    mut commands: Commands,
    time: Res<Time>,
    heights: Res<Heights>,
    looks: Option<Res<Looks>>,
    mut breaths: Query<(Entity, &mut Breath, &mut Transform)>,
    creatures: Query<
        (
            Entity,
            &Transform,
            &InTheWay,
            &Mind,
            &Health,
            Option<&arrows::Size>,
        ),
        (Without<Body>, Without<Dies>, Without<Breath>),
    >,
) {
    let dt = time.delta_secs();
    let spreads = wall_spreads();
    for (entity, mut breath, mut transform) in &mut breaths {
        breath.time += dt;
        let here = transform.translation.xz();
        if let Some(laid) = breath.laid {
            // Stopped: a pair of fires across its line every 0.15 s, then it goes.
            let mut laid = laid;
            let next =
                |laid: usize| WALL_EVERY * f32::from(u16::try_from(laid + 1).unwrap_or(u16::MAX));
            while laid < spreads.len() && breath.time >= next(laid) {
                let across = Vec2::new(-breath.direction.y, breath.direction.x) * spreads[laid];
                for at in [here + across, here - across] {
                    if !heights.walkable_on(paths::cell_of(at.x, at.y), breath.level) {
                        continue;
                    }
                    let ground = heights.standing(at.x, at.y, breath.level).unwrap_or(0.0);
                    let mut fire = commands.spawn((
                        WallFire {
                            fire: breath.fire,
                            at,
                            level: breath.level,
                            left: WALL_BURNS,
                            touched: Vec::new(),
                        },
                        Transform::from_xyz(at.x, ground + 30.0, at.y),
                        DespawnOnExit(AppState::InGame),
                    ));
                    if let Some(looks) = &looks {
                        fire.insert((
                            Mesh3d(looks.fire.0.clone()),
                            MeshMaterial3d(looks.fire.1.clone()),
                        ));
                    }
                }
                laid += 1;
            }
            breath.laid = Some(laid);
            if laid >= spreads.len() {
                commands.entity(entity).despawn();
            }
            continue;
        }
        let step = breath.direction * breath_speed(breath.time) * dt;
        let mut at = here + step;
        if !heights.walkable_on(paths::cell_of(at.x, at.y), breath.level) {
            breath.direction = -breath.direction;
            at = here;
        }
        let ground = heights.standing(at.x, at.y, breath.level).unwrap_or(0.0);
        transform.translation = Vec3::new(at.x, ground + breath_height(breath.time, RISE), at.y);
        let burned = met(creatures.iter(), at, breath.level, &breath.touched);
        breath.touched.extend(&burned);
        burn(&mut commands, breath.fire, burned);
        if (at - breath.from).length() >= breath.distance || breath_speed(breath.time) <= 0.0 {
            debug!("breath: {entity} stops at {:.0},{:.0}", at.x, at.y);
            breath.laid = Some(0);
            breath.time = 0.0;
        }
    }
}

/// Burns what each fire of a wall meets, while it burns.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn flare(
    mut commands: Commands,
    time: Res<Time>,
    mut fires: Query<(Entity, &mut WallFire)>,
    creatures: Query<
        (
            Entity,
            &Transform,
            &InTheWay,
            &Mind,
            &Health,
            Option<&arrows::Size>,
        ),
        (Without<Body>, Without<Dies>),
    >,
) {
    for (entity, mut fire) in &mut fires {
        fire.left -= time.delta_secs();
        if fire.left <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let burned = met(creatures.iter(), fire.at, fire.level, &fire.touched);
        fire.touched.extend(&burned);
        burn(&mut commands, fire.fire, burned);
    }
}

/// The Daemon's breath of fire.
pub struct BreathPlugin;

impl Plugin for BreathPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, make_looks).add_systems(
            Update,
            (show, breathe, flare)
                .chain()
                .after(Step::Npcs)
                .in_set(Playing),
        );
    }
}
