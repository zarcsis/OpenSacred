//! The ground of a sector (docs/02 §5): every cell's diamond, the ground tile with its floor
//! overlays, composed once into a slot of the sector's atlas (one slot per distinct stack of
//! layers), and draped over a mesh of the cells with their corner heights and light, as the
//! original draws a cell as a quad of its diamond. The mesh lies `iso::GROUND_BEHIND` back along the line of sight, so that it
//! never hides an object.
//!
//! The atlas of the median sector is 13 MB, a fifth of the sector's image on screen; a shader
//! that composes the layers would shrink it further.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::{
    Assets, Commands, Entity, Image, Mesh, Mesh3d, MeshMaterial3d, Resource, StandardMaterial,
    Vec3, default,
};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use opensacred_data::world::{Cell, DIAMONDS, SIDE, Sector, World};

use crate::iso;

/// A diamond's box in a tile sheet and on screen.
const DIAMOND_W: usize = 96;
const DIAMOND_H: usize = 48;
/// A slot of the ground atlas: a diamond's box and a border of one pixel, which repeats the
/// box's edge so that filtering does not blend in the neighbouring slot.
const SLOT_W: usize = DIAMOND_W + 2;
const SLOT_H: usize = DIAMOND_H + 2;
/// Slots per row of the atlas.
const SLOTS_PER_ROW: usize = 64;

/// The corner heights of the cells of the loaded sectors, in world units, and their natures (the
/// high nibble of the ground byte, docs/02 §5.2), by global cell.
#[derive(Resource, Default)]
pub struct Heights {
    cells: HashMap<(u32, u32), LoadedCell>,
}

struct LoadedCell {
    heights: [f32; 4],
    nature: u8,
}

impl Heights {
    /// Adds the cells of `sector`.
    pub fn add(&mut self, sector: &Sector) {
        for (index, cell) in sector.cells.iter().enumerate() {
            let (Ok(row), Ok(column), Ok(side)) = (
                u32::try_from(index / SIDE),
                u32::try_from(index % SIDE),
                u32::try_from(SIDE),
            ) else {
                continue;
            };
            let key = (sector.x * side + column, sector.y * side + row);
            self.cells.insert(
                key,
                LoadedCell {
                    heights: cell.heights.map(|h| iso::height(f32::from(h))),
                    nature: cell.ground >> 4,
                },
            );
        }
    }

    /// The cell of world `(x, z)`; `None` outside the loaded sectors.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the cell of a point in the world is a small non-negative whole number, checked"
    )]
    fn cell(&self, x: f32, z: f32) -> Option<(&LoadedCell, f32, f32)> {
        let (fx, fz) = (x / iso::CELL, z / iso::CELL);
        let (cx, cz) = (fx.floor(), fz.floor());
        if !(0.0..1e6).contains(&cx) || !(0.0..1e6).contains(&cz) {
            return None;
        }
        let cell = self.cells.get(&(cx as u32, cz as u32))?;
        Some((cell, fx - cx, fz - cz))
    }

    /// The nature of the cell at world `(x, z)`.
    pub fn nature(&self, x: f32, z: f32) -> Option<u8> {
        self.cell(x, z).map(|(cell, _, _)| cell.nature)
    }

    /// The height of the ground at world `(x, z)`, interpolated between the corners of its cell
    /// (left, top, right, bottom); `None` outside the loaded sectors.
    pub fn at(&self, x: f32, z: f32) -> Option<f32> {
        let (cell, u, v) = self.cell(x, z)?;
        let [left, top, right, bottom] = cell.heights;
        Some(
            top * (1.0 - u) * (1.0 - v)
                + right * u * (1.0 - v)
                + left * (1.0 - u) * v
                + bottom * u * v,
        )
    }
}

/// The top-left corner of diamond `k` (0..18) in its 256 × 256 sheet: nine rows 25 texels apart,
/// two columns 104 apart, odd rows shifted by 52, a margin of 2 (docs/02 §5.6).
fn diamond_origin(k: u32) -> (usize, usize) {
    let (row, column) = (k as usize / 2, k as usize % 2);
    let shift = if row % 2 == 1 { 52 } else { 0 };
    (104 * column + shift + 2, 25 * row)
}

