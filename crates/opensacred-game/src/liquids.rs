//! The surfaces of water, lava and sulphur (docs/re/liquids.md). A cell of ground nature `0x90`
//! (`WATERA`) takes the liquid material of its sector's first slot, one of `0xA0` (`WATERB`) that
//! of its second. A material is a series of frames of `texture.pak` (`B_WATER00` to `49`) with a
//! few parameters, the table that the original fills at `0x4189DF`: the clear waters, the opaque
//! ones (the swamp's `E_WATER`, one kind of `B_WATER`), the lavas and the sulphur.
//!
//! As the original draws them (`0x62DD00`, `0x62E0E2`): a frame spans 4 × 4 cells along the grid,
//! and every material runs through its frames once in `CYCLE_MS`, whatever their count; a cell's
//! surface lies flat at height 0 over its bed, each corner as opaque as the bed is deep there,
//! the bed's height times the material's factor, and blended by that alpha. Before the
//! surfaces, the clear waters have a second layer on their beds: the frames again, a frame over
//! 2 × 2 cells, added to the frame, brighter over the deeper bed and gone by night (#118,
//! docs/decisions/2026-10-17-liquids.md).

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use bevy::asset::RenderAssetUsages;
use bevy::color::ColorToComponents;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::{
    AlphaMode, App, AssetServer, Assets, Color, Commands, Entity, Handle, Image,
    IntoScheduleConfigs, Mesh, Mesh3d, MeshMaterial3d, Plugin, Res, ResMut, Resource,
    StandardMaterial, Time, Update, default,
};
use opensacred_data::world::{Cell, SIDE, Sector};

use crate::blended::{self, Layer, Sorted};
use crate::daynight::{self, Clock};
use crate::files;
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
    /// The byte at `+0xCC`: the clear waters, which have the second layer.
    clear: bool,
}

/// The original's 14 liquid materials, in its order (`0x4189DF`); a sector names two of them.
const MATERIALS: [Material; 14] = [
    material("b_water", 50, -12, true),
    material("b_water", 50, -12, true),
    material("c_water", 50, -12, true),
    material("d_water", 50, -12, true),
    material("a_lava", 50, -255, false),
    material("b_lava", 50, -255, false),
    material("c_lava", 20, -255, false),
    material("a_schwefel", 20, -255, false),
    material("d_lava", 50, -255, false),
    material("e_water", 50, -255, false),
    material("f_water", 50, -24, true),
    material("g_water", 50, -12, true),
    material("e_lava", 50, -255, false),
    material("b_water", 50, -12, false),
];

const fn material(series: &'static str, frames: usize, depth_alpha: i32, clear: bool) -> Material {
    Material {
        series,
        frames,
        depth_alpha,
        clear,
    }
}

/// The alpha of a surface's corner over a bed `height` pixels high, of `material`, out of 1.
fn corner_alpha(material: &Material, height: i8) -> f32 {
    let alpha = (i32::from(height) * material.depth_alpha).clamp(0, 255);
    f32::from(u8::try_from(alpha).unwrap_or(u8::MAX)) / 255.0
}

/// The second layer's grey at a corner over a bed `height` pixels high, out of 255: eight times
/// the bed's depth, held to 0…255 (`0x62DD00`), 80 over a bed at −10 and white from −32 down.
fn bed_grey(height: i8) -> u8 {
    u8::try_from((-8 * i32::from(height)).clamp(0, 255)).unwrap_or(u8::MAX)
}

/// What the night leaves of the second layer at a minute of the day, out of 1: its corners'
/// grey times `255 − N`, over 256 (`>> 8`), so that it goes over the dusk.
fn bed_light(minutes: f64) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a share of 0 to 255, out of 256"
    )]
    let light = ((255.0 - daynight::level(minutes)) / 256.0) as f32;
    light
}

/// Ground natures (the high nibble of a cell's ground byte) of the two liquid slots.
const SLOTS: [u8; 2] = [0x9, 0xA];
/// The original's cycle of the frames: the time in milliseconds, halved, modulo 1,024.
const CYCLE_MS: u128 = 2048;
/// Cells a frame spans along each axis of the grid.
const FRAME_CELLS: f32 = 4.0;
/// Cells a frame of the second layer spans: its template steps by 0.5 (`0x624AB6`).
const BED_CELLS: f32 = 2.0;
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

/// The liquids of a sector, built off the main thread: the surfaces' meshes and the clear
/// waters' second layer's, by material.
pub struct Surfaces {
    surfaces: Vec<(u8, Mesh)>,
    beds: Vec<(u8, Mesh)>,
}

