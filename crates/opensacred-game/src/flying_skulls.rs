//! The look of the death's kiss's flying skull (905, `FX_FLYINGSKULL`, #1092; docs/re/effects.md,
//! "The flying skull"): each skull `spells` sends shows its model at twice its size, turned to its
//! heading, at a point that wobbles about it, and leaves a trail of orange glows behind it, added
//! whole and untinted by the night (`opensacred_rules::particles::FlyingSkull`); its skull gone,
//! its model goes and its trail fades out.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, FlyingSkull, skull_turn};

use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::spawning::Rng;
use crate::spells::Skull;
use crate::{iso, models as model_looks};

/// The trail's dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x905))
    }
}

/// What the looks take, found at the first skull: its model, relative to the asset directory, and
/// its trail's material.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    model: Option<String>,
    trail: Option<Handle<Effect>>,
}

/// A skull's look, which outlives its skull while its trail fades: its skull while it flies, its
/// system, its model, its place and the place before, and its trail's mesh.
#[derive(Component)]
struct Shown {
    skull: Option<Entity>,
    look: FlyingSkull,
    model: Option<Entity>,
    place: Vec3,
    before: Option<Vec3>,
    mesh: Handle<Mesh>,
}

/// Gives each new skull its look: its model and its trail (`0x7A6370`).
fn show(
    mut commands: Commands,
    skulls: Query<(Entity, &Skull, &Transform), Added<Skull>>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, skull, place) in &skulls {
        if !looks.made {
            looks.made = true;
            if let Some(data) = data.as_deref() {
                looks.model = crate::npcs::model_files(&data.0.assets)
                    .ok()
                    .and_then(|files| files.get(particles::SKULL_MODEL).cloned());
                if looks.model.is_none() {
                    warn!(
                        "flying skulls: no {}; the skulls are not drawn",
                        particles::SKULL_MODEL
                    );
                }
                let image =
                    particle_texture(&data.0.assets, &assets, particles::SKULL_TRAIL_TEXTURE);
                if image.is_none() {
                    warn!(
                        "flying skulls: no texture {}; their trails are not drawn",
                        particles::SKULL_TRAIL_TEXTURE
                    );
                }
                looks.trail = image.map(|image| effects.add(whole_material(image)));
            }
        }
        let model = looks.model.as_ref().map(|model| {
            commands
                .spawn((
                    model_looks::scene(&assets, model),
                    Transform::from_translation(place.translation)
                        .with_scale(Vec3::splat(particles::SKULL_SCALE * iso::MODEL_SCALE)),
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
        if let Some(material) = looks.trail.clone() {
            shown.with_child((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
            ));
        }
        shown.insert(Shown {
            skull: Some(entity),
            look: FlyingSkull::new(skull.seconds()),
            model,
            place: place.translation,
            before: None,
            mesh,
        });
    }
}

/// Runs each skull's look a frame on (`0x7A5F40`, `0x7A6040`): its model at its wobbling point,
/// turned to its heading, while it flies, and its trail; it goes with its last glow.
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown)>,
    skulls: Query<(&Skull, &Transform)>,
    mut transforms: Query<&mut Transform, Without<Skull>>,
    (mut dice, mut meshes): (ResMut<Dice>, ResMut<Assets<Mesh>>),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let ramp = FlyingSkull::ramp();
    for (entity, mut look) in &mut shown {
        let look = &mut *look;
        let heading = match look.skull.map(|skull| skulls.get(skull)) {
            Some(Ok((skull, place))) => {
                look.place = place.translation;
                Some(skull.heading())
            }
            _ => {
                if look.skull.take().is_some() {
                    look.look.end();
                }
                None
            }
        };
        let before = look.before.unwrap_or(look.place);
        look.look
            .update(dt, (models(before), models(look.place)), &mut dice.0);
        look.before = Some(look.place);
        match (look.look.flies(), look.model) {
            (Some(wobble), Some(model)) => {
                if let Ok(mut at) = transforms.get_mut(model) {
                    at.translation = look.place + world(wobble);
                    if let Some(heading) = heading {
                        let [x, y, _] = models(heading);
                        at.rotation = Quat::from_rotation_y(skull_turn([x, y]));
                    }
                }
            }
            (None, Some(model)) => {
                commands.entity(model).try_despawn();
                look.model = None;
            }
            _ => {}
        }
        if look.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut quads = Quads::default();
        for glow in &look.look.system.particles {
            quads.push(
                world(glow.place),
                glow.size * s,
                0.0,
                (Vec2::ZERO, 1.0),
                ramp.at(glow.life),
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&look.mesh) {
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The flying skulls' looks.
pub struct FlyingSkullsPlugin;

impl Plugin for FlyingSkullsPlugin {
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
