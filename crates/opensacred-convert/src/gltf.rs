//! glTF 2.0 binary (`.glb`) of a GRN model (docs/02 §4.2, §4.5). The skeleton becomes a tree of
//! nodes, and every drawn mesh a skinned mesh with one primitive per pass; the materials refer to
//! the PNG masters of `opensacred-convert textures`. Animation clips become glTF animations of the
//! bone nodes ([`crate::animation`] turns their curves into keyframes). The game's models are
//! Z-up, as exported from 3ds Max, and glTF is Y-up: positions, directions and transforms are
//! turned by −90° about X, a rotation, so the winding stays counter-clockwise, and a model facing
//! Max's front (−Y) faces glTF's front (+Z). In the bind pose 719 of the 875 biped models face
//! −Y, but 148, among them `SERAPHIM.GRN` and `GLADIATOR.GRN`, face +X; in the game the
//! animations turn them. A bone whose scale/shear is not diagonal becomes two nodes, since glTF
//! scales along a node's own axes only ([`animation::Shear`]). Where glTF cannot hold the
//! original, the choice is recorded in `docs/decisions/`.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, bail};
use opensacred_formats::grn::{
    Animation, Material, Mesh, Model, Pass, Skeleton, Source, Surface, TextureFormat,
};
use serde_json::{Value, json};

use crate::animation::{self, Keyframes, Mat3, Shear};

/// Influences per vertex in `JOINTS_0`/`WEIGHTS_0`, the most that engines such as Bevy read.
const MAX_INFLUENCES: usize = 4;
/// glTF `componentType` and `bufferView.target` codes.
const UNSIGNED_SHORT: u32 = 5123;
const UNSIGNED_INT: u32 = 5125;
const FLOAT: u32 = 5126;
const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;

/// What the export kept, dropped or replaced in one model.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    /// Meshes exported as glTF meshes.
    pub meshes: usize,
    /// Meshes drawn only with unnamed color materials: helper geometry, not exported.
    pub helper_meshes: usize,
    pub primitives: usize,
    /// glTF vertices: distinct corners of position, normal, texture coordinates and color.
    pub vertices: usize,
    pub triangles: usize,
    /// Triangle corners with a zero normal, which get the normal of their triangle.
    pub zero_normals: usize,
    /// Vertices with more than four influences, of which the four largest are kept.
    pub reduced_vertices: usize,
    /// Bones with a scale/shear off the diagonal, in the bind pose or in a track, which get an
    /// outer node for it.
    pub sheared_bones: usize,
    /// Embedded textures (VTex or raw) whose image goes into the file: `texture.pak` has no image
    /// of their name.
    pub embedded_textures: usize,
    /// Embedded textures that refer to the image of `texture.pak` with their name instead, as the
    /// game binds them (community-reported).
    pub twinned_textures: usize,
    /// Texture materials whose image name is not in `texture.pak`: mostly equipment, which the
    /// game textures by the index an item gives (docs/02 §4.2, community-reported).
    pub unnamed_textures: usize,
    /// Animations written: clips with at least one track bound to a node.
    pub animations: usize,
    /// Tracks bound to a node.
    pub tracks: usize,
    /// Tracks of a bone that the model lacks, such as the helper `Bip01 Footsteps`; left out.
    pub unbound_tracks: usize,
    /// Second tracks of a bone in one clip (the two `FX_*_IDLE_BH.GRN`); left out.
    pub duplicate_tracks: usize,
    /// Bones held in the bind pose by a clip because another clip of the file moves them.
    pub rest_tracks: usize,
    /// Tracks with a scale/shear off the diagonal, sampled.
    pub sheared_tracks: usize,
    /// Keys at which a rotation curve of the original jumps (docs/02 §4.2).
    pub rotation_jumps: usize,
    /// Keyframes written, over all samplers.
    pub keyframes: usize,
}

impl Stats {
    pub fn add(&mut self, other: &Self) {
        let Self {
            meshes,
            helper_meshes,
            primitives,
            vertices,
            triangles,
            zero_normals,
            reduced_vertices,
            sheared_bones,
            embedded_textures,
            twinned_textures,
            unnamed_textures,
            animations,
            tracks,
            unbound_tracks,
            duplicate_tracks,
            rest_tracks,
            sheared_tracks,
            rotation_jumps,
            keyframes,
        } = other;
        self.meshes += meshes;
        self.helper_meshes += helper_meshes;
        self.primitives += primitives;
        self.vertices += vertices;
        self.triangles += triangles;
        self.zero_normals += zero_normals;
        self.reduced_vertices += reduced_vertices;
        self.sheared_bones += sheared_bones;
        self.embedded_textures += embedded_textures;
        self.twinned_textures += twinned_textures;
        self.unnamed_textures += unnamed_textures;
        self.animations += animations;
        self.tracks += tracks;
        self.unbound_tracks += unbound_tracks;
        self.duplicate_tracks += duplicate_tracks;
        self.rest_tracks += rest_tracks;
        self.sheared_tracks += sheared_tracks;
        self.rotation_jumps += rotation_jumps;
        self.keyframes += keyframes;
    }
}

/// Animation clip of a file: the tracks of `animation` name the bones of `skeleton` by channel
/// and play on the bones of the model with the same names.
#[derive(Debug, Clone, Copy)]
pub struct Clip<'c> {
    pub name: &'c str,
    /// The model's motion slots that play the clip (`ModelDesc +0x70`, the game's motion enum
    /// according to openheilig); empty for a file's own tracks.
    pub slots: &'c [usize],
    pub skeleton: &'c Skeleton<'c>,
    pub animation: &'c Animation,
}

/// The `.glb` file of `model` on `skeleton` with the animations `clips`. `texture_uri` gives the
/// URI of the image of `texture.pak` with a texture's name, relative to the file, or `None` if
/// there is none; `embedded` holds the PNG of an embedded texture without such an image, by
/// texture index, and goes into the file.
pub fn export(
    skeleton: &Skeleton,
    model: &Model,
    clips: &[Clip],
    embedded: &[Option<Vec<u8>>],
    texture_uri: impl Fn(&[u8]) -> Option<String>,
) -> Result<(Vec<u8>, Stats)> {
    let mut gltf = Builder {
        model,
        texture_uri: &texture_uri,
        embedded,
        stats: Stats::default(),
        buffer: Vec::new(),
        views: Vec::new(),
        accessors: Vec::new(),
        materials: Vec::new(),
        material_index: HashMap::new(),
        textures: Vec::new(),
        images: Vec::new(),
        image_index: HashMap::new(),
        time_index: HashMap::new(),
        rest: Vec::new(),
        outer: Vec::new(),
    };
    let targets = Targets::new(skeleton);
    let bound = clips
        .iter()
        .map(|clip| targets.bind(clip))
        .collect::<Result<Vec<_>>>()?;
    let mut sheared: Vec<bool> = skeleton
        .bones
        .iter()
        .map(|b| off_diagonal(&b.scale_shear))
        .collect();
    for (clip, bound) in clips.iter().zip(&bound) {
        for (track, binding) in clip.animation.tracks.iter().zip(bound) {
            if let Binding::Node(bone) = *binding
                && track.scale_shear.keys.iter().any(off_diagonal)
            {
                sheared[bone] = true;
            }
        }
    }
    let (mut nodes, bind) = gltf.skeleton(skeleton, &sheared)?;
    let skeleton_root = gltf.outer[0].unwrap_or(0);
    let mut roots = vec![skeleton_root];
    let mut meshes = Vec::new();
    let mut skins = Vec::new();
    for (index, mesh) in model.meshes.iter().enumerate() {
        let passes: Vec<_> = model.passes.iter().filter(|p| p.mesh == index).collect();
        let drawn: Vec<_> = passes
            .iter()
            .copied()
            .filter(|p| !is_helper(&model.materials[p.material]))
            .collect();
        if drawn.is_empty() {
            gltf.stats.helper_meshes += usize::from(!passes.is_empty());
            continue;
        }
        let name = latin1(mesh.name);
        meshes.push(gltf.mesh(&name, mesh, &drawn)?);
        skins.push(gltf.skin(mesh, &bind, skeleton_root)?);
        roots.push(nodes.len());
        nodes.push(json!({"name": name, "mesh": meshes.len() - 1, "skin": skins.len() - 1}));
        gltf.stats.meshes += 1;
    }
    // Granny poses a bone that no track of a clip moves in its bind pose, and an engine keeps
    // what the last clip left: a node that one clip moves gets its bind pose in the others.
    let moves: Vec<HashSet<usize>> = bound
        .iter()
        .map(|bound| {
            let bones = bound.iter().filter_map(|b| match b {
                Binding::Node(bone) => Some(*bone),
                _ => None,
            });
            bones
                .flat_map(|bone| [Some(bone), gltf.outer[bone]])
                .flatten()
                .collect()
        })
        .collect();
    let moved: HashSet<usize> = moves.iter().flatten().copied().collect();
    let mut animations = Vec::new();
    for ((clip, bound), moves) in clips.iter().zip(&bound).zip(&moves) {
        let mut rest: Vec<usize> = moved.difference(moves).copied().collect();
        rest.sort_unstable();
        let animation = gltf
            .animation(clip, bound, &rest)
            .with_context(|| format!("clip `{}`", clip.name))?;
        animations.extend(animation);
    }
    gltf.stats.animations = animations.len();

    let mut root = json!({
        "asset": {"version": "2.0", "generator": "opensacred-convert"},
        "scene": 0,
        "scenes": [{"nodes": roots}],
        "nodes": nodes,
    });
    for (key, list) in [
        ("meshes", meshes),
        ("skins", skins),
        ("animations", animations),
        ("materials", gltf.materials),
        ("textures", gltf.textures),
        ("images", gltf.images),
        ("accessors", gltf.accessors),
        ("bufferViews", gltf.views),
    ] {
        if !list.is_empty() {
            root[key] = Value::Array(list);
        }
    }
    if !gltf.buffer.is_empty() {
        root["buffers"] = json!([{"byteLength": gltf.buffer.len()}]);
    }
    let json = serde_json::to_vec(&root).context("serializing the glTF JSON")?;
    Ok((glb(json, gltf.buffer)?, gltf.stats))
}

