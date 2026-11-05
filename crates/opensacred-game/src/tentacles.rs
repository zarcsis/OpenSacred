//! The looks of the Daemon's tentacles (1013, `FX_DAYOFTHETENTACLE`, #1092; docs/re/effects.md,
//! "The tentacles"): a tentacle that `spells` raises at a corpse stands as five writhing tendrils
//! of black dots facing the camera over two rings of black runes turning against each other, and
//! sinks as it ends (`opensacred_rules::particles::Tentacle`); his aura lays four such rings
//! about his feet while his effect holds (`opensacred_rules::particles::TentacleAura`). Both are
//! laid over the scene by their alpha, in the world's camera.

use bevy::camera::visibility::NoFrustumCulling;
use bevy::prelude::*;
use opensacred_rules::particles::{self, RuneRing, TentacleAura as Aura};

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;
use crate::spells::{Tentacle, TentacleAura};

/// How far over the ground the rings lie, in world units.
const GROUND_LIFT: f32 = 0.5;
/// The black of the tendrils and the runes (`0xFF000000`).
const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// The looks' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x1013))
    }
}

/// The materials the looks take, found at the first: the tendrils' dots' and each rune's.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    dots: Option<Handle<Effect>>,
    runes: Vec<Option<Handle<Effect>>>,
}

impl Looks {
    /// Finds the looks' textures at the first look asked.
    fn make(&mut self, data: Option<&Data>, assets: &AssetServer, effects: &mut Assets<Effect>) {
        if self.made {
            return;
        }
        self.made = true;
        let Some(data) = data else {
            return;
        };
        let mut material = |name: &str| {
            let image = particle_texture(&data.0.assets, assets, name);
            if image.is_none() {
                warn!("tentacles: no texture {name}; it is not drawn");
            }
            image.map(|image| effects.add(particle_material(image, AlphaMode::Blend)))
        };
        self.dots = material(particles::TENTACLE_TEXTURE);
        self.runes = (1..=particles::TENTACLE_RUNES)
            .map(|rune| material(&particles::tentacle_rune(rune)))
            .collect();
    }

    /// The meshes of the runes, a mesh of each rune's material, made as children of `entity`.
    fn rune_meshes(
        &self,
        commands: &mut Commands,
        entity: Entity,
        meshes: &mut Assets<Mesh>,
    ) -> Vec<Handle<Mesh>> {
        self.runes
            .iter()
            .map(|material| {
                let mesh = meshes.add(empty());
                if let Some(material) = material.clone() {
                    commands.spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material),
                        Transform::IDENTITY,
                        NoFrustumCulling,
                        ChildOf(entity),
                    ));
                }
                mesh
            })
            .collect()
    }
}

/// A tentacle's look: its tendrils and rings, its dots' mesh and its runes'.
#[derive(Component)]
struct TentacleLook {
    tentacle: particles::Tentacle,
    dots: Handle<Mesh>,
    runes: Vec<Handle<Mesh>>,
}

/// The aura's look: its rings and its runes' meshes.
#[derive(Component)]
struct AuraLook {
    aura: Aura,
    runes: Vec<Handle<Mesh>>,
}

/// Gives each new tentacle and aura its look (`0x7B0DF0`, `0x7B1990`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn show(
    mut commands: Commands,
    (tentacles, auras): (
        Query<(Entity, &Tentacle), Added<Tentacle>>,
        Query<(Entity, &TentacleAura), Added<TentacleAura>>,
    ),
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes, mut dice): (
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
        ResMut<Dice>,
    ),
) {
    for (entity, tentacle) in &tentacles {
        looks.make(data.as_deref(), &assets, &mut effects);
        let runes = looks.rune_meshes(&mut commands, entity, &mut meshes);
        let dots = meshes.add(empty());
        if let Some(material) = looks.dots.clone() {
            commands.spawn((
                Mesh3d(dots.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                ChildOf(entity),
            ));
        }
        commands.entity(entity).insert(TentacleLook {
            tentacle: particles::Tentacle::new(tentacle.stands, &mut dice.0),
            dots,
            runes,
        });
    }
    for (entity, aura) in &auras {
        looks.make(data.as_deref(), &assets, &mut effects);
        let runes = looks.rune_meshes(&mut commands, entity, &mut meshes);
        commands.entity(entity).insert(AuraLook {
            aura: Aura::new(aura.seconds, &mut dice.0),
            runes,
        });
    }
}

/// Lays the rings `rings` of the object's runes `runes` flat over the ground, each segment into
/// its rune's mesh of `handles`.
fn lay(rings: &[RuneRing], runes: &[u8], handles: &[Handle<Mesh>], meshes: &mut Assets<Mesh>) {
    let lift = Vec3::Y * GROUND_LIFT;
    let mut quads: Vec<Quads> = handles.iter().map(|_| Quads::default()).collect();
    for ring in rings {
        for (index, corners) in ring.segments() {
            let Some(rune) = runes
                .get(index)
                .and_then(|&rune| usize::from(rune).checked_sub(1))
            else {
                continue;
            };
            if let Some(quads) = quads.get_mut(rune) {
                quads.push_corners(
                    corners.map(|[x, y]| world([x, y, 0.0]) + lift),
                    [Vec2::Y, Vec2::ZERO, Vec2::X, Vec2::ONE],
                    BLACK,
                );
            }
        }
    }
    for (quads, handle) in quads.into_iter().zip(handles) {
        if let Some(mut mesh) = meshes.get_mut(handle) {
            quads.into_mesh(&mut mesh);
        }
    }
}

/// Runs each tentacle and the aura a frame on (`0x7B1480`, `0x7B14F0`): the tentacle's tendrils
/// and rings, sinking past its seconds; the aura's rings at its bearer's feet, gone with its
/// time.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut tentacles: Query<&mut TentacleLook>,
    mut auras: Query<(Entity, &TentacleAura, &mut AuraLook, &mut Transform)>,
    bearers: Query<&GlobalTransform>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for mut look in &mut tentacles {
        let TentacleLook {
            tentacle,
            dots,
            runes,
        } = &mut *look;
        tentacle.update(dt);
        let mut quads = Quads::default();
        for (at, half) in tentacle.dots() {
            quads.push(world(at), half * s, 0.0, (Vec2::ZERO, 1.0), BLACK);
        }
        if let Some(mut mesh) = meshes.get_mut(&*dots) {
            quads.into_mesh(&mut mesh);
        }
        lay(&tentacle.rings(), &tentacle.runes, runes, &mut meshes);
    }
    for (entity, aura, mut look, mut place) in &mut auras {
        look.aura.update(dt);
        if look.aura.done() {
            commands.entity(entity).despawn();
            continue;
        }
        if let Ok(bearer) = bearers.get(aura.bearer) {
            place.translation = bearer.translation();
        }
        lay(
            &look.aura.rings(),
            &look.aura.runes,
            &look.runes,
            &mut meshes,
        );
    }
}

/// The tentacles' looks.
pub struct TentaclesPlugin;

impl Plugin for TentaclesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (show, run)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
