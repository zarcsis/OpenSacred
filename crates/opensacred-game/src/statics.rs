//! The static objects of a sector (docs/02 §6.1): each object's sprite, assembled from pieces of
//! atlases, drawn with its top-left corner at the object's screen position.
//!
//! The original draws the ground, then the objects back to front. Here a sprite is a quad in the
//! plane of the screen, as flat as the original's, set at the depth of the centre of its cell,
//! so that the depth buffer orders the sprites and the hero among themselves; the ground lies
//! behind all of them (`terrain`). The pieces of one atlas share one mesh. Miniatures (flag
//! 0x20) are squares cut from their type's atlas; the animated ones (flag 0x40) show their
//! first frame, still.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::{
    AlphaMode, AssetServer, Assets, Commands, Entity, Mesh, Mesh3d, MeshMaterial3d,
    StandardMaterial, Vec2, Vec3, default,
};
use opensacred_data::world::{SIDE, Sector, Sprites, StaticObject, World};

use crate::iso;

/// The quads of the pieces of one atlas.
struct Batch {
    /// The atlas, relative to the asset directory.
    atlas: String,
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

/// The sprite meshes of a sector, built on the CPU.
#[derive(Default)]
pub struct Statics {
    batches: Vec<Batch>,
    pub objects: usize,
    pub drawn: usize,
}

/// `dir/relative` with `..` resolved, relative to `root`, with `/` separators.
fn asset_path(root: &Path, dir: &Path, relative: &str) -> Result<String> {
    let mut path = PathBuf::new();
    for component in dir.join(relative).components() {
        match component {
            Component::ParentDir => {
                path.pop();
            }
            other => path.push(other),
        }
    }
    let relative = path
        .strip_prefix(root)
        .with_context(|| format!("{} lies outside {}", path.display(), root.display()))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

/// Width and height of a PNG, from its header.
fn png_size(path: &Path) -> Result<(f32, f32)> {
    let file = fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .with_context(|| format!("reading {}", path.display()))?;
    let info = reader.info();
    Ok((info.width as f32, info.height as f32))
}

/// The objects and their children, depth first.
fn flatten<'a>(objects: &'a [StaticObject], out: &mut Vec<&'a StaticObject>) {
    for object in objects {
        out.push(object);
        flatten(&object.children, out);
    }
}

/// Reads the sprite table, `world/sprites.bin`.
pub fn read_sprites(assets: &Path) -> Result<Sprites> {
    let table = assets
        .join("world")
        .join(opensacred_data::world::SPRITES_FILE);
    let data = fs::read(&table).with_context(|| format!("reading {}", table.display()))?;
    Sprites::decode(&data).with_context(|| format!("decoding {}", table.display()))
}

/// Flags of objects the original does not draw with the world's art (`0x0062AE90`, openheilig).
const HIDDEN: u32 = 0x290;
/// A miniature: a square of its type's atlas instead of a sprite.
const MINIATURE: u32 = 0x20;
/// An animated miniature: a grid of frames, rotated.
const ANIMATED: u32 = 0x40;
/// The side of a miniature atlas, fixed in the original (openheilig).
const MINIATURE_ATLAS: f32 = 256.0;
/// The bits of an object type's flags (its `items.pak` record `+0x00` as a u32) that put its
/// objects in the original's draw lists other than 3 (LGP `sub_80E0E96`, openheilig): `0x4`
/// under the ordinary sprites, in list 0 for a miniature or an object of mask 1, in list 2 for
/// the rest; `0x800000` in list 4, over them.
const UNDERLAY: u32 = 0x4;
const OVERLAY: u32 = 0x80_0000;
/// How far back along the line of sight the objects of lists 0 and 2 go: under the sprites of
/// list 3 and the hero, above the ground, their order within a list unchanged.
pub const LIST_0_BACK: f32 = 4_000.0;
pub const LIST_2_BACK: f32 = 3_000.0;
/// How far forward the objects of list 4 come: over the sprites of list 3 around them. Whether
/// the original draws them over the characters too is not known.
const LIST_4_FORWARD: f32 = 1_000.0;

/// A quad to draw: an atlas, the corners on screen (top-left, top-right, bottom-right,
/// bottom-left) relative to the object's position, and the rectangle of the atlas in pixels.
struct Quad {
    atlas: u32,
    corners: [Vec2; 4],
    source: [f32; 4],
}

/// The quads of an object: its sprite's pieces, or its miniature's square; none when the object
/// has neither or is not drawn.
fn quads(object: &StaticObject, sprites: &Sprites) -> Vec<Quad> {
    if object.flags & HIDDEN != 0 {
        return Vec::new();
    }
    if object.flags & MINIATURE != 0 {
        let Some(mini) = object.miniature else {
            return Vec::new();
        };
        let [a, b, c, _, _] = mini.params.map(f32::from);
        let (size, source, angle) = if object.flags & ANIMATED != 0 {
            // Columns and rows of frames, a rotation in 1/256 turns, timing and frames: the first
            // frame is drawn, still.
            if a == 0.0 || b == 0.0 {
                return Vec::new();
            }
            let size = Vec2::new(MINIATURE_ATLAS / a, MINIATURE_ATLAS / b);
            (
                size,
                [0.0, 0.0, size.x, size.y],
                c * std::f32::consts::TAU / 256.0,
            )
        } else {
            (Vec2::splat(c), [a, b, a + c, b + c], 0.0)
        };
        // Rotated about the position, screen y down.
        let (sin, cos) = angle.sin_cos();
        let turn = |p: Vec2| Vec2::new(p.x * cos + p.y * sin, p.y * cos - p.x * sin);
        let corners = [
            Vec2::ZERO,
            Vec2::new(size.x, 0.0),
            size,
            Vec2::new(0.0, size.y),
        ]
        .map(turn);
        return vec![Quad {
            atlas: mini.atlas,
            corners,
            source,
        }];
    }
    let Some(Some(sprite)) = usize::try_from(object.sprite)
        .ok()
        .filter(|&s| s != 0)
        .and_then(|s| sprites.sprites.get(s))
    else {
        return Vec::new();
    };
    sprite
        .pieces
        .iter()
        .map(|piece| {
            let [x0, y0, x1, y1] = piece.rect.map(f32::from);
            Quad {
                atlas: piece.atlas,
                corners: [
                    Vec2::new(x0, y0),
                    Vec2::new(x1, y0),
                    Vec2::new(x1, y1),
                    Vec2::new(x0, y1),
                ],
                source: piece.source.map(f32::from),
            }
        })
        .collect()
}

/// Builds the sprite and miniature quads of `sector`.
pub fn build(assets: &Path, world: &World, sprites: &Sprites, sector: &Sector) -> Result<Statics> {
    let dir = assets.join("world");
    let mut objects = Vec::new();
    flatten(&sector.statics, &mut objects);
    let right = iso::right();
    let up = iso::up();
    let mut batches: BTreeMap<u32, Batch> = BTreeMap::new();
    let mut sizes: BTreeMap<u32, (f32, f32)> = BTreeMap::new();
    let mut drawn = 0;
    for object in &objects {
        let quads = quads(object, sprites);
        if quads.is_empty() {
            continue;
        }
        let cell = &sector.cells[usize::from(object.cell)];
        let (row, column) = (
            usize::from(object.cell) / SIDE,
            usize::from(object.cell) % SIDE,
        );
        let cx = (sector.x as usize * SIDE + column) as f32 + 0.5;
        let cy = (sector.y as usize * SIDE + row) as f32 + 0.5;
        let lift = cell.heights.iter().map(|&h| f32::from(h)).sum::<f32>() / 4.0;
        // The centre of the cell in the world and on screen; the depth of the object's draw list.
        let back = if object.type_flags & OVERLAY != 0 {
            -LIST_4_FORWARD
        } else if object.type_flags & UNDERLAY != 0 {
            if object.flags & MINIATURE != 0 || object.mask == 1 {
                LIST_0_BACK
            } else {
                LIST_2_BACK
            }
        } else {
            0.0
        };
        let anchor = iso::ground(cx, cy) + Vec3::Y * iso::height(lift) + iso::forward() * back;
        let anchor_screen = Vec2::new(48.0 * (cx - cy), 24.0 * (cx + cy) - lift);
        let position = Vec2::new(object.x as f32, object.y as f32);
        let at = |corner: Vec2| -> [f32; 3] {
            let screen = position + corner;
            let p = anchor
                + right * ((screen.x - anchor_screen.x) / iso::PIXELS_PER_UNIT)
                + up * ((anchor_screen.y - screen.y) / iso::PIXELS_PER_UNIT);
            p.to_array()
        };
        for quad in quads {
            let size = match sizes.get(&quad.atlas) {
                Some(&size) => size,
                None => {
                    let name = world
                        .atlases
                        .get(usize::try_from(quad.atlas)?)
                        .with_context(|| format!("no atlas {}", quad.atlas))?;
                    let size = png_size(&dir.join(name))?;
                    sizes.insert(quad.atlas, size);
                    size
                }
            };
            let batch = match batches.get_mut(&quad.atlas) {
                Some(batch) => batch,
                None => {
                    let name = &world.atlases[usize::try_from(quad.atlas)?];
                    batches.entry(quad.atlas).or_insert(Batch {
                        atlas: asset_path(assets, &dir, name)?,
                        positions: Vec::new(),
                        uvs: Vec::new(),
                        indices: Vec::new(),
                    })
                }
            };
            // Half a texel in from the source's edges: filtering would blend in the texels
            // around it and draw seams between the pieces of a sprite.
            let [u0, v0, u1, v1] = quad.source;
            let (u0, v0, u1, v1) = (u0 + 0.5, v0 + 0.5, u1 - 0.5, v1 - 0.5);
            let first = u32::try_from(batch.positions.len())?;
            batch.positions.extend(quad.corners.map(at));
            let (w, h) = size;
            batch.uvs.extend([
                [u0 / w, v0 / h],
                [u1 / w, v0 / h],
                [u1 / w, v1 / h],
                [u0 / w, v1 / h],
            ]);
            batch
                .indices
                .extend([first, first + 2, first + 1, first, first + 3, first + 2]);
        }
        drawn += 1;
    }
    Ok(Statics {
        batches: batches.into_values().collect(),
        objects: objects.len(),
        drawn,
    })
}

/// Spawns a mesh per atlas, unlit, its edges through alpha to coverage; returns the entities.
pub fn spawn(
    commands: &mut Commands,
    statics: Statics,
    assets: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Vec<Entity> {
    let mut entities = Vec::new();
    for batch in statics.batches {
        let normals = vec![(-iso::forward()).to_array(); batch.positions.len()];
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, batch.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, batch.uvs)
        .with_inserted_indices(Indices::U32(batch.indices));
        let material = StandardMaterial {
            base_color_texture: Some(assets.load(batch.atlas)),
            unlit: true,
            cull_mode: None,
            alpha_mode: AlphaMode::AlphaToCoverage,
            ..default()
        };
        let entity = commands
            .spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(material)),
            ))
            .id();
        entities.push(entity);
    }
    entities
}
