//! The scripts' Pong (#2008; docs/re/effects.md, "The scripts' Pong"): the court of two bats and
//! a ball that every tree's `Sector84080Enter` makes (`npcs`), the FX types 1156 to 1158. The
//! hero's bat follows the hero across the court while he stands in its columns, the other bat
//! follows the ball, and the ball flies off the court's walls and the bats
//! (`opensacred_rules::pong`); each ball the hero's bat returns sounds `PONG` at it, brightens it
//! for a second and gives the hero experience by the ball's speed.
//!
//! The bats are lilac boxes of lines over their faces of `Fx_dwarf_shield` (`0x7BCC60`), the ball
//! a blue glow on the ground under a white one, with two shrinking rings of `Fx_twirl_rays` while
//! it waits (`0x7BD630`); all untinted by the night, each piece at its place in the original's
//! order of drawing (`fx::Orders::at`, #2220).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::pong::{self, Ball, Bat};
use opensacred_rules::spawning::Rng;

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, particle_texture, plain_material, whole_material};
use crate::game::{AppState, Data};
use crate::terrain::Heights;
use crate::{iso, paths};

/// The FX types of the court's pieces (`TYPE_FX_PONG_BATTER`, `_BALL`, `_BATTER_AI`).
const HERO_BAT: u32 = 1156;
const BALL: u32 = 1157;
const OTHER_BAT: u32 = 1158;

/// A piece of the court, of its FX type, which `npcs` makes for a script's `CreateObj`.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Piece(pub u32);

impl Piece {
    /// The piece of the FX type `kind`, if it is one of the court's.
    pub(crate) fn of(kind: u32) -> Option<Self> {
        (HERO_BAT..=OTHER_BAT).contains(&kind).then_some(Self(kind))
    }
}

/// A bat's state: its side, its brightening, the hero it follows (`+0x68`), its clock and its
/// meshes, the faces' and the edges'.
#[derive(Component)]
struct Batting {
    bat: Bat,
    bright: f32,
    holder: Option<Entity>,
    clock: f32,
    faces: Handle<Mesh>,
    edges: Handle<Mesh>,
}

/// The ball's state, its dice, its clock and its meshes: its glow on the ground, its rings while
/// it waits and its glow over it.
#[derive(Component)]
struct Flying {
    ball: Ball,
    dice: Rng,
    clock: f32,
    ground: Handle<Mesh>,
    rings: Handle<Mesh>,
    glow: Handle<Mesh>,
}

/// The court's materials, made as the first piece shows; none where a texture is missing.
#[derive(Resource, Default)]
struct Materials {
    made: bool,
    shield: Option<Handle<Effect>>,
    lines: Option<Handle<Effect>>,
    ground: Option<Handle<Effect>>,
    rings: Option<Handle<Effect>>,
    glow: Option<Handle<Effect>>,
}

/// The bat's box (`0x7BBD90`): 140 by 60 by 30 in the models' units, its length along the cells'
/// columns, across the court.
const BOX_HALF: [f32; 2] = [70.0, 30.0];
const BOX_HEIGHT: f32 = 30.0;
/// The lines' lilac (`0xFFA0A0FF`) and their half width in world units.
const LILAC: [f32; 4] = [160.0 / 255.0, 160.0 / 255.0, 1.0, 1.0];
const LINE_HALF: f32 = 0.7;
/// The ball's ground glow's blue (`0xFF4040FF`) and its glow's height over it.
const BLUE: [f32; 4] = [64.0 / 255.0, 64.0 / 255.0, 1.0, 1.0];
const GLOW_UP: f32 = 15.0;
/// How far over the ground the flat quads lie, in world units.
const GROUND_LIFT: f32 = 0.5;
/// The hero's bat takes a hero within 16 cells of it (`0x5ABD80`).
const REACH_CELLS: i32 = 16;
/// The seed of the ball's `rand()` (**estimate**: any).
const SEED: usize = 0x485;

