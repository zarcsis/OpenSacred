//! The teleport's sights (774 `FX_TELEPORT_OUT`, 775 `FX_TELEPORT_IN`, #1092; docs/re/effects.md,
//! "The teleport"): as the Mage's teleport acts, or a creature's that a script's `Attack` orders
//! (`script_attacks`, #1831), a ring of blue sparks about his feet and another at the arrival,
//! each opening, rising into a cylinder, putting him at the arrival as it holds (`spells`'s
//! `Teleporting` or `script_attacks`' `Phase`, at the same 3.35 s over the cast's rate), then
//! sinking and closing; the departure's rides on him until then and stays where he was, the
//! arrival's rides on him from then (`opensacred_rules::particles::TeleportSight`). Each lights
//! the night by 170; none is tinted by it.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    TELEPORT_CORNERS, TELEPORT_LIGHT, TELEPORT_TEXTURE, TeleportSight,
};

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;

/// A sight to make at its entity's place ([`make`]): its caster, whether it is the departure's,
/// and the cast's rate.
#[derive(Component)]
struct Unmade {
    caster: Entity,
    departure: bool,
    rate: f32,
}

/// A sight that shows: its caster, whether it rides on him now, its look and its mesh.
#[derive(Component)]
struct Shown {
    caster: Entity,
    rides: bool,
    departure: bool,
    look: TeleportSight,
    mesh: Handle<Mesh>,
}

/// A teleport's sight as a save keeps it (#3778): its caster by its name, whether at the
/// departure, and its look.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename = "SavedTeleport")]
pub struct SavedLook {
    #[serde(with = "crate::names::required")]
    caster: Entity,
    departure: bool,
    look: TeleportSight,
}

impl bevy::ecs::entity::MapEntities for SavedLook {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.caster = mapper.get_mapped(self.caster);
    }
}

/// The looks in `world` as a save keeps them (#3778).
pub(crate) fn saved(world: &mut World) -> Vec<SavedLook> {
    let mut shown = world.query::<&Shown>();
    shown
        .iter(world)
        .map(|shown| SavedLook {
            caster: shown.caster,
            departure: shown.departure,
            look: shown.look.clone(),
        })
        .collect()
}

/// Makes a loaded save's looks again (#3778), each going on as it went; one of a creature the
/// load finds not is left out.
pub(crate) fn again(world: &mut World, looks: Vec<SavedLook>) {
    for saved in looks {
        if crate::names::found(Some(saved.caster)).is_none() {
            continue;
        }
        world.spawn((
            Unmade {
                caster: saved.caster,
                departure: saved.departure,
                rate: 1.0,
            },
            crate::fx::Resumes(saved.look),
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// The sparks' material, made as first asked; none where its texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<Option<Handle<Effect>>>);

/// The sights' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x774))
    }
}

/// Starts the teleport's two sights: at `caster`'s feet `from` and at the arrival `to`, at the
/// cast's rate (`0x522B28`, `0x522C00`).
pub(crate) fn spawn(commands: &mut Commands, caster: Entity, (from, to): (Vec3, Vec3), rate: f32) {
    for (at, departure) in [(from, true), (to, false)] {
        commands.spawn((
            Unmade {
                caster,
                departure,
                rate,
            },
            crate::daynight::Light(TELEPORT_LIGHT),
            Transform::from_translation(at),
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// Gives each new sight its sparks, its mesh and its material (`0x771370`, `0x771E20`).
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade, Option<&crate::fx::Resumes<TeleportSight>>)>,
    (data, assets, mut looks, mut dice): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Dice>,
    ),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, unmade, resumed) in &unmade {
        let material = looks
            .0
            .get_or_insert_with(|| {
                let image = data
                    .as_ref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, TELEPORT_TEXTURE));
                if image.is_none() {
                    warn!("teleports: no texture {TELEPORT_TEXTURE}; the teleports are not drawn");
                }
                image.map(|image| effects.add(particle_material(image, AlphaMode::Add)))
            })
            .clone();
        let mesh = meshes.add(empty());
        let mut shown = commands.entity(entity);
        shown
            .remove::<(Unmade, crate::fx::Resumes<TeleportSight>)>()
            .insert(Shown {
                caster: unmade.caster,
                rides: unmade.departure,
                departure: unmade.departure,
                look: resumed.map_or_else(
                    || TeleportSight::new(unmade.rate, &mut dice.0),
                    |crate::fx::Resumes(look)| look.clone(),
                ),
                mesh: mesh.clone(),
            });
        if let Some(material) = material {
            shown.with_child((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
            ));
        }
    }
}

/// Runs each sight a frame on (`0x771410`, `0x771C10`): on its caster's feet while it rides,
/// the departure's left where he was and the arrival's riding on him from his arrival, its sparks
/// facing the camera in its blue corners; it goes once it is closed.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut sights: Query<(Entity, &mut Shown, &mut Transform)>,
    places: Query<&GlobalTransform>,
    (mut meshes, mut dice): (ResMut<Assets<Mesh>>, ResMut<Dice>),
    orders: crate::fx::Orders,
) {
    let dt = time.delta_secs();
    for (entity, mut shown, mut transform) in &mut sights {
        let shown = &mut *shown;
        if shown.rides
            && let Ok(at) = places.get(shown.caster)
        {
            transform.translation = at.translation();
        }
        if shown.look.update(dt, 0.0, &mut dice.0) {
            shown.rides = !shown.departure;
        }
        if shown.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut quads = Quads::default();
        for (at, half, turn) in shown.look.sparks(&mut dice.0) {
            quads.push_facing_turned(world(at), half * iso::MODEL_SCALE, turn, TELEPORT_CORNERS);
        }
        if let Some(mut mesh) = meshes.get_mut(&shown.mesh) {
            // Just behind its caster while it rides on him, else at its place, as the original
            // draws the system before the creature (#2188).
            let order = if shown.rides {
                orders.borne_by(shown.caster)
            } else {
                orders.at(transform.translation)
            };
            quads.in_order(order, transform.translation);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The teleport's sights.
pub struct TeleportsPlugin;

impl Plugin for TeleportsPlugin {
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
