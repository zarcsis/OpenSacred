//! Tests on the GOG installation files: every GRN file of `PAK/models*.pak` parses with its CRCs,
//! its skeleton and its model, the counts of docs/02 §4.2 hold, and every node tree, skeleton and
//! model matches the Python prototype `tools/analysis/formats/grn_dump.py`. CI does not have the
//! game files, so every test here is ignored by default.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::iter;

use crc32fast::Hasher;

use super::*;
use crate::mdl::{Kind, MdlPak, Record};
use crate::testing::{game_file, run_python, scratch_dir};

const PAKS: [&str; 2] = ["PAK/models.pak", "PAK/models03.pak"];

/// Calls `f` with every record of both paks in order and its GRN file, `None` for the
/// placeholders.
fn for_each_grn(mut f: impl FnMut(&str, &Record, Option<&Grn>)) {
    for path in PAKS {
        let data = game_file(path);
        let pak = MdlPak::parse(&data).unwrap();
        for index in 0..pak.len() {
            let record = pak.record(index).unwrap();
            let grn = record.parse_grn().unwrap_or_else(|e| panic!("{path}: {e}"));
            f(path, &record, grn.as_ref());
        }
    }
}

fn all_nodes<'g, 'a>(grn: &'g Grn<'a>) -> impl Iterator<Item = &'g Node<'a>> {
    grn.sections().iter().flat_map(Section::nodes)
}

#[test]
#[ignore = "needs the game files in OriginalGame/"]
fn every_grn_parses_and_counts_match_docs() {
    let mut files = 0;
    // (kind, tag) → (nodes, files with the tag)
    let mut tags: HashMap<(Kind, u32), (usize, usize)> = HashMap::new();
    for_each_grn(|path, record, grn| {
        let Some(grn) = grn else {
            return;
        };
        files += 1;
        let sections = grn.sections();
        let section_tags: Vec<_> = sections.iter().map(|s| s.tag).collect();
        assert_eq!(
            section_tags,
            [0xCA5E_0102, SECTION_OBJECT, 0xCA5E_0101],
            "{path} {}",
            record.index
        );
        // Section 0102: `0200`, `1003` with `1001` → `1002` and `1000`; section 0101 is empty.
        let fixed = &sections[0];
        let fixed_tags: Vec<_> = fixed.nodes().iter().map(|n| n.tag).collect();
        assert_eq!(
            fixed_tags,
            [
                0xCA5E_0200,
                0xCA5E_1003,
                0xCA5E_1001,
                0xCA5E_1002,
                0xCA5E_1000
            ]
        );
        assert_eq!(fixed.children(1).collect::<Vec<_>>(), [2, 4]);
        assert!(sections[2].nodes().is_empty());

        let mut counts = HashMap::new();
        for node in all_nodes(grn) {
            *counts.entry(node.tag).or_insert(0) += 1;
        }
        for (tag, n) in counts {
            let entry = tags.entry((record.kind, tag)).or_default();
            entry.0 += n;
            entry.1 += 1;
        }
    });
    assert_eq!(files, 4993);
    let count = |kind, tag| tags.get(&(kind, tag)).copied().unwrap_or_default();
    assert_eq!(count(Kind::Model, TAG_BONE), (70_242, 1572));
    assert_eq!(count(Kind::Model, TAG_MESH), (2555, 1568));
    assert_eq!(count(Kind::Motion, TAG_MESH).1, 23);
    assert_eq!(count(Kind::Motion, TAG_TRACK), (258_502, 3417));
    assert_eq!(count(Kind::Model, TAG_TRACK), (60, 2));
    // The only terminators are the ones that end the sections, and they are not in `nodes()`.
    assert_eq!(count(Kind::Model, TAG_END), (0, 0));
    assert_eq!(count(Kind::Motion, TAG_END), (0, 0));
}

