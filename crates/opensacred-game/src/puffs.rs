//! The particle systems that make their particles once and live while they do (#1261;
//! docs/re/effects.md, "The undead's effects"): the ring of an undead that rises again and the
//! dust of a corpse that vanishes, the variants 3 and 4 of 781 (`TYPE_FX_PUZZLE_SOLVED_V`,
//! `opensacred_rules::particles::{rise_ring, body_end}`), drawn as the fires' billboards (`fx`): a
//! quad of each particle facing the camera, of its ramp's colour at its life, not tinted by the
//! night unless the light at it tints it (a script's smoke, `Burst::tinted`). A ring follows the
//! creature it rises about; the dust stays where the corpse lay. A burst that streams emits on
//! from its start for its time (`Burst::stream`, the cloud of a creature's burst, #1264). A
//! burst whose layer pulls draws its particles back to its place (`Layer::pull`, a shot's
//! impact, `gun_fires`). A burst made at a creature's model is drawn over it, depth tested against
//! it ([`spawn_over`], the level-up's sparks, #2762). The other modules' bursts are made here too
//! (`hit_sparks`, `change_sparks`, `poison_rings`, `lightning_strikes`, `script_particles`).

use std::collections::HashMap;

use bevy::prelude::*;
use opensacred_rules::particles::{Blend, Burst, System};

