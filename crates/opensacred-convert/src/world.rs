//! `world` subcommand: `World/sectors.keyx` and `sectors.wldx` with `Static.PAK`, `Floor.PAK`,
//! `PAK/tiles.pak`, `PAK/items.pak`, `PAK/mixed.pak` and `PAK/texture.pak` →
//! `<out>/world/world.ron`, `<out>/world/sectors/<x>-<y>.sector`, `<out>/world/sprites.bin` and
//! `<out>/world/sprites/<record>.png` (docs/02 §5; the format is [`opensacred_data::world`]). Tile
//! sheets and the miniatures' atlases are referred to as the PNGs that `opensacred-convert
//! textures` writes to `<out>/textures/texture/`, and in the game's index as the KTX2 files that
//! `build` makes of them ([`built_index`]).
//!
//! Each sprite of `mixed.pak` with pieces is assembled here once, its pieces copied one to one
//! from the atlases of `texture.pak` into one image over its rectangle (#838): so that the game
//! filters the texels on either side of a piece's edge as it does inside a piece, where filtering
//! each piece in its own atlas could not reach the next piece's texels and the step between two
//! pieces showed as a line once magnified. The rectangle is the sprite's size from its pieces'
//! smallest corner, past which the pieces' blocks hold transparent texels only (docs/02 §3.3):
//! where the art is cut at the rectangle's edge to go on in another sprite, its edge's texels then
//! reach the edge, as the original draws them, rather than fading out into those texels half a
//! texel short of it (#35). The atlases that only the sprites' pieces use leave the game's assets
//! ([`World::assembled_from`]).

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use opensacred_data::world::{
    Building, Cell, INDEX, Layer, LayerCell, Miniature, Overlay, SPRITES_DIR, SPRITES_FILE, Sector,
    SectorEntry, Shadow, Sprite, Sprites, StaticObject, World, sprite_image,
};
use opensacred_formats::items::Items;
use opensacred_formats::mixed::Sprites as MixedPak;
use opensacred_formats::tex::{TexPak, Texture};
use opensacred_formats::world::{Floors, Keyx, SectorKey, Statics, Tiles};
use rayon::prelude::*;

use crate::textures;
use crate::verify::read_game_file;

/// Deepest nesting of child objects followed; the game's deepest is 1.
const MAX_DEPTH: usize = 8;
/// Flag of a miniature static: a square of its type's atlas instead of a sprite.
const MINIATURE: u32 = 0x20;
/// The flag of a building's object, whose children are its floors (`0x6354D0`).
const BUILDING: u32 = 0x10;
/// The flag of a building's object open to the sky, all of whose floors' objects the client
/// draws whatever its state (`0x62AE90`, docs/re/drawing.md).
const OPEN: u32 = 0x400;
/// The bit of a type's flags (`items.pak +0x00` as a u32) whose statics cast a shadow
/// (`0x62AE90`, docs/re/drawing.md).
const CASTS_SHADOW: u32 = 0x1_0000;
/// The bit of a type's flags whose statics, the upper storeys' floors, the client draws in a
/// horizontal plane through the depth buffer (`0x62FF60`, docs/re/drawing.md, "Depth").
const FLOOR: u32 = 0x200;
/// Units of the models' height per step of a floor's `+0x33` (`0x62FF60`).
const FLOOR_STEP: u16 = 28;

/// Directory of the converted world.
pub fn output_dir(out: &Path) -> PathBuf {
    out.join("world")
}

/// `50-39.sector`.
pub fn file_name(x: u32, y: u32) -> String {
    format!("{x}-{y}.sector")
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub sectors: usize,
    pub overlays: usize,
    pub statics: usize,
    /// Static objects drawn by a sprite, and by a miniature.
    pub sprite_statics: usize,
    pub miniatures: usize,
    /// Sprites with an image, the atlases their images were assembled from, and those of the
    /// miniatures.
    pub sprites: usize,
    pub assembled_from: usize,
    pub atlases: usize,
    /// Tile sheets whose texture record is empty: no image to refer to.
    pub missing_sheets: usize,
    /// Bytes of the sector files.
    pub bytes: u64,
}

/// The original files, read once.
struct Sources {
    keyx: Vec<u8>,
    wldx: Vec<u8>,
    tiles: Vec<u8>,
    statics: Vec<u8>,
    floors: Vec<u8>,
    textures: Vec<u8>,
    items: Vec<u8>,
    mixed: Vec<u8>,
}

