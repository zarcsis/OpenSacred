//! The Wood Elf's multi-shot's sight (1005, 861, `cParticleSystem_elve_sm_cast`; #1092;
//! docs/re/effects.md, "The Wood Elf's sights"): at her left hand's third finger for the art's
//! seconds, green comets going round the hand on axes at random, each a star with a tail of
//! shrinking glows, after a first burst of sparks pulled back to the hand; all added whole,
//! facing the camera, untinted by the night (`opensacred_rules::particles::MultiShot`).
//!
//! The `Rundum` with a bow's ring (861 of 1061, #2240): as its arrows leave, for 1.3 s at the
//! shooter's root, a horizontal ring of green glows through its left hand's third finger and a
//! star at the hand, shrinking away (`opensacred_rules::particles::BowRundumRing`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    BowRundumRing, MULTI_SHOT_BONE, MULTI_SHOT_GREEN, MULTI_SHOT_HEAD, MULTI_SHOT_TEXTURES,
    MultiShot,
};

use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;

/// A sight to make: its bearer, the art's level and its seconds.
#[derive(Component)]
struct Unmade {
    bearer: Entity,
    level: u32,
    seconds: f32,
}

/// A sight that shows: its bearer, its look and its meshes, the glows', the heads' and the
/// sparks'.
#[derive(Component)]
struct Shown {
    bearer: Entity,
    look: MultiShot,
    parts: [Handle<Mesh>; 3],
}

/// The sights' materials, added whole, by their textures; none where a texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<[Option<Handle<Effect>>; 3]>);

/// The sights' dice (`rand()`).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x861))
    }
}

/// Shows the multi-shot's sight at `bearer`'s left hand for `seconds`, of the art's level in play
/// `level` (`0x50C633`).
pub(crate) fn multi_shot(commands: &mut Commands, bearer: Entity, level: u32, seconds: f32) {
    commands.spawn((
        Unmade {
            bearer,
            level,
            seconds,
        },
        DespawnOnExit(AppState::InGame),
    ));
}

/// A `Rundum` with a bow's ring to make at its shooter.
#[derive(Component)]
struct UnmadeRing(Entity);

/// A multiple shot's sight as a save keeps it (#3776, the epic #3753): its bearer by its name
/// and its look; a load makes it again on its bearer, going on as it went.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedElfSight {
    #[serde(with = "crate::names::required")]
    bearer: Entity,
    look: MultiShot,
}

impl bevy::ecs::entity::MapEntities for SavedElfSight {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.bearer = mapper.get_mapped(self.bearer);
    }
}

/// A bow's round strike's ring as a save keeps it (#3776): its bearer by its name and its
/// look.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedElfRing {
    #[serde(with = "crate::names::required")]
    bearer: Entity,
    look: BowRundumRing,
}

impl bevy::ecs::entity::MapEntities for SavedElfRing {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.bearer = mapper.get_mapped(self.bearer);
    }
}

/// The elf's sights and rings in `world` as a save keeps them (#3776).
pub(crate) fn saved(world: &mut World) -> (Vec<SavedElfSight>, Vec<SavedElfRing>) {
    let mut shown = world.query::<&Shown>();
    let sights = shown
        .iter(world)
        .map(|shown| SavedElfSight {
            bearer: shown.bearer,
            look: shown.look.clone(),
        })
        .collect();
    let mut rings = world.query::<&Ring>();
    let rings = rings
        .iter(world)
        .map(|ring| SavedElfRing {
            bearer: ring.bearer,
            look: ring.look.clone(),
        })
        .collect();
    (sights, rings)
}

