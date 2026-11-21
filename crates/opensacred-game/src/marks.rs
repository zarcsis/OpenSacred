//! The marks laid on the ground (779, `cParticleSystem_temptex`; #1092; docs/re/effects.md, "The
//! Wood Elf's sights"): a square of a texture flat on the ground at a turn at random, laid over
//! by its alpha in the light's colour, which holds for its seconds and then fades over 4 s
//! (`opensacred_rules::particles::Mark`): the lost souls' dark star of streaks at each soul's
//! starting point, the blood decals of a burst's body parts (`body_parts`) and the mark of blood
//! where a creature's spit lands (`spits`). The night tints it as it does the ground under it. A
//! body's blood puddle (907, `opensacred_rules::particles::Puddle`, #2516) is laid so too,
//! unturned, growing and fading by its own clock, and a blow's blood splashes (903,
//! `opensacred_rules::particles::Splashes`, #2773), small blobs of the blood's colour that fall
//! about a place for a second and lie twelve.
//!
//! On the ground they lie in list 0 (`fx::Orders::in_list_0`, in the blended layer of list 0's
//! underlays), as the original's walk appends the FX objects of the flag `0x4` (#3285;
//! docs/re/drawing.md, "The models"): under the statics' shadows, the underlays of list 2 and
//! every sprite and model, whatever their rows; on a building's floor at their places in list 3's
//! order, as the walk appends them there.

use std::collections::HashMap;

use bevy::camera::visibility::NoFrustumCulling;
use bevy::prelude::*;
use opensacred_rules::particles::{Mark, Puddle, SPLASH_GRID, SPLASH_TEXTURE, Splashes};

use crate::blended::Layer;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, layered, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;
use crate::terrain::Level;

/// A mark to lay: its texture, what is laid ([`Kind`]) and the level it lies on.
#[derive(Component)]
struct Unmade {
    texture: &'static str,
    kind: Kind,
    on: Level,
}

/// What is laid: a mark of its seconds before it fades and its half side, a body's puddle, or a
/// blow's splashes, of the blood's colour.
#[derive(Clone, Copy, Debug)]
enum Kind {
    Mark { hold: f32, half: f32 },
    Puddle,
    Splashes([f32; 3]),
}

/// What shows: a mark and its half side, a puddle, or a blow's splashes.
#[derive(Clone, Debug)]
enum Look {
    Mark(Mark, f32),
    Puddle(Puddle),
    Splashes(Box<Splashes>),
}

impl Look {
    fn update(&mut self, dt: f32, dice: &mut Rng) {
        match self {
            Self::Mark(mark, _) => mark.update(dt),
            Self::Puddle(puddle) => puddle.update(dt),
            Self::Splashes(splashes) => splashes.update(dt.min(0.1), dice),
        }
    }

    fn done(&self) -> bool {
        match self {
            Self::Mark(mark, _) => mark.done(),
            Self::Puddle(puddle) => puddle.done(),
            Self::Splashes(splashes) => splashes.done(),
        }
    }

    /// Its half side, and its alpha; a blow's splashes none, each blob its own.
    fn square(&self) -> (f32, f32) {
        match self {
            Self::Mark(mark, half) => (*half, mark.alpha()),
            Self::Puddle(puddle) => (puddle.half(), puddle.alpha()),
            Self::Splashes(_) => (0.0, 0.0),
        }
    }
}

/// A mark that shows: its look, its turn and its mesh.
#[derive(Component)]
struct Shown {
    look: Look,
    turn: f32,
    mesh: Handle<Mesh>,
    /// The level it lies on: the ground in list 0, a building's floor in list 3's order.
    on: Level,
}

/// The marks' materials by their textures and whether they lie in list 0's layer, laid over by
/// their alpha; none where the texture is missing.
#[derive(Resource, Default)]
struct Looks(HashMap<(&'static str, bool), Option<Handle<Effect>>>);

/// The marks' dice (`rand()`).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x779))
    }
}

/// Lays the mark `(texture, hold, half)` at `place` on the level `on`.
pub(crate) fn lay(
    commands: &mut Commands,
    (place, on): (Vec3, Level),
    (texture, hold, half): (&'static str, f32, f32),
) {
    commands.spawn((
        Unmade {
            texture,
            kind: Kind::Mark { hold, half },
            on,
        },
        Transform::from_translation(place),
        DespawnOnExit(AppState::InGame),
    ));
}

/// Lays a body's blood puddle of `texture` on the ground at `place` (907, `0x76E150`).
pub(crate) fn puddle(commands: &mut Commands, place: Vec3, texture: &'static str) {
    commands.spawn((
        Unmade {
            texture,
            kind: Kind::Puddle,
            on: None,
        },
        Transform::from_translation(place),
        DespawnOnExit(AppState::InGame),
    ));
}

/// Lays a blow's blood splashes of the blood `colour` about `place` on the level `on` (903,
/// `0x7A5C30`).
pub(crate) fn splashes(commands: &mut Commands, (place, on): (Vec3, Level), colour: [f32; 3]) {
    commands.spawn((
        Unmade {
            texture: SPLASH_TEXTURE,
            kind: Kind::Splashes(colour),
            on,
        },
        Transform::from_translation(place),
        DespawnOnExit(AppState::InGame),
    ));
}

/// Gives each new mark its look, its turn at random (`rand()/32767 · 2π`), its mesh and its
/// material (`0x76DD40`), in list 0's layer on the ground; a puddle and a blow's splashes no turn
/// (`0x76E150`, `0x7A5C30`).
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    (data, assets, mut looks, mut dice): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Dice>,
    ),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, unmade) in &unmade {
        let listed = unmade.on.is_none();
        let material = looks
            .0
            .entry((unmade.texture, listed))
            .or_insert_with(|| {
                let image = data
                    .as_ref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, unmade.texture));
                if image.is_none() {
                    warn!(
                        "marks: no texture {}; its mark is not drawn",
                        unmade.texture
                    );
                }
                image.map(|image| {
                    let material = particle_material(image, AlphaMode::Blend);
                    effects.add(if listed {
                        layered(material, Layer::Underlays)
                    } else {
                        material
                    })
                })
            })
            .clone();
        let (look, turn) = match unmade.kind {
            Kind::Mark { hold, half } => {
                let roll = f32::from(u16::try_from(dice.0.below(32_768)).unwrap_or(0)) / 32_767.0;
                (
                    Look::Mark(Mark::new(hold), half),
                    roll * std::f32::consts::TAU,
                )
            }
            Kind::Puddle => (Look::Puddle(Puddle::default()), 0.0),
            Kind::Splashes(colour) => (Look::Splashes(Box::new(Splashes::new(colour))), 0.0),
        };
        let mesh = meshes.add(empty());
        let mut made = commands.entity(entity);
        made.remove::<Unmade>().insert((
            Shown {
                look,
                turn,
                mesh: mesh.clone(),
                on: unmade.on,
            },
            Visibility::default(),
        ));
        if let Some(material) = material {
            made.with_child((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
            ));
        }
    }
}

