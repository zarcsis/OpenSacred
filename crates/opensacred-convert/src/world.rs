//! `world` subcommand: `World/sectors.keyx` and `sectors.wldx` with `Static.PAK`, `Floor.PAK`,
//! `PAK/tiles.pak`, `PAK/items.pak` and `PAK/mixed.pak` → `<out>/world/world.ron`,
//! `<out>/world/sectors/<x>-<y>.sector` and `<out>/world/sprites.bin` (docs/02 §5; the format is
//! [`opensacred_data::world`]). Tile sheets and atlases are referred to as the PNGs that
//! `opensacred-convert textures` writes to `<out>/textures/texture/`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use opensacred_data::world::{
    Cell, INDEX, Layer, LayerCell, Miniature, Overlay, Piece, SPRITES_FILE, Sector, SectorEntry,
    Sprite, Sprites, StaticObject, World,
};
use opensacred_formats::items::Items;
use opensacred_formats::mixed::Sprites as MixedPak;
use opensacred_formats::tex::TexPak;
use opensacred_formats::world::{Floors, Keyx, SectorKey, Statics, Tiles};
use rayon::prelude::*;

use crate::textures;
use crate::verify::read_game_file;

/// Deepest nesting of child objects followed; the game's deepest is 1.
const MAX_DEPTH: usize = 8;
/// Flag of a miniature static: a square of its type's atlas instead of a sprite.
const MINIATURE: u32 = 0x20;

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
    /// Sprites with pieces, and the atlases of sprites and miniatures.
    pub sprites: usize,
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

    let mut atlases = Atlases::default();
    let sprites = convert_sprites(&mixed, &texture_pak, &mut atlases).context("PAK/mixed.pak")?;
    summary.sprites = sprites.sprites.iter().flatten().count();
    // The atlases of the miniature types, so that every sector finds them.
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
        })
        .collect();
    entries.sort_by_key(|e| (e.y, e.x));
    let world = World {
        sectors: entries,
        sheets,
        atlases: atlases.paths,
    };
    let text = ron::ser::to_string_pretty(&world, ron::ser::PrettyConfig::default())
        .context("writing world.ron")?;
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

/// The sprites of `mixed.pak`, their pieces' atlases added to `atlases`.
fn convert_sprites(mixed: &MixedPak, pak: &TexPak, atlases: &mut Atlases) -> Result<Sprites> {
    let mut sprites = vec![None];
    for index in 1..mixed.len() {
        let sprite = mixed.get(index)?;
        if sprite.pieces.is_empty() {
            sprites.push(None);
            continue;
        }
        let mut pieces = Vec::with_capacity(sprite.pieces.len());
        for piece in &sprite.pieces {
            let atlas = atlases.add(piece.texture, pak)?;
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
                atlas,
                source,
                rect: piece.rect,
            });
        }
        sprites.push(Some(Sprite {
            width: sprite.width,
            height: sprite.height,
            pieces,
        }));
    }
    Ok(Sprites { sprites })
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
    })
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
            children: chain_objects(tables, object.first_child, cell, depth + 1)?,
        });
    }
    Ok(out)
}

fn to_index(record: u32) -> Result<usize> {
    usize::try_from(record).context("record index")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{repo_root, scratch_dir};

    #[test]
    fn file_names_are_grid_positions() {
        assert_eq!(file_name(50, 39), "50-39.sector");
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
        let sprites = Sprites::decode(&fs::read(dir.join(SPRITES_FILE)).unwrap()).unwrap();
        assert_eq!(sprites.sprites.len(), 32_096);
        assert_eq!(sprites.sprites.iter().flatten().count(), 16_255);
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
