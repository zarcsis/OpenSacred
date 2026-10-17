//! Meshes of a GRN file and what draws them (docs/02 §4.2): the meshes with their vertex lists,
//! triangles and weights; the form, which binds every mesh to bones of the skeleton; the
//! textures; the materials; and the passes of the model, each drawing triangles of one mesh with
//! one material. Every vertex attribute has indices of its own, as in 3ds Max: a triangle indexes
//! positions and normals, and a pass gives the texture coordinates of its corners. Tag names
//! follow grn2glb (`src/core/grn_types.h`, MIT); the layout is confirmed on every file by
//! `tools/analysis/formats/grn_dump.py`.

use super::objects::TAG_OBJECT_REF;
use super::skeleton::{TAG_FORM, TAG_FORMS};
use super::{Node, Objects, Section, TAG_MESH, as_u64, one_based};
use crate::error::{ErrorKind, Result, field, to_usize};
use crate::reader::Reader;

const TAG_MESHES: u32 = 0xCA5E_0602;
const TAG_VERTEX_SETS: u32 = 0xCA5E_0604;
const TAG_VERTEX_SET: u32 = 0xCA5E_0603;
const TAG_POSITIONS: u32 = 0xCA5E_0801;
const TAG_NORMALS: u32 = 0xCA5E_0802;
const TAG_TEXCOORD_SETS: u32 = 0xCA5E_0804;
const TAG_TEXCOORD_SET: u32 = 0xCA5E_0803;
const TAG_WEIGHTS: u32 = 0xCA5E_0702;
const TAG_TRIANGLES: u32 = 0xCA5E_0901;
const TAG_FORM_MESHES: u32 = 0xCA5E_0C06;
const TAG_FORM_MESH: u32 = 0xCA5E_0C03;
/// Always empty; grn2glb calls it the form mesh info.
const TAG_FORM_MESH_INFO: u32 = 0xCA5E_0C04;
const TAG_MESH_BONES: u32 = 0xCA5E_0C09;
const TAG_MESH_BONE: u32 = 0xCA5E_0C0A;
const TAG_TEXTURES: u32 = 0xCA5E_0304;
const TAG_TEXTURE: u32 = 0xCA5E_0301;
const TAG_IMAGES: u32 = 0xCA5E_0305;
const TAG_IMAGE: u32 = 0xCA5E_0303;
const TAG_MATERIALS: u32 = 0xCA5E_0D01;
const TAG_MATERIAL: u32 = 0xCA5E_0D00;
const TAG_MATERIAL_COLOR: u32 = 0xCA5E_0D02;
const TAG_MATERIAL_TEXTURE: u32 = 0xCA5E_0D03;
const TAG_MODELS: u32 = 0xCA5E_0E01;
const TAG_MODEL: u32 = 0xCA5E_0E00;
const TAG_PASSES: u32 = 0xCA5E_0E07;
const TAG_PASS: u32 = 0xCA5E_0E02;
const TAG_SOURCES: u32 = 0xCA5E_0E03;
const TAG_SOURCE_SET: u32 = 0xCA5E_0E04;
const TAG_SOURCE_CONSTANT: u32 = 0xCA5E_0E05;
const TAG_PASS_TRIANGLES: u32 = 0xCA5E_0E06;
const VECTOR_SIZE: usize = 12;
const TRIANGLE_SIZE: usize = 24;
const MESH_BONE_SIZE: usize = 32;

/// The meshes of an object section and what draws them. File references, numbered from 1, are
/// resolved to indices from 0.
#[derive(Debug, Clone, PartialEq)]
pub struct Model<'a> {
    /// In file order; none in 4 model files and in the motion files other than 23.
    pub meshes: Vec<Mesh<'a>>,
    pub textures: Vec<Texture<'a>>,
    pub materials: Vec<Material<'a>>,
    /// In file order.
    pub passes: Vec<Pass>,
}

/// Mesh: lists of vertex attributes, each indexed on its own. 17 meshes of the game have no
/// vertices and no triangles.
#[derive(Debug, Clone, PartialEq)]
pub struct Mesh<'a> {
    /// `__ObjectName` of the mesh's object.
    pub name: &'a [u8],
    /// In the model frame of the bind pose; Z is up.
    pub positions: Vec<[f32; 3]>,
    /// Unit, except `(0, 0, 0)` ones that some triangle corners use.
    pub normals: Vec<[f32; 3]>,
    /// Texture-coordinate sets, 0–2, entries `[u, v, w]`; a pass picks its set.
    pub texcoord_sets: Vec<Vec<[f32; 3]>>,
    pub triangles: Vec<Triangle>,
    /// One list per position; empty for a rigid mesh, which follows `bones[0]`.
    pub weights: Vec<Vec<Influence>>,
    /// The skeleton bones the form binds the mesh to; an [`Influence`] counts them from 0. A rigid
    /// mesh has one.
    pub bones: Vec<MeshBone>,
}

/// Triangle (24 B): `u32` positions, then `u32` normals, of its three corners. In 99.4% of the
/// game's triangles the corners run counter-clockwise around the normals, in a right-handed frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Triangle {
    pub positions: [usize; 3],
    pub normals: [usize; 3],
}

/// Influence of a bone on a vertex. The influences of a vertex, 1–7 of them in no fixed order,
/// name distinct bones; in the game files their weights are positive and sum to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Influence {
    /// Index into [`Mesh::bones`].
    pub bone: usize,
    pub weight: f32,
}