use crate::frame::{Playing, Unpaused};
use crate::fx::{Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;

/// A burst to make at its entity's place ([`make`]), following `follows` where given, or drawn
/// over the model of `over`.
#[derive(Component)]
pub struct Unmade {
    burst: Burst,
    follows: Option<Entity>,
    over: Option<Entity>,
}

/// Starts `burst` at `at`, following the entity `follows` where given: the burst's entity.
pub fn spawn(commands: &mut Commands, burst: Burst, at: Vec3, follows: Option<Entity>) -> Entity {
    commands
        .spawn((
            Unmade {
                burst,
                follows,
                over: None,
            },
            Transform::from_translation(at),
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ))
        .id()
}

/// Starts `burst` at `at`, the root of the model of `creature`, standing there and drawn over
/// that model as the original draws a system after the creature standing in its cell, depth
/// tested against it (`fx::Orders::over`; the level-up's sparks, #2762): the burst's entity.
pub fn spawn_over(commands: &mut Commands, burst: Burst, at: Vec3, creature: Entity) -> Entity {
    commands
        .spawn((
            Unmade {
                burst,
                follows: None,
                over: Some(creature),
            },
            Transform::from_translation(at),
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ))
        .id()
}

/// [`spawn`] from a world, as the undead's raising runs (`undead`).
pub fn spawn_in(world: &mut World, burst: Burst, at: Vec3, follows: Option<Entity>) {
    world.spawn((
        Unmade {
            burst,
            follows,
            over: None,
        },
        Transform::from_translation(at),
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
}

/// A burst that lives: its texture's grid, its layer, its ramp, its particles, the entity it
/// follows, the creature whose model it is drawn over, the mesh its quads go into and the dice of
/// what it streams.
#[derive(Component)]
struct Puff {
    burst: Burst,
    system: System,
    follows: Option<Entity>,
    over: Option<Entity>,
    mesh: Handle<Mesh>,
    dice: opensacred_rules::spawning::Rng,
}

/// The seed of the streams' `rand()` (**estimate**: any).
const SEED: usize = 0x329;

/// A burst's particles and its dice of what it streams, which a load gives it ([`make`]).
pub struct Running {
    system: System,
    dice: opensacred_rules::spawning::Rng,
}

/// A burst as a save keeps it (#3773, the epic #3753), as the original's save keeps a particle
/// system with every particle (docs/re/saves.md, "What a load gives back of the doings"): its
/// burst, its place, the creature it follows and the one whose model it is drawn over by their
/// names, and its particles and its dice, none for one not made yet; a load makes it again
/// there, its particles living on.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedPuff {
    burst: Burst,
    at: Vec3,
    #[serde(with = "crate::names::optional")]
    follows: Option<Entity>,
    #[serde(with = "crate::names::optional")]
    over: Option<Entity>,
    running: Option<(System, opensacred_rules::spawning::Rng)>,
}

impl bevy::ecs::entity::MapEntities for SavedPuff {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.follows = self.follows.map(|follows| mapper.get_mapped(follows));
        self.over = self.over.map(|over| mapper.get_mapped(over));
    }
}

/// The bursts in `world` as a save keeps them (#3773).
pub(crate) fn saved(world: &mut World) -> Vec<SavedPuff> {
    let mut puffs = world.query::<(&Puff, &Transform)>();
    let mut saved: Vec<SavedPuff> = puffs
        .iter(world)
        .map(|(puff, at)| SavedPuff {
            burst: puff.burst.clone(),
            at: at.translation,
            follows: puff.follows,
            over: puff.over,
            running: Some((puff.system.clone(), puff.dice.clone())),
        })
        .collect();
    let mut unmade = world.query::<(&Unmade, &Transform)>();
    saved.extend(unmade.iter(world).map(|(unmade, at)| SavedPuff {
        burst: unmade.burst.clone(),
        at: at.translation,
        follows: unmade.follows,
        over: unmade.over,
        running: None,
    }));
    saved
}

/// Makes a loaded save's bursts again where they were (#3773), their particles living on; one
/// whose creature the load finds not stands where it was, drawn as any.
pub(crate) fn again(world: &mut World, puffs: Vec<SavedPuff>) {
    for saved in puffs {
        let mut made = world.spawn((
            Unmade {
                burst: saved.burst,
                follows: crate::names::found(saved.follows),
                over: crate::names::found(saved.over),
            },
            Transform::from_translation(saved.at),
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        if let Some((system, dice)) = saved.running {
            made.insert(crate::fx::Resumes(Running { system, dice }));
        }
    }
}

/// The bursts' materials by their texture and blend, made as they are first asked; none where the
/// texture is missing.
#[derive(Resource, Default)]
struct Materials(HashMap<(&'static str, Blend, bool), Option<Handle<crate::fx::Effect>>>);

/// Gives each new burst its particles, its mesh and its material, in the camera that draws after
/// the night's pass, as the untinted effects (#1191), unless the light at it tints it.
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade, Option<&crate::fx::Resumes<Running>>)>,
    data: Option<Res<Data>>,
    assets: Res<AssetServer>,
    (mut made, mut materials, mut meshes): (
        ResMut<Materials>,
        ResMut<Assets<crate::fx::Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (entity, unmade, resumed) in &unmade {
        let burst = unmade.burst.clone();
        let material = made
            .0
            .entry((burst.texture, burst.blend, burst.tinted))
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
                    let material = match burst.blend {
                        Blend::Add => particle_material(image, AlphaMode::Add),
                        Blend::Whole => crate::fx::whole_material(image),
                        Blend::Over => particle_material(image, AlphaMode::Blend),
                    };
                    // A tinted burst, which the world's camera draws, among the objects (#3459).
                    materials.add(if burst.tinted {
                        crate::fx::among_objects(material)
                    } else {
                        material
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
        let index = usize::try_from(entity.index_u32()).unwrap_or(0);
        // A loaded burst's particles live on (#3773).
        let (system, dice) = match resumed {
            Some(crate::fx::Resumes(running)) => (running.system.clone(), running.dice.clone()),
            None => {
                let mut system = System::new(count);
                system.particles.clone_from(&burst.particles);
                (system, crate::seed::dice(SEED ^ index))
            }
        };
        let mut puff = commands.entity(entity);
        if burst.light > 0.0 {
            puff.insert(crate::daynight::Light(burst.light));
        }
        puff.remove::<(Unmade, crate::fx::Resumes<Running>)>()
            .insert(Puff {
                burst,
                system,
                follows: unmade.follows,
                over: unmade.over,
                mesh: mesh.clone(),
                dice,
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
            over,
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
                own(burst.ramp.at(particle.life), particle.colour),
            );
        }
        // Over the creature it was made at, as the original draws the system after it (#2762),
        // while that creature's model stands in the order.
        if let Some(order) = (*over).and_then(|over| orders.over(over)) {
            quads.as_drawn(Some(order), transform.translation);
            quads.into_mesh(&mut mesh);
            continue;
        }
        // Just behind the creature it follows, else at its place, as the original draws the
        // system before the creature (#2188).
        let order = (*follows).map_or_else(
            || orders.at(transform.translation),
            |follows| orders.borne_by(follows),
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

/// A particle's colour of its ramp's `ramp`, times its own `colour` where its block rolled one
/// (each glow its own, as the arrows' trails, `opensacred_rules::particles::arrow_trail`); white
/// for the others, which keep their ramp's.
fn own([r, g, b, a]: [f32; 4], [cr, cg, cb]: [f32; 3]) -> [f32; 4] {
    [r * cr, g * cg, b * cb, a]
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

    #[test]
    fn a_burst_saved_lives_on_by_the_name_of_what_it_follows() {
        use crate::names::{Id, Loading, Named, Names, NamesPlugin, Refs, Saving, mapped};
        let mut app = App::new();
        app.add_plugins(NamesPlugin);
        let hero = app.world_mut().spawn(Named(Id::Hero)).id();
        let burst = opensacred_rules::particles::rise_ring();
        let mut system = System::new(4);
        system.particles.clone_from(&burst.particles);
        system.clock = 0.7;
        let at = Vec3::new(5.0, 0.0, 6.0);
        app.world_mut().spawn((
            Puff {
                burst: burst.clone(),
                system: system.clone(),
                follows: Some(hero),
                over: None,
                mesh: Handle::default(),
                dice: crate::seed::dice(SEED),
            },
            Transform::from_translation(at),
        ));
        let mut refs = Refs::default();
        let kept = mapped(
            saved(app.world_mut()),
            &mut Saving {
                world: app.world(),
                refs: &mut refs,
            },
        );
        let bytes = postcard::to_stdvec(&(&refs, &kept)).unwrap();
        let (refs, kept): (Refs, Vec<SavedPuff>) = postcard::from_bytes(&bytes).unwrap();
        let mut loaded = App::new();
        loaded.add_plugins(NamesPlugin);
        loaded.world_mut().spawn_batch((0..3).map(|_| ()));
        let hero = loaded.world_mut().spawn(Named(Id::Hero)).id();
        let kept = mapped(
            kept,
            &mut Loading {
                refs: &refs,
                names: loaded.world().resource::<Names>(),
                lost: 0,
            },
        );
        again(loaded.world_mut(), kept);
        let world = loaded.world_mut();
        let made: Vec<_> = world
            .query::<(&Unmade, &Transform, &crate::fx::Resumes<Running>)>()
            .iter(world)
            .map(|(unmade, place, resumed)| {
                (
                    unmade.burst.clone(),
                    unmade.follows,
                    place.translation,
                    resumed.0.system.clone(),
                )
            })
            .collect();
        assert_eq!(made, [(burst, Some(hero), at, system)]);
    }
}
