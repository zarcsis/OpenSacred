//! The Seraphim's casts' sights (844, `cParticleSystem_sera_sm_cast`; #1092; docs/re/effects.md,
//! "The Seraphim's casts"), added whole, facing the camera, untinted by the night and drawn just
//! behind her model, as the original draws a system before the creature it follows
//! (`fx::Orders`, #2178):
//!
//! - her BFG's charge (1018) at her right hand's third finger as the art starts (`0x506077`): a
//!   glow growing for 3 s, then a star and pale blue sparks pulled back to the finger until
//!   4.3 s, with its light (`opensacred_rules::particles::SeraCharge`);
//! - her energy bolts' sight (1019) at each hand for the effect's seconds, from its point
//!   (`0x504B96`, `0x504C4F`): four blue comets going round the hand after a burst of sparks
//!   (`opensacred_rules::particles::MultiShot::seraphim`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    MULTI_SHOT_TEXTURES, MultiShot, SERA_BOLTS_BONES, SERA_CHARGE_BONE, SERA_CHARGE_GLOW,
    SERA_CHARGE_TEXTURES, SeraCharge,
};

use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;

/// The BFG, whose start makes the charge.
const BFG: u16 = 1018;

/// The textures of the sights' parts: the charge's glow and its star and sparks, the bolts'
/// comets' glows and their burst's sparks.
const TEXTURES: [&str; 4] = [
    SERA_CHARGE_TEXTURES[0],
    SERA_CHARGE_TEXTURES[1],
    MULTI_SHOT_TEXTURES[0],
    MULTI_SHOT_TEXTURES[2],
];

/// A sight to make: its bearer, and the bolts' seconds and hand, or none for the charge.
#[derive(Component)]
struct Unmade {
    bearer: Entity,
    bolts: Option<(f32, usize)>,
}

/// A sight's look.
enum Look {
    Charge(SeraCharge),
    Bolts(MultiShot),
}

/// A sight that shows: its bearer, the bone it stands at, its look, whether its light is lit,
/// and the meshes of its two parts.
#[derive(Component)]
struct Shown {
    bearer: Entity,
    bone: &'static str,
    look: Look,
    lit: bool,
    parts: [Handle<Mesh>; 2],
}

/// The sights' materials, added whole, by [`TEXTURES`]; none where a texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<[Option<Handle<Effect>>; 4]>);

/// The sights' dice (`rand()`).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x844))
    }
}

/// Shows the energy bolts' sight at each of `bearer`'s hands for `seconds`, the effect's.
pub(crate) fn bolts(commands: &mut Commands, bearer: Entity, seconds: f32) {
    for hand in 0..SERA_BOLTS_BONES.len() {
        commands.spawn((
            Unmade {
                bearer,
                bolts: Some((seconds, hand)),
            },
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// Shows the BFG's charge as the art starts (`0x506077`).
fn charge(
    mut commands: Commands,
    started: Query<(Entity, &crate::making::Making), Added<crate::making::Playing>>,
) {
    for (bearer, making) in &started {
        if making.art == BFG {
            commands.spawn((
                Unmade {
                    bearer,
                    bolts: None,
                },
                DespawnOnExit(AppState::InGame),
            ));
        }
    }
}

/// Gives each new sight its look, its meshes and its materials (`0x7905E0`, `0x790CE0`).
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    places: Query<&GlobalTransform>,
    (data, assets, mut looks, mut dice): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Dice>,
    ),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, unmade) in &unmade {
        let Ok(at) = places.get(unmade.bearer) else {
            commands.entity(entity).despawn();
            continue;
        };
        let materials = looks
            .0
            .get_or_insert_with(|| {
                TEXTURES.map(|name| {
                    let image = data
                        .as_ref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("sera sights: no texture {name}; its part is not drawn");
                    }
                    image.map(|image| effects.add(whole_material(image)))
                })
            })
            .clone();
        let (bone, look, [first, second]) = match unmade.bolts {
            Some((seconds, hand)) => (
                SERA_BOLTS_BONES[hand],
                Look::Bolts(MultiShot::seraphim(seconds, &mut dice.0)),
                [2, 3],
            ),
            None => (
                SERA_CHARGE_BONE,
                Look::Charge(SeraCharge::default()),
                [0, 1],
            ),
        };
        let parts: [Handle<Mesh>; 2] = std::array::from_fn(|_| meshes.add(empty()));
        let mut made = commands.entity(entity);
        made.remove::<Unmade>().insert((
            Shown {
                bearer: unmade.bearer,
                bone,
                look,
                lit: false,
                parts: parts.clone(),
            },
            Transform::from_translation(at.translation()),
            Visibility::default(),
        ));
        for (mesh, material) in parts
            .into_iter()
            .zip([first, second].map(|k| materials[k].clone()))
        {
            if let Some(material) = material {
                made.with_child((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                ));
            }
        }
    }
}

