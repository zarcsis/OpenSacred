//! Tests on the GOG installation files: every table of `bin/` against the fingerprints of the
//! Python prototype `tools/analysis/formats/bin_tables_dump.py`. CI does not have the game files,
//! so the test is ignored by default.

use std::ffi::OsStr;
use std::fs;

use super::*;
use crate::testing::{game_file, repo_root, run_python, scratch_dir};

fn line<T: ToString>(values: impl IntoIterator<Item = T>) -> String {
    values
        .into_iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `canonical` of the prototype: one line per record.
fn lines(path: &str, data: &[u8]) -> Vec<String> {
    match path {
        "bin/Balance.bin" => {
            let b = balance(data).unwrap();
            assert_eq!((b.image.len(), b.rest.len()), (1224, 4857));
            vec![line(b.image), line(b.rest)]
        }
        "bin/World.bin" => world_index(data).unwrap().into_iter().map(line).collect(),
        "bin/World2.bin" => vec![line(world_map(data).unwrap())],
        "bin/sets.bin" => item_sets(data)
            .unwrap()
            .into_iter()
            .map(|s| match s {
                None => "-".to_owned(),
                Some(s) => {
                    let mut v: Vec<u32> = s.items.to_vec();
                    v.push(s.hash);
                    v.extend(s.bytes.map(u32::from));
                    line(v)
                }
            })
            .collect(),
        "bin/wpmod.bin" => weapon_modifiers(data)
            .unwrap()
            .into_iter()
            .map(|m| {
                let entries: Vec<String> = m.entries.into_iter().map(line).collect();
                format!("{} | {}", line(m.words), entries.join(" "))
            })
            .collect(),
        "bin/wea.bin" => weapon_groups(data).unwrap().into_iter().map(line).collect(),
        "bin/Rust.bin" => armour_switches(data)
            .unwrap()
            .into_iter()
            .map(|pairs| line(pairs.into_iter().flatten()))
            .collect(),
        "bin/static10_18.bin" => static_remap(data).unwrap().into_iter().map(line).collect(),
        "bin/treppe.bin" => rows(data).unwrap().into_iter().map(line).collect(),
        "bin/merc.bin" | "bin/Addon/merc.bin" => rows(data)
            .unwrap()
            .into_iter()
            .map(|r| line(&r[1..]))
            .collect(),
        "bin/MultiStart.bin" => rows(data)
            .unwrap()
            .into_iter()
            .map(|r| line(&r[1..3]))
            .collect(),
        other => panic!("{other}"),
    }
}

#[test]
#[ignore = "needs the game files in OriginalGame/ and Python"]
fn every_table_matches_bin_tables_dump_py() {
    let out = scratch_dir("bin-tables");
    let rows_path = out.join("rows.tsv");
    let game = repo_root().join("OriginalGame");
    run_python(
        "tools/analysis/formats/bin_tables_dump.py",
        &[
            OsStr::new("--game-dir"),
            game.as_os_str(),
            OsStr::new("--rows"),
            rows_path.as_os_str(),
        ],
    );
    let theirs = fs::read_to_string(&rows_path).unwrap();
    let theirs: Vec<&str> = theirs.lines().skip(1).collect();
    assert_eq!(theirs.len(), 12);
    for row in theirs {
        let path = row.split('\t').next().unwrap();
        let lines = lines(path, &game_file(path));
        let ours = format!(
            "{path}\t{}\t{:08x}",
            lines.len(),
            crc32fast::hash(lines.join("\n").as_bytes())
        );
        assert_eq!(ours, row);
    }
    fs::remove_dir_all(out).unwrap();
}

/// The key table against `tools/analysis/pe/balance_keys.py`'s, read off the client's code; and
/// the variables it names in `Balance.bin` (docs/02 §6.3).
#[test]
#[ignore = "needs the game files in OriginalGame/ and Python with tools/requirements.txt"]
fn the_balance_keys_match_balance_keys_py() {
    let out = scratch_dir("balance-keys");
    let table = out.join("keys.tsv");
    let game = repo_root().join("OriginalGame");
    run_python(
        "tools/analysis/pe/balance_keys.py",
        &[
            OsStr::new("--game-dir"),
            game.as_os_str(),
            OsStr::new("--table"),
            table.as_os_str(),
        ],
    );
    let theirs = fs::read_to_string(&table).unwrap();
    let ours: Vec<String> = BALANCE_KEYS
        .iter()
        .map(|k| {
            let kind = match k.kind {
                Kind::F32 => "f32",
                Kind::I32 => "i32",
            };
            format!("{}\t{}\t{kind}", k.name, k.offset)
        })
        .collect();
    assert_eq!(theirs.lines().skip(1).collect::<Vec<_>>(), ours);
    fs::remove_dir_all(out).unwrap();
    let b = balance(&game_file("bin/Balance.bin")).unwrap();
    let f32_of = |name: &str| {
        let key = BALANCE_KEYS.iter().find(|k| k.name == name).unwrap();
        assert_eq!(key.kind, Kind::F32);
        f32::from_bits(b.word(key).unwrap())
    };
    assert_eq!(
        (
            f32_of("GameSpeed"),
            f32_of("TransSize"),
            f32_of("TransValu")
        ),
        (1.25, 190.0, 50.0)
    );
    let skill = BALANCE_KEYS
        .iter()
        .find(|k| k.name == "skilllearn2")
        .unwrap();
    assert_eq!((skill.kind, b.word(skill)), (Kind::I32, Some(6)));
}
