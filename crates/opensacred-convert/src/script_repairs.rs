//! Repairs of the scripts' data (docs/02 §7.4) that `netscript` makes as it writes the trees.
//!
//! - A start script's cell cut short (#723, docs/decisions/2026-10-25-chest-cell.md): a repair
//!   names its trees and its record by the names in the record and the one after it; a tree it
//!   names whose start script lacks such a record is left as it is, with a warning.
//! - The quests' defects that the community reports (#356; docs/re/netscript.md, "The community's
//!   quest defects"), the maintainer's decisions of 2026-10-25: an [`Edit`] changes the functions
//!   of one name in every tree that holds them, and a [`Rename`] makes every record that names a
//!   function no tree defines name the one meant. A function whose records are not what its
//!   repair expects is left as it is, with a warning.

use opensacred_data::netscript::{Body, Function, Operand, Record, Tree, Value};

/// `TriggerPatch`, which puts cells under a trigger (docs/re/movement.md, "The chests' cells").
const TRIGGER_PATCH: u16 = 50;
/// `CreateObj`.
const CREATE_OBJ: u16 = 8;
/// `SetTimer`.
const SET_TIMER: u16 = 56;

/// A start script's `TriggerPatch` whose last cell (`0x2A`) runs past its record's end, so that the
/// client reads it from whatever follows the record (docs/re/netscript.md, "Truncated operands"),
/// given the cell (`0x04`) of the `CreateObj` after it: the cell of the object that it closes, as
/// the patch before every other object of its kind names.
#[derive(Debug, Clone, Copy)]
pub struct Cell {
    /// What the cell is, for the summary and the warnings.
    pub what: &'static str,
    /// The trees whose start scripts hold the record.
    pub trees: &'static [&'static str],
    /// The trigger that the patch names (`0x01`).
    pub trigger: &'static str,
    /// The name (`0x29`) of the object after it.
    pub object: &'static str,
}

/// The repairs that `netscript` makes.
pub const CELLS: [Cell; 1] = [CHEST];

/// A chest of the Underworld's and the multiplayer's start scripts, the one of their 22 chests of
/// its object (1339) whose patch names no cell; each of the other 21 has one patch, on its own
/// cell. Its cell is closed to every creature, as the other chests' are.
const CHEST: Cell = Cell {
    what: "a chest's cell",
    trees: &[
        "bin/Addon/NetScript",
        "bin/Addon/NetScriptCamp",
        "bin/Addon/TYPE_NPC_DAEMONIN",
        "bin/Addon/TYPE_NPC_DARKELVE",
        "bin/Addon/TYPE_NPC_ELVE",
        "bin/Addon/TYPE_NPC_GLADIATOR",
        "bin/Addon/TYPE_NPC_MAGICIAN",
        "bin/Addon/TYPE_NPC_SERAPHIM",
        "bin/Addon/TYPE_NPC_VAMPIRELADY",
        "bin/Addon/TYPE_NPC_ZWERG",
        "bin/NetScript",
    ],
    trigger: "chestTrigger",
    object: "chest_Pilzwald_14",
};

/// What [`apply`] did to a tree's start script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// No repair names the tree.
    None,
    /// Repaired: the number of cells given.
    Repaired(usize),
    /// A tree that a repair names whose start script lacks its record: left as it is.
    Mismatch(String),
}

/// Applies the repairs of `cells` that name the tree `source` to its start script `start`.
pub fn apply(cells: &[Cell], source: &str, start: &mut [Record]) -> Outcome {
    let mut repaired = 0;
    let mut named = false;
    for cell in cells.iter().filter(|c| c.trees.contains(&source)) {
        named = true;
        let found = (1..start.len()).find_map(|i| {
            let at = object_cell(&start[i], cell.object)?;
            truncated_patch(&start[i - 1], cell.trigger).then_some((i - 1, at))
        });
        let Some((i, at)) = found else {
            return Outcome::Mismatch(format!(
                "{source}'s start script has no patch of {} with a truncated cell before {}, \
                 which its repair gives {}; left as it is",
                cell.trigger, cell.object, cell.what
            ));
        };
        if let Body::Operands(ops) = &mut start[i].body
            && let Some(last) = ops.last_mut()
        {
            last.value = Value::I32x3(at);
            repaired += 1;
        }
    }
    if named {
        Outcome::Repaired(repaired)
    } else {
        Outcome::None
    }
}

