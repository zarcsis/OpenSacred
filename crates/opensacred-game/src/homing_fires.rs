//! The look of the creatures' homing fires (#1092; docs/re/effects.md, "The homing fires"): the
//! big demons' (910) and Anducar's guards' (1164) flames burning at the fire, in its caster's
//! hand as it charges and left in the world behind it as it flies, added and untinted by the
//! night (`opensacred_rules::particles::HomingFlames`); at its burst on the hero or at a cell
//! that stops it, a ball of flames; its flight over, its last flames burn out. Its flames drift
//! with the renderer's wind, which its layers take at each frame (`daynight::Clock::wind`,
//! #3154).

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

/// A look carried alone by a load, its fire gone, which [`show`] makes and then lets go
/// (#3779).
#[derive(Component)]
struct Alone;

/// A homing fire's look as a save keeps it (#3779, the epic #3753): its state, its place and the place
/// before. A load gives a flying fire's to its fire made again (`creature_casts::SavedCastObject`), and makes one
/// that outlives its fire again on its own.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename = "SavedHomingFlames")]
pub struct SavedLook {
    flames: HomingFlames,
    place: Vec3,
    before: Option<Vec3>,
}

impl SavedLook {
    /// The look `shown` as a save keeps it.
    fn of(shown: &Shown) -> Self {
        Self {
            flames: shown.flames.clone(),
            place: shown.place,
            before: shown.before,
        }
    }
}

/// The look of the fire `fire` as a save keeps it with its fire, if it shows (#3779).
pub(crate) fn saved_look(world: &World, fire: Entity) -> Option<SavedLook> {
    let mut shown = world.try_query::<&Shown>()?;
    shown
        .iter(world)
        .find(|shown| shown.fire == Some(fire))
        .map(SavedLook::of)
}

/// The looks in `world` that outlive their fires, as a save keeps them (#3779).
pub(crate) fn saved(world: &mut World) -> Vec<SavedLook> {
    let mut shown = world.query::<&Shown>();
    shown
        .iter(world)
        .filter(|shown| shown.fire.is_none())
        .map(SavedLook::of)
        .collect()
}

/// Makes a loaded save's looks that outlived their fires again, each on its own (#3779).
pub(crate) fn again(world: &mut World, looks: Vec<SavedLook>) {
    for saved in looks {
        world.spawn((
            Alone,
            Transform::from_translation(saved.place),
            crate::fx::Resumes(saved),
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// Gives each new fire its look (`0x7A6A40`, `0x7BF690`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn show(
    mut commands: Commands,
    fires: Query<
        (
            Entity,
            Option<&Flames>,
            &Transform,
            Option<&crate::fx::Resumes<SavedLook>>,
            Has<Alone>,
        ),
        Or<(Added<Flames>, Added<Alone>)>,
    >,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, flames, place, resumed, alone) in &fires {
        // A loaded fire's look goes on as it went, one whose fire had ended on its own (#3779).
        let kept = match (resumed, flames) {
            (Some(crate::fx::Resumes(saved)), _) => saved.clone(),
            (None, Some(flames)) => SavedLook {
                flames: HomingFlames::new(flames.0),
                place: place.translation,
                before: None,
            },
            (None, None) => continue,
        };
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
            fire: (!alone).then_some(entity),
            flames: kept.flames,
            place: kept.place,
            before: kept.before,
            mesh,
        });
        if alone {
            commands.entity(entity).despawn();
        } else if resumed.is_some() {
            commands
                .entity(entity)
                .remove::<crate::fx::Resumes<SavedLook>>();
        }
    }
}

/// Runs each fire's look a frame on (`0x7A6AD0`, `0x7A6D30`): its flames along its way, its
/// burst where it bursts, and its end with its last flame.
fn burn(
    mut commands: Commands,
    (time, clock): (Res<Time>, Option<Res<crate::daynight::Clock>>),
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
        look.flames.wind = clock
            .as_deref()
            .map_or([0.0; 3], crate::daynight::Clock::wind);
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