/// The original tables a sector's objects refer to.
struct Tables<'a> {
    statics: Statics<'a>,
    floors: Floors<'a>,
    items: Items<'a>,
    /// Index into [`World::atlases`] of every atlas texture.
    atlases: &'a HashMap<u32, u32>,
}

/// Atlases in the order of first use, and their PNGs.
#[derive(Default)]
struct Atlases {
    index: HashMap<u32, u32>,
    paths: Vec<String>,
}

impl Atlases {
    fn add(&mut self, texture: u32, pak: &TexPak) -> Result<u32> {
        if let Some(&made) = self.index.get(&texture) {
            return Ok(made);
        }
        let index = usize::try_from(texture).context("texture index")?;
        let record = pak
            .record(index)?
            .with_context(|| format!("atlas {texture}: empty texture record"))?;
        self.paths.push(png_path(index, record.header.name()));
        let made = u32::try_from(self.paths.len() - 1).context("atlas count")?;
        self.index.insert(texture, made);
        Ok(made)
    }
}

/// URI of the PNG of texture record `index`, relative to `world.ron`.
fn png_path(index: usize, name: &[u8]) -> String {
    format!(
        "../textures/{}/{}",
        textures::PAKS[0].1,
        textures::file_name(index, name)
    )
}

/// Converts every sector, or for `only` the one at that grid position.
pub fn convert(game_dir: &Path, out: &Path, only: Option<(u32, u32)>) -> Result<Summary> {
    let read = |rel: &str| read_game_file(&game_dir.join(rel));
    let sources = Sources {
        keyx: read("World/sectors.keyx")?,
        wldx: read("World/sectors.wldx")?,
        tiles: read("PAK/tiles.pak")?,
        statics: read("World/Static.PAK")?,
        floors: read("World/Floor.PAK")?,
        textures: read(textures::PAKS[0].0)?,
        items: read("PAK/Items.pak")?,
        mixed: read("PAK/mixed.pak")?,
    };
    let keyx = Keyx::parse(&sources.keyx).context("World/sectors.keyx")?;
    let tiles = Tiles::parse(&sources.tiles).context("PAK/tiles.pak")?;
    let statics = Statics::parse(&sources.statics).context("World/Static.PAK")?;
    let floors = Floors::parse(&sources.floors).context("World/Floor.PAK")?;
    let texture_pak = TexPak::parse(&sources.textures).context(textures::PAKS[0].0)?;
    let items = Items::parse(&sources.items).context("PAK/items.pak")?;
    let mixed = MixedPak::parse(&sources.mixed).context("PAK/mixed.pak")?;

    let dir = output_dir(out);
    let sector_dir = dir.join("sectors");
    fs::create_dir_all(&sector_dir)
        .with_context(|| format!("creating {}", sector_dir.display()))?;

    let mut summary = Summary::default();
    let mut sheets = Vec::with_capacity(tiles.len() / 18);
    for first in (0..tiles.len()).step_by(18) {
        let texture = tiles.get(first).context("PAK/tiles.pak")?.texture;
        let index = usize::try_from(texture).context("texture index")?;
        let record = texture_pak
            .record(index)
            .with_context(|| format!("tile {first}: texture {index}"))?;
        sheets.push(match record {
            Some(record) => png_path(index, record.header.name()),
            None => {
                summary.missing_sheets += 1;
                String::new()
            }
        });
    }

    let (sprites, assembled_from) =
        convert_sprites(&mixed, &texture_pak, &dir).context("PAK/mixed.pak")?;
    summary.sprites = sprites.sprites.iter().flatten().count();
    summary.assembled_from = assembled_from.len();
    // The atlases of the miniature types, so that every sector finds them.
    let mut atlases = Atlases::default();
    for index in 0..items.len() {
        if let Some(item) = items.get(index)?
            && matches!(item.kind, 8 | 12)
        {
            atlases
                .add(item.texture, &texture_pak)
                .with_context(|| format!("PAK/items.pak record {index}"))?;
        }
    }
    let file = sprites
        .encode()
        .map_err(|e| anyhow::anyhow!("encoding the sprites: {e}"))?;
    let path = dir.join(SPRITES_FILE);
    fs::write(&path, file).with_context(|| format!("writing {}", path.display()))?;
    let tables = Tables {
        statics,
        floors,
        items,
        atlases: &atlases.index,
    };

    let keys: Vec<&SectorKey> = keyx
        .sectors
        .iter()
        .filter(|k| only.is_none_or(|(x, y)| (k.x, k.y) == (x, y)))
        .collect();
    if let Some((x, y)) = only
        && keys.is_empty()
    {
        bail!("no sector at ({x}, {y})");
    }
    let converted = keys
        .par_iter()
        .map(|key| {
            let what = || format!("sector ({}, {})", key.x, key.y);
            let sector = convert_sector(key, &sources.wldx, &tables).with_context(what)?;
            let overlays = sector.cells.iter().map(|c| c.overlays.len()).sum::<usize>();
            let objects = count(&sector.statics, &|_| true);
            let drawn = count(&sector.statics, &|o| o.sprite != 0);
            let miniatures = count(&sector.statics, &|o| o.miniature.is_some());
            let file = sector
                .encode()
                .map_err(|e| anyhow::anyhow!("encoding: {e}"))
                .with_context(what)?;
            let path = sector_dir.join(file_name(key.x, key.y));
            fs::write(&path, &file).with_context(|| format!("writing {}", path.display()))?;
            let bytes = u64::try_from(file.len()).unwrap_or(u64::MAX);
            Ok([
                overlays,
                objects,
                drawn,
                miniatures,
                usize::try_from(bytes).unwrap_or(0),
            ])
        })
        .collect::<Result<Vec<_>>>()?;
    for [overlays, objects, drawn, miniatures, bytes] in converted {
        summary.sectors += 1;
        summary.overlays += overlays;
        summary.statics += objects;
        summary.sprite_statics += drawn;
        summary.miniatures += miniatures;
        summary.bytes += u64::try_from(bytes).unwrap_or(0);
    }
    summary.atlases = atlases.paths.len();

    let mut entries: Vec<SectorEntry> = keyx
        .sectors
        .iter()
        .map(|k| SectorEntry {
            x: k.x,
            y: k.y,
            id: k.id,
            file: format!("sectors/{}", file_name(k.x, k.y)),
            music: k.music,
            area: k.area,
            dungeon: k.dungeon,
        })
        .collect();
    entries.sort_by_key(|e| (e.y, e.x));
    let assembled_from = assembled_from
        .into_iter()
        .map(|texture| {
            let index = usize::try_from(texture).context("texture index")?;
            let record = texture_pak
                .record(index)?
                .with_context(|| format!("atlas {texture}: empty texture record"))?;
            Ok(png_path(index, record.header.name()))
        })
        .collect::<Result<Vec<_>>>()?;
    let world = World {
        sectors: entries,
        sheets,
        atlases: atlases.paths,
        assembled_from,
    };
    let text =
        ron::ser::to_string_pretty(&world, crate::utf8::pretty()).context("writing world.ron")?;
    let path = dir.join(INDEX);
    fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
    Ok(summary)
}

