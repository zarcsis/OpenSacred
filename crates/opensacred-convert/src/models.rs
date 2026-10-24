//! `models` subcommand: the records of `PAK/models*.pak` → glTF binaries in
//! `<out>/models/<pak>/<index>-<name>.glb` (docs/02 §4.5; the conversion is in [`gltf`]). A
//! model's file holds its meshes and, as animations, the motions of its motion slots; a motion's
//! file holds its skeleton and its own animation. A texture is referenced as the PNG that
//! `opensacred-convert textures` writes to `<out>/textures/texture/`, and in the game's file as the
//! KTX2 file that `build` makes of it ([`built`]). The records that [`model_repairs`] names are
//! repaired as they are written.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use opensacred_data::texture;
use opensacred_formats::grn::{Animation, Model, Skeleton};
use opensacred_formats::mdl::{Kind, MdlPak, Record};
use opensacred_formats::tex::TexPak;
use rayon::prelude::*;
use serde_json::{Value, json};

use crate::embedded;
use crate::gltf::{self, Clip, Stats};
use crate::ktx::{self, Use};
use crate::model_repairs::{self, Outcome};
use crate::textures;
use crate::verify::read_game_file;

/// Model containers and their output directories under `<out>/models/`.
pub const PAKS: [(&str, &str); 2] = [
    ("PAK/models.pak", "models"),
    ("PAK/models03.pak", "models03"),
];

/// Directory of the files converted from container `dir_name` (see [`PAKS`]).
pub fn output_dir(out: &Path, dir_name: &str) -> PathBuf {
    out.join("models").join(dir_name)
}

/// `0661-seraphim.glb`: the four-digit record index, then the [`textures::stem`] of the name
/// without `.grn`.
pub fn file_name(index: usize, name: &[u8]) -> String {
    format!("{index:04}-{}.glb", textures::stem(name, b".grn"))
}

/// The images of `texture.pak` by name, found as the game finds them: the GRN texture's name
/// with `.TGA` added, without regard to case, and of several records with that name the last
/// (community-reported, openheilig `formats/granny-grn.md`).
#[derive(Debug, Default)]
pub struct TextureIndex {
    /// Upper-case record name → URI of its PNG relative to a file in `<out>/models/<pak>/`.
    by_name: HashMap<Vec<u8>, String>,
}

impl TextureIndex {
    pub fn read(game_dir: &Path) -> Result<Self> {
        let (pak, dir_name) = textures::PAKS[0];
        let path = game_dir.join(pak);
        Self::parse(&read_game_file(&path)?, dir_name).with_context(|| path.display().to_string())
    }

    /// Index of the texture container `data`, whose PNGs are in `<out>/textures/<dir_name>/`.
    pub fn parse(data: &[u8], dir_name: &str) -> Result<Self> {
        let pak = TexPak::parse(data)?;
        let mut by_name = HashMap::new();
        for index in 0..pak.len() {
            if let Some(record) = pak.record(index)? {
                let name = record.header.name();
                let uri = format!(
                    "../../textures/{dir_name}/{}",
                    textures::file_name(index, name)
                );
                by_name.insert(name.to_ascii_uppercase(), uri);
            }
        }
        Ok(Self { by_name })
    }

    /// URI of the image of the GRN texture `name`.
    pub fn uri(&self, name: &[u8]) -> Option<String> {
        let mut key = name.to_ascii_uppercase();
        key.extend_from_slice(b".TGA");
        self.by_name.get(&key).cloned()
    }
}

/// Name of the clip of a motion record: the record name without `.GRN`, as the game writes it.
pub fn clip_name(name: &[u8]) -> String {
    let stem = name
        .len()
        .checked_sub(4)
        .filter(|&n| name[n..].eq_ignore_ascii_case(b".grn"))
        .map_or(name, |n| &name[..n]);
    stem.iter().map(|&b| char::from(b)).collect()
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Summary {
    /// Files written for model records.
    pub models: usize,
    /// Files written for motion records.
    pub motions: usize,
    /// Their statistics, added up.
    pub stats: Stats,
    /// Models repaired ([`model_repairs`]), and the positions whose bones changed.
    pub repaired_models: usize,
    pub repaired_positions: usize,
    /// Records that a repair names, left as they are because their data is not what it expects.
    pub warnings: Vec<String>,
}

/// A motion of a model's slots, read for its clip.
struct Motion<'a> {
    name: String,
    slots: Vec<usize>,
    skeleton: Skeleton<'a>,
    animation: Animation,
}

