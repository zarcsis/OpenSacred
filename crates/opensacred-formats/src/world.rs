//! The world (docs/02 §5): `World/sectors.keyx` indexes the sectors of a 100 × 100 grid,
//! `World/sectors.wldx` holds each as a zlib stream of 64 × 64 floor cells; `PAK/tiles.pak` names
//! the ground tiles of the cells, `World/Static.PAK` the static objects and `World/Floor.PAK` the
//! floor overlays that hang off the cells in chains. The layout follows the openheilig
//! description (`formats/world-sectors.md`, CC BY-SA 4.0) and is confirmed on every record by
//! `tools/analysis/world/world_dump.py`.

use crate::error::{ErrorKind, Result, field, to_usize};
use crate::pak::{HEADER_SIZE, Pak};
use crate::reader::Reader;
use crate::zlib::inflate;

/// Sectors per side of the world grid.
pub const GRID: u32 = 100;
/// Cells per side of a sector.
pub const SIDE: usize = 64;
/// Cells of a sector, row by row: cell `r · 64 + c`.
pub const CELLS: usize = SIDE * SIDE;
const KEY_RECORD: usize = 768;
const CELL_SIZE: usize = 32;
/// The sector stream: a 32-byte name, the cells, a third chunk.
const NAME_SIZE: usize = 32;
const CELL_CHUNK: usize = CELL_SIZE * CELLS;
/// MSVC's fill of uninitialized heap memory, which some fields keep.
const FILL: u32 = 0xCDCD_CDCD;
/// A layer's header in the third chunk, and the kind every header has.
const LAYER_HEADER: usize = 36;
const LAYER_KIND: u32 = 6;
pub const TILES_MAGIC: [u8; 3] = *b"ISO";
pub const OBJ_MAGIC: [u8; 3] = *b"OBJ";
/// Directory type of the records of `Static.PAK` and of `Floor.PAK`.
const STATIC_TYPE: u32 = 134;
const FLOOR_TYPE: u32 = 135;

/// `sectors.keyx`: `"WLK"` v5, a PAK-style 0x100-byte header, then one 768-byte record per sector.
#[derive(Debug, Clone)]
pub struct Keyx {
    pub sectors: Vec<SectorKey>,
}

/// Record of `sectors.keyx`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectorKey {
    /// Grid position from `+0x20` (`y · 100 + x`); the name in the stream is not reliable.
    pub x: u32,
    pub y: u32,
    /// `+0x24`: the sector's id, which `Static.PAK` refers to.
    pub id: u32,
    /// `+0xEC`, `+0xF0`: the stream's place in `sectors.wldx`.
    pub offset: u32,
    pub compressed: u32,
    /// `+0x108`: the inflated size.
    pub inflated: u32,
    /// `+0x2E0`, `+0x2E1`: the liquid materials (0..=13) of the sector's `WATERA` cells (nature
    /// 9) and `WATERB` cells (nature 10).
    pub liquids: [u8; 2],
    /// `+0x6C`, the size of the third chunk: the sector's layers, the ground's and the
    /// buildings' insides ([`Layer`]).
    pub layers: u32,
    /// `+0x2C0`: the sector's area (56 values; the client's region music turns it into a region
    /// through a table, docs/re/sound.md).
    pub area: u8,
    /// `+0x2D8`: the sound id of the sector's music (`MUSIC_WOOD01` in the start sector), 0 for
    /// none (docs/re/sound.md).
    pub music: u32,
    /// Whether the sector is a dungeon: of the four entries of 28 bytes from `+0x1EE`, one whose
    /// bit is set in the mask at `+0x1ED` starts with the word 4 (`0x622E20`, docs/re/daynight.md).
    pub dungeon: bool,
}

