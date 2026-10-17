//! Tests on the GOG installation files: every record of the tables against the rows of the Python
//! prototype `tools/analysis/formats/tables_dump.py`, written back to its bytes. CI does not have
//! the game files, so the test is ignored by default.

use std::ffi::OsStr;
use std::fs;

use super::tests::{creature_bytes, motion_bytes, trigger_bytes, weapon_bytes};
use super::*;
use crate::items::Items;
use crate::testing::{game_file, repo_root, run_python, scratch_dir};

fn hex(text: &str) -> String {
    text.chars()
        .map(|c| format!("{:02x}", u32::from(c)))
        .collect()
}

#[test]
#[ignore = "needs the game files in OriginalGame/ and Python"]
fn every_record_matches_tables_dump_py() {
    let out = scratch_dir("tables");
    let rows = out.join("rows.tsv");
    let game = repo_root().join("OriginalGame");
    run_python(
        "tools/analysis/formats/tables_dump.py",
        &[
            OsStr::new("--game-dir"),
            game.as_os_str(),
            OsStr::new("--rows"),
            rows.as_os_str(),
        ],
    );
    let crc = |bytes: &[u8]| format!("{:08x}", crc32fast::hash(bytes));
    let mut ours = Vec::new();
    for (i, c) in creatures(&game_file("PAK/Creature.pak"))
        .unwrap()
        .iter()
        .enumerate()
    {
        let bytes = creature_bytes(c);
        ours.push(format!(
            "creature\t{i}\t{}\t{}\t{}\t{}",
            c.id,
            c.class,
            c.flags,
            crc(&bytes)
        ));
    }
    for (i, w) in weapons(&game_file("PAK/Weapon.pak"))
        .unwrap()
        .iter()
        .enumerate()
    {
        ours.push(format!(
            "weapon\t{i}\t{}\t{}\t{}\t{}",
            w.id,
            w.parent,
            hex(&w.name),
            crc(&weapon_bytes(w))
        ));
    }
    for (i, m) in motions(&game_file("PAK/MOTIONS.PAK"))
        .unwrap()
        .iter()
        .enumerate()
    {
        let bytes = motion_bytes(u16::try_from(i).unwrap(), m);
        ours.push(format!(
            "motion\t{i}\t{}\t{}\t{}",
            m.value_06,
            m.value_be,
            crc(&bytes)
        ));
    }
    let triggers = triggers(&game_file("World/Triggers.PAK")).unwrap();
    assert_eq!(triggers.iter().flatten().count(), 1_246);
    for (i, t) in triggers.iter().enumerate() {
        let (id, target) = t.map_or((0, 0), |t| (t.id, t.target));
        ours.push(format!(
            "trigger\t{i}\t{id}\t{target}\t{}",
            crc(&trigger_bytes(t.as_ref()))
        ));
    }
    let data = game_file("PAK/Items03.pak");
    let items = Items::parse(&data).unwrap();
    for i in 0..items.len() {
        if let Some(record) = items.record(i).unwrap() {
            ours.push(format!(
                "items03\t{i}\t{}\t{}\t{}",
                record.kind,
                hex(&record.name),
                crc(items.bytes(i).unwrap())
            ));
        }
    }
    let theirs = fs::read_to_string(&rows).unwrap();
    let theirs: Vec<&str> = theirs.lines().skip(1).collect();
    assert_eq!(ours.len(), theirs.len());
    for (a, b) in ours.iter().zip(&theirs) {
        assert_eq!(a, b);
    }
    // Every used slot of items.pak reads with every field.
    let data = game_file("PAK/Items.pak");
    let items = Items::parse(&data).unwrap();
    let used = (0..items.len())
        .filter_map(|i| items.record(i).unwrap())
        .count();
    assert_eq!(used, 17_408);
    fs::remove_dir_all(out).unwrap();
}
