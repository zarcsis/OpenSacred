//! NetScript translated into Luau (#723; docs/re/netscript.md, docs/02 §7): each tree's scripts
//! and function blocks as Luau sources, the masters that stage B compiles into the game's
//! bytecode.
//!
//! A tree's sources lie in a directory of its own: `start.luau` holds the start script and the
//! starting quest flags (`StartCode`, `QuestCode`); `sectors/<n>.luau` a sector's `Sector<n>Init`
//! and `…Enter`; `regions/<n>.luau` a region's `Region<n>Init`; `quests/<id>.luau` the functions
//! of a quest; `functions/<shape>.luau` the other functions of one name's shape, the name with its
//! numbers taken out (`fillchest_chestR.luau` for every `fillchest_chestR<n>_<m>`). Each file
//! returns a table of its functions by their names, and `index.ron` names the file of every
//! function, by which the engine loads a function as it is first called.
//!
//! A function's flow is structured as the walker runs it (`flow`): its conditions and guards
//! become tests (`if test { … } then`, `if not group_is_dead { … } then return false end`), a
//! block 41 `if block(n) then`, its ends `return true`, `return false`, `return "done"` and
//! `return "again"`. Every other record is a call named by its type's keyword in snake case
//! (`CreateNPC` → `create_npc`; `type39`, `type47` and `dialog_entry` for the types without one),
//! its operands a table keyed by their tags (`_01`, `_2A`), a tag that repeats a list of its
//! values in their order; `CallFunktion` is `call("name", { … })` where it names one. A value is a
//! number, a string,
//! `true` for a marker, a list for a tuple, `var("name")` for a variable, and a number's
//! expression (`A+VAR(B)`), whose value the client looks up in the variables, `var("A" ..
//! var("B"))`; a string built at run time (`Res:…+VAR(…)`) stays the string the engine expands;
//! a payload the client reads from past its record's end is `truncated()`. The translation is
//! strict: a record of no type it knows, an expression it cannot read or a flow that does not
//! structure fails it, naming the tree, the function and the record.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use anyhow::{Context, Result, bail};
use opensacred_data::netscript::{Body, Operand, Record, Tree, Value};
use opensacred_formats::netscript as original;
use serde::{Deserialize, Serialize};

use crate::flow::{Ending, Stmt, Test, structure};

/// The index of a tree's functions, `index.ron` in its directory: for each name the functions of
/// that name in the tree's order, a name that several share (the quests' `ToDo:-1.1`, 1,986
/// functions of 142 names in a campaign tree) by each of them (**hypothesis**: the client's
/// lookup by name, `0x460590`, takes the first).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    pub functions: BTreeMap<String, Vec<Entry>>,
}

/// A function in a tree's sources: its file and its key in the file's table, its name or, for a
/// name that the file holds already, the name and `#<n>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub file: String,
    pub key: String,
}

/// The index's file in a tree's directory.
pub const INDEX: &str = "index.ron";

/// The name of the start script and of the starting quest flags among a tree's functions.
pub const START: &str = "StartCode";
pub const QUEST_FLAGS: &str = "QuestCode";

/// The record types the translation writes as the flow, not as calls.
const IF: u16 = 58;
const ELSE: u16 = 59;
const ELSEIF: u16 = 66;
const BLOCK: u16 = 41;
const BLOCK_END: u16 = 42;
const CALL: u16 = 22;
const DIALOG_ENTRY: u16 = 40;

/// The Luau name of a record type: its keyword in snake case, `type<n>` or `dialog_entry` for the
/// types without one; none for those of the flow.
fn call_name(kind: u16) -> Result<String> {
    if matches!(kind, IF | ELSE | ELSEIF | BLOCK | BLOCK_END | 0) {
        bail!("the record type {kind} is the flow's, not a call");
    }
    if kind == DIALOG_ENTRY {
        return Ok("dialog_entry".to_owned());
    }
    match original::name(kind) {
        Some(keyword) => Ok(snake(keyword)),
        None if matches!(kind, 39 | 47) => Ok(format!("type{kind}")),
        None => bail!("no record type {kind}"),
    }
}

