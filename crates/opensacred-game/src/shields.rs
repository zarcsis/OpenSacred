//! The shields and the regenerations about their bearers (#1092; docs/re/effects.md, "The holy
//! shield, the shield wall, the mana's spring and the regeneration"): the Seraphim's holy shield
//! (22, 819), a pale blue cylinder of light lit by a band that sweeps round its foot over an arc
//! on the ground; the Mage's shield wall (30, 834), two red-tinted walls built column by column
//! about him, a glow at each column's foot; and his mana's spring (31, 835) and regeneration (32,
//! 836), a symbol turning flat at the bearer's feet over a stack of soft glows. Each stands at its
//! bearer's root while its spell lasts, untinted by the night; the two shields are lit by a light
//! of 300 and flash at each arrow they turn aside (`opensacred_rules::particles::{HolyShield,
//! ShieldWall, Regen}`).
//!
//! Not followed: a spell that the shield wall turns aside, which no part of the game turns aside
//! yet, and the mana's spring of the set 32's seventh piece.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    HOLY_SHIELD_GROUND, HOLY_SHIELD_HEIGHT, HOLY_SHIELD_TEXTURES, HOLY_SHIELD_U, HolyShield,
    REGEN_GLOW_TEXTURE, REGEN_GLOWS, REGEN_SYMBOL, Regen, SHIELD_COLUMNS, SHIELD_LIGHT,
    SHIELD_RADIUS, SHIELD_WALL_COLOUR, SHIELD_WALL_GLOW, SHIELD_WALL_GLOW_CORNERS,
    SHIELD_WALL_HEIGHT, SHIELD_WALL_TEXTURES, SHIELD_WALL_TOP_V, ShieldWall,
};

use crate::frame::{Playing, Unpaused};

/// The spells whose effects of turning arrows aside light their shields.
const HOLY_SHIELD: u32 = 22;
const SHIELD_WALL: u32 = 30;
use crate::fx::{
    Effect, Quads, empty, particle_material, particle_texture, repeated_texture, whole_material,
};
use crate::game::{AppState, Data};
use crate::{daynight, iso};

/// Which sight a spell shows about its bearer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// The holy shield (819).
    Holy,
    /// The shield wall (834).
    Wall,
    /// The mana's spring (835) or the regeneration (836).
    Regen(Regen),
}

/// An arrow turned aside from a bearer by the effect keyed by a spell, which lights the bearer's
/// sight of that spell, the holy shield's (22) or the shield wall's (30) (`0x5B6620(8)`).
#[derive(Message)]
pub(crate) struct Flash(pub Entity, pub u32);

/// A sight to make: its kind, its bearer and its seconds.
#[derive(Component)]
struct Unmade {
    kind: Kind,
    bearer: Entity,
    seconds: f32,
}

/// What a sight shows and how far it has run.
enum Look {
    Holy(HolyShield),
    Wall(ShieldWall),
    Regen(Regen, f32, f32),
}

impl Look {
    /// A run of `dt` seconds.
    fn update(&mut self, dt: f32) {
        match self {
            Self::Holy(shield) => shield.update(dt),
            Self::Wall(wall) => wall.update(dt),
            Self::Regen(_, clock, _) => *clock += dt,
        }
    }

    /// Whether it is over.
    fn done(&self) -> bool {
        match self {
            Self::Holy(shield) => shield.done(),
            Self::Wall(wall) => wall.done(),
            Self::Regen(_, clock, seconds) => clock > seconds,
        }
    }
}

/// A sight that shows: its bearer, its look, and its parts' meshes.
#[derive(Component)]
struct Shown {
    bearer: Entity,
    look: Look,
    parts: Vec<Handle<Mesh>>,
}

/// The sights' materials, made as the first asks, by kind: the holy shield's cylinder and ground
/// added whole; the shield wall's two walls added whole and its glows added by their alpha; the
/// regenerations' symbols and their glows added by their alpha. None where a texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<Materials>);

