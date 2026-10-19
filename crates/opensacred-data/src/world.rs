//! The converted world (docs/02 §5, §12.1): an index and one file per sector.
//!
//! `world.ron` holds the [`World`] index: the sectors of the 100 × 100 grid, the ground tile
//! sheets and the atlases of the static objects. `sectors/<x>-<y>.sector` holds a [`Sector`] and
//! `sprites.bin` the [`Sprites`] of the static objects, each postcard-encoded in a zlib stream
//! ([`Sector::encode`], [`Sprites::encode`]). A tile is the original `tiles.pak` index: tile `t` is
//! diamond `t % 18` (docs/02 §5.4) of sheet `t / 18`.

use serde::{Deserialize, Serialize};

use crate::packed::{DecodeError, decode, encode};

/// Version of the sector and sprite encodings; a file of another version does not load.
pub const SECTOR_VERSION: u32 = 7;
/// The index file under the world directory.
pub const INDEX: &str = "world.ron";
/// The sprite table under the world directory.
pub const SPRITES_FILE: &str = "sprites.bin";
/// Cells per side of a sector.
pub const SIDE: usize = 64;
/// Diamonds per tile sheet.
pub const DIAMONDS: u32 = 18;

/// The world index, `world.ron`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct World {
    pub sectors: Vec<SectorEntry>,
    /// Image of every tile sheet, relative to `world.ron`: sheet `t / 18` of tile `t`.
    pub sheets: Vec<String>,
    /// Image of every atlas that sprites and miniatures cut pieces from, relative to
    /// `world.ron`; [`Piece::atlas`] and [`Miniature::atlas`] index it.
    pub atlases: Vec<String>,
}

/// A sector of the grid; 3,950 of the 10,000 positions have none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectorEntry {
    pub x: u32,
    pub y: u32,
    /// The original sector id, which scripts and static objects name.
    pub id: u32,
    /// The file, relative to `world.ron`.
    pub file: String,
    /// The sound id of the sector's music, 0 for none (docs/re/sound.md).
    #[serde(default)]
    pub music: u32,
    /// The sector's area, which the client turns into a region (docs/re/sound.md).
    #[serde(default)]
    pub area: u8,
    /// Whether the sector is a dungeon, lit by a grey ramp at any hour (docs/re/daynight.md).
    #[serde(default)]
    pub dungeon: bool,
}

/// A sector: 64 × 64 cells and the static objects placed on them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sector {
    pub x: u32,
    pub y: u32,
    pub id: u32,
    /// Liquid materials (0..=13) of the `WATERA` and of the `WATERB` cells (docs/02 §5.2).
    pub liquids: [u8; 2],
    /// 4,096 cells, row by row: cell `r · 64 + c`.
    pub cells: Vec<Cell>,
    /// The objects of the cells' chains, cell by cell, in chain order.
    pub statics: Vec<StaticObject>,
    /// The sector's inside layers, layer 1 first: the buildings' own cells, which the hero walks
    /// inside them (docs/re/movement.md).
    pub layers: Vec<Layer>,
    /// The sector's buildings, by their anchors' cells in order.
    pub buildings: Vec<Building>,
}

/// A building (docs/re/movement.md, "The layers"): the first static of flag `0x10` in its anchor
/// cell's chain, to which the offsets of its cells point; its children are its floors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Building {
    /// The anchor, cell `r · 64 + c` of the sector.
    pub cell: u16,
    /// The floors from the first, the ground floor inside, up: of each, the index from 1 in
    /// [`Sector::layers`] of the inside layer that holds its grid.
    pub floors: Vec<u16>,
    /// Each floor's height above the ground, as [`floors`]: its object's `Static.PAK +0x33`
    /// times 28, in the models' units (0 for floor 1; **hypothesis**: the height the client
    /// raises the floor's creatures to, the height of the plane of its floor's sprites,
    /// docs/re/drawing.md, "Depth").
    ///
    /// [`floors`]: Building::floors
    pub heights: Vec<u16>,
}

/// An inside layer: a grid of cells of a building's own, from its first cell's global position
/// (docs/02 §5.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layer {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// `width · height` cells, row by row.
    pub cells: Vec<LayerCell>,
}

/// A cell of an inside layer: of a cell's fields, those its layer sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerCell {
    /// The second heights (`+0x18`), in the inside layers 10, 15, 20, …: the heights a creature
    /// stands at inside, as [`Cell::heights2`].
    pub heights2: [i8; 4],
    /// Offsets of the parent object.
    pub offsets: [i8; 2],
    /// 0, or 4: a door.
    pub flags: u8,
    /// The ground byte: its low nibble 1 or 2 makes the cell unwalkable.
    pub ground: u8,
}