/// Objects of `list` and of their children that `filter` accepts.
fn count(list: &[StaticObject], filter: &dyn Fn(&StaticObject) -> bool) -> usize {
    list.iter()
        .map(|o| usize::from(filter(o)) + count(&o.children, filter))
        .sum()
}

/// A piece of a sprite: a rectangle of a `texture.pak` atlas, copied to a rectangle of the same
/// size in the sprite.
struct Piece {
    /// The atlas's `texture.pak` record.
    texture: u32,
    /// `x0, y0, x1, y1` in the atlas, pixels.
    source: [u16; 4],
    /// `x0, y0, x1, y1` in the sprite, pixels; pieces may run past the sprite's size.
    rect: [u16; 4],
}

/// A sprite of `mixed.pak` with pieces: its record, its size and its pieces.
struct Pieces {
    record: u32,
    width: u16,
    height: u16,
    pieces: Vec<Piece>,
}

impl Pieces {
    /// The rectangle that the sprite's image covers: from the pieces' smallest corner, past which
    /// they hold transparent texels only (docs/02 §3.3), cut to the sprite's width and height.
    fn rect(&self) -> [u16; 4] {
        let mut covered = [u16::MAX, u16::MAX, 0, 0];
        for piece in &self.pieces {
            let [x0, y0, x1, y1] = piece.rect;
            covered = [
                covered[0].min(x0),
                covered[1].min(y0),
                covered[2].max(x1),
                covered[3].max(y1),
            ];
        }
        let [x0, y0, x1, y1] = covered;
        [
            x0,
            y0,
            x1.min(x0.saturating_add(self.width)),
            y1.min(y0.saturating_add(self.height)),
        ]
    }
}

