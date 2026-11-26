//! The mortar shells' impacts (1020, `FX_GRENADEIMPACT`, #2160; docs/re/effects.md, "The mortar's
//! impact"): where the Dwarf's shell lands (`guns`), the screen shakes, a flash added and a dark
//! cloud laid over grow as they fade, 75 tongues of fire flare up and out, sparks burst up and
//! fall bouncing, all for about a second, and a crater lies under them on the ground for 10 s and
//! fades over 4 (`opensacred_rules::particles::GrenadeImpact`). The crater and the tongues are
//! tinted by the night, the flash, the cloud and the sparks not.

use std::collections::HashMap;

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    GRENADE_CRATER_HALF, GRENADE_CRATERS, GRENADE_EXPLOSIONS, GRENADE_QUAKE, GrenadeImpact,
};

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;

/// How far over the ground the flat parts lie, in world units.
const GROUND_LIFT: f32 = 0.5;

/// Its parts, each a mesh: the crater, the flash, the cloud, the tongues, the sparks added and the
/// sparks laid over.
const PARTS: usize = 6;

/// An impact to make at its entity's place ([`make`]).
#[derive(Component)]
struct Unmade;

/// An impact: its look and the meshes of its parts.
#[derive(Component)]
struct Impact {
    look: GrenadeImpact,
    meshes: [Handle<Mesh>; PARTS],
}

/// The impacts' materials by their texture and whether laid over; none where a texture is missing.
#[derive(Resource, Default)]
struct Looks(HashMap<(&'static str, bool, bool), Option<Handle<Effect>>>);

/// The impacts' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x1020))
    }
}

/// Starts an impact at `at`, the shell's point on the ground, the screen shaking by 8 pixels as it
/// starts (`0x7B70A0`).
pub(crate) fn land(commands: &mut Commands, at: Vec3) {
    commands.queue(|world: &mut World| {
        if let Some(mut weather) = world.get_resource_mut::<crate::weather::Weather>() {
            weather.set(opensacred_rules::weather::Event::Quake(GRENADE_QUAKE));
        }
    });
    commands.spawn((
        Unmade,
        Transform::from_translation(at),
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
}

/// An impact as a save keeps it (#3774, the epic #3753): its place and its look, none for one
/// not made yet; a load makes it again there, going on as it went.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedImpact {
    at: Vec3,
    look: Option<GrenadeImpact>,
}

/// The impacts in `world` as a save keeps them (#3774).
pub(crate) fn saved(world: &mut World) -> Vec<SavedImpact> {
    let mut impacts = world.query::<(&Transform, Option<&Impact>, Has<Unmade>)>();
    impacts
        .iter(world)
        .filter(|(_, impact, unmade)| impact.is_some() || *unmade)
        .map(|(at, impact, _)| SavedImpact {
            at: at.translation,
            look: impact.map(|impact| impact.look.clone()),
        })
        .collect()
}

/// Makes a loaded save's impacts again where they were (#3774), with no quake.
pub(crate) fn again(world: &mut World, impacts: Vec<SavedImpact>) {
    for saved in impacts {
        let mut made = world.spawn((
            Unmade,
            Transform::from_translation(saved.at),
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(look) = saved.look {
            made.insert(crate::fx::Resumes(look));
        }
    }
}

/// Gives each new impact its look, its meshes and their materials (`0x7B67E0`).
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, Option<&crate::fx::Resumes<GrenadeImpact>>), With<Unmade>>,
    (data, assets, mut looks, mut dice): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Dice>,
    ),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, resumed) in &unmade {
        // A loaded impact goes on as it went (#3774).
        let look = match resumed {
            Some(crate::fx::Resumes(look)) => look.clone(),
            None => GrenadeImpact::new(&mut dice.0),
        };
        let mut material = |name: &'static str, over: bool, tinted: bool| {
            looks
                .0
                .entry((name, over, tinted))
                .or_insert_with(|| {
                    let image = data
                        .as_ref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("grenade impacts: no texture {name}; its part is not drawn");
                    }
                    let mode = if over {
                        AlphaMode::Blend
                    } else {
                        AlphaMode::Add
                    };
                    // A tinted part, which the world's camera draws, among the objects (#3459).
                    image.map(|image| {
                        let material = particle_material(image, mode);
                        effects.add(if tinted {
                            crate::fx::among_objects(material)
                        } else {
                            material
                        })
                    })
                })
                .clone()
        };
        let [cloud, flash, sparks, tongues] = GRENADE_EXPLOSIONS;
        let crater = GRENADE_CRATERS[look.crater.min(GRENADE_CRATERS.len() - 1)];
        // The crater and the tongues lie with the world, tinted by the night.
        let parts = [
            (material(crater, true, true), true),
            (material(flash, false, false), false),
            (material(cloud, true, false), false),
            (material(tongues, true, true), true),
            (material(sparks, false, false), false),
            (material(sparks, true, false), false),
        ];
        let meshes_of: [Handle<Mesh>; PARTS] = std::array::from_fn(|_| meshes.add(empty()));
        let mut made = commands.entity(entity);
        made.remove::<(Unmade, crate::fx::Resumes<GrenadeImpact>)>()
            .insert(Impact {
                look,
                meshes: meshes_of.clone(),
            });
        for (mesh, (material, tinted)) in meshes_of.into_iter().zip(parts) {
            let Some(material) = material else {
                continue;
            };
            made.with_child((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
            ))
            .insert_if(RenderLayers::layer(iso::UNTINTED), || !tinted);
        }
    }
}