#[test]
#[ignore = "needs the game files in OriginalGame/"]
fn every_skeleton_reads_and_counts_match_docs() {
    // kind → [files, bones, files with repeated bone names, bone names outside printable ASCII]
    let mut stats: HashMap<Kind, [usize; 4]> = HashMap::new();
    for_each_grn(|path, record, grn| {
        let Some(grn) = grn else {
            return;
        };
        let skeleton = grn
            .skeleton()
            .unwrap_or_else(|e| panic!("{path} record {}: {e}", record.index));
        let bones = &skeleton.bones;
        assert_eq!(bones[0].name, b"__Root", "{path} record {}", record.index);
        let mut names: Vec<_> = bones.iter().map(|b| b.name).collect();
        let odd = names
            .iter()
            .filter(|name| name.iter().any(|b| !(0x20..0x7F).contains(b)))
            .count();
        names.sort_unstable();
        names.dedup();
        let entry = stats.entry(record.kind).or_default();
        for (stat, n) in
            entry
                .iter_mut()
                .zip([1, bones.len(), usize::from(names.len() < bones.len()), odd])
        {
            *stat += n;
        }
    });
    assert_eq!(stats[&Kind::Model], [1572, 70_242, 32, 26]);
    assert_eq!(stats[&Kind::Motion], [3421, 258_760, 106, 89]);
}

#[test]
#[ignore = "needs the game files in OriginalGame/"]
fn every_model_reads_and_counts_match_docs() {
    let mut stats: HashMap<(Kind, &str), usize> = HashMap::new();
    for_each_grn(|path, record, grn| {
        let Some(grn) = grn else {
            return;
        };
        let model = grn
            .model()
            .unwrap_or_else(|e| panic!("{path} record {}: {e}", record.index));
        let mut add = |stat, n: usize| *stats.entry((record.kind, stat)).or_default() += n;
        add("files with meshes", usize::from(!model.meshes.is_empty()));
        add(
            "files with a skinned mesh",
            usize::from(model.meshes.iter().any(|m| !m.weights.is_empty())),
        );
        for mesh in &model.meshes {
            let empty = mesh.positions.is_empty();
            add("meshes", 1);
            add("meshes without vertices", usize::from(empty));
            add("positions", mesh.positions.len());
            add("normals", mesh.normals.len());
            add("triangles", mesh.triangles.len());
            let skinned = !mesh.weights.is_empty();
            add("skinned meshes", usize::from(skinned));
            add("rigid meshes", usize::from(!skinned && !empty));
            let many = mesh.weights.iter().filter(|w| w.len() > 4).count();
            add("vertices with more than 4 influences", many);
            add("meshes with such vertices", usize::from(many > 0));
            // The weights of a vertex are positive and sum to 1.
            let odd = mesh.weights.iter().filter(|influences| {
                let sum: f32 = influences.iter().map(|i| i.weight).sum();
                influences.iter().any(|i| i.weight <= 0.0) || (sum - 1.0).abs() >= 1e-5
            });
            add("vertices with odd weights", odd.count());
            let zero: Vec<_> = mesh.normals.iter().map(|n| *n == [0.0; 3]).collect();
            add("zero normals", zero.iter().filter(|&&z| z).count());
            let corners = mesh.triangles.iter().flat_map(|t| t.normals);
            add(
                "corners on a zero normal",
                corners.filter(|&n| zero[n]).count(),
            );
        }
        let mut owners = vec![0; model.textures.len()];
        for material in &model.materials {
            match material.surface {
                Surface::Texture(texture) => {
                    owners[texture] += 1;
                    add("texture materials", 1);
                }
                Surface::Color(_) => add("color materials", 1),
            }
        }
        assert!(owners.iter().all(|&n| n == 1), "{path} {}", record.index);
        for texture in &model.textures {
            add(format_name(texture.format), 1);
        }
        let mut surfaces = vec![(false, false); model.meshes.len()];
        for pass in &model.passes {
            add("passes", 1);
            let set = |source| matches!(source, Source::Set(_));
            add(
                match (set(pass.color), set(pass.texcoords)) {
                    (false, true) => "passes with a constant and a set",
                    (false, false) => "passes with two constants",
                    (true, true) => "passes with two sets",
                    (true, false) => "passes with a set and a constant",
                },
                1,
            );
            let drawn = &mut surfaces[pass.mesh];
            match model.materials[pass.material].surface {
                Surface::Texture(_) => drawn.0 = true,
                Surface::Color(_) => drawn.1 = true,
            }
        }
        add(
            "meshes drawn with color materials only",
            surfaces.iter().filter(|&&s| s == (false, true)).count(),
        );
        add(
            "meshes drawn with both kinds",
            surfaces.iter().filter(|&&s| s == (true, true)).count(),
        );
    });
    // (statistic, models, motions), docs/02 §4.2
    let expected = [
        ("files with meshes", 1568, 23),
        ("files with a skinned mesh", 1010, 19),
        ("meshes", 2555, 207),
        ("meshes without vertices", 17, 0),
        ("positions", 412_698, 20_353),
        ("normals", 510_181, 24_264),
        ("triangles", 706_145, 34_414),
        ("skinned meshes", 1679, 57),
        ("rigid meshes", 859, 150),
        ("vertices with more than 4 influences", 60, 0),
        ("meshes with such vertices", 16, 0),
        ("vertices with odd weights", 0, 0),
        ("zero normals", 868, 1027),
        ("corners on a zero normal", 2476, 2927),
        ("texture materials", 2083, 131),
        ("color materials", 181, 144),
        ("external textures", 1923, 21),
        ("VTex textures", 104, 22),
        ("VTex textures with alpha", 41, 88),
        ("RGBX textures", 6, 0),
        ("RGBA textures", 9, 0),
        ("passes", 2891, 302),
        ("passes with a constant and a set", 2715, 158),
        ("passes with two constants", 174, 144),
        ("passes with two sets", 2, 0),
        ("passes with a set and a constant", 0, 0),
        ("meshes drawn with color materials only", 181, 144),
        ("meshes drawn with both kinds", 0, 0),
    ];
    let count = |kind, stat| stats.get(&(kind, stat)).copied().unwrap_or(0);
    let wrong: Vec<_> = expected
        .iter()
        .filter(|&&(stat, models, motions)| {
            (count(Kind::Model, stat), count(Kind::Motion, stat)) != (models, motions)
        })
        .map(|&(stat, ..)| (stat, count(Kind::Model, stat), count(Kind::Motion, stat)))
        .collect();
    assert!(wrong.is_empty(), "(statistic, models, motions): {wrong:?}");
}