impl Keyx {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let pak = Pak::parse(data)?;
        let file = Reader::new(data);
        if pak.magic() != *b"WLK" || pak.version() != 5 {
            return Err(file.error(ErrorKind::Magic {
                expected: *b"WLK",
                expected_version: 5,
                found: pak.magic(),
                found_version: pak.version(),
            }));
        }
        // The header's first fields: count, grid width, grid height. `Pak::parse` read the count as
        // a directory length; the keyx records take its place.
        let &[count, width, height, ..] = pak.header_fields();
        if (width, height) != (GRID, GRID) {
            return Err(file.error_at(
                8,
                field("grid size", width.into(), format!("{GRID} x {GRID}")),
            ));
        }
        let mut sectors = Vec::with_capacity(to_usize(count).min(10_000));
        let mut records = Reader::at(data, HEADER_SIZE)
            .take(to_usize(count).saturating_mul(KEY_RECORD), "keyx records")?;
        for index in 0..to_usize(count) {
            let mut r = records.take(KEY_RECORD, "keyx record")?.with_record(index);
            let start = r.offset();
            skip_to(&mut r, start, 0x20)?;
            let key_at = r.offset();
            let key = r.u32()?;
            if key >= GRID * GRID {
                return Err(r.error_at(key_at, field("grid key", key.into(), "below 10000".into())));
            }
            let id = r.u32()?;
            skip_to(&mut r, start, 0x6C)?;
            let layers = r.u32()?;
            skip_to(&mut r, start, 0xEC)?;
            let offset = r.u32()?;
            let compressed = r.u32()?;
            skip_to(&mut r, start, 0x108)?;
            let inflated = r.u32()?;
            skip_to(&mut r, start, 0x1ED)?;
            let mask = r.u8()?;
            let mut dungeon = false;
            for entry in 0..4 {
                let code = r.u16()?;
                dungeon |= mask & (1 << entry) != 0 && code == 4;
                r.bytes(26, "keyx sector entry")?;
            }
            skip_to(&mut r, start, 0x2C0)?;
            let area = r.u8()?;
            skip_to(&mut r, start, 0x2D8)?;
            let music = r.u32()?;
            skip_to(&mut r, start, 0x2E0)?;
            let liquids = [r.u8()?, r.u8()?];
            sectors.push(SectorKey {
                x: key % GRID,
                y: key / GRID,
                id,
                offset,
                compressed,
                inflated,
                liquids,
                layers,
                area,
                music,
                dungeon,
            });
        }
        Ok(Self { sectors })
    }

    /// The sector at grid position `(x, y)`; 3,950 of the 10,000 positions have none.
    pub fn sector(&self, x: u32, y: u32) -> Option<&SectorKey> {
        self.sectors.iter().find(|s| (s.x, s.y) == (x, y))
    }
}

impl SectorKey {
    /// Inflates and reads the sector from `sectors.wldx`.
    pub fn read(&self, wldx: &[u8]) -> Result<Sector> {
        let file = Reader::new(wldx);
        let stream = file.window(
            to_usize(self.offset),
            to_usize(self.compressed),
            "sector stream",
        )?;
        let data =
            inflate(stream.rest(), to_usize(self.inflated)).map_err(|kind| stream.error(kind))?;
        Sector::parse_with(&data, self.layers).map_err(|mut e| {
            // Offsets inside the inflated stream mean little in the file; name the sector's
            // stream instead.
            e.place.offset = stream.offset();
            e
        })
    }
}

/// An inflated sector stream: the name, the cells, the third chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sector {
    /// `"Sector x,y"` and a NUL, filled with `0xCD`; 275 sectors carry another sector's name or
    /// none.
    pub name: [u8; NAME_SIZE],
    /// 4,096 cells, row by row; neighbours share the heights of their common corners.
    pub cells: Vec<Cell>,
    /// The third chunk: the layers' headers, the inside layers' cells, 256 bytes of unknown use.
    pub tail: Vec<u8>,
    /// The inside layers of the third chunk, the layers from 1 (layer 0 is the sector's cells).
    pub layers: Vec<Layer>,
}

/// An inside layer of a sector (docs/02 §5.2, docs/re/movement.md): a grid of cells of a
/// building's own, which the hero walks inside it. Its header in the third chunk is 36 bytes:
/// `u32 x, u32 y` the global cell of its first cell, `u16 width, u16 height`, `u32 kind` (6),
/// `u32` the offset of its cells in the stream, `u32` their size (`width · height · 32`), 12 zero
/// bytes; the header of layer 0 is the sector's own, at its origin, of size 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    pub x: u32,
    pub y: u32,
    pub width: u16,
    pub height: u16,
    /// `width · height` cells, row by row, of the sector's cell layout: in every layer of the
    /// game only their second heights, offsets, flags (0, or 4 a door) and ground bytes are set,
    /// but two cells' first static.
    pub cells: Vec<Cell>,
}

/// Floor cell, 32 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    /// `+0x00`: tile of `tiles.pak`.
    pub tile: u32,
    /// `+0x04`: the first static of the cell's chain in `Static.PAK`; 0 for none. (`+0x08` is 0
    /// in every cell: the head of an object list removed before release.)
    pub first_static: u32,
    /// `+0x0C`: the first floor overlay of the cell's chain in `Floor.PAK`; 0 for none.
    pub first_floor: u32,
    /// `+0x10`: corner heights left, top, right, bottom (screen pixels at zoom 1, multiples of
    /// 5); under a liquid the depth of the bed.
    pub heights: [i8; 4],
    /// `+0x14`: corner light.
    pub light: [u8; 4],
    /// `+0x18`: a second corner height, the height a creature stands at, in steps of 2.5 units
    /// of the models' height (docs/re/movement.md, "The height").
    pub heights2: [i8; 4],
    /// `+0x1C`: offsets of the parent object (openheilig).
    pub offsets: [i8; 2],
    /// `+0x1E`: bit 0 structure, bit 1 room interior, bit 2 door (openheilig).
    pub flags: u8,
    /// `+0x1F`: high nibble the nature (4 `GRASS`, 9 `WATERA`, 10 `WATERB`, 11 `STONEROAD`, …,
    /// docs/02 §5.2), low nibble unknown.
    pub ground: u8,
}

