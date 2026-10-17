//! Tests on the GOG installation files: every string of `global.res` against the table of the
//! Python prototype `tools/analysis/formats/global_res_dump.py`. CI does not have the game files,
//! so the test is ignored by default.

use std::ffi::OsStr;
use std::fs;

use super::*;
use crate::testing::{game_file, repo_root, run_python, scratch_dir};

#[test]
#[ignore = "needs the game files in OriginalGame/ and Python"]
fn strings_match_global_res_dump_py() {
    let out = scratch_dir("text");
    let table = out.join("strings.tsv");
    let game_dir = repo_root().join("OriginalGame");
    run_python(
        "tools/analysis/formats/global_res_dump.py",
        &[
            OsStr::new("--game-dir"),
            game_dir.as_os_str(),
            OsStr::new("--records"),
            table.as_os_str(),
        ],
    );
    let data = game_file("scripts/us/global.res");
    let res = GlobalRes::parse(&data).unwrap();
    let text = fs::read_to_string(&table).unwrap();
    let mut lines = text.lines().skip(1);
    for entry in res.entries() {
        let string = entry.text().unwrap();
        let ours = format!(
            "{}\t{}\t{}\t{}",
            entry.id,
            entry.flags,
            entry.utf16.len(),
            crc32fast::hash(string.as_bytes())
        );
        assert_eq!(lines.next().unwrap(), ours);
    }
    assert!(lines.next().is_none());
    assert_eq!(res.entries().len(), 23_123);
    // The hero names, `res:1` .. `res:5`, by the hash of their number.
    assert_eq!(
        res.get(sacred_hash(b"1")).unwrap().text().unwrap(),
        "Seraphim"
    );
    fs::remove_dir_all(out).unwrap();
}