/// Whether `record` is a `TriggerPatch` of `trigger` whose last operand is a truncated cell.
fn truncated_patch(record: &Record, trigger: &str) -> bool {
    let Body::Operands(ops) = &record.body else {
        return false;
    };
    record.kind == TRIGGER_PATCH
        && ops
            .first()
            .is_some_and(|o| o.tag == 0x01 && matches!(&o.value, Value::Str(s) if s == trigger))
        && ops
            .last()
            .is_some_and(|o| o.tag == 0x2A && o.value == Value::Truncated)
}

/// The cell of `record` if it is a `CreateObj` of the object `name`.
fn object_cell(record: &Record, name: &str) -> Option<[i32; 3]> {
    let Body::Operands(ops) = &record.body else {
        return None;
    };
    if record.kind != CREATE_OBJ
        || !ops
            .iter()
            .any(|o| o.tag == 0x29 && matches!(&o.value, Value::Str(s) if s == name))
    {
        return None;
    }
    ops.iter().find_map(|o| match (o.tag, &o.value) {
        (0x04, Value::I32x3(at)) => Some(*at),
        _ => None,
    })
}

/// A repair of the functions of one name, in every tree that holds them: `edit` changes a copy of
/// a function's records and answers what it did not find where they are not what it expects.
#[derive(Debug, Clone, Copy)]
pub struct Edit {
    /// What the repair mends, with its issue, for the summary and the warnings.
    pub what: &'static str,
    /// The functions' name.
    pub function: &'static str,
    pub edit: fn(&mut Vec<Record>) -> Result<(), String>,
}

/// A repair of the records that name a function no tree defines (`from`) where the one meant
/// (`to`) is defined: in a tree whose records name `from` and that defines `to` and not `from`,
/// every string operand `from` of its scripts and functions becomes `to`.
#[derive(Debug, Clone, Copy)]
pub struct Rename {
    /// What the repair mends, with its issue, for the summary and the warnings.
    pub what: &'static str,
    pub from: &'static str,
    pub to: &'static str,
}

/// The repairs of functions that `netscript` makes.
pub const EDITS: [Edit; 5] = [
    Edit {
        what: "quest 10260's item named (#957)",
        function: "fillchest_chestQJ_03",
        edit: |records| riddle_item(records),
    },
    Edit {
        what: "quest 1202's four days (#931)",
        function: "QIS_OnEnter1202",
        edit: |records| timer(records, "zeit1202", 4_320, 5_760),
    },
    Edit {
        what: "quest 1803's three days (#932)",
        function: "QIS_OnEnter1803",
        edit: |records| timer(records, "zeitum1803", 10_080, 4_320),
    },
    Edit {
        what: "quest 602's knight's id (#949)",
        function: "ok1_602",
        edit: |records| knight(records, 5),
    },
    Edit {
        what: "quest 602's knight's id, removed (#949)",
        function: "ritterweg_602",
        edit: |records| knight(records, 1),
    },
];

/// The repairs of names that `netscript` makes.
pub const RENAMES: [Rename; 3] = [
    Rename {
        what: "quest 1401's soldier's death function (#944)",
        from: "attack1401",
        to: "attacke1401",
    },
    Rename {
        what: "quest 1402's smugglers' function (#944)",
        from: "schmugler1402",
        to: "schmuggler1402",
    },
    Rename {
        what: "quest 5003's seeker's decline (#955)",
        from: "ablehnen5000",
        to: "ablehnen5003",
    },
];

/// The repairs of a tree's functions that were made, and the warnings of those that were not.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    /// What each repair made mends ([`Edit::what`], [`Rename::what`]), once a tree.
    pub made: Vec<&'static str>,
    pub warnings: Vec<String>,
}

