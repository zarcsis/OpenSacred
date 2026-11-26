//! The look of the Daemon's choir (#1092; docs/re/effects.md, "The choir"): on each creature it
//! holds (`spells`), `cParticleSystem_songsfromhell` (1011, `0x7B0110`) fades in over a second,
//! then raises black wisps of `PARTICLE_GLOW01` about it over five black discs that beat with its
//! clock, both laid over by their alpha (`opensacred_rules::particles::Choir`); untinted by the
//! night. Its seconds over or its object gone, it makes no more wisps and goes with its last.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, Choir};
use opensacred_rules::spawning::Rng;

use crate::bolt_looks::world;
use crate::frame::Playing;
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;

/// How far over the ground the lowest disc lies, in world units, so that the ground's own faces
/// do not cut it.
const GROUND_LIFT: f32 = 0.5;

/// A choir's object to give its look: the size of the creature it holds and its seconds
/// (`spells`).
#[derive(Component)]
pub(crate) struct Look {
    pub size: f32,
    pub seconds: f32,
}

/// A choir's look: its object while it lives, its system, its dice, its place, and its parts'
/// meshes, the wisps' and the discs'.
#[derive(Component)]
struct Shown {
    object: Option<Entity>,
    choir: Choir,
    dice: Rng,
    place: Vec3,
    parts: [Handle<Mesh>; 2],
}

/// The choirs' material, made as the first choir shows; none where its texture is missing.
#[derive(Resource, Default)]
struct Material(Option<Option<Handle<Effect>>>);

/// A choir's look as a save keeps it with its object (#3775): its size and seconds, and its
/// wisps and discs, dice and place, none for one not shown yet.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedLook {
    size: f32,
    seconds: f32,
    shown: Option<(Choir, Rng, Vec3)>,
}

impl bevy::ecs::entity::MapEntities for SavedLook {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, _: &mut E) {}
}

/// The look of the choir's object `object` as a save keeps it, if it has one.
pub(crate) fn saved_look(world: &World, object: Entity) -> Option<SavedLook> {
    let look = world.get::<Look>(object)?;
    let shown = world.try_query::<&Shown>().and_then(|mut shown| {
        shown
            .iter(world)
            .find(|shown| shown.object == Some(object))
            .map(|shown| (shown.choir.clone(), shown.dice.clone(), shown.place))
    });
    Some(SavedLook {
        size: look.size,
        seconds: look.seconds,
        shown,
    })
}

/// Gives the loaded object `object` its look as the save kept it.
pub(crate) fn look_again(object: &mut EntityWorldMut<'_>, saved: SavedLook) {
    object.insert(Look {
        size: saved.size,
        seconds: saved.seconds,
    });
    if let Some(shown) = saved.shown {
        object.insert(crate::fx::Resumes(shown));
    }
}

/// The seed of the choirs' `rand()` (**estimate**: any).
const SEED: usize = 0x3F3;

/// Gives each new choir its look (`0x7B0200`): an entity of its own and its parts.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn show(
    mut commands: Commands,
    choirs: Query<
        (
            Entity,
            &Look,
            &GlobalTransform,
            Option<&crate::fx::Resumes<(Choir, Rng, Vec3)>>,
        ),
        Added<Look>,
    >,
    (data, assets, mut made, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Material>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (object, look, place, resumed) in &choirs {
        let material = made
            .0
            .get_or_insert_with(|| {
                let image = data.as_ref().and_then(|data| {
                    particle_texture(&data.0.assets, &assets, particles::CHOIR_TEXTURE)
                });
                if image.is_none() {
                    warn!("choirs: no texture {}", particles::CHOIR_TEXTURE);
                }
                image.map(|image| materials.add(particle_material(image, AlphaMode::Blend)))
            })
            .clone();
        let place = place.translation();
        let shown = commands
            .spawn((
                Transform::from_translation(place),
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let parts = [meshes.add(empty()), meshes.add(empty())];
        if let Some(material) = material {
            for mesh in &parts {
                commands.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                    ChildOf(shown),
                ));
            }
        }
        let index = usize::try_from(shown.index_u32()).unwrap_or(0);
        // A loaded look shows on as it showed (#3775).
        let (choir, dice, place) = match resumed {
            Some(crate::fx::Resumes(shown)) => shown.clone(),
            None => (
                Choir::new(look.size, look.seconds),
                crate::seed::dice(SEED ^ index),
                place,
            ),
        };
        commands
            .entity(object)
            .remove::<crate::fx::Resumes<(Choir, Rng, Vec3)>>();
        commands.entity(shown).insert(Shown {
            object: Some(object),
            choir,
            dice,
            place,
            parts,
        });
    }
}

/// Runs each choir's look a frame on (`0x7B0040`, `0x7B0110`): with its creature, its wisps riding
/// with it and its discs under it; then its wisps fade and it goes.
fn sing(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &mut Transform)>,
    objects: Query<&GlobalTransform, With<Look>>,
    mut meshes: ResMut<Assets<Mesh>>,
    orders: crate::fx::Orders,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let (across, depth) = iso::fx_axes();
    for (entity, mut look, mut transform) in &mut shown {
        let look = &mut *look;
        match look.object.map(|object| objects.get(object)) {
            Some(Ok(place)) => look.place = place.translation(),
            _ => {
                if look.object.take().is_some() {
                    look.choir.end();
                }
            }
        }
        look.choir.update(dt, &mut look.dice);
        if look.choir.done() {
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation = look.place;
        let mut wisps = Quads::default();
        for wisp in &look.choir.system.particles {
            wisps.push(
                world(wisp.place),
                wisp.size * s,
                0.0,
                (Vec2::ZERO, 1.0),
                Choir::wisp(wisp.life),
            );
        }
        let mut discs = Quads::default();
        for (height, half) in look.choir.discs() {
            let lift = Vec3::Y * (height * s + GROUND_LIFT);
            let half = half * s;
            discs.push_corners(
                [
                    lift + (-across - depth) * half,
                    lift + (across - depth) * half,
                    lift + (across + depth) * half,
                    lift + (-across + depth) * half,
                ],
                [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
                [0.0, 0.0, 0.0, 1.0],
            );
        }
        // At its object's place in the order, as the original draws the system (#2188).
        let order = orders.at(look.place);
        for (handle, mut part) in look.parts.iter().zip([wisps, discs]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.in_order(order, look.place);
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The choirs' looks.
pub struct ChoirsPlugin;

impl Plugin for ChoirsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Material>().add_systems(
            Update,
            (show, sing)
                .chain()
                .in_set(Playing)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
