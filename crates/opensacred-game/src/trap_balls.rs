//! The look of the thrown balls (867, `FX_BALL`, #1092; docs/re/effects.md, "The thrown
//! balls"): each ball of the Dark Elf's traps and of the goblin mages' spell 53 shows its bottle
//! tumbling as it flies and leaves a trail of glows behind it, added in its variant's colour and
//! untinted by the night (`opensacred_rules::particles::TrapBall`); its ball landed, its trail
//! fades out and its bottle goes with its last glow.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, TrapBall};

use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::spawning::Rng;
use crate::{iso, models as model_looks};

/// A thrown ball's variant, the art or the spell that throws it (the event's `+0x3C`), which
/// colours its glows; its look follows it ([`show`]).
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Thrown(pub(crate) u16);

/// The balls' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x867))
    }
}

/// What the looks take, found at the first ball: its model, relative to the asset directory, and
/// its glows' material.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    model: Option<String>,
    glows: Option<Handle<Effect>>,
}

/// A ball's look, which outlives its ball while its glows fade: its ball while it flies, its
/// system, its model, its place and the place before, and its glows' mesh.
#[derive(Component)]
struct Shown {
    ball: Option<Entity>,
    look: TrapBall,
    model: Option<Entity>,
    place: Vec3,
    before: Option<Vec3>,
    mesh: Handle<Mesh>,
}

/// Gives each new ball its look (`0x79E630`, `0x79EA50`): its model and its glows.
fn show(
    mut commands: Commands,
    balls: Query<(Entity, &Thrown, &Transform), Added<Thrown>>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes, mut dice): (
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
        ResMut<Dice>,
    ),
) {
    for (entity, thrown, place) in &balls {
        let Some(colour) = particles::ball_colour(thrown.0) else {
            continue;
        };
        if !looks.made {
            looks.made = true;
            if let Some(data) = data.as_deref() {
                looks.model = crate::npcs::model_files(&data.0.assets)
                    .ok()
                    .and_then(|files| files.get(particles::BALL_MODEL).cloned());
                if looks.model.is_none() {
                    warn!(
                        "thrown balls: no {}; their bottles are not drawn",
                        particles::BALL_MODEL
                    );
                }
                let image = particle_texture(&data.0.assets, &assets, particles::BALL_TEXTURE);
                if image.is_none() {
                    warn!(
                        "thrown balls: no texture {}; their glows are not drawn",
                        particles::BALL_TEXTURE
                    );
                }
                looks.glows =
                    image.map(|image| effects.add(particle_material(image, AlphaMode::Add)));
            }
        }
        let model = looks.model.as_ref().map(|model| {
            commands
                .spawn((
                    model_looks::scene(&assets, model),
                    Transform::from_translation(place.translation)
                        .with_scale(Vec3::splat(iso::MODEL_SCALE)),
                    model_looks::Ordered::default(),
                    DespawnOnExit(AppState::InGame),
                ))
                .id()
        });
        let mesh = meshes.add(empty());
        let mut shown = commands.spawn((
            Transform::IDENTITY,
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(material) = looks.glows.clone() {
            shown.with_child((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
            ));
        }
        shown.insert(Shown {
            ball: Some(entity),
            look: TrapBall::new(colour, &mut dice.0),
            model,
            place: place.translation,
            before: None,
            mesh,
        });
    }
}

/// Runs each ball's look a frame on (`0x79E730`, `0x79E900`): its bottle at its place, tumbling
/// about the models' x and then y, and its glows; it goes with its last glow.
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown)>,
    balls: Query<&Transform, With<Thrown>>,
    mut transforms: Query<&mut Transform, Without<Thrown>>,
    (mut dice, mut meshes, orders): (ResMut<Dice>, ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let (across, down) = iso::fx_axes();
    for (entity, mut look) in &mut shown {
        let look = &mut *look;
        match look.ball.map(|ball| balls.get(ball)) {
            Some(Ok(place)) => look.place = place.translation,
            _ => {
                if look.ball.take().is_some() {
                    look.look.end();
                }
            }
        }
        let before = look.before.unwrap_or(look.place);
        look.look
            .update(dt, (models(before), models(look.place)), &mut dice.0);
        look.before = Some(look.place);
        if look.look.done() {
            if let Some(model) = look.model {
                commands.entity(model).try_despawn();
            }
            commands.entity(entity).despawn();
            continue;
        }
        if let Some(mut at) = look.model.and_then(|model| transforms.get_mut(model).ok()) {
            let [x, y] = look.look.turns;
            at.translation = look.place;
            at.rotation = Quat::from_axis_angle(down, y) * Quat::from_axis_angle(across, x);
        }
        let mut quads = Quads::default();
        for glow in &look.look.system.particles {
            quads.push(
                world(glow.place),
                glow.size * s,
                0.0,
                (Vec2::ZERO, 1.0),
                look.look.glow_colour(glow.life),
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&look.mesh) {
            // At its object's place in the order, which a moving object heads in its cell (#2188).
            quads.in_order(orders.at(look.place), Vec3::ZERO);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The thrown balls' looks.
pub struct TrapBallsPlugin;

impl Plugin for TrapBallsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (show, fly)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