/// Bone a form mesh binds (32 B): the skeleton bone and, in its bind-pose frame, a sphere around
/// its origin and a box that hold the positions weighted on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshBone {
    /// Index into [`super::Skeleton::bones`].
    pub bone: usize,
    pub radius: f32,
    pub min: [f32; 3],
    pub max: [f32; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Texture<'a> {
    /// `__ObjectName` of the texture's object; the game looks the texture up in `texture.pak` as
    /// `"%s.TGA"` (community-reported).
    pub name: &'a [u8],
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    /// The image data that follows the header, empty for [`TextureFormat::External`].
    pub payload: &'a [u8],
}

/// Texture format code; the names follow grn2glb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    /// 0: `width × height × 4` bytes.
    Rgbx,
    /// 1: `width × height × 4` bytes.
    Rgba,
    /// 4: a VTex stream of the Bink family, from the bytes `BIKi`.
    VTex,
    /// 5: a VTex stream with alpha.
    VTexAlpha,
    /// 7: an image outside the file, without payload.
    External,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material<'a> {
    /// `__ObjectName` of the material's object; in the game files only color materials go without.
    pub name: Option<&'a [u8]>,
    pub surface: Surface,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Surface {
    /// Index into [`Model::textures`]. In every file of the game each texture has one material.
    Texture(usize),
    /// RGBA; the game's color materials paint helper geometry.
    Color([f32; 4]),
}

/// Pass: triangles of one mesh drawn with one material.
#[derive(Debug, Clone, PartialEq)]
pub struct Pass {
    /// Index into [`Model::meshes`], resolved through the form mesh the pass names.
    pub mesh: usize,
    /// Index into [`Model::materials`].
    pub material: usize,
    /// The vertex color: a white constant in the game files, except two with a set of `(1, 1, 1)`.
    pub color: Source,
    /// Texture coordinates: a set, or a constant for a mesh without sets.
    pub texcoords: Source,
    /// The passes of a mesh draw each of its triangles once, in any interleaving.
    pub triangles: Vec<PassTriangle>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Source {
    Constant([f32; 3]),
    /// Index into [`Mesh::texcoord_sets`].
    Set(usize),
}

/// Triangle of a pass with the set entries of its corners, for each source that is a set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PassTriangle {
    /// Index into [`Mesh::triangles`].
    pub triangle: usize,
    pub color: Option<[usize; 3]>,
    pub texcoords: Option<[usize; 3]>,
}

impl<'a> Model<'a> {
    /// The meshes `0602 → 0601…`, bound by the only form `0C01 → 0C00 → 0C06 → 0C03…` to a
    /// skeleton of `bones` bones; the textures `0304 → 0301…`; the materials `0D01 → 0D00…`; the
    /// passes of the only model `0E01 → 0E00 → 0E07 → 0E02…`.
    pub fn parse(section: &Section<'a>, objects: &Objects<'a>, bones: usize) -> Result<Self> {
        let list = section.child(None, TAG_MESHES, "GRN meshes")?;
        let mesh_nodes = section.list(Some(list), TAG_MESH, "GRN meshes")?;
        let (form_meshes, mut bindings) = read_form_meshes(section, mesh_nodes.len(), bones)?;
        let mut meshes = Vec::with_capacity(mesh_nodes.len());
        for (node, binding) in mesh_nodes.iter().zip(&mut bindings) {
            let bones = binding.take().ok_or_else(|| {
                node.data().error(ErrorKind::TagCount {
                    what: "GRN form meshes binding the mesh",
                    tag: TAG_FORM_MESH,
                    found: 0,
                })
            })?;
            meshes.push(read_mesh(section, node, objects, bones)?);
        }
        let textures = read_textures(section, objects)?;
        let materials = read_materials(section, objects, textures.len())?;
        let passes = read_passes(section, &mesh_nodes, &meshes, &form_meshes, materials.len())?;
        Ok(Self {
            meshes,
            textures,
            materials,
            passes,
        })
    }
}

/// The form meshes in file order as mesh indices, and the bone list of every mesh.
type FormMeshes = (Vec<usize>, Vec<Option<Vec<MeshBone>>>);

/// Form mesh: `u32` mesh, numbered from 1, each mesh once; an empty `0C04`; `0C09` → the
/// [`MeshBone`] entries `0C0A`.
fn read_form_meshes(section: &Section, meshes: usize, bones: usize) -> Result<FormMeshes> {
    let list = section.child(None, TAG_FORMS, "GRN forms")?;
    let form = section.child(Some(list), TAG_FORM, "GRN forms")?;
    let list = section.child(Some(form), TAG_FORM_MESHES, "GRN form")?;
    let mut order = Vec::with_capacity(meshes);
    let mut bindings = vec![None; meshes];
    for node in section.list(Some(list), TAG_FORM_MESH, "GRN form meshes")? {
        let mesh = one_based(&mut node.sized(4, "form mesh size")?, meshes, "form mesh")?;
        section
            .child(Some(node), TAG_FORM_MESH_INFO, "GRN form mesh")?
            .sized(0, "form mesh info size")?;
        let list = section.child(Some(node), TAG_MESH_BONES, "GRN form mesh")?;
        let mut entries = Vec::new();
        for entry in section.list(Some(list), TAG_MESH_BONE, "GRN mesh bones")? {
            let mut r = entry.sized(MESH_BONE_SIZE, "mesh bone size")?;
            entries.push(MeshBone {
                bone: index(&mut r, bones, "mesh bone")?,
                radius: r.f32()?,
                min: r.f32s()?,
                max: r.f32s()?,
            });
        }
        let binding = bindings.get_mut(mesh).filter(|binding| binding.is_none());
        let Some(binding) = binding else {
            return Err(node.data().error(ErrorKind::TagCount {
                what: "GRN form meshes binding the mesh",
                tag: TAG_FORM_MESH,
                found: 2,
            }));
        };
        *binding = Some(entries);
        order.push(mesh);
    }
    Ok((order, bindings))
}

