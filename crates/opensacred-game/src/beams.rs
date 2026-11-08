//! The laser beams (901, `FX_LASERBEAM`, #1092, #2119; docs/re/effects.md, "The laser beams"):
//! `createLaserbeam` (`0x604030`) lays a chain of segments from a point to another, the start of
//! each a cell of ground on, as far as a cell that it may not pass (`ways::laser_places`,
//! `Heights::passes`); each segment stays where it was laid, two crossed quads along it added
//! whole, untinted by the night, in its variant's texture and colour, fading after its variant's
//! hold (`opensacred_rules::particles::LaserSegment`), and plays its variant's sound as it starts.
//! Each segment, an object of its own, lies at its start's place in the original's order of
//! drawing (`fx::Orders::at`, #2220): a beam across several cells lies among their creatures
//! cell by cell.
//! A magic skeleton's drain lays one of the variant 2 from its killer's spine to its own, the
//! hellfire one of the variant 1 from its ball to its target's neck at each strike, the big demons
//! one from each eye, and the Vampiress's bite one of the variant 0 from her spine to its target's.
//!
//! Not followed: the segments' lights of 80, as the world's lights wait for #61.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{LaserSegment, laser_axes};
use opensacred_rules::ways;

use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;
use crate::terrain::Level;

/// The spine's bone, which the drains and the bite join, and its height on a model that has none.
pub(crate) const SPINE: [&str; 1] = ["Bip01 Spine2"];
const SPINE_HIGH: f32 = 60.0;

/// An end of a beam: a point of the world, or a bone of a creature, else its place that high in
/// the models' frame.
#[derive(Clone, Copy, Debug)]
pub(crate) enum End {
    At(Vec3),
    Bone(Entity, &'static [&'static str], f32),
}

/// The bones' names by their index (`0x401000`, the table of 32-byte names at `0x8E7200`;
/// docs/re/effects.md, "The bones"), of which the life leech's beam rolls one of its target's.
pub(crate) static BONES: [&str; 50] = [
    "__Root",
    "Bip01 L Hand",
    "Bip01 R Hand",
    "Bip01 Neck",
    "horse_saddle",
    "bip01_particle01",
    "Bip01 L Thigh",
    "Bip01 R Thigh",
    "bip01_particle01",
    "Bip01 L Forearm",
    "Bip01 R Forearm",
    "Bone_weapon_01",
    "Bone_weapon_02",
    "Bip01 L Finger31",
    "Bip01 R Finger31",
    "weapon_fx01",
    "weapon_fx02",
    "weapon_fx03",
    "weapon_gl01",
    "weapon_gm01",
    "Bip01 Head",
    "Bip01 L Finger3",
    "Bip01 R Finger3",
    "Bip01 Spine3",
    "Bip01 Spine2",
    "Bone_spine",
    "fx_streak01",
    "fx_streak02",
    "fx_streak03",
    "fx_streak04",
    "fx_streak05",
    "fx_streak06",
    "fx_streak07",
    "fx_streak08",
    "fx_streak09",
    "fx_streak10",
    "Bip01 R Foot",
    "Bip01 L Foot",
    "Bip01 Head",
    "horse_bone",
    "Bip01",
    "Head_02",
    "Bip01 Pelvis",
    "Bip01 L UpperArm",
    "Bip01 R UpperArm",
    "Bip01 HeadNub",
    "canon_fx01",
    "Bone09",
    "Canon_fx01",
    "Shildbone",
];

impl End {
    /// A creature's bone of the table's `index` ([`BONES`]), else its root (`0x44CA30`, the bone
    /// 0).
    pub(crate) fn bone(of: Entity, index: usize) -> Self {
        Self::Bone(of, BONES.get(index..=index).unwrap_or(&BONES[..1]), 0.0)
    }

    /// A creature's spine.
    pub(crate) fn spine(of: Entity) -> Self {
        Self::Bone(of, &SPINE, SPINE_HIGH)
    }

    /// A creature's neck, else its root (`0x44CBB0` of the bone 3).
    pub(crate) fn neck(of: Entity) -> Self {
        Self::Bone(of, &[crate::arrows::NECK], 0.0)
    }
}

/// A beam to lay ([`lay`]): its ends and its variant.
#[derive(Component)]
struct Unlaid {
    from: End,
    to: End,
    variant: u8,
}

/// Lays a beam of `variant` from `from` to `to` (`0x604030`), at the next run of its system.
pub(crate) fn lay(commands: &mut Commands, (from, to): (End, End), variant: u8) {
    commands.spawn((
        Unlaid { from, to, variant },
        Transform::default(),
        DespawnOnExit(AppState::InGame),
    ));
}

/// A segment that lives: its system, its length and axes in the models' frame, and its mesh.
#[derive(Component)]
struct Segment {
    look: LaserSegment,
    length: f32,
    axes: [[f32; 3]; 3],
    mesh: Handle<Mesh>,
}

