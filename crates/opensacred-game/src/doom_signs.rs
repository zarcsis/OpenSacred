//! The Daemon's call of death's sign at a kill (1010, `cParticleSystem_callofdeath`; #1092;
//! docs/re/effects.md, "The call of death's sign"): at the corpse of each creature that the hero
//! kills while it holds the call of death (`spells`' `killed`, `0x5339F3`), its
//! `fx_transdim_sign` flat on the ground three times over, of the half side 1.4 times the dead
//! type's size, each turned the less the later, spinning and braking, the last fading, over
//! 1.76 s; added by their alpha, untinted by the night, at the corpse's place in the order, just
//! behind a creature standing there (`fx::Orders`, #2178)
//! (`opensacred_rules::particles::DoomSign`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{DOOM_SIGN_TEXTURE, DoomSign};

use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;

/// How far over the ground the signs lie, in the models' units, clear of its triangles.
const GROUND_LIFT: f32 = 0.5;

/// A sign to make: the corpse's place and its type's size.
#[derive(Component)]
struct Unmade {
    place: Vec3,
    size: f32,
}

/// A sign that shows: its look and its mesh.
#[derive(Component)]
struct Shown {
    look: DoomSign,
    mesh: Handle<Mesh>,
}

/// The signs' material, `fx_transdim_sign` added by its alpha, made as the first asks; `None`
/// where the texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<Option<Handle<Effect>>>);

/// Shows the call of death's sign at the corpse at `place` of a type of the size `size`
/// (`0x5339F3`, the object 1010 at the corpse on the ground).
pub(crate) fn show(commands: &mut Commands, place: Vec3, size: f32) {
    commands.spawn((Unmade { place, size }, DespawnOnExit(AppState::InGame)));
}

/// Gives each new sign its look, its mesh and its material (`0x7AFB80`, `0x7AFD70`).
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, unmade) in &unmade {
        let material = looks
            .0
            .get_or_insert_with(|| {
                let image = data
                    .as_deref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, DOOM_SIGN_TEXTURE));
                if image.is_none() {
                    warn!("doom signs: no texture {DOOM_SIGN_TEXTURE}; they are not drawn");
                }
                image.map(|image| effects.add(particle_material(image, AlphaMode::Add)))
            })
            .clone();
        let Some(material) = material else {
            commands.entity(entity).despawn();
            continue;
        };
        let mesh = meshes.add(empty());
        commands
            .entity(entity)
            .remove::<Unmade>()
            .insert((
                Shown {
                    look: DoomSign::new(unmade.size),
                    mesh: mesh.clone(),
                },
                Transform::from_translation(unmade.place),
                Visibility::default(),
            ))
            .with_child((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
            ));
    }
}

/// Runs each sign a frame on (`0x7AFC10`) and draws it (`0x7AFC60`): its signs flat on the ground
/// at its place, each turned by its angle (`0x7631E0`), white at its alpha; it goes with its fade.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut signs: Query<(Entity, &mut Shown, &Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    orders: Orders,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let (across, depth) = iso::fx_axes();
    for (entity, mut shown, transform) in &mut signs {
        let shown = &mut *shown;
        shown.look.update(dt);
        if shown.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let half = shown.look.half();
        let mut quads = Quads::default();
        for (turn, alpha) in shown.look.signs() {
            let (sin, cos) = turn.sin_cos();
            let corner = |x: f32, y: f32| {
                (across * (x * cos + y * sin)
                    + depth * (-x * sin + y * cos)
                    + Vec3::Y * GROUND_LIFT)
                    * s
            };
            quads.push_corners(
                [
                    corner(-half, -half),
                    corner(half, -half),
                    corner(half, half),
                    corner(-half, half),
                ],
                [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
                [1.0, 1.0, 1.0, alpha],
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&shown.mesh) {
            quads.in_order(orders.at(transform.translation), transform.translation);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The call of death's signs.
pub struct DoomSignsPlugin;

impl Plugin for DoomSignsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>().add_systems(
            Update,
            (make, run)
                .chain()
                .in_set(Playing)
                .in_set(Unpaused)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
