//! The particle systems that make their particles once and live while they do (#1261;
//! docs/re/effects.md, "The undead's effects"): the ring of an undead that rises again and the
//! dust of a corpse that vanishes, the variants 3 and 4 of 781 (`TYPE_FX_PUZZLE_SOLVED_V`,
//! `opensacred_rules::particles::{rise_ring, dust}`), drawn as the fires' billboards (`fx`): a
//! quad of each particle facing the camera, of its ramp's colour at its life, not tinted by the
//! night unless the light at it tints it (a script's smoke, `Burst::tinted`). A ring follows the
//! creature it rises about; the dust stays where the corpse lay. A burst that streams emits on
//! from its start for its time (`Burst::stream`, the cloud of a creature's burst, #1264). A
//! burst whose layer pulls draws its particles back to its place (`Layer::pull`, a shot's
//! impact, `gun_fires`).

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

/// Starts `burst` at `at`, following the entity `follows` where given: the burst's entity.
pub fn spawn(commands: &mut Commands, burst: Burst, at: Vec3, follows: Option<Entity>) -> Entity {
    commands
        .spawn((
            Unmade { burst, follows },
            Transform::from_translation(at),
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ))
        .id()
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
/// follows, the mesh its quads go into and the dice of what it streams.
#[derive(Component)]
struct Puff {
    burst: Burst,
    system: System,
    follows: Option<Entity>,
    mesh: Handle<Mesh>,
    dice: opensacred_rules::spawning::Rng,
}

/// The seed of the streams' `rand()` (**estimate**: any).
const SEED: usize = 0x329;

/// The bursts' materials by their texture and blend, made as they are first asked; none where the
/// texture is missing.
#[derive(Resource, Default)]
struct Materials(HashMap<(&'static str, Blend), Option<Handle<crate::fx::Effect>>>);

/// Gives each new burst its particles, its mesh and its material, in the camera that draws after
/// the night's pass, as the untinted effects (#1191), unless the light at it tints it.
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    data: Option<Res<Data>>,
    assets: Res<AssetServer>,
    (mut made, mut materials, mut meshes): (
        ResMut<Materials>,
        ResMut<Assets<crate::fx::Effect>>,
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
                image.map(|image| {
                    materials.add(match burst.blend {
                        Blend::Add => particle_material(image, AlphaMode::Add),
                        Blend::Whole => crate::fx::whole_material(image),
                        Blend::Over => particle_material(image, AlphaMode::Blend),
                    })
                })
            })
            .clone();
        let mesh = meshes.add(empty());
        let tinted = burst.tinted;
        let count = burst
            .stream
            .map_or(0, |stream| stream.count)
            .max(burst.particles.len());
        let mut system = System::new(count);
        system.particles.clone_from(&burst.particles);
        let index = usize::try_from(entity.index_u32()).unwrap_or(0);
        let mut puff = commands.entity(entity);
        if burst.light > 0.0 {
            puff.insert(crate::daynight::Light(burst.light));
        }
        puff.remove::<Unmade>().insert(Puff {
            burst,
            system,
            follows: unmade.follows,
            mesh: mesh.clone(),
            dice: crate::seed::dice(SEED ^ index),
        });
        if let Some(material) = material {
            puff.with_children(|parts| {
                parts
                    .spawn((
                        Mesh3d(mesh),
                        MeshMaterial3d(material),
                        Transform::default(),
                        bevy::camera::visibility::NoFrustumCulling,
                    ))
                    .insert_if(
                        bevy::camera::visibility::RenderLayers::layer(iso::UNTINTED),
                        || !tinted,
                    );
            });
        }
    }
}

/// Ages each burst's particles a frame on by its layer, emits on those it streams while its time
/// lasts, keeps it at the entity it follows, and lays its quads out facing the camera; a burst
/// whose particles are all gone, its stream over, goes.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut puffs: Query<(Entity, &mut Puff, &mut Transform)>,
    places: Query<&GlobalTransform>,
    mut meshes: ResMut<Assets<Mesh>>,
    orders: crate::fx::Orders,
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut puff, mut transform) in &mut puffs {
        let Puff {
            burst,
            system,
            follows,
            mesh,
            dice,
        } = &mut *puff;
        if let Some(at) = follows.and_then(|follows| places.get(follows).ok()) {
            transform.translation = at.translation();
        }
        let layers = std::slice::from_ref(&burst.layer);
        if burst.layer.pull == 0.0 {
            system.update_over(dt, layers, burst.ground);
        } else {
            system.update_toward(dt, layers, [0.0; 3]);
        }
        let streaming = burst.stream.filter(|stream| system.clock < stream.until);
        if let Some(stream) = streaming {
            system.emit(
                std::slice::from_ref(&stream.block),
                layers,
                stream.count,
                dice,
            );
        }
        if system.particles.is_empty() && streaming.is_none() {
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
            let frame = Vec2::new(
                f32::from(particle.frame % side),
                f32::from(particle.frame / side % side),
            );
            quads.push(
                crate::bolt_looks::world(particle.place),
                particle.size * iso::MODEL_SCALE,
                particle.angle,
                (frame / grid, 1.0 / grid),
                burst.ramp.at(particle.life),
            );
        }
        // Just behind the creature it follows, else at its place, as the original draws the
        // system before the creature (#2188).
        let order = (*follows).map_or_else(
            || orders.at(transform.translation),
            |follows| orders.behind(follows),
        );
        quads.in_order(order, transform.translation);
        quads.into_mesh(&mut mesh);
    }
}

/// A point of the world at `at` in the frame of a burst at `origin`: the models' units, the third
/// coordinate up.
pub fn local(at: Vec3, origin: Vec3) -> [f32; 3] {
    crate::bolt_looks::models(at - origin)
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
        let at = origin + crate::bolt_looks::world([2.0, -1.0, 3.0]);
        let [x, y, z] = local(at, origin);
        assert!((x - 2.0).abs() < 1e-3 && (y + 1.0).abs() < 1e-3 && (z - 3.0).abs() < 1e-3);
    }
}
