//! The beam that a magic skeleton draws from its killer as it drains it at its first death (#1263;
//! docs/re/effects.md, "The undead's effects"): `createLaserbeam` (`0x604030`, from `0x533029`),
//! a chain of `FX_LASERBEAM` (901) segments, one a cell long, from the killer's `Bip01 Spine2` to
//! the skeleton's; each 20 wide, of `FX_VAMPBLOODLINE` added at half grey, the chain wobbling most
//! at its middle and each segment showing a quarter of the texture that flickers, all fading over
//! a second. Drawn as the other effects, untinted by the night (`iso::UNTINTED`).
//!
//! Not followed: the chain's stop at a cell that fails `0x636A60`, and its light of 80, as the
//! world's lights wait for #61.

use bevy::prelude::*;
use opensacred_rules::spawning::Rng;

use crate::frame::{Playing, Unpaused};
use crate::fx::{Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;

/// The texture, the segments' length (a cell, `0x892048`'s 53.66563) and width, in the original's
/// units, the colour, and how long the beam lives (`0x604030`).
const TEXTURE: &str = "FX_VAMPBLOODLINE";
const SEGMENT: f32 = 53.665_63;
const WIDTH: f32 = 20.0;
const GREY: f32 = 0.5;
const LIVES: f32 = 1.0;
/// How far the chain's middle wobbles across it, in the original's units (**estimate**).
const WOBBLE: f32 = 8.0;
/// The bones that the beam joins, the spine's, else the creature's place this high up.
const SPINE: [&str; 1] = ["Bip01 Spine2"];
const SPINE_HIGH: f32 = 60.0;

/// A beam to make from `from` to `to` ([`make`]).
#[derive(Component)]
pub struct Unmade {
    from: Entity,
    to: Entity,
}

/// Draws a beam from the killer `from` to the skeleton `to`.
pub fn spawn(commands: &mut Commands, from: Entity, to: Entity) {
    commands.spawn((
        Unmade { from, to },
        Transform::default(),
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
}

/// A beam that lives: its ends, its age and its mesh.
#[derive(Component)]
struct Beam {
    from: Entity,
    to: Entity,
    age: f32,
    mesh: Handle<Mesh>,
}

/// The beams' material, made as the first is asked; none where the texture is missing.
#[derive(Resource, Default)]
struct Material(Option<Option<Handle<StandardMaterial>>>);

/// The beams' rolls of the wobble and the flicker.
#[derive(Resource)]
struct Rolls(Rng);

impl Default for Rolls {
    fn default() -> Self {
        Self(crate::seed::dice(0x0B_EA_A5))
    }
}

/// Gives each new beam its mesh and its material.
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    data: Option<Res<Data>>,
    assets: Res<AssetServer>,
    (mut made, mut materials, mut meshes): (
        ResMut<Material>,
        ResMut<Assets<StandardMaterial>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (entity, unmade) in &unmade {
        let material = made
            .0
            .get_or_insert_with(|| {
                let image = data
                    .as_ref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, TEXTURE));
                if image.is_none() {
                    warn!("beams: no texture {TEXTURE}; the beams are not drawn");
                }
                image.map(|image| materials.add(particle_material(image, AlphaMode::Add)))
            })
            .clone();
        let mesh = meshes.add(empty());
        let mut beam = commands.entity(entity);
        beam.remove::<Unmade>().insert(Beam {
            from: unmade.from,
            to: unmade.to,
            age: 0.0,
            mesh: mesh.clone(),
        });
        if let Some(material) = material {
            beam.with_child((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::default(),
                bevy::camera::visibility::NoFrustumCulling,
                bevy::camera::visibility::RenderLayers::layer(iso::UNTINTED),
            ));
        }
    }
}

/// The joints of a chain from `from` to `to`, one a segment of `segment` long, each pushed across
/// the beam along `across` by `wobble` times its share of the roll in `-1..1` and the sine of its
/// place along the chain, the ends kept.
fn joints(from: Vec3, to: Vec3, (segment, across): (f32, Vec3), wobble: &[f32]) -> Vec<Vec3> {
    let length = from.distance(to);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a count of segments, small and not negative"
    )]
    let count = (length / segment.max(f32::EPSILON))
        .ceil()
        .clamp(1.0, 256.0) as usize;
    (0..=count)
        .map(|k| {
            #[expect(clippy::cast_precision_loss, reason = "a joint's index, below 257")]
            let t = k as f32 / count as f32;
            let push = wobble.get(k).copied().unwrap_or(0.0) * (std::f32::consts::PI * t).sin();
            from.lerp(to, t) + across * push
        })
        .collect()
}

