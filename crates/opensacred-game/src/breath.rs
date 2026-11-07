//! The Daemon's breath of fire 1102 (#760; docs/re/arts.md, "The Daemon's"): it leaves him toward
//! his point at `150·(1 − t/10)` units a second, bouncing over the ground, turning back from a cell
//! that may not be walked on, and burns each hostile creature it meets once
//! (`opensacred_rules::arts::Burn`, kept on him, `burns`); as far as the point, at most 650, it
//! stops, lays a pair of fires every 0.15 s across its line (`wall_spreads`), and goes; each fire
//! burns for 10 s each creature it meets once.
//! They look as their systems (#1092; docs/re/effects.md, "The breath of fire"): the breath leaves
//! flames and smoke along its way while it flies, its glow at it and its light of 100 with it
//! until its last flame dies (`opensacred_rules::particles::walloffire`); each fire of its wall
//! burns flames standing on the ground for its 10 s, its light of 200 with it until its last
//! (`walloffire_line`); all untinted by the night (`iso::UNTINTED`). The breath's look lies at its
//! breath's place in the original's order of drawing, each fire's flames at the fire's place
//! (`fx::Orders::at`, #2220).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::arts::{
    BREATH_MOST, Burn, WALL_BURNS, WALL_EVERY, breath_height, breath_speed, wall_spreads,
};
use opensacred_rules::particles::{self, System, walloffire, walloffire_line};
use opensacred_rules::spawning::Rng;

use crate::ai::Mind;
use crate::bolt_looks::{models, world};
use crate::burns::Burns;
use crate::crowd::InTheWay;
use crate::fight::Health;
use crate::frame::{Playing, Step};
use crate::fx::{Effect, Orders, Quads, empty, layer_quads, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::hero::Body;
use crate::npcs::Dies;
use crate::terrain::{Heights, Level};
use crate::{arrows, daynight, iso, paths};

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

/// A fire of a breath's wall: its fire, its place and its ground's height, its level, how long it
/// burns on, the creatures it has burned, and its look: its flames, their dice and their mesh.
#[derive(Component)]
struct WallFire {
    fire: Fire,
    at: Vec2,
    ground: f32,
    level: Level,
    left: f32,
    touched: Vec<Entity>,
    flames: System,
    dice: Rng,
    mesh: Handle<Mesh>,
}

/// The seed of the looks' `rand()` (**estimate**: any).
const SEED: usize = 0x3EE;

/// A breath's look, which outlives its breath while its flames burn: its breath while it lives,
/// its flames and smoke, their dice, its place and the place before, its lamp, and its parts'
/// meshes, the flames', the smoke's and the glow's.
#[derive(Component)]
struct Look {
    breath: Option<Entity>,
    system: System,
    dice: Rng,
    place: Vec3,
    before: Option<Vec3>,
    lamp: Entity,
    parts: [Handle<Mesh>; 3],
}

/// The looks' systems, and their materials, made as the first breath leaves: the breath's flames
/// added, its smoke laid over, its glow and the wall's flames added whole; none where a texture is
/// missing.
#[derive(Resource)]
struct Looks {
    breath: particles::Fire,
    wall: particles::Fire,
    materials: Option<[Option<Handle<Effect>>; 4]>,
}

impl Default for Looks {
    fn default() -> Self {
        Self {
            breath: walloffire(),
            wall: walloffire_line(),
            materials: None,
        }
    }
}

/// Makes the looks' materials.
fn make_looks(
    data: Option<&Data>,
    assets: &AssetServer,
    materials: &mut Assets<Effect>,
) -> [Option<Handle<Effect>>; 4] {
    let breath = walloffire();
    let wall = walloffire_line();
    [
        (breath.texture, Some(AlphaMode::Add)),
        (breath.texture, Some(AlphaMode::Blend)),
        (particles::WALLOFFIRE_GLOW_TEXTURE, None),
        (wall.texture, None),
    ]
    .map(|(texture, mode)| {
        let image = data.and_then(|data| particle_texture(&data.0.assets, assets, texture));
        if image.is_none() {
            warn!("breath: no texture {texture}; its part is not drawn");
        }
        image.map(|image| {
            materials.add(match mode {
                Some(mode) => particle_material(image, mode),
                None => crate::fx::whole_material(image),
            })
        })
    })
}

/// A part of a look: its mesh under `parent` in `material`, untinted by the night.
fn part(commands: &mut Commands, parent: Entity, mesh: Handle<Mesh>, material: Handle<Effect>) {
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::IDENTITY,
        NoFrustumCulling,
        RenderLayers::layer(iso::UNTINTED),
        ChildOf(parent),
    ));
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

