//! The converted world (docs/02 §5, §12.1): an index and one file per sector.
//!
//! `world.ron` holds the [`World`] index: the sectors of the 100 × 100 grid, the ground tile
//! sheets and the atlases of the miniatures. `sectors/<x>-<y>.sector` holds a [`Sector`] and
//! `sprites.bin` the [`Sprites`] of the static objects, each postcard-encoded in a zlib stream
//! ([`Sector::encode`], [`Sprites::encode`]); `sprites/<record>` holds each sprite's image,
//! assembled from its pieces by the converter ([`sprite_image`], #838). A tile is the original
//! `tiles.pak` index: tile `t` is diamond `t % 18` (docs/02 §5.4) of sheet `t / 18`.

use serde::{Deserialize, Serialize};

use crate::packed::{DecodeError, decode, encode};

/// Version of the sector and sprite encodings; a file of another version does not load.
pub const SECTOR_VERSION: u32 = 9;
/// The index file under the world directory.
pub const INDEX: &str = "world.ron";
/// The sprite table under the world directory.
pub const SPRITES_FILE: &str = "sprites.bin";
/// The directory of the sprites' images under the world directory.
pub const SPRITES_DIR: &str = "sprites";
/// The levels of a sprite's image in the game's assets, the image first (#839): the game packs
/// them into its pages' levels and takes the one that the zoom and the window's height call for.
pub const SPRITE_LEVELS: usize = 4;
/// The multiple of texels to which a sprite's image is repeated at its right and bottom edges,
/// so that each level halves the one before whole (`2^(SPRITE_LEVELS − 1)`); the game packs the
/// images at its multiples, so that an image's rectangle at level `n` is its rectangle shifted
/// right by `n` bits.
pub const SPRITE_ALIGN: usize = 1 << (SPRITE_LEVELS - 1);
/// Cells per side of a sector.
pub const SIDE: usize = 64;
/// Diamonds per tile sheet.
pub const DIAMONDS: u32 = 18;

/// The world index, `world.ron`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct World {
    pub sectors: Vec<SectorEntry>,
    /// Image of every tile sheet, relative to `world.ron`: sheet `t / 18` of tile `t`.
    pub sheets: Vec<String>,
    /// Image of every atlas that miniatures cut their squares from, relative to `world.ron`;
    /// [`Miniature::atlas`] indexes it.
    pub atlases: Vec<String>,
    /// The atlases that the sprites' images were assembled from, relative to `world.ron`, in the
    /// masters' index alone: the converter's `build` leaves out of the game's assets those that
    /// [`World::atlases`] does not name (#838).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assembled_from: Vec<String>,
}

/// A sector of the grid; 3,950 of the 10,000 positions have none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
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
    /// The sector's weather, its entries in order (docs/re/weather.md), none for most sectors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weather: Vec<SectorWeather>,
    /// The sector's nature (`sectors.keyx +0x1D8`, a cell's nature byte): crows (`0x10`) or bats
    /// (`0x80`) fly over the camera in the sector by it, any other value but 0 running their timer
    /// alone (docs/re/weather.md), and the ambient loop is chosen by it (docs/re/sound.md); named
    /// `fliers` in a `world.ron` written before #1093.
    #[serde(default, alias = "fliers", skip_serializing_if = "is_zero")]
    pub nature: u8,
    /// How far what the sector draws reaches past its cells' diamonds on the original's screen at
    /// zoom 1, in pixels up, down, left and right ([`Sector::reach`], #1656): by which the game
    /// tells the sectors that a view shows. `None` in a `world.ron` written before it, which the
    /// game takes for the most that any sector reaches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reach: Option<[u16; 4]>,
}

/// Whether `byte` is 0, which `world.ron` leaves out.
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's `skip_serializing_if` passes a reference"
)]
fn is_zero(byte: &u8) -> bool {
    *byte == 0
}

/// A kind of a sector's weather (`sectors.keyx +0x1EE`'s types 0, 1, 2 and 5; docs/02 §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub enum WeatherKind {
    Rain,
    Fog,
    Snow,
    Butterflies,
}

/// An entry of a sector's weather: its kind, the shortest and the longest time off and on, in
/// seconds, and the least and the most intensity (docs/re/weather.md, "How the weather is
/// chosen").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct SectorWeather {
    pub kind: WeatherKind,
    pub off: [u32; 2],
    pub on: [u32; 2],
    pub intensity: [u8; 2],
}

/// A sector: 64 × 64 cells and the static objects placed on them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
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