/// A floor cell: a ground tile with overlays, heights and light at its four corners (left, top,
/// right, bottom).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    pub tile: u32,
    /// Tiles drawn over the ground, in order.
    pub overlays: Vec<Overlay>,
    /// Screen pixels at zoom 1; under a liquid the depth of the bed.
    pub heights: [i8; 4],
    pub light: [u8; 4],
    /// A second height per corner: the height a creature stands at there, in steps of 2.5 units
    /// of the models' height (docs/re/movement.md, "The height").
    pub heights2: [i8; 4],
    /// Offsets of the parent object (community-reported).
    pub offsets: [i8; 2],
    /// Bit 0 structure, bit 1 room interior, bit 2 door (community-reported).
    pub flags: u8,
    /// High nibble the nature (4 `GRASS`, 9 `WATERA`, 10 `WATERB`, 11 `STONEROAD`, …, docs/02
    /// §5.2), low nibble unknown.
    pub ground: u8,
}

/// A floor overlay: the art tile's colour through the alpha of the mask tile, or of the art tile
/// itself without one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Overlay {
    pub art: u32,
    pub mask: Option<u32>,
}

/// A static object: a sprite or a model placed in the world.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticObject {
    /// The cell whose chain holds the object.
    pub cell: u16,
    /// The original type, an `Items.pak` record.
    pub kind: u32,
    pub flags: u32,
    /// Position in isometric screen pixels at zoom 1: `48 (cx − cy)`, `24 (cx + cy)` for the
    /// global cell `(cx, cy)`.
    pub x: i32,
    pub y: i32,
    /// The original record index, which triggers and scripts name.
    pub record: u32,
    /// The type's `items.pak +0x00` as a u32: the low byte its kind (2 creatures and equipment,
    /// 8 and 12 miniatures, 9 world statics, 13 interactive objects, 25 trees and walls, …), the
    /// bits `0x4` and `0x800000` its draw list (docs/02 §6.1); 0 for a type without a record.
    pub type_flags: u32,
    /// `Static.PAK +0x2B`, the mask: 0 or one bit of 1 to 16 (docs/02 §6.1).
    pub mask: u8,
    /// The type's sprite in [`Sprites`], drawn with its top left corner at the position; 0 for
    /// none.
    pub sprite: u32,
    /// For a miniature (flag `0x20`): what it cuts from its type's atlas instead of a sprite.
    pub miniature: Option<Miniature>,
    /// The type's shadow, for a type that casts one (bit `0x10000` of [`type_flags`]).
    ///
    /// [`type_flags`]: StaticObject::type_flags
    pub shadow: Option<Shadow>,
    /// For a type of bit `0x200` of [`type_flags`], an upper storey's floor: the height of the
    /// plane the original draws it in, in the models' units (`Static.PAK +0x33` times 28,
    /// docs/re/drawing.md, "Depth").
    ///
    /// [`type_flags`]: StaticObject::type_flags
    pub floor: Option<u16>,
    /// Objects that belong to this one, in chain order.
    pub children: Vec<StaticObject>,
}

/// A static's shadow, its type's (`items.pak +0x5B`…`+0x65`, docs/02 §6.1): a quad of a cell of
/// the shadow atlas `SHADOW_TREE00` (16 × 16 cells of 32 × 32 texels) on the ground at the
/// static's foot (docs/re/drawing.md, "The statics' shadows").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shadow {
    /// The silhouette's cell, `16 · row + column`.
    pub cell: u16,
    /// The static's foot, from its position (its sprite's top-left corner), in pixels at
    /// zoom 1.
    pub foot: [i32; 2],
    /// The quad's height in pixels at zoom 1; it is twice as wide, its bottom edge on the foot's
    /// line, centred on the foot.
    pub size: u16,
    /// Whether the quad's top edge is shifted right by its height: the shadow leans up and to the
    /// right; otherwise a rectangle.
    pub leaning: bool,
}

/// A miniature: a square of an atlas, still or animated (community-reported, openheilig).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Miniature {
    /// Index into [`World::atlases`].
    pub atlas: u32,
    /// Without flag `0x40`: x, y and size of the square in pixels; with it columns, rows,
    /// rotation (1/256 turn), timing divisor and frames.
    pub params: [u8; 5],
}

/// The sprites of the static objects: `mixed.pak` record `i` is `sprites[i]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sprites {
    pub sprites: Vec<Option<Sprite>>,
}

/// A sprite assembled from pieces of atlases, drawn with its top left corner at an object's
/// position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sprite {
    pub width: u16,
    pub height: u16,
    pub pieces: Vec<Piece>,
}

