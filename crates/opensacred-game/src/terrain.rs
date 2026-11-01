//! The ground of a sector (docs/02 §5): every cell's diamond, the ground tile with its floor
//! overlays, composed once into a slot of the sector's atlas (one slot per distinct stack of
//! layers), and draped over a mesh of the cells with their corner heights and light, as the
//! original draws a cell as a quad of its diamond. The mesh lies `iso::GROUND_BEHIND` back along the line of sight, so that it
//! never hides an object.
//!
//! The atlas of the median sector is 13 MB, a fifth of the sector's image on screen; a shader
//! that composes the layers would shrink it further.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin};
use bevy::prelude::{
    App, Asset, Assets, Commands, Entity, Handle, Image, Mesh, Mesh3d, MeshMaterial3d, Plugin,
    Resource, TypePath, Vec3,
};
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, RenderPipelineDescriptor, SpecializedMeshPipelineError,
    TextureDimension, TextureFormat,
};
use bevy::shader::ShaderRef;
use opensacred_data::texture;
pub use opensacred_data::texture::Rgba;
use opensacred_data::world::{Cell, DIAMONDS, LayerCell, SIDE, Sector, World};

use crate::{buildings, files, iso, liquids};

/// A diamond's box in a tile sheet and on screen.
const DIAMOND_W: usize = 96;
const DIAMOND_H: usize = 48;
/// A slot of the ground atlas: a diamond's box and a border of one pixel, which repeats the
/// box's edge so that filtering does not blend in the neighbouring slot.
const SLOT_W: usize = DIAMOND_W + 2;
const SLOT_H: usize = DIAMOND_H + 2;
/// Slots per row of the atlas.
const SLOTS_PER_ROW: usize = 64;

/// The corner heights of the cells of the loaded sectors, in world units, the drawn ground's and
/// the heights the creatures stand at, and their natures (the high nibble of the ground byte,
/// docs/02 §5.2), by global cell.
#[derive(Resource, Default)]
pub struct Heights {
    cells: HashMap<(u32, u32), LoadedCell>,
    /// The inside layers of the loaded sectors.
    layers: HashMap<LayerKey, LoadedLayer>,
    /// The floors of the loaded sectors' buildings, from floor 1 up.
    floors: HashMap<buildings::Anchor, Vec<LayerKey>>,
    /// The building and the floor, from 1, that each inside layer is.
    floor_of: HashMap<LayerKey, (buildings::Anchor, u8)>,
    /// The loaded sectors' buildings open to the sky ([`Self::open`]).
    open: HashSet<buildings::Anchor>,
    /// How high, in world units, each inside layer that is a building's floor lies above the
    /// ground ([`Self::lift`]).
    lifts: HashMap<LayerKey, f32>,
    /// The cells closed to every creature, at their levels: the closed triggers'
    /// ([`Self::occupy`]).
    occupied: HashSet<((i32, i32), Level)>,
    /// The liquid materials of the loaded sectors' two slots ([`Self::liquids`]).
    liquids: HashMap<(u32, u32), [u8; 2]>,
}

/// An inside layer: of its sector, by its index from 1 (docs/re/movement.md).
pub type LayerKey = (u32, u32, u16);
/// Where the hero walks: the ground (`None`) or an inside layer.
pub type Level = Option<LayerKey>;
/// A level of a place of the way, and whether the step onto the place went from off the stairs
/// onto a stair, which takes the hero to another floor: the client moves a creature between
/// floors only from a stair it did not arrive on, its flag `0x100`, which a cell off the stairs
/// clears (`0x5FCCB0`, docs/re/movement.md).
pub type Footing = (Level, bool);

/// Where a hero put on a cell by a command stands ([`Heights::landing`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Landing {
    /// On the cell, at a level.
    On(Level),
    /// On the ground, at the cell nearest that a hero may step into and walk on from.
    Near((i32, i32)),
}

/// What a cell of the loaded sectors is, as the command `cell-info` tells it
/// ([`Heights::about`]).
#[derive(Clone, Debug, PartialEq)]
pub struct About {
    /// The ground byte (`+0x1F`) and the flags (`+0x1E`, docs/02 §5.2).
    pub ground: u8,
    pub flags: u8,
    /// The corner heights of the drawn ground and those a creature stands at, left, top, right
    /// and bottom.
    pub heights: [f32; 4],
    pub standing: [f32; 4],
    /// Whether the hero may walk on it, whether an arrow flies over it, whether it is an entrance
    /// of its building, and whether a step on it sounds as on a bridge.
    pub walkable: bool,
    pub flown_over: bool,
    pub entrance: bool,
    pub bridged: bool,
    /// The building it is a cell of.
    pub building: Option<buildings::Anchor>,
    /// Whether a closed trigger holds it on the ground ([`Heights::occupy`]).
    pub closed: bool,
    /// The cells of the inside layers whose grids hold it.
    pub inside: Vec<Inside>,
}

/// A cell of an inside layer ([`About`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Inside {
    pub level: LayerKey,
    /// The building and the floor, from 1, that the layer is.
    pub floor: Option<(buildings::Anchor, u8)>,
    pub walkable: bool,
    pub flown_over: bool,
    pub exit: bool,
    /// The floors a stair there takes a creature up (down when below 0), and the cells it moves
    /// it by.
    pub stair: Option<(i8, [i8; 2])>,
    /// Whether a closed trigger holds it.
    pub closed: bool,
    /// Whether a hero who comes onto the layer by the entrances of the ground reaches it
    /// ([`Heights::landing`]'s room).
    pub room: bool,
}

/// How far from a cell given by a command, in cells along each axis, a hero that cannot stand
/// on it is put on the ground nearest it: a sector's side.
pub const LANDING_REACH: i32 = 64;

/// How many places a hero put on a cell by a command must reach from it not to be shut in
/// there ([`Heights::shut_in`]): a quarter of a sector's cells, an estimate; the pocket that
/// #458 met by the building of 4,37 anchored at (268, 2378) is a single cell.
const OPEN: usize = 1024;

/// An inside layer's grid: its first cell, its size, and its cells.
struct LoadedLayer {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    cells: Vec<Spot>,
}

/// A cell of an inside layer: whether the hero may walk on it, whether an arrow flies over it
/// (the low nibble 2), whether it is an exit, which takes the hero out onto the ground, whether it
/// is a stair, and the heights a creature stands at on its corners ([`second_height`]).
#[derive(Clone, Copy)]
struct Spot {
    walkable: bool,
    flown_over: bool,
    exit: bool,
    stair: Option<Stair>,
    standing: [f32; 4],
}

/// A stair of a building's floor (the low nibbles 5 to 8 of the ground byte, `0x5FCCB0`): it
/// takes a creature `floors` floors up or down (not below the ground) and moves it by `offsets`
/// cells, the stair cell's offsets (`+0x1C`, `+0x1D`), onto the other floor's grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stair {
    floors: i8,
    offsets: [i8; 2],
}

impl Stair {
    /// The stair of a cell of an inside layer, if it is one: 5 one floor up, 6 one down, 7 two
    /// up, 8 two down.
    fn of(cell: &LayerCell) -> Option<Self> {
        let floors = match cell.ground & 0xF {
            5 => 1,
            6 => -1,
            7 => 2,
            8 => -2,
            _ => return None,
        };
        Some(Self {
            floors,
            offsets: cell.offsets,
        })
    }
}

impl LoadedLayer {
    /// The cell at global `(cx, cy)`, if the grid holds it.
    fn spot(&self, (cx, cy): (i32, i32)) -> Option<Spot> {
        let (x, y) = (cx - self.x, cy - self.y);
        if !(0..self.width).contains(&x) || !(0..self.height).contains(&y) {
            return None;
        }
        self.cells
            .get(usize::try_from(y * self.width + x).ok()?)
            .copied()
    }

    /// Whether global cell `(cx, cy)` may be walked on, if the grid holds it.
    fn get(&self, cell: (i32, i32)) -> Option<bool> {
        self.spot(cell).map(|spot| spot.walkable)
    }
}

/// A cell's flag of a door.
const DOOR: u8 = 0x4;
/// The low nibbles of the ground byte that move an object between the ground and a building's
/// inside as it steps onto the cell (`0x5FCCB0`, docs/re/movement.md): an entrance, on a
/// building's cell of the ground, takes it onto the building's inside layer; an exit, on an
/// inside layer, takes it out onto the ground.
const ENTRANCE: u8 = 9;
const EXIT: u8 = 10;
/// The low nibble of the ground byte of the cells that may not be walked on but that an arrow
/// flies over: the ponds, the fences and the walls (`0x5BAA60`, docs/re/fight.md, "The archers").
const FLOWN_OVER: u8 = 2;
/// The natures of the two liquids, `WATERA` and `WATERB`, and of a bridge (docs/02 §5.2).
const LIQUIDS: [u8; 2] = [9, 10];
/// The natures' names, by the high nibble of the ground byte, as the client's debug line names
/// them (`0x6CD702`, docs/02 §5.2).
pub const NATURES: [&str; 16] = [
    "none",
    "DESERT",
    "ARCTIC",
    "WALD",
    "GRASS",
    "SUMPF",
    "LAVA",
    "BRIDGE",
    "DUNGEON",
    "WATERA",
    "WATERB",
    "STONEROAD",
    "KIESROAD",
    "IND-STONE",
    "IND-WOOD",
    "KIES",
];
const BRIDGE: u8 = 7;

/// Units of the models' height a step of a cell's second corner heights (`+0x18`) is: the client
/// stands a creature on them, times 2.5 (`0x636520`, docs/re/movement.md, "The height").
const STANDING_STEP: f32 = 2.5;

