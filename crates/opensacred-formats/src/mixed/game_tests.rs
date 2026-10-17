//! Tests on the GOG installation files: every record of `items.pak` and `mixed.pak` against the
//! tables of the Python prototype `tools/analysis/formats/mixed_dump.py`. CI does not have the game
//! files, so the test is ignored by default.

use std::ffi::OsStr;
use std::fs;

use super::*;
use crate::items::Items;
use crate::testing::{game_file, repo_root, run_python, scratch_dir};

#[test]
#[ignore = "needs the game files in OriginalGame/ and Python with tools/requirements.txt"]
fn items_and_sprites_match_mixed_dump_py() {
    let out = scratch_dir("mixed");
    let (items_table, sprites_table) = (out.join("items.tsv"), out.join("sprites.tsv"));
    let game_dir = repo_root().join("OriginalGame");
    run_python(
        "tools/analysis/formats/mixed_dump.py",
        &[
            OsStr::new("--game-dir"),
            game_dir.as_os_str(),
            OsStr::new("--items"),
            items_table.as_os_str(),
            OsStr::new("--sprites"),
            sprites_table.as_os_str(),
        ],
    );

    let data = game_file("PAK/items.pak");
    let items = Items::parse(&data).unwrap();
    let text = fs::read_to_string(&items_table).unwrap();
    let mut lines = text.lines().skip(1);
    for index in 0..items.len() {
        let crc = crc32fast::hash(items.bytes(index).unwrap());
        let ours = match items.get(index).unwrap() {
            Some(item) => format!("{index}\t1\t{}\t{}\t{crc}", item.kind, item.sprite),
            None => format!("{index}\t0\t0\t0\t{crc}"),
        };
        assert_eq!(lines.next().unwrap(), ours);
    }
    assert!(lines.next().is_none());

    let data = game_file("PAK/mixed.pak");
    let sprites = Sprites::parse(&data).unwrap();
    let text = fs::read_to_string(&sprites_table).unwrap();
    let mut lines = text.lines().skip(1);
    let mut pieces = 0;
    for index in 0..sprites.len() {
        let crc = crc32fast::hash(sprites.bytes(index).unwrap());
        let ours = if index == 0 {
            format!("0\t0\t0\t0\t{crc}")
        } else {
            let sprite = sprites.get(index).unwrap();
            pieces += sprite.pieces.len();
            format!(
                "{index}\t{}\t{}\t{}\t{crc}",
                sprite.pieces.len(),
                sprite.width,
                sprite.height
            )
        };
        assert_eq!(lines.next().unwrap(), ours);
    }
    assert!(lines.next().is_none());
    assert_eq!((sprites.len(), pieces), (32_096, 209_956));
    fs::remove_dir_all(out).unwrap();
}
