//! The marks laid on the ground (779, `cParticleSystem_temptex`; #1092; docs/re/effects.md, "The
//! Wood Elf's sights"): a square of a texture flat on the ground at a turn at random, laid over
//! by its alpha in the light's colour, which holds for its seconds and then fades over 4 s
//! (`opensacred_rules::particles::Mark`): the lost souls' dark star of streaks at each soul's
//! starting point. The night tints it as it does the ground under it. A body's blood puddle (907,
//! `opensacred_rules::particles::Puddle`, #2516) is laid so too, unturned, growing and fading by
//! its own clock.

use std::collections::HashMap;

use bevy::camera::visibility::NoFrustumCulling;
use bevy::prelude::*;
use opensacred_rules::particles::{Mark, Puddle};

use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, ground_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;

/// A mark to lay: its texture, and its seconds before it fades and its half side, or a puddle's
/// clock.
#[derive(Component)]
struct Unmade {
    texture: &'static str,
    kind: Kind,
}

/// What is laid: a mark of its seconds before it fades and its half side, or a body's puddle,
/// of its body.
#[derive(Clone, Copy, Debug)]
enum Kind {
    Mark { hold: f32, half: f32 },
    Puddle(Entity),
}

/// What shows: a mark and its half side, or a puddle.
#[derive(Clone, Copy, Debug)]
enum Look {
    Mark(Mark, f32),
    Puddle(Puddle),
}

impl Look {
    fn update(&mut self, dt: f32) {
        match self {
            Self::Mark(mark, _) => mark.update(dt),
            Self::Puddle(puddle) => puddle.update(dt),
        }
    }

    fn done(&self) -> bool {
        match self {
            Self::Mark(mark, _) => mark.done(),
            Self::Puddle(puddle) => puddle.done(),
        }
    }

    /// Its half side, and its alpha.
    fn square(&self) -> (f32, f32) {
        match self {
            Self::Mark(mark, half) => (*half, mark.alpha()),
            Self::Puddle(puddle) => (puddle.half(), puddle.alpha()),
        }
    }
}

/// A mark that shows: its look, its turn and its mesh.
#[derive(Component)]
struct Shown {
    look: Look,
    turn: f32,
    mesh: Handle<Mesh>,
    /// A puddle's body, whose model it lies under while the body lies (#2586).
    body: Option<Entity>,
}

/// The marks' materials by their textures, laid over by their alpha; none where the texture is
/// missing.
#[derive(Resource, Default)]
struct Looks(HashMap<&'static str, Option<Handle<Effect>>>);

/// The marks' dice (`rand()`).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x779))
    }
}

/// Lays the mark `(texture, hold, half)` on the ground at `place`.
pub(crate) fn lay(
    commands: &mut Commands,
    place: Vec3,
    (texture, hold, half): (&'static str, f32, f32),
) {
    commands.spawn((
        Unmade {
            texture,
            kind: Kind::Mark { hold, half },
        },
        Transform::from_translation(place),
        DespawnOnExit(AppState::InGame),
    ));
}

/// Lays the blood puddle of `body` of `texture` on the ground at `place` (907, `0x76E150`).
pub(crate) fn puddle(
    commands: &mut Commands,
    (place, body): (Vec3, Entity),
    texture: &'static str,
) {
    commands.spawn((
        Unmade {
            texture,
            kind: Kind::Puddle(body),
        },
        Transform::from_translation(place),
        DespawnOnExit(AppState::InGame),
    ));
}

/// Gives each new mark its look, its turn at random (`rand()/32767 · 2π`), its mesh and its
/// material (`0x76DD40`); a puddle none (`0x76E150`).
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
        let material = looks
            .0
            .entry(unmade.texture)
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
                image.map(|image| effects.add(ground_material(image)))
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
            Kind::Puddle(_) => (Look::Puddle(Puddle::default()), 0.0),
        };
        let body = match unmade.kind {
            Kind::Puddle(body) => Some(body),
            Kind::Mark { .. } => None,
        };
        let mesh = meshes.add(empty());
        let mut made = commands.entity(entity);
        made.remove::<Unmade>().insert((
            Shown {
                look,
                turn,
                mesh: mesh.clone(),
                body,
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
/// to 1 at `(s, s)`; each goes once faded.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut marks: Query<(Entity, &mut Shown, &GlobalTransform)>,
    (mut meshes, orders): (ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs();
    let s = iso::MODEL_SCALE;
    let (across, depth) = iso::fx_axes();
    for (entity, mut mark, place) in &mut marks {
        mark.look.update(dt);
        if mark.look.done() {
            commands.entity(entity).despawn();
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
            Look::Puddle(_) => [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
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
            // At its object's place in the order, as the original draws the system (#2188); a
            // puddle just behind its body while the body lies, which covers it as the original's
            // models of their true depths cover its flat quad, whatever cells the two take
            // (#2586).
            let origin = place.translation();
            let order = mark
                .body
                .and_then(|body| orders.behind(body))
                .or_else(|| orders.at(origin));
            quads.in_order(order, origin);
            quads.on_level(origin.y);
            quads.into_mesh(&mut mesh);
        }
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