/// A keyword in snake case: `CreateNPC` → `create_npc`, `NPC_TalkTo` → `npc_talk_to`.
fn snake(keyword: &str) -> String {
    let chars: Vec<char> = keyword.chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let before = i.checked_sub(1).map(|j| chars[j]);
            let after = chars.get(i + 1).copied();
            let starts_word = match before {
                Some(b) if b.is_ascii_lowercase() || b.is_ascii_digit() => true,
                Some(b) if b.is_ascii_uppercase() => after.is_some_and(|a| a.is_ascii_lowercase()),
                _ => false,
            };
            if starts_word && !out.ends_with('_') {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// A Luau string literal of `text`.
fn string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{{{:x}}}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A list of the numbers `values`.
fn list<T: ToString>(values: &[T]) -> String {
    let items: Vec<String> = values.iter().map(ToString::to_string).collect();
    format!("{{ {} }}", items.join(", "))
}

/// The Luau expression of a number's expression `A+VAR(B)…` (the operand reader `0x473C48`):
/// the name built of its parts, literal or a variable's value, looked up as a variable.
fn expression(text: &str) -> Result<String> {
    let mut parts = Vec::new();
    for part in text.split('+') {
        let upper = part.to_ascii_uppercase();
        if upper.starts_with("VAR(") && part.ends_with(')') {
            parts.push(format!("var({})", string(&part[4..part.len() - 1])));
        } else if !part.is_empty() && !part.contains(['(', ')']) {
            parts.push(string(part));
        } else {
            bail!("the expression {text:?} does not read as names and VAR(…)");
        }
    }
    Ok(format!("var({})", parts.join(" .. ")))
}

/// The Luau value of an operand's payload.
fn value(value: &Value) -> Result<String> {
    Ok(match value {
        Value::Marker => "true".to_owned(),
        Value::U8(v) => v.to_string(),
        Value::U16(v) => v.to_string(),
        Value::I32(v) => v.to_string(),
        Value::U16x2(v) => list(v),
        Value::U16x4(v) => list(v),
        Value::I32x2(v) => list(v),
        Value::I32x3(v) => list(v),
        Value::I32x4(v) => list(v),
        Value::Str(s) | Value::Name(s) => string(s),
        Value::Strs(a, b) => format!("{{ {}, {} }}", string(a), string(b)),
        Value::NumStr {
            number,
            text,
            expression: None,
        } => format!("{{ {number}, {} }}", string(text)),
        Value::NumStr {
            text,
            expression: Some(e),
            ..
        } => format!("{{ {}, {} }}", expression(e)?, string(text)),
        Value::Variable(name) => format!("var({})", string(name)),
        Value::Flag { flag, text: None } => flag.to_string(),
        Value::Flag {
            flag,
            text: Some(text),
        } => format!("{{ {flag}, {} }}", string(text)),
        // The payload the client reads from past the record's end (the 11 `TriggerPatch`
        // records' last 0x2A, docs/re/netscript.md, "Truncated operands"), for the engine.
        Value::Truncated => "truncated()".to_owned(),
    })
}

/// The table of `operands`, keyed by their tags, a tag that repeats a list of its values.
fn operands_table(operands: &[Operand]) -> Result<String> {
    let mut by_tag: BTreeMap<u8, Vec<String>> = BTreeMap::new();
    for operand in operands {
        by_tag
            .entry(operand.tag)
            .or_default()
            .push(value(&operand.value).with_context(|| format!("its tag {:02X}", operand.tag))?);
    }
    if by_tag.is_empty() {
        return Ok("{}".to_owned());
    }
    let fields: Vec<String> = by_tag
        .into_iter()
        .map(|(tag, values)| match <[String; 1]>::try_from(values) {
            Ok([one]) => format!("_{tag:02X} = {one}"),
            Err(values) => format!("_{tag:02X} = {{ {} }}", values.join(", ")),
        })
        .collect();
    Ok(format!("{{ {} }}", fields.join(", ")))
}

/// The call of a record run for what it does, or tested for its result.
fn call(record: &Record) -> Result<String> {
    match &record.body {
        // A `CallFunktion` that names its function first; the 22 that name none stay as they are.
        Body::Operands(operands)
            if record.kind == CALL
                && matches!(
                    operands.first(),
                    Some(Operand {
                        value: Value::Str(_),
                        ..
                    })
                ) =>
        {
            let (name, rest) = match operands.split_first() {
                Some((
                    Operand {
                        value: Value::Str(name),
                        ..
                    },
                    rest,
                )) => (name, rest),
                _ => bail!("a CallFunktion names no function first"),
            };
            if rest.is_empty() {
                Ok(format!("call({})", string(name)))
            } else {
                Ok(format!("call({}, {})", string(name), operands_table(rest)?))
            }
        }
        Body::Operands(operands) if matches!(record.kind, IF | ELSEIF) => {
            Ok(format!("test {}", operands_table(operands)?))
        }
        Body::Operands(operands) => Ok(format!(
            "{} {}",
            call_name(record.kind)?,
            operands_table(operands)?
        )),
        Body::Dialog { name, values } => Ok(format!(
            "dialog_entry {{ name = {}, values = {} }}",
            string(name),
            list(values)
        )),
        Body::Block(selector) => Ok(format!("block({selector})")),
    }
}

/// The Luau of a function's end.
fn ending(ending: Ending) -> &'static str {
    match ending {
        Ending::Ran => "return true",
        Ending::Stopped => "return false",
        Ending::Done => "return \"done\"",
        Ending::Again => "return \"again\"",
    }
}

/// Writes the statements `stmts` of the function of `records`, indented by `depth`.
fn statements(out: &mut String, depth: usize, records: &[Record], stmts: &[Stmt]) -> Result<()> {
    let indent = "    ".repeat(depth);
    let test = |test: &Test| {
        let (Test::Condition(i) | Test::Guard(i) | Test::Block(i)) = *test;
        call(&records[i]).with_context(|| format!("its record {i}"))
    };
    for (n, stmt) in stmts.iter().enumerate() {
        match stmt {
            Stmt::Run(i) => {
                let line = call(&records[*i]).with_context(|| format!("its record {i}"))?;
                let _ = writeln!(out, "{indent}{line}");
            }
            Stmt::Return(end) => {
                let _ = writeln!(out, "{indent}{}", ending(*end));
                // Luau allows nothing after a return in its block.
                if n + 1 < stmts.len() {
                    bail!("a statement follows a return");
                }
            }
            Stmt::If {
                test: tested,
                then,
                otherwise,
            } => {
                let (head, then, otherwise) = if then.is_empty() {
                    ("if not ", otherwise, then)
                } else {
                    ("if ", then, otherwise)
                };
                let _ = writeln!(out, "{indent}{head}{} then", test(tested)?);
                statements(out, depth + 1, records, then)?;
                let mut rest = otherwise;
                // A test alone in an `else` is an `elseif`.
                while let [
                    Stmt::If {
                        test: tested,
                        then,
                        otherwise,
                    },
                ] = rest.as_slice()
                    && !then.is_empty()
                {
                    let _ = writeln!(out, "{indent}elseif {} then", test(tested)?);
                    statements(out, depth + 1, records, then)?;
                    rest = otherwise;
                }
                if !rest.is_empty() {
                    let _ = writeln!(out, "{indent}else");
                    statements(out, depth + 1, records, rest)?;
                }
                let _ = writeln!(out, "{indent}end");
            }
        }
    }
    Ok(())
}

/// Writes the function `name` of `records` into a file's table.
fn function(out: &mut String, name: &str, records: &[Record]) -> Result<()> {
    let stmts = structure(records)?;
    let _ = writeln!(out, "F[{}] = function()", string(name));
    statements(out, 1, records, &stmts)?;
    let _ = writeln!(out, "end\n");
    Ok(())
}

/// The file of the function `name` of the quest `quest` (−1 for none) within a tree's directory.
fn file_of(name: &str, quest: i32) -> String {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if let Some(n) = name
        .strip_prefix("Sector")
        .and_then(|rest| {
            rest.strip_suffix("Init")
                .or_else(|| rest.strip_suffix("Enter"))
        })
        .filter(|n| digits(n))
    {
        return format!("sectors/{n}.luau");
    }
    if let Some(n) = name
        .strip_prefix("Region")
        .and_then(|rest| rest.strip_suffix("Init"))
        .filter(|n| digits(n))
    {
        return format!("regions/{n}.luau");
    }
    if quest >= 0 {
        return format!("quests/{quest}.luau");
    }
    // The name's shape: its numbers out, what a file's name cannot hold made `_`, one `_` at a
    // time.
    let mut shape = String::new();
    for c in name.chars() {
        match c {
            c if c.is_ascii_digit() => {}
            c if c.is_ascii_alphanumeric() || c == '_' => shape.push(c),
            _ => {
                if !shape.ends_with('_') {
                    shape.push('_');
                }
            }
        }
    }
    let parts: Vec<&str> = shape.split('_').filter(|part| !part.is_empty()).collect();
    let shape = if parts.is_empty() {
        "_".to_owned()
    } else {
        parts.join("_")
    };
    format!("functions/{shape}.luau")
}

/// The Luau sources of a tree from `source`, by their paths within its directory, and its
/// index; an error names the function and the record that do not translate.
pub fn translate(source: &str, tree: &Tree) -> Result<(BTreeMap<String, String>, Index)> {
    let mut groups: BTreeMap<String, Vec<(String, &[Record])>> = BTreeMap::new();
    let mut index = Index::default();
    let named = [(START, -1, &tree.start), (QUEST_FLAGS, -1, &tree.quest)]
        .into_iter()
        .chain(
            tree.functions
                .iter()
                .map(|f| (f.name.as_str(), f.quest, &f.records)),
        );
    for (name, quest, records) in named {
        let file = if matches!(name, START | QUEST_FLAGS) && quest < 0 {
            "start.luau".to_owned()
        } else {
            file_of(name, quest)
        };
        let group = groups.entry(file.clone()).or_default();
        let same = group
            .iter()
            .filter(|(key, _)| key.split('#').next() == Some(name))
            .count();
        let key = if same == 0 {
            name.to_owned()
        } else {
            format!("{name}#{}", same + 1)
        };
        group.push((key.clone(), records));
        index
            .functions
            .entry(name.to_owned())
            .or_default()
            .push(Entry { file, key });
    }
    let mut files = BTreeMap::new();
    for (file, functions) in groups {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "-- {source}: {}, translated from NetScript by opensacred-convert (#723).\n",
            file.trim_end_matches(".luau")
        );
        let _ = writeln!(out, "local F = {{}}\n");
        for (name, records) in functions {
            function(&mut out, &name, records)
                .with_context(|| format!("translating {source}'s function {name:?}"))?;
        }
        let _ = writeln!(out, "return F");
        files.insert(file, out);
    }
    Ok((files, index))
}