/// A color material without a name: the default material of a 3ds Max object with none
/// assigned, on helper geometry such as bone shapes (docs/02 §4.2; hypothesis: the game does not
/// draw it).
fn is_helper(material: &Material) -> bool {
    matches!(material.surface, Surface::Color(_)) && material.name.is_none()
}

struct Builder<'m, F> {
    model: &'m Model<'m>,
    texture_uri: &'m F,
    embedded: &'m [Option<Vec<u8>>],
    stats: Stats,
    /// The binary chunk and the glTF arrays that describe it.
    buffer: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
    materials: Vec<Value>,
    /// glTF material of each GRN material, made on first use.
    material_index: HashMap<usize, usize>,
    textures: Vec<Value>,
    images: Vec<Value>,
    /// glTF texture of each image URI.
    image_index: HashMap<String, usize>,
    /// Accessor of each list of keyframe times, by the bits of the times: channels share them.
    time_index: HashMap<Vec<u32>, usize>,
    /// The bind pose of every skeleton node in glTF's frame: translation, rotation, scale.
    rest: Vec<([f32; 3], [f32; 4], [f32; 3])>,
    /// The outer node of every bone with a scale/shear off the diagonal (see [`Shear`]).
    outer: Vec<Option<usize>>,
}

/// The model's bones that a clip's bones play on: the bone with the same chain of names from the
/// root, else the only bone with the same name (names repeat in 138 files).
struct Targets<'s> {
    by_path: HashMap<Vec<&'s [u8]>, usize>,
    by_name: HashMap<&'s [u8], Vec<usize>>,
}

impl<'s> Targets<'s> {
    fn new(skeleton: &'s Skeleton) -> Self {
        let mut by_path = HashMap::new();
        let mut by_name: HashMap<_, Vec<_>> = HashMap::new();
        for (index, bone) in skeleton.bones.iter().enumerate() {
            by_path.entry(path(skeleton, index)).or_insert(index);
            by_name.entry(bone.name).or_default().push(index);
        }
        Self { by_path, by_name }
    }

    /// Where each track of `clip` plays, in track order.
    fn bind(&self, clip: &Clip) -> Result<Vec<Binding>> {
        let mut taken = HashSet::new();
        clip.animation
            .tracks
            .iter()
            .map(|track| {
                let bone = clip
                    .skeleton
                    .bone_of_channel(track.channel)
                    .with_context(|| {
                        format!(
                            "clip `{}`: no bone has channel {}",
                            clip.name, track.channel
                        )
                    })?;
                Ok(match self.find(clip.skeleton, bone) {
                    None => Binding::Missing,
                    Some(node) if !taken.insert(node) => Binding::Taken,
                    Some(node) => Binding::Node(node),
                })
            })
            .collect()
    }

    /// The model's bone for bone `bone` of `skeleton`.
    fn find(&self, skeleton: &Skeleton, bone: usize) -> Option<usize> {
        if let Some(&found) = self.by_path.get(&path(skeleton, bone)) {
            return Some(found);
        }
        match self.by_name.get(skeleton.bones[bone].name)?.as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    }
}

/// Where a track of a clip plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Binding {
    Node(usize),
    /// The model lacks the bone.
    Missing,
    /// An earlier track of the clip plays on the same bone.
    Taken,
}

/// Names of the bones from the root down to `bone`.
fn path<'s>(skeleton: &Skeleton<'s>, bone: usize) -> Vec<&'s [u8]> {
    let mut names = Vec::new();
    let mut next = Some(bone);
    while let Some(index) = next {
        names.push(skeleton.bones[index].name);
        next = skeleton.bones[index].parent;
    }
    names.reverse();
    names
}

/// Corner attribute that comes from a pass source: an entry of a set, or a constant, kept as
/// the bits of its floats so that equal constants of different passes make the same vertex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Attribute {
    Set(usize, usize),
    Constant([u32; 3]),
}

impl Attribute {
    fn of(source: Source, entries: Option<[usize; 3]>, corner: usize) -> Self {
        match (source, entries) {
            (Source::Set(set), Some(entries)) => Self::Set(set, entries[corner]),
            (Source::Set(_), None) => Self::Constant([0; 3]),
            (Source::Constant(value), _) => Self::Constant(value.map(f32::to_bits)),
        }
    }

    fn value(self, mesh: &Mesh) -> [f32; 3] {
        match self {
            Self::Set(set, entry) => mesh.texcoord_sets[set][entry],
            Self::Constant(bits) => bits.map(f32::from_bits),
        }
    }
}

/// Normal of a corner: a stored one, or that of the triangle with this index for a zero normal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Normal {
    Stored(usize),
    Face(usize),
}

/// What makes a glTF vertex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Corner {
    position: usize,
    normal: Normal,
    texcoords: Attribute,
    color: Attribute,
}