/// Makes a loaded save's sights and rings again on their bearers (#3776); those of a bearer the
/// load finds not are left out.
pub(crate) fn again(world: &mut World, (sights, rings): (Vec<SavedElfSight>, Vec<SavedElfRing>)) {
    for saved in sights {
        if crate::names::found(Some(saved.bearer)).is_none() {
            continue;
        }
        world.spawn((
            Unmade {
                bearer: saved.bearer,
                level: 0,
                seconds: 0.0,
            },
            crate::fx::Resumes(saved.look),
            DespawnOnExit(AppState::InGame),
        ));
    }
    for saved in rings {
        if crate::names::found(Some(saved.bearer)).is_none() {
            continue;
        }
        world.spawn((
            UnmadeRing(saved.bearer),
            crate::fx::Resumes(saved.look),
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// A `Rundum` with a bow's ring that shows: its shooter, its look and the meshes of its glows
/// and its star.
#[derive(Component)]
struct Ring {
    bearer: Entity,
    look: BowRundumRing,
    parts: [Handle<Mesh>; 2],
}

/// Shows the `Rundum` with a bow's ring at `bearer` as its arrows leave (`0x4DEB2A`, the FX
/// event `0x54` of the art 1061).
pub(crate) fn bow_rundum(commands: &mut Commands, bearer: Entity) {
    commands.spawn((UnmadeRing(bearer), DespawnOnExit(AppState::InGame)));
}

/// Gives each new ring its look, the hand's point from the shooter's root in the models' frame
/// (`0x795880`, `0x795140` at its first run), its meshes and its materials.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn make_ring(
    mut commands: Commands,
    unmade: Query<(
        Entity,
        &UnmadeRing,
        Option<&crate::fx::Resumes<BowRundumRing>>,
    )>,
    (places, children, bones, worn): (
        Query<&GlobalTransform>,
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    let s = iso::MODEL_SCALE;
    let (across, depth) = iso::fx_axes();
    for (entity, &UnmadeRing(bearer), resumed) in &unmade {
        let Ok(root) = places.get(bearer).map(GlobalTransform::translation) else {
            commands.entity(entity).despawn();
            continue;
        };
        let hand = crate::making::bone(bearer, &[MULTI_SHOT_BONE], &children, (&bones, &worn))
            .unwrap_or(root);
        let d = hand - root;
        let offset = [d.dot(across) / s, d.dot(depth) / s, d.y / s];
        let materials = materials(&mut looks, data.as_deref(), &assets, &mut effects);
        let parts: [Handle<Mesh>; 2] = std::array::from_fn(|_| meshes.add(empty()));
        let mut made = commands.entity(entity);
        made.remove::<(UnmadeRing, crate::fx::Resumes<BowRundumRing>)>()
            .insert((
                Ring {
                    bearer,
                    // A loaded ring goes on as it went (#3776).
                    look: resumed.map_or_else(
                        || BowRundumRing::new(offset),
                        |crate::fx::Resumes(look)| look.clone(),
                    ),
                    parts: parts.clone(),
                },
                Transform::from_translation(root),
                Visibility::default(),
            ));
        for (mesh, material) in parts.into_iter().zip(materials) {
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

/// Runs each ring a frame on (`0x795140`) and draws it (`0x7952C0`): at its shooter's root,
/// unturned, its glows and its star facing the camera, just behind the shooter's model; it goes
/// past its 1.3 s, or with its shooter.
fn ring(
    mut commands: Commands,
    time: Res<Time>,
    mut rings: Query<(Entity, &mut Ring, &mut Transform)>,
    places: Query<&GlobalTransform>,
    (mut meshes, orders): (ResMut<Assets<Mesh>>, Orders),
) {
    let s = iso::MODEL_SCALE;
    let (across, depth) = iso::fx_axes();
    let frame = |[x, y, z]: [f32; 3]| (across * x + depth * y + Vec3::Y * z) * s;
    let whole = (Vec2::ZERO, 1.0);
    for (entity, mut shown, mut transform) in &mut rings {
        let shown = &mut *shown;
        let Ok(root) = places.get(shown.bearer).map(GlobalTransform::translation) else {
            commands.entity(entity).despawn();
            continue;
        };
        shown.look.update(time.delta_secs());
        if shown.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation = root;
        let mut quads: [Quads; 2] = std::array::from_fn(|_| Quads::default());
        for (point, half) in shown.look.glows() {
            quads[0].push(frame(point), half * s, 0.0, whole, MULTI_SHOT_GREEN);
        }
        let (point, half) = shown.look.star();
        quads[1].push(frame(point), half * s, 0.0, whole, MULTI_SHOT_GREEN);
        let order = orders.borne_by(shown.bearer);
        for (handle, mut part) in shown.parts.iter().zip(quads) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.in_order(order, root);
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The sights' materials, made at the first: the glows', the stars' and the sparks'.
fn materials(
    looks: &mut Looks,
    data: Option<&Data>,
    assets: &AssetServer,
    effects: &mut Assets<Effect>,
) -> [Option<Handle<Effect>>; 3] {
    looks
        .0
        .get_or_insert_with(|| {
            MULTI_SHOT_TEXTURES.map(|name| {
                let image = data.and_then(|data| particle_texture(&data.0.assets, assets, name));
                if image.is_none() {
                    warn!("elf sights: no texture {name}; its part is not drawn");
                }
                image.map(|image| effects.add(whole_material(image)))
            })
        })
        .clone()
}

/// Gives each new sight its look, its meshes and its materials (`0x7950A0`, `0x795880`).
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade, Option<&crate::fx::Resumes<MultiShot>>)>,
    places: Query<&GlobalTransform>,
    (data, assets, mut looks, mut dice): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Dice>,
    ),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, unmade, resumed) in &unmade {
        let Ok(at) = places.get(unmade.bearer) else {
            commands.entity(entity).despawn();
            continue;
        };
        let materials = materials(&mut looks, data.as_deref(), &assets, &mut effects);
        let parts: [Handle<Mesh>; 3] = std::array::from_fn(|_| meshes.add(empty()));
        let mut made = commands.entity(entity);
        made.remove::<(Unmade, crate::fx::Resumes<MultiShot>)>()
            .insert((
                Shown {
                    bearer: unmade.bearer,
                    // A loaded sight goes on as it went (#3776).
                    look: resumed.map_or_else(
                        || MultiShot::new(unmade.level, unmade.seconds, &mut dice.0),
                        |crate::fx::Resumes(look)| look.clone(),
                    ),
                    parts: parts.clone(),
                },
                Transform::from_translation(at.translation()),
                Visibility::default(),
            ));
        for (mesh, material) in parts.into_iter().zip(materials) {
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

/// Runs each sight a frame on (`0x795140`) and draws it (`0x7952C0`): at its bearer's left hand,
/// else its place, each comet's tail of glows and its head, then the burst's sparks, all facing
/// the camera in the comets' green or the sparks' ramp, drawn just behind its bearer's model, as
/// the original draws the system before the creature it follows (`fx::Orders`, #2178); it goes at
/// once with its seconds, or with its bearer.
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
    let dt = time.delta_secs();
    let s = iso::MODEL_SCALE;
    let (across, depth) = iso::fx_axes();
    let frame = |[x, y, z]: [f32; 3]| (across * x + depth * y + Vec3::Y * z) * s;
    for (entity, mut shown, mut transform) in &mut sights {
        let shown = &mut *shown;
        let Ok(place) = places.get(shown.bearer).map(GlobalTransform::translation) else {
            commands.entity(entity).despawn();
            continue;
        };
        shown.look.update(dt, &mut dice.0);
        if shown.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation =
            crate::making::bone(shown.bearer, &[MULTI_SHOT_BONE], &children, (&bones, &worn))
                .unwrap_or(place);
        let mut quads: [Quads; 3] = std::array::from_fn(|_| Quads::default());
        let whole = (Vec2::ZERO, 1.0);
        for (tail, head) in shown.look.comets() {
            for (point, half) in tail {
                quads[0].push(frame(point), half * s, 0.0, whole, MULTI_SHOT_GREEN);
            }
            quads[1].push(
                frame(head),
                MULTI_SHOT_HEAD * s,
                0.0,
                whole,
                MULTI_SHOT_GREEN,
            );
        }
        for spark in &shown.look.sparks.particles {
            quads[2].push(
                frame(spark.place),
                spark.size * s,
                0.0,
                whole,
                shown.look.spark_colour(spark.life),
            );
        }
        let order = orders.borne_by(shown.bearer);
        for (handle, mut part) in shown.parts.iter().zip(quads) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.in_order(order, transform.translation);
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The Wood Elf's sights.
pub struct ElfSightsPlugin;

impl Plugin for ElfSightsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>()
            .init_resource::<Dice>()
            .add_systems(
                Update,
                (make, run, make_ring, ring)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