#[cfg(test)]
mod tests {
    use opensacred_data::netscript::Function;

    use super::*;

    fn record(kind: u16, operands: Vec<Operand>) -> Record {
        Record {
            kind,
            body: Body::Operands(operands),
        }
    }

    fn op(tag: u8, value: Value) -> Operand {
        Operand { tag, value }
    }

    #[test]
    fn keywords_become_snake_case_calls() {
        assert_eq!(snake("CreateNPC"), "create_npc");
        assert_eq!(snake("SetNPCState"), "set_npc_state");
        assert_eq!(snake("NPC_TalkTo"), "npc_talk_to");
        assert_eq!(snake("SetRG"), "set_rg");
        assert_eq!(snake("DelOBJ"), "del_obj");
        assert_eq!(snake("Toete_NPC"), "toete_npc");
        assert_eq!(call_name(1).unwrap(), "create_npc");
        assert_eq!(call_name(39).unwrap(), "type39");
        assert!(call_name(IF).is_err() && call_name(200).is_err());
    }

    #[test]
    fn operands_are_a_table_by_their_tags() {
        let create = record(
            1,
            vec![
                op(0x01, Value::Str("res:1".into())),
                op(0x04, Value::I32x3([1098, 905, 0])),
                op(0x0E, Value::Marker),
                op(0x9F, Value::Variable("dq_level".into())),
                op(
                    0x48,
                    Value::NumStr {
                        number: -1,
                        text: "Q".into(),
                        expression: Some("MaxVerstecke_r+Var(RNr)".into()),
                    },
                ),
                op(0x19, Value::I32x3([76, 100, 1])),
                op(0x19, Value::I32x3([77, 50, 1])),
            ],
        );
        assert_eq!(
            call(&create).unwrap(),
            "create_npc { _01 = \"res:1\", _04 = { 1098, 905, 0 }, _0E = true, \
             _19 = { { 76, 100, 1 }, { 77, 50, 1 } }, \
             _48 = { var(\"MaxVerstecke_r\" .. var(\"RNr\")), \"Q\" }, _9F = var(\"dq_level\") }"
        );
        let called = record(
            CALL,
            vec![
                op(0x01, Value::Str("sprich_10".into())),
                op(0x01, Value::Str("res:18103".into())),
            ],
        );
        assert_eq!(
            call(&called).unwrap(),
            "call(\"sprich_10\", { _01 = \"res:18103\" })"
        );
        assert_eq!(
            call(&record(50, vec![op(0x2A, Value::Truncated)])).unwrap(),
            "trigger_patch { _2A = truncated() }"
        );
        assert!(expression("A+VAR(B").is_err());
        assert_eq!(string("a\"b\\c\n"), "\"a\\\"b\\\\c\\n\"");
    }