impl<F: Fn(&[u8]) -> Option<String>> Builder<'_, F> {
    /// One node per bone with the bind pose in glTF's frame, in bone order, then an outer node
    /// for every bone marked in `sheared` (see [`Shear`]); and the bind-pose world transform of
    /// every bone node. The world transforms are exact, so the inverse bind matrices undo the
    /// nodes.
    fn skeleton(
        &mut self,
        skeleton: &Skeleton,
        sheared: &[bool],
    ) -> Result<(Vec<Value>, Vec<Affine>)> {
        let bones = &skeleton.bones;
        let mut next = bones.len();
        self.outer = sheared
            .iter()
            .map(|&s| {
                s.then(|| {
                    next += 1;
                    next - 1
                })
            })
            .collect();
        let mut nodes: Vec<Value> = vec![Value::Null; next];
        self.rest = vec![([0.0; 3], [0.0, 0.0, 0.0, 1.0], [1.0; 3]); next];
        let mut world: Vec<Affine> = Vec::with_capacity(bones.len());
        for (index, bone) in bones.iter().enumerate() {
            let name = latin1(bone.name);
            let translation = to_gltf(bone.translation);
            let rotation = normalized(to_gltf_rotation(bone.rotation))
                .with_context(|| format!("bone {index}: zero rotation quaternion"))?;
            let ss = bone.scale_shear;
            let (top, local) = match self.outer[index] {
                None => {
                    let scale = to_gltf_scale([ss[0], ss[4], ss[8]]);
                    nodes[index] = json!({
                        "name": name,
                        "translation": translation,
                        "rotation": rotation,
                        "scale": scale,
                    });
                    self.rest[index] = (translation, rotation, scale);
                    (index, Affine::trs(translation, rotation, scale))
                }
                Some(outer) => {
                    self.stats.sheared_bones += 1;
                    let shear = Shear::new(&to_gltf_shear(&ss), None);
                    let outer_rotation =
                        animation::compose(&widen(rotation), &shear.rotation).map(narrow);
                    let scale = shear.scale.map(narrow);
                    let inner_rotation = animation::conjugate(&shear.rotation).map(narrow);
                    nodes[outer] = json!({
                        "name": format!("{name} scale/shear"),
                        "translation": translation,
                        "rotation": outer_rotation,
                        "scale": scale,
                        "children": [index],
                    });
                    nodes[index] = json!({"name": name, "rotation": inner_rotation});
                    self.rest[outer] = (translation, outer_rotation, scale);
                    self.rest[index] = ([0.0; 3], inner_rotation, [1.0; 3]);
                    let outer_local = Affine::trs(translation, outer_rotation, scale);
                    let inner_local = Affine::trs([0.0; 3], inner_rotation, [1.0; 3]);
                    (outer, outer_local.then(&inner_local))
                }
            };
            world.push(match bone.parent {
                Some(parent) => world[parent].then(&local),
                None => local,
            });
            if let Some(parent) = bone.parent {
                match nodes[parent].get_mut("children") {
                    Some(Value::Array(children)) => children.push(json!(top)),
                    _ => nodes[parent]["children"] = json!([top]),
                }
            }
        }
        Ok((nodes, world))
    }

    /// The mesh with one primitive per pass in `passes` and attributes shared by them.
    fn mesh(&mut self, name: &str, mesh: &Mesh, passes: &[&Pass]) -> Result<Value> {
        let with_texcoords = passes.iter().any(|p| matches!(p.texcoords, Source::Set(_)));
        let with_colors = passes.iter().any(|p| matches!(p.color, Source::Set(_)));
        self.stats.reduced_vertices += mesh
            .weights
            .iter()
            .filter(|w| w.len() > MAX_INFLUENCES)
            .count();

        let mut corners: Vec<Corner> = Vec::new();
        let mut index: HashMap<Corner, u32> = HashMap::new();
        let mut primitives = Vec::with_capacity(passes.len());
        for pass in passes {
            let mut indices = Vec::with_capacity(pass.triangles.len() * 3);
            for drawn in &pass.triangles {
                let triangle = &mesh.triangles[drawn.triangle];
                for corner in 0..3 {
                    let normal = triangle.normals[corner];
                    let normal = if mesh.normals[normal] == [0.0; 3] {
                        self.stats.zero_normals += 1;
                        Normal::Face(drawn.triangle)
                    } else {
                        Normal::Stored(normal)
                    };
                    let key = Corner {
                        position: triangle.positions[corner],
                        normal,
                        texcoords: Attribute::of(pass.texcoords, drawn.texcoords, corner),
                        color: Attribute::of(pass.color, drawn.color, corner),
                    };
                    let next = u32::try_from(corners.len()).context("too many vertices")?;
                    indices.push(match index.entry(key) {
                        Entry::Occupied(slot) => *slot.get(),
                        Entry::Vacant(slot) => {
                            corners.push(key);
                            *slot.insert(next)
                        }
                    });
                }
            }
            self.stats.triangles += pass.triangles.len();
            let indices = self.indices(&indices);
            primitives.push((indices, self.material(pass.material)));
        }
        self.stats.primitives += primitives.len();
        self.stats.vertices += corners.len();

        let positions: Vec<_> = corners
            .iter()
            .map(|c| to_gltf(mesh.positions[c.position]))
            .collect();
        let normals: Vec<_> = corners
            .iter()
            .map(|c| match c.normal {
                Normal::Stored(n) => {
                    normalized(to_gltf(mesh.normals[n])).unwrap_or([0.0, 1.0, 0.0])
                }
                Normal::Face(t) => face_normal(mesh, t),
            })
            .collect();
        let mut attributes = json!({
            "POSITION": self.vectors(&positions, true),
            "NORMAL": self.vectors(&normals, false),
        });
        if with_texcoords {
            let uv: Vec<_> = corners
                .iter()
                .map(|c| {
                    let [u, v, _] = c.texcoords.value(mesh);
                    [u, v]
                })
                .collect();
            attributes["TEXCOORD_0"] = json!(self.vectors(&uv, false));
        }
        if with_colors {
            // glTF colors stay in 0..=1; a set holds values such as -6e-8.
            let rgb: Vec<_> = corners
                .iter()
                .map(|c| c.color.value(mesh).map(|v| v.clamp(0.0, 1.0)))
                .collect();
            attributes["COLOR_0"] = json!(self.vectors(&rgb, false));
        }
        let mut joints = Vec::with_capacity(corners.len());
        let mut weights = Vec::with_capacity(corners.len());
        for corner in &corners {
            let (j, w) = influences(mesh, corner.position)?;
            joints.push(j);
            weights.push(w);
        }
        attributes["JOINTS_0"] = json!(self.joints(&joints));
        attributes["WEIGHTS_0"] = json!(self.vectors(&weights, false));

        let primitives: Vec<_> = primitives
            .into_iter()
            .map(|(indices, material)| {
                json!({"attributes": attributes, "indices": indices, "material": material})
            })
            .collect();
        Ok(json!({"name": name, "primitives": primitives}))
    }

    /// The skin of `mesh`: its bones as joints, with the inverses of their bind-pose transforms;
    /// `root` is the skeleton's top node.
    fn skin(&mut self, mesh: &Mesh, bind: &[Affine], root: usize) -> Result<Value> {
        let mut matrices = Vec::with_capacity(mesh.bones.len());
        for bone in &mesh.bones {
            let inverse = bind[bone.bone]
                .inverse()
                .with_context(|| format!("bone {}: singular bind pose", bone.bone))?;
            matrices.push(inverse.columns());
        }
        let joints: Vec<_> = mesh.bones.iter().map(|b| b.bone).collect();
        let view = self.view(&floats_le(matrices.iter().flatten()), None);
        let count = matrices.len();
        let accessor = self.accessor(view, FLOAT, count, "MAT4", None);
        Ok(json!({"joints": joints, "inverseBindMatrices": accessor, "skeleton": root}))
    }

    /// The glTF material of GRN material `index`.
    fn material(&mut self, index: usize) -> usize {
        if let Some(&made) = self.material_index.get(&index) {
            return made;
        }
        let material = &self.model.materials[index];
        let mut pbr = json!({"metallicFactor": 0.0, "roughnessFactor": 1.0});
        match material.surface {
            Surface::Color(rgba) => pbr["baseColorFactor"] = json!(rgba),
            Surface::Texture(texture) => {
                if let Some(texture) = self.texture(texture) {
                    pbr["baseColorTexture"] = json!({"index": texture});
                }
            }
        }
        let mut out = json!({"pbrMetallicRoughness": pbr});
        if let Some(name) = material.name {
            out["name"] = json!(latin1(name));
        }
        self.materials.push(out);
        let made = self.materials.len() - 1;
        self.material_index.insert(index, made);
        made
    }

    /// The glTF texture of GRN texture `index`, if its image is available.
    fn texture(&mut self, index: usize) -> Option<usize> {
        let texture = &self.model.textures[index];
        // The game binds every texture by name, whatever the file embeds.
        let Some(uri) = (self.texture_uri)(texture.name) else {
            if let Some(Some(png)) = self.embedded.get(index) {
                self.stats.embedded_textures += 1;
                let view = self.view(png, None);
                self.images.push(json!({
                    "name": latin1(texture.name),
                    "bufferView": view,
                    "mimeType": "image/png",
                }));
                self.textures.push(json!({"source": self.images.len() - 1}));
                return Some(self.textures.len() - 1);
            }
            self.stats.unnamed_textures += 1;
            return None;
        };
        if texture.format != TextureFormat::External {
            self.stats.twinned_textures += 1;
        }
        if let Some(&made) = self.image_index.get(&uri) {
            return Some(made);
        }
        self.images
            .push(json!({"name": latin1(texture.name), "uri": uri}));
        self.textures.push(json!({"source": self.images.len() - 1}));
        let made = self.textures.len() - 1;
        self.image_index.insert(uri, made);
        Some(made)
    }

    /// The glTF animation of `clip` whose tracks play as `bound` says ([`Targets::bind`]) and
    /// which holds the nodes `rest` in their bind pose; `None` if no track binds. A track gives
    /// its bone three channels: translation, rotation and scale.
    fn animation(
        &mut self,
        clip: &Clip,
        bound: &[Binding],
        rest: &[usize],
    ) -> Result<Option<Value>> {
        let mut channels = Vec::new();
        let mut samplers = Vec::new();
        for (track, binding) in clip.animation.tracks.iter().zip(bound) {
            let node = match *binding {
                Binding::Node(node) => node,
                Binding::Missing => {
                    self.stats.unbound_tracks += 1;
                    continue;
                }
                Binding::Taken => {
                    self.stats.duplicate_tracks += 1;
                    continue;
                }
            };
            self.stats.tracks += 1;
            // A sheared bone's translation, rotation and scale go to its outer node.
            let top = self.outer[node].unwrap_or(node);
            let translation = animation::vectors(&track.translation, |&k| widen(to_gltf(k)));
            if let Some(keyframes) = translation {
                channels.push(self.channel(&mut samplers, top, "translation", &keyframes));
            }
            if self.outer[node].is_some() && track.scale_shear.keys.iter().any(off_diagonal) {
                self.stats.sheared_tracks += 1;
                let keyframes = animation::sheared(
                    &track.rotation,
                    &track.scale_shear,
                    |&q| widen(to_gltf_rotation(q)),
                    |m| flatten(&to_gltf_shear(m)),
                );
                if let Some(k) = keyframes {
                    channels.push(self.channel(&mut samplers, top, "rotation", &k.outer_rotation));
                    channels.push(self.channel(&mut samplers, top, "scale", &k.scale));
                    channels.push(self.channel(&mut samplers, node, "rotation", &k.inner_rotation));
                }
                continue;
            }
            let rotation = animation::rotations(&track.rotation, |&q| widen(to_gltf_rotation(q)));
            let scale = animation::vectors(&track.scale_shear, |m| {
                widen(to_gltf_scale([m[0], m[4], m[8]]))
            });
            if let Some((keyframes, jumps)) = rotation {
                self.stats.rotation_jumps += jumps;
                channels.push(self.channel(&mut samplers, top, "rotation", &keyframes));
            }
            if let Some(keyframes) = scale {
                channels.push(self.channel(&mut samplers, top, "scale", &keyframes));
            }
            if top != node {
                // A diagonal scale/shear in this clip: the bone's own node stays unturned.
                let duration = clip.animation.duration();
                let times = if duration > 0.0 {
                    vec![0.0, duration]
                } else {
                    vec![0.0]
                };
                let unturned = held(&times, [0.0, 0.0, 0.0, 1.0]);
                channels.push(self.channel(&mut samplers, node, "rotation", &unturned));
            }
        }
        if channels.is_empty() {
            return Ok(None);
        }
        let duration = clip.animation.duration();
        let times = if duration > 0.0 {
            vec![0.0, duration]
        } else {
            vec![0.0]
        };
        for &node in rest {
            self.stats.rest_tracks += 1;
            let (translation, rotation, scale) = self.rest[node];
            let translation = held(&times, translation);
            channels.push(self.channel(&mut samplers, node, "translation", &translation));
            let rotation = held(&times, rotation);
            channels.push(self.channel(&mut samplers, node, "rotation", &rotation));
            let scale = held(&times, scale);
            channels.push(self.channel(&mut samplers, node, "scale", &scale));
        }
        let mut out = json!({"name": clip.name, "channels": channels, "samplers": samplers});
        if !clip.slots.is_empty() {
            out["extras"] = json!({"slots": clip.slots});
        }
        Ok(Some(out))
    }

    /// A channel animating `path` of node `node` with `keyframes`; its sampler is added to
    /// `samplers`.
    fn channel<const N: usize>(
        &mut self,
        samplers: &mut Vec<Value>,
        node: usize,
        path: &str,
        keyframes: &Keyframes<N>,
    ) -> Value {
        self.stats.keyframes += keyframes.times.len();
        let input = self.times(&keyframes.times);
        let view = self.view(&floats_le(keyframes.values.iter().flatten()), None);
        let kind = if N == 4 { "VEC4" } else { "VEC3" };
        let output = self.accessor(view, FLOAT, keyframes.values.len(), kind, None);
        samplers.push(json!({
            "input": input,
            "output": output,
            "interpolation": keyframes.sampling.name(),
        }));
        json!({"sampler": samplers.len() - 1, "target": {"node": node, "path": path}})
    }

    /// Accessor of keyframe times, shared by equal lists; glTF wants their bounds.
    fn times(&mut self, times: &[f32]) -> usize {
        let key: Vec<u32> = times.iter().map(|t| t.to_bits()).collect();
        if let Some(&made) = self.time_index.get(&key) {
            return made;
        }
        let view = self.view(&floats_le(times.iter()), None);
        let range = (json!([times[0]]), json!([times[times.len() - 1]]));
        let made = self.accessor(view, FLOAT, times.len(), "SCALAR", Some(range));
        self.time_index.insert(key, made);
        made
    }

    /// Accessor of `values` in a view of their own; with `bounds`, the per-component minimum
    /// and maximum, which `POSITION` requires.
    fn vectors<const N: usize>(&mut self, values: &[[f32; N]], bounds: bool) -> usize {
        let view = self.view(&floats_le(values.iter().flatten()), Some(ARRAY_BUFFER));
        let kind = match N {
            2 => "VEC2",
            3 => "VEC3",
            _ => "VEC4",
        };
        let range = bounds.then(|| {
            let mut min = [f32::INFINITY; N];
            let mut max = [f32::NEG_INFINITY; N];
            for value in values {
                for ((lo, hi), &v) in min.iter_mut().zip(&mut max).zip(value) {
                    *lo = lo.min(v);
                    *hi = hi.max(v);
                }
            }
            (json!(min.to_vec()), json!(max.to_vec()))
        });
        self.accessor(view, FLOAT, values.len(), kind, range)
    }

    fn joints(&mut self, values: &[[u16; 4]]) -> usize {
        let bytes: Vec<u8> = values
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let view = self.view(&bytes, Some(ARRAY_BUFFER));
        self.accessor(view, UNSIGNED_SHORT, values.len(), "VEC4", None)
    }

    fn indices(&mut self, values: &[u32]) -> usize {
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        let view = self.view(&bytes, Some(ELEMENT_ARRAY_BUFFER));
        self.accessor(view, UNSIGNED_INT, values.len(), "SCALAR", None)
    }

    /// Buffer view of `bytes`, aligned to 4 bytes in the binary chunk.
    fn view(&mut self, bytes: &[u8], target: Option<u32>) -> usize {
        self.buffer.resize(self.buffer.len().next_multiple_of(4), 0);
        let mut view =
            json!({"buffer": 0, "byteOffset": self.buffer.len(), "byteLength": bytes.len()});
        if let Some(target) = target {
            view["target"] = json!(target);
        }
        self.buffer.extend_from_slice(bytes);
        self.views.push(view);
        self.views.len() - 1
    }

    fn accessor(
        &mut self,
        view: usize,
        component: u32,
        count: usize,
        kind: &str,
        range: Option<(Value, Value)>,
    ) -> usize {
        let mut accessor =
            json!({"bufferView": view, "componentType": component, "count": count, "type": kind});
        if let Some((min, max)) = range {
            accessor["min"] = min;
            accessor["max"] = max;
        }
        self.accessors.push(accessor);
        self.accessors.len() - 1
    }
}

