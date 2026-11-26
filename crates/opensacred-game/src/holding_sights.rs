//! The holds' sights (#1092; docs/re/effects.md, "The burning light and the holy conversion"): the
//! burning light (816) about a creature that the Seraphim's irritation holds, that a blow knocks
//! back or that a brain-footer stares at (`creature_arts`), pale blue sparks bursting now and then
//! about its neck and falling, left in the world, for the hold's seconds, 3 s or the stare's 2 s,
//! the Dwarf's 1083's thrown target's for its hold and each creature it bowls over's for 2 s
//! (`making`, #3142), and each creature that the Gladiator's 1003 stuns for its hold (`making`,
//! #3639) (`opensacred_rules::particles::BurningLight`); and the holy conversion's halo (806) at a
//! converted creature's head, forty small blue-violet glows on a ring that swells away as the
//! conversion ends (`particles::HolyConversion`). Both are added and untinted by the night.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    BURNING_KNOCKED, BURNING_TEXTURE, BurningLight, CONVERSION_TEXTURE, HolyConversion,
};

use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{
    Effect, Orders, Quads, empty, particle_material, particle_texture, whole_material,
};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;

/// The bones the sights stand at: the neck (3), else the root, and the head (20).
const NECK: [&str; 1] = ["Bip01 Neck"];
const HEAD: [&str; 1] = ["Bip01 Head"];

/// A sight to make about its creature ([`make`]).
#[derive(Component)]
enum Unmade {
    Burning { target: Entity, seconds: f32 },
    Conversion { target: Entity, seconds: f32 },
}

/// A burning light that shows: its creature, its look and its mesh.
#[derive(Component)]
struct Burning {
    target: Entity,
    look: BurningLight,
    mesh: Handle<Mesh>,
}

/// A conversion's halo that shows: its creature, its look and its mesh.
#[derive(Component)]
struct Halo {
    target: Entity,
    look: HolyConversion,
    mesh: Handle<Mesh>,
}

/// A holding's sight as a save keeps it (#3779): a burning light or a holy conversion, its
/// creature by its name and its look; a load makes it again at its creature.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename = "SavedHoldingSight")]
pub enum SavedLook {
    Burning {
        #[serde(with = "crate::names::required")]
        target: Entity,
        look: BurningLight,
    },
    Conversion {
        #[serde(with = "crate::names::required")]
        target: Entity,
        look: HolyConversion,
    },
}

impl bevy::ecs::entity::MapEntities for SavedLook {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        match self {
            Self::Burning { target, .. } | Self::Conversion { target, .. } => {
                *target = mapper.get_mapped(*target);
            }
        }
    }
}

/// The holdings' sights in `world` as a save keeps them (#3779).
pub(crate) fn saved(world: &mut World) -> Vec<SavedLook> {
    let mut burning = world.query::<&Burning>();
    let mut saved: Vec<SavedLook> = burning
        .iter(world)
        .map(|burning| SavedLook::Burning {
            target: burning.target,
            look: burning.look.clone(),
        })
        .collect();
    let mut halos = world.query::<&Halo>();
    saved.extend(halos.iter(world).map(|halo| SavedLook::Conversion {
        target: halo.target,
        look: halo.look.clone(),
    }));
    saved
}

/// Makes a loaded save's holdings' sights again (#3779); one whose creature the load finds not
/// is left out.
pub(crate) fn again(world: &mut World, looks: Vec<SavedLook>) {
    for saved in looks {
        match saved {
            SavedLook::Burning { target, look } => {
                if crate::names::found(Some(target)).is_none() {
                    continue;
                }
                world.spawn((
                    Unmade::Burning {
                        target,
                        seconds: 0.0,
                    },
                    crate::fx::Resumes(look),
                    DespawnOnExit(AppState::InGame),
                ));
            }
            SavedLook::Conversion { target, look } => {
                if crate::names::found(Some(target)).is_none() {
                    continue;
                }
                world.spawn((
                    Unmade::Conversion {
                        target,
                        seconds: 0.0,
                    },
                    crate::fx::Resumes(look),
                    DespawnOnExit(AppState::InGame),
                ));
            }
        }
    }
}

