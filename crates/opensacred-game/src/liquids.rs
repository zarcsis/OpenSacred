//! The surfaces of water, lava and sulphur (docs/re/liquids.md). A cell of ground nature `0x90`
//! (`WATERA`) takes the liquid material of its sector's first slot, one of `0xA0` (`WATERB`) that
//! of its second. A material is a series of frames of `texture.pak` (`B_WATER00` to `49`) with a
//! few parameters, the table that the original fills at `0x4189DF`: the clear waters, the opaque
//! ones (the swamp's `E_WATER`, one kind of `B_WATER`), the lavas and the sulphur.
//!
//! As the original draws them (`0x62DD00`, `0x62E0E2`): a frame spans 4 × 4 cells along the grid,
//! and every material runs through its frames once in `CYCLE_MS`, whatever their count; a cell's
//! surface lies flat at height 0 over its bed, each corner as opaque as the bed is deep there,
//! the bed's height times the material's factor, and blended by that alpha. The clear waters'
//! second layer is not drawn (docs/decisions/2026-10-17-liquids.md).

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::{
    AlphaMode, App, AssetServer, Assets, Color, Commands, Entity, Handle, Image, Mesh, Mesh3d,
    MeshMaterial3d, Plugin, Res, ResMut, Resource, StandardMaterial, Time, Update, default,
};
use opensacred_data::world::{Cell, SIDE, Sector};

use crate::blended::{self, Layer, Sorted};
use crate::iso;

/// A liquid material of the original's table.
struct Material {
    /// The frames' name in `texture.pak`, lower case, before the frame's two digits.
    series: &'static str,
    frames: usize,
    /// The int at `+0xD0` of the original's record: a corner of the surface takes an alpha of
    /// its bed's height times it, held to 0…255 (`0x62DD00`). −12 for most waters, opaque from a
    /// bed 22 pixels deep; −255 for the lavas, the sulphur and the swamp's water, opaque over any
    /// bed below 0.
    depth_alpha: i32,
}

/// The original's 14 liquid materials, in its order (`0x4189DF`); a sector names two of them.
const MATERIALS: [Material; 14] = [
    material("b_water", 50, -12),
    material("b_water", 50, -12),
    material("c_water", 50, -12),
    material("d_water", 50, -12),
    material("a_lava", 50, -255),
    material("b_lava", 50, -255),
    material("c_lava", 20, -255),
    material("a_schwefel", 20, -255),
    material("d_lava", 50, -255),
    material("e_water", 50, -255),
    material("f_water", 50, -24),
    material("g_water", 50, -12),
    material("e_lava", 50, -255),
    material("b_water", 50, -12),
];

const fn material(series: &'static str, frames: usize, depth_alpha: i32) -> Material {
    Material {
        series,
        frames,
        depth_alpha,
    }
}

/// The alpha of a surface's corner over a bed `height` pixels high, of `material`, out of 1.
fn corner_alpha(material: &Material, height: i8) -> f32 {
    let alpha = (i32::from(height) * material.depth_alpha).clamp(0, 255);
    f32::from(u8::try_from(alpha).unwrap_or(u8::MAX)) / 255.0
}

/// Ground natures (the high nibble of a cell's ground byte) of the two liquid slots.
const SLOTS: [u8; 2] = [0x9, 0xA];
/// The original's cycle of the frames: the time in milliseconds, halved, modulo 1,024.
const CYCLE_MS: u128 = 2048;
/// Cells a frame spans along each axis of the grid.
const FRAME_CELLS: f32 = 4.0;
/// How far back along the line of sight the surfaces lie: in front of the ground and of the
/// miniatures of the liquid cells, which the original draws before them, behind the underlays
/// of list 0, which it draws after them (`0x62AE90`, docs/re/drawing.md). The margins hold
/// across the tallest of those miniatures (`statics::UNDER_LIQUIDS_BACK`) and the underlays of
/// list 0 beside the water, up to 124 pixels below their cell's centre, which the surface's
/// slope brings 170 units nearer: a surface at height 0 lies nearer than one at its bed's
/// height, and 150 units further back than before keeps every margin that held over the bed
/// (counted on the sector files 2026-10-19, #103).
pub const BACK: f32 = 4_450.0;
/// Cells per side of a surface's mesh: a sector's liquid is cut into pieces so that each sorts
/// among the blended objects (the shadows) by its own depth, not by the middle of a whole sector.
const CHUNK: usize = 16;

/// The liquid surfaces of a sector, built off the main thread: meshes by material.
pub struct Surfaces(Vec<(u8, Mesh)>);

