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