impl Cell {
    /// Ground type of the high nibble of `+0x1F`.
    pub fn ground_type(&self) -> u8 {
        self.ground >> 4
    }
}

/// Reads `count` cells of 32 bytes.
fn parse_cells(r: &mut Reader, count: usize) -> Result<Vec<Cell>> {
    let mut cells = Vec::with_capacity(count);
    for _ in 0..count {
        let at = r.offset();
        let tile = r.u32()?;
        let first_static = r.u32()?;
        let objects = r.u32()?;
        if objects != 0 {
            return Err(r.error_at(at + 8, field("cell +0x08", objects.into(), "0".into())));
        }
        let first_floor = r.u32()?;
        let signed = |b: [u8; 4]| b.map(|x| i8::from_le_bytes([x]));
        let heights = signed(r.array("cell heights")?);
        let light = r.array("cell light")?;
        let heights2 = signed(r.array("cell second heights")?);
        let [a, b] = r.array("cell offsets")?;
        let offsets = [i8::from_le_bytes([a]), i8::from_le_bytes([b])];
        let flags = r.u8()?;
        let ground = r.u8()?;
        cells.push(Cell {
            tile,
            first_static,
            first_floor,
            heights,
            light,
            heights2,
            offsets,
            flags,
            ground,
        });
    }
    Ok(cells)
}

impl Sector {
    /// Parses an inflated stream without its layers; offsets in errors are from its start.
    pub fn parse(data: &[u8]) -> Result<Self> {
        Self::parse_with(data, 0)
    }

    /// Parses an inflated stream and the `layers` headers of its third chunk (`keyx +0x6C`), the
    /// first the ground's; offsets in errors are from the stream's start.
    pub fn parse_with(data: &[u8], layers: u32) -> Result<Self> {
        let mut r = Reader::new(data);
        let name = r.array("sector name")?;
        let mut cells_data = r.take(CELL_CHUNK, "sector cells")?;
        let cells = parse_cells(&mut cells_data, CELLS)?;
        let tail = r.rest().to_vec();
        let mut headers = r.take(
            to_usize(layers).saturating_mul(LAYER_HEADER),
            "layer headers",
        )?;
        let mut inside = Vec::new();
        for index in 0..to_usize(layers) {
            let mut h = headers
                .take(LAYER_HEADER, "layer header")?
                .with_record(index);
            let start = h.offset();
            let (x, y) = (h.u32()?, h.u32()?);
            let (width, height) = (h.u16()?, h.u16()?);
            let kind = h.u32()?;
            if kind != LAYER_KIND {
                return Err(h.error_at(
                    start + 12,
                    field("layer kind", kind.into(), LAYER_KIND.to_string()),
                ));
            }
            let (offset, size) = (h.u32()?, h.u32()?);
            let cells = usize::from(width) * usize::from(height);
            if to_usize(size) != cells * CELL_SIZE {
                return Err(h.error_at(
                    start + 20,
                    field("layer size", size.into(), (cells * CELL_SIZE).to_string()),
                ));
            }
            if index == 0 {
                // The ground's: the sector's own cells.
                if cells != 0 || offset != 0 {
                    return Err(h.error_at(
                        start + 8,
                        field("ground layer size", u64::from(size), "0".into()),
                    ));
                }
                continue;
            }
            let mut data = Reader::new(data)
                .window(to_usize(offset), to_usize(size), "layer cells")?
                .with_record(index);
            inside.push(Layer {
                x,
                y,
                width,
                height,
                cells: parse_cells(&mut data, cells)?,
            });
        }
        Ok(Self {
            name,
            cells,
            tail,
            layers: inside,
        })
    }

    /// Cell at row `r`, column `c`.
    pub fn cell(&self, r: usize, c: usize) -> Option<&Cell> {
        (r < SIDE && c < SIDE)
            .then(|| self.cells.get(r * SIDE + c))
            .flatten()
    }
}

/// A PAK container of fixed-size records that lie back to back after the directory.
#[derive(Debug)]
struct Records<'a> {
    pak: Pak<'a>,
    size: usize,
}

