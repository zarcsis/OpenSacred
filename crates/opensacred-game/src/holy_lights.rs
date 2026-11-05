//! The look of the Seraphim's holy light (#1092; docs/re/effects.md, "The holy light"): where its
//! object stands (`spells`), `cParticleSystem_holylight` draws for its seconds (804, `0x781100`) a
//! column of light 1,000 high of its vertices' colours alone, added by their alpha and pulsing,
//! its ground symbol turning and a glow on the ground, both added whole, and sparks of
//! `PARTICLE_GLOW03` rising from it, its column and sparks narrowing as it fades
//! (`opensacred_rules::particles::HolyLight`); all untinted by the night, its light of 200 at it.
//!
//! Not followed: the sparks' colours' spread (±15 each, `+0x2138`), and its Direct3D point light on
//! the models.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, Column, HOLY_COLUMN, HolyLight, holy_pulse};
use opensacred_rules::spawning::Rng;

use crate::bolt_looks::world;
use crate::frame::Playing;
use crate::fx::{Effect, Quads, empty, particle_texture, plain_material, whole_material};
use crate::game::{AppState, Data};
use crate::{daynight, iso};

/// How far over the ground the symbol and the glow lie, in world units, so that the ground's own
/// faces do not cut them.
const GROUND_LIFT: f32 = 0.5;

/// A holy light's object to give its look, of its seconds (`spells::strikes_about`).
#[derive(Component)]
pub(crate) struct Look(pub f32);

/// A holy light's look: its sparks and fade, its clock, its dice, and its parts' meshes, the
/// column's, the symbol's, the glow's and the sparks' (its lamp a [`Lamp`] of its own).
#[derive(Component)]
struct Shown {
    light: HolyLight,
    clock: f32,
    dice: Rng,
    parts: [Handle<Mesh>; 4],
}

/// A holy light's lamp, of the object it lights.
#[derive(Component)]
struct Lamp(Entity);

/// The column's, the symbol's and the glow's and sparks' materials, made as the first holy light
/// shows; none where a texture is missing.
#[derive(Resource, Default)]
struct Materials(Option<[Option<Handle<Effect>>; 3]>);

/// The seed of the looks' `rand()` (**estimate**: any).
const SEED: usize = 0x324;

/// Gives each new holy light its look (`0x7816A0`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn show(
    mut commands: Commands,
    looks: Query<(Entity, &Look, &Transform), Added<Look>>,
    (data, assets, mut made, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Materials>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (entity, &Look(seconds), transform) in &looks {
        let [column, symbol, glow] = made
            .0
            .get_or_insert_with(|| {
                let column = Some(materials.add(plain_material()));
                let [symbol, glow] = [particles::HOLY_SYMBOL_TEXTURE, particles::HOLY_GLOW_TEXTURE]
                    .map(|name| {
                        let image = data
                            .as_ref()
                            .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                        if image.is_none() {
                            warn!("holy lights: no texture {name}; its part is not drawn");
                        }
                        image.map(|image| materials.add(whole_material(image)))
                    });
                [column, symbol, glow]
            })
            .clone();
        let parts = [
            meshes.add(empty()),
            meshes.add(empty()),
            meshes.add(empty()),
            meshes.add(empty()),
        ];
        for (mesh, material) in parts.iter().zip([column, symbol, glow.clone(), glow]) {
            if let Some(material) = material {
                commands.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                    ChildOf(entity),
                ));
            }
        }
        commands.spawn((
            Transform::from_translation(transform.translation.with_y(0.0)),
            daynight::Light(particles::HOLY_LIGHT_LIGHT),
            Lamp(entity),
            DespawnOnExit(AppState::InGame),
        ));
        // Its Direct3D light at its place (`0x780C70`, `0x781100`).
        commands.spawn(crate::fx_lights::child(crate::fx_lights::PALE_BLUE, entity));
        let index = usize::try_from(entity.index_u32()).unwrap_or(0);
        commands.entity(entity).insert(Shown {
            light: HolyLight::new(seconds),
            clock: 0.0,
            dice: crate::seed::dice(SEED ^ index),
            parts,
        });
    }
}