/// A building (docs/re/movement.md, "Changing layers"): the first static of flag `0x10` in its
/// anchor cell's chain, to which the offsets of its cells point; its children are its floors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
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
    /// Whether the building is open to the sky, its object's flag `0x400`: the castles' walls
    /// and towers, whose floors' objects the client draws whatever the building's state
    /// (docs/re/drawing.md, "Buildings: masks and layers").
    pub open: bool,
}

/// An inside layer: a grid of cells of a building's own, from its first cell's global position
/// (docs/02 §5.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
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
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct LayerCell {
    /// The second heights (`+0x18`), in the inside layers 10, 15, 20, …: the heights a creature
    /// stands at inside, as [`Cell::heights2`].
    pub heights2: [i8; 4],
    /// Offsets of the parent object.
    pub offsets: [i8; 2],
    /// 0, or 4: a door.
    pub flags: u8,
    /// The ground byte: its low nibble 1 or 2 makes the cell unwalkable, and 3 and 5 to 10 change
    /// the layer of what steps onto it, as [`Cell::ground`]'s.
    pub ground: u8,
}

/// A floor cell: a ground tile with overlays, heights and light at its four corners (left, top,
/// right, bottom).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
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
    /// The offsets to the cell of the building's anchor, whose chain holds the building's object
    /// (docs/02 §5.2).
    pub offsets: [i8; 2],
    /// Bit 0 structure, bit 1 a settlement's ground, bit 2 door, a cell walked by its trigger's state, bit
    /// 3 unwalkable (docs/02 §5.2).
    pub flags: u8,
    /// High nibble the nature (4 `GRASS`, 9 `WATERA`, 10 `WATERB`, 11 `STONEROAD`, …), low nibble
    /// 1 or 2 unwalkable, 3 and 5 to 10 a change of layer for what steps onto the cell (docs/02
    /// §5.2; docs/re/movement.md, "Which cells may be walked on", "Changing layers").
    pub ground: u8,
}

/// A floor overlay: the art tile's colour through the alpha of the mask tile, or of the art tile
/// itself without one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Overlay {
    pub art: u32,
    pub mask: Option<u32>,
}

/// A static object: a sprite or a model placed in the world.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
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
    // None in the tests' random objects, which a type of its own cannot nest (#1434); the
    // sector's `statics` take random objects.
    #[cfg_attr(test, proptest(value = "Vec::new()"))]
    pub children: Vec<StaticObject>,
}

/// A static's shadow, its type's (`items.pak +0x5B`…`+0x65`, docs/02 §6.1): a quad of a cell of
/// the shadow atlas `SHADOW_TREE00` (16 × 16 cells of 32 × 32 texels) on the ground at the
/// static's foot (docs/re/drawing.md, "The statics' shadows").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
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
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Miniature {
    /// Index into [`World::atlases`].
    pub atlas: u32,
    /// Without flag `0x40`: x, y and size of the square in pixels; with it columns, rows,
    /// rotation (1/256 turn), timing divisor and frames.
    pub params: [u8; 5],
}

/// The sprites of the static objects: `mixed.pak` record `i` is `sprites[i]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Sprites {
    pub sprites: Vec<Option<Sprite>>,
}

/// A sprite, drawn with its top left corner at an object's position: its image, `sprites/<record>`
/// ([`sprite_image`]), covers the rectangle `rect` of it, which the converter assembled from the
/// sprite's pieces of `mixed.pak` (docs/02 §3.3, #838).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Sprite {
    pub width: u16,
    pub height: u16,
    /// `x0, y0, x1, y1` of the image in the sprite, pixels: from the pieces' smallest corner,
    /// cut to the sprite's width and height.
    pub rect: [u16; 4],
}

/// The image of sprite `record` (`mixed.pak`'s record) under the world directory, of the
/// extension `extension`: `png` among the masters, `ktx2` in the game's assets.
pub fn sprite_image(record: u32, extension: &str) -> String {
    format!("{SPRITES_DIR}/{record:05}.{extension}")
}

/// The flags of the static objects that the game does not draw with the world's art
/// (`0x62AE90`), and of a miniature, which it cuts from its type's atlas.
const NOT_DRAWN: u32 = 0x290;
const MINIATURE: u32 = 0x20;
/// The side of a miniatures' atlas in pixels, which an animated one's columns and rows divide.
const MINIATURE_ATLAS: i64 = 256;