/// Makes each new piece's look and state.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn make(
    mut commands: Commands,
    pieces: Query<(Entity, &Piece, &Transform), Added<Piece>>,
    (data, assets, mut materials, mut effects, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Materials>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (entity, &Piece(kind), transform) in &pieces {
        if !materials.made {
            materials.made = true;
            let mut whole = |name: &str| {
                let image = data
                    .as_ref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                if image.is_none() {
                    warn!("pong: no texture {name}");
                }
                image.map(|image| effects.add(whole_material(image)))
            };
            materials.shield = whole("Fx_dwarf_shield");
            materials.ground = whole("PARTICLE_GLOW03");
            materials.rings = whole("Fx_twirl_rays");
            materials.glow = whole("PARTICLE_GLOW06");
            materials.lines = Some(effects.add(plain_material()));
        }
        let mut part = |mesh: &Handle<Mesh>, material: &Option<Handle<Effect>>| {
            if let Some(material) = material {
                commands.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                    ChildOf(entity),
                ));
            }
        };
        if kind == BALL {
            let (ground, rings, glow) = (
                meshes.add(empty()),
                meshes.add(empty()),
                meshes.add(empty()),
            );
            part(&ground, &materials.ground);
            part(&rings, &materials.rings);
            part(&glow, &materials.glow);
            // Its heading the frame's y turned by up to a half a radian either way (the FX event
            // `0xA3`'s case, `0x5A5863`).
            let index = usize::try_from(entity.index_u32()).unwrap_or(0);
            let mut dice = crate::seed::dice(SEED ^ index);
            let roll = f32::from(u16::try_from(dice.below(0x8000)).unwrap_or(0)) / 32_767.0;
            let (sin, cos) = (roll - 0.5).sin_cos();
            let heading = world([-sin, cos, 0.0]).xz().normalize_or(Vec2::Y);
            let at = transform.translation.xz();
            commands.entity(entity).insert(Flying {
                ball: Ball::new(at.to_array(), heading.to_array()),
                dice,
                clock: 0.0,
                ground,
                rings,
                glow,
            });
        } else {
            let (faces, edges) = (meshes.add(empty()), meshes.add(empty()));
            part(&faces, &materials.shield);
            part(&edges, &materials.lines);
            commands.entity(entity).insert(Batting {
                bat: if kind == HERO_BAT {
                    Bat::Hero
                } else {
                    Bat::Other
                },
                bright: 0.0,
                holder: None,
                clock: 0.0,
                faces,
                edges,
            });
        }
    }
}

/// A frame of the game (`0x5ABD80`, `0x5A9152`, `0x7BD560`, `0x7BCBC0`): the bats follow, the
/// ball flies and comes back off them, and the hero's returns give him their experience.
fn play(
    time: Res<Time>,
    heights: Res<Heights>,
    heroes: Query<(Entity, &GlobalTransform), With<crate::hero::Body>>,
    mut bats: Query<(Entity, &mut Transform, &mut Batting), Without<Flying>>,
    mut balls: Query<(&mut Transform, &mut Flying), Without<Batting>>,
    (mut sounds, mut experience, rules): (
        MessageWriter<crate::sfx::Play>,
        ResMut<crate::fight::Experience>,
        Res<crate::fight::Rules>,
    ),
) {
    let dt = time.delta_secs().min(0.1);
    let hero = heroes.iter().next();
    let Ok((mut ball_at, mut flying)) = balls.single_mut() else {
        return;
    };
    let flying = &mut *flying;
    flying.clock += dt;
    flying.ball.tick(dt);
    let heading = Vec2::from(flying.ball.heading);
    let scale = crate::bolts::flight_scale(heading);
    let stops = |cell: paths::Cell| !heights.walkable_at(cell, None);
    let cell_of = |[x, z]: [f32; 2]| paths::cell_of(x, z);
    flying
        .ball
        .fly(dt, scale, (stops, cell_of), &mut flying.dice);
    let [x, z] = flying.ball.place;
    ball_at.translation.x = x;
    ball_at.translation.z = z;
    for (entity, mut at, mut batting) in &mut bats {
        batting.clock += dt;
        batting.bright = (batting.bright - dt).max(0.0);
        // The hero's bat follows the hero, the other the ball, within the court's columns.
        let followed = match batting.bat {
            Bat::Hero => {
                let here = paths::cell_of(at.translation.x, at.translation.z);
                let near = hero.filter(|(_, hero)| {
                    let (cx, cy) = paths::cell_of(hero.translation().x, hero.translation().z);
                    (cx - here.0).abs() <= REACH_CELLS && (cy - here.1).abs() <= REACH_CELLS
                });
                batting.holder = near.map(|(hero, _)| hero);
                near.map(|(_, hero)| hero.translation().xz())
            }
            Bat::Other => Some(Vec2::new(x, z)),
        };
        if let Some(followed) = followed
            && pong::follows(paths::cell_of(followed.x, followed.y).0)
        {
            at.translation.x = followed.x;
        }
        let bat_at = at.translation.xz();
        let across = (x - bat_at.x).abs();
        if (z - bat_at.y).abs() < pong::REACH_ALONG && flying.ball.flying {
            debug!(
                "pong: the ball by the {:?} bat, {across:.0} across, heading {:?}, holder {:?}",
                batting.bat, flying.ball.heading, batting.holder
            );
        }
        let Some(earned) = flying.ball.returns(batting.bat, bat_at.to_array()) else {
            continue;
        };
        batting.bright = pong::BRIGHTENING;
        sounds.write(crate::sfx::Play {
            sound: crate::sfx::Of::Id(pong::SOUND),
            at: Some(entity),
            rate: (pong::SOUND_RATE, crate::sfx::Roll::None),
        });
        if batting.bat == Bat::Hero && batting.holder.is_some() {
            let grew = experience.add(u64::from(earned), &rules);
            info!(
                "pong: the hero returns the ball and earns {earned} experience, {} in all",
                experience.points
            );
            if grew {
                info!("pong: the hero reaches level {}", experience.level);
            }
        }
    }
}