/// Mesh: `0604 → 0603` with the positions `0801`, the normals `0802` and the sets `0804 →
/// 0803…`, missing in an empty mesh; the weights `0702`; the triangles `0901`; the object `0F04`.
fn read_mesh<'a>(
    section: &Section<'a>,
    node: &Node<'a>,
    objects: &Objects<'a>,
    bones: Vec<MeshBone>,
) -> Result<Mesh<'a>> {
    let name = object_name(section, node, objects, "GRN mesh")?;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut texcoord_sets = Vec::new();
    if let Some(list) = section.optional_child(Some(node), TAG_VERTEX_SETS, "GRN mesh")? {
        let set = section.child(Some(list), TAG_VERTEX_SET, "GRN vertex sets")?;
        let positions_node = section.child(Some(set), TAG_POSITIONS, "GRN vertex set")?;
        positions = vectors(positions_node.data(), "positions size")?;
        let normals_node = section.child(Some(set), TAG_NORMALS, "GRN vertex set")?;
        normals = vectors(normals_node.data(), "normals size")?;
        let list = section.child(Some(set), TAG_TEXCOORD_SETS, "GRN vertex set")?;
        for set in section.list(Some(list), TAG_TEXCOORD_SET, "GRN texture-coordinate sets")? {
            let mut r = set.data();
            let at = r.offset();
            let components = r.u32()?;
            if components != 3 {
                return Err(r.error_at(
                    at,
                    field(
                        "texture-coordinate components",
                        components.into(),
                        "3".into(),
                    ),
                ));
            }
            texcoord_sets.push(vectors(r, "texture-coordinate set size")?);
        }
    }

    let mut r = section.child(Some(node), TAG_TRIANGLES, "GRN mesh")?.data();
    check_multiple(&r, TRIANGLE_SIZE, "triangles size")?;
    let mut triangles = Vec::with_capacity(r.rest().len() / TRIANGLE_SIZE);
    while !r.rest().is_empty() {
        triangles.push(Triangle {
            positions: corners(&mut r, positions.len(), "triangle position")?,
            normals: corners(&mut r, normals.len(), "triangle normal")?,
        });
    }

    let weights = read_weights(
        section.child(Some(node), TAG_WEIGHTS, "GRN mesh")?.data(),
        positions.len(),
        bones.len(),
    )?;
    if weights.is_empty() && bones.len() != 1 {
        return Err(node.data().error(field(
            "bones of a rigid mesh",
            as_u64(bones.len()),
            "1".into(),
        )));
    }
    Ok(Mesh {
        name,
        positions,
        normals,
        texcoord_sets,
        triangles,
        weights,
        bones,
    })
}

/// Weights: `u32` vertices, 0 for a rigid mesh or else the position count; `u32` the highest
/// local bone and `u32` the most influences on a vertex, both 0 for a rigid mesh; then per vertex
/// `u32 n` and `n × {u32 local bone, f32 weight}`, filling the node.
fn read_weights(mut r: Reader, positions: usize, bones: usize) -> Result<Vec<Vec<Influence>>> {
    let count_at = r.offset();
    let count = r.u32()?;
    let highest_at = r.offset();
    let highest = r.u32()?;
    let most_at = r.offset();
    let most = r.u32()?;
    if count != 0 && to_usize(count) != positions {
        return Err(r.error_at(
            count_at,
            field(
                "weighted vertices",
                count.into(),
                format!("0 or {positions}, the positions"),
            ),
        ));
    }
    let mut weights = Vec::with_capacity(to_usize(count));
    let (mut seen_highest, mut seen_most) = (0, 0);
    for _ in 0..count {
        let at = r.offset();
        let n = r.u32()?;
        if n == 0 {
            return Err(r.error_at(at, field("influences", 0, "at least 1".into())));
        }
        let mut influences = Vec::new();
        for _ in 0..n {
            let bone = index(&mut r, bones, "influence bone")?;
            influences.push(Influence {
                bone,
                weight: r.f32()?,
            });
            seen_highest = seen_highest.max(bone);
        }
        seen_most = seen_most.max(influences.len());
        weights.push(influences);
    }
    for (at, name, found, expected) in [
        (highest_at, "highest local bone", highest, seen_highest),
        (most_at, "most influences", most, seen_most),
    ] {
        if to_usize(found) != expected {
            return Err(r.error_at(
                at,
                field(
                    name,
                    found.into(),
                    format!("{expected}, as in the vertices"),
                ),
            ));
        }
    }
    if !r.rest().is_empty() {
        return Err(r.error(field(
            "bytes after the last vertex",
            as_u64(r.rest().len()),
            "0".into(),
        )));
    }
    Ok(weights)
}