/// A mesh being built.
#[derive(Default)]
struct Piece {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colours: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

/// Whether the liquid material `material` is a clear water (the byte `+0xCC` of its record),
/// whose cells the water's sound counts (`0x62DD00`).
pub fn clear(material: u8) -> bool {
    MATERIALS
        .get(usize::from(material))
        .is_some_and(|material| material.clear)
}

/// Whether `cell` lies under a liquid, of either slot.
pub fn liquid(cell: &Cell) -> bool {
    SLOTS.contains(&(cell.ground >> 4))
}

/// Builds the surfaces of `sector`'s liquid cells, and the second layer of its clear waters.
pub fn build(sector: &Sector) -> Surfaces {
    let (sx, sy) = (sector.x as usize * SIDE, sector.y as usize * SIDE);
    let mut surfaces = Vec::new();
    let mut beds = Vec::new();
    for (slot, nature) in SLOTS.into_iter().enumerate() {
        let material = sector.liquids[slot];
        let Some(spec) = MATERIALS.get(usize::from(material)) else {
            continue;
        };
        let mut pieces: HashMap<(usize, usize), Piece> = HashMap::new();
        let mut bed_pieces: HashMap<(usize, usize), Piece> = HashMap::new();
        for (index, cell) in sector.cells.iter().enumerate() {
            if cell.ground >> 4 != nature {
                continue;
            }
            let (row, column) = (index / SIDE, index % SIDE);
            if spec.clear {
                bed(
                    bed_pieces.entry((row / CHUNK, column / CHUNK)).or_default(),
                    cell,
                    (sx + column, sy + row),
                );
            }
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
        surfaces.extend(meshes(pieces).into_iter().map(|mesh| (material, mesh)));
        beds.extend(meshes(bed_pieces).into_iter().map(|mesh| (material, mesh)));
    }
    Surfaces { surfaces, beds }
}

/// Adds the second layer of the clear water's cell at global `(x, y)` to `piece`: the cell's
/// ground quad on its bed, with its heights, as the original draws it (`0x62DD00`), each corner
/// as grey as the bed is deep there, the frame mapped over 2 × 2 cells. Each corner lies on the
/// screen where the bed's corner does, at the depth of the surface there, so that it lies over
/// and under what the surface does.
fn bed(piece: &mut Piece, cell: &Cell, (x, y): (usize, usize)) {
    let Ok(first) = u32::try_from(piece.positions.len()) else {
        return;
    };
    // Left, top, right, bottom as grid points (x, y), the order of the cell's heights.
    let corners = [(x, y + 1), (x, y), (x + 1, y), (x + 1, y + 1)];
    for (k, (gx, gy)) in corners.into_iter().enumerate() {
        let (gx, gy) = (gx as f32, gy as f32);
        let height = cell.heights[k];
        // A pixel of height lifts a point as far on the screen as a 48th of a cell back along
        // both axes of the grid does on the ground.
        let back = f32::from(height) / 48.0;
        let point = iso::ground(gx - back, gy - back) + iso::forward() * BACK;
        piece.positions.push(point.to_array());
        piece.uvs.push([gx / BED_CELLS, gy / BED_CELLS]);
        // The original's grey multiplies the frame's 8-bit values; the material multiplies in
        // linear light, and the frame holds sRGB values (`srgb`).
        let grey = bed_grey(height);
        piece
            .colours
            .push(Color::srgb_u8(grey, grey, grey).to_linear().to_f32_array());
    }
    // The ground's quad, split along its left and right corners (`terrain`).
    piece
        .indices
        .extend([first, first + 1, first + 2, first, first + 2, first + 3]);
}

/// The meshes of `pieces`, in the order of their keys.
fn meshes(mut pieces: HashMap<(usize, usize), Piece>) -> Vec<Mesh> {
    let mut keys: Vec<_> = pieces.keys().copied().collect();
    keys.sort_unstable();
    let mut meshes = Vec::new();
    for key in keys {
        let Some(piece) = pieces.remove(&key) else {
            continue;
        };
        let normals = vec![[0.0, 1.0, 0.0]; piece.positions.len()];
        meshes.push(
            Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, piece.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, piece.uvs)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, piece.colours)
            .with_inserted_indices(Indices::U32(piece.indices)),
        );
    }
    meshes
}

