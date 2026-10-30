//! Tests on the GOG installation files: every table of `bin/` against the fingerprints of the
//! Python prototype `tools/analysis/formats/bin_tables_dump.py`. CI does not have the game files,
//! so the test is ignored by default.

use std::ffi::OsStr;
use std::fs;

use super::*;
use crate::testing::{game_file, run_python, scratch_dir};

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
            let t = b.tail;
            let mut lines = vec![line(b.image)];
            lines.extend(t.lists.into_iter().map(|(key, records)| {
                line([key].into_iter().chain(records.into_iter().flatten()))
            }));
            lines.push(line(t.weights.into_iter().flat_map(|(k, w)| [k, w])));
            lines.push(line(t.qualities.into_iter().flat_map(|(k, v)| [k, v])));
            lines.extend(
                t.rows
                    .into_iter()
                    .map(|(key, words)| line([key].into_iter().chain(words))),
            );
            lines.extend(t.chances.into_iter().map(|(effect, floats)| {
                line([effect].into_iter().chain(floats.map(f32::to_bits)))
            }));
            lines.push(line(t.rest));
            lines
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
    let game = opensacred_locate::original();
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
#[ignore = "needs the game files in OriginalGame/ and the Python of pyproject.toml through uv"]
fn the_balance_keys_match_balance_keys_py() {
    let out = scratch_dir("balance-keys");
    let table = out.join("keys.tsv");
    let game = opensacred_locate::original();
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

/// The tail of `Balance.bin` (docs/02 §6.3, #413): its five tables end at byte 22,048, and hold
/// the drops' weights of the quality keys, the quality pass's values and the effects' chance
/// parameters; `GoldProLevel`'s two values are named.
#[test]
#[ignore = "needs the game files in OriginalGame/"]
fn the_tail_of_balance_bin_holds_the_drops_tables() {
    let data = game_file("bin/Balance.bin");
    let b = balance(&data).unwrap();
    let t = &b.tail;
    assert_eq!(
        (
            t.lists.len(),
            t.weights.len(),
            t.qualities.len(),
            t.rows.len(),
            t.chances.len()
        ),
        (33, 18, 16, 9, 5)
    );
    assert_eq!(data.len() - 4 * t.rest.len(), 22_048);
    assert!(t.weights.iter().map(|&(key, _)| key).eq(0..18));
    let weights: Vec<u32> = t.weights.iter().map(|&(_, weight)| weight).collect();
    assert_eq!(
        weights,
        [
            7, 9, 0, 3, 5, 6, 8, 10, 12, 13, 14, 15, 15, 15, 15, 15, 300, 544
        ]
    );
    assert!(t.qualities.iter().map(|&(key, _)| key).eq(0..16));
    let values: Vec<u32> = t.qualities.iter().map(|&(_, value)| value).collect();
    assert_eq!(
        values,
        [0, 1, 2, 3, 4, 5, 6, 7, 9, 11, 13, 15, 18, 21, 25, 25]
    );
    assert_eq!(t.rows[0], (1, [140, 5, 25, 5, 15, 55, 0]));
    assert_eq!(
        t.chances.iter().find(|&&(effect, _)| effect == 868),
        Some(&(868, [500.0, 502.0, 25.0]))
    );
    let word = |name: &str| b.word(BALANCE_KEYS.iter().find(|k| k.name == name).unwrap());
    assert_eq!(
        (word("GoldProLevel"), word("GoldProLevel[1]")),
        (Some(20), Some(2000))
    );
}