/// The height in world units of a second corner height.
fn second_height(step: i8) -> f32 {
    iso::height(f32::from(step) * STANDING_STEP * iso::PIXELS_PER_MODEL_HEIGHT)
}

/// The height at `(u, v)` of a cell (its fractions along X and Z from its top corner) whose
/// corners, left, top, right and bottom, stand at `corners`, as the client interpolates them
/// (`0x636520`): linearly in the triangle of the diamond's centre, at the corners' mean, and of
/// its side the point lies by, toward the right or the left corner as the point lies right or
/// left of the centre on the screen, toward the bottom or the top one as it lies below or above.
fn on_triangles([left, top, right, bottom]: [f32; 4], u: f32, v: f32) -> f32 {
    let centre = (left + top + right + bottom) / 4.0;
    // On the screen, the point lies `48 (u − v)` pixels right of the centre and `24 (u + v − 1)`
    // below it.
    let (across, down) = (u - v, u + v - 1.0);
    let side = if across > 0.0 { right } else { left };
    let end = if down > 0.0 { bottom } else { top };
    centre + across.abs() * (side - centre) + down.abs() * (end - centre)
}

/// Whether a creature's step on a cell sounds as on a bridge over a liquid, as the client's
/// `0x639080` gives the footstep `BRIDGE` for the cell's nature: a liquid cell whose second corner
/// heights (left, top, right, bottom) differ from its first and rise above 0 at the right or the
/// bottom corner, or at both the left and the top.
fn bridged(cell: &Cell) -> bool {
    let [left, top, right, bottom] = cell.heights2;
    LIQUIDS.contains(&(cell.ground >> 4))
        && cell.heights2 != cell.heights
        && ((left > 0 && top > 0) || right > 0 || bottom > 0)
}

struct LoadedCell {
    heights: [f32; 4],
    /// The heights a creature stands at on the cell's corners ([`second_height`]).
    standing: [f32; 4],
    /// The ground byte (`+0x1F`), and its high nibble, the nature.
    ground: u8,
    nature: u8,
    /// Under a liquid, the first corner heights as the sector's file has them, its bed's
    /// ([`Heights::liquid_bed`]).
    bed: Option<[i8; 4]>,
    /// Whether a step on the cell sounds as on a bridge ([`bridged`]).
    bridged: bool,
    /// The cell's flags (docs/02 §5.2) and the building it belongs to.
    flags: u8,
    building: Option<buildings::Anchor>,
    /// Whether the hero may walk on the cell ([`walkable`]), and whether an arrow flies over it
    /// (the low nibble 2).
    walkable: bool,
    flown_over: bool,
    /// Whether the Daemon in his form of 1094 may step onto it though it may not be walked on
    /// (`opensacred_rules::arts::wades`).
    wadable: bool,
    /// Whether the cell is an entrance of its building ([`ENTRANCE`]).
    entrance: bool,
}

/// Whether a cell of the ground or of an inside layer may be walked on, by its flags and ground
/// byte, by the client's test (`0x636C10`, docs/re/movement.md): not when the low nibble of its
/// ground byte is 1 or 2, or its flags have bit 3, but a door (flag bit 2), which its trigger's
/// state opens (**hypothesis**: the game has no states, and the doors are open). A building is
/// its mass on the ground, 1 in the low nibble, and its inside a layer of its own.
fn walkable(flags: u8, ground: u8) -> bool {
    flags & DOOR != 0 || !(matches!(ground & 0xF, 1 | 2) || flags & 0x8 != 0)
}

