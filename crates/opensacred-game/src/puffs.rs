//! The particle systems that make their particles once and live while they do (#1261;
//! docs/re/effects.md, "The undead's effects"): the ring of an undead that rises again and the
//! dust of a corpse that vanishes, the variants 3 and 4 of 781 (`TYPE_FX_PUZZLE_SOLVED_V`,
//! `opensacred_rules::particles::{rise_ring, dust}`), drawn as the fires' billboards (`fx`): a
//! quad of each particle facing the camera, of its ramp's colour at its life, not tinted by the
//! night. A ring follows the creature it rises about; the dust stays where the corpse lay.

use std::collections::HashMap;

use bevy::prelude::*;
use opensacred_rules::particles::{Blend, Burst, System};

use crate::frame::{Playing, Unpaused};
use crate::fx::{Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;

/// A burst to make at its entity's place ([`make`]), following `follows` where given.
#[derive(Component)]
pub struct Unmade {
    burst: Burst,
    follows: Option<Entity>,
}

/// Starts `burst` at `at`, following the entity `follows` where given.
pub fn spawn(commands: &mut Commands, burst: Burst, at: Vec3, follows: Option<Entity>) {
    commands.spawn((
        Unmade { burst, follows },
        Transform::from_translation(at),
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
}

/// [`spawn`] from a world, as the undead's raising runs (`undead`).
pub fn spawn_in(world: &mut World, burst: Burst, at: Vec3, follows: Option<Entity>) {
    world.spawn((
        Unmade { burst, follows },
        Transform::from_translation(at),
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
}

/// A burst that lives: its texture's grid, its layer, its ramp, its particles, the entity it
/// follows and the mesh its quads go into.
#[derive(Component)]
struct Puff {
    burst: Burst,
    system: System,
    follows: Option<Entity>,
    mesh: Handle<Mesh>,
}

/// The bursts' materials by their texture and blend, made as they are first asked; none where the
/// texture is missing.
#[derive(Resource, Default)]
struct Materials(HashMap<(&'static str, Blend), Option<Handle<StandardMaterial>>>);

/// Gives each new burst its particles, its mesh and its material, in the camera that draws after
/// the night's pass, as the untinted effects (#1191).
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    data: Option<Res<Data>>,
    assets: Res<AssetServer>,
    (mut made, mut materials, mut meshes): (
        ResMut<Materials>,
        ResMut<Assets<StandardMaterial>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (entity, unmade) in &unmade {
        let burst = unmade.burst.clone();
        let material = made
            .0
            .entry((burst.texture, burst.blend))
            .or_insert_with(|| {
                let image = data
                    .as_ref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, burst.texture));
                if image.is_none() {
                    warn!(
                        "puffs: no texture {}; its bursts are not drawn",
                        burst.texture
                    );
                }
                let alpha_mode = match burst.blend {
                    Blend::Add => AlphaMode::Add,
                    Blend::Over => AlphaMode::Blend,
                };
                image.map(|image| materials.add(particle_material(image, alpha_mode)))
            })
            .clone();
        let mesh = meshes.add(empty());
        let mut system = System::new(burst.particles.len());
        system.particles.clone_from(&burst.particles);
        let mut puff = commands.entity(entity);
        puff.remove::<Unmade>().insert(Puff {
            burst,
            system,
            follows: unmade.follows,
            mesh: mesh.clone(),
        });
        if let Some(material) = material {
            puff.with_child((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::default(),
                bevy::camera::visibility::NoFrustumCulling,
                bevy::camera::visibility::RenderLayers::layer(iso::UNTINTED),
            ));
        }
    }
}

/// Ages each burst's particles a frame on by its layer, keeps it at the entity it follows, and
/// lays its quads out facing the camera; a burst whose particles are all gone goes.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut puffs: Query<(Entity, &mut Puff, &mut Transform)>,
    places: Query<&GlobalTransform>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut puff, mut transform) in &mut puffs {
        let Puff {
            burst,
            system,
            follows,
            mesh,
        } = &mut *puff;
        if let Some(at) = follows.and_then(|follows| places.get(follows).ok()) {
            transform.translation = at.translation();
        }
        system.update(dt, std::slice::from_ref(&burst.layer));
        if system.particles.is_empty() {
            commands.entity(entity).despawn();
            continue;
        }
        let Some(mut mesh) = meshes.get_mut(mesh) else {
            continue;
        };
        let side = burst.grid.max(1);
        let grid = f32::from(side);
        let mut quads = Quads::default();
        for particle in &system.particles {
            // The models' frame, its third coordinate up, at the world's scale.
            let [x, y, z] = particle.place;
            let frame = Vec2::new(
                f32::from(particle.frame % side),
                f32::from(particle.frame / side % side),
            );
            quads.push(
                Vec3::new(x, z, y) * iso::MODEL_SCALE,
                particle.size * iso::MODEL_SCALE,
                particle.angle,
                (frame / grid, 1.0 / grid),
                burst.ramp.at(particle.life),
            );
        }
        quads.into_mesh(&mut mesh);
    }
}

/// A point of the world at `at` in the frame of a burst at `origin`: the models' units, the third
/// coordinate up.
pub fn local(at: Vec3, origin: Vec3) -> [f32; 3] {
    let d = (at - origin) / iso::MODEL_SCALE;
    [d.x, d.z, d.y]
}

/// The bursts that make their particles once.
pub struct PuffsPlugin;

impl Plugin for PuffsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Materials>().add_systems(
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
    fn a_point_of_the_world_takes_the_bursts_frame() {
        let origin = Vec3::new(100.0, 5.0, 200.0);
        let at = origin + Vec3::new(2.0, 3.0, -1.0) * iso::MODEL_SCALE;
        let [x, y, z] = local(at, origin);
        assert!((x - 2.0).abs() < 1e-3 && (y + 1.0).abs() < 1e-3 && (z - 3.0).abs() < 1e-3);
    }
}