/// The corners of a flat square of the half side `half` (the models' units) about the origin,
/// turned by `angle` in the frame, `lift` over the ground.
fn flat(half: f32, angle: f32, lift: f32) -> [Vec3; 4] {
    let (sin, cos) = angle.sin_cos();
    let corner =
        |x: f32, y: f32| world([x * cos - y * sin, x * sin + y * cos, 0.0]) * half + Vec3::Y * lift;
    [
        corner(-1.0, -1.0),
        corner(1.0, -1.0),
        corner(1.0, 1.0),
        corner(-1.0, 1.0),
    ]
}

/// Draws the pieces a frame on, each at its place in the order (#2220).
fn draw(
    bats: Query<(&Batting, &Transform)>,
    balls: Query<(&Flying, &Transform)>,
    (mut meshes, orders): (ResMut<Assets<Mesh>>, Orders),
) {
    let s = iso::MODEL_SCALE;
    let uvs = [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y];
    for (batting, at) in &bats {
        let order = orders.at(at.translation);
        // Its box, its length along the cells' columns: the world's X.
        let [hx, hz] = BOX_HALF.map(|h| h * s);
        let corner = |x: f32, y: f32, z: f32| Vec3::new(x * hx, y * BOX_HEIGHT * s, z * hz);
        let grey = ((batting.clock.sin() + batting.bright) * 20.0 + 100.0) / 255.0;
        let colour = [grey, grey, grey, 1.0];
        let mut faces = Quads::default();
        for face in [
            [
                (-1.0, 0.0, -1.0),
                (1.0, 0.0, -1.0),
                (1.0, 1.0, -1.0),
                (-1.0, 1.0, -1.0),
            ],
            [
                (-1.0, 0.0, 1.0),
                (1.0, 0.0, 1.0),
                (1.0, 1.0, 1.0),
                (-1.0, 1.0, 1.0),
            ],
            [
                (-1.0, 0.0, -1.0),
                (-1.0, 0.0, 1.0),
                (-1.0, 1.0, 1.0),
                (-1.0, 1.0, -1.0),
            ],
            [
                (1.0, 0.0, -1.0),
                (1.0, 0.0, 1.0),
                (1.0, 1.0, 1.0),
                (1.0, 1.0, -1.0),
            ],
            [
                (-1.0, 1.0, -1.0),
                (1.0, 1.0, -1.0),
                (1.0, 1.0, 1.0),
                (-1.0, 1.0, 1.0),
            ],
            [
                (-1.0, 0.0, -1.0),
                (1.0, 0.0, -1.0),
                (1.0, 0.0, 1.0),
                (-1.0, 0.0, 1.0),
            ],
        ] {
            faces.push_corners(face.map(|(x, y, z)| corner(x, y, z)), uvs, colour);
        }
        // Its twelve edges, thin quads facing the camera.
        let mut edges = Quads::default();
        let points = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
        for k in 0..4 {
            let (x0, z0) = points[k];
            let (x1, z1) = points[(k + 1) % 4];
            for y in [0.0, 1.0] {
                line(&mut edges, corner(x0, y, z0), corner(x1, y, z1));
            }
            line(&mut edges, corner(x0, 0.0, z0), corner(x0, 1.0, z0));
        }
        if let Some(mut mesh) = meshes.get_mut(&batting.faces) {
            faces.in_order(order, at.translation);
            faces.into_mesh(&mut mesh);
        }
        if let Some(mut mesh) = meshes.get_mut(&batting.edges) {
            edges.in_order(order, at.translation);
            edges.into_mesh(&mut mesh);
        }
    }
    for (flying, at) in &balls {
        let t = flying.clock;
        let wait = flying.ball.wait;
        let r = (1.0 - wait) * (7.0 * (4.0 * t).cos() + 35.0);
        let mut ground = Quads::default();
        ground.push_corners(flat(2.0 * r, 0.0, GROUND_LIFT), uvs, BLUE);
        let mut rings = Quads::default();
        if !flying.ball.flying {
            for (size, turn) in [(100.0, -1.7), (90.0, -1.3)] {
                rings.push_corners(flat(size * wait * wait, turn * t, GROUND_LIFT), uvs, LILAC);
            }
        }
        let mut glow = Quads::default();
        glow.push(
            Vec3::Y * GLOW_UP * s,
            r * s,
            0.0,
            (Vec2::ZERO, 1.0),
            [1.0; 4],
        );
        let order = orders.at(at.translation);
        for (mesh, mut quads) in [
            (&flying.ground, ground),
            (&flying.rings, rings),
            (&flying.glow, glow),
        ] {
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                quads.in_order(order, at.translation);
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// A thin quad of the lilac from `a` to `b` facing the camera.
fn line(quads: &mut Quads, a: Vec3, b: Vec3) {
    let side = (b - a).cross(iso::forward()).normalize_or_zero() * LINE_HALF;
    quads.push_corners(
        [a - side, a + side, b + side, b - side],
        [Vec2::ZERO; 4],
        LILAC,
    );
}

/// The scripts' Pong.
pub struct PongPlugin;

impl Plugin for PongPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Materials>().add_systems(
            Update,
            (make, play.in_set(Unpaused), draw)
                .chain()
                .in_set(Playing)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