/// Applies the repairs `edits` and `renames` to the tree `source`.
pub fn repair(edits: &[Edit], renames: &[Rename], source: &str, tree: &mut Tree) -> Report {
    let mut report = Report::default();
    for edit in edits {
        let mut made = false;
        for function in tree
            .functions
            .iter_mut()
            .filter(|f| f.name == edit.function)
        {
            let mut records = function.records.clone();
            match (edit.edit)(&mut records) {
                Ok(()) => {
                    function.records = records;
                    made = true;
                }
                Err(missing) => report.warnings.push(format!(
                    "{source}'s function {} lacks {missing}, which the repair of {} needs; \
                     left as it is",
                    edit.function, edit.what
                )),
            }
        }
        if made {
            report.made.push(edit.what);
        }
    }
    for rename in renames {
        let defined = |name: &str| tree.functions.iter().any(|f| f.name == name);
        let named = scripts(tree)
            .flatten()
            .any(|r| names(r, rename.from).next().is_some());
        if !named {
            continue;
        }
        if !defined(rename.to) || defined(rename.from) {
            report.warnings.push(format!(
                "{source} names {} but defines {}, which the repair of {} needs; left as it is",
                rename.from,
                if defined(rename.from) {
                    format!("{} itself", rename.from)
                } else {
                    format!("no {}", rename.to)
                },
                rename.what
            ));
            continue;
        }
        let Tree {
            start,
            quest,
            functions,
            ..
        } = tree;
        for records in [start, quest]
            .into_iter()
            .chain(functions.iter_mut().map(|f| &mut f.records))
        {
            rename_in(records, rename.from, rename.to);
        }
        report.made.push(rename.what);
    }
    report
}

/// A tree's start script, its quests' flags and its functions' records.
fn scripts(tree: &Tree) -> impl Iterator<Item = &Vec<Record>> {
    [&tree.start, &tree.quest]
        .into_iter()
        .chain(tree.functions.iter().map(|f: &Function| &f.records))
}

/// The operands of `record`.
fn operands(record: &Record) -> &[Operand] {
    match &record.body {
        Body::Operands(ops) => ops,
        _ => &[],
    }
}

/// The string operands of `record` that are `name`.
fn names<'a>(record: &'a Record, name: &'a str) -> impl Iterator<Item = &'a Operand> {
    operands(record)
        .iter()
        .filter(move |o| matches!(&o.value, Value::Str(s) if s == name))
}

/// Makes every string operand `from` of `records` `to`; the number made.
fn rename_in(records: &mut [Record], from: &str, to: &str) -> usize {
    let mut made = 0;
    for record in records {
        if let Body::Operands(ops) = &mut record.body {
            for o in ops {
                if matches!(&o.value, Value::Str(s) if s == from) {
                    o.value = Value::Str(to.to_owned());
                    made += 1;
                }
            }
        }
    }
    made
}

/// Whether `record` has the operand of `tag` and `value`.
fn has(record: &Record, tag: u8, value: &Value) -> bool {
    operands(record)
        .iter()
        .any(|o| o.tag == tag && o.value == *value)
}

fn text(s: &str) -> Value {
    Value::Str(s.to_owned())
}

/// The index of the one record of `records` of `kind` that `test` takes; `what` names it.
fn the(
    records: &[Record],
    what: &str,
    kind: u16,
    test: impl Fn(&Record) -> bool,
) -> Result<usize, String> {
    let mut found = (0..records.len()).filter(|&i| records[i].kind == kind && test(&records[i]));
    match (found.next(), found.next()) {
        (Some(i), None) => Ok(i),
        _ => Err(format!("one {what}")),
    }
}

