//! `netscript` subcommand: the NetScript bytecode of the 20 trees of `bin/` → one tree file each
//! under `<out>/netscript/` (docs/02 §7; the format is [`opensacred_data::netscript`]) and, on request,
//! a readable listing of each tree next to it.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use opensacred_data::netscript::{
    Body, Function, INDEX, NetScript, Operand, Quest, Record, Region, RegionEntry, Tree, TreeEntry,
    Value,
};
use opensacred_formats::netscript as original;
use rayon::prelude::*;

use crate::verify::{GOG_MANIFEST, parse_manifest, read_game_file};

pub fn output_dir(out: &Path) -> PathBuf {
    out.join("netscript")
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub trees: usize,
    pub records: usize,
    pub functions: usize,
    pub bytes: u64,
}

/// The trees, as the manifest names their directories: those that hold a `FunkCode.bin`.
fn trees() -> Result<Vec<String>> {
    let mut trees: Vec<String> = parse_manifest(GOG_MANIFEST)?
        .into_iter()
        .filter_map(|e| e.path.strip_suffix("/FunkCode.bin").map(str::to_owned))
        .collect();
    trees.sort();
    Ok(trees)
}

/// The tree file of a directory: `bin/Addon/TYPE_NPC_ELVE` → `addon-type_npc_elve.nsc`.
fn file_name(source: &str) -> String {
    let name = source.strip_prefix("bin/").unwrap_or(source);
    format!("{}.nsc", name.replace('/', "-").to_ascii_lowercase())
}

fn value(value: &original::Value) -> Value {
    use original::Value as V;
    match value.clone() {
        V::Marker => Value::Marker,
        V::U8(v) => Value::U8(v),
        V::U16(v) => Value::U16(v),
        V::U16x2(v) => Value::U16x2(v),
        V::U16x4(v) => Value::U16x4(v),
        V::I32(v) => Value::I32(v),
        V::I32x2(v) => Value::I32x2(v),
        V::I32x3(v) => Value::I32x3(v),
        V::I32x4(v) => Value::I32x4(v),
        V::Str(s) => Value::Str(s),
        V::Strs(a, b) => Value::Strs(a, b),
        V::NumStr {
            number,
            text,
            expression,
        } => Value::NumStr {
            number,
            text,
            expression,
        },
        V::Name(s) => Value::Name(s),
        V::Variable(s) => Value::Variable(s),
        V::Flag { flag, text } => Value::Flag { flag, text },
        V::Truncated => Value::Truncated,
    }
}

fn record(record: &original::Record) -> Record {
    let body = match &record.body {
        original::Body::Operands(operands) => Body::Operands(
            operands
                .iter()
                .map(|o| Operand {
                    tag: o.tag,
                    value: value(&o.value),
                })
                .collect(),
        ),
        original::Body::Dialog(d) => Body::Dialog {
            name: d.name.clone(),
            values: d.values,
        },
        original::Body::Block(n) => Body::Block(*n),
    };
    Record {
        kind: record.kind,
        body,
    }
}

/// The script of `<dir>/<name>`; a file the installation lacks (the empty quest files) has no
/// records.
fn script(game_dir: &Path, dir: &str, name: &str) -> Result<Vec<Record>> {
    let path = game_dir.join(dir).join(name);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = read_game_file(&path)?;
    let script = original::Script::parse(&data).with_context(|| format!("{dir}/{name}"))?;
    Ok(script.records.iter().map(record).collect())
}

