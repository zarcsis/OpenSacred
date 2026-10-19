//! Tests on the GOG installation files: every sector of `sectors.wldx` reads, every chain of
//! `Static.PAK` and `Floor.PAK` reads, and every sector matches the Python prototype
//! `tools/analysis/world/world_dump.py`. CI does not have the game files, so every test here is
//! ignored by default.

use std::ffi::OsStr;
use std::fs;

use crc32fast::Hasher;

use super::*;
use crate::testing::{game_file, repo_root, run_python, scratch_dir};

/// The 32 bytes of a cell, rebuilt from its fields: the fields cover the whole record.
fn cell_bytes(cell: &Cell) -> [u8; 32] {
    let mut out = [0; 32];
    for (k, word) in [cell.tile, cell.first_static, 0, cell.first_floor]
        .iter()
        .enumerate()
    {
        out[4 * k..4 * k + 4].copy_from_slice(&word.to_le_bytes());
    }
    let bytes = |v: [i8; 4]| v.map(|x| x.to_le_bytes()[0]);
    out[0x10..0x14].copy_from_slice(&bytes(cell.heights));
    out[0x14..0x18].copy_from_slice(&cell.light);
    out[0x18..0x1C].copy_from_slice(&bytes(cell.heights2));
    out[0x1C] = cell.offsets[0].to_le_bytes()[0];
    out[0x1D] = cell.offsets[1].to_le_bytes()[0];
    out[0x1E] = cell.flags;
    out[0x1F] = cell.ground;
    out
}