/// A rectangle of an atlas, copied to a rectangle of the same size in the sprite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Piece {
    /// Index into [`World::atlases`].
    pub atlas: u32,
    /// `x0, y0, x1, y1` in the atlas, pixels.
    pub source: [u16; 4],
    /// `x0, y0, x1, y1` in the sprite, pixels; pieces may run past the sprite's size.
    pub rect: [u16; 4],
}

impl Sector {
    /// The sector file: the version and the sector, postcard-encoded, in a zlib stream.
    pub fn encode(&self) -> Result<Vec<u8>, postcard::Error> {
        encode(SECTOR_VERSION, self)
    }

    /// The sector of a file that [`Sector::encode`] wrote.
    pub fn decode(file: &[u8]) -> Result<Self, DecodeError> {
        decode(SECTOR_VERSION, file)
    }
}

impl Sprites {
    /// The sprite file, encoded as a sector file is.
    pub fn encode(&self) -> Result<Vec<u8>, postcard::Error> {
        encode(SECTOR_VERSION, self)
    }

    pub fn decode(file: &[u8]) -> Result<Self, DecodeError> {
        decode(SECTOR_VERSION, file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sector() -> Sector {
        let cell = |tile| Cell {
            tile,
            overlays: vec![Overlay {
                art: tile + 1,
                mask: (tile % 2 == 0).then_some(36),
            }],
            heights: [0, -5, 10, 125],
            light: [255, 235, 215, 195],
            heights2: [0; 4],
            offsets: [-1, 1],
            flags: 1,
            ground: 0x93,
        };
        Sector {
            x: 50,
            y: 39,
            id: 2579,
            liquids: [0, 4],
            cells: (0..4096).map(cell).collect(),
            statics: vec![StaticObject {
                cell: 65,
                kind: 801,
                flags: 32,
                x: 96,
                y: -48,
                record: 7,
                type_flags: 0x1_0009,
                mask: 1,
                sprite: 812,
                miniature: None,
                shadow: Some(Shadow {
                    cell: 74,
                    foot: [78, 100],
                    size: 80,
                    leaning: false,
                }),
                floor: None,
                children: vec![StaticObject {
                    cell: 65,
                    kind: 1036,
                    flags: 0x20,
                    x: 0,
                    y: 0,
                    record: 8,
                    type_flags: 12,
                    mask: 0,
                    sprite: 0,
                    miniature: Some(Miniature {
                        atlas: 3,
                        params: [64, 0, 64, 0, 0],
                    }),
                    shadow: None,
                    floor: None,
                    children: vec![],
                }],
            }],
            layers: vec![Layer {
                x: 3249,
                y: 2568,
                width: 2,
                height: 1,
                cells: vec![
                    LayerCell {
                        heights2: [15; 4],
                        offsets: [-1, 2],
                        flags: 0,
                        ground: 0xD1,
                    },
                    LayerCell {
                        heights2: [0; 4],
                        offsets: [0, 0],
                        flags: 4,
                        ground: 0xD0,
                    },
                ],
            }],
            buildings: vec![Building {
                cell: 65,
                floors: vec![1],
                heights: vec![0],
            }],
        }
    }

    #[test]
    fn a_sector_round_trips_through_its_file() {
        let sector = sector();
        let file = sector.encode().unwrap();
        assert_eq!(Sector::decode(&file).unwrap(), sector);
    }

    #[test]
    fn sprites_round_trip_through_their_file() {
        let sprites = Sprites {
            sprites: vec![
                None,
                Some(Sprite {
                    width: 105,
                    height: 90,
                    pieces: vec![Piece {
                        atlas: 0,
                        source: [64, 0, 107, 70],
                        rect: [0, 0, 43, 70],
                    }],
                }),
            ],
        };
        assert_eq!(
            Sprites::decode(&sprites.encode().unwrap()).unwrap(),
            sprites
        );
    }

    #[test]
    fn another_version_does_not_load() {
        let mut bytes = postcard::to_allocvec(&(SECTOR_VERSION + 1)).unwrap();
        bytes.extend(postcard::to_allocvec(&sector()).unwrap());
        let file = miniz_oxide::deflate::compress_to_vec_zlib(&bytes, 6);
        assert_eq!(
            Sector::decode(&file),
            Err(DecodeError::Version {
                found: SECTOR_VERSION + 1,
                expected: SECTOR_VERSION
            })
        );
        assert!(matches!(
            Sector::decode(b"not zlib"),
            Err(DecodeError::Inflate(_))
        ));
    }
}