impl Heights {
    /// The tests' house, as in sector 50,40: on the ground of sector (0, 0), a mass of cells
    /// 4..=6 on rows 4..=6 with its door at (5, 6), an entrance (nibble 9), and the cells in front
    /// of it on row 7 building cells of nibble 10, exits; its inside, the layer `(0, 0, 1)` of
    /// cells 4..=7 on rows 4..=7, free but its back wall, the door again, the exits in front of
    /// it, and a column (7) that is walkable on the ground too.
    #[cfg(test)]
    pub(crate) fn house() -> Self {
        use opensacred_data::world::Layer;

        let cell = |ground, flags| Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [0; 4],
            offsets: [0; 2],
            flags,
            ground,
        };
        let mut cells = vec![cell(0x40, 0); SIDE * SIDE];
        for x in 4..=6 {
            for y in 4..=6 {
                cells[y * SIDE + x] = cell(0xD1, 0x1);
            }
            cells[7 * SIDE + x] = cell(0x4A, 0x1);
        }
        cells[6 * SIDE + 5] = cell(0xD9, 0x5);
        let inside = |x: u32, y: u32| LayerCell {
            heights2: [0; 4],
            offsets: [0; 2],
            flags: if (x, y) == (1, 2) { 4 } else { 0 },
            ground: match (x, y) {
                (0..=2, 0) => 0xD1,
                (1, 2) => 0xD9,
                (0..=2, 3) => 0xDA,
                _ => 0xD0,
            },
        };
        let sector = Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells,
            statics: Vec::new(),
            layers: vec![Layer {
                x: 4,
                y: 4,
                width: 4,
                height: 4,
                cells: (0..4)
                    .flat_map(|y| (0..4).map(move |x| inside(x, y)))
                    .collect(),
            }],
            buildings: Vec::new(),
        };
        let mut heights = Self::default();
        heights.add(&sector);
        heights
    }

    /// Adds the cells of `sector` and its inside layers.
    pub fn add(&mut self, sector: &Sector) {
        self.liquids.insert((sector.x, sector.y), sector.liquids);
        for (index, layer) in sector.layers.iter().enumerate() {
            let (Ok(key), Ok(x), Ok(y), Ok(width), Ok(height)) = (
                u16::try_from(index + 1),
                i32::try_from(layer.x),
                i32::try_from(layer.y),
                i32::try_from(layer.width),
                i32::try_from(layer.height),
            ) else {
                continue;
            };
            let cells = layer
                .cells
                .iter()
                .map(|cell| Spot {
                    walkable: walkable(cell.flags, cell.ground),
                    flown_over: cell.ground & 0xF == FLOWN_OVER,
                    exit: cell.ground & 0xF == EXIT,
                    stair: Stair::of(cell),
                    standing: cell.heights2.map(second_height),
                })
                .collect();
            self.layers.insert(
                (sector.x, sector.y, key),
                LoadedLayer {
                    x,
                    y,
                    width,
                    height,
                    cells,
                },
            );
        }
        for building in &sector.buildings {
            let Some(anchor) = buildings::cell(sector, usize::from(building.cell)) else {
                continue;
            };
            let floors: Vec<LayerKey> = building
                .floors
                .iter()
                .map(|&layer| (sector.x, sector.y, layer))
                .collect();
            for (n, &key) in floors.iter().enumerate() {
                if let Ok(floor) = u8::try_from(n + 1) {
                    self.floor_of.insert(key, (anchor, floor));
                }
                if let Some(&height) = building.heights.get(n) {
                    self.lifts.insert(
                        key,
                        iso::height(f32::from(height) * iso::PIXELS_PER_MODEL_HEIGHT),
                    );
                }
            }
            self.floors.insert(anchor, floors);
            if building.open {
                self.open.insert(anchor);
            }
        }
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
                    standing: cell.heights2.map(second_height),
                    ground: cell.ground,
                    nature: cell.ground >> 4,
                    bed: liquids::liquid(cell).then_some(cell.heights),
                    bridged: bridged(cell),
                    flags: cell.flags,
                    building: buildings::anchor(sector, index),
                    walkable: walkable(cell.flags, cell.ground),
                    flown_over: cell.ground & 0xF == FLOWN_OVER,
                    wadable: opensacred_rules::arts::wades(cell.ground, cell.heights),
                    entrance: cell.ground & 0xF == ENTRANCE
                        && cell.flags & buildings::BUILDING_CELL != 0,
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

    /// The building the cell at world `(x, z)` belongs to, and the cell's flags.
    pub fn building(&self, x: f32, z: f32) -> Option<(buildings::Anchor, u8)> {
        self.cell(x, z)
            .and_then(|(cell, _, _)| cell.building.map(|building| (building, cell.flags)))
    }

    /// Whether global cell `(cx, cy)` of the ground may be walked on; not outside the loaded
    /// sectors.
    pub fn walkable(&self, (cx, cy): (i32, i32)) -> bool {
        match (u32::try_from(cx), u32::try_from(cy)) {
            (Ok(cx), Ok(cy)) => self.cells.get(&(cx, cy)).is_some_and(|cell| cell.walkable),
            _ => false,
        }
    }

    /// Whether a creature may step into a cell at a level, as the client's test of a step does
    /// (`0x4F7E10`, docs/re/movement.md, "The objects in the way"): the cell may be walked on, on
    /// the ground or in an inside layer's grid, and no closed trigger holds it (the creatures and
    /// the chests that stop only some are `crowd`'s).
    pub fn walkable_on(&self, cell: (i32, i32), level: Level) -> bool {
        self.walkable_at(cell, level) && !self.occupied.contains(&(cell, level))
    }

    /// Whether the Daemon in his form of 1094 may step into a cell at a level that may not be
    /// walked on (`0x4EBCA0` at `0x4EBD62`): a cell of the ground that he wades over
    /// (`opensacred_rules::arts::wades`) and no closed trigger holds; the inside layers keep no
    /// first heights, and none there.
    pub fn wadable_on(&self, (cx, cy): (i32, i32), level: Level) -> bool {
        level.is_none()
            && !self.occupied.contains(&((cx, cy), level))
            && match (u32::try_from(cx), u32::try_from(cy)) {
                (Ok(x), Ok(y)) => self.cells.get(&(x, y)).is_some_and(|cell| cell.wadable),
                _ => false,
            }
    }

    /// Whether a cell may be walked on at a level, on the ground or in an inside layer's grid,
    /// whatever objects take it (`0x636C10`).
    pub fn walkable_at(&self, cell: (i32, i32), level: Level) -> bool {
        match level {
            None => self.walkable(cell),
            Some(key) => self
                .layers
                .get(&key)
                .and_then(|layer| layer.get(cell))
                .unwrap_or(false),
        }
    }

    /// Whether an arrow flies over a cell at a level that may not be walked on: one of the low
    /// nibble 2 of its ground byte, on the ground or in an inside layer's grid (`0x5BAA60`).
    pub fn flown_over(&self, (cx, cy): (i32, i32), level: Level) -> bool {
        match level {
            None => match (u32::try_from(cx), u32::try_from(cy)) {
                (Ok(x), Ok(y)) => self.cells.get(&(x, y)).is_some_and(|cell| cell.flown_over),
                _ => false,
            },
            Some(key) => self
                .layers
                .get(&key)
                .and_then(|layer| layer.spot((cx, cy)))
                .is_some_and(|spot| spot.flown_over),
        }
    }

    /// The flags (`+0x1E`) of global cell `(cx, cy)` of the ground; `None` outside the loaded
    /// sectors.
    pub fn flags(&self, (cx, cy): (i32, i32)) -> Option<u8> {
        let cell = (u32::try_from(cx).ok()?, u32::try_from(cy).ok()?);
        self.cells.get(&cell).map(|cell| cell.flags)
    }

    /// How many floors a loaded building has.
    pub fn floor_count(&self, building: buildings::Anchor) -> usize {
        self.floors.get(&building).map_or(0, Vec::len)
    }

    /// Marks a cell at a level as closed to every creature: a cell that a start script's
    /// `TriggerPatch` puts under a closed trigger, the chests' and the barrels' among them
    /// (`0x4EBCA0`, docs/re/movement.md, "The chests' cells"), which the ways go round and the
    /// steps stop at. It stays closed while the game runs: no script of the start opens them,
    /// and a sector that comes in again closes them again.
    pub fn occupy(&mut self, cell: (i32, i32), level: Level) {
        self.occupied.insert((cell, level));
    }

    /// Frees a cell that [`Self::occupy`] closed: a door's, as its trigger opens (#372).
    pub fn free(&mut self, cell: (i32, i32), level: Level) {
        self.occupied.remove(&(cell, level));
    }

    /// The places a hero at a place goes to next, as the client moves an object between levels
    /// as it steps onto a cell (`0x5FCCB0`, docs/re/movement.md): from a stair stepped onto from
    /// off the stairs, only the cell of the floor that the stair leads to; at an entrance or an
    /// exit, only the same cell at the level that it leads to; else the walkable neighbours at
    /// its level.
    pub fn neighbours(&self, place: ((i32, i32), Footing)) -> Vec<((i32, i32), Footing)> {
        self.neighbours_by(place, |cell, level| self.walkable_on(cell, level))
    }

    /// [`Self::neighbours`], a cell at a level stepped into where `walkable` holds it.
    pub fn neighbours_by(
        &self,
        (cell, (level, climbing)): ((i32, i32), Footing),
        walkable: impl Fn((i32, i32), Level) -> bool,
    ) -> Vec<((i32, i32), Footing)> {
        if climbing && let Some((to, floor)) = self.climb(cell, level) {
            return vec![(to, (floor, false))];
        }
        let through = self.transition(cell, level);
        if through != level && walkable(cell, through) {
            return vec![(cell, (through, false))];
        }
        let on_stairs = self.on_stairs(cell, level);
        crate::paths::around(cell, |next| walkable(next, level))
            .map(|next| (next, (level, !on_stairs && self.on_stairs(next, level))))
            .collect()
    }

    /// The stair at a cell of a level, if it is one.
    fn stair(&self, cell: (i32, i32), level: Level) -> Option<Stair> {
        self.layers
            .get(&level?)
            .and_then(|layer| layer.spot(cell))
            .and_then(|spot| spot.stair)
    }

    /// Whether a cell of a level is a stair.
    pub fn on_stairs(&self, cell: (i32, i32), level: Level) -> bool {
        self.stair(cell, level).is_some()
    }

    /// Where a stair at a cell of a building's floor takes a creature (`0x5FCCB0`): its floors up
    /// or down, onto the ground at the same cell below floor 1, else onto that floor of the
    /// building at the cell that the stair's offsets give; `None` where the building has no
    /// such floor, or the cell is no stair.
    pub fn climb(&self, cell: (i32, i32), level: Level) -> Option<((i32, i32), Level)> {
        let stair = self.stair(cell, level)?;
        let (building, floor) = self.floor(level)?;
        let to = (i16::from(floor) + i16::from(stair.floors)).max(0);
        if to == 0 {
            return Some((cell, None));
        }
        let next = *self
            .floors
            .get(&building)?
            .get(usize::try_from(to - 1).ok()?)?;
        let [dx, dy] = stair.offsets.map(i32::from);
        Some(((cell.0 + dx, cell.1 + dy), Some(next)))
    }

    /// How high over its grid's heights a creature at a level stands: a building's floor's height
    /// (`Static.PAK +0x33` of the floor's object, times 28 units of the models' height), 0 on the
    /// ground and on floor 1, as the client adds it (`0x449950`, docs/re/movement.md, "The
    /// height").
    pub fn lift(&self, level: Level) -> f32 {
        level
            .and_then(|key| self.lifts.get(&key))
            .copied()
            .unwrap_or(0.0)
    }

    /// The level of floor `floor` (from 1) of `building`: `None` where it is not loaded.
    pub fn floor_level(&self, building: buildings::Anchor, floor: usize) -> Level {
        self.floors
            .get(&building)
            .and_then(|floors| floors.get(floor.checked_sub(1)?))
            .copied()
    }

    /// The building and the floor, from 1, that a level is.
    pub fn floor(&self, level: Level) -> Option<(buildings::Anchor, u8)> {
        self.floor_of.get(&level?).copied()
    }

    /// Whether a building is open to the sky, its object's flag `0x400`: the client draws the
    /// objects of all its floors whatever its state (docs/re/drawing.md, "Buildings: masks and
    /// layers").
    pub fn open(&self, building: buildings::Anchor) -> bool {
        self.open.contains(&building)
    }

    /// The building one of whose floor `floor` (from 1) has a grid that holds global `cell`: the
    /// building of the cell on the ground, else any loaded one's.
    pub fn building_on_floor(&self, cell: (i32, i32), floor: usize) -> Option<buildings::Anchor> {
        let holds = |building: &buildings::Anchor| {
            self.floors
                .get(building)
                .and_then(|floors| floors.get(floor.checked_sub(1)?))
                .and_then(|key| self.layers.get(key))
                .is_some_and(|layer| layer.spot(cell).is_some())
        };
        let ground = match (u32::try_from(cell.0), u32::try_from(cell.1)) {
            (Ok(cx), Ok(cy)) => self.cells.get(&(cx, cy)).and_then(|c| c.building),
            _ => None,
        };
        ground
            .filter(holds)
            .or_else(|| self.floors.keys().copied().filter(holds).min())
    }

    /// The level of a hero at a level that steps onto a cell: on the ground, an entrance of a
    /// building takes it onto the building's inside ([`Self::level_of`]); on an inside layer, an
    /// exit takes it out onto the ground; any other cell leaves it where it is (`0x5FCCB0`).
    pub fn transition(&self, cell: (i32, i32), level: Level) -> Level {
        match level {
            None => {
                let entrance = match (u32::try_from(cell.0), u32::try_from(cell.1)) {
                    (Ok(cx), Ok(cy)) => self.cells.get(&(cx, cy)).is_some_and(|c| c.entrance),
                    _ => false,
                };
                if entrance { self.level_of(cell) } else { None }
            }
            Some(key) => {
                let exit = self
                    .layers
                    .get(&key)
                    .and_then(|layer| layer.spot(cell))
                    .is_some_and(|spot| spot.exit);
                if exit { None } else { level }
            }
        }
    }

    /// The inside layer that a hero at a cell of a building goes onto, through its entrance or
    /// at a start on its mass: the building's floor 1, the first child of its object
    /// (`0x5FCCB0`, `0x635440`), where its grid holds the cell walkable; for a building whose
    /// floors are not known, the first of the inside layers whose grids hold the cell walkable.
    pub fn level_of(&self, cell: (i32, i32)) -> Level {
        let walkable_in = |key: &LayerKey| {
            self.layers
                .get(key)
                .and_then(|layer| layer.get(cell))
                .unwrap_or(false)
        };
        let building = match (u32::try_from(cell.0), u32::try_from(cell.1)) {
            (Ok(cx), Ok(cy)) => self.cells.get(&(cx, cy)).and_then(|c| c.building),
            _ => None,
        };
        if let Some(floors) = building.and_then(|b| self.floors.get(&b)) {
            return floors.first().copied().filter(walkable_in);
        }
        self.layers
            .keys()
            .filter(|key| walkable_in(key))
            .copied()
            .min()
    }

    /// Where a hero put on global `cell` by a command (`cell`, `sector`) stands, so that it can
    /// walk on (#458): where the ground may not be walked on there, on the inside layer that
    /// [`Self::level_of`] gives where the cell is one of its rooms ([`Self::room`]); else on
    /// the cell of the ground nearest it, the cell itself first, within [`LANDING_REACH`] along
    /// each axis, that a hero may step into and that does not shut it in ([`Self::shut_in`]);
    /// `None` where there is none. The client has no such command: its hero comes onto a
    /// building's floor by an entrance or a stair, or starts on one where a script puts it,
    /// onto floor 1 wherever the floor's grid holds the cell walkable, and so do the empty
    /// cells of the grid about its rooms.
    pub fn landing(&self, cell: (i32, i32)) -> Option<Landing> {
        if !self.walkable(cell)
            && let Some(level) = self.level_of(cell)
            && self.room(cell, level)
        {
            return Some(Landing::On(Some(level)));
        }
        let (x, y) = cell;
        let mut near: Vec<(i32, i32)> = (-LANDING_REACH..=LANDING_REACH)
            .flat_map(|dy| (-LANDING_REACH..=LANDING_REACH).map(move |dx| (x + dx, y + dy)))
            .filter(|&at| self.walkable_on(at, None))
            .collect();
        // Nearest first; of those as near, row by row.
        near.sort_by_key(|&(ax, ay)| (ax - x).pow(2) + (ay - y).pow(2));
        let mut shut = HashSet::new();
        for at in near {
            if shut.contains(&(at, (None, false))) {
                continue;
            }
            match self.shut_in(at) {
                None if at == cell => return Some(Landing::On(None)),
                None => return Some(Landing::Near(at)),
                Some(reached) => shut.extend(reached),
            }
        }
        None
    }

    /// What global `cell` of the loaded sectors is, on the ground and on the inside layers whose
    /// grids hold it, for `cell-info`; `None` outside the loaded sectors.
    pub fn about(&self, cell: (i32, i32)) -> Option<About> {
        let loaded = self
            .cells
            .get(&(u32::try_from(cell.0).ok()?, u32::try_from(cell.1).ok()?))?;
        let mut inside: Vec<Inside> = self
            .layers
            .iter()
            .filter_map(|(&level, layer)| {
                let spot = layer.spot(cell)?;
                Some(Inside {
                    level,
                    floor: self.floor_of.get(&level).copied(),
                    walkable: spot.walkable,
                    flown_over: spot.flown_over,
                    exit: spot.exit,
                    stair: spot.stair.map(|stair| (stair.floors, stair.offsets)),
                    closed: self.occupied.contains(&(cell, Some(level))),
                    room: self.room(cell, level),
                })
            })
            .collect();
        inside.sort_by_key(|inside| (inside.floor, inside.level));
        Some(About {
            ground: loaded.ground,
            flags: loaded.flags,
            heights: loaded.heights,
            standing: loaded.standing,
            walkable: loaded.walkable,
            flown_over: loaded.flown_over,
            entrance: loaded.entrance,
            bridged: loaded.bridged,
            building: loaded.building,
            closed: self.occupied.contains(&(cell, None)),
            inside,
        })
    }

    /// The places that a hero on global `cell` of the ground reaches, by the ways of
    /// [`Self::neighbours`] through the buildings' doors and floors, whatever closed triggers
    /// hold, when they are fewer than [`OPEN`]: a pocket of the ground that it cannot walk out
    /// of, such as a building's cell of the ground amid its masses; `None` where it walks on.
    fn shut_in(&self, cell: (i32, i32)) -> Option<HashSet<((i32, i32), Footing)>> {
        self.reached(cell, None)
    }

    /// How many places a hero on global `cell` of `level` reaches, as [`Self::shut_in`] counts
    /// them, when they are fewer than [`OPEN`]: the size of the pocket it stands in; `None` where
    /// it walks on (`cell-info`).
    pub fn pocket(&self, cell: (i32, i32), level: Level) -> Option<usize> {
        self.reached(cell, level).map(|reached| reached.len())
    }

    /// The places that a hero on global `cell` of `level` reaches, when they are fewer than
    /// [`OPEN`] ([`Self::shut_in`]).
    fn reached(&self, cell: (i32, i32), level: Level) -> Option<HashSet<((i32, i32), Footing)>> {
        let walkable = |at, on| self.walkable_at(at, on);
        let start = (cell, (level, false));
        let mut seen = HashSet::from([start]);
        let mut queue = vec![start];
        while let Some(place) = queue.pop() {
            for next in self.neighbours_by(place, walkable) {
                if seen.insert(next) {
                    if seen.len() >= OPEN {
                        return None;
                    }
                    queue.push(next);
                }
            }
        }
        Some(seen)
    }

    /// Whether a hero that comes onto inside layer `level` by the entrances of the ground that
    /// lead onto it reaches its global `cell`, by the ways of [`Self::neighbours`] through the
    /// building's floors, whatever closed triggers hold (a closed door, a chest).
    fn room(&self, cell: (i32, i32), level: LayerKey) -> bool {
        let walkable = |at, on| self.walkable_at(at, on);
        let mut seen: HashSet<((i32, i32), Footing)> = self
            .cells
            .iter()
            .filter(|(_, loaded)| loaded.entrance)
            .filter_map(|(&(x, y), _)| Some((i32::try_from(x).ok()?, i32::try_from(y).ok()?)))
            .filter(|&at| self.level_of(at) == Some(level) && walkable(at, Some(level)))
            .map(|at| (at, (Some(level), false)))
            .collect();
        let mut queue: Vec<_> = seen.iter().copied().collect();
        while let Some(place) = queue.pop() {
            if place.0 == cell && place.1.0 == Some(level) {
                return true;
            }
            for next in self.neighbours_by(place, walkable) {
                // Out by an exit or down the stairs onto the ground, it has left the building.
                if next.1.0.is_some() && seen.insert(next) {
                    queue.push(next);
                }
            }
        }
        false
    }

    /// The bed of the global cell `(cx, cy)` under a liquid, the natures `WATERA` and `WATERB`: its
    /// four first corner heights as its sector's file has them, by which the crabs and fish take
    /// their cells (`0x47C5B0`); `None` for any other cell and outside the loaded sectors.
    pub fn liquid_bed(&self, (cx, cy): (i32, i32)) -> Option<[i8; 4]> {
        let key = (u32::try_from(cx).ok()?, u32::try_from(cy).ok()?);
        self.cells.get(&key)?.bed
    }

    /// The liquid material of the global cell `(cx, cy)` under a liquid: its sector's material of
    /// the cell's slot, `WATERA` the first and `WATERB` the second (docs/02 §5.1); `None` for any
    /// other cell and outside the loaded sectors.
    pub fn liquid_material(&self, (cx, cy): (i32, i32)) -> Option<u8> {
        let key = (u32::try_from(cx).ok()?, u32::try_from(cy).ok()?);
        let cell = self.cells.get(&key)?;
        cell.bed?;
        let side = u32::try_from(SIDE).ok()?;
        let slots = self.liquids(((key.0 / side), (key.1 / side)))?;
        slots.get(usize::from(cell.nature.checked_sub(9)?)).copied()
    }

    /// The liquid materials of a loaded sector's two slots (docs/02 §5.1).
    pub fn liquids(&self, sector: (u32, u32)) -> Option<[u8; 2]> {
        self.liquids.get(&sector).copied()
    }

    /// The nature of the cell at world `(x, z)`.
    pub fn nature(&self, x: f32, z: f32) -> Option<u8> {
        self.cell(x, z).map(|(cell, _, _)| cell.nature)
    }

    /// The nature a creature's step on the cell at world `(x, z)` sounds by, as the client's
    /// `0x639080` gives it to the footstep: `BRIDGE` on a bridge over a liquid ([`bridged`]),
    /// else the cell's own.
    pub fn step_nature(&self, x: f32, z: f32) -> Option<u8> {
        self.cell(x, z)
            .map(|(cell, _, _)| if cell.bridged { BRIDGE } else { cell.nature })
    }

    /// The height a creature at world `(x, z)` on a level stands at, as the client gives it
    /// (`0x636520`, `0x449950`): the second corner heights of the level's cell there, or of the
    /// ground's where the level's grid has none, interpolated on the triangles of the cell's
    /// centre ([`on_triangles`]), and on a building's floor the floor's height over them
    /// ([`Self::lift`]); `None` outside the loaded sectors.
    pub fn standing(&self, x: f32, z: f32, level: Level) -> Option<f32> {
        let (fx, fz) = (x / iso::CELL, z / iso::CELL);
        let corners = level
            .and_then(|key| self.layers.get(&key))
            .and_then(|layer| layer.spot(crate::paths::cell_of(x, z)))
            .map(|spot| spot.standing)
            .or_else(|| self.cell(x, z).map(|(cell, _, _)| cell.standing))?;
        Some(on_triangles(corners, fx - fx.floor(), fz - fz.floor()) + self.lift(level))
    }

    /// The height of the drawn ground at world `(x, z)`, interpolated between the corners of its
    /// cell (left, top, right, bottom); `None` outside the loaded sectors. A creature stands at
    /// [`Self::standing`] instead.
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
/// two columns 104 apart, odd rows shifted by 52, a margin of 2 (docs/02 §5.4).
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

/// A texture that the game reads on the CPU, as the converter writes it: 8-bit RGBA, its texels as
/// the master's (`opensacred_data::texture`).
pub fn read_texture(path: &Path) -> Result<Rgba> {
    let data = files::read(path).with_context(|| format!("reading {}", path.display()))?;
    texture::read(&data).with_context(|| format!("decoding {}", path.display()))
}

/// The tile sheets, decoded on first use.
struct Sheets<'a> {
    dir: &'a Path,
    world: &'a World,
    loaded: HashMap<u32, Rgba>,
}