    #[test]
    fn functions_go_to_their_sector_region_quest_or_shape() {
        assert_eq!(file_of("Sector3250511Init", -1), "sectors/3250511.luau");
        assert_eq!(file_of("Sector3250511Enter", -1), "sectors/3250511.luau");
        assert_eq!(file_of("Region12Init", -1), "regions/12.luau");
        assert_eq!(file_of("QIS_OnEnter7609", 7609), "quests/7609.luau");
        assert_eq!(
            file_of("fillchest_chestR12_3", -1),
            "functions/fillchest_chestR.luau"
        );
        assert_eq!(file_of("ToDo:-1.2", -1), "functions/ToDo.luau");
        assert_eq!(
            file_of("Dialog:DLG_17_START", -1),
            "functions/Dialog_DLG_START.luau"
        );
        assert_eq!(file_of("123", -1), "functions/_.luau");
    }

    /// Every tree translates, each function in the index once, and every file compiles.
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn every_tree_translates_into_files_that_compile() {
        use rayon::prelude::*;
        let game = crate::testing::repo_root().join("OriginalGame");
        let sources = crate::netscript::trees().unwrap();
        let counts: Vec<(usize, usize)> = sources
            .par_iter()
            .map(|source| {
                let tree = crate::netscript::tree(&game, source).unwrap();
                let (files, index) =
                    translate(source, &tree).unwrap_or_else(|e| panic!("{source}: {e:#}"));
                let entries: usize = index.functions.values().map(Vec::len).sum();
                assert_eq!(entries, tree.functions.len() + 2, "{source}");
                let compiler = mlua::chunk::Compiler::new();
                for (file, text) in &files {
                    compiler
                        .compile(text)
                        .unwrap_or_else(|e| panic!("{source} {file}: {e}"));
                }
                (files.len(), entries)
            })
            .collect();
        let functions: usize = counts.iter().map(|&(_, f)| f).sum();
        assert_eq!(functions, 403_900);
    }

