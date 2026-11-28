//! Repairs of the models' data (docs/02 §4.2, §4.6) that `models` makes as it writes the records
//! they name, each as narrow as its defect (CLAUDE.md, "Code conventions"). A named record whose
//! data is not what its repair expects is left as it is, with a warning.

use opensacred_formats::grn::{Bone, Influence, Mesh, Model, Skeleton};

/// What [`apply`] did to a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// No repair names the record.
    None,
    /// Repaired: the number of positions whose bones changed, none for a repair of the sockets.
    Repaired(usize),
    /// A record that a repair names, whose data is not what the repair expects: left as it is.
    Mismatch(String),
}

/// A repair of a record's skeleton and model: the number of positions whose bones it changed, or
/// why the data is not what it expects, before it changes anything.
type Repair = fn(&mut Skeleton, &mut Model) -> Result<usize, String>;

/// The repairs by the names of the records they repair.
const REPAIRS: [(&[&[u8]], Repair); 4] = [
    (&[EARTH_AMULET], earth_amulet),
    (&[CANNON], cannon_strap),
    (&DRAGONS, dragon_tail),
    (&[CLAWS], claw_sockets),
];

/// Repairs `skeleton` and `model` of the model record `name` if a repair names it; the record
/// name is compared without regard to case, as the command line's `--model` is.
pub fn apply(name: &[u8], skeleton: &mut Skeleton, model: &mut Model) -> Outcome {
    let Some((_, repair)) = REPAIRS
        .iter()
        .find(|(names, _)| names.iter().any(|n| name.eq_ignore_ascii_case(n)))
    else {
        return Outcome::None;
    };
    match repair(skeleton, model) {
        Ok(positions) => Outcome::Repaired(positions),
        Err(reason) => Outcome::Mismatch(reason),
    }
}

/// `DWARF_AM_ERDE.GRN` (model 792), the dwarf's earth amulet, the only one of the 150 amulets
/// whose mesh follows a bone that its class's hero lacks: its mesh `Amulett_erd` is rigid on a
/// bone `Amulett_erd` under `__Root`, where the dwarf's 18 other amulets follow `Bip01 Spine3`
/// (#398, docs/02 §4.6). Its skeleton is also the only one of the 19 without the `Root` that
/// `DWARF.GRN` has: `Bip01` hangs under `__Root` with the transform it has under `Root` in the
/// others, so the file's bind pose lies 0.34 units off theirs, while its mesh lies where theirs
/// lie. The repair gives the skeleton their `Root` and binds the mesh rigidly to `Bip01 Spine3`,
/// its positions unchanged: worn by bone names, it then sits on the dwarf where its siblings sit
/// (docs/decisions/models.md, "Dwarf's earth amulet on the chest").
const EARTH_AMULET: &[u8] = b"DWARF_AM_ERDE.GRN";
/// The amulet's mesh and the bone it follows in the file.
const EARTH_AMULET_MESH: &[u8] = b"Amulett_erd";
const EARTH_AMULET_POSITIONS: usize = 46;
/// The bone the dwarf's other amulets follow.
const SPINE3: &[u8] = b"Bip01 Spine3";

/// The `Root` bone of the dwarf's 18 other amulets, bit for bit: a translation of
/// (−0.0418, −0.3351, 0), a turn of 1.5e-5 degrees about Y and no scale; `DWARF.GRN`'s has the
/// same translation and rotation.
fn root() -> Bone<'static> {
    Bone {
        name: b"Root",
        parent: Some(0),
        channel: 0,
        translation: [
            f32::from_bits(0xBD2B_2F1B),
            f32::from_bits(0xBEAB_910E),
            0.0,
        ],
        rotation: [0.0, f32::from_bits(0x340A_8279), 0.0, 1.0],
        scale_shear: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
    }
}

/// The [`Repair`] of [`EARTH_AMULET`].
fn earth_amulet(skeleton: &mut Skeleton, model: &mut Model) -> Result<usize, String> {
    let bones = &skeleton.bones;
    if bones.iter().any(|b| b.name == b"Root") {
        return Err("the skeleton has a `Root` already".to_owned());
    }
    let bip01 = only_bone(skeleton, b"Bip01")?;
    if bones[bip01].parent != Some(0) {
        return Err("`Bip01` does not hang under the root".to_owned());
    }
    let spine3 = only_bone(skeleton, SPINE3)?;
    let index = only_mesh(model, EARTH_AMULET_MESH)?;
    let mesh = &model.meshes[index];
    let pivot = match mesh.bones.as_slice() {
        [only] if mesh.weights.is_empty() => only.bone,
        _ => return Err("the mesh `Amulett_erd` is not rigid".to_owned()),
    };
    if mesh.positions.len() != EARTH_AMULET_POSITIONS {
        return Err(format!(
            "the mesh `Amulett_erd` has {} positions, not {EARTH_AMULET_POSITIONS}",
            mesh.positions.len()
        ));
    }
    if bones[pivot].name != EARTH_AMULET_MESH || bones[pivot].parent != Some(0) {
        return Err(
            "the mesh `Amulett_erd` does not follow a bone `Amulett_erd` under the root".to_owned(),
        );
    }

    // `Root` becomes bone 1, the root's first child as in the other amulets, and `Bip01`'s
    // parent; the later bones move up by one. Its channel is one that no other bone has, so that
    // no track plays on it.
    let moved = |bone: usize| if bone == 0 { 0 } else { bone + 1 };
    let channel = bones.iter().map(|b| b.channel + 1).max().unwrap_or(0);
    for bone in &mut skeleton.bones {
        bone.parent = bone.parent.map(moved);
    }
    skeleton.bones[bip01].parent = Some(1);
    skeleton.bones.insert(1, Bone { channel, ..root() });
    for mesh in &mut model.meshes {
        for bone in &mut mesh.bones {
            bone.bone = moved(bone.bone);
        }
    }
    // The bounds of the entry, which the export does not read, stay those of the pivot.
    model.meshes[index].bones[0].bone = moved(spine3);
    Ok(EARTH_AMULET_POSITIONS)
}

/// `DWARF_LEATHER_CANNON.GRN` (model 1415), the cannon of item 3015: five of the 43 positions of
/// its strap `shoulder_leather01`, at shoulder height, are wholly on `Bip01 Pelvis`, 17 to 23
/// units below them, among positions on `Bip01 L Clavicle` and `Bip01 Neck`; worn on the dwarf,
/// the strap's sides from them stretch up to 22 times in the dwarf's clips (#399, docs/02 §4.6).
/// The repair weights them wholly on `Bip01 L Clavicle`, the bone of most of the strap's positions
/// around them (docs/decisions/models.md, "Dwarf's cannon strap on the clavicle").
const CANNON: &[u8] = b"DWARF_LEATHER_CANNON.GRN";
const STRAP: &[u8] = b"shoulder_leather01";
const STRAP_POSITIONS: usize = 43;
/// The strap's positions on the pelvis, by index, and where they lie in the file, to 0.001.
const ON_PELVIS: [(usize, [f32; 3]); 5] = [
    (3, [10.970, 8.466, 0.356]),
    (17, [6.121, 9.165, 1.274]),
    (26, [7.480, -7.085, 2.911]),
    (27, [6.319, 8.329, 5.694]),
    (32, [11.314, 8.811, 1.120]),
];
const PELVIS: &[u8] = b"Bip01 Pelvis";
const CLAVICLE: &[u8] = b"Bip01 L Clavicle";

