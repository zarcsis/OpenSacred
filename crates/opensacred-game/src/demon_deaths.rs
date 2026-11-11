//! The big demons' death (919, `FX_BIG_DEMON_DEATH`, #2537; docs/re/effects.md, "The big demons'
//! death"): at the body of a big demon (53, 94) at its first tick of lying, in place of a blood
//! puddle (`bodies`), a dim ball of glows that swells over 2 s and pulses, and ten writhing rays of
//! `FX_GHOST2` that shoot from it in turn for some 13 s, all added whole, then fading away over
//! 3.3 s; its light in the world's table as it grows and fades
//! (`opensacred_rules::particles::DemonDeath`). Untinted by the night (**hypothesis**: the
//! system's flag `0x40` is not read for the glows and the rays).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    DEMON_DEATH_GLOW, DEMON_DEATH_GLOW_COLOUR, DEMON_DEATH_RAYS, DEMON_DEATH_TEXTURE, DemonDeath,
};

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::spawning::Rng;
use crate::{daynight, iso};

/// A big demon's death to make at its place ([`make`]).
#[derive(Component)]
pub(crate) struct Unmade;

/// Makes a big demon's death at the place `at` of its body (`0x53FE31`, the object 919 by the FX
/// event `0x7F`).
pub(crate) fn spawn(commands: &mut Commands, at: Vec3) {
    commands.spawn((
        Unmade,
        Transform::from_translation(at),
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
}

/// A death that shows: its system, its glows' and its rays' meshes, and its light.
#[derive(Component)]
struct Shown {
    death: DemonDeath,
    glows: Handle<Mesh>,
    rays: Handle<Mesh>,
    light: Entity,
}

/// The glows' and the rays' materials, added whole, made at the first death; none where a texture
/// is missing.
#[derive(Resource, Default)]
struct Looks(Option<[Option<Handle<Effect>>; 2]>);

/// The deaths' dice (`rand()`).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x919))
    }
}

/// Gives each new death its system (`0x7A9D30`), its meshes and materials and its light.
fn make(
    mut commands: Commands,
    unmade: Query<Entity, With<Unmade>>,
    (data, assets, mut looks, mut dice): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Dice>,
    ),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for entity in &unmade {
        let materials = looks
            .0
            .get_or_insert_with(|| {
                [DEMON_DEATH_GLOW, DEMON_DEATH_TEXTURE].map(|name| {
                    let image = data
                        .as_deref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("demon deaths: no texture {name}; its part is not drawn");
                    }
                    image.map(|image| effects.add(whole_material(image)))
                })
            })
            .clone();
        let [glows, rays] = [meshes.add(empty()), meshes.add(empty())];
        for (mesh, material) in [glows.clone(), rays.clone()].into_iter().zip(materials) {
            if let Some(material) = material {
                commands.spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                    ChildOf(entity),
                ));
            }
        }
        // Its light in the world's table, on the ground at its place (`+0x9C`).
        let light = commands
            .spawn((daynight::Light(0.0), Transform::IDENTITY, ChildOf(entity)))
            .id();
        debug!("demon deaths: {entity} starts");
        commands.entity(entity).remove::<Unmade>().insert(Shown {
            death: DemonDeath::new(&mut dice.0),
            glows,
            rays,
            light,
        });
    }
}

/// Runs each death a frame on (`0x7A9610`) and draws it (`0x7A97F0`): its ball's glows facing the
/// viewer at their slices, its rays' ribbons, its texture's `u` 0 on one edge and 1 on the other
/// and its `v` the point's eighth along, all at its place in the order; its light; it goes once
/// faded.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &GlobalTransform)>,
    mut lights: Query<&mut daynight::Light>,
    (mut meshes, orders, mut dice): (ResMut<Assets<Mesh>>, Orders, ResMut<Dice>),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut look, place) in &mut shown {
        let look = &mut *look;
        look.death.update(dt, &mut dice.0);
        if look.death.done() {
            debug!("demon deaths: {entity} ends");
            commands.entity(entity).despawn();
            continue;
        }
        if let Ok(mut light) = lights.get_mut(look.light) {
            light.0 = look.death.light();
        }
        let mut glows = Quads::default();
        for (offset, half) in look.death.glows() {
            glows.push(
                world(offset),
                half * s,
                0.0,
                (Vec2::ZERO, 1.0),
                DEMON_DEATH_GLOW_COLOUR,
            );
        }
        let mut rays = Quads::default();
        for k in 0..DEMON_DEATH_RAYS {
            let ribbon = look.death.ribbon(k);
            for pair in ribbon.windows(2) {
                let ([near_a, near_b], v0) = pair[0];
                let ([far_a, far_b], v1) = pair[1];
                rays.push_corners(
                    [world(near_a), world(near_b), world(far_b), world(far_a)],
                    [
                        Vec2::new(0.0, v0),
                        Vec2::new(1.0, v0),
                        Vec2::new(1.0, v1),
                        Vec2::new(0.0, v1),
                    ],
                    [1.0; 4],
                );
            }
        }
        // At its object's place in the order, as the original draws the system (#2188).
        let origin = place.translation();
        let order = orders.at(origin);
        for (handle, mut part) in [(&look.glows, glows), (&look.rays, rays)] {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.in_order(order, origin);
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The big demons' deaths.
pub struct DemonDeathsPlugin;

impl Plugin for DemonDeathsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>()
            .init_resource::<Dice>()
            .add_systems(OnExit(AppState::InGame), |mut looks: ResMut<Looks>| {
                looks.0 = None;
            })
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