/// Whether pixel `(x, y)` of a diamond's box lies inside the diamond.
fn inside(x: usize, y: usize) -> bool {
    let dx = (x as f32 + 0.5 - 48.0).abs() / 48.0;
    let dy = (y as f32 + 0.5 - 24.0).abs() / 24.0;
    dx + dy <= 1.0
}

/// An RGBA image decoded from a PNG.
struct Rgba {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

fn read_png(path: &Path) -> Result<Rgba> {
    let file = fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder
        .read_info()
        .with_context(|| format!("reading {}", path.display()))?;
    let size = reader
        .output_buffer_size()
        .with_context(|| format!("{}: image too large", path.display()))?;
    let mut pixels = vec![0; size];
    let info = reader
        .next_frame(&mut pixels)
        .with_context(|| format!("decoding {}", path.display()))?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        bail!("{}: not 8-bit RGBA after expansion", path.display());
    }
    pixels.truncate(info.buffer_size());
    Ok(Rgba {
        width: usize::try_from(info.width)?,
        height: usize::try_from(info.height)?,
        pixels,
    })
}

/// The tile sheets, decoded on first use.
struct Sheets<'a> {
    dir: &'a Path,
    world: &'a World,
    loaded: HashMap<u32, Rgba>,
}

impl Sheets<'_> {
    fn get(&mut self, sheet: u32) -> Result<&Rgba> {
        if !self.loaded.contains_key(&sheet) {
            let name = self
                .world
                .sheets
                .get(usize::try_from(sheet)?)
                .with_context(|| format!("no tile sheet {sheet}"))?;
            let image = read_png(&self.dir.join(name))?;
            self.loaded.insert(sheet, image);
        }
        self.loaded
            .get(&sheet)
            .with_context(|| format!("tile sheet {sheet}"))
    }

    /// The colour and alpha of pixel `(x, y)` of tile `tile`'s diamond box.
    fn pixel(&mut self, tile: u32, x: usize, y: usize) -> Result<[u8; 4]> {
        let sheet = self.get(tile / DIAMONDS)?;
        let (ox, oy) = diamond_origin(tile % DIAMONDS);
        let (sx, sy) = (ox + x, oy + y);
        if sx >= sheet.width || sy >= sheet.height {
            return Ok([0; 4]);
        }
        let at = 4 * (sy * sheet.width + sx);
        let mut rgba = [0; 4];
        rgba.copy_from_slice(&sheet.pixels[at..at + 4]);
        Ok(rgba)
    }
}

/// The layers of a cell: its ground tile, then each overlay's art and mask.
type Layers = Vec<(u32, Option<u32>)>;

fn layers(cell: &Cell) -> Layers {
    std::iter::once((cell.tile, None))
        .chain(cell.overlays.iter().map(|o| (o.art, o.mask)))
        .collect()
}

/// The ground atlas of a sector: one slot per distinct stack of layers (748 for the median
/// sector, at most 3,315), and the slot of every cell.
struct Atlas {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
    slots: Vec<usize>,
}

/// Composes a slot: the ground tile, then each overlay's colour through its mask's alpha (or its
/// own), each clipped to the diamond; then the pixels around the diamond repeat its edge along
/// each row, and the border repeats the box.
fn compose_slot(layers: &Layers, sheets: &mut Sheets<'_>, slot: &mut [[u8; 4]]) -> Result<()> {
    let at = |x: usize, y: usize| (y + 1) * SLOT_W + x + 1;
    for &(art, mask) in layers {
        for y in 0..DIAMOND_H {
            for x in 0..DIAMOND_W {
                if !inside(x, y) {
                    continue;
                }
                let colour = sheets.pixel(art, x, y)?;
                let alpha = match mask {
                    Some(mask) => sheets.pixel(mask, x, y)?[3],
                    None => colour[3],
                };
                let out = &mut slot[at(x, y)];
                let a = u16::from(alpha);
                for k in 0..3 {
                    let blended = (u16::from(colour[k]) * a + u16::from(out[k]) * (255 - a)) / 255;
                    out[k] = u8::try_from(blended).unwrap_or(u8::MAX);
                }
                out[3] = 255;
            }
        }
    }
    for y in 0..DIAMOND_H {
        let first = (0..DIAMOND_W)
            .find(|&x| inside(x, y))
            .unwrap_or(DIAMOND_W / 2);
        let last = (0..DIAMOND_W)
            .rev()
            .find(|&x| inside(x, y))
            .unwrap_or(DIAMOND_W / 2);
        for x in 0..first {
            slot[at(x, y)] = slot[at(first, y)];
        }
        for x in last + 1..DIAMOND_W {
            slot[at(x, y)] = slot[at(last, y)];
        }
    }
    for y in 1..=DIAMOND_H {
        slot[y * SLOT_W] = slot[y * SLOT_W + 1];
        slot[y * SLOT_W + SLOT_W - 1] = slot[y * SLOT_W + DIAMOND_W];
    }
    slot.copy_within(SLOT_W..2 * SLOT_W, 0);
    slot.copy_within(
        DIAMOND_H * SLOT_W..(DIAMOND_H + 1) * SLOT_W,
        (DIAMOND_H + 1) * SLOT_W,
    );
    Ok(())
}