/// `JOINTS_0` and `WEIGHTS_0` of position `position`: its influences, the largest four
/// renormalized, padded with joint 0 at weight 0; a rigid mesh follows its only bone.
fn influences(mesh: &Mesh, position: usize) -> Result<([u16; 4], [f32; 4])> {
    let Some(list) = mesh.weights.get(position) else {
        return Ok(([0; 4], [1.0, 0.0, 0.0, 0.0]));
    };
    let mut list = list.clone();
    list.sort_by(|a, b| b.weight.total_cmp(&a.weight));
    list.truncate(MAX_INFLUENCES);
    let sum: f32 = list.iter().map(|i| i.weight).sum();
    if sum <= 0.0 {
        bail!("position {position}: influences without weight");
    }
    let mut joints = [0; 4];
    let mut weights = [0.0; 4];
    for (k, influence) in list.iter().enumerate() {
        joints[k] = u16::try_from(influence.bone).context("joint index above 65535")?;
        weights[k] = influence.weight / sum;
    }
    Ok((joints, weights))
}

/// Unit normal of triangle `index` in glTF's frame, or up for a triangle without area.
fn face_normal(mesh: &Mesh, index: usize) -> [f32; 3] {
    let [a, b, c] = mesh.triangles[index]
        .positions
        .map(|p| to_gltf(mesh.positions[p]));
    let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let cross = [
        e1[1] * e2[2] - e1[2] * e2[1],
        e1[2] * e2[0] - e1[0] * e2[2],
        e1[0] * e2[1] - e1[1] * e2[0],
    ];
    normalized(cross).unwrap_or([0.0, 1.0, 0.0])
}