/// Runs each sight a frame on (`0x790680`) and draws it (`0x790820`) at its bearer's bone, else
/// its place: the charge's sparks and its glow, its light from 3 s (`fx_lights`); the bolts'
/// comets' glows and their burst's sparks; it goes with its end, or with its bearer.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut sights: Query<(Entity, &mut Shown, &mut Transform)>,
    (places, children, bones, worn): (
        Query<&GlobalTransform>,
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (mut meshes, mut dice): (ResMut<Assets<Mesh>>, ResMut<Dice>),
    orders: Orders,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let (across, depth) = iso::fx_axes();
    let frame = |[x, y, z]: [f32; 3]| (across * x + depth * y + Vec3::Y * z) * s;
    let whole = (Vec2::ZERO, 1.0);
    for (entity, mut shown, mut transform) in &mut sights {
        let shown = &mut *shown;
        let Ok(place) = places.get(shown.bearer).map(GlobalTransform::translation) else {
            commands.entity(entity).despawn();
            continue;
        };
        transform.translation =
            crate::making::bone(shown.bearer, &[shown.bone], &children, (&bones, &worn))
                .unwrap_or(place);
        let mut quads: [Quads; 2] = std::array::from_fn(|_| Quads::default());
        match &mut shown.look {
            Look::Charge(charge) => {
                charge.update(dt, &mut dice.0);
                if charge.done() {
                    commands.entity(entity).despawn();
                    continue;
                }
                if charge.lit() && !shown.lit {
                    shown.lit = true;
                    commands.spawn(crate::fx_lights::child(crate::fx_lights::ICE_BLUE, entity));
                }
                for spark in &charge.sparks.particles {
                    quads[1].push(
                        frame(spark.place),
                        spark.size * s,
                        0.0,
                        whole,
                        SeraCharge::spark_colour(spark.life),
                    );
                }
                if let Some((part, half)) = charge.glow()
                    && let Some(quads) = quads.get_mut(part)
                {
                    quads.push(Vec3::ZERO, half * s, 0.0, whole, SERA_CHARGE_GLOW);
                }
            }
            Look::Bolts(sight) => {
                sight.update(dt, &mut dice.0);
                if sight.done() {
                    commands.entity(entity).despawn();
                    continue;
                }
                let colour = sight.colour();
                for (tail, _) in sight.comets() {
                    for (point, half) in tail {
                        quads[0].push(frame(point), half * s, 0.0, whole, colour);
                    }
                }
                for spark in &sight.sparks.particles {
                    quads[1].push(
                        frame(spark.place),
                        spark.size * s,
                        0.0,
                        whole,
                        sight.spark_colour(spark.life),
                    );
                }
            }
        }
        let order = orders.behind(shown.bearer);
        for (handle, mut part) in shown.parts.iter().zip(quads) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.in_order(order, transform.translation);
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The Seraphim's casts' sights.
pub struct SeraSightsPlugin;

impl Plugin for SeraSightsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>()
            .init_resource::<Dice>()
            .add_systems(
                Update,
                (charge, make, run)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