impl<'a> Records<'a> {
    fn parse(
        data: &'a [u8],
        magic: [u8; 3],
        version: u8,
        kind: Option<u32>,
        size: usize,
    ) -> Result<Self> {
        let pak = Pak::parse_expecting(data, magic, version)?;
        if let Some(kind) = kind
            && let Some((index, entry)) = pak
                .entries()
                .iter()
                .enumerate()
                .find(|(_, e)| e.kind != kind)
        {
            return Err(Reader::new(data).error_at(
                pak.entry_offset(index),
                field("entry type", entry.kind.into(), kind.to_string()),
            ));
        }
        Ok(Self { pak, size })
    }

    fn len(&self) -> usize {
        self.pak.entries().len()
    }

    /// Record `index` as stored, checked against its directory entry.
    fn bytes(&self, index: usize) -> Result<&'a [u8]> {
        let entry = self.pak.entry(index)?;
        if to_usize(entry.size) != self.size {
            return Err(Reader::new(self.pak.data()).error_at(
                self.pak.entry_offset(index) + 8,
                field("record size", entry.size.into(), self.size.to_string()),
            ));
        }
        let mut r = self.pak.record_reader(index, entry);
        r.bytes(self.size, "record")
    }
}

/// `PAK/tiles.pak`: `"ISO"` v3, 90,132 tiles of 64 bytes.
#[derive(Debug)]
pub struct Tiles<'a> {
    records: Records<'a>,
}

/// Ground tile: one of the 18 diamonds of a 256 × 256 sheet of `texture.pak`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tile<'a> {
    /// The source image, `isoNNN.tga`; the 18 tiles of a sheet share it.
    pub name: &'a [u8],
    /// Record of `texture.pak`.
    pub texture: u32,
    /// The diamond of the sheet, 0..=17: the tile's index modulo 18 (docs/02 §5.4).
    pub diamond: u32,
}

impl<'a> Tiles<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        Ok(Self {
            records: Records::parse(data, TILES_MAGIC, 3, None, 64)?,
        })
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Tile `index`: `char name[32]`, `u32` texture, `u32` diamond, then `0`, `65536` and 16 zero
    /// bytes.
    pub fn get(&self, index: usize) -> Result<Tile<'a>> {
        let bytes = self.records.bytes(index)?;
        let mut r = Reader::new(bytes).with_record(index);
        let name = r.bytes(32, "tile name")?;
        let name = name.split(|&b| b == 0).next().unwrap_or_default();
        let texture = r.u32()?;
        let diamond_at = r.offset();
        let diamond = r.u32()?;
        let expected = index % 18;
        if to_usize(diamond) != expected {
            return Err(r.error_at(
                diamond_at,
                field("tile diamond", diamond.into(), expected.to_string()),
            ));
        }
        Ok(Tile {
            name,
            texture,
            diamond,
        })
    }
}

/// `World/Static.PAK`: `"OBJ"` v1, 1,038,014 static objects of 64 bytes; record 0 is empty.
#[derive(Debug)]
pub struct Statics<'a> {
    records: Records<'a>,
}

/// Static object: a sprite or a model placed in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Static {
    /// `+0x04`: the object's type, an `Items.pak` record.
    pub kind: u32,
    /// `+0x08`.
    pub flags: u32,
    /// `+0x0C`: the id of the sector (keyx `+0x24`).
    pub sector: u16,
    /// `+0x0E`, `+0x12`: position in isometric screen pixels, `48 (cx − cy)` and `24 (cx + cy)`
    /// for the global cell `(cx, cy)` (openheilig).
    pub x: i32,
    pub y: i32,
    /// `+0x17`: the first of the object's children; 0 (or the fill `0xCDCDCDCD`) for none.
    pub first_child: u32,
    /// `+0x1B`: the object whose child this is; 0 for none.
    pub parent: u32,
    /// `+0x1F`: the next object of the chain; 0 ends it.
    pub next: u32,
    /// `+0x27`: a trigger (openheilig); 0 in all but 3,007 objects.
    pub trigger: u32,
    /// `+0x2B`: the mask (openheilig): 0 or one bit of 1 to 16; an underlay type's object with
    /// mask 1 goes to draw list 0 (docs/02 §6.1).
    pub mask: u8,
    /// `+0x2D`: for a building's floor (a child of a static of flag `0x10`), the index from 1 of
    /// the inside layer of its sector (`+0x0C`) that holds its grid (`0x634C50`,
    /// docs/re/movement.md).
    pub layer: u8,
    /// `+0x2E`: for a miniature (flag `0x20`), the square of its type's atlas: x, y and size in
    /// pixels; with flag `0x40` columns, rows, rotation, timing and frames of an animation
    /// (openheilig).
    pub miniature: [u8; 5],
    /// `+0x33`: for a type of bit `0x200`, an upper storey's floor, the height of the plane the
    /// client draws it in, in steps of 28 pixels (`0x62FF60`, docs/re/drawing.md, "Depth").
    pub height: u8,
}

