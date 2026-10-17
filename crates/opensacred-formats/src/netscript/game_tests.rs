//! Tests on the GOG installation files: every script file of the 20 trees against the table of the
//! Python prototype `tools/analysis/scripts/netscript_dump.py` (the CRC of a canonical listing of
//! every operand). CI does not have the game files, so the tests are ignored by default.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use super::*;
use crate::testing::{game_file, repo_root, run_python, scratch_dir};

const SCRIPT_FILES: [&str; 4] = [
    "FunkCode.bin",
    "StartCode.bin",
    "QuestCode.bin",
    "QuestPoolCode.bin",
];

/// Paths of the script files under `dir`, relative to the installation.
fn script_files(game: &Path, dir: &Path, out: &mut Vec<String>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            script_files(game, &path, out);
        } else if path
            .file_name()
            .is_some_and(|name| SCRIPT_FILES.iter().any(|f| OsStr::new(f) == name))
        {
            let relative = path.strip_prefix(game).unwrap();
            out.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
}

/// A string as the prototype writes it: its bytes in hex, quoted.
fn hex(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        write!(out, "{:02x}", u32::from(c)).unwrap();
    }
    out.push('"');
    out
}

fn join<T: ToString>(values: &[T]) -> String {
    values
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// `canonical` of netscript_dump.py.
fn canonical(record: &Record) -> String {
    let operands = match &record.body {
        Body::Dialog(d) => {
            return format!(
                "{}:dialog -1 {} {},{},0",
                record.kind,
                hex(&d.name),
                d.values[0],
                d.values[1]
            );
        }
        Body::Block(n) => return format!("{}:block {n}", record.kind),
        Body::Operands(operands) => operands,
    };
    let parts: Vec<String> = operands
        .iter()
        .map(|o| {
            let value = match &o.value {
                Value::Marker => return format!("{:02x}", o.tag),
                Value::Truncated => "truncated".to_owned(),
                Value::U8(v) => v.to_string(),
                Value::U16(v) => v.to_string(),
                Value::I32(v) => v.to_string(),
                Value::U16x2(v) => join(v),
                Value::U16x4(v) => join(v),
                Value::I32x2(v) => join(v),
                Value::I32x3(v) => join(v),
                Value::I32x4(v) => join(v),
                Value::Str(t) => hex(t),
                Value::Name(t) => format!("name{}", hex(t)),
                Value::Variable(t) => format!("var{}", hex(t)),
                Value::Strs(a, b) => format!("{}{}", hex(a), hex(b)),
                Value::NumStr {
                    number,
                    text,
                    expression,
                } => format!(
                    "{number}{}{}",
                    hex(text),
                    expression.as_deref().map(hex).unwrap_or_default()
                ),
                Value::Flag { flag, text } => {
                    format!("{flag}{}", text.as_deref().map(hex).unwrap_or_default())
                }
            };
            format!("{:02x}={value}", o.tag)
        })
        .collect();
    format!("{}:{}", record.kind, parts.join(" "))
}

fn ints<T: ToString>(values: &[T]) -> String {
    join(values)
}

/// `canonical_vectors` of netscript_dump.py.
fn canonical_vectors(v: &Vectoren) -> Vec<String> {
    let mut lines: Vec<String> = v
        .functions
        .iter()
        .map(|f| {
            format!(
                "f {} {} {} {} {}",
                hex(&f.name),
                f.offset,
                f.length,
                f.quest,
                f.region
            )
        })
        .collect();
    for q in &v.quests {
        lines.push(format!("q {} {} {}", q.id, hex(&q.name), ints(&q.values)));
    }
    for r in &v.regions {
        let lists: Vec<String> = r.lists.iter().map(|l| ints(l)).collect();
        lines.push(format!(
            "r {} {} {} {}",
            r.id,
            hex(&r.name),
            ints(&r.values),
            lists.join("|")
        ));
        for e in &r.entries {
            let tail: String = e.tail.iter().map(|b| format!("{b:02x}")).collect();
            lines.push(format!("e {} {} {tail}", ints(&e.list), ints(&e.values)));
        }
    }
    lines
}

#[test]
#[ignore = "needs the game files in OriginalGame/ and Python"]
fn every_script_matches_netscript_dump_py() {
    let out = scratch_dir("netscript");
    let (files, names, vectors) = (
        out.join("files.tsv"),
        out.join("names.tsv"),
        out.join("vectors.tsv"),
    );
    let game = repo_root().join("OriginalGame");
    run_python(
        "tools/analysis/scripts/netscript_dump.py",
        &[
            OsStr::new("--game-dir"),
            game.as_os_str(),
            OsStr::new("--files"),
            files.as_os_str(),
            OsStr::new("--names"),
            names.as_os_str(),
            OsStr::new("--vectors"),
            vectors.as_os_str(),
        ],
    );
    let table = |path: &Path| -> BTreeMap<String, String> {
        fs::read_to_string(path)
            .unwrap()
            .lines()
            .skip(1)
            .map(|line| {
                let (path, rest) = line.split_once('\t').unwrap();
                (path.to_owned(), rest.to_owned())
            })
            .collect()
    };
    let (expected, expected_vectors) = (table(&files), table(&vectors));
    assert_eq!(expected_vectors.len(), 20);

    let mut paths = Vec::new();
    script_files(&game, &game.join("bin"), &mut paths);
    assert_eq!(
        paths.iter().cloned().collect::<BTreeSet<_>>(),
        expected.keys().cloned().collect()
    );
    let mut records = 0;
    for path in &paths {
        let data = game_file(path);
        let script = Script::parse(&data).unwrap_or_else(|e| panic!("{path}: {e}"));
        let operands: usize = script
            .records
            .iter()
            .map(|r| match &r.body {
                Body::Operands(o) => o.len(),
                _ => 0,
            })
            .sum();
        let truncated = script
            .records
            .iter()
            .filter(|r| {
                matches!(&r.body, Body::Operands(o) if o.last().is_some_and(|o| o.value == Value::Truncated))
            })
            .count();
        let listing: Vec<String> = script.records.iter().map(canonical).collect();
        let ours = format!(
            "{}\t{}\t{operands}\t{truncated}\t{:08x}",
            data.len(),
            script.records.len(),
            crc32fast::hash(listing.join("\n").as_bytes())
        );
        assert_eq!(ours, expected[path], "{path}");
        records += script.records.len();
        if let Some(dir) = path.strip_suffix("/FunkCode.bin") {
            let vectors = format!("{dir}/Vectoren.bin");
            let v =
                Vectoren::parse(&game_file(&vectors)).unwrap_or_else(|e| panic!("{vectors}: {e}"));
            let ranges = v.record_ranges(&script).unwrap();
            assert_eq!(ranges.last().map(|r| r.end), Some(script.records.len()));
            let crc = crc32fast::hash(canonical_vectors(&v).join("\n").as_bytes());
            let ours = format!(
                "{}\t{}\t{}\t{crc:08x}",
                v.functions.len(),
                v.quests.len(),
                v.regions.len()
            );
            assert_eq!(ours, expected_vectors[&vectors], "{vectors}");
        }
    }
    assert_eq!(records, 2_203_278);

    // The keywords, read off the resolvers by the prototype.
    let mut by_type: BTreeMap<u16, Vec<&str>> = BTreeMap::new();
    for &(name, kind) in KEYWORDS {
        by_type.entry(kind).or_default().push(name);
    }
    let ours: Vec<String> = by_type
        .into_iter()
        .map(|(kind, mut names)| {
            names.sort_unstable();
            format!("{kind}\t{}", names.join("/"))
        })
        .collect();
    let theirs = fs::read_to_string(&names).unwrap();
    assert_eq!(ours, theirs.lines().skip(1).collect::<Vec<_>>());
    fs::remove_dir_all(out).unwrap();
}
