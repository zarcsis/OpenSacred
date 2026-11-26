//! The look of the souls that fly to the Dark Elf's soulcatcher (780, `FX_PUZZLE_SOLVED`, #1092;
//! docs/re/effects.md, "The souls"): the dying creature's figure in pale green sparks, one at
//! each of its vertices taken as it dies, rising and streaming ever faster to its soul as it
//! flies to him, added whole and untinted; where it arrives or a cell stops it its sparks stop
//! and fade out (`opensacred_rules::particles::SoulSparks`), at its soul's place in the original's
//! order of drawing as it flies (`fx::Orders::at`, #2220). A body that vanishes as it has lain
//! its time leaves the same figure, which rises and fades where it lay (`bodies`, #2501).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, SoulSparks};

use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;

/// A soul, which takes its look ([`show`]): the creature whose figure it takes, and whether it
/// homes on nothing, a vanishing body's, which only marks where its figure lies and goes as its
/// look is made (`SoulSparks::rising`).
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Sparks {
    pub(crate) from: Entity,
    pub(crate) rising: bool,
}

/// The points of a soul's figure taken as it was made, in the world, which its look takes in
/// place of its creature's: a script's group member that goes at once (`group_looks`, #2893).
#[derive(Component, Debug, Clone)]
pub(crate) struct Figure(pub(crate) Vec<Vec3>);

/// The sparks' material, made at the first soul; none where its texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<Option<Handle<Effect>>>);

/// A soul's look, which outlives its soul while its sparks fade: its soul while it flies, its
/// system, its place and its mesh.
#[derive(Component)]
struct Shown {
    soul: Option<Entity>,
    look: SoulSparks,
    place: Vec3,
    mesh: Handle<Mesh>,
}

/// A look carried alone by a load, its soul gone, which [`show`] makes and then lets go (#3809).
#[derive(Component)]
struct Alone;

/// A soul's look as a save keeps it (#3809, the epic #3753): its sparks and its place. A load
/// gives a flying soul's to its soul sent on again (`souls::SavedSoul`), and makes one that
/// outlives its soul, or a vanishing body's rising figure, again on its own.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename = "SavedSoulSparks")]
pub struct SavedLook {
    look: SoulSparks,
    place: Vec3,
}

impl SavedLook {
    /// The look `shown` as a save keeps it.
    fn of(shown: &Shown) -> Self {
        Self {
            look: shown.look.clone(),
            place: shown.place,
        }
    }
}

/// The look of the soul `soul` as a save keeps it with its soul, if it shows (#3809).
pub(crate) fn saved_look(world: &World, soul: Entity) -> Option<SavedLook> {
    let mut shown = world.try_query::<&Shown>()?;
    shown
        .iter(world)
        .find(|shown| shown.soul == Some(soul))
        .map(SavedLook::of)
}

/// Gives the loaded soul `soul` its look as the save kept it (#3809), of no creature's figure.
pub(crate) fn look_again(soul: &mut EntityWorldMut<'_>, saved: SavedLook) {
    soul.insert((
        Sparks {
            from: Entity::PLACEHOLDER,
            rising: false,
        },
        crate::fx::Resumes(saved),
    ));
}

/// The souls' looks in `world` that outlive their souls, as a save keeps them (#3809).
pub(crate) fn saved(world: &mut World) -> Vec<SavedLook> {
    let mut shown = world.query::<&Shown>();
    shown
        .iter(world)
        .filter(|shown| shown.soul.is_none())
        .map(SavedLook::of)
        .collect()
}

/// Makes a loaded save's souls' looks that outlived their souls again, each on its own (#3809).
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

/// Gives each new soul its look (`0x5A46B3`, `0x76CD50`): the sparks of its creature's figure,
/// every `⌊V/500⌋`th vertex of its meshes but its worn items', skinned to its pose (`0x767FB0`,
/// `0x44FC30`; `vertices`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn show(
    mut commands: Commands,
    souls: Query<
        (
            Entity,
            Option<&Sparks>,
            &Transform,
            Option<&Figure>,
            Option<&crate::fx::Resumes<SavedLook>>,
            Has<Alone>,
        ),
        Or<(Added<Sparks>, Added<Alone>)>,
    >,
    shapes: crate::vertices::ModelPoints,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (soul, sparks, place, figure, resumed, alone) in &souls {
        let material = looks
            .0
            .get_or_insert_with(|| {
                let image = data.as_deref().and_then(|data| {
                    particle_texture(&data.0.assets, &assets, particles::SOUL_SPARK_TEXTURE)
                });
                if image.is_none() {
                    warn!(
                        "soul sparks: no texture {}; the souls are not drawn",
                        particles::SOUL_SPARK_TEXTURE
                    );
                }
                image.map(|image| effects.add(whole_material(image)))
            })
            .clone();
        // A loaded soul's look goes on as it went, one whose soul had gone on its own (#3809).
        let (look, soul, place) = match (resumed, sparks) {
            (Some(crate::fx::Resumes(saved)), _) => {
                if alone {
                    commands.entity(soul).despawn();
                } else {
                    commands
                        .entity(soul)
                        .remove::<crate::fx::Resumes<SavedLook>>();
                }
                (saved.look.clone(), (!alone).then_some(soul), saved.place)
            }
            (None, Some(sparks)) => {
                let points = figure.map_or_else(
                    || shapes.picked(sparks.from, &meshes),
                    |figure| figure.0.clone(),
                );
                debug!(
                    "soul sparks: {soul} takes {} sparks of {}",
                    points.len(),
                    sparks.from
                );
                let points = points.into_iter().map(models);
                if sparks.rising {
                    commands.entity(soul).despawn();
                    (SoulSparks::rising(points), None, place.translation)
                } else {
                    (SoulSparks::new(points), Some(soul), place.translation)
                }
            }
            (None, None) => continue,
        };
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
            soul,
            look,
            place,
            mesh,
        });
    }
}

/// Runs each soul's look a frame on (`0x76C9D0`, `0x76CB10`): its sparks drawn to its soul as it
/// flies, their stop where it ends, and its end with its last spark.
fn stream(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown)>,
    souls: Query<&Transform, With<Sparks>>,
    (mut meshes, orders): (ResMut<Assets<Mesh>>, Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut look) in &mut shown {
        let look = &mut *look;
        match look.soul.map(|soul| souls.get(soul)) {
            Some(Ok(place)) => look.place = place.translation,
            _ => {
                if look.soul.take().is_some() {
                    look.look.arrive();
                }
            }
        }
        look.look.update(dt, models(look.place));
        if look.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut quads = Quads::default();
        let [r, g, b, _] = particles::SOUL_COLOUR;
        for spark in &look.look.system.particles {
            quads.push(
                world(spark.place),
                spark.size * s,
                spark.angle,
                (Vec2::ZERO, 1.0),
                [r, g, b, spark.life / 255.0],
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&look.mesh) {
            quads.in_order(orders.at(look.place), Vec3::ZERO);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The souls' looks.
pub struct SoulSparksPlugin;

impl Plugin for SoulSparksPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>().add_systems(
            Update,
            (show, stream)
                .chain()
                .in_set(Playing)
                .in_set(Unpaused)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
