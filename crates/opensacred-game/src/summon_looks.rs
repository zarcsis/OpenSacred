//! The summonings' sights (777, `FX_BIGSUMMON`, #1092; docs/re/effects.md, "The summoning"): at
//! each summoning's point, a hero's (`followers`) or a creature's (`creature_casts`), twenty
//! orbits swell out from its middle 20 over the ground to 160 and back over 3.14 s, a rim of ten
//! translucent triangles of the caster's colour laid over everything, two glows at the middle,
//! white bolts from it as the creature comes, and for a hero's the runes on the ground, turning
//! either way and fading in and out (`opensacred_rules::particles::BigSummon`); its light on the
//! night swells with it; none is tinted by the night.

use std::collections::HashMap;

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    BigSummon, SUMMON_GLOW, SUMMON_GLOW_COLOUR, SUMMON_RUNES, summon_colour,
};

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{
    Effect, Quads, empty, particle_material, particle_texture, plain_over_material, whole_material,
};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;

/// How far over the ground the runes lie, and how wide a bolt's line is, in world units.
const GROUND_LIFT: f32 = 0.5;
const BOLT_WIDTH: f32 = 0.75;

/// A summoning's sight to make at its entity's place for its caster ([`make`]).
#[derive(Component)]
struct Unmade {
    caster: Entity,
}

/// A summoning's sight that shows: its look and the meshes of its plain and glowing runes, its
/// triangles, its bolts and its glows.
#[derive(Component)]
struct Shown {
    look: BigSummon,
    meshes: [Handle<Mesh>; 5],
}

/// The sights' materials, made as first asked by their texture and whether laid over: none
/// where a texture is missing.
#[derive(Resource, Default)]
struct Looks(
    HashMap<(&'static str, bool), Option<Handle<Effect>>>,
    Option<Handle<Effect>>,
);

/// The sights' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x777))
    }
}

/// Starts the sight of a summoning by `caster` at `at` (the event 4, `0x523413`, `0x512B39`).
pub(crate) fn spawn(commands: &mut Commands, at: Vec3, caster: Entity) {
    commands.spawn((
        Unmade { caster },
        crate::daynight::Light(0.0),
        Transform::from_translation(at),
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
}

/// Gives each new sight its look in its caster's colour, the hero's with runes (`+0xA9` 0), its
/// meshes and its materials (`0x7699A0`, `0x76A440`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    (hero, heroes, minds): (
        Option<Res<crate::hero::Hero>>,
        Query<(), With<crate::hero::Body>>,
        Query<&crate::ai::Mind>,
    ),
    (data, assets, mut looks, mut dice): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Dice>,
    ),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, unmade) in &unmade {
        let by_hero = heroes.contains(unmade.caster);
        let caster = if by_hero {
            hero.as_deref().map_or(0, |hero| hero.creature)
        } else {
            minds.get(unmade.caster).map_or(0, |mind| mind.creature)
        };
        let vampiress = matches!(caster, 6 | 7);
        let mut material = |name: &'static str, over: bool| {
            looks
                .0
                .entry((name, over))
                .or_insert_with(|| {
                    let image = data
                        .as_ref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("summon looks: no texture {name}; its part is not drawn");
                    }
                    image.map(|image| {
                        effects.add(if over {
                            particle_material(image, AlphaMode::Blend)
                        } else {
                            whole_material(image)
                        })
                    })
                })
                .clone()
        };
        let [plain, glowing] = SUMMON_RUNES[usize::from(!vampiress)];
        let parts = [
            material(plain, true),
            material(glowing, false),
            None,
            None,
            material(SUMMON_GLOW, false),
        ];
        let plain_over = looks
            .1
            .get_or_insert_with(|| effects.add(plain_over_material()))
            .clone();
        let meshes_of: [Handle<Mesh>; 5] = std::array::from_fn(|_| meshes.add(empty()));
        let mut shown = commands.entity(entity);
        shown.remove::<Unmade>().insert(Shown {
            look: BigSummon::new(summon_colour(caster), by_hero, &mut dice.0),
            meshes: meshes_of.clone(),
        });
        for (index, (mesh, material)) in meshes_of.into_iter().zip(parts).enumerate() {
            let material = if matches!(index, 2 | 3) {
                Some(plain_over.clone())
            } else {
                material
            };
            if let Some(material) = material {
                shown.with_child((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                ));
            }
        }
        debug!(
            "summon looks: {}'s summoning, the caster's type {caster}, runes {by_hero}, vampiress {vampiress}",
            unmade.caster
        );
    }
}

/// A line from `from` to `to` facing the camera, `width` to each side, in `colour`.
fn line(quads: &mut Quads, (from, to): (Vec3, Vec3), width: f32, colour: [f32; 4]) {
    let across = (to - from).cross(iso::forward()).normalize_or_zero() * width;
    quads.push_shaded(
        [from - across, from + across, to + across, to - across],
        [colour; 4],
    );
}

/// Runs each sight a frame on (`0x769C60`, `0x769FC0`): its runes flat on the ground, its
/// triangles and its bolts laid over everything, its glows facing the camera at its middle, its
/// light; it goes once it is over.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut sights: Query<(Entity, &mut Shown, &mut crate::daynight::Light)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut dice: ResMut<Dice>,
) {
    let s = iso::MODEL_SCALE;
    for (entity, mut shown, mut light) in &mut sights {
        shown.look.update(time.delta_secs());
        if shown.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        light.0 = shown.look.light();
        let mut runes = [Quads::default(), Quads::default()];
        for rune in shown.look.runes() {
            let (sin, cos) = rune.turn.sin_cos();
            let corner = |(x, y): (f32, f32)| {
                world([
                    rune.half * (x * cos - y * sin),
                    rune.half * (x * sin + y * cos),
                    0.0,
                ]) + Vec3::Y * GROUND_LIFT
            };
            if let Some(quads) = runes.get_mut(rune.texture) {
                quads.push_corners(
                    [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(corner),
                    [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
                    rune.colour,
                );
            }
        }
        let mut triangles = Quads::default();
        for (corners, colours) in shown.look.triangles() {
            let [a, b, c] = corners.map(world);
            let [ca, cb, cc] = colours;
            triangles.push_shaded([a, b, c, c], [ca, cb, cc, cc]);
        }
        let mut bolts = Quads::default();
        if shown.look.bolts_show() {
            for bolt in shown.look.bolts(&mut dice.0) {
                for pair in bolt.windows(2) {
                    line(
                        &mut bolts,
                        (world(pair[0]), world(pair[1])),
                        BOLT_WIDTH,
                        [1.0; 4],
                    );
                }
            }
        }
        let mut glows = Quads::default();
        let middle = world(BigSummon::middle());
        for half in shown.look.glows() {
            glows.push(middle, half * s, 0.0, (Vec2::ZERO, 1.0), SUMMON_GLOW_COLOUR);
        }
        let [plain, glowing] = runes;
        for (handle, quads) in shown
            .meshes
            .iter()
            .zip([plain, glowing, triangles, bolts, glows])
        {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The summonings' sights.
pub struct SummonLooksPlugin;

impl Plugin for SummonLooksPlugin {
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