#[test]
#[ignore = "needs the game files in OriginalGame/"]
fn every_animation_reads_and_counts_match_docs() {
    let mut stats: HashMap<(Kind, &str), usize> = HashMap::new();
    for_each_grn(|path, record, grn| {
        let Some(grn) = grn else {
            return;
        };
        let skeleton = grn.skeleton().unwrap();
        let animation = grn
            .animation()
            .unwrap_or_else(|e| panic!("{path} record {}: {e}", record.index));
        let mut add = |stat, n: usize| *stats.entry((record.kind, stat)).or_default() += n;
        if animation.tracks.is_empty() {
            return;
        }
        add("files with tracks", 1);
        let mut per_bone = vec![0; skeleton.bones.len()];
        for track in &animation.tracks {
            let bone = skeleton.bone_of_channel(track.channel);
            let bone = bone.unwrap_or_else(|| panic!("{path} record {}", record.index));
            per_bone[bone] += 1;
            add("tracks", 1);
            match track.layout {
                Layout::Split => add("split tracks", 1),
                Layout::Interleaved => {
                    add("interleaved tracks", 1);
                    add("interleaved frames", track.translation.times.len());
                }
            }
            if track.layout == Layout::Split {
                for (curve, mode) in [
                    ("translation", track.translation.interpolation),
                    ("rotation", track.rotation.interpolation),
                    ("scale/shear", track.scale_shear.interpolation),
                ] {
                    add(curve_mode(curve, mode), 1);
                }
            }
            add("rotation keys", track.rotation.keys.len());
            add("scale/shear keys", track.scale_shear.keys.len());
        }
        add(
            "files whose tracks animate every bone once",
            usize::from(per_bone.iter().all(|&n| n == 1)),
        );
        add(
            "files with two tracks of a bone",
            usize::from(per_bone.iter().any(|&n| n > 1)),
        );
    });
    // (statistic, models, motions), docs/02 §4.2
    let expected = [
        ("files with tracks", 2, 3417),
        ("tracks", 60, 258_502),
        ("split tracks", 4, 255_489),
        ("interleaved tracks", 56, 3013),
        ("interleaved frames", 0, 283_758),
        ("files whose tracks animate every bone once", 2, 3415),
        ("files with two tracks of a bone", 0, 2),
        ("translation mode 2", 4, 255_489),
        ("rotation mode 2", 4, 255_489),
        ("scale/shear mode 0", 0, 1262),
        ("scale/shear mode 1", 4, 171_412),
        ("scale/shear mode 2", 0, 82_815),
        ("rotation keys", 12, 6_639_940),
        ("scale/shear keys", 8, 880_151),
    ];
    let count = |kind, stat| stats.get(&(kind, stat)).copied().unwrap_or(0);
    let wrong: Vec<_> = expected
        .iter()
        .filter(|&&(stat, models, motions)| {
            (count(Kind::Model, stat), count(Kind::Motion, stat)) != (models, motions)
        })
        .map(|&(stat, ..)| (stat, count(Kind::Model, stat), count(Kind::Motion, stat)))
        .collect();
    assert!(wrong.is_empty(), "(statistic, models, motions): {wrong:?}");
}