/// Composes the atlas of a sector's cells.
fn compose(sector: &Sector, sheets: &mut Sheets<'_>) -> Result<Atlas> {
    let mut stacks: HashMap<Layers, usize> = HashMap::new();
    let mut order: Vec<Layers> = Vec::new();
    let slots = sector
        .cells
        .iter()
        .map(|cell| {
            let key = layers(cell);
            *stacks.entry(key.clone()).or_insert_with(|| {
                order.push(key);
                order.len() - 1
            })
        })
        .collect();
    let columns = order.len().clamp(1, SLOTS_PER_ROW);
    let rows = order.len().div_ceil(SLOTS_PER_ROW).max(1);
    let (width, height) = (columns * SLOT_W, rows * SLOT_H);
    let mut pixels = vec![0u8; 4 * width * height];
    let mut slot = vec![[0u8; 4]; SLOT_W * SLOT_H];
    for (index, layers) in order.iter().enumerate() {
        slot.fill([0; 4]);
        compose_slot(layers, sheets, &mut slot)?;
        let (sx, sy) = (
            (index % SLOTS_PER_ROW) * SLOT_W,
            (index / SLOTS_PER_ROW) * SLOT_H,
        );
        for y in 0..SLOT_H {
            let row = 4 * ((sy + y) * width + sx);
            for x in 0..SLOT_W {
                pixels[row + 4 * x..row + 4 * x + 4].copy_from_slice(&slot[y * SLOT_W + x]);
            }
        }
    }
    Ok(Atlas {
        width,
        height,
        pixels,
        slots,
    })
}