/// [`Looks`]' materials.
struct Materials {
    holy: [Option<Handle<Effect>>; 2],
    wall: [Option<Handle<Effect>>; 3],
    mana: [Option<Handle<Effect>>; 2],
    health: [Option<Handle<Effect>>; 2],
}

/// Shows the sight of `kind` about `bearer` for `seconds`, its spell's value 0 at its level.
pub(crate) fn show(commands: &mut Commands, kind: Kind, bearer: Entity, seconds: f32) {
    commands.spawn((
        Unmade {
            kind,
            bearer,
            seconds,
        },
        DespawnOnExit(AppState::InGame),
    ));
}

/// Gives each new sight its look at its bearer's place, its meshes and its materials, and the
/// shields their light; a bearer's new sight of a kind takes the place of the one it had.
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    shown: Query<(Entity, &Shown)>,
    places: Query<&GlobalTransform>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, unmade) in &unmade {
        let Ok(at) = places.get(unmade.bearer) else {
            commands.entity(entity).despawn();
            continue;
        };
        for (old, sight) in &shown {
            if sight.bearer == unmade.bearer && same_kind(&sight.look, unmade.kind) {
                commands.entity(old).despawn();
            }
        }
        let materials = looks.0.get_or_insert_with(|| {
            let mut material = |name: &str, whole: bool, repeated: bool| {
                let image = data.as_ref().and_then(|data| {
                    if repeated {
                        repeated_texture(&data.0.assets, &assets, name)
                    } else {
                        particle_texture(&data.0.assets, &assets, name)
                    }
                });
                if image.is_none() {
                    warn!("shields: no texture {name}; its part is not drawn");
                }
                image.map(|image| {
                    effects.add(if whole {
                        whole_material(image)
                    } else {
                        particle_material(image, AlphaMode::Add)
                    })
                })
            };
            let [shield, ground] = HOLY_SHIELD_TEXTURES;
            let [first, second, glow] = SHIELD_WALL_TEXTURES;
            let regen_glow = material(REGEN_GLOW_TEXTURE, false, false);
            Materials {
                holy: [material(shield, true, true), material(ground, true, false)],
                wall: [
                    material(first, true, true),
                    material(second, true, true),
                    material(glow, false, false),
                ],
                mana: [
                    material(Regen::Mana.texture(), false, false),
                    regen_glow.clone(),
                ],
                health: [material(Regen::Health.texture(), false, false), regen_glow],
            }
        });
        let (look, chosen): (Look, Vec<Option<Handle<Effect>>>) = match unmade.kind {
            Kind::Holy => (
                Look::Holy(HolyShield::new(unmade.seconds)),
                materials.holy.to_vec(),
            ),
            Kind::Wall => (
                Look::Wall(ShieldWall::new(unmade.seconds)),
                materials.wall.to_vec(),
            ),
            Kind::Regen(regen) => (
                Look::Regen(regen, 0.0, unmade.seconds),
                match regen {
                    Regen::Mana => materials.mana.to_vec(),
                    Regen::Health => materials.health.to_vec(),
                },
            ),
        };
        let parts: Vec<Handle<Mesh>> = chosen.iter().map(|_| meshes.add(empty())).collect();
        let mut made = commands.entity(entity);
        made.remove::<Unmade>().insert((
            Shown {
                bearer: unmade.bearer,
                look,
                parts: parts.clone(),
            },
            Transform::from_translation(at.translation()),
            Visibility::default(),
        ));
        if matches!(unmade.kind, Kind::Holy | Kind::Wall) {
            made.insert(daynight::Light(SHIELD_LIGHT));
        }
        for (mesh, material) in parts.into_iter().zip(chosen) {
            if let Some(material) = material {
                made.with_child((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                ));
            }
        }
    }
}

/// Whether a sight's look is of `kind`: the two regenerations one kind each.
fn same_kind(look: &Look, kind: Kind) -> bool {
    match (look, kind) {
        (Look::Holy(_), Kind::Holy) | (Look::Wall(_), Kind::Wall) => true,
        (Look::Regen(a, ..), Kind::Regen(b)) => *a == b,
        _ => false,
    }
}