impl<'a> Statics<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        Ok(Self {
            records: Records::parse(data, OBJ_MAGIC, 1, Some(STATIC_TYPE), 64)?,
        })
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Record `index` as stored.
    pub fn bytes(&self, index: usize) -> Result<&'a [u8]> {
        self.records.bytes(index)
    }

    /// Static `index`, from 1; its first field holds its own index.
    pub fn get(&self, index: usize) -> Result<Static> {
        let bytes = self.records.bytes(index)?;
        let mut r = Reader::new(bytes).with_record(index);
        own_index(&mut r, index)?;
        let kind = r.u32()?;
        let flags = r.u32()?;
        let sector = r.u16()?;
        let x = i32::from_le_bytes(r.array("static x")?);
        let y = i32::from_le_bytes(r.array("static y")?);
        r.bytes(0x17 - 0x16, "static")?;
        let link = |r: &mut Reader| -> Result<u32> {
            let at = r.offset();
            let value = r.u32()?;
            let value = if value == FILL { 0 } else { value };
            if to_usize(value) >= self.len() {
                return Err(r.error_at(
                    at,
                    field("static link", value.into(), format!("below {}", self.len())),
                ));
            }
            Ok(value)
        };
        let first_child = link(&mut r)?;
        let parent = link(&mut r)?;
        let next = link(&mut r)?;
        r.bytes(0x27 - 0x23, "static")?;
        let trigger = r.u32()?;
        let mask = r.u8()?;
        r.bytes(0x2D - 0x2C, "static")?;
        let layer = r.u8()?;
        let miniature = r.array("static miniature")?;
        let height = r.u8()?;
        Ok(Static {
            kind,
            flags,
            sector,
            x,
            y,
            first_child,
            parent,
            next,
            trigger,
            mask,
            layer,
            miniature,
            height,
        })
    }

    /// The chain from `head`, head first, as indices; empty for 0. A chain longer than the file
    /// is a loop and an error.
    pub fn chain(&self, head: u32) -> Result<Vec<u32>> {
        chain(head, self.len(), |index| {
            self.get(to_usize(index)).map(|s| s.next)
        })
    }
}

/// `World/Floor.PAK`: `"OBJ"` v1, 6,713,136 floor overlays of 16 bytes; record 0 is empty.
#[derive(Debug)]
pub struct Floors<'a> {
    records: Records<'a>,
}

/// Floor overlay: a tile drawn over a cell's ground through the alpha of a mask tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Floor {
    /// The tile drawn, the low 17 bits of `+0x04`.
    pub art: u32,
    /// The tile whose alpha masks it, the high 15 bits of `+0x04`; 0 for none.
    pub mask: u32,
    /// `+0x0C`: the next overlay of the chain; 0 ends it.
    pub next: u32,
}

impl<'a> Floors<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        Ok(Self {
            records: Records::parse(data, OBJ_MAGIC, 1, Some(FLOOR_TYPE), 16)?,
        })
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Record `index` as stored.
    pub fn bytes(&self, index: usize) -> Result<&'a [u8]> {
        self.records.bytes(index)
    }

    /// Overlay `index`, from 1: own index, tiles, `0` or the fill, next.
    pub fn get(&self, index: usize) -> Result<Floor> {
        let bytes = self.records.bytes(index)?;
        let mut r = Reader::new(bytes).with_record(index);
        own_index(&mut r, index)?;
        let tiles = r.u32()?;
        r.u32()?; // +0x08: 0 or the fill 0xCDCDCDCD
        let next_at = r.offset();
        let next = r.u32()?;
        if to_usize(next) >= self.len() {
            return Err(r.error_at(
                next_at,
                field("floor link", next.into(), format!("below {}", self.len())),
            ));
        }
        Ok(Floor {
            art: tiles & 0x1_FFFF,
            mask: tiles >> 17,
            next,
        })
    }

    /// The chain from `head`, head first, as indices; empty for 0.
    pub fn chain(&self, head: u32) -> Result<Vec<u32>> {
        chain(head, self.len(), |index| {
            self.get(to_usize(index)).map(|f| f.next)
        })
    }
}

/// Moves `r`, a record that starts at `start`, to the record's offset `to`.
fn skip_to(r: &mut Reader, start: usize, to: usize) -> Result<()> {
    r.take(to.saturating_sub(r.offset() - start), "keyx record")?;
    Ok(())
}