/// Converts the records of the container `src` into `dst_dir`, in parallel: all of them, or for
/// a non-empty `only` those with these names (without regard to case). The placeholders
/// `INVALID_MODEL` and `INVALID_MOTION` have no file and are skipped.
pub fn convert_pak(
    src: &Path,
    dst_dir: &Path,
    textures: &TextureIndex,
    only: &[String],
) -> Result<Summary> {
    let data = read_game_file(src)?;
    let pak = MdlPak::parse(&data).with_context(|| src.display().to_string())?;
    fs::create_dir_all(dst_dir).with_context(|| format!("creating {}", dst_dir.display()))?;

    let converted = (0..pak.len())
        .into_par_iter()
        .map(|index| {
            let record = pak
                .record(index)
                .with_context(|| src.display().to_string())?;
            let name = record.name();
            let what = || format!("{}, `{}`", src.display(), name.escape_ascii());
            if !only.is_empty() && !only.iter().any(|n| n.as_bytes().eq_ignore_ascii_case(name)) {
                return Ok(None);
            }
            let Some(grn) = record.parse_grn().with_context(what)? else {
                return Ok(None);
            };
            let mut skeleton = grn.skeleton().with_context(what)?;
            let mut model = grn.model().with_context(what)?;
            let repaired = match record.kind {
                Kind::Model => model_repairs::apply(name, &mut skeleton, &mut model),
                Kind::Motion => Outcome::None,
            };
            let scale = own_scale(record.scale).with_context(what)?;
            scale_model(scale, &mut skeleton, &mut model);
            let mut animation = grn.animation().with_context(what)?;
            scale_animation(scale, &mut animation);
            let own_name = clip_name(name);
            let mut clips = Vec::new();
            if !animation.tracks.is_empty() {
                clips.push(Clip {
                    name: &own_name,
                    slots: &[],
                    skeleton: &skeleton,
                    animation: &animation,
                });
            }
            let mut motions = slot_motions(&pak, &record).with_context(what)?;
            for motion in &mut motions {
                scale_animation(scale, &mut motion.animation);
            }
            clips.extend(motions.iter().map(|m| Clip {
                name: &m.name,
                slots: &m.slots,
                skeleton: &m.skeleton,
                animation: &m.animation,
            }));
            // Embedded textures without an image of their name in texture.pak.
            let images = model
                .textures
                .iter()
                .map(|t| match textures.uri(t.name) {
                    Some(_) => Ok(None),
                    None => embedded::to_png(t)
                        .with_context(|| format!("texture `{}`", t.name.escape_ascii())),
                })
                .collect::<Result<Vec<_>>>()
                .with_context(what)?;
            let (glb, stats) = gltf::export(&skeleton, &model, &clips, &images, |name| {
                textures.uri(name)
            })
            .with_context(what)?;
            let path = dst_dir.join(file_name(index, name));
            fs::write(&path, glb).with_context(|| format!("writing {}", path.display()))?;
            let repaired = match repaired {
                Outcome::Mismatch(reason) => {
                    Outcome::Mismatch(format!("{}: {reason}; written as it is", what()))
                }
                other => other,
            };
            Ok(Some((record.kind, stats, repaired)))
        })
        .collect::<Result<Vec<_>>>()?;

    let mut summary = Summary::default();
    for (kind, stats, repaired) in converted.into_iter().flatten() {
        match kind {
            Kind::Model => summary.models += 1,
            Kind::Motion => summary.motions += 1,
        }
        summary.stats.add(&stats);
        match repaired {
            Outcome::None => {}
            Outcome::Repaired(positions) => {
                summary.repaired_models += 1;
                summary.repaired_positions += positions;
            }
            Outcome::Mismatch(warning) => summary.warnings.push(warning),
        }
    }
    Ok(summary)
}

/// The scale of a record ([`Record::scale`], `ModelDesc +0x470`), the same along a model's three
/// axes; 1 for a motion record, which the client scales by the model that plays it.
fn own_scale(scale: Option<[f32; 3]>) -> Result<f32> {
    match scale {
        None => Ok(1.0),
        Some([x, y, z]) if x == y && y == z && x > 0.0 => Ok(x),
        Some(scale) => anyhow::bail!("scale {scale:?}: not the same positive value on every axis"),
    }
}