/// Lights the shields of the bearers whose arrows they turned aside.
fn flash(mut flashes: MessageReader<Flash>, mut shown: Query<&mut Shown>) {
    for &Flash(bearer, art) in flashes.read() {
        for mut sight in &mut shown {
            if sight.bearer != bearer {
                continue;
            }
            match (&mut sight.look, art) {
                (Look::Holy(shield), HOLY_SHIELD) => shield.flash(),
                (Look::Wall(wall), SHIELD_WALL) => wall.flash(),
                _ => {}
            }
        }
    }
}

/// Runs each sight a frame on (`0x785970`, `0x78AAF0`, `0x78B200` and their draws): at its
/// bearer's place, which is its root's; it goes once it is over, or with its bearer.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &mut Transform)>,
    places: Query<&GlobalTransform>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs();
    let s = iso::MODEL_SCALE;
    let (across, depth) = iso::fx_axes();
    let frame = |[x, y, z]: [f32; 3]| (across * x + depth * y + Vec3::Y * z) * s;
    for (entity, mut sight, mut transform) in &mut shown {
        let sight = &mut *sight;
        sight.look.update(dt);
        let place = places.get(sight.bearer).map(GlobalTransform::translation);
        let Ok(place) = place else {
            commands.entity(entity).despawn();
            continue;
        };
        if sight.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation = place;
        let quads = match &sight.look {
            Look::Holy(shield) => holy(shield, &frame),
            Look::Wall(wall) => walls(wall, &frame),
            Look::Regen(regen, clock, _) => regenerates(*regen, *clock, &frame),
        };
        for (handle, part) in sight.parts.iter().zip(quads) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The point of the column pair `pair` (0 to 30) at the height `z`, in the models' frame: the
/// pair 0 in front of the bearer, toward the viewer, the pairs on round by 12° each.
fn column(pair: u8, z: f32) -> [f32; 3] {
    let angle = std::f32::consts::TAU * f32::from(pair) / f32::from(SHIELD_COLUMNS);
    [SHIELD_RADIUS * angle.sin(), SHIELD_RADIUS * angle.cos(), z]
}

/// A flat quad on the bearer's ground at the height `z` of the half side `half`, turned by
/// `angle` (`0x7631E0`: `(x, y)` to `(x·cos θ + y·sin θ, −x·sin θ + y·cos θ)`), the whole
/// texture, its corners' colours from (−, −) round by (+, −), (+, +) to (−, +).
fn flat(
    quads: &mut Quads,
    frame: &impl Fn([f32; 3]) -> Vec3,
    (z, half, angle): (f32, f32, f32),
    colours: [[f32; 4]; 4],
) {
    let (sin, cos) = angle.sin_cos();
    let corner = |x: f32, y: f32| frame([x * cos + y * sin, -x * sin + y * cos, z]);
    quads.push_corners_shaded(
        [
            corner(-half, -half),
            corner(half, -half),
            corner(half, half),
            corner(-half, half),
        ],
        [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
        colours,
    );
}

/// The holy shield's parts (`0x785A20`): its cylinder of 30 columns, each lit at its foot by the
/// band and at its top by the flash, its texture wrapped three times about it; and its ground's
/// arc turning with the band, white.
fn holy(shield: &HolyShield, frame: &impl Fn([f32; 3]) -> Vec3) -> Vec<Quads> {
    let mut cylinder = Quads::default();
    let top = shield.top();
    let grey = |g: f32| [g, g, g, 1.0];
    for pair in 0..SHIELD_COLUMNS {
        let next = pair + 1;
        let (u0, u1) = (
            HOLY_SHIELD_U * f32::from(pair),
            HOLY_SHIELD_U * f32::from(next),
        );
        cylinder.push_corners_shaded(
            [
                frame(column(pair, 0.0)),
                frame(column(next, 0.0)),
                frame(column(next, HOLY_SHIELD_HEIGHT)),
                frame(column(pair, HOLY_SHIELD_HEIGHT)),
            ],
            [
                Vec2::new(u0, 1.0),
                Vec2::new(u1, 1.0),
                Vec2::new(u1, 0.0),
                Vec2::new(u0, 0.0),
            ],
            [
                grey(shield.foot(pair)),
                grey(shield.foot(next)),
                grey(top),
                grey(top),
            ],
        );
    }
    let mut ground = Quads::default();
    flat(
        &mut ground,
        frame,
        (0.0, HOLY_SHIELD_GROUND, shield.ground_angle()),
        [[1.0; 4]; 4],
    );
    vec![cylinder, ground]
}

/// The shield wall's parts (`0x78AC00`): its two walls over its built columns, of their heights,
/// in its red; and at each built column's foot a glow lying flat, of its breathing size, in its
/// corners' reds.
fn walls(wall: &ShieldWall, frame: &impl Fn([f32; 3]) -> Vec3) -> Vec<Quads> {
    let built = wall.columns();
    let wall_of = |height: f32| {
        let mut quads = Quads::default();
        let top = SHIELD_WALL_HEIGHT * height;
        for pair in 0..built {
            let next = pair + 1;
            let (u0, u1) = (
                HOLY_SHIELD_U * f32::from(pair),
                HOLY_SHIELD_U * f32::from(next),
            );
            quads.push_corners(
                [
                    frame(column(pair, 0.0)),
                    frame(column(next, 0.0)),
                    frame(column(next, top)),
                    frame(column(pair, top)),
                ],
                [
                    Vec2::new(u0, 1.0),
                    Vec2::new(u1, 1.0),
                    Vec2::new(u1, SHIELD_WALL_TOP_V),
                    Vec2::new(u0, SHIELD_WALL_TOP_V),
                ],
                SHIELD_WALL_COLOUR,
            );
        }
        quads
    };
    let [first, second] = wall.heights();
    let mut glows = Quads::default();
    let scale = wall.glow_scale();
    let half = SHIELD_WALL_GLOW * scale;
    let colour = |x: f32, y: f32| {
        SHIELD_WALL_GLOW_CORNERS
            .iter()
            .find(|(at, ..)| *at == [x, y])
            .map_or([1.0; 4], |&(_, _, colour)| colour)
    };
    for pair in 0..built {
        // The square scaled about its own middle, which stands at the column's foot.
        let [cx, cy, _] = column(pair, 0.0);
        let corner = |dx: f32, dy: f32| frame([cx + dx * half, cy + dy * half, 0.0]);
        glows.push_corners_shaded(
            [
                corner(-1.0, -1.0),
                corner(1.0, -1.0),
                corner(1.0, 1.0),
                corner(-1.0, 1.0),
            ],
            [Vec2::Y, Vec2::ONE, Vec2::X, Vec2::ZERO],
            [
                colour(-1.0, -1.0),
                colour(1.0, -1.0),
                colour(1.0, 1.0),
                colour(-1.0, 1.0),
            ],
        );
    }
    vec![wall_of(first), wall_of(second), glows]
}

/// A regeneration's parts (`0x78B230`, `0x78B680`): its symbol turning flat at the bearer's
/// root, white, and its five glows stacked over it, unturned, in its colour.
fn regenerates(regen: Regen, clock: f32, frame: &impl Fn([f32; 3]) -> Vec3) -> Vec<Quads> {
    let mut symbol = Quads::default();
    flat(
        &mut symbol,
        frame,
        (0.0, REGEN_SYMBOL, regen.angle(clock)),
        [[1.0; 4]; 4],
    );
    let mut glows = Quads::default();
    for [half, height] in REGEN_GLOWS {
        flat(
            &mut glows,
            frame,
            (height, half, 0.0),
            [regen.glow_colour(); 4],
        );
    }
    vec![symbol, glows]
}

/// The shields and the regenerations.
pub struct ShieldsPlugin;

impl Plugin for ShieldsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>()
            .add_message::<Flash>()
            .add_systems(
                Update,
                (make, flash, run)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
