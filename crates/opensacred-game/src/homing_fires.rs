//! The look of the creatures' homing fires (#1092; docs/re/effects.md, "The homing fires"): the
//! big demons' (910) and Anducar's guards' (1164) flames burning at the fire, in its caster's
//! hand as it charges and left in the world behind it as it flies, added and untinted by the
//! night (`opensacred_rules::particles::HomingFlames`); at its burst on the hero or at a cell
//! that stops it, a ball of flames; its flight over, its last flames burn out.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, HomingFlames, HomingKind};

use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;

/// A homing fire's kind, which takes its look ([`show`]).
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Flames(pub(crate) HomingKind);

/// A homing fire's burst at a point of the world, on the hero or at a cell that stops it
/// (`creature_casts::home`).
#[derive(Message, Debug, Clone, Copy)]
pub(crate) struct Bursts {
    pub(crate) fire: Entity,
    pub(crate) at: Vec3,
}

/// The flames' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x910))
    }
}

/// The flames' material, made at the first fire; none where its texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<Option<Handle<Effect>>>);

/// A fire's look, which outlives its fire while its flames burn: its fire while it flies, its
/// flames, its place and the place before, and its mesh.
#[derive(Component)]
struct Shown {
    fire: Option<Entity>,
    flames: HomingFlames,
    place: Vec3,
    before: Option<Vec3>,
    mesh: Handle<Mesh>,
}

/// Gives each new fire its look (`0x7A6A40`, `0x7BF690`).
fn show(
    mut commands: Commands,
    fires: Query<(Entity, &Flames, &Transform), Added<Flames>>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, flames, place) in &fires {
        let material = looks
            .0
            .get_or_insert_with(|| {
                let image = data.as_deref().and_then(|data| {
                    particle_texture(&data.0.assets, &assets, particles::HOMING_FIRE_TEXTURE)
                });
                if image.is_none() {
                    warn!(
                        "homing fires: no texture {}; their flames are not drawn",
                        particles::HOMING_FIRE_TEXTURE
                    );
                }
                image.map(|image| effects.add(particle_material(image, AlphaMode::Add)))
            })
            .clone();
        let mesh = meshes.add(empty());
        let mut shown = commands.spawn((
            Transform::IDENTITY,
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(material) = material {
            shown.with_child((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
            ));
        }
        shown.insert(Shown {
            fire: Some(entity),
            flames: HomingFlames::new(flames.0),
            place: place.translation,
            before: None,
            mesh,
        });
    }
}

/// Runs each fire's look a frame on (`0x7A6AD0`, `0x7A6D30`): its flames along its way, its
/// burst where it bursts, and its end with its last flame.
fn burn(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown)>,
    fires: Query<&Transform, With<Flames>>,
    mut bursts: MessageReader<Bursts>,
    (mut dice, mut meshes, orders): (ResMut<Dice>, ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let ramp = HomingFlames::ramp();
    let burst: Vec<Bursts> = bursts.read().copied().collect();
    for (entity, mut look) in &mut shown {
        let look = &mut *look;
        match look.fire.map(|fire| fires.get(fire)) {
            Some(Ok(place)) => look.place = place.translation,
            _ => {
                if let Some(fire) = look.fire.take() {
                    match burst.iter().find(|burst| burst.fire == fire) {
                        Some(burst) => {
                            debug!("homing fires: {fire} bursts at {}", burst.at);
                            look.flames.burst(models(burst.at), &mut dice.0);
                        }
                        None => look.flames.end(),
                    }
                }
            }
        }
        let before = look.before.unwrap_or(look.place);
        look.flames
            .update(dt, (models(before), models(look.place)), &mut dice.0);
        look.before = Some(look.place);
        if look.flames.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut quads = Quads::default();
        for flame in &look.flames.system.particles {
            quads.push(
                world(flame.place),
                flame.size * s,
                flame.angle,
                (Vec2::ZERO, 1.0),
                ramp.at(flame.life),
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&look.mesh) {
            // At its object's place in the order, which a moving object heads in its cell (#2188).
            quads.in_order(orders.at(look.place), Vec3::ZERO);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The homing fires' looks.
pub struct HomingFiresPlugin;

impl Plugin for HomingFiresPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Bursts>()
            .init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (show, burn)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
