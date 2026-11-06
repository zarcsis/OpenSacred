//! The marks laid on the ground (779, `cParticleSystem_temptex`; #1092; docs/re/effects.md, "The
//! Wood Elf's sights"): a square of a texture flat on the ground at a turn at random, laid over
//! by its alpha in the light's colour, which holds for its seconds and then fades over 4 s
//! (`opensacred_rules::particles::Mark`): the lost souls' dark star of streaks at each soul's
//! starting point. The night tints it as it does the ground under it.

use std::collections::HashMap;

use bevy::camera::visibility::NoFrustumCulling;
use bevy::prelude::*;
use opensacred_rules::particles::Mark;

use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;

/// A mark to lay: its texture, its seconds before it fades, its half side.
#[derive(Component)]
struct Unmade {
    texture: &'static str,
    hold: f32,
    half: f32,
}

/// A mark that shows: its look, its half side, its turn and its mesh.
#[derive(Component)]
struct Shown {
    look: Mark,
    half: f32,
    turn: f32,
    mesh: Handle<Mesh>,
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
            hold,
            half,
        },
        Transform::from_translation(place),
        DespawnOnExit(AppState::InGame),
    ));
}

/// Gives each new mark its look, its turn at random (`rand()/32767 · 2π`), its mesh and its
/// material (`0x76DD40`).
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
                image.map(|image| effects.add(particle_material(image, AlphaMode::Blend)))
            })
            .clone();
        let roll = f32::from(u16::try_from(dice.0.below(32_768)).unwrap_or(0)) / 32_767.0;
        let mesh = meshes.add(empty());
        let mut made = commands.entity(entity);
        made.remove::<Unmade>().insert((
            Shown {
                look: Mark::new(unmade.hold),
                half: unmade.half,
                turn: roll * std::f32::consts::TAU,
                mesh: mesh.clone(),
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
/// from 1 at its corner of `(−s, −s)` to 0 at `(s, s)`, white at its alpha; it goes once faded.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut marks: Query<(Entity, &mut Shown)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs();
    let s = iso::MODEL_SCALE;
    let (across, depth) = iso::fx_axes();
    for (entity, mut mark) in &mut marks {
        mark.look.update(dt);
        if mark.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let (sin, cos) = mark.turn.sin_cos();
        let half = mark.half;
        let corner = |x: f32, y: f32| {
            let (x, y) = (x * cos + y * sin, -x * sin + y * cos);
            (across * x + depth * y) * s
        };
        let mut quads = Quads::default();
        quads.push_corners(
            [
                corner(-half, -half),
                corner(half, -half),
                corner(half, half),
                corner(-half, half),
            ],
            [Vec2::ONE, Vec2::Y, Vec2::ZERO, Vec2::X],
            [1.0, 1.0, 1.0, mark.look.alpha()],
        );
        if let Some(mut mesh) = meshes.get_mut(&mark.mesh) {
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