    #[test]
    fn a_tree_translates_into_files_that_compile() {
        let tree = Tree {
            start: vec![
                record(1, vec![op(0x01, Value::Str("res:1".into()))]),
                record(IF, vec![op(0x52, Value::Str("TYPE_NPC_ZWERG".into()))]),
                record(21, vec![op(0x0B, Value::I32(7609))]),
                record(ELSE, Vec::new()),
                record(21, vec![op(0x0B, Value::I32(7625))]),
            ],
            quest: Vec::new(),
            functions: vec![
                Function {
                    name: "Sector3250511Enter".into(),
                    quest: -1,
                    region: 0,
                    records: vec![
                        record(24, vec![op(0x01, Value::Str("goblins".into()))]),
                        Record {
                            kind: BLOCK,
                            body: Body::Block(3),
                        },
                        record(
                            CALL,
                            vec![op(0x01, Value::Str("fillchest_chestR1_2".into()))],
                        ),
                        record(BLOCK_END, Vec::new()),
                    ],
                },
                Function {
                    name: "fillchest_chestR1_2".into(),
                    quest: -1,
                    region: 0,
                    records: vec![record(111, Vec::new())],
                },
            ],
            quests: Vec::new(),
            regions: Vec::new(),
        };
        let (files, index) = translate("bin/Test", &tree).unwrap();
        assert_eq!(
            files.keys().collect::<Vec<_>>(),
            [
                "functions/fillchest_chestR.luau",
                "sectors/3250511.luau",
                "start.luau"
            ]
        );
        assert_eq!(
            index.functions["fillchest_chestR1_2"],
            [Entry {
                file: "functions/fillchest_chestR.luau".into(),
                key: "fillchest_chestR1_2".into()
            }]
        );
        let start = &files["start.luau"];
        assert!(start.contains("if test { _52 = \"TYPE_NPC_ZWERG\" } then\n        set_up_quest { _0B = 7609 }\n    else\n        set_up_quest { _0B = 7625 }\n    end\n    return true\n"), "{start}");
        let sector = &files["sectors/3250511.luau"];
        assert!(sector.contains("    if not group_is_dead { _01 = \"goblins\" } then\n        return false\n    end\n"), "{sector}");
        for (file, text) in &files {
            mlua::chunk::Compiler::new()
                .compile(text)
                .unwrap_or_else(|e| panic!("{file}: {e}\n{text}"));
        }
    }
}
