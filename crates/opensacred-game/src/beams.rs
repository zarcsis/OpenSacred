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
//! one from each eye, the Vampiress's bite one of the variant 0 from her spine to its target's, a
//! strike's life leech one of the variant 2 from the attacker's spine to a bone of the struck
//! one's at random (`fight`), and a brain-footer's stare one of its type's variant (10 to 12) from
//! its spine to a bone of its target's at random (`creature_arts`, #2567), which goes on past a
//! cell that does not pass ([`THROUGH`], #2819); the zhadum one of the variant 5 at the creature
//! it kills and on (`zhadums`, #2920), of two sets of quads whose textures flow along it.
//! Each segment lights the world by 80 on the ground at its start until it ends
//! (`daynight::Light`, `0x7A5327`, #3153; docs/re/effects.md, "Lights").

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{LASER_LIGHT, LaserSegment, laser_axes};
use opensacred_rules::ways;

use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, particle_texture, repeated_texture, whole_material};
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
/// docs/re/effects.md, "The FX event"), of which the life leech's beam and the brain-footer's
/// stare roll one of their target's.
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

/// A beam to lay ([`lay`]): its ends and its flags, its variant in their low seven bits and
/// [`THROUGH`].
#[derive(Component)]
struct Unlaid {
    from: End,
    to: End,
    flags: u8,
}

/// The flag of a beam that goes on past a cell that does not pass (`0x604030`'s `0x80`): the
/// brain-footers' stares' (`0x4D4E50`).
pub(crate) const THROUGH: u8 = 0x80;

/// Lays a beam of `flags` from `from` to `to` (`0x604030`), at the next run of its system: its
/// variant in their low seven bits, and [`THROUGH`].
pub(crate) fn lay(commands: &mut Commands, (from, to): (End, End), flags: u8) {
    commands.spawn((
        Unlaid { from, to, flags },
        Transform::default(),
        DespawnOnExit(AppState::InGame),
    ));
}

/// A segment that lives: its system, its length and axes in the models' frame, and its meshes,
/// its second set's where its variant has one.
#[derive(Component)]
struct Segment {
    look: LaserSegment,
    length: f32,
    axes: [[f32; 3]; 3],
    mesh: Handle<Mesh>,
    second: Option<Handle<Mesh>>,
}

/// A beam's segment as a save keeps it (#3774): its look, its length and axes in the models'
/// frame, and its start; a load makes it again there, going on as it went.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedSegment {
    look: LaserSegment,
    length: f32,
    axes: [[f32; 3]; 3],
    at: Vec3,
}

/// The beams' segments in `world` as a save keeps them (#3774).
pub(crate) fn saved(world: &mut World) -> Vec<SavedSegment> {
    let mut segments = world.query::<(&Segment, &Transform)>();
    segments
        .iter(world)
        .map(|(segment, at)| SavedSegment {
            look: segment.look.clone(),
            length: segment.length,
            axes: segment.axes,
            at: at.translation,
        })
        .collect()
}

/// Makes a loaded save's beams' segments again (#3774), with no sound.
pub(crate) fn again(world: &mut World, segments: Vec<SavedSegment>) {
    if segments.is_empty() {
        return;
    }
    if let Err(e) = world.run_system_cached_with(segments_again, segments) {
        warn!("beams: the saved segments are not made: {e}");
    }
}