/// Scales a model about its origin by its own scale, as the client's `GrannyTransformModel` does
/// as it loads the model (`cGrannyModelManager::getModel`, `0x411F50`; docs/02 §4.1): the meshes'
/// positions and the bounds of their bones, and the bind pose's translations. The bones' turns
/// and scale/shear stay, so the inverse bind matrices, computed from the bind pose, follow.
fn scale_model(scale: f32, skeleton: &mut Skeleton, model: &mut Model) {
    if scale == 1.0 {
        return;
    }
    for bone in &mut skeleton.bones {
        bone.translation = bone.translation.map(|v| v * scale);
    }
    for mesh in &mut model.meshes {
        for position in &mut mesh.positions {
            *position = position.map(|v| v * scale);
        }
        for bone in &mut mesh.bones {
            bone.radius *= scale;
            bone.min = bone.min.map(|v| v * scale);
            bone.max = bone.max.map(|v| v * scale);
        }
    }
}

/// Scales a clip's translations by the scale of the model that plays it, as the client's
/// `GrannyTransformAnimation` does as it opens a motion of the model's slots
/// (`cGrannyModelManager::getMotion`, `0x4118B0`).
fn scale_animation(scale: f32, animation: &mut Animation) {
    if scale == 1.0 {
        return;
    }
    for track in &mut animation.tracks {
        for key in &mut track.translation.keys {
            *key = key.map(|v| v * scale);
        }
    }
}

/// The motions that the slots of a model record name, each once with all its slots, in the
/// order of their records; none for a motion record. Clip names stay unique within the file: a
/// repeated one gets the record index.
fn slot_motions<'a>(pak: &MdlPak<'a>, record: &Record) -> Result<Vec<Motion<'a>>> {
    let mut by_ordinal: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (slot, &ordinal) in record.motions.iter().enumerate() {
        if ordinal != 0 {
            by_ordinal.entry(ordinal).or_default().push(slot);
        }
    }
    let mut out: Vec<Motion> = Vec::with_capacity(by_ordinal.len());
    for (ordinal, slots) in by_ordinal {
        let index = usize::try_from(ordinal)
            .ok()
            .and_then(|o| o.checked_add(pak.models()))
            .filter(|&i| i < pak.len())
            .with_context(|| format!("motion slot {}: no motion {ordinal}", slots[0]))?;
        let motion = pak.record(index)?;
        let what = || format!("motion `{}`", motion.name().escape_ascii());
        let Some(grn) = motion.parse_grn().with_context(what)? else {
            continue;
        };
        let mut name = clip_name(motion.name());
        if out.iter().any(|m| m.name == name) {
            name = format!("{name}#{index}");
        }
        out.push(Motion {
            name,
            slots,
            skeleton: grn.skeleton().with_context(what)?,
            animation: grn.animation().with_context(what)?,
        });
    }
    Ok(out)
}