/// The sights' materials, made as first asked: the sparks added by their alpha, the halo's glows
/// added whole; none where a texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<[Option<Handle<Effect>>; 2]>);

/// The sights' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x816))
    }
}

/// Starts the burning light about `target` for `seconds` (the event `0x2B`).
pub(crate) fn burn(commands: &mut Commands, target: Entity, seconds: f32) {
    commands.spawn((
        Unmade::Burning { target, seconds },
        DespawnOnExit(AppState::InGame),
    ));
}

/// Starts the holy conversion's halo at `target`'s head for `seconds` (the event `0x25`).
pub(crate) fn convert(commands: &mut Commands, target: Entity, seconds: f32) {
    commands.spawn((
        Unmade::Conversion { target, seconds },
        DespawnOnExit(AppState::InGame),
    ));
}

/// Every knock-back burns its creature 3 s, each its own (the creature's event 3, `0x531BB6`);
/// a throw, the Dwarf's 1083's, no event 3, burns its own seconds where `making` throws it
/// (#3142).
fn knocked(
    added: On<Insert<crate::making::Knocked>>,
    knocks: Query<&crate::making::Knocked>,
    mut commands: Commands,
) {
    if knocks
        .get(added.entity)
        .is_ok_and(|knock| knock.throw.is_some())
    {
        return;
    }
    burn(&mut commands, added.entity, BURNING_KNOCKED);
}

/// Gives each new sight its look at its creature, its mesh and its material (`0x77FA60`,
/// `0x782EF0`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn make(
    mut commands: Commands,
    unmade: Query<(
        Entity,
        &Unmade,
        Option<&crate::fx::Resumes<BurningLight>>,
        Option<&crate::fx::Resumes<HolyConversion>>,
    )>,
    (places, sizes): (Query<&GlobalTransform>, Query<&crate::arrows::Size>),
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, unmade, burning, conversion) in &unmade {
        let target = match *unmade {
            Unmade::Burning { target, .. } | Unmade::Conversion { target, .. } => target,
        };
        let Ok(at) = places.get(target) else {
            commands.entity(entity).despawn();
            continue;
        };
        let [sparks, glows] = looks
            .0
            .get_or_insert_with(|| {
                let mut material = |name: &str, whole: bool| {
                    let image = data
                        .as_ref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("holding sights: no texture {name}; its sight is not drawn");
                    }
                    image.map(|image| {
                        effects.add(if whole {
                            whole_material(image)
                        } else {
                            particle_material(image, AlphaMode::Add)
                        })
                    })
                };
                [
                    material(BURNING_TEXTURE, false),
                    material(CONVERSION_TEXTURE, true),
                ]
            })
            .clone();
        let mesh = meshes.add(empty());
        let mut made = commands.entity(entity);
        made.remove::<(
            Unmade,
            crate::fx::Resumes<BurningLight>,
            crate::fx::Resumes<HolyConversion>,
        )>()
        .insert((
            Transform::from_translation(at.translation()),
            Visibility::default(),
        ));
        let material = match *unmade {
            Unmade::Burning { target, seconds } => {
                let size = sizes.get(target).map_or(0.0, |size| size.0);
                made.insert(Burning {
                    target,
                    // A loaded sight goes on as it went (#3779).
                    look: burning.map_or_else(
                        || BurningLight::new(seconds, size),
                        |crate::fx::Resumes(look)| look.clone(),
                    ),
                    mesh: mesh.clone(),
                });
                sparks
            }
            Unmade::Conversion { target, seconds } => {
                made.insert(Halo {
                    target,
                    look: conversion.map_or_else(
                        || HolyConversion::new(seconds),
                        |crate::fx::Resumes(look)| look.clone(),
                    ),
                    mesh: mesh.clone(),
                });
                glows
            }
        };
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
}

