//! The teleport's sights (774 `FX_TELEPORT_OUT`, 775 `FX_TELEPORT_IN`, #1092; docs/re/effects.md,
//! "The teleport"): as the Mage's teleport acts, a ring of blue sparks about his feet and another
//! at the arrival, each opening, rising into a cylinder, putting him at the arrival as it holds
//! (`spells`'s `Teleporting`, at the same 3.35 s over the cast's rate), then sinking and closing;
//! the departure's rides on him until then and stays where he was, the arrival's rides on him
//! from then (`opensacred_rules::particles::TeleportSight`). Each lights the night by 170; none
//! is tinted by it.

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

/// A sight to make ([`make`]): its caster, its place, whether it is the departure's, and the
/// cast's rate.
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
    unmade: Query<(Entity, &Unmade)>,
    (data, assets, mut looks, mut dice): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Dice>,
    ),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, unmade) in &unmade {
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
        shown.remove::<Unmade>().insert(Shown {
            caster: unmade.caster,
            rides: unmade.departure,
            departure: unmade.departure,
            look: TeleportSight::new(unmade.rate, &mut dice.0),
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
        for (at, half) in shown.look.sparks(&mut dice.0) {
            quads.push_facing_turned(world(at), half * iso::MODEL_SCALE, 0.0, TELEPORT_CORNERS);
        }
        if let Some(mut mesh) = meshes.get_mut(&shown.mesh) {
            // Just behind its caster while it rides on him, else at its place, as the original
            // draws the system before the creature (#2188).
            let order = if shown.rides {
                orders.behind(shown.caster)
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