/// The game's file of a model whose master is `glb` (`build`, #700): its images refer to the KTX2
/// textures that `build` makes of the PNG masters, and an image it embeds, a PNG, is embedded as
/// the KTX2 file that `build` would make of it, the binary chunk laid out again around it.
pub fn built(glb: &[u8]) -> Result<Vec<u8>> {
    let (mut json, bin) = gltf::split_glb(glb)?;
    let mut embedded: HashMap<usize, Vec<u8>> = HashMap::new();
    let views = json["bufferViews"].clone();
    for (index, image) in json
        .get_mut("images")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
        .enumerate()
    {
        if let Some(uri) = image.get("uri").and_then(Value::as_str) {
            let stem = uri
                .strip_suffix(".png")
                .with_context(|| format!("image {index} refers to {uri}, not a PNG"))?;
            image["uri"] = json!(format!("{stem}.{}", texture::EXTENSION));
            continue;
        }
        let view = image["bufferView"]
            .as_u64()
            .with_context(|| format!("image {index} has neither a URI nor a buffer view"))?;
        if image["mimeType"] != "image/png" {
            bail!("image {index} embeds a {}, not a PNG", image["mimeType"]);
        }
        let view = usize::try_from(view)?;
        let field = |name: &str| -> Result<usize> {
            Ok(usize::try_from(views[view][name].as_u64().unwrap_or(0))?)
        };
        let (start, length) = (field("byteOffset")?, field("byteLength")?);
        let png = bin
            .get(start..start + length)
            .with_context(|| format!("buffer view {view} lies beyond the binary chunk"))?;
        let ktx2 =
            ktx::encode(png, Use::Sampled).with_context(|| format!("encoding image {index}"))?;
        embedded.insert(view, ktx2);
        image["mimeType"] = json!(format!("image/{}", texture::EXTENSION));
    }
    if embedded.is_empty() {
        let json = serde_json::to_vec(&json).context("serializing the glTF JSON")?;
        return gltf::glb(json, bin);
    }
    // Each view in its order, the embedded images' replaced, each at a multiple of 4 as `gltf`
    // lays them out.
    let mut laid = Vec::with_capacity(bin.len());
    for (index, view) in json
        .get_mut("bufferViews")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
        .enumerate()
    {
        if view["buffer"] != 0 {
            bail!("buffer view {index} is not of the binary chunk");
        }
        let start = usize::try_from(view["byteOffset"].as_u64().unwrap_or(0))?;
        let length = usize::try_from(view["byteLength"].as_u64().unwrap_or(0))?;
        let bytes = match embedded.get(&index) {
            Some(ktx2) => ktx2.as_slice(),
            None => bin
                .get(start..start + length)
                .with_context(|| format!("buffer view {index} lies beyond the binary chunk"))?,
        };
        laid.resize(laid.len().next_multiple_of(4), 0);
        view["byteOffset"] = json!(laid.len());
        view["byteLength"] = json!(bytes.len());
        laid.extend_from_slice(bytes);
    }
    json["buffers"][0]["byteLength"] = json!(laid.len());
    let json = serde_json::to_vec(&json).context("serializing the glTF JSON")?;
    gltf::glb(json, laid)
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use serde_json::Value;

    use super::*;
    use crate::testing::{Tex, repo_root, scratch_dir, tex_pak};

    #[test]
    fn a_built_model_refers_to_the_ktx2_textures_and_embeds_its_own_as_ktx2() {
        let png = textures::encode_png(4, 4, &[200; 64]).unwrap();
        let positions = [1u8, 2, 3, 4, 5, 6];
        let mut bin = png.clone();
        bin.resize(bin.len().next_multiple_of(4), 0);
        let positions_at = bin.len();
        bin.extend_from_slice(&positions);
        let master = json!({
            "asset": {"version": "2.0"},
            "buffers": [{"byteLength": bin.len()}],
            "bufferViews": [
                {"buffer": 0, "byteOffset": 0, "byteLength": png.len()},
                {"buffer": 0, "byteOffset": positions_at, "byteLength": positions.len()},
            ],
            "images": [
                {"name": "dwarf", "uri": "../../textures/texture/00003-dwarf.png"},
                {"name": "own", "bufferView": 0, "mimeType": "image/png"},
            ],
        });
        let glb = gltf::glb(serde_json::to_vec(&master).unwrap(), bin).unwrap();
        let (json, bin) = gltf::split_glb(&built(&glb).unwrap()).unwrap();
        assert_eq!(
            json["images"][0]["uri"],
            "../../textures/texture/00003-dwarf.ktx2"
        );
        assert_eq!(json["images"][1]["mimeType"], "image/ktx2");
        let view = |index: usize| {
            let view = &json["bufferViews"][index];
            let start = usize::try_from(view["byteOffset"].as_u64().unwrap()).unwrap();
            &bin[start..start + usize::try_from(view["byteLength"].as_u64().unwrap()).unwrap()]
        };
        // The embedded image a KTX2 of its mip chain, the other view's bytes where it now lies.
        let ktx2 = ktx2::Reader::new(view(0)).unwrap();
        assert_eq!(ktx2.header().pixel_width, 4);
        assert_eq!(ktx2.header().level_count, 3);
        assert_eq!(view(1), positions);
        // The buffer ends with the last view, the chunk padded to 4 bytes after it.
        let end = json["bufferViews"][1]["byteOffset"].as_u64().unwrap() + 6;
        assert_eq!(json["buffers"][0]["byteLength"].as_u64(), Some(end));
        assert_eq!(bin.len(), usize::try_from(end).unwrap().next_multiple_of(4));
        // A model that embeds nothing keeps its binary chunk, and one without images gets none.
        let mut plain = master.clone();
        plain["images"].as_array_mut().unwrap().pop();
        let original = vec![9; 8];
        let glb = gltf::glb(serde_json::to_vec(&plain).unwrap(), original.clone()).unwrap();
        assert_eq!(gltf::split_glb(&built(&glb).unwrap()).unwrap().1, original);
        plain.as_object_mut().unwrap().remove("images");
        let glb = gltf::glb(serde_json::to_vec(&plain).unwrap(), original.clone()).unwrap();
        let (json, _) = gltf::split_glb(&built(&glb).unwrap()).unwrap();
        assert_eq!(json, plain);
    }

    #[test]
    fn a_models_scale_moves_its_positions_and_bones_but_turns_nothing() {
        use opensacred_formats::grn::{
            Bone, Curve, Interpolation, Layout, Mesh, MeshBone, Track, Triangle,
        };

        let shear = [1.0, 0.5, 0.0, 0.5, 1.0, 0.0, 0.0, 0.0, 2.0];
        let mut skeleton = Skeleton {
            bones: vec![
                Bone {
                    name: b"__Root",
                    parent: None,
                    channel: 0,
                    translation: [0.0; 3],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale_shear: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                },
                Bone {
                    name: b"Object03",
                    parent: Some(0),
                    channel: 1,
                    translation: [-6.0, 3.0, 1.5],
                    rotation: [0.0, 0.0, 0.6, 0.8],
                    scale_shear: shear,
                },
            ],
        };
        let mut model = Model {
            meshes: vec![Mesh {
                name: b"Object03",
                positions: vec![[-12.0, 3.0, 0.0], [0.0, 3.0, 0.0], [-6.0, 9.0, 0.0]],
                normals: vec![[0.0, 0.0, 1.0]],
                texcoord_sets: vec![],
                triangles: vec![Triangle {
                    positions: [0, 1, 2],
                    normals: [0, 0, 0],
                }],
                weights: vec![],
                bones: vec![MeshBone {
                    bone: 1,
                    radius: 30.0,
                    min: [-24.0, -24.0, 0.0],
                    max: [24.0, 24.0, 0.0],
                }],
            }],
            textures: vec![],
            materials: vec![],
            passes: vec![],
        };
        let curve = |keys: Vec<[f32; 3]>| Curve {
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            keys,
        };
        let mut animation = Animation {
            tracks: vec![Track {
                channel: 1,
                layout: Layout::Split,
                translation: curve(vec![[-6.0, 3.0, 1.5], [3.0, 0.0, 6.0]]),
                rotation: Curve {
                    interpolation: Interpolation::Linear,
                    times: vec![0.0],
                    keys: vec![[0.0, 0.0, 0.6, 0.8]],
                },
                scale_shear: Curve {
                    interpolation: Interpolation::Linear,
                    times: vec![0.0],
                    keys: vec![shear],
                },
            }],
        };
        let (old_skeleton, old_animation) = (skeleton.clone(), animation.clone());

        scale_model(0.5, &mut skeleton, &mut model);
        scale_animation(0.5, &mut animation);
        assert_eq!(skeleton.bones[1].translation, [-3.0, 1.5, 0.75]);
        assert_eq!(skeleton.bones[1].rotation, old_skeleton.bones[1].rotation);
        assert_eq!(skeleton.bones[1].scale_shear, shear);
        let mesh = &model.meshes[0];
        assert_eq!(
            mesh.positions,
            [[-6.0, 1.5, 0.0], [0.0, 1.5, 0.0], [-3.0, 4.5, 0.0]]
        );
        assert_eq!(mesh.normals, [[0.0, 0.0, 1.0]]);
        assert_eq!(
            (mesh.bones[0].radius, mesh.bones[0].min, mesh.bones[0].max),
            (15.0, [-12.0, -12.0, 0.0], [12.0, 12.0, 0.0])
        );
        let track = &animation.tracks[0];
        assert_eq!(track.translation.keys, [[-3.0, 1.5, 0.75], [1.5, 0.0, 3.0]]);
        assert_eq!(track.rotation, old_animation.tracks[0].rotation);
        assert_eq!(track.scale_shear, old_animation.tracks[0].scale_shear);
    }

    #[test]
    fn a_models_scale_is_one_value_and_a_motions_is_one() {
        assert_eq!(own_scale(Some([0.33; 3])).unwrap(), 0.33);
        assert!(own_scale(Some([1.0, 2.0, 1.0])).is_err());
        assert!(own_scale(Some([0.0; 3])).is_err());
        assert_eq!(own_scale(None).unwrap(), 1.0);
    }

    #[test]
    fn file_name_is_index_and_sanitized_lower_case_name() {
        assert_eq!(file_name(661, b"SERAPHIM.GRN"), "0661-seraphim.glb");
        // The 33-character name is cut to 32 characters (docs/02 §4.1).
        assert_eq!(
            file_name(1546, b"VAMP_WEAPON_SWORDLONG_2HANDED.GR"),
            "1546-vamp_weapon_swordlong_2handed.gr.glb"
        );
    }

    #[test]
    fn textures_are_found_by_name_and_tga_the_last_record_winning() {
        let tex = |name| {
            Some(Tex {
                name,
                width: 1,
                height: 1,
                format: 6,
                data: vec![0; 4],
            })
        };
        let data = tex_pak(&[
            tex(b"Dwarf.tga"),
            None,
            tex(b"SERA_HAIR.TGA"),
            tex(b"DWARF.TGA"),
        ]);
        let index = TextureIndex::parse(&data, "texture").unwrap();
        let uri = |name: &[u8]| index.uri(name);
        assert_eq!(
            uri(b"dwarf").as_deref(),
            Some("../../textures/texture/00003-dwarf.png")
        );
        assert_eq!(
            uri(b"Sera_hair").as_deref(),
            Some("../../textures/texture/00002-sera_hair.png")
        );
        assert_eq!(uri(b"Sera_head"), None);
        assert_eq!(uri(b"dwarf.tga"), None);
    }

    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn texture_names_resolve_to_the_last_record_of_texture_pak() {
        let index = TextureIndex::read(&repo_root().join("OriginalGame")).unwrap();
        // openheilig's records of the game's choice (community-reported).
        for (name, file) in [
            (b"DWARF".as_slice(), "07986-dwarf.png"),
            (b"daemonia_1024", "07974-daemonia_1024.png"),
            (b"DRYADE_SCOUT_ARMS", "06890-dryade_scout_arms.png"),
            (b"Sera_hair", "07363-sera_hair.png"),
        ] {
            let uri = index.uri(name).unwrap();
            assert_eq!(uri, format!("../../textures/texture/{file}"));
        }
    }

    /// `GOLDCOINS_01.GRN` at its own scale, 0.33 (docs/02 §4.1): its five coins, discs 14.76
    /// units across in the file, lie within 9.70 × 9.69 units, not 29.39 × 29.36.
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn the_smallest_gold_pile_is_a_third_of_its_file() {
        let game = repo_root().join("OriginalGame");
        let out = scratch_dir("models-scale");
        let (pak, dir_name) = PAKS[0];
        let dst = output_dir(&out, dir_name);
        let textures = TextureIndex::read(&game).unwrap();
        let only = ["goldcoins_01.grn".to_owned()];
        let summary = convert_pak(&game.join(pak), &dst, &textures, &only).unwrap();
        assert_eq!(summary.models, 1);

        let glb = fs::read(dst.join("0973-goldcoins_01.glb")).unwrap();
        let json_len =
            usize::try_from(u32::from_le_bytes(glb[12..16].try_into().unwrap())).unwrap();
        let json: Value = serde_json::from_slice(&glb[20..20 + json_len]).unwrap();
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        let mut meshes = 0;
        for mesh in json["meshes"].as_array().unwrap() {
            for primitive in mesh["primitives"].as_array().unwrap() {
                let position =
                    usize::try_from(primitive["attributes"]["POSITION"].as_u64().unwrap());
                let accessor = &json["accessors"][position.unwrap()];
                for axis in 0..3 {
                    lo[axis] = lo[axis].min(accessor["min"][axis].as_f64().unwrap());
                    hi[axis] = hi[axis].max(accessor["max"][axis].as_f64().unwrap());
                }
            }
            meshes += 1;
        }
        assert_eq!(meshes, 5);
        // glTF's x and z lie on the ground.
        let (width, depth) = (hi[0] - lo[0], hi[2] - lo[2]);
        assert!((width - 9.70).abs() < 0.01, "width {width}");
        assert!((depth - 9.69).abs() < 0.01, "depth {depth}");
        fs::remove_dir_all(out).unwrap();
    }

    /// `SERAPHIM.GRN` against the game's own draws: openheilig's trace of the game draws the hero
    /// in six batches of 144, 358, 612, 390, 280 and 270 triangles with the textures of the hair,
    /// the body, the arms, the legs, the head and the boots (community-reported, docs/02 §4.2).
    /// Her file carries the motions of her slots.
    /// glTF-Validator (`GLTF_VALIDATOR`, else `gltf_validator` from `PATH`) finds no error and no
    /// warning, the referenced textures converted next to the file.
    #[test]
    #[ignore = "needs the game files in OriginalGame/ and glTF-Validator"]
    fn seraphim_has_the_games_draws_and_validates() {
        let game = repo_root().join("OriginalGame");
        let out = scratch_dir("models");
        let (pak, dir_name) = PAKS[0];
        let dst = output_dir(&out, dir_name);
        let textures = TextureIndex::read(&game).unwrap();
        let only = ["seraphim.grn".to_owned()];
        let summary = convert_pak(&game.join(pak), &dst, &textures, &only).unwrap();
        assert_eq!((summary.models, summary.stats.triangles), (1, 2054));
        // The 71 motions of her slots, every track bound but helpers such as `Bip01 Footsteps`.
        assert_eq!(summary.stats.animations, 71);

        let path = dst.join("0661-seraphim.glb");
        let glb = fs::read(&path).unwrap();
        let json_len =
            usize::try_from(u32::from_le_bytes(glb[12..16].try_into().unwrap())).unwrap();
        let json: Value = serde_json::from_slice(&glb[20..20 + json_len]).unwrap();
        let index = |value: &Value| usize::try_from(value.as_u64().unwrap()).unwrap();
        let mut draws = Vec::new();
        for mesh in json["meshes"].as_array().unwrap() {
            for primitive in mesh["primitives"].as_array().unwrap() {
                let indices = &json["accessors"][index(&primitive["indices"])];
                let material = &json["materials"][index(&primitive["material"])];
                let texture = &material["pbrMetallicRoughness"]["baseColorTexture"]["index"];
                let image = &json["textures"][index(texture)]["source"];
                let uri = json["images"][index(image)]["uri"].as_str().unwrap();
                draws.push((index(&indices["count"]) / 3, uri.to_owned()));
            }
        }
        let expected = [
            (144, "07363-sera_hair"),
            (358, "07361-sera_body"),
            (612, "07360-sera_arms"),
            (390, "07366-sera_legs"),
            (280, "07365-sera_head"),
            (270, "07362-sera_boots"),
        ]
        .map(|(n, file)| (n, format!("../../textures/texture/{file}.png")));
        assert_eq!(draws, expected);
        let walk = json["animations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["name"] == "SERA_WALK_1H")
            .unwrap();
        let slots: Vec<u64> = (27..=39).collect();
        assert_eq!(walk["extras"]["slots"], serde_json::json!(slots));

        let texture_pak = read_game_file(&game.join(textures::PAKS[0].0)).unwrap();
        let texture_pak = TexPak::parse(&texture_pak).unwrap();
        let texture_dir = textures::output_dir(&out, textures::PAKS[0].1);
        fs::create_dir_all(&texture_dir).unwrap();
        for (_, uri) in &draws {
            let index = uri.rsplit('/').next().unwrap()[..5].parse().unwrap();
            textures::convert_record(&texture_pak, index, &texture_dir).unwrap();
        }
        let validator =
            std::env::var("GLTF_VALIDATOR").unwrap_or_else(|_| "gltf_validator".to_owned());
        let output = Command::new(&validator)
            .arg("-o")
            .arg(&path)
            .output()
            .unwrap_or_else(|e| panic!("cannot run `{validator}` (set GLTF_VALIDATOR): {e}"));
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        let issues = &report["issues"];
        assert_eq!(
            (issues["numErrors"].as_u64(), issues["numWarnings"].as_u64()),
            (Some(0), Some(0)),
            "{issues:#}"
        );
        fs::remove_dir_all(out).unwrap();
    }
}