fn curve_mode(curve: &str, mode: Interpolation) -> &'static str {
    match (curve, mode) {
        ("translation", Interpolation::Quadratic) => "translation mode 2",
        ("rotation", Interpolation::Quadratic) => "rotation mode 2",
        ("scale/shear", Interpolation::Constant) => "scale/shear mode 0",
        ("scale/shear", Interpolation::Linear) => "scale/shear mode 1",
        ("scale/shear", Interpolation::Quadratic) => "scale/shear mode 2",
        _ => "other curve modes",
    }
}

fn format_name(format: TextureFormat) -> &'static str {
    match format {
        TextureFormat::External => "external textures",
        TextureFormat::VTex => "VTex textures",
        TextureFormat::VTexAlpha => "VTex textures with alpha",
        TextureFormat::Rgbx => "RGBX textures",
        TextureFormat::Rgba => "RGBA textures",
    }
}

/// Every record against the `--records` table of `grn_dump.py`: section node counts, the tree
/// fingerprint (CRC-32 of `(tag, own data length, descendants)` of every node in order), the tag
/// counts, the skeleton fingerprint (CRC-32 over the bones of the name, a NUL, the parent field
/// and the 16 floats), the mesh and material fingerprints of [`mesh_crc`] and [`material_crc`] and
/// the animation fingerprint of [`animation_crc`].
#[test]
#[ignore = "needs the game files in OriginalGame/ and the Python of pyproject.toml through uv"]
fn trees_skeletons_and_models_match_grn_dump_py() {
    let out = scratch_dir("grn-trees");
    let table = out.join("records.tsv");
    let game_dir = opensacred_locate::original();
    run_python(
        "tools/analysis/formats/grn_dump.py",
        &[
            OsStr::new("--game-dir"),
            game_dir.as_os_str(),
            OsStr::new("--records"),
            table.as_os_str(),
        ],
    );
    let text = fs::read_to_string(&table).unwrap();
    let mut lines = text.lines();
    let columns: Vec<_> = lines.next().unwrap().split('\t').collect();
    let column = |name: &str| columns.iter().position(|&c| c == name).unwrap();
    let wanted = [
        "pak",
        "index",
        "grn",
        "section_nodes",
        "tree_crc",
        "bones",
        "meshes",
        "tracks",
        "skeleton_crc",
        "mesh_crc",
        "material_crc",
        "animation_crc",
    ]
    .map(column);

    let mut compared = 0;
    for_each_grn(|path, record, grn| {
        let row: Vec<_> = lines.next().unwrap().split('\t').collect();
        let python = wanted.map(|i| row[i]);
        let ours = match grn {
            None => ["empty", "", "", "0", "0", "0", "", "", "", ""].map(str::to_owned),
            Some(grn) => {
                let nodes: Vec<_> = grn
                    .sections()
                    .iter()
                    .map(|s| (s.nodes().len() + 1).to_string())
                    .collect();
                let mut fingerprint = crc32fast::Hasher::new();
                let mut counts = HashMap::new();
                for node in all_nodes(grn) {
                    for value in [
                        node.tag,
                        len_u32(node.bytes().len()),
                        len_u32(node.descendants),
                    ] {
                        fingerprint.update(&value.to_le_bytes());
                    }
                    *counts.entry(node.tag).or_insert(0usize) += 1;
                }
                let count = |tag| counts.get(&tag).copied().unwrap_or(0).to_string();
                let mut skeleton = crc32fast::Hasher::new();
                for bone in grn.skeleton().unwrap().bones {
                    skeleton.update(bone.name);
                    skeleton.update(&[0]);
                    skeleton.update(&len_u32(bone.parent.unwrap_or(0)).to_le_bytes());
                    for value in bone
                        .translation
                        .iter()
                        .chain(&bone.rotation)
                        .chain(&bone.scale_shear)
                    {
                        skeleton.update(&value.to_le_bytes());
                    }
                }
                let model = grn.model().unwrap();
                [
                    "ok".to_owned(),
                    nodes.join(","),
                    fingerprint.finalize().to_string(),
                    count(TAG_BONE),
                    count(TAG_MESH),
                    count(TAG_TRACK),
                    skeleton.finalize().to_string(),
                    mesh_crc(&model).to_string(),
                    material_crc(&model).to_string(),
                    animation_crc(&grn.animation().unwrap()).to_string(),
                ]
            }
        };
        let index = record.index.to_string();
        let mut expected = vec![path, index.as_str()];
        expected.extend(ours.iter().map(String::as_str));
        assert_eq!(python.as_slice(), expected, "{path} record {index}");
        compared += 1;
    });
    assert!(lines.next().is_none());
    assert_eq!(compared, 4997);
    fs::remove_dir_all(out).unwrap();
}