/// A mesh being built.
#[derive(Default)]
struct Piece {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colours: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

/// Whether `cell` lies under a liquid, of either slot.
pub fn liquid(cell: &Cell) -> bool {
    SLOTS.contains(&(cell.ground >> 4))
}

/// Builds the surfaces of `sector`'s liquid cells.
pub fn build(sector: &Sector) -> Surfaces {
    let (sx, sy) = (sector.x as usize * SIDE, sector.y as usize * SIDE);
    let mut surfaces = Vec::new();
    for (slot, nature) in SLOTS.into_iter().enumerate() {
        let material = sector.liquids[slot];
        let Some(spec) = MATERIALS.get(usize::from(material)) else {
            continue;
        };
        let mut pieces: HashMap<(usize, usize), Piece> = HashMap::new();
        for (index, cell) in sector.cells.iter().enumerate() {
            if cell.ground >> 4 != nature {
                continue;
            }
            let (row, column) = (index / SIDE, index % SIDE);
            let piece = pieces.entry((row / CHUNK, column / CHUNK)).or_default();
            let Piece {
                positions,
                uvs,
                colours,
                indices,
            } = piece;
            // Left, top, right, bottom as grid points (row, column).
            let corners = [
                (row + 1, column),
                (row, column),
                (row, column + 1),
                (row + 1, column + 1),
            ];
            let Ok(first) = u32::try_from(positions.len()) else {
                break;
            };
            for (k, (r, c)) in corners.into_iter().enumerate() {
                let (gx, gy) = ((sx + c) as f32, (sy + r) as f32);
                // Flat at height 0: the original's corners are the cell's diamond on the screen,
                // without the bed's heights (`0x62DD00`).
                let point = iso::ground(gx, gy) + iso::forward() * BACK;
                positions.push(point.to_array());
                // A frame spans 4 × 4 cells; which grid axis is u is not verified.
                uvs.push([gx / FRAME_CELLS, gy / FRAME_CELLS]);
                colours.push([1.0, 1.0, 1.0, corner_alpha(spec, cell.heights[k])]);
            }
            // The original's strip: left, top, bottom, right, split along the top and bottom
            // corners, which the alphas run across.
            indices.extend([first, first + 1, first + 3, first + 1, first + 2, first + 3]);
        }
        let mut keys: Vec<_> = pieces.keys().copied().collect();
        keys.sort_unstable();
        for key in keys {
            let Some(piece) = pieces.remove(&key) else {
                continue;
            };
            let normals = vec![[0.0, 1.0, 0.0]; piece.positions.len()];
            let mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, piece.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, piece.uvs)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, piece.colours)
            .with_inserted_indices(Indices::U32(piece.indices));
            surfaces.push((material, mesh));
        }
    }
    Surfaces(surfaces)
}

/// A material's frames and the one material that shows them on every surface.
struct Animated {
    frames: Vec<Handle<Image>>,
    material: Handle<Sorted>,
}

/// The frames of every liquid material, loaded when a surface first needs them.
#[derive(Resource)]
pub struct Liquids {
    /// The frame files of each series, relative to the asset directory, in order.
    files: HashMap<&'static str, Vec<String>>,
    animated: HashMap<u8, Animated>,
}

impl Liquids {
    /// Finds the frames of every series among the converted textures (`textures/texture`).
    pub fn read(assets: &Path) -> Result<Self> {
        let dir = assets.join("textures").join("texture");
        let mut found: HashMap<&'static str, Vec<(usize, String)>> = HashMap::new();
        let entries = fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))?;
        for entry in entries {
            let name = entry
                .with_context(|| format!("reading {}", dir.display()))?
                .file_name()
                .to_string_lossy()
                .into_owned();
            // `01073-b_water00.png`.
            let Some((_, rest)) = name.split_once('-') else {
                continue;
            };
            let Some(stem) = rest.strip_suffix(".png") else {
                continue;
            };
            let Some(split) = stem.len().checked_sub(2) else {
                continue;
            };
            let (series, frame) = stem.split_at(split);
            let Ok(frame) = frame.parse::<usize>() else {
                continue;
            };
            if let Some(m) = MATERIALS.iter().find(|m| m.series == series) {
                found
                    .entry(m.series)
                    .or_default()
                    .push((frame, format!("textures/texture/{name}")));
            }
        }
        let files = found
            .into_iter()
            .map(|(series, mut frames)| {
                frames.sort();
                (series, frames.into_iter().map(|(_, file)| file).collect())
            })
            .collect();
        Ok(Self {
            files,
            animated: HashMap::new(),
        })
    }

    /// The material of liquid `material`, its frames loaded on first use.
    fn material(
        &mut self,
        material: u8,
        assets: &AssetServer,
        materials: &mut Assets<Sorted>,
    ) -> Option<Handle<Sorted>> {
        if let Some(animated) = self.animated.get(&material) {
            return Some(animated.material.clone());
        }
        let spec = MATERIALS.get(usize::from(material))?;
        let files = self.files.get(spec.series)?;
        let frames: Vec<Handle<Image>> = files
            .iter()
            .take(spec.frames)
            .map(|file| {
                assets
                    .load_builder()
                    .with_settings(|settings: &mut ImageLoaderSettings| {
                        settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                            address_mode_u: ImageAddressMode::Repeat,
                            address_mode_v: ImageAddressMode::Repeat,
                            ..ImageSamplerDescriptor::linear()
                        });
                    })
                    .load(file.clone())
            })
            .collect();
        let handle = materials.add(blended::sorted(
            StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: frames.first().cloned(),
                unlit: true,
                cull_mode: None,
                alpha_mode: AlphaMode::Blend,
                ..default()
            },
            Layer::Liquids,
        ));
        self.animated.insert(
            material,
            Animated {
                frames,
                material: handle.clone(),
            },
        );
        Some(handle)
    }
}