/// Ages each beam, joins its ends' spines by its chain of segments facing the camera, and takes it
/// away after its second or where either end is gone.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut beams: Query<(Entity, &mut Beam)>,
    places: Query<&GlobalTransform>,
    children: Query<&Children>,
    bones: Query<(&Name, &GlobalTransform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut rolls: ResMut<Rolls>,
) {
    let dt = time.delta_secs().min(0.1);
    let unit = iso::CELL / SEGMENT;
    for (entity, mut beam) in &mut beams {
        beam.age += dt;
        let spine = |of: Entity| {
            crate::making::bone(of, &SPINE, &children, &bones).or_else(|| {
                places
                    .get(of)
                    .ok()
                    .map(|at| at.translation() + Vec3::Y * iso::height(SPINE_HIGH))
            })
        };
        let (Some(from), Some(to)) = (spine(beam.from), spine(beam.to)) else {
            commands.entity(entity).despawn();
            continue;
        };
        if beam.age >= LIVES {
            commands.entity(entity).despawn();
            continue;
        }
        let Some(mut mesh) = meshes.get_mut(&beam.mesh) else {
            continue;
        };
        // Across the beam within the screen's plane.
        let across = (to - from).cross(iso::forward()).normalize_or(iso::right());
        let rolled: Vec<f32> = (0..=256)
            .map(|_| {
                #[expect(clippy::cast_precision_loss, reason = "a roll below 2001")]
                let roll = rolls.0.below(2001) as f32;
                roll / 1000.0 - 1.0
            })
            .collect();
        let chain = joints(
            from,
            to,
            (SEGMENT * unit, across),
            &rolled.iter().map(|r| r * WOBBLE * unit).collect::<Vec<_>>(),
        );
        let fade = 1.0 - beam.age / LIVES;
        let colour = [GREY * fade, GREY * fade, GREY * fade, 1.0];
        let half = WIDTH * unit / 2.0;
        let mut quads = Quads::default();
        for pair in chain.windows(2) {
            #[expect(clippy::cast_precision_loss, reason = "a quarter, below 4")]
            let quarter = rolls.0.below(4) as f32 / 4.0;
            let (a, b) = (pair[0], pair[1]);
            quads.push_corners(
                [
                    a - across * half,
                    b - across * half,
                    b + across * half,
                    a + across * half,
                ],
                [
                    Vec2::new(0.0, quarter),
                    Vec2::new(1.0, quarter),
                    Vec2::new(1.0, quarter + 0.25),
                    Vec2::new(0.0, quarter + 0.25),
                ],
                colour,
            );
        }
        quads.into_mesh(&mut mesh);
    }
}

/// The drain's beams.
pub struct BeamsPlugin;

impl Plugin for BeamsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Material>()
            .init_resource::<Rolls>()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chain_has_a_joint_a_segment_and_wobbles_most_at_its_middle() {
        let from = Vec3::ZERO;
        let to = Vec3::new(100.0, 0.0, 0.0);
        let across = Vec3::Z;
        let still = joints(from, to, (25.0, across), &[]);
        assert_eq!(still.len(), 5);
        assert_eq!(still[2], Vec3::new(50.0, 0.0, 0.0));
        // A wobble of 1 at every joint: the ends kept, the middle pushed the most.
        let wobbling = joints(from, to, (25.0, across), &[1.0; 5]);
        assert!(wobbling[0].z.abs() < 1e-6 && wobbling[4].z.abs() < 1e-5);
        assert!((wobbling[2].z - 1.0).abs() < 1e-6);
        assert!(wobbling[1].z < wobbling[2].z);
    }
}