impl Sheets<'_> {
    /// Decodes the sheet of tile `tile`, unless it is already.
    fn load(&mut self, tile: u32) -> Result<()> {
        let sheet = tile / DIAMONDS;
        if !self.loaded.contains_key(&sheet) {
            let name = self
                .world
                .sheets
                .get(usize::try_from(sheet)?)
                .with_context(|| format!("no tile sheet {sheet}"))?;
            let image = read_texture(&self.dir.join(name))?;
            self.loaded.insert(sheet, image);
        }
        Ok(())
    }

    /// Tile `tile`'s diamond box, its sheet decoded ([`Self::load`]).
    fn diamond(&self, tile: u32) -> Result<Diamond<'_>> {
        let sheet = tile / DIAMONDS;
        let sheet = self
            .loaded
            .get(&sheet)
            .with_context(|| format!("tile sheet {sheet}"))?;
        Ok(Diamond {
            sheet,
            origin: diamond_origin(tile % DIAMONDS),
        })
    }
}

/// A tile's diamond box in its sheet.
struct Diamond<'a> {
    sheet: &'a Rgba,
    origin: (usize, usize),
}

impl Diamond<'_> {
    /// The colours and alphas of row `y` of the box from pixel `first` on, none beyond its sheet.
    fn row(&self, y: usize, first: usize) -> impl Iterator<Item = [u8; 4]> + '_ {
        let (sx, sy) = (self.origin.0 + first, self.origin.1 + y);
        let texels = if sx < self.sheet.width && sy < self.sheet.height {
            &self.sheet.pixels[4 * (sy * self.sheet.width + sx)..4 * (sy + 1) * self.sheet.width]
        } else {
            &[]
        };
        texels
            .as_chunks::<4>()
            .0
            .iter()
            .copied()
            .chain(std::iter::repeat([0; 4]))
    }
}

