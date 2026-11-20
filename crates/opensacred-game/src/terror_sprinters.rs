//! The looks of the terror sprinters' aura (1167, `FX_TERRORSPRINTER`, #2921; docs/re/effects.md,
//! "The terror sprinters' aura"): the aura that a terror sprinter mage casts (`creature_casts`)
//! lays about its feet a circle of `Fx_circle03` added whole in purple, turning, under a still
//! ring of twenty black runes laid over by their alpha, both growing in over half a second,
//! standing for the aura's 8 s and shrinking away over half a second, and lights the models about
//! it purple (`fx_lights`); each sprinter it reaches wears a smaller copy for the copy's seconds
//! (`opensacred_rules::particles::TerrorSprinter`). A look follows its bearer and goes with it.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    self, TERROR_CIRCLE, TERROR_LIGHT, TERROR_RUNES, TerrorSprinter,
};

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;

/// How far over the ground the circle and the ring lie, in world units.
const GROUND_LIFT: f32 = 0.5;
/// The runes' black (`0xFF000000`).
const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
/// How much nearer the camera the runes' meshes lie than the circle's, in world units.
const RUNES_NEARER: f32 = 0.5;

/// A look asked for a bearer: a sprinter's copy or the mage's own, of its seconds.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Aura {
    pub bearer: Entity,
    pub copy: bool,
    pub seconds: f32,
}

/// Lays the look of the aura `aura` (`0x7C1390`).
pub(crate) fn show(commands: &mut Commands, aura: Aura) {
    commands.spawn((
        aura,
        Transform::default(),
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
}

/// A look shown: its state, its bearer, and its circle's mesh and each rune's.
#[derive(Component)]
struct Shown {
    look: TerrorSprinter,
    bearer: Entity,
    circle: Handle<Mesh>,
    runes: Vec<Handle<Mesh>>,
}

/// The looks' materials, found at the first: the circle's and each rune's.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    circle: Option<Handle<Effect>>,
    runes: Vec<Option<Handle<Effect>>>,
}

impl Looks {
    /// Finds the looks' textures at the first look.
    fn make(&mut self, data: Option<&Data>, assets: &AssetServer, effects: &mut Assets<Effect>) {
        if self.made {
            return;
        }
        self.made = true;
        let Some(data) = data else {
            return;
        };
        let image = |name: &str| {
            let image = particle_texture(&data.0.assets, assets, name);
            if image.is_none() {
                warn!("terror sprinters: no texture {name}; it is not drawn");
            }
            image
        };
        self.circle = image(TERROR_CIRCLE).map(|image| effects.add(whole_material(image)));
        self.runes = (1..=TERROR_RUNES)
            .map(|rune| {
                image(&particles::terror_rune(rune))
                    .map(|image| effects.add(particle_material(image, AlphaMode::Blend)))
            })
            .collect();
    }
}

/// The looks' dice (`rand()`; **estimate**: any seed).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x48F))
    }
}

/// Gives each new look its state, its meshes and, the mage's, its light (`0x7C1050`).
fn make(
    mut commands: Commands,
    auras: Query<(Entity, &Aura), Without<Shown>>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes, mut dice): (
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
        ResMut<Dice>,
    ),
) {
    for (entity, aura) in &auras {
        looks.make(data.as_deref(), &assets, &mut effects);
        // Both untinted by the night, the runes' black as the circle's purple; the runes a little
        // nearer the camera, so that they are laid over the circle after it, as the original
        // draws them.
        let mut child = |material: Option<Handle<Effect>>, nearer: f32| {
            let mesh = meshes.add(empty());
            if let Some(material) = material {
                commands.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::from_translation(-iso::forward() * nearer),
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                    ChildOf(entity),
                ));
            }
            mesh
        };
        let circle = child(looks.circle.clone(), 0.0);
        let runes = looks
            .runes
            .clone()
            .into_iter()
            .map(|material| child(material, RUNES_NEARER))
            .collect();
        if !aura.copy {
            commands.spawn(crate::fx_lights::child(
                TERROR_LIGHT,
                crate::fx_lights::LOW,
                entity,
            ));
        }
        commands.entity(entity).insert(Shown {
            look: TerrorSprinter::new(aura.copy, aura.seconds, &mut dice.0),
            bearer: aura.bearer,
            circle,
            runes,
        });
        debug!(
            "terror sprinters: a look on {}, a copy {}, {} s",
            aura.bearer, aura.copy, aura.seconds
        );
    }
}

/// The corners of a flat square of the half side `half` turned by `turn` about its middle, in
/// the models' frame at the world's scale, a little over the ground.
fn flat(half: f32, turn: f32) -> [Vec3; 4] {
    let (sin, cos) = turn.sin_cos();
    let corner = |x: f32, y: f32| {
        world([x * cos - y * sin, x * sin + y * cos, 0.0]) * half + Vec3::Y * GROUND_LIFT
    };
    [
        corner(-1.0, -1.0),
        corner(1.0, -1.0),
        corner(1.0, 1.0),
        corner(-1.0, 1.0),
    ]
}

/// Runs each look a frame on at its bearer's feet (`0x7C1120`, `0x7C11C0`): its circle and its
/// ring, at its place in the order; it goes as it shrinks away or with its bearer.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &mut Transform)>,
    bearers: Query<&GlobalTransform>,
    (mut meshes, orders): (ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut shown, mut at) in &mut shown {
        let shown = &mut *shown;
        let Ok(bearer) = bearers.get(shown.bearer) else {
            commands.entity(entity).despawn();
            continue;
        };
        if !shown.look.update(dt) {
            commands.entity(entity).despawn();
            continue;
        }
        at.translation = bearer.translation();
        let origin = at.translation;
        let order = orders.at(origin);
        let (half, turn, colour) = shown.look.circle();
        let mut circle = Quads::default();
        circle.push_corners(
            flat(half, turn),
            [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
            colour,
        );
        if let Some(mut mesh) = meshes.get_mut(&shown.circle) {
            circle.in_order(order, origin);
            circle.into_mesh(&mut mesh);
        }
        let mut runes: Vec<Quads> = shown.runes.iter().map(|_| Quads::default()).collect();
        for (index, corners) in shown.look.ring().segments() {
            let Some(rune) = shown
                .look
                .runes
                .get(index)
                .and_then(|&rune| usize::from(rune).checked_sub(1))
            else {
                continue;
            };
            if let Some(quads) = runes.get_mut(rune) {
                quads.push_corners(
                    corners.map(|[x, y]| world([x, y, 0.0]) + Vec3::Y * GROUND_LIFT),
                    [Vec2::Y, Vec2::ZERO, Vec2::X, Vec2::ONE],
                    BLACK,
                );
            }
        }
        for (mut quads, handle) in runes.into_iter().zip(&shown.runes) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                quads.in_order(order, origin);
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The terror sprinters' aura's looks.
pub struct TerrorSprintersPlugin;

impl Plugin for TerrorSprintersPlugin {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flat_square_lies_over_the_ground_turned_about_its_middle() {
        let s = iso::MODEL_SCALE;
        let [a, b, c, _] = flat(40.0, 0.0);
        assert!((a.y - GROUND_LIFT).abs() < 1e-6 && (c.y - GROUND_LIFT).abs() < 1e-6);
        assert!(((b - a).length() - 80.0 * s).abs() < 1e-3);
        assert!(((a + c) / 2.0).xz().length() < 1e-4);
        // A quarter turn takes each corner to the next.
        let [turned, ..] = flat(40.0, std::f32::consts::FRAC_PI_2);
        assert!((turned - flat(40.0, 0.0)[1]).length() < 1e-3);
    }
}