/// Spawns a sector's liquid surfaces; returns the entities.
pub fn spawn(
    commands: &mut Commands,
    surfaces: Surfaces,
    liquids: &mut Liquids,
    assets: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<Sorted>,
) -> Vec<Entity> {
    let mut entities = Vec::new();
    for (material, mesh) in surfaces.0 {
        let Some(handle) = liquids.material(material, assets, materials) else {
            continue;
        };
        entities.push(
            commands
                .spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(handle)))
                .id(),
        );
    }
    entities
}

/// The frame of `count` to show at `ms` milliseconds, as the original picks it: the share of the
/// cycle that has passed, times the frames.
fn frame_at(ms: u128, count: usize) -> Option<usize> {
    let count = u128::try_from(count).ok()?;
    usize::try_from(ms % CYCLE_MS * count / CYCLE_MS).ok()
}

/// Shows each material's frame of the moment, once it has loaded: a material whose texture is
/// not loaded is not drawn at all.
pub fn animate(
    time: Res<Time>,
    liquids: Res<Liquids>,
    images: Res<Assets<Image>>,
    mut materials: ResMut<Assets<Sorted>>,
) {
    let ms = time.elapsed().as_millis();
    for animated in liquids.animated.values() {
        let Some(frame) = frame_at(ms, animated.frames.len())
            .and_then(|i| animated.frames.get(i))
            .filter(|frame| images.contains(*frame))
        else {
            continue;
        };
        let Some(material) = materials.get(&animated.material) else {
            continue;
        };
        if material.base.base_color_texture.as_ref() == Some(frame) {
            continue;
        }
        if let Some(mut material) = materials.get_mut(&animated.material) {
            material.base.base_color_texture = Some(frame.clone());
        }
    }
}

/// The liquids' frames.
pub struct LiquidsPlugin;

impl Plugin for LiquidsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, animate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_liquid_is_as_opaque_as_its_bed_is_deep() {
        let (water, f_water, lava) = (&MATERIALS[0], &MATERIALS[10], &MATERIALS[4]);
        let alpha = |m, h| (corner_alpha(m, h) * 255.0).round();
        // `B_WATER`: clear over a bed at 0 or above, 60, 120, 180 at −5, −10, −15, opaque from −22.
        assert_eq!(
            [0, 5, -5, -10, -15, -22, -30].map(|h| alpha(water, h)),
            [0.0, 0.0, 60.0, 120.0, 180.0, 255.0, 255.0]
        );
        // `F_WATER` twice as fast, a lava opaque over any bed below 0.
        assert_eq!(alpha(f_water, -5), 120.0);
        assert_eq!([0, -1, -30].map(|h| alpha(lava, h)), [0.0, 255.0, 255.0]);
    }

    #[test]
    fn a_surface_lies_flat_over_its_bed() {
        let cell = |ground, heights| Cell {
            tile: 0,
            overlays: Vec::new(),
            heights,
            light: [255; 4],
            heights2: [0; 4],
            offsets: [0; 2],
            flags: 0,
            ground,
        };
        let mut cells = vec![cell(0x40, [0; 4]); SIDE * SIDE];
        // A `B_WATER` cell over a bed sloping from 0 at the left to −20 at the right.
        cells[2 * SIDE + 3] = cell(0x90, [0, -10, -20, -10]);
        let sector = Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0, 4],
            cells,
            statics: Vec::new(),
            layers: Vec::new(),
        };
        let Surfaces(surfaces) = build(&sector);
        let [(0, mesh)] = &surfaces[..] else {
            panic!("one surface of material 0");
        };
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        // Every corner at height 0, the surface's depth behind it.
        let lifted = iso::forward() * BACK;
        for p in positions {
            assert!((p[1] - lifted.y).abs() < 1e-3, "{p:?}");
        }
        let Some(bevy::mesh::VertexAttributeValues::Float32x4(colours)) =
            mesh.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("colours");
        };
        let alphas: Vec<f32> = colours.iter().map(|c| (c[3] * 255.0).round()).collect();
        assert_eq!(alphas, [0.0, 120.0, 240.0, 120.0]);
    }

    #[test]
    fn every_material_runs_through_its_frames_in_two_seconds() {
        assert_eq!(frame_at(0, 50), Some(0));
        assert_eq!(frame_at(2047, 50), Some(49));
        assert_eq!(frame_at(2048, 50), Some(0));
        // 20 frames take as long as 50.
        assert_eq!(frame_at(1024, 20), Some(10));
        assert_eq!(frame_at(1024, 50), Some(25));
    }
}