impl Sector {
    /// How far what the sector draws reaches past its cells' diamonds on the original's screen at
    /// zoom 1, in pixels up, down, left and right (#1656): each drawn static object's sprite
    /// from its chain cell's centre, `48 (cx − cy)`, `24 (cx + cy + 1)` for the global cell
    /// `(cx, cy)`, its sprite's rectangle at the object's position; a miniature's square, turned
    /// about the position, and a lamp's glow of three times its side about it, within twice its
    /// side's diagonal of the position; and the ground's corners raised or lowered by their
    /// heights. Of the 6,050 sectors the median's sprites reach 391 pixels up, the farthest's
    /// 2,034.
    pub fn reach(&self, sprites: &Sprites) -> [u16; 4] {
        let side = i64::try_from(SIDE).unwrap_or(64);
        let mut most = [0i64; 4];
        for &height in self.cells.iter().flat_map(|cell| &cell.heights) {
            most[0] = most[0].max(i64::from(height));
            most[1] = most[1].max(-i64::from(height));
        }
        let mut stack: Vec<&StaticObject> = self.statics.iter().collect();
        while let Some(object) = stack.pop() {
            stack.extend(&object.children);
            if object.flags & NOT_DRAWN != 0 {
                continue;
            }
            let (row, column) = (i64::from(object.cell) / side, i64::from(object.cell) % side);
            let cx = i64::from(self.x) * side + column;
            let cy = i64::from(self.y) * side + row;
            let (centre_x, centre_y) = (48 * (cx - cy), 24 * (cx + cy + 1));
            let (x, y) = (i64::from(object.x), i64::from(object.y));
            let [x0, y0, x1, y1] = if object.flags & MINIATURE != 0 {
                let Some(miniature) = object.miniature else {
                    continue;
                };
                let [a, b, c, _, _] = miniature.params.map(i64::from);
                let side = if a > 0 && b > 0 {
                    (MINIATURE_ATLAS / a).max(MINIATURE_ATLAS / b)
                } else {
                    c
                };
                // Twice the diagonal, rounded up.
                let around = side * 29 / 10;
                [x - around, y - around, x + around, y + around]
            } else {
                let Some(Some(sprite)) = usize::try_from(object.sprite)
                    .ok()
                    .and_then(|index| sprites.sprites.get(index))
                else {
                    continue;
                };
                let [x0, y0, x1, y1] = sprite.rect.map(i64::from);
                [x + x0, y + y0, x + x1, y + y1]
            };
            let reach = [centre_y - y0, y1 - centre_y, centre_x - x0, x1 - centre_x];
            for (most, reach) in most.iter_mut().zip(reach) {
                *most = (*most).max(reach);
            }
        }
        most.map(|pixels| u16::try_from(pixels.max(0)).unwrap_or(u16::MAX))
    }

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
                open: true,
            }],
        }
    }

    #[test]
    fn a_sectors_reach_is_its_sprites_and_its_heights_past_its_cells() {
        let mut sector = sector();
        // The cell 65 of the sector at 50,39, the global cell 3201,2497: its centre on screen.
        let (cx, cy) = (3201, 2497);
        let (x, y) = (48 * (cx - cy), 24 * (cx + cy + 1));
        let object = |flags, (x, y), sprite, miniature| StaticObject {
            cell: 65,
            kind: 1,
            flags,
            x,
            y,
            record: 1,
            type_flags: 9,
            mask: 0,
            sprite,
            miniature,
            shadow: None,
            floor: None,
            children: Vec::new(),
        };
        // A sprite of 80 × 320 from 40 pixels left of the centre and 300 above it, with a
        // miniature of 16 at the centre (twice its diagonal about it), and a hidden object's
        // sprite, which counts nothing.
        let mut tall = object(0, (x - 40, y - 300), 1, None);
        tall.children.push(object(
            0x20,
            (x, y),
            0,
            Some(Miniature {
                atlas: 0,
                params: [0, 0, 16, 0, 0],
            }),
        ));
        sector.statics = vec![tall, object(0x10, (x - 5_000, y - 5_000), 2, None)];
        let sprite = |side| {
            Some(Sprite {
                width: side,
                height: 4 * side,
                rect: [0, 0, side, 4 * side],
            })
        };
        let sprites = Sprites {
            sprites: vec![None, sprite(80), sprite(10_000)],
        };
        // Up by the sprite, down past the corners' lowest of −5 by the miniature, as left and
        // right.
        assert_eq!(sector.reach(&sprites), [300, 46, 46, 46]);
        // The corners' highest, 125, without the objects.
        sector.statics.clear();
        assert_eq!(sector.reach(&sprites), [125, 5, 0, 0]);
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
                    rect: [0, 0, 43, 70],
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

    #[test]
    fn a_sectors_nature_reads_by_its_old_name_too() {
        let entry = |field: &str| {
            let text =
                format!(r#"(x: 53, y: 45, id: 7, file: "sectors/53-45.sector", {field}: 48)"#);
            ron::from_str::<SectorEntry>(&text).unwrap().nature
        };
        assert_eq!((entry("nature"), entry("fliers")), (0x30, 0x30));
    }
}