/// Texture: `0305 → 0303` `{u32 width, u32 height, u32 format}` and the payload; the object `0F04`.
fn read_textures<'a>(section: &Section<'a>, objects: &Objects<'a>) -> Result<Vec<Texture<'a>>> {
    let list = section.child(None, TAG_TEXTURES, "GRN textures")?;
    let mut textures = Vec::new();
    for node in section.list(Some(list), TAG_TEXTURE, "GRN textures")? {
        let name = object_name(section, node, objects, "GRN texture")?;
        let images = section.child(Some(node), TAG_IMAGES, "GRN texture")?;
        let mut r = section
            .child(Some(images), TAG_IMAGE, "GRN texture images")?
            .data();
        let width = r.u32()?;
        let height = r.u32()?;
        let format_at = r.offset();
        let code = r.u32()?;
        let payload = r.rest();
        let format = match code {
            0 => TextureFormat::Rgbx,
            1 => TextureFormat::Rgba,
            4 => TextureFormat::VTex,
            5 => TextureFormat::VTexAlpha,
            7 => TextureFormat::External,
            _ => {
                return Err(r.error_at(
                    format_at,
                    field("texture format", code.into(), "0, 1, 4, 5 or 7".into()),
                ));
            }
        };
        let size = match format {
            TextureFormat::External => Some(0),
            TextureFormat::Rgbx | TextureFormat::Rgba => {
                Some(u64::from(width) * u64::from(height) * 4)
            }
            TextureFormat::VTex | TextureFormat::VTexAlpha => None,
        };
        match size {
            Some(size) if as_u64(payload.len()) != size => {
                return Err(r.error(field(
                    "texture payload size",
                    as_u64(payload.len()),
                    size.to_string(),
                )));
            }
            None if !payload.starts_with(b"BIKi") => {
                return Err(r.error(ErrorKind::Signature {
                    what: "VTex texture",
                }));
            }
            _ => {}
        }
        textures.push(Texture {
            name,
            width,
            height,
            format,
            payload,
        });
    }
    Ok(textures)
}

/// Material: `0D03` `{0, u32 texture from 1, 1}` or the color `0D02` `f32 rgba[4]`; an object
/// `0F04`, optional for a color material.
fn read_materials<'a>(
    section: &Section<'a>,
    objects: &Objects<'a>,
    textures: usize,
) -> Result<Vec<Material<'a>>> {
    let list = section.child(None, TAG_MATERIALS, "GRN materials")?;
    let mut materials = Vec::new();
    for node in section.list(Some(list), TAG_MATERIAL, "GRN materials")? {
        let surface =
            match section.optional_child(Some(node), TAG_MATERIAL_TEXTURE, "GRN material")? {
                Some(texture) => {
                    // `{0, texture, 1}`: the outer words are the same in every file.
                    let mut r = texture.sized(12, "material texture size")?;
                    r.u32()?;
                    Surface::Texture(one_based(&mut r, textures, "material texture")?)
                }
                None => Surface::Color(
                    section
                        .child(Some(node), TAG_MATERIAL_COLOR, "GRN material")?
                        .sized(16, "material color size")?
                        .f32s()?,
                ),
            };
        let name = match section.optional_child(Some(node), TAG_OBJECT_REF, "GRN material")? {
            Some(reference) => Some(read_name(reference, objects, "GRN material")?),
            None => None,
        };
        materials.push(Material { name, surface });
    }
    Ok(materials)
}

/// Passes of the only model `0E00` (`u32` its form, 1): `{u32 form mesh from 0, u32 material
/// from 1}`, then `0E03` with the color and the texture-coordinate source, and `0E06`: `u32`
/// count, then per triangle a `u32` triangle and, corner by corner and within a corner source by
/// source, a `u32` entry for every source that is a set. A file without meshes has no `0E07`.
fn read_passes(
    section: &Section,
    mesh_nodes: &[&Node],
    meshes: &[Mesh],
    form_meshes: &[usize],
    materials: usize,
) -> Result<Vec<Pass>> {
    let list = section.child(None, TAG_MODELS, "GRN models")?;
    let model = section.child(Some(list), TAG_MODEL, "GRN models")?;
    one_based(&mut model.sized(4, "model size")?, 1, "model form")?;
    let nodes = match section.optional_child(Some(model), TAG_PASSES, "GRN model")? {
        Some(list) => section.list(Some(list), TAG_PASS, "GRN passes")?,
        None => Vec::new(),
    };
    // How many passes draw each triangle of each mesh.
    let mut drawn: Vec<Vec<u8>> = meshes
        .iter()
        .map(|mesh| vec![0; mesh.triangles.len()])
        .collect();
    let mut passes = Vec::with_capacity(nodes.len());
    for node in nodes {
        let mut r = node.sized(8, "pass size")?;
        let at = r.offset();
        let form_mesh = r.u32()?;
        let (mesh_index, mesh) = match form_meshes.get(to_usize(form_mesh)) {
            Some(&index) => (index, &meshes[index]),
            None => {
                return Err(r.error_at(
                    at,
                    field(
                        "pass form mesh",
                        form_mesh.into(),
                        format!("below {}, the form meshes", form_meshes.len()),
                    ),
                ));
            }
        };
        let material = one_based(&mut r, materials, "pass material")?;

        let list = section.child(Some(node), TAG_SOURCES, "GRN pass")?;
        let sources: Vec<_> = section.children_of(Some(list)).collect();
        let [color, texcoords] = sources.as_slice() else {
            return Err(list.data().error(field(
                "pass sources",
                as_u64(sources.len()),
                "2".into(),
            )));
        };
        let color = read_source(color, mesh)?;
        let texcoords = read_source(texcoords, mesh)?;

        let rows = section.child(Some(node), TAG_PASS_TRIANGLES, "GRN pass")?;
        let mut r = rows.data();
        let count = to_usize(r.u32()?);
        let sets: Vec<_> = [color, texcoords]
            .iter()
            .map(|source| match source {
                Source::Set(set) => Some(mesh.texcoord_sets[*set].len()),
                Source::Constant(_) => None,
            })
            .collect();
        let row = 4 + 12 * sets.iter().flatten().count();
        let len = count.checked_mul(row).and_then(|n| n.checked_add(4));
        let mut r = rows.sized(len.unwrap_or(usize::MAX), "pass triangles size")?;
        r.u32()?;
        let mut triangles = Vec::with_capacity(count);
        for _ in 0..count {
            let at = r.offset();
            let triangle = index(&mut r, mesh.triangles.len(), "pass triangle")?;
            let times = &mut drawn[mesh_index][triangle];
            if *times > 0 {
                return Err(r.error_at(
                    at,
                    field(
                        "pass triangle",
                        as_u64(triangle),
                        "a triangle no earlier pass draws".into(),
                    ),
                ));
            }
            *times = 1;
            let mut corners = [None, None];
            for corner in 0..3 {
                for (entries, size) in corners.iter_mut().zip(&sets) {
                    if let Some(size) = *size {
                        let entry = index(&mut r, size, "pass set entry")?;
                        entries.get_or_insert([0; 3])[corner] = entry;
                    }
                }
            }
            let [color, texcoords] = corners;
            triangles.push(PassTriangle {
                triangle,
                color,
                texcoords,
            });
        }
        passes.push(Pass {
            mesh: mesh_index,
            material,
            color,
            texcoords,
            triangles,
        });
    }
    for (node, drawn) in mesh_nodes.iter().zip(&drawn) {
        if let Some(triangle) = drawn.iter().position(|&times| times == 0) {
            let data = section.child(Some(node), TAG_TRIANGLES, "GRN mesh")?.data();
            return Err(data.error_at(
                data.offset() + triangle * TRIANGLE_SIZE,
                field("passes drawing the triangle", 0, "1".into()),
            ));
        }
    }
    Ok(passes)
}