/// Sprites assembled at once by one thread, their atlases decoded once for them: neighbouring
/// records take their pieces from neighbouring atlases, so that each thread holds few.
const SPRITES_AT_ONCE: usize = 64;

/// The sprites of `mixed.pak`, each with its rectangle, and their images assembled from their
/// pieces of the atlases of `pak` into `<dir>/sprites/` (#838); the atlases they were assembled
/// from, as `pak`'s records.
fn convert_sprites(mixed: &MixedPak, pak: &TexPak, dir: &Path) -> Result<(Sprites, Vec<u32>)> {
    let mut sprites = vec![None];
    let mut assembled = Vec::new();
    for index in 1..mixed.len() {
        let sprite = mixed.get(index)?;
        if sprite.pieces.is_empty() {
            sprites.push(None);
            continue;
        }
        let mut pieces = Vec::with_capacity(sprite.pieces.len());
        for piece in &sprite.pieces {
            let texture = pak
                .record(usize::try_from(piece.texture).context("texture index")?)?
                .context("atlas without image")?;
            let (w, h) = (
                f64::from(texture.header.width),
                f64::from(texture.header.height),
            );
            let [u0, v0, u1, v1] = piece.uv.map(f64::from);
            // The fractions fall on whole pixels (docs/02 §3.3).
            let source = [u0 * w, v0 * h, u1 * w, v1 * h].map(|v| pixel(v.round()));
            pieces.push(Piece {
                texture: piece.texture,
                source,
                rect: piece.rect,
            });
        }
        let pieces = Pieces {
            record: u32::try_from(index).context("sprite record")?,
            width: sprite.width,
            height: sprite.height,
            pieces,
        };
        let rect = pieces.rect();
        // A rectangle without texels, which draws nothing, has no image.
        if rect[2] <= rect[0] || rect[3] <= rect[1] {
            sprites.push(None);
            continue;
        }
        sprites.push(Some(Sprite {
            width: sprite.width,
            height: sprite.height,
            rect,
        }));
        assembled.push(pieces);
    }
    let images = dir.join(SPRITES_DIR);
    if images.exists() {
        fs::remove_dir_all(&images).with_context(|| format!("removing {}", images.display()))?;
    }
    fs::create_dir_all(&images).with_context(|| format!("creating {}", images.display()))?;
    assembled
        .par_chunks(SPRITES_AT_ONCE)
        .try_for_each(|chunk| {
            let mut atlases = HashMap::new();
            for sprite in chunk {
                let (width, height, pixels) = assemble(sprite, pak, &mut atlases)
                    .with_context(|| format!("sprite {}", sprite.record))?;
                let path = dir.join(sprite_image(sprite.record, "png"));
                let png = textures::encode_png(width, height, &pixels)?;
                fs::write(&path, png).with_context(|| format!("writing {}", path.display()))?;
            }
            anyhow::Ok(())
        })?;
    let mut from: Vec<u32> = assembled
        .iter()
        .flat_map(|sprite| sprite.pieces.iter().map(|piece| piece.texture))
        .collect();
    from.sort_unstable();
    from.dedup();
    Ok((Sprites { sprites }, from))
}

/// The image of `sprite` over its rectangle ([`Pieces::rect`]): its pieces copied one to one from
/// their atlases of `pak`, decoded into `atlases` on first use; its width, height and 8-bit RGBA
/// texels, rows from the top. Past the pieces, and where a piece runs past its atlas, the texels
/// are transparent.
fn assemble(
    sprite: &Pieces,
    pak: &TexPak,
    atlases: &mut HashMap<u32, Texture>,
) -> Result<(u32, u32, Vec<u8>)> {
    let rect = sprite.rect();
    let width = usize::from(rect[2] - rect[0]);
    let height = usize::from(rect[3] - rect[1]);
    let mut pixels = vec![0u8; 4 * width * height];
    for piece in &sprite.pieces {
        let atlas = match atlases.entry(piece.texture) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let record = pak
                    .record(usize::try_from(piece.texture)?)?
                    .with_context(|| format!("atlas {}: empty texture record", piece.texture))?;
                entry.insert(
                    record
                        .decode()
                        .with_context(|| format!("decoding atlas {}", piece.texture))?,
                )
            }
        };
        let (atlas_width, atlas_height) = (
            usize::try_from(atlas.width)?,
            usize::try_from(atlas.height)?,
        );
        let [u0, v0, u1, v1] = piece.source.map(usize::from);
        let x = usize::from(piece.rect[0] - rect[0]);
        let y = usize::from(piece.rect[1] - rect[1]);
        // The part of the piece within the rectangle and the atlas.
        let u1 = u1.min(atlas_width).min((u0 + width).saturating_sub(x));
        let v1 = v1.min(atlas_height).min((v0 + height).saturating_sub(y));
        if u1 <= u0 {
            continue;
        }
        for v in v0..v1 {
            let from = 4 * (v * atlas_width + u0);
            let to = 4 * (v * atlas_width + u1);
            let at = 4 * ((y + v - v0) * width + x);
            pixels[at..at + to - from].copy_from_slice(&atlas.rgba[from..to]);
        }
    }
    Ok((u32::try_from(width)?, u32::try_from(height)?, pixels))
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "an atlas coordinate, a whole number from 0 to 256"
)]
fn pixel(value: f64) -> u16 {
    value.clamp(0.0, f64::from(u16::MAX)) as u16
}

