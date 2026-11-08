//! The Dwarf's guns' sights at his cannon's muzzle, the bone `canon_fx01` of his back's cannon
//! (#1092; docs/re/effects.md, "The Dwarf's guns' fires"): his flamethrower's (1016) flames
//! streaming 200 a second along the muzzle's axis, re-aimed each frame, for `3.25/r` s, their
//! dark smoke laid over by its alpha and their fire added, then a thin smoke rising until
//! `6.25/r`; his cannon's flash (1017) and his mortar's (1018), placed once at the muzzle, smoke,
//! sparks drawn back, fire and streaks shot along it, for `0.18/r` and 0.15 s
//! (`opensacred_rules::particles::DwarfFireSight`). Each lights the models (1, 0.8, 0.4) at the
//! muzzle while it fires; none is tinted by the night. The flamethrower's flames lie just behind
//! the Dwarf's model, as their system follows him, the shots' at their muzzle's place, where
//! their objects were placed, in the original's order of drawing (`fx::Orders`, #2220).

use std::collections::HashMap;

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{DWARF_SMOKE_TEXTURE, DwarfFire, DwarfFireSight, Particle, Ramp};

use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;

/// The Dwarf's cannon's muzzle (46), else his right hand.
const MUZZLE: [&str; 2] = ["canon_fx01", "Bip01 R Hand"];

/// A sight that fires: its shooter, its look, its muzzle's point and axis in the models' frame
/// (the shots' fixed at their first frame), the meshes of its smoke laid over, its fire added and
/// the flamethrower's last smoke, and its light while it shines.
#[derive(Component)]
struct Firing {
    shooter: Entity,
    look: DwarfFireSight,
    muzzle: Option<([f32; 3], [f32; 3])>,
    meshes: [Handle<Mesh>; 3],
    light: Option<Entity>,
}

/// The sights' materials by their texture and whether laid over; none where a texture is missing.
#[derive(Resource, Default)]
struct Looks(HashMap<(&'static str, bool), Option<Handle<Effect>>>);

/// The sights' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x1016))
    }
}

/// Starts the sight of `kind` at `shooter`'s muzzle at the clips' rate (the FX events `0x94` to
/// `0x96`, `0x516C36`, `0x5172F0`, `0x516A4A`).
pub(crate) fn fire(commands: &mut Commands, kind: DwarfFire, shooter: Entity) {
    commands.spawn((
        Unmade { kind, shooter },
        Transform::IDENTITY,
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
}

/// A sight to make ([`make`]).
#[derive(Component)]
struct Unmade {
    kind: DwarfFire,
    shooter: Entity,
}

/// Gives each new sight its look, its meshes and their materials, in the camera that draws after
/// the night's pass.
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, unmade) in &unmade {
        let mut material = |name: &'static str, over: bool| {
            looks
                .0
                .entry((name, over))
                .or_insert_with(|| {
                    let image = data
                        .as_ref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("dwarf fires: no texture {name}; its part is not drawn");
                    }
                    let mode = if over {
                        AlphaMode::Blend
                    } else {
                        AlphaMode::Add
                    };
                    image.map(|image| effects.add(particle_material(image, mode)))
                })
                .clone()
        };
        let texture = unmade.kind.texture();
        let parts = [
            material(texture, true),
            material(texture, false),
            material(DWARF_SMOKE_TEXTURE, true),
        ];
        let meshes_of: [Handle<Mesh>; 3] = std::array::from_fn(|_| meshes.add(empty()));
        let mut made = commands.entity(entity);
        made.remove::<Unmade>().insert(Firing {
            shooter: unmade.shooter,
            look: DwarfFireSight::new(unmade.kind, opensacred_rules::arts::GUN_RATE),
            muzzle: None,
            meshes: meshes_of.clone(),
            light: None,
        });
        for (mesh, material) in meshes_of.into_iter().zip(parts) {
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
        debug!("dwarf fires: {:?} of {}", unmade.kind, unmade.shooter);
    }
}

/// The muzzle's point and its unit axis, the bone's `x`, in the models' frame.
fn muzzle(
    shooter: Entity,
    children: &Query<&Children>,
    (bones, worn): (&Query<(&Name, &GlobalTransform)>, &crate::making::Worn),
) -> Option<([f32; 3], [f32; 3])> {
    let bone = crate::making::bone_entity(shooter, &MUZZLE, children, (bones, worn))?;
    let (_, at) = bones.get(bone).ok()?;
    let axis = Vec3::from(models(at.affine().transform_vector3(Vec3::X))).try_normalize()?;
    Some((models(at.translation()), axis.to_array()))
}