/// Pass source: `0E05`, a constant `f32 rgb[3]`, or `0E04`, a `u32` set of the mesh from 0.
fn read_source(node: &Node, mesh: &Mesh) -> Result<Source> {
    match node.tag {
        TAG_SOURCE_CONSTANT => Ok(Source::Constant(
            node.sized(12, "constant source size")?.f32s()?,
        )),
        TAG_SOURCE_SET => {
            let mut r = node.sized(4, "set source size")?;
            Ok(Source::Set(index(
                &mut r,
                mesh.texcoord_sets.len(),
                "source set",
            )?))
        }
        tag => Err(node.data().error(field(
            "pass source tag",
            tag.into(),
            format!("{TAG_SOURCE_SET:#x} or {TAG_SOURCE_CONSTANT:#x}"),
        ))),
    }
}

/// `__ObjectName` of the object that the only `0F04` child of `node` refers to.
fn object_name<'a>(
    section: &Section<'a>,
    node: &Node<'a>,
    objects: &Objects<'a>,
    what: &'static str,
) -> Result<&'a [u8]> {
    read_name(
        section.child(Some(node), TAG_OBJECT_REF, what)?,
        objects,
        what,
    )
}

/// `__ObjectName` of the object a `0F04` reference names, numbered from 1.
fn read_name<'a>(
    reference: &Node<'a>,
    objects: &Objects<'a>,
    what: &'static str,
) -> Result<&'a [u8]> {
    let mut r = reference.sized(4, "object reference size")?;
    let at = r.offset();
    let object = one_based(&mut r, objects.objects().len(), "object")?;
    objects.objects()[object].name().ok_or_else(|| {
        r.error_at(
            at,
            ErrorKind::NoProperty {
                what,
                key: "__ObjectName",
            },
        )
    })
}

/// `f32[3]` vectors that fill the rest of `r`.
fn vectors(mut r: Reader, what: &'static str) -> Result<Vec<[f32; 3]>> {
    check_multiple(&r, VECTOR_SIZE, what)?;
    let mut out = Vec::with_capacity(r.rest().len() / VECTOR_SIZE);
    while !r.rest().is_empty() {
        out.push(r.f32s()?);
    }
    Ok(out)
}

fn check_multiple(r: &Reader, size: usize, what: &'static str) -> Result<()> {
    let len = r.rest().len();
    if !len.is_multiple_of(size) {
        return Err(r.error(field(what, as_u64(len), format!("a multiple of {size}"))));
    }
    Ok(())
}

/// Reads three [`index`]es, one per corner of a triangle.
fn corners(r: &mut Reader, count: usize, what: &'static str) -> Result<[usize; 3]> {
    Ok([
        index(r, count, what)?,
        index(r, count, what)?,
        index(r, count, what)?,
    ])
}