fn convert_sector(key: &SectorKey, wldx: &[u8], tables: &Tables) -> Result<Sector> {
    let sector = key.read(wldx)?;
    let mut cells = Vec::with_capacity(sector.cells.len());
    let mut objects = Vec::new();
    let mut buildings = Vec::new();
    for (index, cell) in sector.cells.iter().enumerate() {
        let cell_index = u16::try_from(index).context("cell index")?;
        let mut overlays = Vec::new();
        for record in tables.floors.chain(cell.first_floor)? {
            let floor = tables.floors.get(to_index(record)?)?;
            overlays.push(Overlay {
                art: floor.art,
                mask: (floor.mask != 0).then_some(floor.mask),
            });
        }
        objects.extend(chain_objects(tables, cell.first_static, cell_index, 0)?);
        if let Some(building) = building(tables, key, cell.first_static, cell_index)? {
            buildings.push(building);
        }
        cells.push(Cell {
            tile: cell.tile,
            overlays,
            heights: cell.heights,
            light: cell.light,
            heights2: cell.heights2,
            offsets: cell.offsets,
            flags: cell.flags,
            ground: cell.ground,
        });
    }
    let layers = sector
        .layers
        .iter()
        .map(|layer| Layer {
            x: layer.x,
            y: layer.y,
            width: u32::from(layer.width),
            height: u32::from(layer.height),
            cells: layer
                .cells
                .iter()
                .map(|cell| LayerCell {
                    heights2: cell.heights2,
                    offsets: cell.offsets,
                    flags: cell.flags,
                    ground: cell.ground,
                })
                .collect(),
        })
        .collect();
    Ok(Sector {
        x: key.x,
        y: key.y,
        id: key.id,
        liquids: key.liquids,
        cells,
        statics: objects,
        layers,
        buildings,
    })
}

/// The building whose object lies in the chain from `head`, of cell `cell`: the first static of
/// [`BUILDING`] there, its children its floors in order, each the inside layer of the sector that
/// holds its grid (`0x6354D0`, `0x635440`, `0x634C50`; docs/re/movement.md).
fn building(tables: &Tables, key: &SectorKey, head: u32, cell: u16) -> Result<Option<Building>> {
    for record in tables.statics.chain(head)? {
        let object = tables.statics.get(to_index(record)?)?;
        if object.flags & BUILDING == 0 {
            continue;
        }
        let (mut floors, mut heights) = (Vec::new(), Vec::new());
        for child in tables.statics.chain(object.first_child)? {
            let floor = tables.statics.get(to_index(child)?)?;
            if u32::from(floor.sector) != key.id {
                bail!(
                    "building {record}: floor {child} lies in sector {}",
                    floor.sector
                );
            }
            floors.push(u16::from(floor.layer));
            heights.push(u16::from(floor.height) * FLOOR_STEP);
        }
        return Ok(Some(Building {
            cell,
            floors,
            heights,
            open: object.flags & OPEN != 0,
        }));
    }
    Ok(None)
}

