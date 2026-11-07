//! The look of the light 20 (#1092; docs/re/effects.md, "The light"): on its bearer for its
//! seconds (`spells`), `cParticleSystem_maglite` draws (817, `0x784E90`) a ball of light of 21
//! flat slices of `PARTICLE_GLOW03` added by their alpha (`opensacred_rules::particles::
//! maglite_slices`) about its bearer's head (the FX event `0x2C`'s bone 20, kept on it at each
//! frame by the flag `0x200`), untinted by the night, and lights the night about it by the
//! spell's second value at its level. The ball lies just behind its bearer's model, in the
//! original's order of drawing (`fx::Orders::behind`, #2220).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, maglite_slices};

use crate::frame::Playing;
use crate::fx::{Effect, Orders, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::{daynight, iso};

/// The bone the ball keeps to (bone 20).
const HEAD: [&str; 1] = ["Bip01 Head"];

/// A light's object to give its look: the radius of its light and its bearer
/// (`spells::strikes_about`).
#[derive(Component)]
pub(crate) struct Look {
    pub light: f32,
    pub bearer: Option<Entity>,
}

/// A light's look: its bearer, its ball's entity and the ball's mesh.
#[derive(Component)]
struct Shown {
    bearer: Option<Entity>,
    ball: Entity,
    mesh: Handle<Mesh>,
}

/// A light's lamp, of the object it goes with.
#[derive(Component)]
struct Lamp(Entity);

/// The ball's material, made as the first light shows; none where its texture is missing.
#[derive(Resource, Default)]
struct Made(Option<Option<Handle<Effect>>>);

/// Gives each new light its ball and its lamp (`0x7850D0`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn show(
    mut commands: Commands,
    looks: Query<(Entity, &Look, &Transform), Added<Look>>,
    (data, assets, mut made, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Made>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (entity, look, transform) in &looks {
        let material = made
            .0
            .get_or_insert_with(|| {
                let image = data.as_ref().and_then(|data| {
                    particle_texture(&data.0.assets, &assets, particles::MAGLITE_TEXTURE)
                });
                if image.is_none() {
                    warn!(
                        "magic lights: no texture {}; the light is not drawn",
                        particles::MAGLITE_TEXTURE
                    );
                }
                image.map(|image| materials.add(particle_material(image, AlphaMode::Add)))
            })
            .clone();
        let mesh = meshes.add(empty());
        let mut ball = commands.spawn((
            Transform::IDENTITY,
            Visibility::default(),
            NoFrustumCulling,
            RenderLayers::layer(iso::UNTINTED),
            ChildOf(entity),
        ));
        if let Some(material) = material {
            ball.insert((Mesh3d(mesh.clone()), MeshMaterial3d(material)));
        }
        let ball = ball.id();
        commands.entity(entity).insert(Shown {
            bearer: look.bearer,
            ball,
            mesh,
        });
        debug!(
            "magic lights: {entity} at {:.0},{:.0} on {:?}, its light {}",
            transform.translation.x, transform.translation.z, look.bearer, look.light
        );
        if look.light > 0.0 {
            commands.spawn((
                Transform::from_translation(transform.translation.with_y(0.0)),
                daynight::Light(look.light),
                Lamp(entity),
                DespawnOnExit(AppState::InGame),
            ));
        }
    }
}

/// The ball: its slices flat in the systems' frame about its centre, at the world's scale.
fn ball() -> Quads {
    let s = iso::MODEL_SCALE;
    let (across, down) = iso::fx_axes();
    let mut quads = Quads::default();
    for (height, half) in maglite_slices() {
        let at = |x: f32, y: f32| (across * x * half + down * y * half + Vec3::Y * height) * s;
        quads.push_corners(
            [at(-1.0, -1.0), at(1.0, -1.0), at(1.0, 1.0), at(-1.0, 1.0)],
            [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
            particles::MAGLITE_COLOUR,
        );
    }
    quads
}

/// Keeps each light's ball at its bearer's head (`0x5AA480`), its object at the bearer's place,
/// and lays the ball just behind its bearer's model in the order, as its system follows the
/// bearer (the flag `0x200`), or at its object's place without one (#2220).
fn keep(
    shown: Query<(&Shown, &GlobalTransform)>,
    mut balls: Query<&mut Transform>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (mut meshes, orders): (ResMut<Assets<Mesh>>, Orders),
) {
    for (shown, place) in &shown {
        let head = shown
            .bearer
            .and_then(|bearer| crate::making::bone(bearer, &HEAD, &children, (&bones, &worn)));
        let Ok(mut at) = balls.get_mut(shown.ball) else {
            continue;
        };
        if let Some(head) = head {
            at.translation = head - place.translation();
        }
        let order = match shown.bearer {
            Some(bearer) => orders.behind(bearer),
            None => orders.at(place.translation()),
        };
        if let Some(mut mesh) = meshes.get_mut(&shown.mesh) {
            let mut quads = ball();
            quads.in_order(order, place.translation() + at.translation);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// Keeps each light's lamp on its object's place on the ground, and puts it out as its object
/// goes.
fn follow(
    mut commands: Commands,
    mut lamps: Query<(Entity, &Lamp, &mut Transform)>,
    shown: Query<&GlobalTransform, With<Shown>>,
) {
    for (lamp, &Lamp(of), mut transform) in &mut lamps {
        match shown.get(of) {
            Ok(place) => transform.translation = place.translation().with_y(0.0),
            Err(_) => commands.entity(lamp).despawn(),
        }
    }
}

/// The lights' looks.
pub struct MagicLightsPlugin;

impl Plugin for MagicLightsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Made>().add_systems(
            Update,
            (show, keep, follow)
                .chain()
                .in_set(Playing)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