/// `mesh_crc` of `grn_dump.py`: CRC-32 over the meshes in order of the name and a NUL, then each
/// list as a `u32` count and its items, all `u32` and `f32` LE: positions, normals, sets (each a
/// count and its entries), triangles (positions, then normals), weights (per vertex a count and
/// `(local bone, weight)` pairs), bones (bone, radius, min, max).
fn mesh_crc(model: &Model) -> u32 {
    let mut h = Hasher::new();
    for mesh in &model.meshes {
        h.update(mesh.name);
        h.update(&[0]);
        vectors(&mut h, &mesh.positions);
        vectors(&mut h, &mesh.normals);
        put(&mut h, mesh.texcoord_sets.len());
        for set in &mesh.texcoord_sets {
            vectors(&mut h, set);
        }
        put(&mut h, mesh.triangles.len());
        for triangle in &mesh.triangles {
            for &index in triangle.positions.iter().chain(&triangle.normals) {
                put(&mut h, index);
            }
        }
        put(&mut h, mesh.weights.len());
        for influences in &mesh.weights {
            put(&mut h, influences.len());
            for influence in influences {
                put(&mut h, influence.bone);
                floats(&mut h, [&influence.weight]);
            }
        }
        put(&mut h, mesh.bones.len());
        for bone in &mesh.bones {
            put(&mut h, bone.bone);
            floats(
                &mut h,
                iter::once(&bone.radius).chain(&bone.min).chain(&bone.max),
            );
        }
    }
    h.finalize()
}