/// The objects of the chain from `head`, each with the objects of its child chain.
fn chain_objects(tables: &Tables, head: u32, cell: u16, depth: usize) -> Result<Vec<StaticObject>> {
    if depth > MAX_DEPTH {
        bail!("static {head}: children nested deeper than {MAX_DEPTH}");
    }
    let mut out = Vec::new();
    for record in tables.statics.chain(head)? {
        let object = tables.statics.get(to_index(record)?)?;
        let item = tables.items.get(to_index(object.kind)?)?;
        let miniature = match item {
            Some(item) if object.flags & MINIATURE != 0 => {
                let atlas = tables
                    .atlases
                    .get(&item.texture)
                    .copied()
                    .with_context(|| format!("static {record}: no atlas {}", item.texture))?;
                Some(Miniature {
                    atlas,
                    params: object.miniature,
                })
            }
            _ => None,
        };
        out.push(StaticObject {
            cell,
            kind: object.kind,
            flags: object.flags,
            x: object.x,
            y: object.y,
            record,
            type_flags: item.map_or(0, |i| i.flags),
            mask: object.mask,
            sprite: match (item, &miniature) {
                (Some(item), None) => item.sprite,
                _ => 0,
            },
            miniature,
            shadow: item
                .filter(|item| item.flags & CASTS_SHADOW != 0)
                .map(|item| {
                    let shadow = item.shadow;
                    Shadow {
                        cell: shadow.cell,
                        foot: shadow.half_foot.map(|half| 2 * i32::from(half)),
                        size: shadow.size,
                        leaning: shadow.shape != 0,
                    }
                }),
            floor: item
                .filter(|item| item.flags & FLOOR != 0)
                .map(|_| u16::from(object.height) * FLOOR_STEP),
            children: chain_objects(tables, object.first_child, cell, depth + 1)?,
        });
    }
    Ok(out)
}

fn to_index(record: u32) -> Result<usize> {
    usize::try_from(record).context("record index")
}

/// The path of `name`, relative to `world.ron`, relative to the masters' root.
fn master_path(name: &str) -> Result<String> {
    let path = format!("world/{name}");
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            ".." => {
                parts
                    .pop()
                    .with_context(|| format!("{name} lies outside the masters"))?;
            }
            "." | "" => {}
            part => parts.push(part),
        }
    }
    Ok(parts.join("/"))
}

/// What the world index `masters` says of the textures, relative to the masters' root
/// (`build`): those it names, the tile sheets and the miniatures' atlases, which the game composes
/// on the CPU (#700), and the atlases that the sprites' images were assembled from and that it
/// does not name, which the game reads no more (#838).
pub fn textures_of(masters: &str) -> Result<(Vec<String>, Vec<String>)> {
    let world: World = ron::from_str(masters).context("reading the masters' world.ron")?;
    let named = world
        .sheets
        .iter()
        .chain(&world.atlases)
        .map(|name| master_path(name))
        .collect::<Result<Vec<_>>>()?;
    let assembled = world
        .assembled_from
        .iter()
        .filter(|name| !world.atlases.contains(name))
        .map(|name| master_path(name))
        .collect::<Result<Vec<_>>>()?;
    Ok((named, assembled))
}

