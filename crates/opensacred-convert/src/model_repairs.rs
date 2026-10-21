//! Repairs of the models' data (docs/02 §4.6) that `models` makes as it writes the records they
//! name, each as narrow as its defect (CLAUDE.md, "Code conventions"). A named record whose data
//! is not what its repair expects is left as it is, with a warning.

use opensacred_formats::grn::{Bone, Model, Skeleton};

/// What [`apply`] did to a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// No repair names the record.
    None,
    /// Repaired: the number of positions whose bones changed.
    Repaired(usize),
    /// A record that a repair names, whose data is not what the repair expects: left as it is.
    Mismatch(String),
}

/// A repair of a record's skeleton and model: the number of positions whose bones it changed, or
/// why the data is not what it expects, before it changes anything.
type Repair = fn(&mut Skeleton, &mut Model) -> Result<usize, String>;

/// The repairs by the name of the record they repair.
const REPAIRS: [(&[u8], Repair); 1] = [(EARTH_AMULET, earth_amulet)];

/// Repairs `skeleton` and `model` of the model record `name` if a repair names it; the record
/// name is compared without regard to case, as the command line's `--model` is.
pub fn apply(name: &[u8], skeleton: &mut Skeleton, model: &mut Model) -> Outcome {
    let Some((_, repair)) = REPAIRS.iter().find(|(n, _)| name.eq_ignore_ascii_case(n)) else {
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
/// (docs/decisions/2026-10-21-earth-amulet.md).
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
    let mut meshes = model
        .meshes
        .iter()
        .enumerate()
        .filter(|(_, m)| m.name == EARTH_AMULET_MESH)
        .map(|(index, _)| index);
    let (Some(index), None) = (meshes.next(), meshes.next()) else {
        return Err("not one mesh `Amulett_erd`".to_owned());
    };
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
    use crate::testing::{Glb, repo_root, scratch_dir};
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
                .map(|k| [f32::from(u8::try_from(k).unwrap()), 0.0, 1.0])
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
        let game = repo_root().join("OriginalGame");
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
}