/// The [`Repair`] of [`CANNON`].
fn cannon_strap(skeleton: &mut Skeleton, model: &mut Model) -> Result<usize, String> {
    let index = only_mesh(model, STRAP)?;
    let mesh = &model.meshes[index];
    if mesh.positions.len() != STRAP_POSITIONS || mesh.weights.len() != STRAP_POSITIONS {
        return Err(format!(
            "the strap has {} positions and {} weighted ones, not {STRAP_POSITIONS}",
            mesh.positions.len(),
            mesh.weights.len()
        ));
    }
    let pelvis = local_bone(skeleton, mesh, PELVIS)?;
    let clavicle = local_bone(skeleton, mesh, CLAVICLE)?;
    let on_pelvis: Vec<usize> = mesh
        .weights
        .iter()
        .enumerate()
        .filter(|(_, list)| list.iter().any(|i| i.bone == pelvis))
        .map(|(position, _)| position)
        .collect();
    let expected = ON_PELVIS.map(|(position, _)| position);
    if on_pelvis != expected {
        return Err(format!(
            "positions {on_pelvis:?} of the strap are on `Bip01 Pelvis`, not {expected:?}"
        ));
    }
    let wholly = |bone| vec![Influence { bone, weight: 1.0 }];
    for (position, at) in ON_PELVIS {
        if mesh.weights[position] != wholly(pelvis) {
            return Err(format!(
                "position {position} of the strap is not wholly on `Bip01 Pelvis`"
            ));
        }
        if !lies_at(mesh.positions[position], at) {
            return Err(format!(
                "position {position} of the strap does not lie at {at:?}"
            ));
        }
    }
    for (position, _) in ON_PELVIS {
        model.meshes[index].weights[position] = wholly(clavicle);
    }
    Ok(ON_PELVIS.len())
}

/// The eight large dragons, `DRAGON.GRN` to `DRAGON_UNDEAD.GRN` (models 1193 to 1200), share one
/// mesh `Dragon` whose tail's root is weighted wholly to `Bip01 L Thigh` or `Bip01 R Thigh` on its
/// sides and underside, next to positions wholly on `Bip01 Tail`: the most stretched sides of the
/// mesh in each of their three sets of clips, 4.7 times in the walk and 10.6 in the dying clip
/// (#400, docs/02 §4.6). The repair blends the tail into the thighs over two rings of positions
/// wholly on a thigh: those that share a side with a position wholly on `Bip01 Tail` take 2/3 of
/// the tail and 1/3 of their thigh, those that share a side with the first ring 1/3 of the tail
/// (docs/decisions/models.md, "Large dragons' tails blended into the thighs").
const DRAGONS: [&[u8]; 8] = [
    b"DRAGON.GRN",
    b"DRAGON_ICE.GRN",
    b"DRAGON_ICE01.GRN",
    b"DRAGON_ICE03.GRN",
    b"DRAGON_LOROMIR.GRN",
    b"DRAGON_SIRITHCAM.GRN",
    b"DRAGON_SWAMP02.GRN",
    b"DRAGON_UNDEAD.GRN",
];
const DRAGON_MESH: &[u8] = b"Dragon";
const DRAGON_POSITIONS: usize = 1634;
/// Positions of a ring and where they lie in the file, to 0.001.
type Ring = [(usize, [f32; 3]); 6];
/// The rings that the rule finds in the mesh, each with the share of `Bip01 Tail` it takes: four
/// positions of the left side and two of the right in the first, three of each in the second,
/// since the right's position mirroring the left's 225, wholly on the tail, is half on its thigh
/// already.
const TAIL_RINGS: [(Ring, f32); 2] = [
    (
        [
            (152, [13.976, 70.280, 79.967]),
            (233, [21.518, 88.236, 88.518]),
            (585, [40.994, 85.062, 84.519]),
            (591, [31.651, 89.070, 78.442]),
            (1034, [-13.979, 69.774, 80.246]),
            (1084, [-21.519, 88.263, 88.534]),
        ],
        2.0 / 3.0,
    ),
    (
        [
            (149, [21.184, 67.807, 85.548]),
            (576, [47.705, 72.554, 86.160]),
            (586, [24.058, 74.183, 69.788]),
            (1031, [-21.185, 67.837, 85.540]),
            (1325, [-24.061, 74.231, 69.786]),
            (1330, [-31.653, 89.109, 78.458]),
        ],
        1.0 / 3.0,
    ),
];
const TAIL: &[u8] = b"Bip01 Tail";
const THIGHS: [&[u8]; 2] = [b"Bip01 L Thigh", b"Bip01 R Thigh"];

/// The [`Repair`] of [`DRAGONS`].
fn dragon_tail(skeleton: &mut Skeleton, model: &mut Model) -> Result<usize, String> {
    let index = only_mesh(model, DRAGON_MESH)?;
    let mesh = &model.meshes[index];
    if mesh.positions.len() != DRAGON_POSITIONS || mesh.weights.len() != DRAGON_POSITIONS {
        return Err(format!(
            "the mesh `Dragon` has {} positions and {} weighted ones, not {DRAGON_POSITIONS}",
            mesh.positions.len(),
            mesh.weights.len()
        ));
    }
    let tail = local_bone(skeleton, mesh, TAIL)?;
    let thighs = [
        local_bone(skeleton, mesh, THIGHS[0])?,
        local_bone(skeleton, mesh, THIGHS[1])?,
    ];
    let wholly_on = |position: usize, bones: &[usize]| match mesh.weights[position].as_slice() {
        [only] => bones.contains(&only.bone),
        _ => false,
    };
    let mut sides = vec![Vec::new(); mesh.positions.len()];
    for triangle in &mesh.triangles {
        let [a, b, c] = triangle.positions;
        for (u, v) in [(a, b), (b, c), (c, a)] {
            sides[u].push(v);
            sides[v].push(u);
        }
    }
    // The first ring borders on the tail, the second on the first.
    let ring = |next_to: &dyn Fn(usize) -> bool, not_in: &[usize]| -> Vec<usize> {
        (0..mesh.positions.len())
            .filter(|&p| wholly_on(p, &thighs) && !not_in.contains(&p))
            .filter(|&p| sides[p].iter().any(|&q| next_to(q)))
            .collect()
    };
    let first = ring(&|q| wholly_on(q, &[tail]), &[]);
    let second = ring(&|q| first.contains(&q), &first);
    for (found, (expected, _)) in [&first, &second].into_iter().zip(&TAIL_RINGS) {
        let positions = expected.map(|(position, _)| position);
        if *found != positions {
            return Err(format!(
                "the tail's rings hold the positions {first:?} and {second:?}, not those expected"
            ));
        }
        for (position, at) in expected {
            if !lies_at(mesh.positions[*position], *at) {
                return Err(format!(
                    "position {position} of the mesh `Dragon` does not lie at {at:?}"
                ));
            }
        }
    }
    let weights = &mut model.meshes[index].weights;
    for (ring, share) in TAIL_RINGS {
        for (position, _) in ring {
            let thigh = weights[position][0].bone;
            weights[position] = vec![
                Influence {
                    bone: tail,
                    weight: share,
                },
                Influence {
                    bone: thigh,
                    weight: 1.0 - share,
                },
            ];
        }
    }
    Ok(TAIL_RINGS.iter().map(|(ring, _)| ring.len()).sum())
}

