//! The holds' sights (#1092; docs/re/effects.md, "The burning light and the holy conversion"): the
//! burning light (816) about a creature that the Seraphim's irritation holds or that a blow knocks
//! back, pale blue sparks bursting now and then about its neck and falling, left in the world,
//! for the hold's seconds or 3 s (`opensacred_rules::particles::BurningLight`); and the holy
//! conversion's halo (806) at a converted creature's head, forty small blue-violet glows on a ring
//! that swells away as the conversion ends (`particles::HolyConversion`). Both are added and
//! untinted by the night.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    BURNING_KNOCKED, BURNING_TEXTURE, BurningLight, CONVERSION_TEXTURE, HolyConversion,
};

use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture, whole_material};
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

/// Every knock-back burns its creature 3 s, each its own (the creature's event 3, `0x531BB6`).
fn knocked(added: On<Insert<crate::making::Knocked>>, mut commands: Commands) {
    burn(&mut commands, added.entity, BURNING_KNOCKED);
}

/// Gives each new sight its look at its creature, its mesh and its material (`0x77FA60`,
/// `0x782EF0`).
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    (places, sizes): (Query<&GlobalTransform>, Query<&crate::arrows::Size>),
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, unmade) in &unmade {
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
        made.remove::<Unmade>().insert((
            Transform::from_translation(at.translation()),
            Visibility::default(),
        ));
        let material = match *unmade {
            Unmade::Burning { target, seconds } => {
                let size = sizes.get(target).map_or(0.0, |size| size.0);
                made.insert(Burning {
                    target,
                    look: BurningLight::new(seconds, size),
                    mesh: mesh.clone(),
                });
                sparks
            }
            Unmade::Conversion { target, seconds } => {
                made.insert(Halo {
                    target,
                    look: HolyConversion::new(seconds),
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
/// else its place, its sparks facing the camera, turned, of their ramp's colour; it goes once it
/// is over, or with its creature.
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
            quads.into_mesh(&mut mesh);
        }
    }
}

/// Runs each halo a frame on (`0x782A60`, `0x782AD0`): its glows facing the camera about its
/// creature's head in the head bone's axes (the original's bone `y` the glTF's `z`); it goes once
/// it has faded, or with its creature.
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