/// 10260's right vase gives the item 4609 without a name, so the hero's list of main quest items
/// gets its entry of the key 0, which refuses every later quest item (`0x5D8280`): the record
/// names it `res:4609`, as 402 and 601 name theirs.
fn riddle_item(records: &mut [Record]) -> Result<(), String> {
    let i = the(records, "CreateObj of the item 4609", CREATE_OBJ, |r| {
        has(r, 0x02, &Value::I32(4609))
            && has(r, 0x61, &Value::Marker)
            && !operands(r).iter().any(|o| o.tag == 0x01)
    })?;
    if let Body::Operands(ops) = &mut records[i].body {
        ops.insert(
            0,
            Operand {
                tag: 0x01,
                value: text("res:4609"),
            },
        );
    }
    Ok(())
}

/// The `SetTimer` of the function `function` for `from` minutes runs for `to`.
fn timer(records: &mut [Record], function: &str, from: i32, to: i32) -> Result<(), String> {
    let i = the(
        records,
        &format!("SetTimer of {function} for {from} minutes"),
        SET_TIMER,
        |r| has(r, 0x01, &text(function)) && has(r, 0x38, &Value::I32(from)),
    )?;
    if let Body::Operands(ops) = &mut records[i].body {
        for o in ops.iter_mut().filter(|o| o.tag == 0x38) {
            o.value = Value::I32(to);
        }
    }
    Ok(())
}