/// `ORCHIDEE.GRN` (model 1492), the claws of item 1773 and of the Daemon's starting 7908 (through
/// 7803), a gauntlet: a plate over the back of the hand, a grip bar across the palm under it and
/// four blades ahead. Its sockets stand a quarter turn about their X axis, the blades', off every
/// other hand weapon's: in its socket's frame, the hand's, the fan of its blades spreads across
/// the palm where the brass knuckles of the same subclass span the line of the knuckles, as a
/// sword's blade runs along it (#3926, docs/02 §4.2). They also stand at the plate's back, so that
/// the grip lay under the Daemon's fingers and off the middle of her knuckles, where a sword's
/// grip runs through the socket's origin (#3929). The repair turns each socket by −90° about its
/// own X axis, so that the model turns +90° about its blades in either hand, the left socket
/// keeping its 167° from the right as every weapon's does, and moves it so that the grip's middle
/// lies on her hand's grip, at the middle of her four knuckles across her hand
/// (docs/decisions/models.md, "Daemon's claws along the knuckles").
const CLAWS: &[u8] = b"ORCHIDEE.GRN";

/// A socket of the claws, a bone under the root turned about X alone.
struct ClawSocket {
    name: &'static [u8],
    /// Its translation in the file, to 1e-3, and its rotation, to 1e-5.
    at: [f32; 3],
    rotation: [f32; 4],
    /// Its translation repaired: the place that puts the grip bar's middle, in its frame turned,
    /// on the origin of the Daemon's socket of the same name, the grip of her hand, at the middle
    /// of her four knuckles across it (`Bip01 R Finger1` to `4` at 0.965 along her
    /// `Bone_weapon_01`'s Y in her clips of bare hands, `Bip01 L Finger1` to `4` at 0.065 along
    /// her `Bone_weapon_02`'s): the bar, from −4.835 to −2.192 along X, −1.813 to 3.287 across
    /// and −1.389 to −0.628 deep in the file, the plate then lies on the back of her hand from
    /// her wrist to the middle of her fingers.
    moved_to: [f32; 3],
}

/// The right hand's socket at 5° about X and the left's at 172°.
const CLAW_SOCKETS: [ClawSocket; 2] = [
    ClawSocket {
        name: b"Bone_weapon_01",
        at: [-6.246, -0.825, -0.058],
        rotation: [0.043_619, 0.0, 0.0, 0.999_048],
        moved_to: [-3.513_1, -1.699_5, -1.103_7],
    },
    ClawSocket {
        name: b"Bone_weapon_02",
        at: [-6.246, 0.825, -0.058],
        rotation: [0.997_564, 0.0, 0.0, 0.069_756],
        moved_to: [-3.513_1, -0.671_4, -1.033_9],
    },
];
/// The turn by −90° about X that each socket takes after its own rotation.
const QUARTER_BACK: [f32; 4] = [
    -std::f32::consts::FRAC_1_SQRT_2,
    0.0,
    0.0,
    std::f32::consts::FRAC_1_SQRT_2,
];

/// The [`Repair`] of [`CLAWS`].
fn claw_sockets(skeleton: &mut Skeleton, _: &mut Model) -> Result<usize, String> {
    let mut sockets = Vec::new();
    for socket in &CLAW_SOCKETS {
        let name = socket.name.escape_ascii();
        let index = only_bone(skeleton, socket.name)?;
        let bone = &skeleton.bones[index];
        if bone.parent != Some(0) {
            return Err(format!("the socket `{name}` does not hang under the root"));
        }
        if !within(&bone.translation, &socket.at, 1e-3) {
            return Err(format!(
                "the socket `{name}` does not lie at {:?}",
                socket.at
            ));
        }
        if !within(&bone.rotation, &socket.rotation, 1e-5) {
            return Err(format!(
                "the socket `{name}` is not turned by {:?}",
                socket.rotation
            ));
        }
        sockets.push((index, socket));
    }
    for (index, socket) in sockets {
        let bone = &mut skeleton.bones[index];
        bone.rotation = compose(bone.rotation, QUARTER_BACK);
        bone.translation = socket.moved_to;
    }
    Ok(0)
}

/// Whether each of `values` lies within `tolerance` of `expected`'s.
fn within(values: &[f32], expected: &[f32], tolerance: f32) -> bool {
    values
        .iter()
        .zip(expected)
        .all(|(a, b)| (a - b).abs() <= tolerance)
}

