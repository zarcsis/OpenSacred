//! The healings' sights (784, `FX_HEALING`, #1092; docs/re/effects.md, "The healing"): each heal,
//! the hero's spell 7 (`spells`), a healer's (`creature_casts`) or the goblin mages' ball's on its
//! caster's friends, bursts 400 sparks on a ring about its creature's feet that rise ever faster
//! and shrink, turned and added by their alpha in a hero's red or a creature's pale blue, for
//! 2.45 s, untinted by the night (`opensacred_rules::particles::HealingSight`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{HEALING_TEXTURE, HealingSight, healing_corners};

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;

/// A sight to make at its creature's feet ([`make`]): whose heal it is, a hero's or a creature's.
#[derive(Component)]
struct Unmade {
    target: Entity,
    hero: bool,
}

/// A sight that shows where it was made: its look, its corners' colours and its mesh.
#[derive(Component)]
struct Shown {
    look: HealingSight,
    corners: [[f32; 4]; 4],
    mesh: Handle<Mesh>,
}

/// The sparks' material, made as first asked; none where its texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<Option<Handle<Effect>>>);

/// The sights' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x784))
    }
}

/// Starts the sight of a heal of `target`, a hero's or a creature's (the event `0x10`).
pub(crate) fn heal(commands: &mut Commands, target: Entity, hero: bool) {
    commands.spawn((Unmade { target, hero }, DespawnOnExit(AppState::InGame)));
}

/// Gives each new sight its sparks at its creature's feet (`0x774330`, its first run
/// `0x773FC0`), its mesh and its material.
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
        let Ok(at) = places.get(unmade.target) else {
            commands.entity(entity).despawn();
            continue;
        };
        let material = looks
            .0
            .get_or_insert_with(|| {
                let image = data
                    .as_ref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, HEALING_TEXTURE));
                if image.is_none() {
                    warn!("healings: no texture {HEALING_TEXTURE}; the healings are not drawn");
                }
                image.map(|image| effects.add(particle_material(image, AlphaMode::Add)))
            })
            .clone();
        let mesh = meshes.add(empty());
        let mut shown = commands.entity(entity);
        shown.remove::<Unmade>().insert((
            Shown {
                look: HealingSight::new(&mut dice.0),
                corners: healing_corners(unmade.hero),
                mesh: mesh.clone(),
            },
            Transform::from_translation(at.translation()),
            Visibility::default(),
        ));
        if let Some(material) = material {
            shown.with_child((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
            ));
        }
    }
}

/// Runs each sight a frame on (`0x773FC0`, `0x774120`): its sparks turned, facing the camera, in
/// its corners' colours; it goes with its last spark.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut sights: Query<(Entity, &mut Shown, &Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    orders: crate::fx::Orders,
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut shown, at) in &mut sights {
        shown.look.update(dt);
        if shown.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut quads = Quads::default();
        for particle in &shown.look.system.particles {
            quads.push_facing_turned(
                world(particle.place),
                particle.size * iso::MODEL_SCALE,
                particle.angle,
                shown.corners,
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&shown.mesh) {
            // At its place in the order, just behind a creature standing there (#2188).
            quads.in_order(orders.at(at.translation), at.translation);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The healings' sights.
pub struct HealingsPlugin;

impl Plugin for HealingsPlugin {
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