/// Lays `particles` out facing the camera, turned, each of its layer's ramp's colour at its life
/// and its frame of the grid of `side` frames a side, into `over` for those of `over_layer` and
/// `added` for the rest.
fn lay(
    (particles, ramps, side): (&[Particle], &[Ramp], u8),
    over_layer: Option<u8>,
    (over, added): (&mut Quads, &mut Quads),
) {
    let grid = f32::from(side.max(1));
    for particle in particles {
        let Some(ramp) = ramps.get(usize::from(particle.layer)) else {
            continue;
        };
        let cell = Vec2::new(
            f32::from(particle.frame % side.max(1)),
            f32::from(particle.frame / side.max(1) % side.max(1)),
        ) / grid;
        let quads = if Some(particle.layer) == over_layer {
            &mut *over
        } else {
            &mut *added
        };
        quads.push(
            world(particle.place),
            particle.size * iso::MODEL_SCALE,
            particle.angle,
            (cell, 1.0 / grid),
            ramp.at(particle.life),
        );
    }
}

/// Runs each sight a frame on (`0x7B35F0`, `0x7B41B0`, `0x7B4DC0`): the flamethrower at its
/// muzzle each frame, the shots where they were placed, their particles in the world, their light
/// at the muzzle while they fire; it goes once it is over, or with its shooter. The
/// flamethrower's lie just behind its shooter's model (its flag `0x200`), the shots' at their
/// muzzle's place (their flag `0x40`), in the order (#2220).
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut sights: Query<(Entity, &mut Firing)>,
    mut lights: Query<&mut Transform, With<crate::fx_lights::FxLight>>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (mut meshes, mut dice, orders): (ResMut<Assets<Mesh>>, ResMut<Dice>, Orders),
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut firing) in &mut sights {
        let firing = &mut *firing;
        let now = muzzle(firing.shooter, &children, (&bones, &worn));
        let placed = match (firing.look.kind, firing.muzzle) {
            (DwarfFire::Flamer, _) | (_, None) => now,
            (_, Some(fixed)) => Some(fixed),
        };
        let Some((point, along)) = placed.or(firing.muzzle) else {
            commands.entity(entity).despawn();
            continue;
        };
        let before = firing.muzzle.map_or(point, |(at, _)| at);
        firing.muzzle = Some((point, along));
        firing
            .look
            .update(dt, ((before, point), along), &mut dice.0);
        if firing.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        // Its Direct3D light (1, 0.8, 0.4) at the muzzle while it fires (`0x642EF0`).
        match (firing.look.lit(), firing.light) {
            (true, None) => {
                let light = commands
                    .spawn(crate::fx_lights::light(
                        crate::fx_lights::FIRE,
                        crate::fx_lights::HIGH,
                        world(point),
                        entity,
                        (None, Vec3::ZERO),
                    ))
                    .id();
                firing.light = Some(light);
            }
            (true, Some(light)) => {
                if let Ok(mut at) = lights.get_mut(light) {
                    at.translation = world(point);
                }
            }
            (false, Some(light)) => {
                commands.entity(light).try_despawn();
                firing.light = None;
            }
            (false, None) => {}
        }
        let (mut over, mut added, mut smoke) =
            (Quads::default(), Quads::default(), Quads::default());
        let ramps = firing.look.ramps();
        lay(
            (&firing.look.system.particles, &ramps, 2),
            Some(0),
            (&mut over, &mut added),
        );
        let smoke_ramp = [DwarfFireSight::smoke_ramp()];
        lay(
            (&firing.look.smoke.particles, &smoke_ramp, 1),
            Some(0),
            (&mut smoke, &mut Quads::default()),
        );
        let order = match firing.look.kind {
            DwarfFire::Flamer => orders.behind(firing.shooter),
            _ => orders.at(world(point)),
        };
        for (handle, mut quads) in firing.meshes.iter().zip([over, added, smoke]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                quads.in_order(order, Vec3::ZERO);
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The Dwarf's guns' sights.
pub struct DwarfFiresPlugin;

impl Plugin for DwarfFiresPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>()
            .init_resource::<Dice>()
            .add_systems(
                Update,
                (make, run)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .after(crate::frame::Step::Npcs)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
