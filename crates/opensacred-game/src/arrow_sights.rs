//! The Wood Elf's special arrows' sights where they strike (#3129; docs/re/effects.md, "The Wood
//! Elf's arrows' sights"): the club arrow 1007's green ring spreading flat at the struck
//! creature's neck, else its root, for half a second, its light of 200 at the creature
//! (`opensacred_rules::particles::ClubRing`, the object 860 of the event's `+0x3C` 1007); the
//! spider arrow 1006's ten puffs of pale green smoke about the struck creature's left thigh,
//! placed there once, rising away within 1.3 s (`opensacred_rules::particles::spider_smoke`, the
//! object 855 by the FX event `0x4F`, drawn by `puffs`). Both are added and untinted by the
//! night. The explosive arrow's sparks are `hit_sparks`', the trails of 1004's arrow and of 1008's
//! blades `arrows`' (#3131).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    CLUB_RING_LIGHT, CLUB_RING_SEGMENTS, CLUB_RING_TEXTURE, ClubRing, spider_smoke,
};
use opensacred_rules::spawning::Rng;

use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;

/// What a special arrow shows at the creature `on` it strikes: `arrows` sends one at each hit of
/// the club arrow and of the spider arrow.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sight {
    pub look: Look,
    pub on: Entity,
}

/// The club arrow's ring or the spider arrow's smoke.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    Club,
    Spider,
}

/// The ring's bone, the neck (3), else the root (`0x44CB80`); the smoke's, the left thigh (6,
/// the FX event `0x4F`'s, `0x5A51F8`).
const NECK: [&str; 1] = ["Bip01 Neck"];
const THIGH: [&str; 1] = ["Bip01 L Thigh"];

/// The ring's material, made as the first asks, none where its texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<Option<Handle<Effect>>>);

/// The smoke's dice (`rand()`).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x855))
    }
}

/// A club arrow's ring that spreads: its look, its centre and its mesh.
#[derive(Component)]
struct Ring {
    ring: ClubRing,
    centre: Vec3,
    mesh: Handle<Mesh>,
}

/// Makes each arrow's sight at the bone of the creature it struck, else its root.
fn make(
    mut commands: Commands,
    mut sights: MessageReader<Sight>,
    places: Query<&GlobalTransform>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes, mut dice): (
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
        ResMut<Dice>,
    ),
) {
    for sight in sights.read() {
        let names: &[&str] = match sight.look {
            Look::Club => &NECK,
            Look::Spider => &THIGH,
        };
        let Some(point) = crate::making::bone(sight.on, names, &children, (&bones, &worn))
            .or_else(|| places.get(sight.on).ok().map(GlobalTransform::translation))
        else {
            continue;
        };
        debug!("arrow_sights: {:?} at {} ({point})", sight.look, sight.on);
        match sight.look {
            Look::Spider => {
                crate::puffs::spawn(&mut commands, spider_smoke(&mut dice.0), point, None);
            }
            Look::Club => {
                let material = looks
                    .0
                    .get_or_insert_with(|| {
                        let image = data.as_deref().and_then(|data| {
                            particle_texture(&data.0.assets, &assets, CLUB_RING_TEXTURE)
                        });
                        if image.is_none() {
                            warn!("arrow_sights: no texture {CLUB_RING_TEXTURE}; no ring drawn");
                        }
                        image.map(|image| effects.add(whole_material(image)))
                    })
                    .clone();
                let mesh = meshes.add(empty());
                let mut ring = commands.spawn((
                    Ring {
                        ring: ClubRing::default(),
                        centre: point,
                        mesh: mesh.clone(),
                    },
                    Transform::IDENTITY,
                    Visibility::default(),
                    DespawnOnExit(AppState::InGame),
                ));
                // Its light at the creature, on the ground (`+0x9C`).
                ring.with_child((
                    crate::daynight::Light(CLUB_RING_LIGHT),
                    Transform::from_translation(
                        places
                            .get(sight.on)
                            .map_or(point, GlobalTransform::translation)
                            .with_y(0.0),
                    ),
                ));
                // Unlit and untinted, added whole (`0x763530`).
                if let Some(material) = material {
                    ring.with_child((
                        Mesh3d(mesh),
                        MeshMaterial3d(material),
                        Transform::IDENTITY,
                        NoFrustumCulling,
                        RenderLayers::layer(iso::UNTINTED),
                    ));
                }
            }
        }
    }
}

/// Spreads each ring a frame on (`0x7910B0`, `0x7911C4`), and ends it past half a second.
fn spread(
    mut commands: Commands,
    time: Res<Time>,
    mut rings: Query<(Entity, &mut Ring)>,
    (mut meshes, orders): (ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut ring) in &mut rings {
        ring.ring.update(dt);
        if ring.ring.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let (inner, outer) = ring.ring.radii();
        let mut quads = Quads::default();
        quads.push_ring(
            ring.centre,
            (inner * s, outer * s),
            CLUB_RING_SEGMENTS,
            ring.ring.colour(),
        );
        if let Some(mut mesh) = meshes.get_mut(&ring.mesh) {
            // At its object's place in the order, as the original draws the system (#2220).
            quads.in_order(orders.at(ring.centre), Vec3::ZERO);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The special arrows' sights.
pub struct ArrowSightsPlugin;

impl Plugin for ArrowSightsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Sight>()
            .init_resource::<Looks>()
            .init_resource::<Dice>()
            .add_systems(
                Update,
                (make, spread.in_set(Unpaused))
                    .chain()
                    .in_set(Playing)
                    .after(crate::frame::Step::Npcs)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