/// The beams' materials by their textures, made as the first is asked; none where a texture is
/// missing.
#[derive(Resource, Default)]
struct Materials(Vec<(&'static str, Option<Handle<Effect>>)>);

/// Lays each beam asked (`0x604030`): its ends found, the level its first creature stands on (the
/// hero's or a creature's that still stands in the way),
/// its segments laid from the start, each a system of its own at its start, until a cell it may
/// not pass, each playing its variant's sound.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn place(
    mut commands: Commands,
    unlaid: Query<(Entity, &Unlaid)>,
    (places, (levels, heroes), children, bones, worn): (
        Query<&GlobalTransform>,
        (Query<&crate::crowd::InTheWay>, Query<&crate::hero::Body>),
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (heights, data, assets): (
        Res<crate::terrain::Heights>,
        Option<Res<Data>>,
        Res<AssetServer>,
    ),
    (mut made, mut effects, mut meshes, mut sounds): (
        ResMut<Materials>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
        MessageWriter<crate::sfx::PlayAt>,
    ),
) {
    for (entity, beam) in &unlaid {
        commands.entity(entity).despawn();
        let point = |end: End| match end {
            End::At(at) => Some(at),
            End::Bone(of, names, high) => {
                crate::making::bone(of, names, &children, (&bones, &worn)).or_else(|| {
                    places
                        .get(of)
                        .ok()
                        .map(|at| at.translation() + Vec3::Y * high * iso::MODEL_SCALE)
                })
            }
        };
        let (Some(from), Some(to)) = (point(beam.from), point(beam.to)) else {
            continue;
        };
        let level: Level = [beam.from, beam.to]
            .iter()
            .find_map(|end| match *end {
                End::Bone(of, ..) => heroes
                    .get(of)
                    .ok()
                    .map(crate::hero::Body::level)
                    .or_else(|| levels.get(of).ok().map(|way| way.level)),
                End::At(_) => None,
            })
            .flatten();
        let texture = LaserSegment::texture(beam.variant);
        let material = match made.0.iter().find(|(name, _)| *name == texture) {
            Some((_, material)) => material.clone(),
            None => {
                let image = data
                    .as_ref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, texture));
                if image.is_none() {
                    warn!("beams: no texture {texture}; its beams are not drawn");
                }
                let material = image.map(|image| effects.add(whole_material(image)));
                made.0.push((texture, material.clone()));
                material
            }
        };
        let starts = ways::laser_places((from.x, from.z), (to.x, to.z));
        debug!(
            "beams: a beam of {} from {from} to {to} on {level:?} in {} segments",
            beam.variant,
            starts.len()
        );
        let n = u16::try_from(starts.len()).unwrap_or(u16::MAX);
        let step = (to - from) / f32::from(n.max(1));
        let along = models(step);
        let axes = laser_axes(along);
        let length = along.iter().map(|v| v * v).sum::<f32>().sqrt();
        for (k, &(x, z)) in (0_u16..).zip(&starts) {
            if !heights.passes(crate::paths::cell_of(x, z), level) {
                break;
            }
            let start = from + step * f32::from(k);
            let mesh = meshes.add(empty());
            let mut segment = commands.spawn((
                Segment {
                    look: LaserSegment::new(beam.variant, (k, n)),
                    length,
                    axes,
                    mesh: mesh.clone(),
                },
                Transform::from_translation(start),
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
            ));
            if let Some(material) = material.clone() {
                segment.with_child((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                ));
            }
            if let Some(sound) = LaserSegment::sound(beam.variant) {
                sounds.write(crate::sfx::PlayAt {
                    sound: crate::sfx::Of::Id(sound),
                    point: start,
                    rate: (0.5, crate::sfx::Roll::None),
                });
            }
        }
    }
}

/// Runs each segment a frame on (`0x7A3DF0`, `0x7A4D50`): its two quads along its axis in its
/// colour, swaying where its variant sways, at its start's place in the order; it goes as its
/// fade ends.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut segments: Query<(Entity, &mut Segment, &Transform)>,
    (mut meshes, orders): (ResMut<Assets<Mesh>>, Orders),
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut segment, start) in &mut segments {
        let segment = &mut *segment;
        segment.look.update(dt);
        if segment.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let [x, y, z] = segment.axes;
        let at =
            |[a, b, c]: [f32; 3]| world(std::array::from_fn(|i| a * x[i] + b * y[i] + c * z[i]));
        let colour = segment.look.colour();
        let mut quads = Quads::default();
        for (corners, uvs) in segment.look.quads(segment.length) {
            quads.push_corners(corners.map(at), uvs.map(Vec2::from), colour);
        }
        if let Some(mut mesh) = meshes.get_mut(&segment.mesh) {
            quads.in_order(orders.at(start.translation), start.translation);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The laser beams.
pub struct BeamsPlugin;

impl Plugin for BeamsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Materials>().add_systems(
            Update,
            (place, run)
                .chain()
                .in_set(Playing)
                .in_set(Unpaused)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
