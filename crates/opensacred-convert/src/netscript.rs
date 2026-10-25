//! `netscript` subcommand: the NetScript bytecode of the 20 trees of `bin/` → one tree file each
//! under `<out>/netscript/` (docs/02 §7; the format is [`opensacred_data::netscript`]) and, on request,
//! a readable listing of each tree next to it.

use std::collections::BTreeMap;
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

use crate::flow::{Ending, Stmt, Test, structure};
use crate::script_repairs::{self, Outcome};
use crate::verify::{GOG_MANIFEST, parse_manifest, read_game_file};

pub fn output_dir(out: &Path) -> PathBuf {
    out.join("netscript")
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Summary {
    pub trees: usize,
    pub records: usize,
    pub functions: usize,
    pub bytes: u64,
    /// The trees and the cells that the repairs gave (`script_repairs`).
    pub repaired_trees: usize,
    pub repaired_cells: usize,
    /// The repairs of functions made, by what they mend, and the trees each was made in.
    pub repairs: BTreeMap<&'static str, usize>,
    pub warnings: Vec<String>,
}

/// The trees, as the manifest names their directories: those that hold a `FunkCode.bin`.
pub(crate) fn trees() -> Result<Vec<String>> {
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
pub(crate) fn tree(game_dir: &Path, dir: &str) -> Result<Tree> {
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

/// A statement of the structured flow of `records` in the listing, its lines indented by
/// `depth`: a record run, a test of its record (`if`, `if not`, `elseif`, `else`), a return.
fn flow_lines(out: &mut String, depth: usize, records: &[Record], stmts: &[Stmt]) {
    let indent = "  ".repeat(depth);
    let tested = |test: &Test| match *test {
        Test::Condition(i) | Test::Guard(i) | Test::Block(i) => &records[i],
    };
    for stmt in stmts {
        match stmt {
            Stmt::Run(i) => record_line(out, &indent, &records[*i]),
            Stmt::Return(ending) => {
                let _ = writeln!(out, "{indent}return {}", ending_name(*ending));
            }
            Stmt::If {
                test,
                then,
                otherwise,
            } => {
                let (head, then, otherwise) = if then.is_empty() {
                    ("if not ", otherwise, then)
                } else {
                    ("if ", then, otherwise)
                };
                record_line(out, &format!("{indent}{head}"), tested(test));
                flow_lines(out, depth + 1, records, then);
                let mut rest = otherwise;
                // A test alone in an `else` reads as an `elseif`.
                while let [
                    Stmt::If {
                        test,
                        then,
                        otherwise,
                    },
                ] = rest.as_slice()
                    && !then.is_empty()
                {
                    record_line(out, &format!("{indent}elseif "), tested(test));
                    flow_lines(out, depth + 1, records, then);
                    rest = otherwise;
                }
                if !rest.is_empty() {
                    let _ = writeln!(out, "{indent}else");
                    flow_lines(out, depth + 1, records, rest);
                }
                let _ = writeln!(out, "{indent}end");
            }
        }
    }
}

/// How the listing names a function's end.
fn ending_name(ending: Ending) -> &'static str {
    match ending {
        Ending::Ran => "ran",
        Ending::Stopped => "stopped",
        Ending::Done => "done",
        Ending::Again => "again",
    }
}

/// The listing of a tree's functions as their flow structures (`flow::structure`, #723): an
/// error names the tree and the function whose flow does not.
fn flow_listing(source: &str, tree: &Tree) -> Result<String> {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "; {source}: its scripts and its {} functions, structured",
        tree.functions.len()
    );
    for (name, records) in [
        ("StartCode.bin", &tree.start),
        ("QuestCode.bin", &tree.quest),
    ] {
        let stmts = structure(records).with_context(|| format!("structuring {source}/{name}"))?;
        let _ = writeln!(out, "script {name:?}");
        flow_lines(&mut out, 1, records, &stmts);
    }
    for (i, f) in tree.functions.iter().enumerate() {
        let stmts = structure(&f.records)
            .with_context(|| format!("structuring {source}'s function {i} {:?}", f.name))?;
        let _ = writeln!(out, "function {i} {:?}", f.name);
        flow_lines(&mut out, 1, &f.records, &stmts);
    }
    Ok(out)
}

/// Converts every tree; `listing` also writes `<tree>.txt` next to each tree file, and its
/// functions' structured flow, `<tree>.flow.txt`.
pub fn convert(game_dir: &Path, out: &Path, listing_too: bool) -> Result<Summary> {
    let dir = output_dir(out);
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let entries = trees()?
        .par_iter()
        .map(|source| {
            let mut tree = tree(game_dir, source)?;
            let outcome = script_repairs::apply(&script_repairs::CELLS, source, &mut tree.start);
            let report = script_repairs::repair(
                &script_repairs::EDITS,
                &script_repairs::RENAMES,
                &script_repairs::ADDITIONS,
                source,
                &mut tree,
            );
            let file = file_name(source);
            let bytes = tree
                .encode()
                .with_context(|| format!("encoding {source}"))?;
            let path = dir.join(&file);
            fs::write(&path, &bytes).with_context(|| format!("writing {}", path.display()))?;
            // Its Luau sources, in a directory of its own (#723).
            let (sources, index) = crate::luau::translate(source, &tree)?;
            let luau = path.with_extension("");
            for (name, text) in &sources {
                let target = luau.join(name);
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("creating {}", parent.display()))?;
                }
                fs::write(&target, text)
                    .with_context(|| format!("writing {}", target.display()))?;
            }
            let index = ron::ser::to_string_pretty(&index, crate::utf8::pretty())
                .with_context(|| format!("writing {source}'s index"))?;
            let target = luau.join(crate::luau::INDEX);
            fs::write(&target, index).with_context(|| format!("writing {}", target.display()))?;
            if listing_too {
                let text = path.with_extension("txt");
                fs::write(&text, listing(source, &tree))
                    .with_context(|| format!("writing {}", text.display()))?;
                let flow = path.with_extension("flow.txt");
                fs::write(&flow, flow_listing(source, &tree)?)
                    .with_context(|| format!("writing {}", flow.display()))?;
            }
            let entry = TreeEntry {
                source: source.clone(),
                file,
                records: tree.records(),
                functions: tree.functions.len(),
            };
            Ok((
                entry,
                u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                outcome,
                report,
            ))
        })
        .collect::<Result<Vec<_>>>()?;

    let mut summary = Summary::default();
    let mut trees = Vec::with_capacity(entries.len());
    for (entry, bytes, outcome, report) in entries {
        match outcome {
            Outcome::None => {}
            Outcome::Repaired(cells) => {
                summary.repaired_trees += 1;
                summary.repaired_cells += cells;
            }
            Outcome::Mismatch(warning) => summary.warnings.push(warning),
        }
        for what in report.made {
            *summary.repairs.entry(what).or_default() += 1;
        }
        summary.warnings.extend(report.warnings);
        summary.trees += 1;
        summary.records += entry.records;
        summary.functions += entry.functions;
        summary.bytes += bytes;
        trees.push(entry);
    }
    let text = ron::ser::to_string_pretty(&NetScript { trees }, crate::utf8::pretty())
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
        // The installation's records, and those that the quests' repairs add and take away: 2203's
        // win (11 trees), 5005's loss (20), 10255's restart (20), Glubba's death (9), 501's loss
        // (9) and 1702's target (9) gain one each, 1401's day (11) loses its one; 301's and 302's
        // losses gain two (9), 9009's win three (11), the 5.7.4 entrance two (20) and the air's
        // compass three (9); 501's new loss holds two (11); Jacquaire moves; 2303's dialogue (11)
        // loses one.
        assert_eq!(
            summary.records,
            2_203_278 + 11 + 20 + 20 + 9 + 9 + 9 - 11 + 2 * 9 + 3 * 11 + 2 * 20 + 3 * 9 + 2 * 11
                - 11
        );
        // The chest's cell in the start scripts of the 9 Underworld trees and the 2 multiplayer
        // trees, and no warning.
        assert_eq!((summary.repaired_trees, summary.repaired_cells), (11, 11));
        // The quests' repairs, each in every tree that holds its quest.
        assert_eq!(
            summary.repairs,
            BTreeMap::from([
                ("Jacquaire made at 9006's end (#959)", 11),
                ("Jacquaire not made by the box (#959)", 11),
                ("quest 1001's start (#942)", 11),
                ("quest 10255's restart (#940)", 20),
                ("quest 10260's item named (#957)", 20),
                ("quest 1202's four days (#931)", 11),
                ("quest 1303's goods after the failure (#937)", 11),
                ("quest 1401's day (#941)", 11),
                ("quest 1401's soldier's death function (#944)", 11),
                ("quest 1402's smugglers' function (#944)", 11),
                ("quest 1702's target hostile again (#948)", 9),
                ("quest 1803's three days (#932)", 11),
                ("quest 2002's monsters once it is entered (#953)", 20),
                ("quest 2203's count its own (#943)", 11),
                ("quest 2203's win entering it (#936)", 11),
                ("quest 501 lost at 55's end (#947)", 9),
                ("quest 501's ghosts in a group of their own (#951)", 11),
                ("quest 501's loss (#947)", 11),
                ("quest 5003's seeker's decline (#955)", 20),
                ("quest 5005's loss on its last leg (#939)", 20),
                ("quest 61's compass after the four elements (#958)", 9),
                ("quest 61's compass in the 5.7 dungeon (#958)", 20),
                ("quest 602's knight's id (#949)", 11),
                ("quest 602's knight's id, removed (#949)", 11),
                ("quest 670's Glubba dying (#950)", 9),
                ("quest 670's loss (#950)", 9),
                ("quest 7619's follower after the win (#938)", 11),
                ("quest 7619's follower's death after the win (#938)", 11),
                ("quest 9009's win before 2203's barrel (#935)", 11),
                ("quests 301 and 302 lost at 55's start (#946)", 9),
            ])
        );
        assert_eq!(summary.warnings, Vec::<String>::new());
        let index: NetScript =
            ron::from_str(&fs::read_to_string(output_dir(&out).join(INDEX)).unwrap()).unwrap();
        let camp = index
            .trees
            .iter()
            .find(|t| t.source == "bin/NetScriptCamp")
            .unwrap();
        // Its functions and 501's loss.
        assert_eq!(camp.functions, 23_520 + 1);
        let tree = Tree::decode(&fs::read(output_dir(&out).join(&camp.file)).unwrap()).unwrap();
        assert_eq!(tree.records(), camp.records);
        assert_eq!(tree.regions.len(), 22);
        // Once repaired, no tree has a payload that runs past its record's end.
        for entry in &index.trees {
            let tree =
                Tree::decode(&fs::read(output_dir(&out).join(&entry.file)).unwrap()).unwrap();
            let truncated = tree
                .start
                .iter()
                .chain(&tree.quest)
                .chain(tree.functions.iter().flat_map(|f| &f.records))
                .filter(|r| match &r.body {
                    Body::Operands(ops) => ops.iter().any(|o| o.value == Value::Truncated),
                    _ => false,
                })
                .count();
            assert_eq!(truncated, 0, "{}", entry.source);
        }
        fs::remove_dir_all(out).unwrap();
    }

    /// The `CreateObj` records of the Seraphim's add-on tree whose world position (`0x20`), where
    /// the game draws their models, lies off their cell's (`0x04`) centre (docs/02 §7.3, #429):
    /// 1,026 farther than half a cell's diagonal, up to 810 units, 188 of them doors (`items.pak`
    /// 5650 to 5766); the third number repeats the cell's layer.
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn the_objects_world_positions_lie_off_their_cells() {
        const CELL: f64 = 53.665_63;
        let tree = tree(
            &repo_root().join("OriginalGame"),
            "bin/Addon/TYPE_NPC_SERAPHIM",
        )
        .unwrap();
        let objects: Vec<&[Operand]> = tree
            .start
            .iter()
            .chain(&tree.quest)
            .chain(tree.functions.iter().flat_map(|f| &f.records))
            .filter(|r| r.kind == 8)
            .filter_map(|r| match &r.body {
                Body::Operands(ops) => Some(ops.as_slice()),
                _ => None,
            })
            .collect();
        let triple = |ops: &[Operand], tag| {
            ops.iter().find_map(|o| match (o.tag, &o.value) {
                (t, Value::I32x3(v)) if t == tag => Some(*v),
                _ => None,
            })
        };
        let placed = objects.iter().filter(|ops| triple(ops, 0x20).is_some());
        let (mut both, mut far, mut doors, mut farthest) = (0, 0, 0, 0.0_f64);
        for ops in placed.clone() {
            let (Some([cx, cy, layer]), Some([x, y, z])) = (triple(ops, 0x04), triple(ops, 0x20))
            else {
                continue;
            };
            both += 1;
            assert_eq!(z, layer);
            let off = (f64::from(x) - (f64::from(cx) + 0.5) * CELL)
                .hypot(f64::from(y) - (f64::from(cy) + 0.5) * CELL);
            if off > 38.0 {
                far += 1;
                farthest = farthest.max(off);
                let door = ops.iter().any(|o| {
                    o.tag == 0x02 && matches!(o.value, Value::I32(t) if (5650..=5766).contains(&t))
                });
                doors += usize::from(door);
            }
        }
        assert_eq!((objects.len(), placed.count(), both), (8_266, 5_730, 5_703));
        assert_eq!((far, doors), (1_026, 188));
        assert_eq!(farthest.round(), 810.0);
    }
}