/// Z-up (the game, 3ds Max) to Y-up (glTF): the rotation by −90° about X, `(x, y, z) →
/// (x, z, −y)`.
fn to_gltf([x, y, z]: [f32; 3]) -> [f32; 3] {
    [x, z, -y]
}

/// The same change of frame for a rotation quaternion `[x, y, z, w]`: its axis turns with it.
fn to_gltf_rotation([x, y, z, w]: [f32; 4]) -> [f32; 4] {
    [x, z, -y, w]
}

/// The same change of frame for a scale/shear matrix `SS` (9 values, symmetric):
/// `C · SS · Cᵀ` with `C` the turn above.
fn to_gltf_shear(m: &[f32; 9]) -> Mat3 {
    let m = m.map(f64::from);
    [
        [m[0], m[2], -m[1]],
        [m[6], m[8], -m[7]],
        [-m[3], -m[5], m[4]],
    ]
}

fn flatten(m: &Mat3) -> [f64; 9] {
    std::array::from_fn(|k| m[k / 3][k % 3])
}

/// The same change of frame for a diagonal scale: the Y and Z axes trade places.
fn to_gltf_scale([x, y, z]: [f32; 3]) -> [f32; 3] {
    [x, z, y]
}

/// Keyframes that hold `value` at `times`.
fn held<const N: usize>(times: &[f32], value: [f32; N]) -> Keyframes<N> {
    Keyframes {
        sampling: animation::Sampling::Linear,
        times: times.to_vec(),
        values: vec![value; times.len()],
    }
}

/// Whether a scale/shear matrix has an entry off the diagonal above 1e-4 of its largest.
fn off_diagonal(m: &[f32; 9]) -> bool {
    let largest = m.iter().fold(0.0f32, |a, v| a.max(v.abs()));
    [1, 2, 3, 5, 6, 7]
        .iter()
        .any(|&k| m[k].abs() > 1e-4 * largest)
}

fn widen<const N: usize>(v: [f32; N]) -> [f64; N] {
    v.map(f64::from)
}

/// `v` scaled to unit length, computed in `f64`; `None` for a zero vector.
fn normalized<const N: usize>(v: [f32; N]) -> Option<[f32; N]> {
    let length = v
        .iter()
        .map(|&x| f64::from(x) * f64::from(x))
        .sum::<f64>()
        .sqrt();
    (length > 0.0).then(|| v.map(|x| narrow(f64::from(x) / length)))
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "glTF stores f32; rounding to the nearest f32 is intended"
)]
fn narrow(value: f64) -> f32 {
    value as f32
}

fn floats_le<'f>(values: impl Iterator<Item = &'f f32>) -> Vec<u8> {
    values.flat_map(|v| v.to_le_bytes()).collect()
}

/// Bone names are Latin-1 (docs/02 §4.2); glTF names are UTF-8.
fn latin1(name: &[u8]) -> String {
    name.iter().map(|&b| char::from(b)).collect()
}

/// GLB container: the 12-byte header, the JSON chunk padded with spaces, the binary chunk padded
/// with zeros.
fn glb(mut json: Vec<u8>, mut bin: Vec<u8>) -> Result<Vec<u8>> {
    json.resize(json.len().next_multiple_of(4), b' ');
    bin.resize(bin.len().next_multiple_of(4), 0);
    let chunks = [(*b"JSON", json), (*b"BIN\0", bin)];
    let total = 12
        + chunks
            .iter()
            .filter(|(_, data)| !data.is_empty())
            .map(|(_, data)| 8 + data.len())
            .sum::<usize>();
    let le = |n: usize| u32::try_from(n).map(u32::to_le_bytes);
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&le(total).context("GLB over 4 GiB")?);
    for (kind, data) in chunks {
        if data.is_empty() {
            continue;
        }
        out.extend_from_slice(&le(data.len()).context("GLB chunk over 4 GiB")?);
        out.extend_from_slice(&kind);
        out.extend_from_slice(&data);
    }
    Ok(out)
}

/// Affine transform `x → linear · x + translation`, in `f64` so that composing a long bone chain
/// and inverting it loses no precision that `f32` would keep.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Affine {
    /// Rows.
    linear: [[f64; 3]; 3],
    translation: [f64; 3],
}

impl Affine {
    /// `T · R · S` with a unit quaternion `[x, y, z, w]`.
    fn trs(translation: [f32; 3], rotation: [f32; 4], scale: [f32; 3]) -> Self {
        let [x, y, z, w] = rotation.map(f64::from);
        let r = [
            [
                1.0 - 2.0 * (y * y + z * z),
                2.0 * (x * y - z * w),
                2.0 * (x * z + y * w),
            ],
            [
                2.0 * (x * y + z * w),
                1.0 - 2.0 * (x * x + z * z),
                2.0 * (y * z - x * w),
            ],
            [
                2.0 * (x * z - y * w),
                2.0 * (y * z + x * w),
                1.0 - 2.0 * (x * x + y * y),
            ],
        ];
        let s = scale.map(f64::from);
        Self {
            linear: r.map(|row| [row[0] * s[0], row[1] * s[1], row[2] * s[2]]),
            translation: translation.map(f64::from),
        }
    }

    /// `self · other`: `other` first, then `self`.
    fn then(&self, other: &Self) -> Self {
        let a = &self.linear;
        let b = &other.linear;
        let linear = std::array::from_fn(|i| {
            std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum())
        });
        let translation = std::array::from_fn(|i| {
            (0..3).map(|k| a[i][k] * other.translation[k]).sum::<f64>() + self.translation[i]
        });
        Self {
            linear,
            translation,
        }
    }

    fn inverse(&self) -> Option<Self> {
        let m = &self.linear;
        let cofactor = |i: usize, j: usize| {
            let (r0, r1) = ((i + 1) % 3, (i + 2) % 3);
            let (c0, c1) = ((j + 1) % 3, (j + 2) % 3);
            m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0]
        };
        let det: f64 = (0..3).map(|j| m[0][j] * cofactor(0, j)).sum();
        if det.abs() < 1e-12 {
            return None;
        }
        // The inverse is the transposed matrix of cofactors over the determinant.
        let linear: [[f64; 3]; 3] =
            std::array::from_fn(|i| std::array::from_fn(|j| cofactor(j, i) / det));
        let translation = std::array::from_fn(|i| {
            -(0..3)
                .map(|k| linear[i][k] * self.translation[k])
                .sum::<f64>()
        });
        Some(Self {
            linear,
            translation,
        })
    }

    /// The 4×4 matrix in glTF's column-major order.
    fn columns(&self) -> [f32; 16] {
        let mut out = [0.0; 16];
        for j in 0..3 {
            for i in 0..3 {
                out[4 * j + i] = narrow(self.linear[i][j]);
            }
        }
        for i in 0..3 {
            out[12 + i] = narrow(self.translation[i]);
        }
        out[15] = 1.0;
        out
    }
}

#[cfg(test)]
mod tests {
    use opensacred_formats::grn::{
        Bone, Curve, Influence, Interpolation, Layout, Mesh, MeshBone, PassTriangle, Texture,
        Track, Triangle,
    };

    use super::*;

    const WHITE: [f32; 3] = [1.0; 3];
    const IDENTITY: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    const HALF_TURN_SIN: f32 = std::f32::consts::FRAC_1_SQRT_2;

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

