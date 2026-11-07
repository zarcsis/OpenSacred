//! The look of the Dwarf's greed (55, 1155, `cParticleSystem_dwarf_gier`; #1092;
//! docs/re/effects.md, "The Dwarf's greed"): for the spell's seconds tiny sparks about his neck,
//! brightening from black to gold and fading over 0.32 s, each with a white core, and a gold light
//! leaping about them every 0.2 s, flaring up and out each time (`fx_lights`); added whole,
//! untinted by the night, drawn just behind his model (`fx::Orders`, #2178)
//! (`opensacred_rules::particles::Greed`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{GREED_BONE, GREED_CORE, GREED_LIGHT, GREED_TEXTURE, Greed};

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, particle_texture, whole_material};
use crate::fx_lights::FxLight;
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;

/// A greed to make: its bearer and its seconds.
#[derive(Component)]
struct Unmade {
    bearer: Entity,
    seconds: f32,
}

/// A greed that shows: its bearer, its look, its light and the meshes of its sparks and their
/// cores.
#[derive(Component)]
struct Shown {
    bearer: Entity,
    look: Greed,
    light: Entity,
    meshes: [Handle<Mesh>; 2],
}

/// The greeds' material, `PARTICLE_SPARK07` added whole, made as the first asks; `None` where the
/// texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<Option<Handle<Effect>>>);

/// The greeds' dice (`rand()`).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x1155))
    }
}

/// Shows the greed at `bearer` for `seconds`, the spell's value 0 at its level (`0x528A9C`).
pub(crate) fn show(commands: &mut Commands, bearer: Entity, seconds: f32) {
    commands.spawn((Unmade { bearer, seconds }, DespawnOnExit(AppState::InGame)));
}

/// Gives each new greed its meshes, its material and its light (`0x7BB540`, `0x7BB8D0`).
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    places: Query<&GlobalTransform>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, unmade) in &unmade {
        let Ok(at) = places.get(unmade.bearer).map(GlobalTransform::translation) else {
            commands.entity(entity).despawn();
            continue;
        };
        let material = looks
            .0
            .get_or_insert_with(|| {
                let image = data
                    .as_deref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, GREED_TEXTURE));
                if image.is_none() {
                    warn!("dwarf greeds: no texture {GREED_TEXTURE}; its sparks are not drawn");
                }
                image.map(|image| effects.add(whole_material(image)))
            })
            .clone();
        let light = commands
            .spawn(crate::fx_lights::child(GREED_LIGHT, entity))
            .id();
        let parts: [Handle<Mesh>; 2] = std::array::from_fn(|_| meshes.add(empty()));
        let mut made = commands.entity(entity);
        made.remove::<Unmade>().insert((
            Shown {
                bearer: unmade.bearer,
                look: Greed::new(unmade.seconds),
                light,
                meshes: parts.clone(),
            },
            Transform::from_translation(at),
            Visibility::default(),
        ));
        if let Some(material) = material {
            for mesh in parts {
                made.with_child((
                    Mesh3d(mesh),
                    MeshMaterial3d(material.clone()),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                ));
            }
        }
    }
}

/// Runs each greed a frame on (`0x7BB620`) and draws it (`0x7BB830`) at its bearer's neck, else
/// his place: its sparks in gold, then their white cores at 0.4 of their size, all facing the
/// camera, riding with the neck; its light at its point in its flare; it goes with its last spark,
/// or with its bearer.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &mut Transform)>,
    (places, children, bones, worn): (
        Query<&GlobalTransform>,
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    mut lights: Query<(&mut FxLight, &mut Transform), Without<Shown>>,
    (mut meshes, mut dice): (ResMut<Assets<Mesh>>, ResMut<Dice>),
    orders: Orders,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let whole = (Vec2::ZERO, 1.0);
    for (entity, mut greed, mut transform) in &mut shown {
        let greed = &mut *greed;
        let Ok(place) = places.get(greed.bearer).map(GlobalTransform::translation) else {
            commands.entity(entity).despawn();
            continue;
        };
        greed.look.update(dt, &mut dice.0);
        if greed.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation =
            crate::making::bone(greed.bearer, &[GREED_BONE], &children, (&bones, &worn))
                .unwrap_or(place);
        if let Ok((mut light, mut at)) = lights.get_mut(greed.light) {
            light.colour = greed.look.light_colour();
            at.translation = world(greed.look.light_point());
        }
        let mut parts: [Quads; 2] = std::array::from_fn(|_| Quads::default());
        for spark in &greed.look.sparks.particles {
            for (core, (part, share)) in parts.iter_mut().zip([1.0, GREED_CORE]).enumerate() {
                part.push(
                    world(spark.place),
                    spark.size * share * s,
                    0.0,
                    whole,
                    greed.look.colour(spark.life, core == 1),
                );
            }
        }
        let order = orders.behind(greed.bearer);
        for (handle, mut part) in greed.meshes.iter().zip(parts) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.in_order(order, transform.translation);
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The Dwarf's greed's looks.
pub struct DwarfGreedsPlugin;

impl Plugin for DwarfGreedsPlugin {
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