/// A material's frames, the one material that shows them on every surface, and the one of the
/// second layer, for a clear water.
struct Animated {
    frames: Vec<Handle<Image>>,
    material: Handle<Sorted>,
    bed: Option<Handle<Sorted>>,
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
        let entries =
            files::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))?;
        for path in entries {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            // `01073-b_water00.ktx2`.
            let Some((_, rest)) = name.split_once('-') else {
                continue;
            };
            let Some(stem) = rest
                .strip_suffix(opensacred_data::texture::EXTENSION)
                .and_then(|rest| rest.strip_suffix('.'))
            else {
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

    /// The materials of liquid `material`, the surfaces' and the second layer's, its frames
    /// loaded on first use.
    fn material(
        &mut self,
        material: u8,
        assets: &AssetServer,
        materials: &mut Assets<Sorted>,
    ) -> Option<&Animated> {
        if self.animated.contains_key(&material) {
            return self.animated.get(&material);
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
        // Added to the frame (`ONE`, `ONE`), filtered linearly.
        let bed = spec.clear.then(|| {
            materials.add(blended::sorted(
                StandardMaterial {
                    base_color: Color::WHITE,
                    base_color_texture: frames.first().cloned(),
                    unlit: true,
                    cull_mode: None,
                    alpha_mode: AlphaMode::Add,
                    ..default()
                },
                Layer::LiquidBeds,
            ))
        });
        self.animated.insert(
            material,
            Animated {
                frames,
                material: handle,
                bed,
            },
        );
        self.animated.get(&material)
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
    let all = surfaces
        .surfaces
        .into_iter()
        .map(|(material, mesh)| (material, mesh, false))
        .chain(
            surfaces
                .beds
                .into_iter()
                .map(|(material, mesh)| (material, mesh, true)),
        );
    for (material, mesh, bed) in all {
        let Some(animated) = liquids.material(material, assets, materials) else {
            continue;
        };
        let handle = if bed {
            animated.bed.clone()
        } else {
            Some(animated.material.clone())
        };
        let Some(handle) = handle else {
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

/// Shows each material's frame of the moment, once it has loaded, on its surfaces and its
/// second layer: a material whose texture is not loaded is not drawn at all. The second layer
/// takes the night's share by the clock.
pub fn animate(
    time: Res<Time>,
    clock: Option<Res<Clock>>,
    liquids: Res<Liquids>,
    images: Res<Assets<Image>>,
    mut materials: ResMut<Assets<Sorted>>,
) {
    let ms = time.elapsed().as_millis();
    let light = bed_light(clock.map_or(0.0, |clock| clock.minutes()));
    let light = Color::srgb(light, light, light);
    for animated in liquids.animated.values() {
        let Some(frame) = frame_at(ms, animated.frames.len())
            .and_then(|i| animated.frames.get(i))
            .filter(|frame| images.contains(*frame))
        else {
            continue;
        };
        let layers = [
            (Some(&animated.material), None),
            (animated.bed.as_ref(), Some(light)),
        ];
        for (handle, colour) in layers {
            let Some(handle) = handle else {
                continue;
            };
            let Some(material) = materials.get(handle) else {
                continue;
            };
            if material.base.base_color_texture.as_ref() == Some(frame)
                && colour.is_none_or(|colour| material.base.base_color == colour)
            {
                continue;
            }
            if let Some(mut material) = materials.get_mut(handle) {
                material.base.base_color_texture = Some(frame.clone());
                if let Some(colour) = colour {
                    material.base.base_color = colour;
                }
            }
        }
    }
}

/// The liquids' frames.
pub struct LiquidsPlugin;

impl Plugin for LiquidsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, animate.in_set(crate::frame::Playing));
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::{Vec2, Vec3};

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
            buildings: Vec::new(),
        };
        let Surfaces { surfaces, beds } = build(&sector);
        let [(0, mesh)] = &surfaces[..] else {
            panic!("one surface of material 0");
        };
        assert_eq!(beds.len(), 1, "a second layer under the clear water");
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
    fn the_second_layer_is_as_bright_as_the_bed_is_deep() {
        // 80 over a bed at −10, white from −32 down, none over a bed at 0 or above.
        assert_eq!(
            [5, 0, -1, -10, -31, -32, -60].map(bed_grey),
            [0, 0, 8, 80, 248, 255, 255]
        );
        // All of it by day, none by night.
        assert!((bed_light(12.0 * 60.0) - 255.0 / 256.0).abs() < 1e-6);
        assert_eq!(bed_light(0.0), 0.0);
    }

    #[test]
    fn the_second_layer_lies_on_the_bed_at_the_surfaces_depth() {
        let cell = Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0, -10, -20, -10],
            light: [255; 4],
            heights2: [0; 4],
            offsets: [0; 2],
            flags: 0,
            ground: 0x90,
        };
        let mut piece = Piece::default();
        bed(&mut piece, &cell, (3, 2));
        // On the screen, each corner as far below the surface's as the bed is deep, at the depth
        // of the surface's plane there.
        let screen =
            |p: Vec3| Vec2::new(p.dot(iso::right()), -p.dot(iso::up())) * iso::PIXELS_PER_UNIT;
        let corners = [(3.0, 3.0), (3.0, 2.0), (4.0, 2.0), (4.0, 3.0)];
        for ((gx, gy), (position, height)) in corners
            .into_iter()
            .zip(piece.positions.iter().zip(cell.heights))
        {
            let position = Vec3::from_array(*position);
            let flat = iso::ground(gx, gy) + iso::forward() * BACK;
            let shift = screen(position) - screen(flat);
            assert!(
                (shift - Vec2::new(0.0, -f32::from(height))).length() < 1e-2,
                "{shift}"
            );
            assert!((position.y - flat.y).abs() < 1e-3, "{position}");
        }
        assert_eq!(piece.uvs[1], [1.5, 1.0]);
    }

    #[test]
    fn only_the_clear_waters_have_the_second_layer() {
        let clear: Vec<usize> = (0..MATERIALS.len())
            .filter(|&m| MATERIALS[m].clear)
            .collect();
        assert_eq!(clear, [0, 1, 2, 3, 10, 11]);
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