/// Runs each burning light a frame on (`0x77F6B0`, `0x77F840`): its bursts at its creature's neck,
/// else its place, its sparks facing the camera, turned, of their ramp's colour, drawn just
/// behind its creature's model, as the original draws the system before the creature it follows
/// (`fx::Orders`, #2178); it goes once it is over, or with its creature.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn burn_on(
    mut commands: Commands,
    time: Res<Time>,
    mut lights: Query<(Entity, &mut Burning, &Transform)>,
    (places, children, bones, worn): (
        Query<&GlobalTransform>,
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (mut meshes, mut dice): (ResMut<Assets<Mesh>>, ResMut<Dice>),
    orders: Orders,
) {
    let dt = time.delta_secs();
    let ramp = BurningLight::ramp();
    for (entity, mut light, origin) in &mut lights {
        let light = &mut *light;
        let Ok(at) = places.get(light.target) else {
            commands.entity(entity).despawn();
            continue;
        };
        let neck = crate::making::bone(light.target, &NECK, &children, (&bones, &worn))
            .unwrap_or(at.translation());
        light
            .look
            .update(dt, models(neck - origin.translation), &mut dice.0);
        if light.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut quads = Quads::default();
        for spark in &light.look.system.particles {
            quads.push(
                world(spark.place),
                spark.size * iso::MODEL_SCALE,
                spark.angle,
                (Vec2::ZERO, 1.0),
                ramp.at(spark.life),
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&light.mesh) {
            quads.in_order(orders.borne_by(light.target), origin.translation);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// Runs each halo a frame on (`0x782A60`, `0x782AD0`): its glows facing the camera about its
/// creature's head in the head bone's axes (the original's bone `y` the glTF's `z`), drawn just
/// behind its creature's model, as the original draws the system before the creature it follows
/// (`fx::Orders`, #2178); it goes once it has faded, or with its creature.
fn glow_on(
    mut commands: Commands,
    time: Res<Time>,
    mut halos: Query<(Entity, &mut Halo, &mut Transform)>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    mut meshes: ResMut<Assets<Mesh>>,
    orders: Orders,
) {
    let dt = time.delta_secs();
    for (entity, mut halo, mut transform) in &mut halos {
        let halo = &mut *halo;
        halo.look.update(dt);
        if halo.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let Some(head) = crate::making::bone_entity(halo.target, &HEAD, &children, (&bones, &worn))
            .and_then(|bone| bones.get(bone).ok())
            .map(|(_, at)| *at)
        else {
            commands.entity(entity).despawn();
            continue;
        };
        transform.translation = head.translation();
        let mut quads = Quads::default();
        for glow in halo.look.glows() {
            let [x, y, z] = glow.place;
            let point = head.transform_point(Vec3::new(x, z, y)) - head.translation();
            quads.push(
                point,
                glow.half * iso::MODEL_SCALE,
                0.0,
                (Vec2::ZERO, 1.0),
                glow.colour,
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&halo.mesh) {
            quads.in_order(orders.borne_by(halo.target), head.translation());
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The holds' sights.
pub struct HoldingSightsPlugin;

impl Plugin for HoldingSightsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>()
            .init_resource::<Dice>()
            .add_observer(knocked)
            .add_systems(
                Update,
                (make, burn_on, glow_on)
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
    use crate::making::Knocked;

    /// The seconds of the burning lights made so far.
    fn lights(app: &mut App) -> Vec<f32> {
        app.world_mut()
            .query::<&Unmade>()
            .iter(app.world())
            .filter_map(|unmade| match *unmade {
                Unmade::Burning { seconds, .. } => Some(seconds),
                Unmade::Conversion { .. } => None,
            })
            .collect()
    }

    #[test]
    fn a_knock_back_burns_3_s_and_a_throw_leaves_its_light_to_its_thrower() {
        let mut app = App::new();
        app.add_observer(knocked);
        let by = app.world_mut().spawn_empty().id();
        let knock = |throw| Knocked {
            by,
            from: Vec3::ZERO,
            left: 1_000.0,
            held: 0.0,
            throw,
            unheld: false,
        };
        app.world_mut().spawn(knock(None));
        app.world_mut().flush();
        assert_eq!(lights(&mut app), [BURNING_KNOCKED]);
        app.world_mut()
            .spawn(knock(Some(opensacred_rules::arts::Flight::new(
                [10.0, 0.0],
                80.0,
            ))));
        app.world_mut().flush();
        assert_eq!(lights(&mut app), [BURNING_KNOCKED]);
    }
}