/// Every sector against the `--records` table of `world_dump.py`: the inflated size, the third
/// chunk, the liquids, the CRC-32 of the cells rebuilt from their fields, and the chains of the
/// cells: the number of records and the CRC-32 over the cells in order of every chained record
/// (the cell index, then the record as stored), each static followed by its child chain; and
/// the inside layers: their number and the CRC-32 over their places, sizes and cells.
#[test]
#[ignore = "needs the game files in OriginalGame/ and Python with tools/requirements.txt"]
fn sectors_and_chains_match_world_dump_py() {
    let out = scratch_dir("world");
    let table = out.join("sectors.tsv");
    let game_dir = repo_root().join("OriginalGame");
    run_python(
        "tools/analysis/world/world_dump.py",
        &[
            OsStr::new("--game-dir"),
            game_dir.as_os_str(),
            OsStr::new("--records"),
            table.as_os_str(),
        ],
    );
    let text = fs::read_to_string(&table).unwrap();
    let mut lines = text.lines();
    let columns: Vec<_> = lines.next().unwrap().split('\t').collect();
    assert_eq!(
        columns,
        [
            "index",
            "x",
            "y",
            "id",
            "inflated",
            "tail",
            "liquids",
            "cells_crc",
            "tail_crc",
            "statics",
            "static_crc",
            "floors",
            "floor_crc",
            "layers",
            "layers_crc",
            "area",
            "music",
            "dungeon",
            "buildings",
            "buildings_crc"
        ]
    );

    let keyx_data = game_file("World/sectors.keyx");
    let wldx = game_file("World/sectors.wldx");
    let static_data = game_file("World/Static.PAK");
    let floor_data = game_file("World/Floor.PAK");
    let keyx = Keyx::parse(&keyx_data).unwrap();
    let statics = Statics::parse(&static_data).unwrap();
    let floors = Floors::parse(&floor_data).unwrap();
    assert_eq!(
        (keyx.sectors.len(), statics.len(), floors.len()),
        (6050, 1_038_014, 6_713_136)
    );

    let mut chained = (0, 0);
    let mut inside = 0;
    for key in &keyx.sectors {
        let sector = key.read(&wldx).unwrap();
        let mut cells = Hasher::new();
        for cell in &sector.cells {
            cells.update(&cell_bytes(cell));
        }
        let (mut static_crc, mut floor_crc) = (Hasher::new(), Hasher::new());
        let (mut n_statics, mut n_floors) = (0, 0);
        for (index, cell) in sector.cells.iter().enumerate() {
            let index = u32::try_from(index).unwrap().to_le_bytes();
            for head in statics.chain(cell.first_static).unwrap() {
                let first_child = statics.get(to_usize(head)).unwrap().first_child;
                for record in std::iter::once(head).chain(statics.chain(first_child).unwrap()) {
                    static_crc.update(&index);
                    static_crc.update(statics.bytes(to_usize(record)).unwrap());
                    n_statics += 1;
                }
            }
            for record in floors.chain(cell.first_floor).unwrap() {
                floor_crc.update(&index);
                floor_crc.update(floors.bytes(to_usize(record)).unwrap());
                n_floors += 1;
            }
        }
        chained.0 += n_statics;
        chained.1 += n_floors;
        let mut layers = Hasher::new();
        for layer in &sector.layers {
            layers.update(&layer.x.to_le_bytes());
            layers.update(&layer.y.to_le_bytes());
            layers.update(&layer.width.to_le_bytes());
            layers.update(&layer.height.to_le_bytes());
            for cell in &layer.cells {
                layers.update(&cell_bytes(cell));
            }
        }
        inside += sector.layers.len();
        // The buildings: the first static of flag 0x10 in a cell's chain, its children its floors,
        // each with the inside layer that holds its grid.
        let (mut buildings, mut buildings_crc) = (0, Hasher::new());
        for (index, cell) in sector.cells.iter().enumerate() {
            let chain = statics.chain(cell.first_static).unwrap();
            let Some(building) = chain
                .iter()
                .map(|&k| statics.get(to_usize(k)).unwrap())
                .find(|s| s.flags & 0x10 != 0)
            else {
                continue;
            };
            let floors = statics.chain(building.first_child).unwrap();
            buildings += 1;
            buildings_crc.update(&u32::try_from(index).unwrap().to_le_bytes());
            buildings_crc.update(&u32::try_from(floors.len()).unwrap().to_le_bytes());
            for floor in floors {
                buildings_crc.update(&[statics.get(to_usize(floor)).unwrap().layer]);
            }
        }
        let ours = [
            (key.y * GRID + key.x).to_string(),
            key.x.to_string(),
            key.y.to_string(),
            key.id.to_string(),
            key.inflated.to_string(),
            sector.tail.len().to_string(),
            format!("{},{}", key.liquids[0], key.liquids[1]),
            cells.finalize().to_string(),
            crc32fast::hash(&sector.tail).to_string(),
            n_statics.to_string(),
            static_crc.finalize().to_string(),
            n_floors.to_string(),
            floor_crc.finalize().to_string(),
            sector.layers.len().to_string(),
            layers.finalize().to_string(),
            key.area.to_string(),
            key.music.to_string(),
            u8::from(key.dungeon).to_string(),
            buildings.to_string(),
            buildings_crc.finalize().to_string(),
        ];
        let row: Vec<_> = lines.next().unwrap().split('\t').collect();
        assert_eq!(row, ours, "sector ({}, {})", key.x, key.y);
    }
    assert!(lines.next().is_none());
    // Every static but record 0 and two orphans, every overlay but record 0 (docs/02 §5.5).
    assert_eq!(chained, (1_038_011, 6_713_135));
    // The buildings' inside layers (docs/02 §5.2).
    assert_eq!(inside, 1_870);
    fs::remove_dir_all(out).unwrap();
}

/// The tiles of the starting sector (50, 39) name sheets of `texture.pak`.
#[test]
#[ignore = "needs the game files in OriginalGame/"]
fn the_starting_sector_reads_with_its_tiles() {
    let keyx_data = game_file("World/sectors.keyx");
    let wldx = game_file("World/sectors.wldx");
    let tiles_data = game_file("PAK/tiles.pak");
    let keyx = Keyx::parse(&keyx_data).unwrap();
    let tiles = Tiles::parse(&tiles_data).unwrap();
    assert_eq!(tiles.len(), 90_132);
    let key = keyx.sector(50, 39).unwrap();
    assert_eq!(key.id, 2579);
    let sector = key.read(&wldx).unwrap();
    assert!(sector.name.starts_with(b"Sector 50,39\0"));
    for cell in &sector.cells {
        let tile = tiles.get(to_usize(cell.tile)).unwrap();
        assert!(tile.name.to_ascii_lowercase().starts_with(b"iso"));
    }
}