/// `material_crc` of `grn_dump.py`: CRC-32, all `u32` and `f32` LE, over the textures (a count;
/// each the name and a NUL, width, height, format code, payload length, the payload), the
/// materials (a count; each 1, the name and a NUL and the texture for a texture material, or 0,
/// 1 with the name and a NUL or 0 without a name, and the RGBA for a color material) and the
/// passes (a count; each the mesh, the material, per source 0 and the RGB or 1 and the set, the
/// triangle count and per triangle the triangle and the set entries, corner by corner).
fn material_crc(model: &Model) -> u32 {
    let mut h = Hasher::new();
    put(&mut h, model.textures.len());
    for texture in &model.textures {
        h.update(texture.name);
        h.update(&[0]);
        h.update(&texture.width.to_le_bytes());
        h.update(&texture.height.to_le_bytes());
        let code = match texture.format {
            TextureFormat::Rgbx => 0,
            TextureFormat::Rgba => 1,
            TextureFormat::VTex => 4,
            TextureFormat::VTexAlpha => 5,
            TextureFormat::External => 7,
        };
        put(&mut h, code);
        put(&mut h, texture.payload.len());
        h.update(texture.payload);
    }
    put(&mut h, model.materials.len());
    for material in &model.materials {
        match material.surface {
            Surface::Texture(texture) => {
                put(&mut h, 1);
                h.update(material.name.unwrap_or_default());
                h.update(&[0]);
                put(&mut h, texture);
            }
            Surface::Color(rgba) => {
                put(&mut h, 0);
                put(&mut h, usize::from(material.name.is_some()));
                if let Some(name) = material.name {
                    h.update(name);
                    h.update(&[0]);
                }
                floats(&mut h, &rgba);
            }
        }
    }
    put(&mut h, model.passes.len());
    for pass in &model.passes {
        put(&mut h, pass.mesh);
        put(&mut h, pass.material);
        for source in [pass.color, pass.texcoords] {
            match source {
                Source::Constant(rgb) => {
                    put(&mut h, 0);
                    floats(&mut h, &rgb);
                }
                Source::Set(set) => {
                    put(&mut h, 1);
                    put(&mut h, set);
                }
            }
        }
        put(&mut h, pass.triangles.len());
        for triangle in &pass.triangles {
            put(&mut h, triangle.triangle);
            for corner in 0..3 {
                for entries in [triangle.color, triangle.texcoords].into_iter().flatten() {
                    put(&mut h, entries[corner]);
                }
            }
        }
    }
    h.finalize()
}

/// `animation_crc` of `grn_dump.py`: CRC-32, all `u32` and `f32` LE, over the track count; per
/// track its channel (from 0) and layout code (1 split, 0 interleaved), then per curve its mode,
/// its key count, its times and its keys.
fn animation_crc(animation: &Animation) -> u32 {
    let mut h = Hasher::new();
    put(&mut h, animation.tracks.len());
    for track in &animation.tracks {
        put(&mut h, track.channel);
        put(&mut h, usize::from(track.layout == Layout::Split));
        curve_crc(&mut h, &track.translation);
        curve_crc(&mut h, &track.rotation);
        curve_crc(&mut h, &track.scale_shear);
    }
    h.finalize()
}

fn curve_crc<const N: usize>(h: &mut Hasher, curve: &Curve<N>) {
    h.update(&curve.interpolation.code().to_le_bytes());
    put(h, curve.keys.len());
    floats(h, &curve.times);
    floats(h, curve.keys.iter().flatten());
}

fn put(h: &mut Hasher, value: usize) {
    h.update(&len_u32(value).to_le_bytes());
}

fn floats<'f>(h: &mut Hasher, values: impl IntoIterator<Item = &'f f32>) {
    for value in values {
        h.update(&value.to_le_bytes());
    }
}

/// A count and the vectors.
fn vectors(h: &mut Hasher, list: &[[f32; 3]]) {
    put(h, list.len());
    floats(h, list.iter().flatten());
}

fn len_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap()
}