/// Puts each new breath at its start, and gives it its look: its flames and smoke, its glow and
/// its lamp (`0x7AE5A0`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn show(
    mut commands: Commands,
    mut looks: ResMut<Looks>,
    breaths: Query<(Entity, &Breath), Added<Breath>>,
    heights: Res<Heights>,
    (data, assets, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (entity, breath) in &breaths {
        let ground = heights
            .standing(breath.from.x, breath.from.y, breath.level)
            .unwrap_or(0.0);
        let place = Vec3::new(breath.from.x, ground + RISE, breath.from.y);
        commands.entity(entity).insert((
            Transform::from_translation(place),
            DespawnOnExit(AppState::InGame),
        ));
        let made = looks
            .materials
            .get_or_insert_with(|| make_looks(data.as_deref(), &assets, &mut materials))
            .clone();
        let look = commands
            .spawn((
                Transform::IDENTITY,
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let lamp = commands
            .spawn((
                Transform::from_translation(place.with_y(0.0)),
                daynight::Light(looks.breath.light),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let parts = [
            meshes.add(empty()),
            meshes.add(empty()),
            meshes.add(empty()),
        ];
        for (mesh, material) in parts.iter().zip(&made) {
            if let Some(material) = material {
                part(&mut commands, look, mesh.clone(), material.clone());
            }
        }
        let index = usize::try_from(look.index_u32()).unwrap_or(0);
        commands.entity(look).insert(Look {
            breath: Some(entity),
            system: System::new(looks.breath.count),
            dice: crate::seed::dice(SEED ^ index),
            place,
            before: None,
            lamp,
            parts,
        });
    }
}

/// Runs each breath's look a frame on (`0x7AE290`, `0x7AE4F0`): while its breath flies, its flames
/// and smoke left along the way it went, and its glow at it; once its breath stopped no more,
/// and once its breath is gone and its last flame has died it goes, its lamp with it. The whole
/// look lies at its breath's place in the order, where its object draws it (#2220).
fn glow(
    mut commands: Commands,
    time: Res<Time>,
    looks: Res<Looks>,
    mut shown: Query<(Entity, &mut Look)>,
    breaths: Query<(&Breath, &Transform)>,
    mut lamps: Query<&mut Transform, (With<daynight::Light>, Without<Breath>)>,
    (mut meshes, orders): (ResMut<Assets<Mesh>>, Orders),
) {
    let fire = &looks.breath;
    let dt = time.delta_secs().min(0.1);
    for (entity, mut look) in &mut shown {
        let look = &mut *look;
        let flying = match look.breath.map(|breath| breaths.get(breath)) {
            Some(Ok((breath, transform))) => {
                look.place = transform.translation;
                breath.laid.is_none()
            }
            Some(Err(_)) => {
                look.breath = None;
                false
            }
            None => false,
        };
        look.system.update(dt, &fire.layers);
        if flying {
            let before = look.before.unwrap_or(look.place);
            look.system.emit_along(
                &fire.blocks,
                &fire.layers,
                fire.count,
                (models(before), models(look.place)),
                dt,
                &mut look.dice,
            );
        }
        look.before = Some(look.place);
        if look.breath.is_none() && look.system.particles.is_empty() {
            commands.entity(look.lamp).despawn();
            commands.entity(entity).despawn();
            continue;
        }
        if let Ok(mut lamp) = lamps.get_mut(look.lamp) {
            lamp.translation = look.place.with_y(0.0);
        }
        // The particles lie in the world, in the models' frame: the look stands at its origin.
        let order = orders.at(look.place);
        for (layer, handle) in look.parts.iter().take(2).enumerate() {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                let mut quads = layer_quads(fire, &look.system, layer);
                quads.in_order(order, Vec3::ZERO);
                quads.into_mesh(&mut mesh);
            }
        }
        if let Some(mut mesh) = meshes.get_mut(&look.parts[2]) {
            let mut quads = Quads::default();
            quads.push(
                look.place,
                particles::WALLOFFIRE_GLOW_HALF * iso::MODEL_SCALE,
                0.0,
                (Vec2::ZERO, 1.0),
                [1.0; 4],
            );
            quads.in_order(order, Vec3::ZERO);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The quads of a wall's fire's flames about its foot, standing on their feet in its ramp, each
/// its frame of the 4 × 4 by its age (`0x7AEBF0`, the flags `0x99`).
pub(crate) fn wall_quads(fire: &particles::Fire, flames: &System) -> Quads {
    let grid = fire.grid.max(1);
    let side = 1.0 / f32::from(grid);
    let mut quads = Quads::default();
    for flame in &flames.particles {
        let frame = particles::age_frame(flame.life, grid);
        let corner = Vec2::new(f32::from(frame % grid), f32::from(frame / grid)) * side;
        let colour = fire
            .ramps
            .first()
            .map_or([1.0; 4], |ramp| ramp.at(flame.life));
        quads.push_footed(
            world(flame.place),
            flame.size * iso::MODEL_SCALE,
            0.0,
            (corner, side),
            colour,
        );
    }
    quads
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
    looks: Res<Looks>,
    mut meshes: ResMut<Assets<Mesh>>,
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
                    let mesh = meshes.add(empty());
                    // Its light at its place at the height 0 (`0x41B200`), its flames on the
                    // ground.
                    let fire = commands
                        .spawn((
                            Transform::from_xyz(at.x, 0.0, at.y),
                            Visibility::default(),
                            daynight::Light(looks.wall.light),
                            DespawnOnExit(AppState::InGame),
                        ))
                        .id();
                    let material = looks.materials.as_ref().and_then(|made| made[3].clone());
                    if let Some(material) = material {
                        let flames = commands
                            .spawn((
                                Mesh3d(mesh.clone()),
                                MeshMaterial3d(material),
                                Transform::from_xyz(0.0, ground, 0.0),
                                NoFrustumCulling,
                                RenderLayers::layer(iso::UNTINTED),
                            ))
                            .id();
                        commands.entity(fire).add_child(flames);
                    }
                    let index = usize::try_from(fire.index_u32()).unwrap_or(0);
                    commands.entity(fire).insert(WallFire {
                        fire: breath.fire,
                        at,
                        ground,
                        level: breath.level,
                        left: WALL_BURNS,
                        touched: Vec::new(),
                        flames: System::new(looks.wall.count),
                        dice: crate::seed::dice(SEED ^ 0x3EF ^ index),
                        mesh,
                    });
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

/// Burns what each fire of a wall meets while it burns, and runs its flames a frame on
/// (`0x7AEB80`): made while it burns, it goes with its last. Its flames lie at its place in the
/// order, where its object draws them (#2220).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn flare(
    mut commands: Commands,
    time: Res<Time>,
    looks: Res<Looks>,
    (mut meshes, orders): (ResMut<Assets<Mesh>>, Orders),
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
    let dt = time.delta_secs().min(0.1);
    for (entity, mut fire) in &mut fires {
        let fire = &mut *fire;
        fire.left -= time.delta_secs();
        let wall = &looks.wall;
        fire.flames.update(dt, &wall.layers);
        if fire.left > 0.0 {
            fire.flames
                .emit(&wall.blocks, &wall.layers, wall.count, &mut fire.dice);
        }
        if let Some(mut mesh) = meshes.get_mut(&fire.mesh) {
            // The flames' mesh stands on the fire's ground at its place.
            let foot = Vec3::new(fire.at.x, fire.ground, fire.at.y);
            let mut quads = wall_quads(wall, &fire.flames);
            quads.in_order(orders.at(foot), foot);
            quads.into_mesh(&mut mesh);
        }
        if fire.left <= 0.0 {
            if fire.flames.particles.is_empty() {
                commands.entity(entity).despawn();
            }
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
        app.init_resource::<Looks>().add_systems(
            Update,
            (show, breathe, glow, flare)
                .chain()
                .after(Step::Npcs)
                .in_set(Playing),
        );
    }
}