/// Lays `colour` over `out` through `alpha`, which leaves it opaque.
fn blend(out: &mut [u8; 4], colour: [u8; 4], alpha: u8) {
    match alpha {
        u8::MAX => out[..3].copy_from_slice(&colour[..3]),
        0 => {}
        alpha => {
            let a = u16::from(alpha);
            for k in 0..3 {
                let blended = (u16::from(colour[k]) * a + u16::from(out[k]) * (255 - a)) / 255;
                out[k] = u8::try_from(blended).unwrap_or(u8::MAX);
            }
        }
    }
    out[3] = u8::MAX;
}

/// The first and the last pixel of each row of a diamond's box inside the diamond ([`inside`]),
/// where the row has any: the diamond is convex, so the row's pixels inside it are those between.
type Spans = [Option<(usize, usize)>; DIAMOND_H];

fn spans() -> Spans {
    std::array::from_fn(|y| {
        let first = (0..DIAMOND_W).find(|&x| inside(x, y))?;
        let last = (0..DIAMOND_W).rev().find(|&x| inside(x, y))?;
        Some((first, last))
    })
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
/// each row, and the border repeats the box. Each layer's sheets are looked up once, its
/// pixels walked along the diamond's `spans`: a sector of the 3 × 3 built while the frame waits
/// (`sectors`, #1650) took some 300 ms over a lookup and a test a pixel.
fn compose_slot(
    layers: &Layers,
    sheets: &mut Sheets<'_>,
    spans: &Spans,
    slot: &mut [[u8; 4]],
) -> Result<()> {
    let at = |x: usize, y: usize| (y + 1) * SLOT_W + x + 1;
    for &(art, mask) in layers {
        sheets.load(art)?;
        if let Some(mask) = mask {
            sheets.load(mask)?;
        }
        let colours = sheets.diamond(art)?;
        let alphas = mask.map(|mask| sheets.diamond(mask)).transpose()?;
        for (y, span) in spans.iter().enumerate() {
            let Some((first, last)) = *span else {
                continue;
            };
            let row = slot[at(first, y)..=at(last, y)].iter_mut();
            match &alphas {
                Some(alphas) => {
                    let texels = colours.row(y, first).zip(alphas.row(y, first));
                    for (out, (colour, alpha)) in row.zip(texels) {
                        blend(out, colour, alpha[3]);
                    }
                }
                None => {
                    for (out, colour) in row.zip(colours.row(y, first)) {
                        blend(out, colour, colour[3]);
                    }
                }
            }
        }
    }
    for (y, span) in spans.iter().enumerate() {
        let (first, last) = span.unwrap_or((DIAMOND_W / 2, DIAMOND_W / 2));
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
    let spans = spans();
    for (index, layers) in order.iter().enumerate() {
        slot.fill([0; 4]);
        compose_slot(layers, sheets, &spans, &mut slot)?;
        let (sx, sy) = (
            (index % SLOTS_PER_ROW) * SLOT_W,
            (index / SLOTS_PER_ROW) * SLOT_H,
        );
        for (y, line) in slot.as_chunks::<SLOT_W>().0.iter().enumerate() {
            let row = 4 * ((sy + y) * width + sx);
            pixels[row..row + 4 * SLOT_W].copy_from_slice(line.as_flattened());
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
/// in the sector, in pixels, as the texture coordinate (`ground.wesl` finds the cell's slot).
fn mesh(sector: &Sector) -> Mesh {
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
        // The top corner of the cell's diamond on the sector's flat screen.
        let (top_x, top_y) = (48.0 * (column - row), 24.0 * (column + row));
        let first = u32::try_from(positions.len()).unwrap_or(u32::MAX);
        for (k, ((cx, cy), (dx, dy))) in corners.into_iter().enumerate() {
            let point = iso::ground(sx + cx, sy + cy)
                + Vec3::Y * iso::height(f32::from(cell.heights[k]))
                + iso::forward() * iso::GROUND_BEHIND;
            positions.push(point.to_array());
            uvs.push([top_x + dx, top_y + dy]);
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
        files::read_to_string(&index).with_context(|| format!("reading {}", index.display()))?;
    ron::from_str(&text).with_context(|| format!("parsing {}", index.display()))
}

/// Reads sector `(x, y)`; `None` where the grid has no sector.
pub fn read_sector(assets: &Path, world: &World, (x, y): (u32, u32)) -> Result<Option<Sector>> {
    let Some(entry) = world.sectors.iter().find(|s| (s.x, s.y) == (x, y)) else {
        return Ok(None);
    };
    let path = assets.join("world").join(&entry.file);
    let data = files::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let sector = Sector::decode(&data).with_context(|| format!("decoding {}", path.display()))?;
    Ok(Some(sector))
}

/// The ground of a sector, built off the main thread.
pub struct Ground {
    image: Image,
    cells: Image,
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
    let mesh = mesh(sector);
    let side = u32::try_from(SIDE)?;
    let slots: Vec<u8> = atlas
        .slots
        .iter()
        .map(|&slot| u32::try_from(slot).map(u32::to_le_bytes))
        .collect::<Result<Vec<_>, _>>()?
        .concat();
    let cells = Image::new(
        Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        slots,
        TextureFormat::R32Uint,
        RenderAssetUsages::RENDER_WORLD,
    );
    let image = Image::new(
        Extent3d {
            width: u32::try_from(atlas.width)?,
            height: u32::try_from(atlas.height)?,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        atlas.pixels,
        // The values as stored: the shader multiplies them by the light before decoding.
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    Ok(Ground { image, cells, mesh })
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

/// Where the hero starts in a sector given by hand (`--sector`): the centre of the walkable cell
/// nearest the sector's centre, at the height a creature stands at there (the mean of the cell's
/// second corner heights, [`Heights::standing`]), or the sector's centre where no cell is
/// walkable.
pub fn start(sector: &Sector) -> Vec3 {
    let middle = SIDE / 2;
    let Some((index, cell)) = sector
        .cells
        .iter()
        .enumerate()
        .filter(|(_, cell)| walkable(cell.flags, cell.ground))
        .min_by_key(|(index, _)| {
            (index % SIDE).abs_diff(middle).pow(2) + (index / SIDE).abs_diff(middle).pow(2)
        })
    else {
        return center(sector);
    };
    let mean = cell.heights2.map(second_height).iter().sum::<f32>() / 4.0;
    iso::ground(
        sector.x as f32 * SIDE as f32 + (index % SIDE) as f32 + 0.5,
        sector.y as f32 * SIDE as f32 + (index / SIDE) as f32 + 0.5,
    ) + Vec3::Y * mean
}

/// The shader of [`GroundMaterial`], embedded in the binary.
const SHADER: &str = "embedded://opensacred_game/ground.wesl";

/// The ground's material, unlit (`ground.wesl`): the sector's ground filtered as one image, each
/// texel from the slot of the cell it lies in, so that the cells' edges blend as their insides do
/// (filtering each cell's slot alone left a step at every edge, a grid of diamonds once
/// magnified); the texel as stored, in sRGB values, times the vertex colour, the corners' light,
/// then decoded to linear light. The original multiplies the 8-bit texel by its corners' diffuse
/// colour (docs/02 §5.3); the vertex colour of a `StandardMaterial` multiplies in linear light
/// instead, which leaves shaded ground too light.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct GroundMaterial {
    #[texture(0)]
    atlas: Handle<Image>,
    /// The slot of every cell, row by row.
    #[texture(1, sample_type = "u_int")]
    cells: Handle<Image>,
}

impl Material for GroundMaterial {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    /// The depth prepass holds the models' depths alone (`models`).
    fn enable_prepass() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// The ground's material and its shader.
pub struct GroundPlugin;

impl Plugin for GroundPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "ground.wesl");
        app.add_plugins(MaterialPlugin::<GroundMaterial>::default());
    }
}

/// Spawns a ground as an unlit mesh, the tiles carrying their own shading and the corners their
/// light, as in the original; returns the entities.
pub fn spawn(
    commands: &mut Commands,
    ground: Ground,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<GroundMaterial>,
) -> Vec<Entity> {
    let material = GroundMaterial {
        atlas: images.add(ground.image),
        cells: images.add(ground.cells),
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
    use opensacred_data::world::{Cell, Layer, LayerCell};

    use super::*;

    #[test]
    fn a_low_nibble_of_one_or_two_or_flag_bit_three_blocks_the_way() {
        let cell = |ground, flags| Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [0; 4],
            offsets: [0; 2],
            flags,
            ground,
        };
        let mut cells = vec![cell(0x40, 0); SIDE * SIDE];
        // A grass cell, a building's mass, water, a cell of flag bit 3, a door of nibble 0.
        cells[1] = cell(0xD1, 0x3);
        cells[2] = cell(0x92, 0);
        cells[3] = cell(0x40, 0x8);
        cells[4] = cell(0xD0, 0x4);
        let sector = Sector {
            x: 2,
            y: 3,
            id: 0,
            liquids: [0; 2],
            cells,
            statics: Vec::new(),
            layers: Vec::new(),
            buildings: Vec::new(),
        };
        let mut heights = Heights::default();
        heights.add(&sector);
        let side = i32::try_from(SIDE).unwrap();
        let walkable = |column| heights.walkable((2 * side + column, 3 * side));
        assert_eq!(
            [0, 1, 2, 3, 4].map(walkable),
            [true, false, false, false, true]
        );
        // Outside the loaded sectors.
        assert!(!heights.walkable((0, 0)));
    }

    #[test]
    fn a_liquids_cell_tells_its_bed_and_its_sector_its_liquids() {
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
        // Grass sunk to −10, the water of the first slot and of the second.
        cells[0] = cell(0x40, [-10; 4]);
        cells[1] = cell(0x92, [-10, -5, -10, -10]);
        cells[2] = cell(0xA0, [-20; 4]);
        let sector = Sector {
            x: 2,
            y: 3,
            id: 0,
            liquids: [1, 11],
            cells,
            statics: Vec::new(),
            layers: Vec::new(),
            buildings: Vec::new(),
        };
        let mut heights = Heights::default();
        heights.add(&sector);
        let side = i32::try_from(SIDE).unwrap();
        let bed = |column| heights.liquid_bed((2 * side + column, 3 * side));
        assert_eq!(
            [0, 1, 2].map(bed),
            [None, Some([-10, -5, -10, -10]), Some([-20; 4])]
        );
        assert_eq!(heights.liquids((2, 3)), Some([1, 11]));
        assert_eq!(
            (heights.liquid_bed((0, 0)), heights.liquids((0, 0))),
            (None, None)
        );
    }

    #[test]
    fn an_object_takes_its_cell_which_the_way_goes_round() {
        let cell = Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [0; 4],
            offsets: [0; 2],
            flags: 0,
            ground: 0x40,
        };
        let sector = Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells: vec![cell; SIDE * SIDE],
            statics: Vec::new(),
            layers: Vec::new(),
            buildings: Vec::new(),
        };
        let mut heights = Heights::default();
        heights.add(&sector);
        // A chest at (5, 5) on the ground.
        heights.occupy((5, 5), None);
        assert!(heights.walkable((5, 5)), "its cell may be walked on");
        assert!(
            !heights.walkable_on((5, 5), None),
            "but no creature steps in"
        );
        assert!(heights.walkable_on((5, 6), None));
        let around: Vec<_> = heights
            .neighbours(((5, 4), (None, false)))
            .into_iter()
            .map(|(cell, _)| cell)
            .collect();
        // Not into it, nor past its corner.
        assert!(around.contains(&(6, 4)));
        assert!(!around.contains(&(5, 5)) && !around.contains(&(6, 5)));
        // The way from one side of it to the other goes round it: two steps more.
        let way = crate::paths::path(((5, 4), (None, false)), (5, 6), |place| {
            heights.neighbours(place)
        })
        .unwrap();
        assert_eq!(way.len(), 5);
        assert!(way.iter().all(|&(cell, _)| cell != (5, 5)));
    }

    #[test]
    fn a_sector_given_by_hand_starts_on_the_walkable_cell_nearest_its_centre() {
        let cell = |ground, height| Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [5 * height; 4],
            light: [255; 4],
            heights2: [height; 4],
            offsets: [0; 2],
            flags: 0,
            ground,
        };
        // Rock (nibble 1) but for two cells of grass, (40, 30) at height 8 and (10, 10).
        let mut cells = vec![cell(0xD1, 0); SIDE * SIDE];
        cells[30 * SIDE + 40] = cell(0x40, 8);
        cells[10 * SIDE + 10] = cell(0x40, 0);
        let mut sector = Sector {
            x: 2,
            y: 3,
            id: 0,
            liquids: [0; 2],
            cells,
            statics: Vec::new(),
            layers: Vec::new(),
            buildings: Vec::new(),
        };
        // At the height a creature stands at there, its second heights' (`+0x18`).
        assert_eq!(
            start(&sector),
            iso::ground(2.0 * 64.0 + 40.5, 3.0 * 64.0 + 30.5) + Vec3::Y * second_height(8)
        );
        // No walkable cell: the sector's centre.
        for cell in &mut sector.cells {
            cell.ground = 0xD1;
        }
        assert_eq!(start(&sector), center(&sector));
    }

    #[test]
    fn an_entrance_takes_the_hero_in_and_an_exit_out() {
        let heights = Heights::house();
        let house = Some((0, 0, 1));
        assert!(!heights.walkable((5, 5)) && heights.walkable_on((5, 5), house));
        assert!(!heights.walkable_on((5, 4), house));
        assert!(heights.walkable((5, 6)) && heights.walkable_on((5, 6), house));
        // Onto the door from outside, in; onto the cells in front of it from inside, out.
        assert_eq!(heights.transition((5, 6), None), house);
        assert_eq!(heights.transition((5, 7), house), None);
        assert_eq!(
            heights.neighbours(((5, 6), (None, false))),
            vec![((5, 6), (house, false))]
        );
        assert_eq!(
            heights.neighbours(((5, 7), (house, false))),
            vec![((5, 7), (None, false))]
        );
        // Nowhere else: an exit on the ground, the entrance inside, a cell walkable at both
        // levels.
        assert_eq!(heights.transition((5, 7), None), None);
        assert_eq!(heights.transition((5, 6), house), house);
        assert_eq!(heights.transition((7, 5), None), None);
        assert_eq!(heights.transition((7, 5), house), house);
        for place in [
            ((5, 7), None),
            ((5, 6), house),
            ((7, 5), None),
            ((7, 5), house),
        ] {
            assert!(
                heights
                    .neighbours((place.0, (place.1, false)))
                    .iter()
                    .all(|&(_, (level, _))| level == place.1),
                "{place:?}"
            );
        }
        // The way in and the way out pass the door and the cells in front of it.
        let neighbours = |place| heights.neighbours(place);
        let way = crate::paths::path(((5, 9), (None, false)), (5, 5), neighbours).unwrap();
        assert_eq!(way.last(), Some(&((5, 5), (house, false))));
        let through = way.windows(2).find(|w| w[0].1 != w[1].1).unwrap();
        assert_eq!(through, [((5, 6), (None, false)), ((5, 6), (house, false))]);
        let way = crate::paths::path(((5, 5), (house, false)), (5, 9), neighbours).unwrap();
        assert_eq!(way.last(), Some(&((5, 9), (None, false))));
        let through = way.windows(2).find(|w| w[0].1 != w[1].1).unwrap();
        assert_eq!(through[1].1, (None, false));
        assert_eq!(through[0].0.1, 7);
        // A hero on the mass is inside.
        assert_eq!(heights.level_of((5, 5)), house);
    }

    #[test]
    fn the_stairs_take_the_hero_between_a_buildings_floors() {
        let cell = |ground, flags, offsets| Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [0; 4],
            offsets,
            flags,
            ground,
        };
        // A house of cells 4..=6 on rows 4..=6, its cells pointing to its anchor (4, 4), its door
        // an entrance at (5, 6).
        let mut cells = vec![cell(0x40, 0, [0, 0]); SIDE * SIDE];
        for x in 4..=6_i8 {
            for y in 4..=6_i8 {
                let at = usize::try_from(y).unwrap() * SIDE + usize::try_from(x).unwrap();
                cells[at] = cell(0xD1, 0x1, [4 - x, 4 - y]);
            }
        }
        cells[6 * SIDE + 5] = cell(0xD9, 0x5, [-1, -2]);
        let free = LayerCell {
            heights2: [0; 4],
            offsets: [0; 2],
            flags: 0,
            ground: 0xD0,
        };
        let stair = |ground, offsets| LayerCell {
            ground,
            offsets,
            ..free
        };
        // Floor 1 over the house, a stair up at (6, 5) to (5, 4) of floor 2; floor 2 a cell up
        // and left, 3..=6, a stair down at (5, 4) back to (6, 5), another down two floors at
        // (3, 3).
        let grid = |x0: u32, stairs: &[((u32, u32), LayerCell)]| Layer {
            x: x0,
            y: x0,
            width: 4,
            height: 4,
            cells: (0..4)
                .flat_map(|y| (0..4).map(move |x| (x0 + x, x0 + y)))
                .map(|at| {
                    stairs
                        .iter()
                        .find(|(s, _)| *s == at)
                        .map_or(free, |&(_, c)| c)
                })
                .collect(),
        };
        let sector = Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells,
            statics: Vec::new(),
            layers: vec![
                grid(4, &[((6, 5), stair(0xD5, [-1, -1]))]),
                grid(
                    3,
                    &[((5, 4), stair(0xD6, [1, 1])), ((3, 3), stair(0xD8, [0, 0]))],
                ),
            ],
            buildings: vec![opensacred_data::world::Building {
                cell: u16::try_from(4 * SIDE + 4).unwrap(),
                floors: vec![1, 2],
                heights: vec![0, 112],
                open: false,
            }],
        };
        let mut heights = Heights::default();
        heights.add(&sector);
        let (first, second) = (Some((0, 0, 1)), Some((0, 0, 2)));
        // In at the door onto floor 1; the floors of the building at (4, 4).
        assert_eq!(heights.transition((5, 6), None), first);
        assert_eq!(heights.level_of((5, 5)), first);
        assert_eq!(heights.floor(second), Some(((4, 4), 2)));
        // Floor 2 112 units of the models' height up, 192 pixels on the original's screen; the
        // ground and floor 1 at the ground's.
        let raised = iso::height(192.32) - heights.lift(second);
        assert!(raised.abs() < 0.1, "{raised}");
        assert_eq!(heights.lift(first), 0.0);
        assert_eq!(heights.lift(None), 0.0);
        assert_eq!(heights.floor_level((4, 4), 2), second);
        assert_eq!(heights.floor_level((4, 4), 3), None);
        // Up the stair to (5, 4) of floor 2, down again to (6, 5); two floors down, the ground.
        assert_eq!(heights.climb((6, 5), first), Some(((5, 4), second)));
        assert_eq!(heights.climb((5, 4), second), Some(((6, 5), first)));
        assert_eq!(heights.climb((3, 3), second), Some(((3, 3), None)));
        assert_eq!(heights.climb((5, 5), first), None);
        // A step onto a stair from off the stairs climbs; from a stair, or standing on one, not.
        assert!(
            heights
                .neighbours(((6, 4), (first, false)))
                .contains(&((6, 5), (first, true)))
        );
        assert_eq!(
            heights.neighbours(((6, 5), (first, true))),
            vec![((5, 4), (second, false))]
        );
        assert!(
            heights
                .neighbours(((5, 4), (second, false)))
                .iter()
                .all(|&(_, footing)| footing == (second, false))
        );
        // The way from floor 1 to a cell of floor 2 alone goes up the stair.
        let way = crate::paths::path(((4, 5), (first, false)), (3, 4), |place| {
            heights.neighbours(place)
        })
        .unwrap();
        assert_eq!(way.last(), Some(&((3, 4), (second, false))));
        let up = way.windows(2).find(|w| w[0].1.0 != w[1].1.0).unwrap();
        assert_eq!(up, [((6, 5), (first, true)), ((5, 4), (second, false))]);
        // An object of floor 2 off the house's cells on the ground belongs to the house.
        assert_eq!(heights.building_on_floor((3, 3), 2), Some((4, 4)));
        assert_eq!(heights.building_on_floor((3, 3), 1), None);
    }

    /// A cell of a sector of the tests.
    fn cell(ground: u8, flags: u8, offsets: [i8; 2]) -> Cell {
        Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [0; 4],
            offsets,
            flags,
            ground,
        }
    }

    /// A house of cells 4..=10 on rows 4..=10, its cells pointing to its anchor (4, 4), its door an
    /// entrance at (7, 10) where `door`.
    fn house(door: bool) -> Sector {
        let mut cells = vec![cell(0x40, 0, [0, 0]); SIDE * SIDE];
        for x in 4..=10_i8 {
            for y in 4..=10_i8 {
                let at = usize::try_from(y).unwrap() * SIDE + usize::try_from(x).unwrap();
                cells[at] = cell(0xD1, 0x1, [4 - x, 4 - y]);
            }
        }
        if door {
            cells[10 * SIDE + 7] = cell(0xD9, 0x5, [-3, -6]);
        }
        // Floor 1 over the whole mass and the row in front of it: its rooms (6..=8) within
        // walls (rows and columns 5 and 9) opened at (7, 9) onto the door, which walls flank,
        // and the exits in front of the door; about the walls, the empty cells of the grid,
        // all zeros, walkable but reached from no room (#458).
        let inside = |x: u32, y: u32| LayerCell {
            heights2: [0; 4],
            offsets: [0; 2],
            flags: if (x, y) == (7, 10) { 4 } else { 0 },
            ground: match (x, y) {
                (7, 10) => 0xD9,
                (6..=8, 11) => 0xDA,
                (7, 9) | (6..=8, 6..=8) => 0xD0,
                (6 | 8, 10) | (5 | 9, 5..=9) | (5..=9, 5 | 9) => 0xD1,
                _ => 0,
            },
        };
        Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells,
            statics: Vec::new(),
            layers: vec![Layer {
                x: 4,
                y: 4,
                width: 7,
                height: 8,
                cells: (4..=11)
                    .flat_map(|y| (4..=10).map(move |x| inside(x, y)))
                    .collect(),
            }],
            buildings: vec![opensacred_data::world::Building {
                cell: u16::try_from(4 * SIDE + 4).unwrap(),
                floors: vec![1],
                heights: vec![0],
                open: false,
            }],
        }
    }

    #[test]
    fn a_cell_tells_its_ground_its_floors_and_its_pocket() {
        let mut heights = Heights::default();
        heights.add(&house(true));
        let first = (0, 0, 1);
        // The door: an entrance of the house on the ground, an entrance on floor 1 too, in its
        // room.
        let door = heights.about((7, 10)).unwrap();
        assert_eq!((door.ground, door.flags), (0xD9, 0x5));
        assert!(door.walkable && door.entrance && !door.closed);
        assert_eq!(door.building, Some((4, 4)));
        assert_eq!(door.inside.len(), 1);
        let inside = &door.inside[0];
        assert_eq!(inside.level, first);
        assert_eq!(inside.floor, Some(((4, 4), 1)));
        assert!(inside.walkable && inside.room && !inside.exit);
        // A room's cell: the mass on the ground; a wall's, walkable on no level; an empty cell
        // behind the walls, walkable on floor 1 but in no room.
        let room = heights.about((7, 7)).unwrap();
        assert!(!room.walkable && room.inside[0].walkable && room.inside[0].room);
        let wall = heights.about((5, 7)).unwrap();
        assert!(!wall.walkable && !wall.inside[0].walkable);
        let behind = heights.about((4, 6)).unwrap();
        assert!(behind.inside[0].walkable && !behind.inside[0].room);
        // The exits in front of the door; a cell off the house, which no layer holds.
        assert!(heights.about((7, 11)).unwrap().inside[0].exit);
        assert!(heights.about((1, 1)).unwrap().inside.is_empty());
        // A closed trigger's cell; outside the loaded sectors.
        heights.occupy((1, 1), None);
        assert!(heights.about((1, 1)).unwrap().closed);
        assert_eq!(heights.about((64, 0)), None);
        // A cell of the mass on the ground is a pocket of one place; the empty strip behind the
        // walls of floor 1 leads out by the exits in front of the door; the ground walks on.
        assert_eq!(heights.pocket((7, 7), None), Some(1));
        assert_eq!(heights.pocket((4, 6), Some(first)), None);
        assert_eq!(heights.pocket((1, 2), None), None);
    }

    #[test]
    fn a_command_lands_the_hero_on_the_ground_or_in_a_room_else_on_the_ground_nearest() {
        let mut heights = Heights::default();
        heights.add(&house(true));
        let first = Some((0, 0, 1));
        // On the ground where it may be walked on, the door's cell among them.
        assert_eq!(heights.landing((1, 1)), Some(Landing::On(None)));
        assert_eq!(heights.landing((7, 10)), Some(Landing::On(None)));
        // In the rooms, reached from the door.
        assert_eq!(heights.landing((7, 7)), Some(Landing::On(first)));
        assert_eq!(heights.landing((7, 9)), Some(Landing::On(first)));
        // The empty cells behind the walls, which floor 1's grid holds walkable, as the client
        // would put a hero that starts there: on the ground nearest them instead.
        assert_eq!(heights.level_of((4, 6)), first);
        assert_eq!(heights.landing((4, 6)), Some(Landing::Near((3, 6))));
        assert_eq!(heights.landing((9, 10)), Some(Landing::Near((9, 11))));
        // A wall's cell, which no floor holds walkable.
        assert_eq!(heights.landing((5, 7)), Some(Landing::Near((3, 7))));
        // A closed door bars no room; a closed trigger's cell of the ground is not stood on.
        heights.occupy((7, 10), None);
        heights.occupy((7, 10), first);
        heights.occupy((3, 6), None);
        assert_eq!(heights.landing((7, 7)), Some(Landing::On(first)));
        assert_eq!(heights.landing((4, 6)), Some(Landing::Near((3, 5))));
        // A house without an entrance has no room to land in.
        let mut heights = Heights::default();
        heights.add(&house(false));
        assert_eq!(heights.level_of((7, 7)), first);
        assert_eq!(heights.landing((7, 7)), Some(Landing::Near((7, 3))));
        // Nor does a pocket of the ground within a wall, cells 21..=23 on rows 21..=23: the
        // hero lands on the ground beyond it, out of which it walks.
        let mut sector = house(true);
        for x in 20..=24 {
            for y in 20..=24 {
                if [x, y].iter().any(|&v| v == 20 || v == 24) {
                    sector.cells[y * SIDE + x] = cell(0x42, 0, [0, 0]);
                }
            }
        }
        let mut heights = Heights::default();
        heights.add(&sector);
        assert!(heights.walkable((22, 22)));
        assert_eq!(heights.landing((22, 22)), Some(Landing::Near((22, 19))));
        assert_eq!(heights.landing((21, 21)), Some(Landing::Near((21, 19))));
        assert_eq!(heights.landing((25, 22)), Some(Landing::On(None)));
        // Nothing to stand on within reach.
        let mut heights = Heights::default();
        heights.add(&Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells: vec![cell(0x92, 0, [0, 0]); SIDE * SIDE],
            statics: Vec::new(),
            layers: Vec::new(),
            buildings: Vec::new(),
        });
        assert_eq!(heights.landing((5, 5)), None);
    }

    #[test]
    fn the_mesh_maps_the_cells_to_the_flat_screen_of_their_sector() {
        let cell = Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [0; 4],
            offsets: [0; 2],
            flags: 0,
            ground: 0,
        };
        let sector = Sector {
            x: 3,
            y: 5,
            id: 0,
            liquids: [0; 2],
            cells: vec![cell; SIDE * SIDE],
            statics: Vec::new(),
            layers: Vec::new(),
            buildings: Vec::new(),
        };
        let mesh = mesh(&sector);
        let Some(bevy::mesh::VertexAttributeValues::Float32x2(uvs)) =
            mesh.attribute(Mesh::ATTRIBUTE_UV_0)
        else {
            panic!("no texture coordinates");
        };
        // Cell (row 2, column 5): its corners left, top, right, bottom, whatever the sector.
        let first = 4 * (2 * SIDE + 5);
        let top = [48.0 * 3.0, 24.0 * 7.0];
        assert_eq!(uvs[first + 1], top);
        assert_eq!(uvs[first], [top[0] - 48.0, top[1] + 24.0]);
        assert_eq!(uvs[first + 2], [top[0] + 48.0, top[1] + 24.0]);
        assert_eq!(uvs[first + 3], [top[0], top[1] + 48.0]);
    }

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
            layers: Vec::new(),
            buildings: Vec::new(),
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

    #[test]
    fn a_creature_stands_on_the_second_heights_on_the_triangles_of_its_cell() {
        let cell = |heights2| Cell {
            tile: 0,
            overlays: Vec::new(),
            // A pond's bed, which the creatures do not stand on.
            heights: [-10; 4],
            light: [255; 4],
            heights2,
            offsets: [0; 2],
            flags: 0,
            ground: 0x90,
        };
        let mut cells = vec![cell([0; 4]); SIDE * SIDE];
        // Left, top, right, bottom of cell (row 0, column 1): its centre at their mean, 3.
        cells[1] = cell([4, 0, 8, 0]);
        // A grid of one cell over it, 10 steps up, floor 1 of a building raised 28 units.
        let sector = Sector {
            x: 2,
            y: 3,
            id: 0,
            liquids: [0; 2],
            cells,
            statics: Vec::new(),
            layers: vec![Layer {
                x: 2 * 64 + 1,
                y: 3 * 64,
                width: 1,
                height: 1,
                cells: vec![LayerCell {
                    heights2: [10; 4],
                    offsets: [0; 2],
                    flags: 0,
                    ground: 0xD0,
                }],
            }],
            buildings: vec![opensacred_data::world::Building {
                cell: 1,
                floors: vec![1],
                heights: vec![28],
                open: false,
            }],
        };
        let mut heights = Heights::default();
        heights.add(&sector);
        let at = |cx: f32, cy: f32, level| {
            let p = iso::ground(2.0 * SIDE as f32 + cx, 3.0 * SIDE as f32 + cy);
            heights.standing(p.x, p.z, level).unwrap()
        };
        let steps = |value: f32| value * second_height(1);
        let close = |a: f32, b: f32| (a - b).abs() < 0.05;
        // The corners and the centre.
        assert!(close(at(1.0, 0.0, None), 0.0));
        assert!(close(at(1.5, 0.5, None), steps(3.0)));
        assert!(close(at(1.999, 0.001, None), steps(8.0)));
        assert!(close(at(1.001, 0.999, None), steps(4.0)));
        // One point in each triangle, off its medians, where a bilinear surface differs: right
        // and bottom, right and top, left and top, left and bottom.
        assert!(close(at(1.75, 0.375, None), steps(4.5)));
        assert!(close(at(1.625, 0.125, None), steps(4.75)));
        assert!(close(at(1.125, 0.375, None), steps(1.75)));
        assert!(close(at(1.375, 0.875, None), steps(2.75)));
        // A unit of the second heights is 2.5 units of the models' height.
        assert!(close(
            second_height(4),
            iso::height(10.0 * iso::PIXELS_PER_MODEL_HEIGHT)
        ));
        // On the layer: its cell's heights and the floor's over them; beside its grid, the
        // ground's cell's heights and the floor's.
        let floor = Some((2, 3, 1));
        let lift = iso::height(28.0 * iso::PIXELS_PER_MODEL_HEIGHT);
        assert!(close(heights.lift(floor), lift));
        assert!(close(at(1.5, 0.5, floor), steps(10.0) + lift));
        assert!(close(at(2.5, 0.5, floor), lift));
        // Outside the loaded sectors.
        assert!(heights.standing(-1.0, 0.0, None).is_none());
    }

    #[test]
    fn a_step_on_a_bridge_over_a_liquid_sounds_as_on_a_bridge() {
        let cell = |ground, heights, heights2| Cell {
            tile: 0,
            overlays: Vec::new(),
            heights,
            light: [255; 4],
            heights2,
            offsets: [0; 2],
            flags: 0,
            ground,
        };
        let bed = [-10; 4];
        let mut cells = vec![cell(0x40, [0; 4], [0; 4]); SIDE * SIDE];
        // Water under a deck; under one at the left corner alone; under its own heights; water
        // a little below the surface; water B under a deck; grass raised.
        cells[1] = cell(0x90, bed, [3, 3, 6, 6]);
        cells[2] = cell(0x90, bed, [5, 0, 0, 0]);
        cells[3] = cell(0x90, [5; 4], [5; 4]);
        cells[4] = cell(0x90, bed, [0, -1, -1, 0]);
        cells[5] = cell(0xA0, bed, [0, 0, 0, 2]);
        cells[6] = cell(0x40, [10; 4], [2; 4]);
        let sector = Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells,
            statics: Vec::new(),
            layers: Vec::new(),
            buildings: Vec::new(),
        };
        let mut heights = Heights::default();
        heights.add(&sector);
        let nature = |column: f32| {
            let p = iso::ground(column + 0.5, 0.5);
            heights.step_nature(p.x, p.z).unwrap()
        };
        assert_eq!(
            [1.0, 2.0, 3.0, 4.0, 5.0, 6.0].map(nature),
            [BRIDGE, 9, 9, 9, BRIDGE, 4]
        );
        // The ambient loop takes the cell's own nature.
        let p = iso::ground(1.5, 0.5);
        assert_eq!(heights.nature(p.x, p.z), Some(9));
    }

    #[test]
    fn a_slot_composes_along_the_diamonds_rows_as_a_test_of_every_pixel_does() {
        let sheet = |width: usize, height: usize, seed: usize| texture::Rgba {
            width,
            height,
            pixels: (0..4 * width * height)
                .map(|i| u8::try_from((i * 7 + seed * 13) % 251).unwrap())
                .collect(),
        };
        let world = World {
            sectors: Vec::new(),
            sheets: Vec::new(),
            atlases: Vec::new(),
            assembled_from: Vec::new(),
        };
        // The second sheet narrower than its diamonds' boxes reach.
        let mut sheets = Sheets {
            dir: Path::new(""),
            world: &world,
            loaded: HashMap::from([(0, sheet(256, 256, 1)), (1, sheet(150, 100, 2))]),
        };
        // A ground tile, an overlay through another tile's mask, and one through its own alpha
        // partly beyond its sheet.
        let layers: Layers = vec![(3, None), (6, Some(DIAMONDS + 2)), (DIAMONDS + 5, None)];
        let mut slot = vec![[0u8; 4]; SLOT_W * SLOT_H];
        compose_slot(&layers, &mut sheets, &spans(), &mut slot).unwrap();

        let pixel = |tile: u32, x: usize, y: usize| {
            let sheet = &sheets.loaded[&(tile / DIAMONDS)];
            let (ox, oy) = diamond_origin(tile % DIAMONDS);
            let (sx, sy) = (ox + x, oy + y);
            if sx >= sheet.width || sy >= sheet.height {
                return [0; 4];
            }
            let at = 4 * (sy * sheet.width + sx);
            [0, 1, 2, 3].map(|k| sheet.pixels[at + k])
        };
        let mut expected = vec![[0u8; 4]; SLOT_W * SLOT_H];
        for &(art, mask) in &layers {
            for y in 0..DIAMOND_H {
                for x in (0..DIAMOND_W).filter(|&x| inside(x, y)) {
                    let colour = pixel(art, x, y);
                    let a = u16::from(mask.map_or(colour[3], |mask| pixel(mask, x, y)[3]));
                    let out = &mut expected[(y + 1) * SLOT_W + x + 1];
                    for k in 0..3 {
                        let blended =
                            (u16::from(colour[k]) * a + u16::from(out[k]) * (255 - a)) / 255;
                        out[k] = u8::try_from(blended).unwrap();
                    }
                    out[3] = 255;
                }
            }
        }
        for y in 0..DIAMOND_H {
            let row = (y + 1) * SLOT_W + 1;
            let inner: Vec<usize> = (0..DIAMOND_W).filter(|&x| inside(x, y)).collect();
            for &x in &inner {
                assert_eq!(slot[row + x], expected[row + x], "pixel {x},{y}");
            }
            // Outside the diamond, each row repeats its edge.
            assert_eq!(slot[row], slot[row + inner[0]]);
            assert_eq!(
                slot[row + DIAMOND_W - 1],
                slot[row + inner[inner.len() - 1]]
            );
        }
    }
}
