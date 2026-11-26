//! The scripts' bubble prisons (#2750; docs/re/effects.md, "The bubble prison"): an object of
//! `FX_BUBBLEPRISON` (853) that a script's `CreateObj` makes (`npcs`), as the Daemon's quest 209
//! stands one about its captive Seraphim, draws as `cParticleSystem_bubbleprison` (`0x79C570`) a
//! ring of `FX_RINGXGLOW` flat on the ground, added whole, and ten open cylinders of
//! `PARTICLE_BLOB01` about its place, added by their alpha, each turning at its own height, the
//! last rising ever faster and starting again from the ground
//! (`opensacred_rules::particles::BubblePrison`); untinted by the night. It stands until its
//! `DelNPC`.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, BubblePrison};
use opensacred_rules::spawning::Rng;

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;

/// How far over its place its ring lies, in world units, so that the ground's own faces do not
/// cut it.
const GROUND_LIFT: f32 = 0.5;

/// The seed of the prisons' `rand()` (**estimate**: any).
const SEED: usize = 0x355;

/// A bubble prison's object, which `npcs` makes for a script's `CreateObj` of 853.
#[derive(Component)]
pub(crate) struct Bubble;

/// A prison's look: its cylinders, their dice, and its parts' meshes, the ring's and the
/// cylinders'.
#[derive(Component)]
struct Shown {
    bubble: BubblePrison,
    dice: Rng,
    parts: [Handle<Mesh>; 2],
}

/// A bubble prison's look as a save keeps it (#3814, the epic #3753): its object by its name and its look's
/// state; a load gives it to its object made again, going on as it went.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename = "SavedBubblePrison")]
pub struct SavedLook {
    #[serde(with = "crate::names::required")]
    object: Entity,
    shown: (BubblePrison, Rng),
}

impl bevy::ecs::entity::MapEntities for SavedLook {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.object = mapper.get_mapped(self.object);
    }
}

/// The looks in `world` as a save keeps them (#3814).
pub(crate) fn saved(world: &mut World) -> Vec<SavedLook> {
    let mut shown = world.query::<(Entity, &Shown)>();
    shown
        .iter(world)
        .map(|(object, shown)| SavedLook {
            object,
            shown: (shown.bubble.clone(), shown.dice.clone()),
        })
        .collect()
}

/// Gives a loaded save's looks to their objects made again (#3814): one that shows already takes
/// its state, another takes it as it shows; one whose object the load finds not is left out.
pub(crate) fn again(world: &mut World, looks: Vec<SavedLook>) {
    for saved in looks {
        let Some(mut object) = crate::names::found(Some(saved.object))
            .and_then(|object| world.get_entity_mut(object).ok())
        else {
            continue;
        };
        match object.get_mut::<Shown>() {
            Some(mut shown) => {
                let (bubble, dice) = saved.shown;
                shown.bubble = bubble;
                shown.dice = dice;
            }
            None => {
                object.insert(crate::fx::Resumes(saved.shown));
            }
        }
    }
}

/// The prisons' materials, the ring's and the cylinders', made as the first prison shows; none
/// where a texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<[Option<Handle<Effect>>; 2]>);

/// Gives each new prison its look (`0x79C380`, `0x79C860`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn show(
    mut commands: Commands,
    bubbles: Query<(Entity, Option<&crate::fx::Resumes<(BubblePrison, Rng)>>), Added<Bubble>>,
    (data, assets, mut looks, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (bubble, resumed) in &bubbles {
        let made = looks
            .0
            .get_or_insert_with(|| {
                let texture = |name: &str| {
                    let image = data
                        .as_deref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("bubble prisons: no texture {name}; its part is not drawn");
                    }
                    image
                };
                [
                    texture(particles::BUBBLE_RING)
                        .map(|image| materials.add(whole_material(image))),
                    texture(particles::BUBBLE_BLOB)
                        .map(|image| materials.add(particle_material(image, AlphaMode::Add))),
                ]
            })
            .clone();
        let parts = [meshes.add(empty()), meshes.add(empty())];
        for (mesh, material) in parts.iter().zip(made) {
            let Some(material) = material else {
                continue;
            };
            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
                ChildOf(bubble),
            ));
        }
        // A loaded prison goes on as it went (#3814).
        let (look, dice) = match resumed {
            Some(crate::fx::Resumes(shown)) => shown.clone(),
            None => {
                let index = usize::try_from(bubble.index_u32()).unwrap_or(0);
                let mut dice = crate::seed::dice(SEED ^ index);
                (BubblePrison::new(&mut dice), dice)
            }
        };
        commands
            .entity(bubble)
            .remove::<crate::fx::Resumes<(BubblePrison, Rng)>>()
            .insert(Shown {
                bubble: look,
                dice,
                parts,
            });
    }
}

/// Runs each prison a frame on (`0x79C490`, `0x79C570`): its cylinders turn, the last rising,
/// over its ring on the ground.
fn run(
    time: Res<Time>,
    mut shown: Query<(&mut Shown, &GlobalTransform)>,
    (mut meshes, orders): (ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let (across, depth) = iso::fx_axes();
    let ring_colour = particles::argb(particles::BUBBLE_RING_COLOUR);
    let colour = particles::argb(particles::BUBBLE_COLOUR);
    for (mut look, place) in &mut shown {
        let look = &mut *look;
        look.bubble.update(dt, &mut look.dice);
        let mut ring = Quads::default();
        let half = particles::BUBBLE_RING_HALF * s;
        let lift = Vec3::Y * GROUND_LIFT;
        ring.push_corners(
            [
                lift + (-across - depth) * half,
                lift + (across - depth) * half,
                lift + (across + depth) * half,
                lift + (-across + depth) * half,
            ],
            [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
            ring_colour,
        );
        let mut cylinders = Quads::default();
        for (turn, height) in look.bubble.cylinders() {
            for (corners, uvs) in BubblePrison::sides(turn, height) {
                cylinders.push_corners(corners.map(world), uvs.map(Vec2::from), colour);
            }
        }
        // At its object's place in the order, as the original draws the system (#2188).
        let origin = place.translation();
        let order = orders.at(origin);
        for (handle, mut part) in look.parts.iter().zip([ring, cylinders]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.in_order(order, origin);
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The bubble prisons.
pub struct BubblePrisonsPlugin;

impl Plugin for BubblePrisonsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>().add_systems(
            Update,
            (show, run)
                .chain()
                .in_set(Playing)
                .in_set(Unpaused)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