/// Makes the saved segments `segments` again ([`again`]).
fn segments_again(
    In(segments): In<Vec<SavedSegment>>,
    mut commands: Commands,
    (data, assets): (Option<Res<Data>>, Res<AssetServer>),
    (mut made, mut effects, mut meshes): (
        ResMut<Materials>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for saved in segments {
        let variant = saved.look.variant();
        let flows = LaserSegment::flows(variant);
        let mut material_of =
            |texture| made.of(texture, flows, (data.as_deref(), &assets, &mut effects));
        let material = material_of(LaserSegment::texture(variant));
        let second = LaserSegment::second_texture(variant).map(&mut material_of);
        spawn_segment(
            &mut commands,
            &mut meshes,
            (material, second),
            (saved.look, saved.length, saved.axes, saved.at),
        );
    }
}

/// Makes a segment of `look`, of its length and axes, at `start`: its light on the ground at the
/// height 0 under its start, whatever its height (`0x5AA26E`), going with it, and its meshes of
/// `material` and its second set's.
fn spawn_segment(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    (material, second): (Option<Handle<Effect>>, Option<Option<Handle<Effect>>>),
    (look, length, axes, start): (LaserSegment, f32, [[f32; 3]; 3], Vec3),
) {
    let mesh = meshes.add(empty());
    let second_mesh = second.as_ref().map(|_| meshes.add(empty()));
    let mut segment = commands.spawn((
        Segment {
            look,
            length,
            axes,
            mesh: mesh.clone(),
            second: second_mesh.clone(),
        },
        Transform::from_translation(start),
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
    segment.with_child((
        Transform::from_xyz(0.0, -start.y, 0.0),
        crate::daynight::Light(LASER_LIGHT),
    ));
    let sets = [(material, Some(mesh)), (second.flatten(), second_mesh)];
    for (material, mesh) in sets {
        if let (Some(material), Some(mesh)) = (material, mesh) {
            segment.with_child((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
            ));
        }
    }
}

impl Materials {
    /// The material of `texture`, made as it is first asked, repeated along its `u` where it
    /// flows; none where the texture is missing.
    fn of(
        &mut self,
        texture: &'static str,
        flows: bool,
        (data, assets, effects): (Option<&Data>, &AssetServer, &mut Assets<Effect>),
    ) -> Option<Handle<Effect>> {
        if let Some((_, material)) = self.0.iter().find(|(name, _)| *name == texture) {
            return material.clone();
        }
        let image = data.and_then(|data| {
            let load = if flows {
                repeated_texture
            } else {
                particle_texture
            };
            load(&data.0.assets, assets, texture)
        });
        if image.is_none() {
            warn!("beams: no texture {texture}; its beams are not drawn");
        }
        let material = image.map(|image| effects.add(whole_material(image)));
        self.0.push((texture, material.clone()));
        material
    }
}

/// The beams' materials by their textures, made as the first is asked; none where a texture is
/// missing.
#[derive(Resource, Default)]
struct Materials(Vec<(&'static str, Option<Handle<Effect>>)>);

/// Lays each beam asked (`0x604030`): its ends found, the level its first creature stands on (the
/// hero's or a creature's that still stands in the way),
/// its segments laid from the start, each a system of its own at its start, until a cell it may
/// not pass unless it goes [`THROUGH`], each playing its variant's sound.
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
        let (variant, through) = (beam.flags & !THROUGH, beam.flags & THROUGH != 0);
        let flows = LaserSegment::flows(variant);
        let mut material_of =
            |texture| made.of(texture, flows, (data.as_deref(), &assets, &mut effects));
        let material = material_of(LaserSegment::texture(variant));
        let second = LaserSegment::second_texture(variant).map(&mut material_of);
        let starts = ways::laser_places((from.x, from.z), (to.x, to.z));
        debug!(
            "beams: a beam of {} from {from} to {to} on {level:?} in {} segments",
            variant,
            starts.len()
        );
        let n = u16::try_from(starts.len()).unwrap_or(u16::MAX);
        let step = (to - from) / f32::from(n.max(1));
        let along = models(step);
        let axes = laser_axes(along);
        let length = along.iter().map(|v| v * v).sum::<f32>().sqrt();
        for (k, &(x, z)) in (0_u16..).zip(&starts) {
            if !through && !heights.passes(crate::paths::cell_of(x, z), level) {
                break;
            }
            let start = from + step * f32::from(k);
            spawn_segment(
                &mut commands,
                &mut meshes,
                (material.clone(), second.clone()),
                (LaserSegment::new(variant, (k, n)), length, axes, start),
            );
            if let Some(sound) = LaserSegment::sound(variant) {
                sounds.write(crate::sfx::PlayAt {
                    sound: crate::sfx::Of::Id(sound),
                    point: start,
                    rate: (0.5, crate::sfx::Roll::None),
                    factor: 1.0,
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
        let order = orders.at(start.translation);
        let sets = [
            (
                Some(segment.look.quads(segment.length)),
                Some(&segment.mesh),
            ),
            (
                segment.look.second_quads(segment.length),
                segment.second.as_ref(),
            ),
        ];
        for (set, mesh) in sets {
            let (Some(set), Some(mesh)) = (set, mesh) else {
                continue;
            };
            let mut quads = Quads::default();
            for (corners, uvs) in set {
                quads.push_corners(corners.map(at), uvs.map(Vec2::from), colour);
            }
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                quads.in_order(order, start.translation);
                quads.into_mesh(&mut mesh);
            }
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