/// Runs each mark a frame on (`0x76DAC0`) and draws it (`0x763CB0`): a square of its half side
/// flat on the ground, turned about the upright by its turn, its texture's `u` and `v` running
/// from 1 at its corner of `(−s, −s)` to 0 at `(s, s)`, white at its alpha; a puddle (`0x76DE50`,
/// `0x76DF00`, the flat quad `0x7631E0`) unturned, its `u` and `v` running from 0 at `(−s, −s)`
/// to 1 at `(s, s)`; a blow's splashes (`0x7A5970`, `0x7A59E0`) each blob a square of its half
/// side flat on the ground about its place, turned by its angle, of its frame of the texture's
/// 2 × 2, in its ramp's colour at its life; each goes once faded. Each lies in the original's
/// order ([`in_order`]).
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut marks: Query<(Entity, &mut Shown, &GlobalTransform)>,
    (mut meshes, orders, mut dice): (ResMut<Assets<Mesh>>, Orders, ResMut<Dice>),
) {
    let dt = time.delta_secs();
    let s = iso::MODEL_SCALE;
    let (across, depth) = iso::fx_axes();
    for (entity, mut mark, place) in &mut marks {
        mark.look.update(dt, &mut dice.0);
        if mark.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        if let Look::Splashes(splashes) = &mark.look {
            let mut quads = Quads::default();
            let side = 1.0 / f32::from(SPLASH_GRID);
            for blob in &splashes.system.particles {
                let (sin, cos) = blob.angle.sin_cos();
                let [px, py, _] = blob.place;
                let corner = |x: f32, y: f32| {
                    let (x, y) = (x * cos + y * sin, -x * sin + y * cos);
                    (across * (px + x * blob.size) + depth * (py + y * blob.size)) * s
                };
                let cell = Vec2::new(
                    f32::from(blob.frame % SPLASH_GRID),
                    f32::from(blob.frame / SPLASH_GRID % SPLASH_GRID),
                ) * side;
                quads.push_corners(
                    [
                        corner(-1.0, -1.0),
                        corner(1.0, -1.0),
                        corner(1.0, 1.0),
                        corner(-1.0, 1.0),
                    ],
                    [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y].map(|uv| cell + uv * side),
                    splashes.ramp.at(blob.life),
                );
            }
            if let Some(mut mesh) = meshes.get_mut(&mark.mesh) {
                in_order(&mut quads, &orders, place.translation(), mark.on);
                quads.into_mesh(&mut mesh);
            }
            continue;
        }
        let (sin, cos) = mark.turn.sin_cos();
        let (half, alpha) = mark.look.square();
        let corner = |x: f32, y: f32| {
            let (x, y) = (x * cos + y * sin, -x * sin + y * cos);
            (across * x + depth * y) * s
        };
        let uvs = match mark.look {
            Look::Mark(..) => [Vec2::ONE, Vec2::Y, Vec2::ZERO, Vec2::X],
            Look::Puddle(_) | Look::Splashes(_) => [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
        };
        let mut quads = Quads::default();
        quads.push_corners(
            [
                corner(-half, -half),
                corner(half, -half),
                corner(half, half),
                corner(-half, half),
            ],
            uvs,
            [1.0, 1.0, 1.0, alpha],
        );
        if let Some(mut mesh) = meshes.get_mut(&mark.mesh) {
            in_order(&mut quads, &orders, place.translation(), mark.on);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// Lays a mark's `quads`, whose mesh's origin is `origin`, in the original's order (#3285): on
/// the ground in list 0, all at one depth behind its cell's place ([`Orders::in_list_0`]), so
/// that every sprite and model and the shadows of list 1 lie over it, a body over its puddle
/// whatever cells the two take; on a building's floor `on` at its place in list 3's order, no
/// nearer than it ([`Orders::at_level`]), as the walk appends it there while its floor byte is
/// not 0 (`0x62C4E3`, `0x62C914`).
fn in_order(quads: &mut Quads, orders: &Orders, origin: Vec3, on: Level) {
    match on {
        None => quads.at_order(orders.in_list_0(origin), origin),
        Some(_) => quads.in_order(orders.at_level(origin, on), origin),
    }
}

/// The marks on the ground.
pub struct MarksPlugin;

impl Plugin for MarksPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>()
            .init_resource::<Dice>()
            .add_systems(
                Update,
                (make, run)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