/// The tree of a directory.
fn tree(game_dir: &Path, dir: &str) -> Result<Tree> {
    let funk_path = format!("{dir}/FunkCode.bin");
    let funk = original::Script::parse(&read_game_file(&game_dir.join(&funk_path))?)
        .with_context(|| funk_path.clone())?;
    let vectoren_path = format!("{dir}/Vectoren.bin");
    let vectoren = original::Vectoren::parse(&read_game_file(&game_dir.join(&vectoren_path))?)
        .with_context(|| vectoren_path.clone())?;
    let ranges = vectoren
        .record_ranges(&funk)
        .with_context(|| format!("{vectoren_path}: a function does not fall on records"))?;
    let functions = vectoren
        .functions
        .iter()
        .zip(ranges)
        .map(|(f, range)| Function {
            name: f.name.clone(),
            quest: f.quest,
            region: f.region,
            records: funk.records[range].iter().map(record).collect(),
        })
        .collect();
    Ok(Tree {
        start: script(game_dir, dir, "StartCode.bin")?,
        quest: script(game_dir, dir, "QuestCode.bin")?,
        functions,
        quests: vectoren
            .quests
            .iter()
            .map(|q| Quest {
                id: q.id,
                name: q.name.clone(),
                values: q.values,
            })
            .collect(),
        regions: vectoren
            .regions
            .iter()
            .map(|r| Region {
                id: r.id,
                name: r.name.clone(),
                values: r.values,
                lists: r.lists.clone(),
                entries: r
                    .entries
                    .iter()
                    .map(|e| RegionEntry {
                        list: e.list.clone(),
                        values: e.values,
                        tail: e.tail,
                    })
                    .collect(),
            })
            .collect(),
    })
}