/// The rotation `a · b` of two quaternions `[x, y, z, w]`: `b` first, in `a`'s frame.
fn compose([ax, ay, az, aw]: [f32; 4], [bx, by, bz, bw]: [f32; 4]) -> [f32; 4] {
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

/// The mesh's own index of the skeleton bone `name`, its local bone (docs/02 §4.2).
fn local_bone(skeleton: &Skeleton, mesh: &Mesh, name: &[u8]) -> Result<usize, String> {
    let mut found = mesh
        .bones
        .iter()
        .enumerate()
        .filter(|(_, b)| {
            skeleton
                .bones
                .get(b.bone)
                .is_some_and(|bone| bone.name == name)
        })
        .map(|(local, _)| local);
    match (found.next(), found.next()) {
        (Some(local), None) => Ok(local),
        _ => Err(format!(
            "the mesh `{}` is not bound once to `{}`",
            mesh.name.escape_ascii(),
            name.escape_ascii()
        )),
    }
}

/// Whether `position` lies at `at` within 0.001 in each coordinate.
fn lies_at(position: [f32; 3], at: [f32; 3]) -> bool {
    position.iter().zip(at).all(|(a, b)| (a - b).abs() <= 1e-3)
}

/// The index of the only mesh named `name`.
fn only_mesh(model: &Model, name: &[u8]) -> Result<usize, String> {
    let mut found = model
        .meshes
        .iter()
        .enumerate()
        .filter(|(_, m)| m.name == name)
        .map(|(index, _)| index);
    match (found.next(), found.next()) {
        (Some(index), None) => Ok(index),
        (None, _) => Err(format!("no mesh `{}`", name.escape_ascii())),
        (Some(_), Some(_)) => Err(format!("two meshes `{}`", name.escape_ascii())),
    }
}

/// The index of the only bone named `name`.
fn only_bone(skeleton: &Skeleton, name: &[u8]) -> Result<usize, String> {
    let mut found = skeleton
        .bones
        .iter()
        .enumerate()
        .filter(|(_, b)| b.name == name)
        .map(|(index, _)| index);
    match (found.next(), found.next()) {
        (Some(index), None) => Ok(index),
        (None, _) => Err(format!("no bone `{}`", name.escape_ascii())),
        (Some(_), Some(_)) => Err(format!("two bones `{}`", name.escape_ascii())),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use opensacred_formats::grn::{
        Material, Mesh, MeshBone, Pass, PassTriangle, Source, Surface, Triangle,
    };
    use opensacred_formats::mdl::MdlPak;

    use super::*;
    use crate::gltf;
    use crate::models::{self, TextureIndex};
    use crate::testing::{Glb, scratch_dir};
    use crate::verify::read_game_file;

    const IDENTITY: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

    fn bone(name: &'static [u8], parent: Option<usize>, translation: [f32; 3]) -> Bone<'static> {
        Bone {
            name,
            parent,
            channel: 0,
            translation,
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale_shear: IDENTITY,
        }
    }

    fn entry(bone: usize) -> MeshBone {
        MeshBone {
            bone,
            radius: 1.0,
            min: [0.0; 3],
            max: [0.0; 3],
        }
    }

    /// A rigid mesh of `positions` positions on `bone`, one triangle drawn by one pass.
    fn rigid(name: &'static [u8], positions: usize, bone: usize) -> Mesh<'static> {
        Mesh {
            name,
            positions: (0..positions)
                .map(|k| [f32::from(u16::try_from(k).unwrap()), 0.0, 1.0])
                .collect(),
            normals: vec![[0.0, -1.0, 0.0]],
            texcoord_sets: vec![],
            triangles: vec![Triangle {
                positions: [0, 1, 2],
                normals: [0, 0, 0],
            }],
            weights: vec![],
            bones: vec![entry(bone)],
        }
    }

    /// The bones of the earth amulet's file in short: no `Root`, the pivot `Amulett_erd` and
    /// `Bip01` under `__Root`.
    fn amulet_skeleton() -> Skeleton<'static> {
        let mut bones = vec![
            bone(b"__Root", None, [0.0; 3]),
            bone(b"Amulett_erd", Some(0), [0.0, -1.3, -0.9]),
            bone(b"Bip01", Some(0), [0.0, 6.9, -20.8]),
            bone(b"Bip01 Spine3", Some(2), [0.0, -0.8, 11.7]),
            bone(b"Bip01 Neck", Some(3), [0.0, -0.5, 11.8]),
        ];
        for (channel, bone) in bones.iter_mut().enumerate() {
            bone.channel = channel;
        }
        bones[3].rotation = [0.5, 0.5, 0.5, 0.5];
        Skeleton { bones }
    }

    /// The amulet's mesh on its pivot and a collar on `Bip01 Neck`.
    fn amulet_model() -> Model<'static> {
        let pass = |mesh| Pass {
            mesh,
            material: 0,
            color: Source::Constant([1.0; 3]),
            texcoords: Source::Constant([0.0; 3]),
            triangles: vec![PassTriangle {
                triangle: 0,
                color: None,
                texcoords: None,
            }],
        };
        Model {
            meshes: vec![rigid(b"Collar", 3, 4), rigid(b"Amulett_erd", 46, 1)],
            textures: vec![],
            materials: vec![Material {
                name: Some(b"gem"),
                surface: Surface::Color([1.0, 0.0, 0.0, 1.0]),
            }],
            passes: vec![pass(0), pass(1)],
        }
    }

    #[test]
    fn the_earth_amulet_gets_a_root_and_follows_spine3() {
        let mut skeleton = amulet_skeleton();
        let mut model = amulet_model();
        let before = model.clone();
        let outcome = apply(b"dwarf_am_erde.grn", &mut skeleton, &mut model);
        assert_eq!(outcome, Outcome::Repaired(46));

        let names: Vec<&[u8]> = skeleton.bones.iter().map(|b| b.name).collect();
        let expected: [&[u8]; 6] = [
            b"__Root",
            b"Root",
            b"Amulett_erd",
            b"Bip01",
            b"Bip01 Spine3",
            b"Bip01 Neck",
        ];
        assert_eq!(names, expected);
        let parents: Vec<_> = skeleton.bones.iter().map(|b| b.parent).collect();
        assert_eq!(parents, [None, Some(0), Some(0), Some(1), Some(3), Some(4)]);
        assert_eq!(
            skeleton.bones[1],
            Bone {
                channel: 5,
                ..root()
            }
        );
        assert_eq!(
            skeleton.bones[1].translation,
            [-0.041_792_97, -0.335_091_05, 0.0]
        );
        // The amulet follows Spine3 rigidly and the collar still its neck; no position moves.
        assert_eq!(model.meshes[1].bones, [entry(4)]);
        assert_eq!(model.meshes[0].bones, [entry(5)]);
        for (mesh, old) in model.meshes.iter().zip(&before.meshes) {
            assert_eq!(mesh.positions, old.positions);
            assert!(mesh.weights.is_empty());
        }
    }

    /// Worn by bone names, a mesh sits where the inverse bind matrix of its joint puts it: the
    /// repaired amulet's is that of a sibling's `Bip01 Spine3`, whose skeleton has the `Root`.
    #[test]
    fn the_repaired_amulet_has_its_siblings_bind_matrix() {
        let mut skeleton = amulet_skeleton();
        let mut model = amulet_model();
        apply(EARTH_AMULET, &mut skeleton, &mut model);
        let mut sibling = amulet_skeleton();
        sibling.bones.insert(1, root());
        for bone in &mut sibling.bones[2..] {
            bone.parent = bone.parent.map(|p| if p == 0 { 0 } else { p + 1 });
        }
        sibling.bones[3].parent = Some(1);
        let mut sibling_model = amulet_model();
        sibling_model.meshes[0].bones = vec![entry(5)];
        sibling_model.meshes[1].bones = vec![entry(4)];

        let skin = |skeleton: &Skeleton, model: &Model| {
            let (data, _) = gltf::export(skeleton, model, &[], &[], |_| None).unwrap();
            let glb = Glb::read(&data);
            let skin = &glb.json["skins"][1];
            let joint = glb.field(&skin["joints"][0]);
            let name = glb.json["nodes"][joint]["name"].clone();
            (name, glb.floats(&skin["inverseBindMatrices"]))
        };
        let (name, matrix) = skin(&skeleton, &model);
        let (sibling_name, sibling_matrix) = skin(&sibling, &sibling_model);
        assert_eq!(
            (name.as_str(), sibling_name.as_str()),
            (Some("Bip01 Spine3"), Some("Bip01 Spine3"))
        );
        assert_eq!(matrix, sibling_matrix);
    }

    #[test]
    fn an_amulet_unlike_the_expected_one_is_left_as_it_is() {
        let mut cases: Vec<(Skeleton, Model, &str)> = Vec::new();
        let mut skeleton = amulet_skeleton();
        skeleton.bones[1].name = b"Root";
        cases.push((skeleton, amulet_model(), "has a `Root` already"));
        let mut model = amulet_model();
        model.meshes[1].positions.pop();
        cases.push((amulet_skeleton(), model, "45 positions"));
        let mut model = amulet_model();
        model.meshes[1].bones[0].bone = 4;
        cases.push((
            amulet_skeleton(),
            model,
            "does not follow a bone `Amulett_erd`",
        ));
        let mut skeleton = amulet_skeleton();
        skeleton.bones[3].name = b"Bip01 Spine2";
        cases.push((skeleton, amulet_model(), "no bone `Bip01 Spine3`"));
        for (mut skeleton, mut model, reason) in cases {
            let (old_skeleton, old_model) = (skeleton.clone(), model.clone());
            match apply(EARTH_AMULET, &mut skeleton, &mut model) {
                Outcome::Mismatch(found) => assert!(found.contains(reason), "{found}"),
                other => panic!("{other:?}"),
            }
            assert_eq!((skeleton, model), (old_skeleton, old_model));
        }
        // Another record is not touched.
        let mut skeleton = amulet_skeleton();
        let mut model = amulet_model();
        assert_eq!(
            apply(b"DWARF_AM_FEUER.GRN", &mut skeleton, &mut model),
            Outcome::None
        );
        assert_eq!((skeleton, model), (amulet_skeleton(), amulet_model()));
    }

    /// Model 792 against its siblings in the game files: the `Root` constants are theirs and the
    /// dwarf's, and the converted amulet follows `Bip01 Spine3` with the inverse bind matrix of
    /// their `Bip01 Spine3` and its positions unchanged.
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn the_earth_amulet_follows_spine3_like_its_siblings() {
        let game = opensacred_locate::original();
        let (pak, dir_name) = models::PAKS[0];
        let data = read_game_file(&game.join(pak)).unwrap();
        let mdl = MdlPak::parse(&data).unwrap();
        let mut amulet = None;
        let mut siblings = 0;
        for index in 0..mdl.models() {
            let record = mdl.record(index).unwrap();
            let name = record.name().to_ascii_uppercase();
            if !(name.starts_with(b"DWARF_AM_") || name == b"DWARF.GRN") {
                continue;
            }
            let grn = record.parse_grn().unwrap().unwrap();
            let skeleton = grn.skeleton().unwrap();
            if name == EARTH_AMULET {
                amulet = Some((index, grn.model().unwrap().meshes[0].positions.clone()));
                continue;
            }
            let found = &skeleton.bones[only_bone(&skeleton, b"Root").unwrap()];
            let expected = root();
            assert_eq!(found.translation, expected.translation, "{index}");
            assert_eq!(found.rotation, expected.rotation, "{index}");
            if name != b"DWARF.GRN" {
                assert_eq!(found.scale_shear, expected.scale_shear, "{index}");
                siblings += 1;
            }
        }
        assert_eq!(siblings, 18);
        let (index, positions) = amulet.unwrap();
        assert_eq!(index, 792);

        let out = scratch_dir("earth-amulet");
        let dst = models::output_dir(&out, dir_name);
        let only = ["DWARF_AM_ERDE.GRN", "DWARF_AM_FEUER.GRN"].map(str::to_owned);
        let summary =
            models::convert_pak(&game.join(pak), &dst, &TextureIndex::default(), &only).unwrap();
        assert_eq!(
            (summary.repaired_models, summary.repaired_positions),
            (1, 46)
        );
        assert!(summary.warnings.is_empty(), "{:?}", summary.warnings);
        let read = |file: &str| Glb::read(&fs::read(dst.join(file)).unwrap());
        let amulet = read("0792-dwarf_am_erde.glb");
        let sibling = read("0793-dwarf_am_feuer.glb");
        let skin = |glb: &Glb| {
            let skin = &glb.json["skins"][0];
            let joints = skin["joints"].as_array().unwrap();
            assert_eq!(joints.len(), 1);
            let node = &glb.json["nodes"][glb.field(&joints[0])];
            assert_eq!(node["name"], "Bip01 Spine3");
            glb.floats(&skin["inverseBindMatrices"])
        };
        let (matrix, sibling_matrix) = (skin(&amulet), skin(&sibling));
        for (a, b) in matrix.iter().zip(&sibling_matrix) {
            assert!((a - b).abs() < 1e-5, "{matrix:?} != {sibling_matrix:?}");
        }
        // The positions are the file's, turned into glTF's frame; every vertex follows joint 0.
        let attributes = &amulet.json["meshes"][0]["primitives"][0]["attributes"];
        let written = amulet.floats(&attributes["POSITION"]);
        for vertex in written.chunks(3) {
            let [x, y, z] = [vertex[0], vertex[1], vertex[2]];
            assert!(positions.contains(&[x, -z, y]), "{vertex:?}");
        }
        assert!(amulet.ints(&attributes["JOINTS_0"]).iter().all(|&j| j == 0));
        fs::remove_dir_all(out).unwrap();
    }

    /// The cannon's bones in short and its strap: 43 positions, the five of [`ON_PELVIS`] on
    /// `Bip01 Pelvis`, the others on `Bip01 Neck` (the first ten) or `Bip01 L Clavicle`, one of
    /// them shared with `Bip01 L UpperArm`.
    fn cannon() -> (Skeleton<'static>, Model<'static>) {
        let skeleton = Skeleton {
            bones: vec![
                bone(b"__Root", None, [0.0; 3]),
                bone(b"Bip01", Some(0), [0.0, 0.0, -17.0]),
                bone(b"Bip01 Pelvis", Some(1), [0.0; 3]),
                bone(b"Bip01 Neck", Some(2), [0.0, -1.0, 23.5]),
                bone(b"Bip01 L Clavicle", Some(3), [5.6, 1.4, -2.8]),
                bone(b"Bip01 L UpperArm", Some(4), [6.0, 0.0, 0.0]),
            ],
        };
        let mut strap = rigid(b"shoulder_leather01", STRAP_POSITIONS, 0);
        strap.bones = [3, 4, 2, 5].map(entry).to_vec();
        let one = |bone| vec![Influence { bone, weight: 1.0 }];
        strap.weights = (0..STRAP_POSITIONS)
            .map(|k| if k < 10 { one(0) } else { one(1) })
            .collect();
        strap.weights[12] = vec![
            Influence {
                bone: 1,
                weight: 0.53,
            },
            Influence {
                bone: 3,
                weight: 0.47,
            },
        ];
        for (position, at) in ON_PELVIS {
            strap.positions[position] = at.map(|v| v + 0.000_2);
            strap.weights[position] = one(2);
        }
        let mut model = amulet_model();
        model.meshes = vec![rigid(b"Cannon", 3, 1), strap];
        (skeleton, model)
    }

    #[test]
    fn the_cannon_straps_positions_on_the_pelvis_go_to_the_clavicle() {
        let (mut skeleton, mut model) = cannon();
        let (old_skeleton, old_model) = (skeleton.clone(), model.clone());
        let outcome = apply(b"Dwarf_leather_cannon.grn", &mut skeleton, &mut model);
        assert_eq!(outcome, Outcome::Repaired(5));
        assert_eq!(skeleton, old_skeleton);
        assert_eq!(model.meshes[0], old_model.meshes[0]);
        let (strap, old) = (&model.meshes[1], &old_model.meshes[1]);
        assert_eq!(
            (&strap.positions, &strap.bones),
            (&old.positions, &old.bones)
        );
        for (position, (list, old_list)) in strap.weights.iter().zip(&old.weights).enumerate() {
            if ON_PELVIS.iter().any(|&(p, _)| p == position) {
                assert_eq!(
                    list,
                    &[Influence {
                        bone: 1,
                        weight: 1.0
                    }]
                );
            } else {
                assert_eq!(list, old_list);
            }
        }
    }

    #[test]
    fn a_strap_unlike_the_expected_one_is_left_as_it_is() {
        let mut cases: Vec<(Skeleton, Model, &str)> = Vec::new();
        let (skeleton, mut model) = cannon();
        model.meshes[1].weights[20] = vec![Influence {
            bone: 2,
            weight: 1.0,
        }];
        cases.push((skeleton, model, "positions [3, 17, 20, 26, 27, 32]"));
        let (skeleton, mut model) = cannon();
        model.meshes[1].weights[17][0].weight = 0.5;
        model.meshes[1].weights[17].push(Influence {
            bone: 1,
            weight: 0.5,
        });
        cases.push((skeleton, model, "position 17 of the strap is not wholly"));
        let (skeleton, mut model) = cannon();
        model.meshes[1].positions[26][2] += 0.01;
        cases.push((skeleton, model, "position 26 of the strap does not lie"));
        let (mut skeleton, model) = cannon();
        skeleton.bones[4].name = b"Bip01 R Clavicle";
        cases.push((skeleton, model, "not bound once to `Bip01 L Clavicle`"));
        let (skeleton, mut model) = cannon();
        model.meshes[1].positions.push([0.0; 3]);
        cases.push((skeleton, model, "44 positions"));
        let (skeleton, mut model) = cannon();
        model.meshes[1].name = b"shoulder_leather02";
        cases.push((skeleton, model, "no mesh `shoulder_leather01`"));
        for (mut skeleton, mut model, reason) in cases {
            let (old_skeleton, old_model) = (skeleton.clone(), model.clone());
            match apply(CANNON, &mut skeleton, &mut model) {
                Outcome::Mismatch(found) => assert!(found.contains(reason), "{found}"),
                other => panic!("{other:?}"),
            }
            assert_eq!((skeleton, model), (old_skeleton, old_model));
        }
    }

    /// Model 1415 in the game files: the converted strap has no vertex on `Bip01 Pelvis`, and its
    /// five positions that were follow `Bip01 L Clavicle` wholly.
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn the_cannon_strap_leaves_the_pelvis() {
        let game = opensacred_locate::original();
        let (pak, dir_name) = models::PAKS[0];
        let out = scratch_dir("cannon-strap");
        let dst = models::output_dir(&out, dir_name);
        let only = ["Dwarf_leather_cannon.grn".to_owned()];
        let summary =
            models::convert_pak(&game.join(pak), &dst, &TextureIndex::default(), &only).unwrap();
        assert_eq!(
            (summary.repaired_models, summary.repaired_positions),
            (1, 5)
        );
        assert!(summary.warnings.is_empty(), "{:?}", summary.warnings);
        let glb = Glb::read(&fs::read(dst.join("1415-dwarf_leather_cannon.glb")).unwrap());
        let mut moved = 0;
        for (at, influences) in vertices(&glb, "shoulder_leather01") {
            assert!(
                influences.iter().all(|(name, _)| name != "Bip01 Pelvis"),
                "{influences:?}"
            );
            if ON_PELVIS.iter().any(|&(_, p)| lies_at(at, p)) {
                assert_eq!(influences, [("Bip01 L Clavicle".to_owned(), 1.0)]);
                moved += 1;
            }
        }
        // A position is several vertices where its corners differ in normal or coordinates.
        assert!(moved >= ON_PELVIS.len(), "{moved}");
        fs::remove_dir_all(out).unwrap();
    }

    /// A vertex of a converted file: where it lies in the file's frame, and its influences by the
    /// name of their joint's node.
    type Vertex = ([f32; 3], Vec<(String, f32)>);

    /// The vertices of the mesh `name` of a converted file.
    fn vertices(glb: &Glb, name: &str) -> Vec<Vertex> {
        let meshes = glb.json["meshes"].as_array().unwrap();
        let mesh = meshes.iter().position(|m| m["name"] == name).unwrap();
        let node = glb.json["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["mesh"] == mesh)
            .unwrap();
        let skin = &glb.json["skins"][glb.field(&node["skin"])];
        let joint = |k: u32| {
            let node = glb.field(&skin["joints"][usize::try_from(k).unwrap()]);
            glb.json["nodes"][node]["name"].as_str().unwrap().to_owned()
        };
        let attributes = &meshes[mesh]["primitives"][0]["attributes"];
        let positions = glb.floats(&attributes["POSITION"]);
        let joints = glb.ints(&attributes["JOINTS_0"]);
        let weights = glb.floats(&attributes["WEIGHTS_0"]);
        positions
            .chunks(3)
            .zip(joints.chunks(4).zip(weights.chunks(4)))
            .map(|(p, (j, w))| {
                let influences = j
                    .iter()
                    .zip(w)
                    .filter(|&(_, &w)| w > 0.0)
                    .map(|(&j, &w)| (joint(j), w))
                    .collect();
                // glTF's (x, y, z) is the file's (x, −z, y).
                ([p[0], -p[2], p[1]], influences)
            })
            .collect()
    }

    /// The dragon's bones in short and a mesh `Dragon` of 1,634 positions on `Bip01 Spine`, but
    /// for the positions of [`TAIL_RINGS`] on their thighs, three on `Bip01 Tail` that the first
    /// ring borders on, and one more on the left thigh away from the tail. Every triangle has a
    /// corner on `Bip01 Spine`.
    fn dragon() -> (Skeleton<'static>, Model<'static>) {
        let skeleton = Skeleton {
            bones: vec![
                bone(b"__Root", None, [0.0; 3]),
                bone(b"Bip01 Spine", Some(0), [0.0, 65.7, 124.1]),
                bone(b"Bip01 Tail", Some(1), [0.0, 18.3, -2.9]),
                bone(b"Bip01 L Thigh", Some(1), [32.7, 4.7, -8.4]),
                bone(b"Bip01 R Thigh", Some(1), [-32.7, 4.7, -8.4]),
            ],
        };
        let mut mesh = rigid(b"Dragon", DRAGON_POSITIONS, 1);
        mesh.bones = (1..5).map(entry).collect();
        let one = |bone| vec![Influence { bone, weight: 1.0 }];
        mesh.weights = vec![one(0); DRAGON_POSITIONS];
        let side = |position: usize| if position < 1000 { 2 } else { 3 };
        let [(first, _), (second, _)] = TAIL_RINGS;
        for &(position, at) in first.iter().chain(&second) {
            mesh.positions[position] = at.map(|v| v - 0.000_3);
            mesh.weights[position] = one(side(position));
        }
        let (tails, away) = ([150, 237, 1088], 600);
        for position in tails {
            mesh.weights[position] = one(1);
        }
        mesh.weights[away] = one(2);
        let triangle = |positions| Triangle {
            positions,
            normals: [0; 3],
        };
        mesh.triangles = vec![triangle([away, 1, 2])];
        for (k, &(position, _)) in first.iter().enumerate() {
            let tail = if position < 1000 {
                tails[k % 2]
            } else {
                tails[2]
            };
            mesh.triangles.push(triangle([position, tail, 0]));
        }
        for (k, &(position, _)) in second.iter().enumerate() {
            mesh.triangles.push(triangle([position, first[k].0, 3]));
        }
        let mut model = amulet_model();
        model.meshes = vec![rigid(b"Wings", 3, 1), mesh];
        (skeleton, model)
    }

    #[test]
    fn the_dragons_tail_root_blends_into_the_thighs() {
        for name in DRAGONS {
            let (mut skeleton, mut model) = dragon();
            let (old_skeleton, old_model) = (skeleton.clone(), model.clone());
            let lower = name.to_ascii_lowercase();
            assert_eq!(
                apply(&lower, &mut skeleton, &mut model),
                Outcome::Repaired(12)
            );
            assert_eq!(skeleton, old_skeleton);
            assert_eq!(model.meshes[0], old_model.meshes[0]);
            let (mesh, old) = (&model.meshes[1], &old_model.meshes[1]);
            assert_eq!((&mesh.positions, &mesh.bones), (&old.positions, &old.bones));
            let share = |position: usize| {
                TAIL_RINGS
                    .iter()
                    .find(|(ring, _)| ring.iter().any(|&(p, _)| p == position))
                    .map(|&(_, share)| share)
            };
            for (position, (list, old_list)) in mesh.weights.iter().zip(&old.weights).enumerate() {
                match share(position) {
                    Some(share) => {
                        let thigh = old_list[0].bone;
                        let expected = [(1, share), (thigh, 1.0 - share)]
                            .map(|(bone, weight)| Influence { bone, weight });
                        assert_eq!(list, &expected, "{position}");
                    }
                    None => assert_eq!(list, old_list, "{position}"),
                }
            }
        }
    }

    #[test]
    fn a_tail_unlike_the_expected_one_is_left_as_it_is() {
        let mut cases: Vec<(Skeleton, Model, &str)> = Vec::new();
        // One more position wholly on a thigh borders on the tail.
        let (skeleton, mut model) = dragon();
        model.meshes[1].weights[601] = vec![Influence {
            bone: 2,
            weight: 1.0,
        }];
        model.meshes[1].triangles.push(Triangle {
            positions: [601, 150, 0],
            normals: [0; 3],
        });
        cases.push((skeleton, model, "the tail's rings hold the positions"));
        let (skeleton, mut model) = dragon();
        model.meshes[1].positions[586][0] += 0.01;
        cases.push((
            skeleton,
            model,
            "position 586 of the mesh `Dragon` does not lie",
        ));
        let (mut skeleton, model) = dragon();
        skeleton.bones[2].name = b"Bip01 Tail1";
        cases.push((skeleton, model, "not bound once to `Bip01 Tail`"));
        let (skeleton, mut model) = dragon();
        model.meshes[1].positions.pop();
        cases.push((skeleton, model, "1633 positions"));
        for (mut skeleton, mut model, reason) in cases {
            let (old_skeleton, old_model) = (skeleton.clone(), model.clone());
            match apply(DRAGONS[0], &mut skeleton, &mut model) {
                Outcome::Mismatch(found) => assert!(found.contains(reason), "{found}"),
                other => panic!("{other:?}"),
            }
            assert_eq!((skeleton, model), (old_skeleton, old_model));
        }
    }

    /// The eight dragons in the game files: each is repaired, and the converted mesh's vertices at
    /// the rings' positions take their shares of `Bip01 Tail` and of their thigh.
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn the_dragons_tail_roots_are_blended() {
        let game = opensacred_locate::original();
        let (pak, dir_name) = models::PAKS[0];
        let out = scratch_dir("dragon-tails");
        let dst = models::output_dir(&out, dir_name);
        let only = DRAGONS.map(|n| String::from_utf8(n.to_vec()).unwrap());
        let summary =
            models::convert_pak(&game.join(pak), &dst, &TextureIndex::default(), &only).unwrap();
        assert_eq!(
            (summary.repaired_models, summary.repaired_positions),
            (8, 96)
        );
        assert!(summary.warnings.is_empty(), "{:?}", summary.warnings);
        for file in ["1193-dragon.glb", "1200-dragon_undead.glb"] {
            let glb = Glb::read(&fs::read(dst.join(file)).unwrap());
            let mut blended = 0;
            for (at, mut influences) in vertices(&glb, "Dragon") {
                for (ring, share) in TAIL_RINGS {
                    if let Some(&(position, _)) = ring.iter().find(|&&(_, p)| lies_at(at, p)) {
                        let thigh = if position < 1000 { "L" } else { "R" };
                        let expected = [
                            (format!("Bip01 {thigh} Thigh"), 1.0 - share),
                            ("Bip01 Tail".to_owned(), share),
                        ];
                        influences.sort_by(|a, b| a.0.cmp(&b.0));
                        assert_eq!(influences.len(), 2, "{file}: {position}");
                        for ((name, weight), (expected_name, expected_weight)) in
                            influences.iter().zip(&expected)
                        {
                            assert_eq!(name, expected_name, "{file}: {position}");
                            assert!(
                                (weight - expected_weight).abs() < 1e-6,
                                "{file}: {position}"
                            );
                        }
                        blended += 1;
                    }
                }
            }
            assert!(blended >= 12, "{file}: {blended}");
        }
        fs::remove_dir_all(out).unwrap();
    }

    /// The claws' bones in short: the mesh's bone and the two sockets under the root, where the
    /// file has them.
    fn claws() -> Skeleton<'static> {
        let mut bones = vec![
            bone(b"__Root", None, [0.0; 3]),
            bone(b"Orchidee5", Some(0), [-4.05, 0.0, 0.09]),
            bone(b"Bone_weapon_02", Some(0), CLAW_SOCKETS[1].at),
            bone(b"Bone_weapon_01", Some(0), CLAW_SOCKETS[0].at),
        ];
        bones[2].rotation = CLAW_SOCKETS[1].rotation;
        bones[3].rotation = CLAW_SOCKETS[0].rotation;
        Skeleton { bones }
    }

    /// The quaternion `q` within 1e-5 of `expected`, either sign.
    fn same_turn(q: [f32; 4], expected: [f32; 4]) -> bool {
        let close = |sign: f32| {
            q.iter()
                .zip(expected)
                .all(|(a, b)| (a - sign * b).abs() <= 1e-5)
        };
        close(1.0) || close(-1.0)
    }

    fn conjugate([x, y, z, w]: [f32; 4]) -> [f32; 4] {
        [-x, -y, -z, w]
    }

    #[test]
    fn the_claws_sockets_turn_the_model_a_quarter_about_its_blades_and_move_to_its_grip() {
        let mut skeleton = claws();
        let mut model = amulet_model();
        let (old_skeleton, old_model) = (skeleton.clone(), model.clone());
        assert_eq!(
            apply(b"Orchidee.grn", &mut skeleton, &mut model),
            Outcome::Repaired(0)
        );
        assert_eq!(model, old_model);
        assert_eq!(skeleton.bones[..2], old_skeleton.bones[..2]);
        let quarter = conjugate(QUARTER_BACK);
        for (socket, expected) in [(3, &CLAW_SOCKETS[0]), (2, &CLAW_SOCKETS[1])] {
            let (new, old) = (&skeleton.bones[socket], &old_skeleton.bones[socket]);
            assert_eq!(
                (new.name, new.parent, new.scale_shear),
                (old.name, old.parent, old.scale_shear)
            );
            assert_eq!(new.translation, expected.moved_to);
            // The socket's frame turned back is the model turned +90° about X in the hand.
            assert!(same_turn(compose(new.rotation, quarter), old.rotation));
        }
        // −85° and 82° about X, a sword's sockets' turns.
        assert!(same_turn(
            skeleton.bones[3].rotation,
            [-0.675_590, 0.0, 0.0, 0.737_277]
        ));
        assert!(same_turn(
            skeleton.bones[2].rotation,
            [0.656_059, 0.0, 0.0, 0.754_710]
        ));
        // The left socket keeps its turn from the right one.
        let between = |s: &Skeleton| compose(conjugate(s.bones[3].rotation), s.bones[2].rotation);
        assert!(same_turn(between(&skeleton), between(&old_skeleton)));
    }

    #[test]
    fn claws_unlike_the_expected_ones_are_left_as_they_are() {
        let mut cases: Vec<(Skeleton, &str)> = Vec::new();
        let mut skeleton = claws();
        skeleton.bones[3].rotation = compose(skeleton.bones[3].rotation, QUARTER_BACK);
        cases.push((skeleton, "the socket `Bone_weapon_01` is not turned"));
        let mut skeleton = claws();
        skeleton.bones[2].translation[1] += 0.01;
        cases.push((skeleton, "the socket `Bone_weapon_02` does not lie at"));
        let mut skeleton = claws();
        skeleton.bones[2].parent = Some(1);
        cases.push((skeleton, "`Bone_weapon_02` does not hang under the root"));
        let mut skeleton = claws();
        skeleton.bones[2].name = b"Bone_weapon_01";
        cases.push((skeleton, "two bones `Bone_weapon_01`"));
        let mut skeleton = claws();
        skeleton.bones.pop();
        cases.push((skeleton, "no bone `Bone_weapon_01`"));
        for (mut skeleton, reason) in cases {
            let mut model = amulet_model();
            let old = skeleton.clone();
            match apply(CLAWS, &mut skeleton, &mut model) {
                Outcome::Mismatch(found) => assert!(found.contains(reason), "{found}"),
                other => panic!("{other:?}"),
            }
            assert_eq!((skeleton, model), (old, amulet_model()));
        }
    }

    /// The vertices of every mesh of a converted file, where they lie in the file's model and in
    /// the frame of its socket `socket`, a node under the root of the identity: the hand's frame
    /// as the item is held.
    fn in_socket_frame(glb: &Glb, socket: &str) -> Vec<([f32; 3], [f32; 3])> {
        let nodes = glb.json["nodes"].as_array().unwrap();
        let socket = nodes.iter().position(|n| n["name"] == socket).unwrap();
        let root = &nodes[0];
        assert_eq!(root["name"], "__Root");
        assert!(
            root["children"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| glb.field(c) == socket)
        );
        let vector = |value: &serde_json::Value| -> Vec<f32> {
            serde_json::from_value(value.clone()).unwrap()
        };
        assert_eq!(vector(&root["rotation"]), [0.0, 0.0, 0.0, 1.0]);
        let at = vector(&nodes[socket]["translation"]);
        let turn = vector(&nodes[socket]["rotation"]);
        let back = conjugate([turn[0], turn[1], turn[2], turn[3]]);
        let mut found = Vec::new();
        for mesh in glb.json["meshes"].as_array().unwrap() {
            for primitive in mesh["primitives"].as_array().unwrap() {
                let positions = glb.floats(&primitive["attributes"]["POSITION"]);
                for p in positions.chunks(3) {
                    let v = [p[0] - at[0], p[1] - at[1], p[2] - at[2], 0.0];
                    let [x, y, z, _] = compose(compose(back, v), conjugate(back));
                    found.push(([p[0], p[1], p[2]], [x, y, z]));
                }
            }
        }
        found
    }

    /// The least and the greatest of `points` along each axis.
    fn bounds(points: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
        let min =
            std::array::from_fn(|axis| points.iter().map(|p| p[axis]).fold(f32::MAX, f32::min));
        let max =
            std::array::from_fn(|axis| points.iter().map(|p| p[axis]).fold(f32::MIN, f32::max));
        (min, max)
    }

    /// The span of `points` along each axis.
    fn spans(points: &[[f32; 3]]) -> [f32; 3] {
        let (min, max) = bounds(points);
        std::array::from_fn(|axis| max[axis] - min[axis])
    }

    /// Model 1492 in the game files beside the brass knuckles 1506: converted, the claws spread
    /// their blades along the line of the knuckles, Y in the hand's frame, as the brass knuckles
    /// span it, their tips bend to the palm, −Z in the right hand and +Z in the left, and the
    /// middle of their grip lies on each socket's origin, at the Daemon's knuckles' middle across.
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn the_claws_spread_along_the_knuckles_from_the_grip() {
        let game = opensacred_locate::original();
        let (pak, dir_name) = models::PAKS[0];
        let out = scratch_dir("claws");
        let dst = models::output_dir(&out, dir_name);
        let only = ["ORCHIDEE.GRN", "SCHLAGRING.GRN"].map(str::to_owned);
        let summary =
            models::convert_pak(&game.join(pak), &dst, &TextureIndex::default(), &only).unwrap();
        assert_eq!(
            (summary.repaired_models, summary.repaired_positions),
            (1, 0)
        );
        assert!(summary.warnings.is_empty(), "{:?}", summary.warnings);
        let read = |file: &str| Glb::read(&fs::read(dst.join(file)).unwrap());
        let (claws, knuckles) = (read("1492-orchidee.glb"), read("1506-schlagring.glb"));
        let held = |glb: &Glb, socket| -> Vec<[f32; 3]> {
            in_socket_frame(glb, socket)
                .into_iter()
                .map(|(_, p)| p)
                .collect()
        };
        for (points, what) in [
            (held(&claws, "Bone_weapon_01"), "claws"),
            (held(&knuckles, "Bone_weapon_01"), "brass knuckles"),
        ] {
            let [_, y, z] = spans(&points);
            assert!(y > 2.0 * z, "{what}: {y} across Y, {z} across Z");
        }
        // Each hand: its palm's side, and the middle of the knuckles across.
        for (socket, palm, across) in [
            ("Bone_weapon_01", -1.0, 0.965),
            ("Bone_weapon_02", 1.0, 0.065),
        ] {
            let placed = in_socket_frame(&claws, socket);
            let points: Vec<[f32; 3]> = placed.iter().map(|&(_, p)| p).collect();
            let [x, y, _] = spans(&points);
            assert!(x > 18.0 && y > 7.0, "{socket}: {x} {y}");
            // The grip bar: behind the blades and below the plate in the file's model.
            let grip: Vec<[f32; 3]> = placed
                .iter()
                .filter(|(model, _)| model[0] < -1.0 && model[1] < -0.3)
                .map(|&(_, p)| p)
                .collect();
            assert_eq!(grip.len(), 260, "{socket}");
            let (low, high) = bounds(&grip);
            // The blades' tips against their roots ahead of the grip: towards the palm.
            let (_, most) = bounds(&points);
            let depth = |from: f32, to: f32| {
                let part: Vec<f32> = points
                    .iter()
                    .filter(|p| (from..to).contains(&p[0]))
                    .map(|p| p[2])
                    .collect();
                part.iter().sum::<f32>() / f32::from(u16::try_from(part.len()).unwrap())
            };
            let bend = depth(most[0] - 2.0, most[0] + 1.0) - depth(high[0] + 1.5, high[0] + 3.5);
            assert!(bend * palm > 0.5, "{socket}: the tips' bend {bend}");
            let middle: [f32; 3] = std::array::from_fn(|axis| (low[axis] + high[axis]) / 2.0);
            for (axis, expected) in [0.0, across, 0.0].into_iter().enumerate() {
                assert!(
                    (middle[axis] - expected).abs() < 0.01,
                    "{socket}: the grip's middle {middle:?}"
                );
            }
        }
        fs::remove_dir_all(out).unwrap();
    }
}