/// The mesh of the cells: a quad per cell over its four corners (left, top, right, bottom), each
/// at its own height, with the corner's light as the vertex colour and its flat screen position
/// in the image as the texture coordinate.
fn mesh(sector: &Sector, atlas: &Atlas) -> Mesh {
    let (sx, sy) = (sector.x as f32 * SIDE as f32, sector.y as f32 * SIDE as f32);
    let mut positions = Vec::with_capacity(4 * sector.cells.len());
    let mut uvs = Vec::with_capacity(4 * sector.cells.len());
    let mut colours = Vec::with_capacity(4 * sector.cells.len());
    let mut indices = Vec::with_capacity(6 * sector.cells.len());
    for (index, cell) in sector.cells.iter().enumerate() {
        let (row, column) = ((index / SIDE) as f32, (index % SIDE) as f32);
        // Corners in the order of the cell's heights: left, top, right, bottom, as grid points
        // (column, row) and as offsets of the top corner on screen.
        let corners = [
            ((column, row + 1.0), (-48.0, 24.0)),
            ((column, row), (0.0, 0.0)),
            ((column + 1.0, row), (48.0, 24.0)),
            ((column + 1.0, row + 1.0), (0.0, 48.0)),
        ];
        // The top corner of the cell's diamond in its atlas slot.
        let slot = atlas.slots[index];
        let top_x = ((slot % SLOTS_PER_ROW) * SLOT_W + 1 + DIAMOND_W / 2) as f32;
        let top_y = ((slot / SLOTS_PER_ROW) * SLOT_H + 1) as f32;
        let first = u32::try_from(positions.len()).unwrap_or(u32::MAX);
        for (k, ((cx, cy), (dx, dy))) in corners.into_iter().enumerate() {
            let point = iso::ground(sx + cx, sy + cy)
                + Vec3::Y * iso::height(f32::from(cell.heights[k]))
                + iso::forward() * iso::GROUND_BEHIND;
            positions.push(point.to_array());
            uvs.push([
                (top_x + dx) / atlas.width as f32,
                (top_y + dy) / atlas.height as f32,
            ]);
            let light = f32::from(cell.light[k]) / 255.0;
            colours.push([light, light, light, 1.0]);
        }
        indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
    .with_inserted_indices(Indices::U32(indices))
}

/// Reads the world index, `world/world.ron`.
pub fn read_world(assets: &Path) -> Result<World> {
    let index = assets.join("world").join(opensacred_data::world::INDEX);
    let text =
        fs::read_to_string(&index).with_context(|| format!("reading {}", index.display()))?;
    ron::from_str(&text).with_context(|| format!("parsing {}", index.display()))
}

/// Reads sector `(x, y)`; `None` where the grid has no sector.
pub fn read_sector(assets: &Path, world: &World, (x, y): (u32, u32)) -> Result<Option<Sector>> {
    let Some(entry) = world.sectors.iter().find(|s| (s.x, s.y) == (x, y)) else {
        return Ok(None);
    };
    let path = assets.join("world").join(&entry.file);
    let data = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let sector = Sector::decode(&data).with_context(|| format!("decoding {}", path.display()))?;
    Ok(Some(sector))
}

/// The ground of a sector, built off the main thread.
pub struct Ground {
    image: Image,
    mesh: Mesh,
}

/// The ground of a sector: the image of its diamonds and the mesh of its cells.
pub fn build(assets: &Path, world: &World, sector: &Sector) -> Result<Ground> {
    let dir: PathBuf = assets.join("world");
    let atlas = {
        let mut sheets = Sheets {
            dir: &dir,
            world,
            loaded: HashMap::new(),
        };
        compose(sector, &mut sheets)?
    };
    let mesh = mesh(sector, &atlas);
    let image = Image::new(
        Extent3d {
            width: u32::try_from(atlas.width)?,
            height: u32::try_from(atlas.height)?,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        atlas.pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    Ok(Ground { image, mesh })
}

/// The centre of a sector on its ground.
pub fn center(sector: &Sector) -> Vec3 {
    let half = SIDE as f32 / 2.0;
    let cell = &sector.cells[SIDE * SIDE / 2 + SIDE / 2];
    let mean = cell.heights.iter().map(|&h| f32::from(h)).sum::<f32>() / 4.0;
    iso::ground(
        sector.x as f32 * SIDE as f32 + half,
        sector.y as f32 * SIDE as f32 + half,
    ) + Vec3::Y * iso::height(mean)
}

/// Spawns a ground as an unlit mesh, the tiles carrying their own shading and the corners their
/// light, as in the original; returns the entities.
pub fn spawn(
    commands: &mut Commands,
    ground: Ground,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Vec<Entity> {
    let material = StandardMaterial {
        base_color_texture: Some(images.add(ground.image)),
        unlit: true,
        cull_mode: None,
        ..default()
    };
    vec![
        commands
            .spawn((
                Mesh3d(meshes.add(ground.mesh)),
                MeshMaterial3d(materials.add(material)),
            ))
            .id(),
    ]
}

#[cfg(test)]
mod tests {
    use opensacred_data::world::Cell;

    use super::*;

    #[test]
    fn heights_interpolate_between_the_corners_of_a_cell() {
        let cell = |heights| Cell {
            tile: 0,
            overlays: Vec::new(),
            heights,
            light: [255; 4],
            heights2: [0; 4],
            offsets: [0; 2],
            flags: 0,
            ground: 0,
        };
        let mut cells = vec![cell([0; 4]); SIDE * SIDE];
        // Left, top, right, bottom of cell (row 0, column 1).
        cells[1] = cell([20, 0, 10, 30]);
        let sector = Sector {
            x: 2,
            y: 3,
            id: 0,
            liquids: [0; 2],
            cells,
            statics: Vec::new(),
        };
        let mut heights = Heights::default();
        heights.add(&sector);
        let corner = |cx: f32, cy: f32| {
            let p = iso::ground(2.0 * SIDE as f32 + cx, 3.0 * SIDE as f32 + cy);
            heights.at(p.x, p.z).unwrap()
        };
        assert!((corner(1.0, 0.0) - 0.0).abs() < 1e-4);
        assert!((corner(1.999, 0.0) - iso::height(10.0)).abs() < 0.05);
        assert!((corner(1.5, 0.5) - iso::height(15.0)).abs() < 1e-3);
        assert!(heights.at(-1.0, 0.0).is_none());
    }
}