/// A square flat on the ground about the impact's place of the half side `half` in the models'
/// units, the whole texture, in `colour`.
fn flat(quads: &mut Quads, half: f32, colour: [f32; 4]) {
    let (across, down) = iso::fx_axes();
    let (a, d) = (
        across * half * iso::MODEL_SCALE,
        down * half * iso::MODEL_SCALE,
    );
    let ground = Vec3::Y * GROUND_LIFT;
    quads.push_corners(
        [
            ground - a - d,
            ground + a - d,
            ground + a + d,
            ground - a + d,
        ],
        [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
        colour,
    );
}

/// Runs each impact a frame on (`0x7B6BE0`, `0x7B6DD0`): its crater, and while it bursts its
/// flash, its cloud, its tongues scaled about its place and its sparks facing the camera, each of
/// its layer's frame of the 2 × 2 grid, the first layer's added and the others laid over; it goes
/// once its crater has faded.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut impacts: Query<(Entity, &mut Impact, &GlobalTransform)>,
    (mut meshes, mut dice, orders): (ResMut<Assets<Mesh>>, ResMut<Dice>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let ramps = GrenadeImpact::ramps();
    for (entity, mut impact, at) in &mut impacts {
        let impact = &mut *impact;
        impact.look.update(dt, &mut dice.0);
        if impact.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut parts: [Quads; PARTS] = std::array::from_fn(|_| Quads::default());
        let [crater, flash, cloud, tongues, added, over] = &mut parts;
        flat(
            crater,
            GRENADE_CRATER_HALF,
            [1.0, 1.0, 1.0, impact.look.crater_alpha()],
        );
        if impact.look.bursting() {
            if let Some((half, alpha)) = impact.look.flash() {
                flat(flash, half, [1.0, 1.0, 1.0, alpha]);
            }
            if let Some((half, alpha)) = impact.look.cloud() {
                flat(cloud, half, [1.0, 1.0, 1.0, alpha]);
            }
            let (scale, alpha) = impact.look.tongue();
            for tongue in &impact.look.tongues {
                let corners = tongue.corners.map(|[x, y, z]| {
                    world([x * scale, y * scale, z * scale]) + Vec3::Y * GROUND_LIFT
                });
                tongues.push_corners(corners, tongue.uvs.map(Vec2::from), [1.0, 1.0, 1.0, alpha]);
            }
            for particle in &impact.look.system.particles {
                let Some(ramp) = ramps.get(usize::from(particle.layer)) else {
                    continue;
                };
                let cell = Vec2::new(
                    f32::from(particle.frame % 2),
                    f32::from(particle.frame / 2 % 2),
                ) * 0.5;
                let quads = if particle.layer == 0 {
                    &mut *added
                } else {
                    &mut *over
                };
                quads.push(
                    world(particle.place),
                    particle.size * s,
                    particle.angle,
                    (cell, 0.5),
                    ramp.at(particle.life),
                );
            }
        }
        // At its object's place in the order, as the original draws the system (#2188).
        let origin = at.translation();
        let order = orders.at(origin);
        for (handle, mut quads) in impact.meshes.iter().zip(parts) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                quads.in_order(order, origin);
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The mortar shells' impacts.
pub struct GrenadeImpactsPlugin;

impl Plugin for GrenadeImpactsPlugin {
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
    fn an_impact_saved_goes_on_as_it_went() {
        let mut world = World::new();
        let look = GrenadeImpact::new(&mut Rng::of(1, 2, 3));
        let at = Vec3::new(4.0, 0.0, 9.0);
        world.spawn((
            Impact {
                look: look.clone(),
                meshes: std::array::from_fn(|_| Handle::default()),
            },
            Transform::from_translation(at),
        ));
        world.spawn((Unmade, Transform::from_translation(Vec3::X)));
        let kept = saved(&mut world);
        let bytes = postcard::to_stdvec(&kept).unwrap();
        let kept: Vec<SavedImpact> = postcard::from_bytes(&bytes).unwrap();
        let mut loaded = World::new();
        again(&mut loaded, kept);
        let mut made = loaded.query::<(&Transform, Option<&crate::fx::Resumes<GrenadeImpact>>)>();
        let mut made: Vec<(Vec3, Option<GrenadeImpact>)> = made
            .iter(&loaded)
            .map(|(place, resumed)| (place.translation, resumed.map(|resumed| resumed.0.clone())))
            .collect();
        made.sort_by(|a, b| a.0.x.total_cmp(&b.0.x));
        assert_eq!(made, [(Vec3::X, None), (at, Some(look))]);
    }
}