/// The game's world index of the masters' `masters` (`build`, #700): the sheets and the atlases
/// named as the KTX2 textures that `build` makes of the PNG masters, without the atlases that the
/// sprites were assembled from.
pub fn built_index(masters: &str) -> Result<String> {
    let mut world: World = ron::from_str(masters).context("reading the masters' world.ron")?;
    for name in world.sheets.iter_mut().chain(&mut world.atlases) {
        if let Some(stem) = name.strip_suffix(".png") {
            *name = format!("{stem}.{}", opensacred_data::texture::EXTENSION);
        }
    }
    world.assembled_from.clear();
    ron::ser::to_string_pretty(&world, crate::utf8::pretty())
        .context("writing the game's world.ron")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{repo_root, scratch_dir};

    #[test]
    fn file_names_are_grid_positions() {
        assert_eq!(file_name(50, 39), "50-39.sector");
    }

    #[test]
    fn the_game_index_names_the_ktx2_textures_of_the_masters_index() {
        let world = World {
            sectors: Vec::new(),
            sheets: vec!["../textures/texture/00007-sheet.png".to_owned()],
            atlases: vec!["../textures/texture/00009-atlas.png".to_owned()],
            assembled_from: vec![
                "../textures/texture/00008-mix1.444.png".to_owned(),
                "../textures/texture/00009-atlas.png".to_owned(),
            ],
        };
        let masters = ron::ser::to_string_pretty(&world, crate::utf8::pretty()).unwrap();
        // The atlas of a miniature that a sprite's pieces were cut from too stays.
        assert_eq!(
            textures_of(&masters).unwrap(),
            (
                vec![
                    "textures/texture/00007-sheet.png".to_owned(),
                    "textures/texture/00009-atlas.png".to_owned()
                ],
                vec!["textures/texture/00008-mix1.444.png".to_owned()]
            )
        );
        let text = built_index(&masters).unwrap();
        assert!(!text.contains("assembled_from"), "{text}");
        let built: World = ron::from_str(&text).unwrap();
        assert_eq!(built.sheets, ["../textures/texture/00007-sheet.ktx2"]);
        assert_eq!(built.atlases, ["../textures/texture/00009-atlas.ktx2"]);
        assert!(built.assembled_from.is_empty());
    }

    #[test]
    fn a_sprite_is_assembled_from_its_pieces_over_its_rectangle() {
        // Two atlases of 8 × 8 texels, red and blue, in a texture.pak.
        let atlas = |name: &'static [u8], [r, g, b, a]: [u8; 4]| crate::testing::Tex {
            name,
            width: 8,
            height: 8,
            format: 6,
            data: [b, g, r, a].repeat(64),
        };
        let pak = crate::testing::tex_pak(&[
            Some(atlas(b"MIX0.444", [255, 0, 0, 255])),
            Some(atlas(b"MIX1.444", [0, 0, 255, 255])),
        ]);
        let pak = TexPak::parse(&pak).unwrap();
        // Two pieces side by side from the two atlases, and a third below them that runs past the
        // sprite's size, which counts from the pieces' smallest corner.
        let piece = |texture, source, rect| Piece {
            texture,
            source,
            rect,
        };
        let sprite = Pieces {
            record: 1,
            width: 8,
            height: 5,
            pieces: vec![
                piece(0, [4, 4, 8, 8], [2, 1, 6, 5]),
                piece(1, [0, 0, 4, 4], [6, 1, 10, 5]),
                piece(1, [0, 0, 8, 2], [2, 5, 10, 7]),
            ],
        };
        // Cut at the size: the third piece's second row goes.
        assert_eq!(sprite.rect(), [2, 1, 10, 6]);
        let mut atlases = HashMap::new();
        let (width, height, pixels) = assemble(&sprite, &pak, &mut atlases).unwrap();
        assert_eq!((width, height, pixels.len()), (8, 5, 4 * 8 * 5));
        assert_eq!(atlases.len(), 2);
        let pixel = |x: usize, y: usize| &pixels[4 * (y * 8 + x)..4 * (y * 8 + x) + 4];
        assert_eq!(pixel(3, 3), [255, 0, 0, 255]);
        assert_eq!(pixel(4, 0), [0, 0, 255, 255]);
        assert_eq!(pixel(7, 4), [0, 0, 255, 255]);
    }

    /// Sector 45,52 holds walls and towers of a castle, buildings of flag 0x400 of two floors
    /// (docs/re/drawing.md, "Buildings: masks and layers", #149); the start's are closed.
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn the_castles_walls_are_open_buildings() {
        // Counted on Static.PAK for #149: the buildings of flag 0x400 whose object lies there.
        const CASTLE_OPEN_BUILDINGS: usize = 22;
        let out = scratch_dir("world-open");
        let game = repo_root().join("OriginalGame");
        convert(&game, &out, Some((45, 52))).unwrap();
        let sectors = output_dir(&out).join("sectors");
        let sector = Sector::decode(&fs::read(sectors.join("45-52.sector")).unwrap()).unwrap();
        let open: Vec<_> = sector.buildings.iter().filter(|b| b.open).collect();
        assert_eq!(open.len(), CASTLE_OPEN_BUILDINGS, "{open:?}");
        assert!(open.iter().all(|b| b.floors.len() == 2), "{open:?}");
        convert(&game, &out, Some((50, 39))).unwrap();
        let start = Sector::decode(&fs::read(sectors.join("50-39.sector")).unwrap()).unwrap();
        assert!(start.buildings.iter().all(|b| !b.open));
    }

    /// The starting sector converts and loads back; the village has its overlays and objects.
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn the_starting_sector_converts_and_loads() {
        let out = scratch_dir("world");
        let summary = convert(&repo_root().join("OriginalGame"), &out, Some((50, 39))).unwrap();
        assert_eq!((summary.sectors, summary.missing_sheets), (1, 0));
        let dir = output_dir(&out);
        let file = fs::read(dir.join("sectors").join("50-39.sector")).unwrap();
        let sector = Sector::decode(&file).unwrap();
        assert_eq!((sector.x, sector.y, sector.id), (50, 39, 2579));
        assert_eq!(sector.cells.len(), 4096);
        assert_eq!(count(&sector.statics, &|_| true), summary.statics);
        // The manor, the pond, the trees and the garden: most objects have a sprite.
        assert!(
            summary.sprite_statics > 500 && summary.miniatures > 0,
            "{summary:?}"
        );
        // The buildings, each floor an inside layer of the sector; the monastery the hero starts
        // in, whose cells point to its anchor, of one floor (docs/re/movement.md).
        assert!(!sector.buildings.is_empty());
        for building in &sector.buildings {
            assert!(
                building
                    .floors
                    .iter()
                    .all(|&f| f >= 1 && usize::from(f) <= sector.layers.len()),
                "{building:?}"
            );
        }
        let start = &sector.cells[(2511 - 39 * 64) * 64 + (3236 - 50 * 64)];
        let anchor = (
            (3236 - 50 * 64) + i32::from(start.offsets[0]),
            (2511 - 39 * 64) + i32::from(start.offsets[1]),
        );
        let monastery = sector
            .buildings
            .iter()
            .find(|b| (i32::from(b.cell) % 64, i32::from(b.cell) / 64) == anchor)
            .unwrap();
        assert_eq!(monastery.floors.len(), MONASTERY_FLOORS);
        // Every building's floor 1 at the ground's height, as many heights as floors.
        for building in &sector.buildings {
            assert_eq!(building.heights.len(), building.floors.len());
            assert_eq!(building.heights.first(), Some(&0), "{building:?}");
        }
        // The statics that cast a shadow carry their type's: the benches' a rectangle.
        const BENCH_2: u32 = 9181;
        // Counted on Static.PAK and items.pak for #100.
        const SHADOWS_AT_START: usize = 267;
        const MONASTERY_FLOORS: usize = 1;
        // The atlases of the sprites' pieces (docs/02 §3.3).
        const MIX_ATLASES: usize = 10_585;
        let shadows = count(&sector.statics, &|o| o.shadow.is_some());
        assert_eq!(shadows, SHADOWS_AT_START);
        let mut all = Vec::new();
        fn flatten<'a>(list: &'a [StaticObject], out: &mut Vec<&'a StaticObject>) {
            for o in list {
                out.push(o);
                flatten(&o.children, out);
            }
        }
        flatten(&sector.statics, &mut all);
        let bench = all.iter().find(|o| o.kind == BENCH_2).unwrap();
        assert_eq!(
            bench.shadow,
            Some(Shadow {
                cell: 74,
                foot: [78, 100],
                size: 80,
                leaning: false
            })
        );
        let sprites = Sprites::decode(&fs::read(dir.join(SPRITES_FILE)).unwrap()).unwrap();
        assert_eq!(sprites.sprites.len(), 32_096);
        assert_eq!(sprites.sprites.iter().flatten().count(), 16_255);
        // Each sprite's image as large as its rectangle, within its size (#838).
        for (record, sprite) in sprites.sprites.iter().enumerate().step_by(97) {
            let Some(sprite) = sprite else { continue };
            let record = u32::try_from(record).unwrap();
            let file = fs::File::open(dir.join(sprite_image(record, "png"))).unwrap();
            let info = png::Decoder::new(std::io::BufReader::new(file))
                .read_info()
                .unwrap();
            let [x0, y0, x1, y1] = sprite.rect;
            assert_eq!(
                (info.info().width, info.info().height),
                (u32::from(x1 - x0), u32::from(y1 - y0)),
                "sprite {record}"
            );
            assert!(x1 - x0 <= sprite.width && y1 - y0 <= sprite.height);
        }
        assert_eq!(summary.assembled_from, MIX_ATLASES);
        assert!(
            summary.overlays > 1000 && summary.statics > 500,
            "{summary:?}"
        );
        let world: World = ron::from_str(&fs::read_to_string(dir.join(INDEX)).unwrap()).unwrap();
        assert_eq!((world.sectors.len(), world.sheets.len()), (6050, 5008));
        let start = world
            .sectors
            .iter()
            .find(|e| (e.x, e.y) == (50, 39))
            .unwrap();
        assert_eq!(start.file, "sectors/50-39.sector");
        assert!(
            world
                .sheets
                .iter()
                .all(|s| s.starts_with("../textures/texture/"))
        );
        assert_eq!(world.atlases.len(), summary.atlases);
        fs::remove_dir_all(out).unwrap();
    }
}