    /// A chain of five bones: `Bip01` turned 90° about Z (up in the game's frame) and scaled
    /// `(1, 2, 3)`, `Hand` sheared.
    fn skeleton() -> Skeleton<'static> {
        let mut bones = vec![
            bone(b"__Root", None, [0.0; 3]),
            bone(b"Bip01", Some(0), [0.0, 0.0, 10.0]),
            bone(b"Spine", Some(1), [1.0, 0.0, 0.0]),
            bone(b"Head", Some(2), [0.0, 0.0, 2.0]),
            bone(b"Hand", Some(3), [0.0, 1.0, 0.0]),
        ];
        for (channel, bone) in bones.iter_mut().enumerate() {
            bone.channel = channel;
        }
        bones[1].rotation = [0.0, 0.0, HALF_TURN_SIN, HALF_TURN_SIN];
        bones[1].scale_shear = [1.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0];
        bones[4].scale_shear = [1.0, 0.5, 0.0, 0.5, 1.0, 0.0, 0.0, 0.0, 1.0];
        Skeleton { bones }
    }

    fn entry(bone: usize) -> MeshBone {
        MeshBone {
            bone,
            radius: 1.0,
            min: [0.0; 3],
            max: [0.0; 3],
        }
    }

    fn weights(list: &[(usize, f32)]) -> Vec<Influence> {
        list.iter()
            .map(|&(bone, weight)| Influence { bone, weight })
            .collect()
    }

    fn rigid(name: &'static [u8], bone: usize) -> Mesh<'static> {
        Mesh {
            name,
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
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

    fn drawn(triangle: usize, texcoords: Option<[usize; 3]>) -> PassTriangle {
        PassTriangle {
            triangle,
            color: None,
            texcoords,
        }
    }

    /// A skinned body of two triangles, drawn by two passes, whose second triangle has a zero
    /// normal and whose third position has five influences; a rigid mesh on `Head` with a named
    /// color material; helper geometry with an unnamed one.
    fn model() -> Model<'static> {
        let square = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
        ];
        let body = Mesh {
            name: b"Body",
            positions: square.clone(),
            normals: vec![[0.0, -1.0, 0.0], [0.0, 0.0, 0.0]],
            texcoord_sets: vec![square],
            triangles: vec![
                Triangle {
                    positions: [0, 1, 2],
                    normals: [0, 0, 1],
                },
                Triangle {
                    positions: [2, 1, 3],
                    normals: [0, 0, 0],
                },
            ],
            weights: vec![
                weights(&[(0, 1.0)]),
                weights(&[(1, 0.5), (0, 0.5)]),
                weights(&[(4, 0.1), (1, 0.25), (0, 0.3), (3, 0.15), (2, 0.2)]),
                weights(&[(4, 1.0)]),
            ],
            bones: (0..5).map(entry).collect(),
        };
        let pass = |mesh, material, texcoords, triangles| Pass {
            mesh,
            material,
            color: Source::Constant(WHITE),
            texcoords,
            triangles,
        };
        Model {
            meshes: vec![body, rigid(b"Ring", 3), rigid(b"Bone01", 0)],
            textures: vec![Texture {
                name: b"Body_skin",
                width: 4,
                height: 4,
                format: TextureFormat::External,
                payload: b"",
            }],
            materials: vec![
                Material {
                    name: Some(b"skin"),
                    surface: Surface::Texture(0),
                },
                Material {
                    name: None,
                    surface: Surface::Color([0.7, 0.7, 0.8, 1.0]),
                },
                Material {
                    name: Some(b"gem"),
                    surface: Surface::Color([1.0, 0.0, 0.0, 1.0]),
                },
            ],
            passes: vec![
                pass(0, 0, Source::Set(0), vec![drawn(1, Some([2, 1, 3]))]),
                pass(1, 2, Source::Constant([0.0; 3]), vec![drawn(0, None)]),
                pass(2, 1, Source::Constant([0.0; 3]), vec![drawn(0, None)]),
                pass(0, 0, Source::Set(0), vec![drawn(0, Some([0, 1, 2]))]),
            ],
        }
    }

    fn uri(name: &[u8]) -> Option<String> {
        (name == b"Body_skin").then(|| "../tex/body_skin.png".to_owned())
    }

    /// A GLB file split into its JSON and its binary chunk.
    struct Glb {
        json: Value,
        bin: Vec<u8>,
    }

    impl Glb {
        fn read(data: &[u8]) -> Self {
            let word = |at: usize| {
                usize::try_from(u32::from_le_bytes(data[at..at + 4].try_into().unwrap())).unwrap()
            };
            assert_eq!(&data[..4], b"glTF");
            assert_eq!((word(4), word(8)), (2, data.len()));
            let json_len = word(12);
            assert_eq!(&data[16..20], b"JSON");
            let json = serde_json::from_slice(&data[20..20 + json_len]).unwrap();
            let bin_at = 20 + json_len;
            assert_eq!(&data[bin_at + 4..bin_at + 8], b"BIN\0");
            let bin = data[bin_at + 8..bin_at + 8 + word(bin_at)].to_vec();
            assert_eq!(bin_at + 8 + bin.len(), data.len());
            Self { json, bin }
        }

        fn field(&self, value: &Value) -> usize {
            usize::try_from(value.as_u64().unwrap()).unwrap()
        }

        /// Bytes of accessor `index` and its component type.
        fn bytes(&self, index: usize) -> (&[u8], u64) {
            let accessor = &self.json["accessors"][index];
            let view = &self.json["bufferViews"][self.field(&accessor["bufferView"])];
            let start = self.field(&view["byteOffset"]);
            let end = start + self.field(&view["byteLength"]);
            (
                &self.bin[start..end],
                accessor["componentType"].as_u64().unwrap(),
            )
        }

        fn floats(&self, index: &Value) -> Vec<f32> {
            let (bytes, kind) = self.bytes(self.field(index));
            assert_eq!(kind, u64::from(FLOAT));
            bytes
                .chunks(4)
                .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                .collect()
        }

        fn ints(&self, index: &Value) -> Vec<u32> {
            match self.bytes(self.field(index)) {
                (bytes, 5123) => bytes
                    .chunks(2)
                    .map(|c| u32::from(u16::from_le_bytes(c.try_into().unwrap())))
                    .collect(),
                (bytes, 5125) => bytes
                    .chunks(4)
                    .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                    .collect(),
                (_, kind) => panic!("component type {kind}"),
            }
        }
    }

    fn export_sample() -> (Glb, Stats) {
        let (data, stats) = export(&skeleton(), &model(), &[], &[], uri).unwrap();
        (Glb::read(&data), stats)
    }

    fn assert_near(found: &[f32], expected: &[f32]) {
        assert_eq!(found.len(), expected.len(), "{found:?}");
        for (a, b) in found.iter().zip(expected) {
            assert!((a - b).abs() < 1e-6, "{found:?} != {expected:?}");
        }
    }

    #[test]
    fn counts_what_it_kept_dropped_and_replaced() {
        let (_, stats) = export_sample();
        assert_eq!(
            stats,
            Stats {
                meshes: 2,
                helper_meshes: 1,
                primitives: 3,
                // The body's corners (2, 1, 3) and (0, 1, 2) share position 1 with the same
                // normal and coordinates; position 2 comes once with the zero normal replaced.
                vertices: 5 + 3,
                triangles: 3,
                zero_normals: 1,
                reduced_vertices: 1,
                sheared_bones: 1,
                embedded_textures: 0,
                unnamed_textures: 0,
                ..Stats::default()
            }
        );
    }

    #[test]
    fn bones_become_nodes_in_the_y_up_frame() {
        let (glb, _) = export_sample();
        let nodes = &glb.json["nodes"];
        assert_eq!(nodes[0]["name"], "__Root");
        assert_eq!(nodes[0]["children"], json!([1]));
        let bip = &nodes[1];
        // Up (Z) becomes Y; the turn about Z becomes a turn about Y; the Y and Z scales swap.
        assert_eq!(bip["translation"], json!([0.0, 10.0, -0.0]));
        assert_eq!(
            bip["rotation"],
            json!([0.0, HALF_TURN_SIN, -0.0, HALF_TURN_SIN])
        );
        assert_eq!(bip["scale"], json!([1.0, 3.0, 2.0]));
        // The sheared hand: an outer node with the translation and the scale along the shear's
        // axes, and the hand's own node under it, turned back.
        assert_eq!(nodes[3]["children"], json!([5]));
        let outer = &nodes[5];
        assert_eq!(outer["name"], "Hand scale/shear");
        assert_eq!(outer["children"], json!([4]));
        assert_eq!(outer["translation"], json!([0.0, 0.0, -1.0]));
        let mut scale: Vec<f64> = (0..3)
            .map(|k| outer["scale"][k].as_f64().unwrap())
            .collect();
        scale.sort_by(f64::total_cmp);
        assert_near(
            &scale.iter().map(|&v| narrow(v)).collect::<Vec<_>>(),
            &[0.5, 1.0, 1.5],
        );
        assert_eq!(nodes[4]["name"], "Hand");
        assert!(nodes[4].get("scale").is_none() && nodes[4].get("translation").is_none());
        // Mesh nodes after the skeleton, at the scene's top level.
        assert_eq!(glb.json["scenes"][0]["nodes"], json!([0, 6, 7]));
        assert_eq!(nodes[7]["name"], "Ring");
    }

    /// World transforms of the skeleton nodes of `json`, from their translations, rotations and
    /// scales.
    fn node_worlds(json: &Value) -> Vec<Affine> {
        let nodes = json["nodes"].as_array().unwrap();
        let floats = |node: &Value, key: &str, default: &[f32]| -> Vec<f32> {
            node.get(key).map_or_else(
                || default.to_vec(),
                |v| {
                    let list = v.as_array().unwrap();
                    list.iter().map(|x| narrow(x.as_f64().unwrap())).collect()
                },
            )
        };
        let local = |node: &Value| {
            Affine::trs(
                floats(node, "translation", &[0.0; 3]).try_into().unwrap(),
                floats(node, "rotation", &[0.0, 0.0, 0.0, 1.0])
                    .try_into()
                    .unwrap(),
                floats(node, "scale", &[1.0; 3]).try_into().unwrap(),
            )
        };
        let parent = |index: usize| {
            nodes.iter().position(|p| {
                p["children"]
                    .as_array()
                    .is_some_and(|c| c.contains(&json!(index)))
            })
        };
        let mut world = Vec::new();
        for index in 0..nodes.len() {
            let mut chain = vec![index];
            while let Some(up) = parent(*chain.last().unwrap()) {
                chain.push(up);
            }
            let w = chain
                .iter()
                .rev()
                .fold(None, |acc: Option<Affine>, &i| {
                    let l = local(&nodes[i]);
                    Some(acc.map_or(l, |a| a.then(&l)))
                })
                .unwrap();
            world.push(w);
        }
        world
    }

    /// The bind pose is exact: each joint's world transform from the nodes times its inverse
    /// bind matrix is the identity.
    #[test]
    fn inverse_bind_matrices_undo_the_nodes() {
        let (glb, _) = export_sample();
        let world = node_worlds(&glb.json);
        for skin in glb.json["skins"].as_array().unwrap() {
            let matrices = glb.floats(&skin["inverseBindMatrices"]);
            for (k, joint) in skin["joints"].as_array().unwrap().iter().enumerate() {
                let columns = world[glb.field(joint)].columns();
                // Column-major 4×4 product of the world transform and the inverse bind matrix.
                let m = &matrices[16 * k..16 * k + 16];
                for i in 0..4 {
                    for j in 0..4 {
                        let value: f32 = (0..4).map(|n| columns[4 * n + i] * m[4 * j + n]).sum();
                        let expected = if i == j { 1.0 } else { 0.0 };
                        assert!((value - expected).abs() < 1e-5, "joint {joint}: {value}");
                    }
                }
            }
        }
    }

    /// `T · R · SS` of a bone in glTF's frame, its full scale/shear included.
    fn exact_local(bone: &Bone) -> Affine {
        let r = Affine::trs([0.0; 3], to_gltf_rotation(bone.rotation), [1.0; 3]);
        let ss = to_gltf_shear(&bone.scale_shear);
        Affine {
            linear: std::array::from_fn(|i| {
                std::array::from_fn(|j| (0..3).map(|k| r.linear[i][k] * ss[k][j]).sum())
            }),
            translation: to_gltf(bone.translation).map(f64::from),
        }
    }

    /// With the outer node, a sheared bone's world transform is the original's exactly.
    #[test]
    fn a_sheared_bone_keeps_its_whole_scale_shear() {
        let (glb, _) = export_sample();
        let world = node_worlds(&glb.json);
        let bones = skeleton().bones;
        let mut exact: Vec<Affine> = Vec::new();
        for bone in &bones {
            let local = exact_local(bone);
            exact.push(bone.parent.map_or(local, |p| exact[p].then(&local)));
        }
        for (found, expected) in world.iter().zip(&exact) {
            assert_near(&found.columns(), &expected.columns());
        }
    }

    fn curve<const N: usize>(
        interpolation: Interpolation,
        times: &[f32],
        keys: &[[f32; N]],
    ) -> Curve<N> {
        Curve {
            interpolation,
            times: times.to_vec(),
            keys: keys.to_vec(),
        }
    }

    /// Track of `channel` with a moving translation, a still rotation and the scale/shear keys
    /// `scale_shear` at 0 and 1 s.
    fn track(channel: usize, scale_shear: [[f32; 9]; 2]) -> Track {
        Track {
            channel,
            layout: Layout::Split,
            translation: curve(
                Interpolation::Quadratic,
                &[0.0, 0.5, 1.0],
                &[[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 1.0]],
            ),
            rotation: curve(
                Interpolation::Quadratic,
                &[0.0, 1.0],
                &[[0.0, 0.0, 0.0, 1.0]; 2],
            ),
            scale_shear: curve(Interpolation::Linear, &[0.0, 1.0], &scale_shear),
        }
    }

    /// The motion's skeleton: the model's bones in another order with other channels, and a
    /// `Footsteps` helper the model lacks.
    fn motion_skeleton() -> Skeleton<'static> {
        let names: [(&[u8], Option<usize>); 6] = [
            (b"__Root", None),
            (b"Bip01", Some(0)),
            (b"Footsteps", Some(1)),
            (b"Spine", Some(1)),
            (b"Head", Some(3)),
            (b"Hand", Some(4)),
        ];
        let bones = names
            .iter()
            .enumerate()
            .map(|(k, &(name, parent))| Bone {
                channel: 5 - k,
                ..bone(name, parent, [0.0; 3])
            })
            .collect();
        Skeleton { bones }
    }

    #[test]
    fn clips_become_animations_on_the_bones_with_the_same_names() {
        let motion = motion_skeleton();
        let sheared = [1.0, 0.25, 0.0, 0.25, 1.0, 0.0, 0.0, 0.0, 1.0];
        let walk = Animation {
            tracks: vec![
                track(4, [IDENTITY; 2]),       // Bip01
                track(0, [IDENTITY, sheared]), // Hand, sheared in this clip
                track(3, [IDENTITY; 2]),       // Footsteps: not in the model
                track(4, [IDENTITY; 2]),       // Bip01 again
            ],
        };
        let idle = Animation {
            tracks: vec![track(2, [IDENTITY; 2])], // Spine
        };
        let clips = [
            Clip {
                name: "WALK",
                slots: &[27, 28],
                skeleton: &motion,
                animation: &walk,
            },
            Clip {
                name: "IDLE",
                slots: &[],
                skeleton: &motion,
                animation: &idle,
            },
        ];
        let (data, stats) = export(&skeleton(), &model(), &clips, &[], uri).unwrap();
        assert_eq!(
            (
                stats.animations,
                stats.tracks,
                stats.unbound_tracks,
                stats.duplicate_tracks
            ),
            (2, 3, 1, 1)
        );
        // WALK holds the spine still; IDLE the hand's two nodes and Bip01.
        assert_eq!((stats.sheared_tracks, stats.rest_tracks), (1, 4));
        let glb = Glb::read(&data);
        let animations = glb.json["animations"].as_array().unwrap();
        let walk = &animations[0];
        assert_eq!(walk["name"], "WALK");
        assert_eq!(walk["extras"], json!({"slots": [27, 28]}));
        assert!(animations[1].get("extras").is_none());

        let channels: Vec<(u64, &str, &str)> = walk["channels"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                let sampler = &walk["samplers"][glb.field(&c["sampler"])];
                (
                    c["target"]["node"].as_u64().unwrap(),
                    c["target"]["path"].as_str().unwrap(),
                    sampler["interpolation"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            channels,
            [
                (1, "translation", "CUBICSPLINE"),
                (1, "rotation", "LINEAR"),
                (1, "scale", "LINEAR"),
                // The hand: its outer node takes the translation, the rotation and the scale.
                (5, "translation", "CUBICSPLINE"),
                (5, "rotation", "LINEAR"),
                (5, "scale", "LINEAR"),
                (4, "rotation", "LINEAR"),
                // The spine, which IDLE moves, in its bind pose.
                (2, "translation", "LINEAR"),
                (2, "rotation", "LINEAR"),
                (2, "scale", "LINEAR"),
            ]
        );
        // Every sampler's input has bounds, and a cubic spline three outputs per input.
        for sampler in walk["samplers"].as_array().unwrap() {
            let input = &glb.json["accessors"][glb.field(&sampler["input"])];
            let output = &glb.json["accessors"][glb.field(&sampler["output"])];
            assert_eq!(input["min"], json!([0.0]));
            assert_eq!(input["max"], json!([1.0]));
            let per_key = if sampler["interpolation"] == "CUBICSPLINE" {
                3
            } else {
                1
            };
            assert_eq!(
                output["count"],
                json!(per_key * input["count"].as_u64().unwrap())
            );
        }
        // Equal time lists share an accessor.
        let inputs: HashSet<_> = walk["samplers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["input"].as_u64().unwrap())
            .collect();
        assert!(inputs.len() < 5, "{inputs:?}");
    }

    #[test]
    fn a_mesh_shares_its_vertices_among_its_passes() {
        let (glb, _) = export_sample();
        let body = &glb.json["meshes"][0];
        assert_eq!(body["name"], "Body");
        let primitives = body["primitives"].as_array().unwrap();
        assert_eq!(primitives.len(), 2);
        let attributes = &primitives[0]["attributes"];
        assert_eq!(attributes, &primitives[1]["attributes"]);
        // Pass 0 draws triangle 1, pass 3 triangle 0; corner 1 of both is position 1.
        assert_eq!(glb.ints(&primitives[0]["indices"]), [0, 1, 2]);
        assert_eq!(glb.ints(&primitives[1]["indices"]), [3, 1, 4]);
        let positions = glb.floats(&attributes["POSITION"]);
        assert_near(&positions[3..6], &[1.0, 0.0, -0.0]);
        assert_near(&positions[6..9], &[1.0, 1.0, -0.0]);
        let accessor = &glb.json["accessors"][glb.field(&attributes["POSITION"])];
        assert_eq!(accessor["min"], json!([0.0, 0.0, -0.0]));
        assert_eq!(accessor["max"], json!([1.0, 1.0, -0.0]));
        // Every normal faces glTF's front, the zero one replaced by its triangle's.
        let normals = glb.floats(&attributes["NORMAL"]);
        for normal in normals.chunks(3) {
            assert_near(normal, &[0.0, 0.0, 1.0]);
        }
        let uv = glb.floats(&attributes["TEXCOORD_0"]);
        assert_near(&uv, &[0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert!(attributes.get("COLOR_0").is_none());
    }

    #[test]
    fn weights_keep_the_four_largest_influences() {
        let (glb, _) = export_sample();
        let attributes = &glb.json["meshes"][0]["primitives"][0]["attributes"];
        let joints = glb.ints(&attributes["JOINTS_0"]);
        let weights = glb.floats(&attributes["WEIGHTS_0"]);
        // Vertex 0 is position 2: of five influences the largest four, renormalized.
        assert_eq!(joints[..4], [0, 1, 2, 3]);
        assert_near(
            &weights[..4],
            &[0.3 / 0.9, 0.25 / 0.9, 0.2 / 0.9, 0.15 / 0.9],
        );
        // Vertex 1 is position 1, two influences; vertex 2 position 3, one.
        assert_eq!(joints[4..12], [1, 0, 0, 0, 4, 0, 0, 0]);
        assert_near(&weights[4..12], &[0.5, 0.5, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn a_rigid_mesh_is_skinned_to_its_bone() {
        let (glb, _) = export_sample();
        assert_eq!(glb.json["skins"][1]["joints"], json!([3]));
        let primitive = &glb.json["meshes"][1]["primitives"][0];
        let attributes = &primitive["attributes"];
        assert_eq!(glb.ints(&attributes["JOINTS_0"]), [0; 12]);
        assert_near(
            &glb.floats(&attributes["WEIGHTS_0"]),
            &[1.0, 0.0, 0.0, 0.0].repeat(3),
        );
        assert!(attributes.get("TEXCOORD_0").is_none());
    }

    #[test]
    fn materials_refer_to_textures_by_uri() {
        let (glb, _) = export_sample();
        let materials = &glb.json["materials"];
        assert_eq!(materials.as_array().unwrap().len(), 2);
        assert_eq!(materials[0]["name"], "skin");
        assert_eq!(
            materials[0]["pbrMetallicRoughness"]["baseColorTexture"],
            json!({"index": 0})
        );
        assert_eq!(
            glb.json["images"],
            json!([{"name": "Body_skin", "uri": "../tex/body_skin.png"}])
        );
        assert_eq!(glb.json["textures"], json!([{"source": 0}]));
        assert_eq!(
            materials[1]["pbrMetallicRoughness"]["baseColorFactor"],
            json!([1.0, 0.0, 0.0, 1.0])
        );
        assert_eq!(glb.json["meshes"][1]["primitives"][0]["material"], 1);
    }

    #[test]
    fn textures_without_image_are_counted() {
        let (data, stats) = export(&skeleton(), &model(), &[], &[], |_| None).unwrap();
        assert_eq!((stats.unnamed_textures, stats.embedded_textures), (1, 0));
        let glb = Glb::read(&data);
        assert!(glb.json.get("images").is_none());
        assert!(
            glb.json["materials"][0]["pbrMetallicRoughness"]
                .get("baseColorTexture")
                .is_none()
        );
    }

    /// The game binds an embedded texture by name too; without an image of its name, the
    /// embedded one goes into the file.
    #[test]
    fn embedded_textures_prefer_the_image_of_their_name() {
        let mut embedded = model();
        embedded.textures[0].format = TextureFormat::VTex;
        let (data, stats) = export(&skeleton(), &embedded, &[], &[], uri).unwrap();
        assert_eq!(
            (
                stats.twinned_textures,
                stats.embedded_textures,
                stats.unnamed_textures
            ),
            (1, 0, 0)
        );
        let glb = Glb::read(&data);
        assert_eq!(glb.json["images"][0]["uri"], "../tex/body_skin.png");

        let png = b"PNG fake".to_vec();
        let (data, stats) =
            export(&skeleton(), &embedded, &[], &[Some(png.clone())], |_| None).unwrap();
        assert_eq!(
            (
                stats.twinned_textures,
                stats.embedded_textures,
                stats.unnamed_textures
            ),
            (0, 1, 0)
        );
        let glb = Glb::read(&data);
        let image = &glb.json["images"][0];
        assert_eq!(image["mimeType"], "image/png");
        assert_eq!(image["name"], "Body_skin");
        let view = &glb.json["bufferViews"][glb.field(&image["bufferView"])];
        let start = glb.field(&view["byteOffset"]);
        assert_eq!(&glb.bin[start..start + png.len()], png.as_slice());
        assert_eq!(
            glb.json["materials"][0]["pbrMetallicRoughness"]["baseColorTexture"],
            json!({"index": 0})
        );
    }

    #[test]
    fn a_color_set_becomes_clamped_vertex_colors() {
        let mut model = model();
        model.meshes[0]
            .texcoord_sets
            .push(vec![[1.5, 0.5, -6e-8]; 4]);
        model.passes[0].color = Source::Set(1);
        model.passes[0].triangles[0].color = Some([0, 1, 2]);
        let (data, _) = export(&skeleton(), &model, &[], &[], uri).unwrap();
        let glb = Glb::read(&data);
        let attributes = &glb.json["meshes"][0]["primitives"][0]["attributes"];
        let colors = glb.floats(&attributes["COLOR_0"]);
        assert_near(&colors[..3], &[1.0, 0.5, 0.0]);
        // Pass 3 has a white constant color.
        assert_near(&colors[colors.len() - 3..], &WHITE);
    }
}