fn ints<T: ToString>(values: &[T]) -> String {
    values
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// An operand of the listing: the tag in hex, then `:` and the value, strings quoted.
fn operand(o: &Operand) -> String {
    let tag = format!("{:02X}", o.tag);
    match &o.value {
        Value::Marker => tag,
        Value::U8(v) => format!("{tag}:{v}"),
        Value::U16(v) => format!("{tag}:{v}"),
        Value::I32(v) => format!("{tag}:{v}"),
        Value::U16x2(v) => format!("{tag}:({})", ints(v)),
        Value::U16x4(v) => format!("{tag}:({})", ints(v)),
        Value::I32x2(v) => format!("{tag}:({})", ints(v)),
        Value::I32x3(v) => format!("{tag}:({})", ints(v)),
        Value::I32x4(v) => format!("{tag}:({})", ints(v)),
        Value::Str(s) => format!("{tag}:{s:?}"),
        Value::Strs(a, b) => format!("{tag}:{a:?},{b:?}"),
        Value::NumStr {
            number,
            text,
            expression,
        } => match expression {
            Some(e) => format!("{tag}:{number},{text:?},{e:?}"),
            None => format!("{tag}:{number},{text:?}"),
        },
        Value::Name(s) => format!("{tag}:name {s:?}"),
        Value::Variable(s) => format!("{tag}:var {s:?}"),
        Value::Flag { flag, text } => match text {
            Some(t) => format!("{tag}:{flag},{t:?}"),
            None => format!("{tag}:{flag}"),
        },
        Value::Truncated => format!("{tag}:truncated"),
    }
}

/// A record of the listing: the keyword of its type (or `type<N>`), then its operands.
fn record_line(out: &mut String, indent: &str, r: &Record) {
    let kind = original::name(r.kind).map_or_else(|| format!("type{}", r.kind), str::to_owned);
    let _ = match &r.body {
        Body::Operands(operands) => {
            let operands: Vec<String> = operands.iter().map(operand).collect();
            writeln!(out, "{indent}{kind} {}", operands.join(" "))
        }
        Body::Dialog { name, values } => {
            writeln!(out, "{indent}{kind} dialog {name:?} ({})", ints(values))
        }
        Body::Block(n) => writeln!(out, "{indent}{kind} block {n}"),
    };
}

/// The readable listing of a tree: its scripts, functions, quests and regions.
fn listing(source: &str, tree: &Tree) -> String {
    let mut out = String::new();
    for (name, records) in [
        ("StartCode.bin", &tree.start),
        ("QuestCode.bin", &tree.quest),
    ] {
        let _ = writeln!(out, "; {source}/{name}: {} records", records.len());
        records.iter().for_each(|r| record_line(&mut out, "", r));
    }
    let _ = writeln!(
        out,
        "; {source}/FunkCode.bin: {} functions",
        tree.functions.len()
    );
    for (i, f) in tree.functions.iter().enumerate() {
        let _ = writeln!(
            out,
            "function {i} {:?} quest {} region {}",
            f.name, f.quest, f.region
        );
        f.records
            .iter()
            .for_each(|r| record_line(&mut out, "  ", r));
    }
    for q in &tree.quests {
        let _ = writeln!(out, "quest {} {:?} ({})", q.id, q.name, ints(&q.values));
    }
    for r in &tree.regions {
        let lists: Vec<String> = r.lists.iter().map(|l| format!("[{}]", ints(l))).collect();
        let _ = writeln!(
            out,
            "region {} {:?} ({}) {}",
            r.id,
            r.name,
            ints(&r.values),
            lists.join(" ")
        );
        for e in &r.entries {
            let _ = writeln!(
                out,
                "  entry [{}] ({}) {:?}",
                ints(&e.list),
                ints(&e.values),
                e.tail
            );
        }
    }
    out
}

/// Converts every tree; `listing` also writes `<tree>.txt` next to each tree file.
pub fn convert(game_dir: &Path, out: &Path, listing_too: bool) -> Result<Summary> {
    let dir = output_dir(out);
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let entries = trees()?
        .par_iter()
        .map(|source| {
            let tree = tree(game_dir, source)?;
            let file = file_name(source);
            let bytes = tree
                .encode()
                .with_context(|| format!("encoding {source}"))?;
            let path = dir.join(&file);
            fs::write(&path, &bytes).with_context(|| format!("writing {}", path.display()))?;
            if listing_too {
                let path = path.with_extension("txt");
                fs::write(&path, listing(source, &tree))
                    .with_context(|| format!("writing {}", path.display()))?;
            }
            let entry = TreeEntry {
                source: source.clone(),
                file,
                records: tree.records(),
                functions: tree.functions.len(),
            };
            Ok((entry, u64::try_from(bytes.len()).unwrap_or(u64::MAX)))
        })
        .collect::<Result<Vec<_>>>()?;

    let mut summary = Summary::default();
    let mut trees = Vec::with_capacity(entries.len());
    for (entry, bytes) in entries {
        summary.trees += 1;
        summary.records += entry.records;
        summary.functions += entry.functions;
        summary.bytes += bytes;
        trees.push(entry);
    }
    let text = ron::ser::to_string_pretty(&NetScript { trees }, ron::ser::PrettyConfig::default())
        .context("writing netscript.ron")?;
    let path = dir.join(INDEX);
    fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{repo_root, scratch_dir};

    #[test]
    fn tree_files_are_named_after_their_directory() {
        assert_eq!(
            file_name("bin/Addon/TYPE_NPC_ELVE"),
            "addon-type_npc_elve.nsc"
        );
        assert_eq!(file_name("bin/NetScriptCamp"), "netscriptcamp.nsc");
    }

    #[test]
    fn the_manifest_has_20_trees() {
        assert_eq!(trees().unwrap().len(), 20);
    }

    #[test]
    fn listings_name_record_types_and_quote_strings() {
        let record = Record {
            kind: 1,
            body: Body::Operands(vec![
                Operand {
                    tag: 0x01,
                    value: Value::Str("res:1".into()),
                },
                Operand {
                    tag: 0x04,
                    value: Value::Name("pos_A".into()),
                },
                Operand {
                    tag: 0x0E,
                    value: Value::Marker,
                },
                Operand {
                    tag: 0x0C,
                    value: Value::I32x3([1, 2, -3]),
                },
            ]),
        };
        let mut out = String::new();
        record_line(&mut out, "", &record);
        record_line(
            &mut out,
            "  ",
            &Record {
                kind: 41,
                body: Body::Block(2),
            },
        );
        assert_eq!(
            out,
            "CreateNPC 01:\"res:1\" 04:name \"pos_A\" 0E 0C:(1,2,-3)\n  type41 block 2\n"
        );
    }

    /// Every tree converts, its records all land in the tree file, and the file reads back.
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn every_tree_converts() {
        let out = scratch_dir("netscript");
        let game = repo_root().join("OriginalGame");
        let summary = convert(&game, &out, false).unwrap();
        assert_eq!(summary.trees, 20);
        assert_eq!(summary.records, 2_203_278);
        let index: NetScript =
            ron::from_str(&fs::read_to_string(output_dir(&out).join(INDEX)).unwrap()).unwrap();
        let camp = index
            .trees
            .iter()
            .find(|t| t.source == "bin/NetScriptCamp")
            .unwrap();
        assert_eq!(camp.functions, 23_520);
        let tree = Tree::decode(&fs::read(output_dir(&out).join(&camp.file)).unwrap()).unwrap();
        assert_eq!(tree.records(), camp.records);
        assert_eq!(tree.regions.len(), 22);
        fs::remove_dir_all(out).unwrap();
    }
}