/// The `u32` at the start of a record of `Static.PAK` or `Floor.PAK`: the record's own index.
fn own_index(r: &mut Reader, index: usize) -> Result<()> {
    let at = r.offset();
    let own = r.u32()?;
    if to_usize(own) != index || index == 0 {
        return Err(r.error_at(
            at,
            field("own index", own.into(), format!("{index}, from 1")),
        ));
    }
    Ok(())
}

fn chain(head: u32, len: usize, next: impl Fn(u32) -> Result<u32>) -> Result<Vec<u32>> {
    let mut out = Vec::new();
    let mut at = head;
    while at != 0 {
        if out.len() >= len {
            return Err(crate::Error {
                place: crate::Place {
                    record: Some(to_usize(head)),
                    offset: 0,
                },
                kind: field(
                    "chain length",
                    u64::try_from(len).unwrap_or(u64::MAX),
                    "a chain without loops".into(),
                ),
            });
        }
        out.push(at);
        at = next(at)?;
    }
    Ok(out)
}

#[cfg(test)]
mod game_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{PakRecord, build_pak};

    /// Container of `records`, each a directory type and its bytes.
    fn pak(magic: [u8; 3], version: u8, records: &[(u32, Vec<u8>)]) -> Vec<u8> {
        let records: Vec<_> = records
            .iter()
            .map(|(kind, bytes)| Some(PakRecord::new(*kind, bytes)))
            .collect();
        build_pak(magic, version, &records)
    }

    fn cell_bytes(tile: u32, first_static: u32, first_floor: u32, height: i8) -> Vec<u8> {
        let mut out = Vec::new();
        for word in [tile, first_static, 0, first_floor] {
            out.extend_from_slice(&word.to_le_bytes());
        }
        out.extend_from_slice(&[height.to_le_bytes()[0]; 4]);
        out.extend_from_slice(&[255, 235, 215, 195]);
        out.extend_from_slice(&[0xFF, 1, 0, 2]);
        out.extend_from_slice(&[0xFE, 3, 0b101, 0x93]);
        out
    }

    fn sector_stream() -> Vec<u8> {
        let mut out = b"Sector 1,2\0".to_vec();
        out.resize(NAME_SIZE, 0xCD);
        for k in 0..CELLS {
            let k = u32::try_from(k).unwrap();
            out.extend(cell_bytes(1000 + k, k % 7, k % 5, -5));
        }
        out.extend_from_slice(b"tail");
        out
    }

    #[test]
    fn reads_the_inside_layers_of_the_third_chunk() {
        let mut stream = sector_stream();
        stream.truncate(NAME_SIZE + CELL_CHUNK);
        let header = |x: u32, y: u32, (w, h): (u16, u16), offset: u32, size: u32| {
            let mut out = Vec::new();
            for word in [x, y] {
                out.extend_from_slice(&word.to_le_bytes());
            }
            out.extend_from_slice(&w.to_le_bytes());
            out.extend_from_slice(&h.to_le_bytes());
            for word in [LAYER_KIND, offset, size, 0, 0, 0] {
                out.extend_from_slice(&word.to_le_bytes());
            }
            out
        };
        let cells_at = u32::try_from(NAME_SIZE + CELL_CHUNK + 2 * LAYER_HEADER).unwrap();
        stream.extend(header(64, 128, (0, 0), 0, 0));
        stream.extend(header(70, 130, (3, 2), cells_at, 6 * 32));
        for k in 0..6 {
            let mut cell = vec![0; 32];
            cell[0x1E] = if k == 4 { 4 } else { 0 };
            cell[0x1F] = if k < 3 { 0xD1 } else { 0 };
            stream.extend(cell);
        }
        stream.extend([0; 256]);
        let sector = Sector::parse_with(&stream, 2).unwrap();
        assert_eq!(sector.layers.len(), 1);
        let layer = &sector.layers[0];
        assert_eq!(
            (layer.x, layer.y, layer.width, layer.height),
            (70, 130, 3, 2)
        );
        let bytes: Vec<_> = layer.cells.iter().map(|c| (c.flags, c.ground)).collect();
        assert_eq!(
            bytes,
            [(0, 0xD1), (0, 0xD1), (0, 0xD1), (0, 0), (4, 0), (0, 0)]
        );
        // A header of another kind.
        let at = NAME_SIZE + CELL_CHUNK + LAYER_HEADER + 12;
        stream[at] = 7;
        assert!(Sector::parse_with(&stream, 2).is_err());
    }

    #[test]
    fn reads_the_cells_of_a_sector() {
        let sector = Sector::parse(&sector_stream()).unwrap();
        assert!(sector.name.starts_with(b"Sector 1,2\0\xCD"));
        assert_eq!(sector.cells.len(), CELLS);
        assert_eq!(sector.tail, b"tail");
        let cell = sector.cell(1, 2).unwrap();
        assert_eq!(cell.tile, 1000 + 66);
        assert_eq!((cell.first_static, cell.first_floor), (66 % 7, 66 % 5));
        assert_eq!(cell.heights, [-5; 4]);
        assert_eq!(cell.light, [255, 235, 215, 195]);
        assert_eq!(cell.heights2, [-1, 1, 0, 2]);
        assert_eq!(cell.offsets, [-2, 3]);
        assert_eq!((cell.flags, cell.ground_type()), (0b101, 9));
        assert!(sector.cell(64, 0).is_none());
    }

    #[test]
    fn the_removed_object_list_is_empty() {
        let mut stream = sector_stream();
        stream[NAME_SIZE + 32 * 5 + 8] = 1;
        let err = Sector::parse(&stream).unwrap_err();
        assert_eq!(err.place.offset, NAME_SIZE + 32 * 5 + 8);
        assert!(
            err.to_string()
                .ends_with("field `cell +0x08` is 1, expected 0"),
            "{err}"
        );
    }

    #[test]
    fn a_short_stream_is_an_error() {
        let stream = sector_stream();
        let err = Sector::parse(&stream[..NAME_SIZE + 100]).unwrap_err();
        assert!(
            matches!(
                err.kind,
                ErrorKind::OutOfBounds {
                    what: "sector cells",
                    ..
                }
            ),
            "{err}"
        );
    }

    /// keyx with the given `(key, id)` records; the stream offsets point at `streams`.
    fn keyx(records: &[(u32, u32, u32, u32, u32)]) -> Vec<u8> {
        let mut data = vec![0; HEADER_SIZE];
        data[..4].copy_from_slice(b"WLK\x05");
        let count = u32::try_from(records.len()).unwrap();
        for (k, value) in [count, GRID, GRID].iter().enumerate() {
            data[4 + 4 * k..8 + 4 * k].copy_from_slice(&value.to_le_bytes());
        }
        for &(key, id, offset, compressed, inflated) in records {
            let mut record = vec![0; KEY_RECORD];
            record[0x20..0x24].copy_from_slice(&key.to_le_bytes());
            record[0x24..0x28].copy_from_slice(&id.to_le_bytes());
            record[0xEC..0xF0].copy_from_slice(&offset.to_le_bytes());
            record[0xF0..0xF4].copy_from_slice(&compressed.to_le_bytes());
            record[0x108..0x10C].copy_from_slice(&inflated.to_le_bytes());
            record[0x2E0] = 1;
            record[0x2E1] = 4;
            data.extend(record);
        }
        data
    }

    #[test]
    fn keyx_finds_a_sector_and_reads_its_stream() {
        let stream = sector_stream();
        let packed = miniz_oxide::deflate::compress_to_vec_zlib(&stream, 6);
        let mut wldx = b"pad".to_vec();
        let offset = u32::try_from(wldx.len()).unwrap();
        wldx.extend_from_slice(&packed);
        let len = |n: usize| u32::try_from(n).unwrap();
        let data = keyx(&[
            (3950, 2579, offset, len(packed.len()), len(stream.len())),
            (1500, 915, 0, 0, 0),
        ]);
        let keyx = Keyx::parse(&data).unwrap();
        assert_eq!(keyx.sectors.len(), 2);
        let key = keyx.sector(50, 39).unwrap();
        assert_eq!((key.id, key.liquids), (2579, [1, 4]));
        assert_eq!(keyx.sector(0, 15).unwrap().id, 915);
        assert!(keyx.sector(1, 1).is_none());
        let sector = key.read(&wldx).unwrap();
        assert_eq!(sector, Sector::parse(&stream).unwrap());

        let mut bad = data.clone();
        bad[HEADER_SIZE + 0x20..HEADER_SIZE + 0x24].copy_from_slice(&10_000u32.to_le_bytes());
        let err = Keyx::parse(&bad).unwrap_err();
        assert_eq!(err.place.record, Some(0));
        assert!(
            err.to_string()
                .ends_with("field `grid key` is 10000, expected below 10000")
        );
    }

    fn statics() -> Vec<u8> {
        let record = |own: u32, kind: u32, x: i32, links: [u32; 3]| {
            let mut r = vec![0u8; 64];
            r[0..4].copy_from_slice(&own.to_le_bytes());
            r[4..8].copy_from_slice(&kind.to_le_bytes());
            r[0x0C..0x0E].copy_from_slice(&7u16.to_le_bytes());
            r[0x0E..0x12].copy_from_slice(&x.to_le_bytes());
            r[0x12..0x16].copy_from_slice(&(-x).to_le_bytes());
            for (k, link) in links.iter().enumerate() {
                r[0x17 + 4 * k..0x1B + 4 * k].copy_from_slice(&link.to_le_bytes());
            }
            r[0x27..0x2B].copy_from_slice(&(own * 10).to_le_bytes());
            r[0x2B] = 1;
            r[0x2D] = 3;
            r
        };
        pak(
            OBJ_MAGIC,
            1,
            &[
                (STATIC_TYPE, vec![0; 64]),
                (STATIC_TYPE, record(1, 801, 96, [FILL, 0, 2])),
                (STATIC_TYPE, record(2, 802, -48, [3, 0, 0])),
                (STATIC_TYPE, record(3, 803, 0, [0, 2, 0])),
            ],
        )
    }

    #[test]
    fn statics_chain_and_keep_their_children() {
        let data = statics();
        let statics = Statics::parse(&data).unwrap();
        assert_eq!(statics.len(), 4);
        let first = statics.get(1).unwrap();
        assert_eq!(
            first,
            Static {
                kind: 801,
                flags: 0,
                sector: 7,
                x: 96,
                y: -96,
                first_child: 0,
                parent: 0,
                next: 2,
                trigger: 10,
                mask: 1,
                layer: 3,
                miniature: [0; 5],
                height: 0,
            }
        );
        assert_eq!(statics.chain(1).unwrap(), [1, 2]);
        assert_eq!(statics.get(2).unwrap().first_child, 3);
        assert_eq!(statics.get(3).unwrap().parent, 2);
        assert!(statics.chain(0).unwrap().is_empty());
        // Record 0 is no object.
        assert!(statics.get(0).is_err());
    }

    #[test]
    fn a_static_link_out_of_range_or_a_loop_is_an_error() {
        let mut data = statics();
        let record = |k: usize| HEADER_SIZE + 12 * 4 + 64 * k;
        data[record(2) + 0x1F..record(2) + 0x23].copy_from_slice(&9u32.to_le_bytes());
        let statics = Statics::parse(&data).unwrap();
        let err = statics.get(2).unwrap_err();
        assert!(
            err.to_string()
                .ends_with("field `static link` is 9, expected below 4"),
            "{err}"
        );

        let mut data = self::statics();
        data[record(2) + 0x1F..record(2) + 0x23].copy_from_slice(&1u32.to_le_bytes());
        let statics = Statics::parse(&data).unwrap();
        assert!(statics.chain(1).is_err());
    }

    #[test]
    fn floors_split_art_and_mask() {
        let record = |own: u32, tiles: u32, fill: u32, next: u32| {
            [own, tiles, fill, next]
                .iter()
                .flat_map(|w| w.to_le_bytes())
                .collect::<Vec<u8>>()
        };
        let data = pak(
            OBJ_MAGIC,
            1,
            &[
                (FLOOR_TYPE, vec![0; 16]),
                (FLOOR_TYPE, record(1, 90_131 | (5 << 17), FILL, 2)),
                (FLOOR_TYPE, record(2, 18, 0, 0)),
            ],
        );
        let floors = Floors::parse(&data).unwrap();
        assert_eq!(
            floors.get(1).unwrap(),
            Floor {
                art: 90_131,
                mask: 5,
                next: 2
            }
        );
        assert_eq!(floors.get(2).unwrap().mask, 0);
        assert_eq!(floors.chain(1).unwrap(), [1, 2]);
        // A static container is not a floor container.
        assert!(Floors::parse(&statics()).is_err());
    }

    #[test]
    fn tiles_name_their_sheet_and_diamond() {
        let tile = |name: &[u8], texture: u32, diamond: u32| {
            let mut r = name.to_vec();
            r.resize(32, 0);
            for word in [texture, diamond, 0, 65_536, 0, 0, 0, 0] {
                r.extend_from_slice(&word.to_le_bytes());
            }
            (0, r)
        };
        let data = pak(
            TILES_MAGIC,
            3,
            &[
                tile(b"iso00.tga", 6000, 0),
                tile(b"iso00.tga", 6000, 1),
                tile(b"iso00.tga", 6000, 5),
            ],
        );
        let tiles = Tiles::parse(&data).unwrap();
        assert_eq!(
            tiles.get(1).unwrap(),
            Tile {
                name: b"iso00.tga",
                texture: 6000,
                diamond: 1
            }
        );
        let err = tiles.get(2).unwrap_err();
        assert!(
            err.to_string()
                .ends_with("field `tile diamond` is 5, expected 2"),
            "{err}"
        );
    }
}