/// Runs each holy light's look a frame on (`0x781030`, `0x781100`); once it fades out it draws
/// nothing, its object's time running out with it.
fn run(time: Res<Time>, mut shown: Query<&mut Shown>, mut meshes: ResMut<Assets<Mesh>>) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for mut shown in &mut shown {
        let shown = &mut *shown;
        shown.clock += dt;
        shown.light.update(dt, &mut shown.dice);
        if shown.light.done() {
            for handle in &shown.parts {
                if let Some(mut mesh) = meshes.get_mut(handle) {
                    Quads::default().into_mesh(&mut mesh);
                }
            }
            continue;
        }
        let pulse = holy_pulse(shown.clock);
        let fade = shown.light.fade;
        let c = HOLY_COLUMN;
        // Scaled by the fade across (`0x653A80`).
        let (across, depth) = iso::fx_axes();
        let at = |x: f32, y: f32, z: f32| (across * x * fade + depth * y * fade + Vec3::Y * z) * s;
        let column = column_quads(&c, c.alphas.map(|alpha| (pulse * alpha).trunc()), at);
        let flat = |half: f32, angle: f32, colour: [f32; 4]| {
            let mut quads = Quads::default();
            let (sin, cos) = angle.sin_cos();
            let corner = |x: f32, y: f32| {
                world([x * cos - y * sin, x * sin + y * cos, 0.0]) * half + Vec3::Y * GROUND_LIFT
            };
            quads.push_corners(
                [
                    corner(-1.0, -1.0),
                    corner(1.0, -1.0),
                    corner(1.0, 1.0),
                    corner(-1.0, 1.0),
                ],
                [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
                colour,
            );
            quads
        };
        let grey = particles::HOLY_SYMBOL_GREY;
        let symbol = flat(
            particles::HOLY_SYMBOL_HALF,
            particles::HOLY_SYMBOL_TURN * shown.clock,
            [grey, grey, grey, 1.0],
        );
        let glow = flat(particles::HOLY_GLOW_HALF, 0.0, particles::HOLY_GLOW_COLOUR);
        let mut sparks = Quads::default();
        for spark in &shown.light.system.particles {
            sparks.push(
                world(spark.place),
                spark.size * s,
                0.0,
                (Vec2::ZERO, 1.0),
                particles::HOLY_SPARK_COLOUR,
            );
        }
        for (handle, quads) in shown.parts.iter().zip([column, symbol, glow, sparks]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The quads of a column of light `c` (the holy light's, the purgatory's): its cylinder and its
/// two bands across it, from its edge out to nothing, its feet's alphas `feet` (out of 255), at
/// the points that `at` gives of the systems' frame. That frame's x lies along the screen's
/// horizontal on the ground (docs/re/effects.md, "Their flight", `iso::fx_axes`), so that the
/// bands face the camera; the quads split as the original's strips are.
pub(crate) fn column_quads(
    c: &Column,
    feet: [f32; 2],
    at: impl Fn(f32, f32, f32) -> Vec3,
) -> Quads {
    let colour = |rgb: [f32; 3], alpha: f32| [rgb[0], rgb[1], rgb[2], alpha / 255.0];
    let mut column = Quads::default();
    let foot = colour(c.foot, feet[0]);
    let top = colour(c.top, c.tops[0]);
    for k in 0..c.segments {
        let angle = |k: u16| std::f32::consts::TAU * f32::from(k) / f32::from(c.segments);
        let (a, b) = (angle(k), angle(k + 1));
        column.push_shaded(
            [
                at(c.radius * a.sin(), c.radius * a.cos(), 0.0),
                at(c.radius * b.sin(), c.radius * b.cos(), 0.0),
                at(c.radius * b.sin(), c.radius * b.cos(), c.height),
                at(c.radius * a.sin(), c.radius * a.cos(), c.height),
            ],
            [foot, foot, top, top],
        );
    }
    let inner_foot = colour(c.foot, feet[1]);
    let inner_top = colour(c.top, c.tops[1]);
    for side in [-1.0, 1.0] {
        let (inner, outer) = (side * c.radius, side * c.band);
        column.push_shaded(
            [
                at(outer, 0.0, 0.0),
                at(inner, 0.0, 0.0),
                at(inner, 0.0, c.height),
                at(outer, 0.0, c.height),
            ],
            [
                colour(c.foot, 0.0),
                inner_foot,
                inner_top,
                colour(c.top, 0.0),
            ],
        );
    }
    column
}

/// Puts out the lamp of each holy light gone or faded out.
fn unlit(mut commands: Commands, lamps: Query<(Entity, &Lamp)>, shown: Query<&Shown>) {
    for (lamp, &Lamp(of)) in &lamps {
        if shown.get(of).map_or(true, |shown| shown.light.done()) {
            commands.entity(lamp).despawn();
        }
    }
}

/// The holy lights' looks.
pub struct HolyLightsPlugin;

impl Plugin for HolyLightsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Materials>().add_systems(
            Update,
            (show, run, unlit)
                .chain()
                .in_set(Playing)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
