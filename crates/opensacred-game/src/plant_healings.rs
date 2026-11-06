//! The Wood Elf's plants' healing's sight (42, 851, `cParticleSystem_naturehealing`; #1092;
//! docs/re/effects.md, "The Wood Elf's sights"): for 3 s a cloud of tiny butterflies of every
//! colour flutters up from about the bearer's feet and stays in the world where each was born,
//! each fading over a second and a half with a faint grey glow; untinted by the night
//! (`opensacred_rules::particles::NatureHealing`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    NATURE_HEALING_GLOW, NATURE_HEALING_GRID, NATURE_HEALING_TEXTURES, NatureHealing,
};

use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;

/// A sight to make: its bearer.
#[derive(Component)]
struct Unmade(Entity);

/// A sight that shows: its bearer, where its root was last, its look and its meshes, the
/// butterflies' and their glows'.
#[derive(Component)]
struct Shown {
    bearer: Entity,
    before: Option<Vec3>,
    look: NatureHealing,
    parts: [Handle<Mesh>; 2],
}

/// The sights' materials: the butterflies laid over by their alpha, their glows added whole; none
/// where a texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<[Option<Handle<Effect>>; 2]>);

/// The sights' dice (`rand()`).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x851))
    }
}

/// Shows the plants' healing's butterflies about `bearer` (`0x526806`).
pub(crate) fn show(commands: &mut Commands, bearer: Entity) {
    commands.spawn((Unmade(bearer), DespawnOnExit(AppState::InGame)));
}

/// Gives each new sight its meshes and its materials (`0x79BE50`, `0x79C1D0`).
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, &Unmade(bearer)) in &unmade {
        let materials = looks
            .0
            .get_or_insert_with(|| {
                let [butterfly, glow] = NATURE_HEALING_TEXTURES;
                let mut material = |name: &str, whole: bool| {
                    let image = data
                        .as_ref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("plant healings: no texture {name}; its part is not drawn");
                    }
                    image.map(|image| {
                        effects.add(if whole {
                            whole_material(image)
                        } else {
                            particle_material(image, AlphaMode::Blend)
                        })
                    })
                };
                [material(butterfly, false), material(glow, true)]
            })
            .clone();
        let parts: [Handle<Mesh>; 2] = std::array::from_fn(|_| meshes.add(empty()));
        let mut made = commands.entity(entity);
        made.remove::<Unmade>().insert((
            Shown {
                bearer,
                before: None,
                look: NatureHealing::default(),
                parts: parts.clone(),
            },
            Transform::IDENTITY,
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

/// Runs each sight a frame on (`0x79BEF0`) and draws it (`0x79BF60`): its butterflies laid along
/// its bearer's way while it lays them, each facing the camera at its place in the world, of its
/// frame of the grid by its age and its colour at the alpha of its life, then its glow of the
/// same half side in the grey of 80; it goes with its last butterfly.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut sights: Query<(Entity, &mut Shown)>,
    places: Query<&GlobalTransform>,
    (mut meshes, mut dice): (ResMut<Assets<Mesh>>, ResMut<Dice>),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let side = 1.0 / f32::from(NATURE_HEALING_GRID);
    for (entity, mut shown) in &mut sights {
        let shown = &mut *shown;
        let place = places
            .get(shown.bearer)
            .map(GlobalTransform::translation)
            .ok()
            .or(shown.before);
        let Some(place) = place else {
            commands.entity(entity).despawn();
            continue;
        };
        let before = shown.before.unwrap_or(place);
        shown
            .look
            .update(dt, (models(before), models(place)), &mut dice.0);
        shown.before = Some(place);
        if shown.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let (mut butterflies, mut glows) = (Quads::default(), Quads::default());
        for butterfly in &shown.look.system.particles {
            let (frame, colour) = NatureHealing::butterfly(butterfly);
            let grid = NATURE_HEALING_GRID;
            let corner = Vec2::new(f32::from(frame % grid), f32::from(frame / grid)) * side;
            let at = world(butterfly.place);
            let half = butterfly.size * s;
            butterflies.push(at, half, 0.0, (corner, side), colour);
            glows.push(at, half, 0.0, (Vec2::ZERO, 1.0), NATURE_HEALING_GLOW);
        }
        for (handle, part) in shown.parts.iter().zip([butterflies, glows]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The plants' healing's butterflies.
pub struct PlantHealingsPlugin;

impl Plugin for PlantHealingsPlugin {
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
