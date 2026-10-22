//! Tests on the GOG installation files: every section of the hero templates against the rows of
//! the Python prototype `tools/analysis/formats/amh_dump.py`. The saves of `save/` are left out:
//! the game rewrites them. CI does not have the game files, so the test is ignored by default.

use std::ffi::OsStr;
use std::fs;

use super::*;
use crate::testing::{game_file, repo_root, run_python, scratch_dir};

#[test]
#[ignore = "needs the game files in OriginalGame/ and Python"]
fn every_section_matches_amh_dump_py() {
    let out = scratch_dir("amh");
    let rows = out.join("rows.tsv");
    let game = repo_root().join("OriginalGame");
    run_python(
        "tools/analysis/formats/amh_dump.py",
        &[
            OsStr::new("--game-dir"),
            game.as_os_str(),
            OsStr::new("--rows"),
            rows.as_os_str(),
        ],
    );
    let mut ours = Vec::new();
    for path in (0..8).map(|i| format!("Templates/Hero{i:02}.ptx")) {
        let amh = Amh::parse(&game_file(&path)).unwrap_or_else(|e| panic!("{path}: {e}"));
        let summary = amh.summary().unwrap();
        assert!((1..=9).contains(&summary.class), "{path}");
        assert!(summary.name.is_empty(), "{path}");
        for s in &amh.sections {
            ours.push(format!(
                "{path}\t{}\t{}\t{}\t{}\t{}\t{:08x}",
                s.slot,
                s.tag,
                s.size,
                u8::from(s.compressed),
                s.data.len(),
                crc32fast::hash(&s.data)
            ));
        }
    }
    let theirs = fs::read_to_string(&rows).unwrap();
    assert_eq!(ours, theirs.lines().skip(1).collect::<Vec<_>>());
    fs::remove_dir_all(out).unwrap();
}

#[test]
#[ignore = "needs the game files in OriginalGame/ and Python"]
fn every_item_matches_amh_dump_py() {
    let out = scratch_dir("amh-items");
    let rows = out.join("items.tsv");
    let game = repo_root().join("OriginalGame");
    run_python(
        "tools/analysis/formats/amh_dump.py",
        &[
            OsStr::new("--game-dir"),
            game.as_os_str(),
            OsStr::new("--items"),
            rows.as_os_str(),
        ],
    );
    let mut ours = Vec::new();
    for path in (0..8).map(|i| format!("Templates/Hero{i:02}.ptx")) {
        let amh = Amh::parse(&game_file(&path)).unwrap_or_else(|e| panic!("{path}: {e}"));
        let packed = amh.backpack().unwrap();
        for item in amh.items().unwrap() {
            ours.push(format!(
                "{path}\t{}\t{}\t{}\t{:08x}",
                item.id,
                item.item,
                u8::from(packed.contains(&item.id)),
                crc32fast::hash(&item.record)
            ));
        }
    }
    let theirs = fs::read_to_string(&rows).unwrap();
    assert_eq!(ours, theirs.lines().skip(1).collect::<Vec<_>>());
    // The templates wear 14 items, one to three each (docs/02 §9).
    assert_eq!(ours.len(), 46);
    assert_eq!(ours.iter().filter(|row| row.contains("\t0\t")).count(), 14);
    fs::remove_dir_all(out).unwrap();
}