/// 602's knight 17136, whom `ok1_602` walks once, is named 17163 after, the two digits swapped:
/// quest 1202's target, whom `ritterweg_602` would delete. The `count` operands become his.
fn knight(records: &mut [Record], count: usize) -> Result<(), String> {
    let found = records
        .iter()
        .map(|r| names(r, "res:17163").count())
        .sum::<usize>();
    if found != count {
        return Err(format!("{count} operands res:17163"));
    }
    rename_in(records, "res:17163", "res:17136");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(kind: u16, ops: Vec<(u8, Value)>) -> Record {
        Record {
            kind,
            body: Body::Operands(
                ops.into_iter()
                    .map(|(tag, value)| Operand { tag, value })
                    .collect(),
            ),
        }
    }

    fn patch(last: Value) -> Record {
        record(
            TRIGGER_PATCH,
            vec![(0x01, Value::Str("chestTrigger".into())), (0x2A, last)],
        )
    }

    fn chest(name: &str) -> Record {
        record(
            CREATE_OBJ,
            vec![
                (0x02, Value::I32(1339)),
                (0x04, Value::I32x3([1641, 911, 0])),
                (0x29, Value::Str(name.into())),
            ],
        )
    }

    #[test]
    fn the_truncated_cell_takes_the_chests_cell() {
        let mut start = vec![patch(Value::Truncated), chest("chest_Pilzwald_14")];
        assert_eq!(
            apply(&CELLS, "bin/NetScript", &mut start),
            Outcome::Repaired(1)
        );
        assert_eq!(start[0], patch(Value::I32x3([1641, 911, 0])));
        assert_eq!(start[1], chest("chest_Pilzwald_14"));
    }

    #[test]
    fn only_the_named_trees_and_records_are_repaired() {
        let mut start = vec![patch(Value::Truncated), chest("chest_Pilzwald_14")];
        assert_eq!(
            apply(&CELLS, "bin/TYPE_NPC_ELVE", &mut start),
            Outcome::None
        );
        assert_eq!(start[0], patch(Value::Truncated));
        for mut start in [
            vec![patch(Value::Truncated), chest("chest_Pilzwald_13")],
            vec![patch(Value::I32x3([1, 2, 0])), chest("chest_Pilzwald_14")],
            vec![chest("chest_Pilzwald_14"), patch(Value::Truncated)],
        ] {
            let before = start.clone();
            assert!(matches!(
                apply(&CELLS, "bin/NetScript", &mut start),
                Outcome::Mismatch(_)
            ));
            assert_eq!(start, before);
        }
    }

    fn function(name: &str, records: Vec<Record>) -> Function {
        Function {
            name: name.to_owned(),
            quest: -1,
            region: 0,
            records,
        }
    }

    fn tree(functions: Vec<Function>) -> Tree {
        Tree {
            start: Vec::new(),
            quest: Vec::new(),
            functions,
            quests: Vec::new(),
            regions: Vec::new(),
        }
    }

    fn vase(name: Option<&str>) -> Record {
        let mut ops = vec![
            (0x02, Value::I32(4609)),
            (0x3C, Value::Name("cpos:hero".into())),
            (0x61, Value::Marker),
        ];
        if let Some(name) = name {
            ops.insert(0, (0x01, text(name)));
        }
        record(CREATE_OBJ, ops)
    }

    #[test]
    fn an_edit_changes_every_function_of_its_name_that_holds_its_records() {
        let mut tree = tree(vec![
            function("fillchest_chestQJ_03", vec![vase(None)]),
            function("fillchest_chestQJ_02", vec![vase(None)]),
        ]);
        let report = repair(&EDITS, &[], "bin/NetScript", &mut tree);
        assert_eq!(report.made, ["quest 10260's item named (#957)"]);
        assert!(report.warnings.is_empty());
        assert_eq!(tree.functions[0].records, [vase(Some("res:4609"))]);
        assert_eq!(tree.functions[1].records, [vase(None)]);
        // Once named, the record is not what the repair expects: left as it is, with a warning.
        let report = repair(&EDITS, &[], "bin/NetScript", &mut tree);
        assert!(report.made.is_empty());
        assert_eq!(report.warnings.len(), 1);
        assert_eq!(tree.functions[0].records, [vase(Some("res:4609"))]);
    }

    fn set_timer(function: &str, minutes: i32) -> Record {
        record(
            SET_TIMER,
            vec![
                (0x0B, Value::I32(1803)),
                (0x38, Value::I32(minutes)),
                (0x01, text(function)),
            ],
        )
    }

    #[test]
    fn a_timer_takes_its_stated_days() {
        let mut records = vec![
            set_timer("zeitum1803", 4_320),
            set_timer("zeitum1803", 10_080),
        ];
        timer(&mut records, "zeitum1803", 10_080, 4_320).unwrap();
        assert_eq!(records, vec![set_timer("zeitum1803", 4_320); 2]);
        assert!(timer(&mut records, "zeitum1803", 10_080, 4_320).is_err());
    }

    fn goto(name: &str) -> Record {
        record(72, vec![(0x01, text(name))])
    }

    #[test]
    fn the_knight_is_named_where_his_digits_are_swapped() {
        let mut records = vec![goto("res:17136"), goto("res:17163"), goto("res:17163")];
        assert!(knight(&mut records.clone(), 3).is_err());
        knight(&mut records, 2).unwrap();
        assert_eq!(records, vec![goto("res:17136"); 3]);
    }

    fn death(name: &str) -> Record {
        record(1, vec![(0x01, text("res:17181")), (0x05, text(name))])
    }

    #[test]
    fn a_misspelt_function_takes_the_defined_name_where_the_tree_defines_it() {
        let rename = [Rename {
            what: "a test",
            from: "attack1401",
            to: "attacke1401",
        }];
        let mut good = tree(vec![
            function("Region13Init", vec![death("attack1401")]),
            function("attacke1401", Vec::new()),
        ]);
        good.start = vec![death("attack1401")];
        let report = repair(&[], &rename, "bin/NetScript", &mut good);
        assert_eq!(report.made, ["a test"]);
        assert_eq!(good.start, [death("attacke1401")]);
        assert_eq!(good.functions[0].records, [death("attacke1401")]);
        // Nothing named: nothing made, no warning.
        assert_eq!(
            repair(&[], &rename, "bin/NetScript", &mut good),
            Report::default()
        );
        // The function meant not defined, or the misspelt one defined: a warning.
        for defined in ["attack1401", "attacke1402"] {
            let mut bad = tree(vec![
                function("Region13Init", vec![death("attack1401")]),
                function(defined, Vec::new()),
            ]);
            let report = repair(&[], &rename, "bin/NetScript", &mut bad);
            assert!(report.made.is_empty());
            assert_eq!(report.warnings.len(), 1, "{defined}");
            assert_eq!(bad.functions[0].records, [death("attack1401")]);
        }
    }
}