/// Reads a `u32` index into a list of `count` items, numbered from 0.
fn index(r: &mut Reader, count: usize, what: &'static str) -> Result<usize> {
    let at = r.offset();
    let value = r.u32()?;
    let index = to_usize(value);
    if index < count {
        Ok(index)
    } else {
        Err(r.error_at(at, field(what, value.into(), format!("below {count}"))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grn::Grn;
    use crate::testing::{
        GrnBone, GrnNode, grn_node, grn_tree, model_nodes, object_grn, remove_grn_subtree,
    };

    // Objects of `skeleton_nodes`: one per bone, numbered from 1.
    const ROOT: u32 = 1;
    const BODY: u32 = 2;
    const HEAD: u32 = 3;
    const WHITE: [f32; 3] = [1.0; 3];

    fn words(values: &[u32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn reals(values: &[f32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn leaf(tag: u32, data: &[u8]) -> Vec<GrnNode> {
        grn_tree(tag, data, vec![])
    }

    /// The vertex lists of a mesh, flat.
    struct Vertices<'v> {
        positions: &'v [f32],
        normals: &'v [f32],
        sets: &'v [&'v [f32]],
    }

    fn mesh(
        object: u32,
        vertices: Option<Vertices>,
        triangles: &[u32],
        weights: Vec<u8>,
    ) -> Vec<GrnNode> {
        let mut children = Vec::new();
        if let Some(Vertices {
            positions,
            normals,
            sets,
        }) = vertices
        {
            let sets = sets
                .iter()
                .map(|set| leaf(TAG_TEXCOORD_SET, &[words(&[3]), reals(set)].concat()))
                .collect();
            let set = grn_tree(
                TAG_VERTEX_SET,
                b"",
                vec![
                    leaf(TAG_POSITIONS, &reals(positions)),
                    leaf(TAG_NORMALS, &reals(normals)),
                    grn_tree(TAG_TEXCOORD_SETS, b"", sets),
                ],
            );
            children.push(grn_tree(TAG_VERTEX_SETS, b"", vec![set]));
        }
        children.push(leaf(TAG_WEIGHTS, &weights));
        children.push(leaf(TAG_TRIANGLES, &words(triangles)));
        children.push(leaf(TAG_OBJECT_REF, &words(&[object])));
        grn_tree(TAG_MESH, b"", children)
    }

    fn weights(highest: u32, most: u32, vertices: &[&[(u32, f32)]]) -> Vec<u8> {
        let count = u32::try_from(vertices.len()).unwrap();
        let mut data = words(&[count, highest, most]);
        for influences in vertices {
            data.extend(words(&[u32::try_from(influences.len()).unwrap()]));
            for (bone, weight) in *influences {
                data.extend(bone.to_le_bytes());
                data.extend(weight.to_le_bytes());
            }
        }
        data
    }

    /// Every bone entry with the radius 2 and the box from -1 to 1.
    fn form_mesh(mesh: u32, bones: &[u32]) -> Vec<GrnNode> {
        let bounds = reals(&[2.0, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0]);
        let entries = bones
            .iter()
            .map(|&bone| leaf(TAG_MESH_BONE, &[words(&[bone]), bounds.clone()].concat()))
            .collect();
        grn_tree(
            TAG_FORM_MESH,
            &words(&[mesh]),
            vec![
                leaf(TAG_FORM_MESH_INFO, b""),
                grn_tree(TAG_MESH_BONES, b"", entries),
            ],
        )
    }

    fn constant(rgb: [f32; 3]) -> Vec<GrnNode> {
        leaf(TAG_SOURCE_CONSTANT, &reals(&rgb))
    }

    fn set(set: u32) -> Vec<GrnNode> {
        leaf(TAG_SOURCE_SET, &words(&[set]))
    }

    fn pass(
        form_mesh: u32,
        material: u32,
        sources: [Vec<GrnNode>; 2],
        rows: &[&[u32]],
    ) -> Vec<GrnNode> {
        let mut data = words(&[u32::try_from(rows.len()).unwrap()]);
        for row in rows {
            data.extend(words(row));
        }
        grn_tree(
            TAG_PASS,
            &words(&[form_mesh, material]),
            vec![
                grn_tree(TAG_SOURCES, b"", sources.into()),
                leaf(TAG_PASS_TRIANGLES, &data),
            ],
        )
    }

    /// A skinned body of two triangles, a rigid head and an empty mesh, which the form binds in
    /// another order; a texture; a color and a texture material; three passes, of which the
    /// body's two draw one triangle each.
    fn sample() -> Vec<GrnNode> {
        let bone = |name, parent| GrnBone {
            name,
            parent,
            transform: [0.0; 16],
        };
        let bones = [
            bone(b"__Root".as_slice(), 0),
            bone(b"Body", 0),
            bone(b"Head", 1),
        ];
        let square = [0., 0., 0., 1., 0., 0., 0., 1., 0., 1., 1., 0.];
        let body = mesh(
            BODY,
            Some(Vertices {
                positions: &square,
                normals: &[0., 0., 1., 0., 0., -1.],
                sets: &[&square],
            }),
            &[0, 1, 2, 0, 0, 1, 2, 1, 3, 1, 1, 0],
            weights(
                1,
                2,
                &[
                    &[(0, 1.0)],
                    &[(1, 0.25), (0, 0.75)],
                    &[(1, 1.0)],
                    &[(0, 1.0)],
                ],
            ),
        );
        let head = mesh(
            HEAD,
            Some(Vertices {
                positions: &[0., 0., 2., 1., 0., 2., 0., 1., 2.],
                normals: &[0., 0., 1.],
                sets: &[],
            }),
            &[0, 1, 2, 0, 0, 0],
            weights(0, 0, &[]),
        );
        let empty = mesh(ROOT, None, &[], weights(0, 0, &[]));
        let meshes = grn_tree(TAG_MESHES, b"", vec![body, head, empty]);
        let forms = grn_tree(
            TAG_FORM_MESHES,
            b"",
            vec![
                form_mesh(3, &[0]),
                form_mesh(1, &[1, 2]),
                form_mesh(2, &[2]),
            ],
        );
        let image = grn_tree(TAG_IMAGES, b"", vec![leaf(TAG_IMAGE, &words(&[64, 32, 7]))]);
        let texture = grn_tree(
            TAG_TEXTURE,
            b"",
            vec![image, leaf(TAG_OBJECT_REF, &words(&[HEAD]))],
        );
        let textures = grn_tree(TAG_TEXTURES, b"", vec![texture]);
        let color = leaf(TAG_MATERIAL_COLOR, &reals(&[0.5, 0.5, 0.5, 1.0]));
        let textured = vec![
            leaf(TAG_MATERIAL_TEXTURE, &words(&[0, 1, 1])),
            leaf(TAG_OBJECT_REF, &words(&[BODY])),
        ];
        let materials = grn_tree(
            TAG_MATERIALS,
            b"",
            vec![
                grn_tree(TAG_MATERIAL, b"", vec![color]),
                grn_tree(TAG_MATERIAL, b"", textured),
            ],
        );
        let passes = grn_tree(
            TAG_PASSES,
            b"",
            vec![
                pass(1, 2, [constant(WHITE), set(0)], &[&[1, 2, 1, 3]]),
                pass(2, 1, [constant(WHITE), constant([0.0; 3])], &[&[0]]),
                // Two sets: within each corner, an entry of the first, then of the second.
                pass(1, 2, [set(0), set(0)], &[&[0, 3, 0, 2, 1, 1, 2]]),
            ],
        );
        let model = grn_tree(
            TAG_MODELS,
            b"",
            vec![grn_tree(TAG_MODEL, &words(&[1]), vec![passes])],
        );
        let top = [meshes, textures, materials, model].into_iter().flatten();
        model_nodes(&bones, forms, top.collect())
    }

    fn error(nodes: Vec<GrnNode>) -> crate::Error {
        Grn::parse(&object_grn(nodes)).unwrap().model().unwrap_err()
    }

    fn assert_error(nodes: Vec<GrnNode>, message: &str) {
        let err = error(nodes);
        assert!(err.to_string().ends_with(message), "{err}");
    }

    /// The `nth` node with `tag`, counted from 0, as an index into `nodes`.
    fn position(nodes: &[GrnNode], tag: u32, nth: usize) -> usize {
        let mut found = nodes.iter().enumerate().filter(|(_, n)| n.tag == tag);
        found.nth(nth).unwrap().0
    }

    #[test]
    fn reads_meshes_bindings_textures_materials_and_passes() {
        let data = object_grn(sample());
        let model = Grn::parse(&data).unwrap().model().unwrap();
        let names: Vec<_> = model.meshes.iter().map(|m| m.name).collect();
        assert_eq!(names, [b"Body".as_slice(), b"Head", b"__Root"]);

        let body = &model.meshes[0];
        assert_eq!(body.positions[3], [1.0, 1.0, 0.0]);
        assert_eq!(body.normals, [[0.0, 0.0, 1.0], [0.0, 0.0, -1.0]]);
        assert_eq!(body.texcoord_sets.len(), 1);
        assert_eq!(body.texcoord_sets[0][1], [1.0, 0.0, 0.0]);
        let triangle = |positions, normals| Triangle { positions, normals };
        assert_eq!(
            body.triangles,
            [
                triangle([0, 1, 2], [0, 0, 1]),
                triangle([2, 1, 3], [1, 1, 0])
            ]
        );
        assert_eq!(body.weights.len(), 4);
        let influence = |bone, weight| Influence { bone, weight };
        assert_eq!(body.weights[1], [influence(1, 0.25), influence(0, 0.75)]);
        let entry = |bone| MeshBone {
            bone,
            radius: 2.0,
            min: [-1.0; 3],
            max: [1.0; 3],
        };
        assert_eq!(body.bones, [entry(1), entry(2)]);

        let head = &model.meshes[1];
        assert!(head.weights.is_empty() && head.texcoord_sets.is_empty());
        assert_eq!(head.bones, [entry(2)]);
        let empty = &model.meshes[2];
        assert!(empty.positions.is_empty() && empty.triangles.is_empty());
        assert_eq!(empty.bones, [entry(0)]);

        assert_eq!(
            model.textures,
            [Texture {
                name: b"Head",
                width: 64,
                height: 32,
                format: TextureFormat::External,
                payload: b"",
            }]
        );
        assert_eq!(
            model.materials,
            [
                Material {
                    name: None,
                    surface: Surface::Color([0.5, 0.5, 0.5, 1.0])
                },
                Material {
                    name: Some(b"Body"),
                    surface: Surface::Texture(0)
                }
            ]
        );
        let drawn = |triangle, color, texcoords| PassTriangle {
            triangle,
            color,
            texcoords,
        };
        assert_eq!(
            model.passes,
            [
                Pass {
                    mesh: 0,
                    material: 1,
                    color: Source::Constant(WHITE),
                    texcoords: Source::Set(0),
                    triangles: vec![drawn(1, None, Some([2, 1, 3]))],
                },
                Pass {
                    mesh: 1,
                    material: 0,
                    color: Source::Constant(WHITE),
                    texcoords: Source::Constant([0.0; 3]),
                    triangles: vec![drawn(0, None, None)],
                },
                Pass {
                    mesh: 0,
                    material: 1,
                    color: Source::Set(0),
                    texcoords: Source::Set(0),
                    triangles: vec![drawn(0, Some([3, 2, 1]), Some([0, 1, 2]))],
                }
            ]
        );
    }

    #[test]
    fn vertex_lists_hold_whole_vectors() {
        let mut nodes = sample();
        grn_node(&mut nodes, TAG_POSITIONS, 0).data.pop();
        assert_error(
            nodes,
            "field `positions size` is 47, expected a multiple of 12",
        );

        let mut nodes = sample();
        grn_node(&mut nodes, TAG_TEXCOORD_SET, 0).data[0] = 2;
        assert_error(
            nodes,
            "field `texture-coordinate components` is 2, expected 3",
        );
    }

    #[test]
    fn triangles_index_their_lists() {
        let mut nodes = sample();
        grn_node(&mut nodes, TAG_TRIANGLES, 0).data[0] = 4;
        assert_error(nodes, "field `triangle position` is 4, expected below 4");

        let mut nodes = sample();
        grn_node(&mut nodes, TAG_TRIANGLES, 0).data[12] = 2;
        assert_error(nodes, "field `triangle normal` is 2, expected below 2");
    }

    #[test]
    fn weights_cover_every_position_or_none() {
        let mut nodes = sample();
        grn_node(&mut nodes, TAG_WEIGHTS, 0).data[0] = 3;
        assert_error(
            nodes,
            "field `weighted vertices` is 3, expected 0 or 4, the positions",
        );

        // A rigid body would follow one of its two bones.
        let mut nodes = sample();
        grn_node(&mut nodes, TAG_WEIGHTS, 0).data = words(&[0, 0, 0]);
        assert_error(nodes, "field `bones of a rigid mesh` is 2, expected 1");
    }

    #[test]
    fn weights_name_bones_of_the_form_mesh_and_fill_the_node() {
        // The bone of vertex 2, after the header and vertices 0 (12 B) and 1 (20 B) and its count.
        let mut nodes = sample();
        grn_node(&mut nodes, TAG_WEIGHTS, 0).data[48] = 2;
        assert_error(nodes, "field `influence bone` is 2, expected below 2");

        let mut nodes = sample();
        grn_node(&mut nodes, TAG_WEIGHTS, 0).data[4] = 0;
        assert_error(
            nodes,
            "field `highest local bone` is 0, expected 1, as in the vertices",
        );

        let mut nodes = sample();
        grn_node(&mut nodes, TAG_WEIGHTS, 0).data.extend([0; 4]);
        assert_error(
            nodes,
            "field `bytes after the last vertex` is 4, expected 0",
        );
    }

    #[test]
    fn the_form_binds_every_mesh_once() {
        let mut nodes = sample();
        grn_node(&mut nodes, TAG_FORM_MESH, 0).data = words(&[1]);
        assert_error(
            nodes,
            "GRN form meshes binding the mesh: 2 with tag 0xca5e0c03, expected one",
        );

        let mut nodes = sample();
        let at = position(&nodes, TAG_FORM_MESH, 0);
        remove_grn_subtree(&mut nodes, at);
        assert_error(
            nodes,
            "GRN form meshes binding the mesh: 0 with tag 0xca5e0c03, expected one",
        );
    }

    #[test]
    fn textures_carry_the_payload_of_their_format() {
        let mut nodes = sample();
        grn_node(&mut nodes, TAG_IMAGE, 0).data[8] = 9;
        assert_error(
            nodes,
            "field `texture format` is 9, expected 0, 1, 4, 5 or 7",
        );

        let mut nodes = sample();
        grn_node(&mut nodes, TAG_IMAGE, 0).data.extend(b"BIKi");
        assert_error(nodes, "field `texture payload size` is 4, expected 0");

        let mut nodes = sample();
        let image = grn_node(&mut nodes, TAG_IMAGE, 0);
        image.data = words(&[2, 1, 1]);
        image.data.extend([0; 7]);
        assert_error(nodes, "field `texture payload size` is 7, expected 8");

        let mut nodes = sample();
        let image = grn_node(&mut nodes, TAG_IMAGE, 0);
        image.data[8] = 5;
        image.data.extend(b"BIKi\x01");
        let data = object_grn(nodes);
        let model = Grn::parse(&data).unwrap().model().unwrap();
        assert_eq!(model.textures[0].format, TextureFormat::VTexAlpha);
        assert_eq!(model.textures[0].payload, b"BIKi\x01");

        let mut nodes = sample();
        let image = grn_node(&mut nodes, TAG_IMAGE, 0);
        image.data[8] = 4;
        image.data.extend(b"BINK");
        assert_error(nodes, "VTex texture signature does not match");
    }

    #[test]
    fn a_material_names_a_texture_of_the_file() {
        let mut nodes = sample();
        grn_node(&mut nodes, TAG_MATERIAL_TEXTURE, 0).data[4] = 2;
        assert_error(nodes, "field `material texture` is 2, expected 1..=1");
    }

    #[test]
    fn a_pass_draws_its_mesh_with_two_sources() {
        let mut nodes = sample();
        grn_node(&mut nodes, TAG_PASS, 0).data[0] = 3;
        assert_error(
            nodes,
            "field `pass form mesh` is 3, expected below 3, the form meshes",
        );

        let mut nodes = sample();
        grn_node(&mut nodes, TAG_SOURCE_SET, 0).data[0] = 1;
        assert_error(nodes, "field `source set` is 1, expected below 1");

        let mut nodes = sample();
        grn_node(&mut nodes, TAG_SOURCE_CONSTANT, 0).tag = TAG_IMAGE;
        assert_error(
            nodes,
            "field `pass source tag` is 3395158787, expected 0xca5e0e04 or 0xca5e0e05",
        );

        let mut nodes = sample();
        let at = position(&nodes, TAG_SOURCE_SET, 0);
        remove_grn_subtree(&mut nodes, at);
        assert_error(nodes, "field `pass sources` is 1, expected 2");

        let mut nodes = sample();
        grn_node(&mut nodes, TAG_PASS_TRIANGLES, 0)
            .data
            .extend([0; 4]);
        assert_error(nodes, "field `pass triangles size` is 24, expected 20");

        let mut nodes = sample();
        grn_node(&mut nodes, TAG_PASS_TRIANGLES, 0).data[8] = 4;
        assert_error(nodes, "field `pass set entry` is 4, expected below 4");
    }

    #[test]
    fn the_passes_of_a_mesh_draw_each_triangle_once() {
        let mut nodes = sample();
        grn_node(&mut nodes, TAG_PASS_TRIANGLES, 2).data[4] = 1;
        assert_error(
            nodes,
            "field `pass triangle` is 1, expected a triangle no earlier pass draws",
        );

        // Without the head's pass, its triangle goes undrawn.
        let mut nodes = sample();
        let at = position(&nodes, TAG_PASS, 1);
        remove_grn_subtree(&mut nodes, at);
        assert_error(
            nodes,
            "field `passes drawing the triangle` is 0, expected 1",
        );
    }
}
