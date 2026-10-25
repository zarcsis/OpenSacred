//! Repairs of the scripts' data (docs/02 §7.4) that `netscript` makes as it writes the trees.
//!
//! - A start script's cell cut short (#723, docs/decisions/2026-10-25-chest-cell.md): a repair
//!   names its trees and its record by the names in the record and the one after it; a tree it
//!   names whose start script lacks such a record is left as it is, with a warning.
//! - The quests' defects that the community reports (#356; docs/re/netscript.md, "The community's
//!   quest defects"), the maintainer's decisions of 2026-10-25: an [`Edit`] changes the functions
//!   of one name in every tree that holds them, a [`Rename`] makes every record that names a
//!   function no tree defines name the one meant, and an [`Addition`] adds a function beside
//!   another. A function whose records are not what its repair expects is left as it is, with a
//!   warning.

use opensacred_data::netscript::{Body, Function, Operand, Record, Tree, Value};

/// `TriggerPatch`, which puts cells under a trigger (docs/re/movement.md, "The chests' cells").
const TRIGGER_PATCH: u16 = 50;
/// `CreateObj`.
const CREATE_OBJ: u16 = 8;
/// `SetTimer`.
const SET_TIMER: u16 = 56;
/// `CreateNPC`, `SetNPCState`, `SetBaseTrigger`, `TriggerQuest`, `CallFunktion`, `Text`,
/// `QuestBook`, `LoseQuest`, `IF`, `ELSE`, `SetButton`, `NOP`, `QuestKompassPos`,
/// `QuestKompassObj`, `ELSEIF`, `SetGroupState` and `InfoPlayer`.
const CREATE_NPC: u16 = 1;
const SET_NPC_STATE: u16 = 3;
const SET_BASE_TRIGGER: u16 = 4;
const TRIGGER_QUEST: u16 = 20;
const CALL_FUNKTION: u16 = 22;
const TEXT: u16 = 26;
const QUEST_BOOK: u16 = 53;
const LOSE_QUEST: u16 = 54;
const IF: u16 = 58;
const ELSE: u16 = 59;
const SET_BUTTON: u16 = 60;
const NOP: u16 = 62;
const QUEST_KOMPASS_POS: u16 = 63;
const QUEST_KOMPASS_OBJ: u16 = 64;
const ELSEIF: u16 = 66;
const SET_GROUP_STATE: u16 = 73;
const INFO_PLAYER: u16 = 132;

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

/// A function that a repair adds, with `records`, to every tree that holds the function `beside`
/// and lacks one of its name, with `beside`'s quest and region.
#[derive(Debug, Clone, Copy)]
pub struct Addition {
    /// What the repair mends, with its issue, for the summary and the warnings.
    pub what: &'static str,
    pub name: &'static str,
    pub beside: &'static str,
    pub records: fn() -> Vec<Record>,
}

/// The repairs of functions that `netscript` makes.
pub const EDITS: [Edit; 23] = [
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
    Edit {
        what: "quest 2203's win entering it (#936)",
        function: "tot2203",
        edit: pure_water,
    },
    Edit {
        what: "quest 1303's goods after the failure (#937)",
        function: "zeit1303",
        edit: goods_late,
    },
    Edit {
        what: "quest 7619's follower after the win (#938)",
        function: "od_7619",
        edit: |records| follower_won(records),
    },
    Edit {
        what: "quest 5005's loss on its last leg (#939)",
        function: "losepe",
        edit: pony_lost,
    },
    Edit {
        what: "quest 10255's restart (#940)",
        function: "btn_accept_10255_REstart",
        edit: spiders_again,
    },
    Edit {
        what: "quest 1401's day (#941)",
        function: "zeit1401",
        edit: snow_day,
    },
    Edit {
        what: "quest 1001's start (#942)",
        function: "QIS_OnEnter1001",
        edit: |records| pigface(records),
    },
    Edit {
        what: "quest 670's Glubba dying (#950)",
        function: "lose670",
        edit: glubba_dies,
    },
    Edit {
        what: "quest 670's loss (#950)",
        function: "QIS_OnLose670",
        edit: |records| glubba_lost(records),
    },
    Edit {
        what: "quest 9009's win before 2203's barrel (#935)",
        function: "Dialog:rg22_goodie1_ziel",
        edit: whisky,
    },
    Edit {
        what: "quests 301 and 302 lost at 55's start (#946)",
        function: "Init55",
        edit: chapter_two,
    },
    Edit {
        what: "quest 501 lost at 55's end (#947)",
        function: "NimmtDaemonenhorn",
        edit: horn,
    },
    Edit {
        what: "quest 1702's target hostile again (#948)",
        function: "ScriptAnimShareef",
        edit: baron,
    },
    Edit {
        what: "quest 2002's monsters once it is entered (#953)",
        function: "OMO-1tpraus20.2c",
        edit: |records| arena(records),
    },
    Edit {
        what: "quest 61's compass in the 5.7 dungeon (#958)",
        function: "OMO-1tpreinHQ_5.7.4",
        edit: dungeon_way,
    },
    Edit {
        what: "quest 61's compass after the four elements (#958)",
        function: "NimmtElementLuft_2",
        edit: air_last,
    },
    Edit {
        what: "Jacquaire made at 9006's end (#959)",
        function: "rg6_goodie1_start2",
        edit: jacquaire_made,
    },
    Edit {
        what: "Jacquaire not made by the box (#959)",
        function: "OMO9006rg6_goodie1_waldrand",
        edit: jacquaire_not_again,
    },
];

/// The functions that `netscript` adds.
pub const ADDITIONS: [Addition; 2] = [
    Addition {
        what: "quest 7619's follower's death after the win (#938)",
        name: FOLLOWER_WON,
        beside: "od_7619",
        records: Vec::new,
    },
    Addition {
        what: "quest 501's loss (#947)",
        name: NOBLES_LOST,
        beside: "QIS_OnEnter501",
        records: nobles_lost,
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

/// Applies the repairs `edits`, `renames` and `additions` to the tree `source`.
pub fn repair(
    edits: &[Edit],
    renames: &[Rename],
    additions: &[Addition],
    source: &str,
    tree: &mut Tree,
) -> Report {
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
    for addition in additions {
        let Some(beside) = tree.functions.iter().find(|f| f.name == addition.beside) else {
            continue;
        };
        if tree.functions.iter().any(|f| f.name == addition.name) {
            report.warnings.push(format!(
                "{source} defines {} already, which the repair of {} adds; left as it is",
                addition.name, addition.what
            ));
            continue;
        }
        let function = Function {
            name: addition.name.to_owned(),
            quest: beside.quest,
            region: beside.region,
            records: (addition.records)(),
        };
        tree.functions.push(function);
        report.made.push(addition.what);
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

/// The `IF` operand of the bit `n` of the variable `variable`.
fn bit(n: i32, variable: &str) -> Value {
    Value::NumStr {
        number: n,
        text: variable.to_owned(),
        expression: None,
    }
}

fn op(tag: u8, value: Value) -> Operand {
    Operand { tag, value }
}

fn new(kind: u16, operands: Vec<Operand>) -> Record {
    Record {
        kind,
        body: Body::Operands(operands),
    }
}

/// The `QuestBook` record of a line `line` in the quest `quest`'s log.
fn log_line(quest: i32, line: &str) -> Record {
    new(
        QUEST_BOOK,
        vec![
            op(0x0B, Value::I32(quest)),
            op(0x0B, Value::I32(1)),
            op(0x01, text(line)),
        ],
    )
}

/// The index of the one `QuestBook` of `records` that writes `line`.
fn the_line(records: &[Record], line: &str) -> Result<usize, String> {
    the(records, &format!("QuestBook of {line}"), QUEST_BOOK, |r| {
        has(r, 0x01, &text(line))
    })
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

/// 2203's six objects count whatever the quest's state, and `tot2203`'s first branch offers the
/// win with the quest never entered: the branch enters it first.
fn pure_water(records: &mut Vec<Record>) -> Result<(), String> {
    if the(records, "IF of bit 3 of 2203a", IF, |r| {
        has(r, 0x49, &bit(3, "2203a"))
    })? != 0
    {
        return Err("its IF of bit 3 of 2203a first".to_owned());
    }
    records.insert(1, new(TRIGGER_QUEST, vec![op(0x0B, Value::I32(2203))]));
    Ok(())
}

/// 1303's timer writes the failure, then the generic completion line, and its target's dialogue,
/// which tests nothing, still takes the goods: the completion line goes, and the target's
/// dialogue is taken away, as `Dialog:ziel1303` takes it at the delivery.
fn goods_late(records: &mut Vec<Record>) -> Result<(), String> {
    let i = the_line(records, "res:NQ_Log_Qend")?;
    records.remove(i);
    records.push(new(
        SET_NPC_STATE,
        vec![
            op(0x01, text("res:17177")),
            op(0x0A, Value::U16(0)),
            op(0x6B, Value::U16(0)),
            op(0x0E, Value::Marker),
        ],
    ));
    Ok(())
}

/// The function with no records that 7619's follower dies with once the quest is won.
const FOLLOWER_WON: &str = "od_threepwood_7619_sieg";

/// 7619's follower keeps the death function that loses the quest after the win: as the group's
/// death gives him his last dialogue, it gives him [`FOLLOWER_WON`] to die with.
fn follower_won(records: &mut [Record]) -> Result<(), String> {
    let i = the(
        records,
        "SetNPCState of the follower's last dialogue",
        SET_NPC_STATE,
        |r| {
            has(r, 0x01, &text("res:19545"))
                && has(r, 0x09, &text("dlg_threepwood_7619_c"))
                && !operands(r).iter().any(|o| o.tag == 0x05)
        },
    )?;
    if let Body::Operands(ops) = &mut records[i].body {
        ops.push(op(0x05, text(FOLLOWER_WON)));
    }
    Ok(())
}

/// 5005's loss on its last leg leaves the mage the dialogue of the win: he gets his own back
/// before the quest is lost, as `exitpe` gives it.
fn pony_lost(records: &mut Vec<Record>) -> Result<(), String> {
    let i = the(records, "LoseQuest", LOSE_QUEST, |_| true)?;
    records.insert(
        i,
        new(
            SET_NPC_STATE,
            vec![
                op(0x01, text("res:17277")),
                op(0x09, text("NPC_Dialog_Magician")),
            ],
        ),
    );
    Ok(())
}

/// 10255's restart writes no line, so its log ends on the failure's: it writes the task again.
fn spiders_again(records: &mut Vec<Record>) -> Result<(), String> {
    if records.iter().any(|r| r.kind == QUEST_BOOK) {
        return Err("a body without a QuestBook".to_owned());
    }
    records.push(log_line(10255, "res:DQ_10255_LOG_HEADER"));
    Ok(())
}

/// 1401's one-day timer writes the generic completion line alone, while the quest stays open: the
/// line goes, and the timer runs nothing.
fn snow_day(records: &mut Vec<Record>) -> Result<(), String> {
    the_line(records, "res:NQ_Log_Qend")?;
    if records.len() != 1 {
        return Err("a body of that line alone".to_owned());
    }
    records.clear();
    Ok(())
}

/// 1001's start writes the line of the kill in its `THEN`, the usual way while the target lives:
/// the line takes the place of the `ELSE`'s `NOP`, the branch where the target's item was taken
/// first, and the `NOP` its place.
fn pigface(records: &mut [Record]) -> Result<(), String> {
    let i = the(records, "IF of bit 20 of 1001 clear", IF, |r| {
        has(r, 0x4A, &bit(20, "1001"))
    })?;
    let kinds: Option<Vec<u16>> = records
        .get(i + 1..i + 5)
        .map(|after| after.iter().map(|r| r.kind).collect());
    if kinds.as_deref() != Some(&[QUEST_BOOK, QUEST_KOMPASS_OBJ, ELSE, NOP]) {
        return Err("a QuestBook, a QuestKompassObj, an ELSE and a NOP after its IF".to_owned());
    }
    records.swap(i + 1, i + 4);
    Ok(())
}

/// Glubba's death function is empty, so his death leaves 670 open with no way to finish it: it
/// loses its quest, as `tot301` loses 301.
fn glubba_dies(records: &mut Vec<Record>) -> Result<(), String> {
    if !records.is_empty() {
        return Err("an empty body".to_owned());
    }
    records.push(new(LOSE_QUEST, Vec::new()));
    Ok(())
}

/// 670's loss writes the generic completion line: it writes the failure line, as 301's death does.
fn glubba_lost(records: &mut [Record]) -> Result<(), String> {
    let i = the_line(records, "Res:NQ_Log_Qend")?;
    rename_in(&mut records[i..=i], "Res:NQ_Log_Qend", "Res:NQ_Log_QFAIL");
    Ok(())
}

/// 9009's target first tests whether the hero carries the barrel 17713, 2203's reward, and offers
/// to buy it, so that 9009's win (bit 25 of `goodies`) is never reached while the hero carries it:
/// the dialogue tests the win first (bit 25 set, bit 26 clear), with the win's line and button.
fn whisky(records: &mut Vec<Record>) -> Result<(), String> {
    let barrel = records
        .first()
        .is_some_and(|r| r.kind == IF && operands(r).iter().any(|o| o.tag == 0x3A));
    if !barrel {
        return Err("an IF of the barrel (0x3A) first".to_owned());
    }
    let win = the(records, "ELSEIF of bit 25 of goodies", ELSEIF, |r| {
        has(r, 0x49, &bit(25, "goodies"))
    })?;
    let (Some(line), Some(button)) = (records.get(win + 1), records.get(win + 2)) else {
        return Err("the win's Text and SetButton".to_owned());
    };
    if line.kind != TEXT || button.kind != SET_BUTTON {
        return Err("the win's Text and SetButton".to_owned());
    }
    let (line, button) = (line.clone(), button.clone());
    records[0].kind = ELSEIF;
    let test = new(
        IF,
        vec![op(0x49, bit(25, "goodies")), op(0x4A, bit(26, "goodies"))],
    );
    records.splice(0..0, [test, line, button]);
    Ok(())
}

/// The `LoseQuest` of the quest `quest`, which does nothing to a quest not entered or finished
/// (it needs the flag 4 and not 2, `0x46CBE0`).
fn lose(quest: i32) -> Record {
    new(LOSE_QUEST, vec![op(0x0B, Value::I32(quest))])
}

/// The index of the one `SetGroupState` of `records` of the group `group`.
fn group_state(records: &[Record], group: i32) -> Result<usize, String> {
    the(
        records,
        &format!("SetGroupState of the group {group}"),
        SET_GROUP_STATE,
        |r| has(r, 0x11, &Value::I32(group)),
    )
}

/// 55's start removes the people of 301 and 302, who hold their wins, testing neither: it loses
/// the two quests first.
fn chapter_two(records: &mut Vec<Record>) -> Result<(), String> {
    let i = group_state(records, 3012)?;
    records.splice(i..i, [lose(301), lose(302)]);
    Ok(())
}

/// The loss of quest 501, which has none: the failure's notice and line, as 301's loss and death
/// write them.
const NOBLES_LOST: &str = "QIS_OnLose501";

fn nobles_lost() -> Vec<Record> {
    vec![
        new(
            INFO_PLAYER,
            vec![op(0x0B, Value::I32(1)), op(0x01, text("res:17656"))],
        ),
        log_line(501, "Res:NQ_Log_QFAIL"),
    ]
}

/// 55's end removes the group 501, the nobles through whom both of 501's wins go, testing nothing
/// of it: it loses 501 first, whose loss [`NOBLES_LOST`] writes in its log.
fn horn(records: &mut Vec<Record>) -> Result<(), String> {
    let i = group_state(records, 501)?;
    records.insert(i, lose(501));
    Ok(())
}

/// 72's `BadToGood` makes the creatures 296 of area 31 the peaceful 297 (`RegionChange`), 1702's
/// target 17215 among them, whose death 1702's win needs: after it, the target takes back the
/// hostile side and the mode word that his `CreateNPC` gives him.
fn baron(records: &mut Vec<Record>) -> Result<(), String> {
    let i = the(records, "CallFunktion of BadToGood", CALL_FUNKTION, |r| {
        has(r, 0x01, &text("BadToGood"))
    })?;
    records.insert(
        i + 1,
        new(
            SET_NPC_STATE,
            vec![
                op(0x01, text("res:17215")),
                op(0x08, Value::Marker),
                op(0x12, Value::Marker),
                op(0x1F, Value::I32(1000)),
            ],
        ),
    );
    Ok(())
}

/// The arena's exit runs `monster2002` where bit 0 of `2002` is set, which the start script sets,
/// and clears it, testing nothing of the quest, so that leaving before the quest is taken leaves
/// it never offered: the branch needs the quest entered too (bit 1).
fn arena(records: &mut [Record]) -> Result<(), String> {
    let i = the(records, "ELSEIF of bit 0 of 2002 alone", ELSEIF, |r| {
        has(r, 0x49, &bit(0, "2002")) && !has(r, 0x49, &bit(1, "2002"))
    })?;
    if let Body::Operands(ops) = &mut records[i].body {
        ops.push(op(0x49, bit(1, "2002")));
    }
    Ok(())
}

/// A `QuestKompassPos` of quest 61 at the cell `(x, y)`.
fn compass(x: i32, y: i32) -> Record {
    new(
        QUEST_KOMPASS_POS,
        vec![
            op(0x0B, Value::I32(x)),
            op(0x0B, Value::I32(y)),
            op(0x0B, Value::I32(61)),
        ],
    )
}

/// The 5.7 dungeon's entrance by Frostgard (5.7.4) points 61's compass for no branch where the
/// elements of the air and of the fire (bits 6 and 9 of `61`) are both held, so that it keeps
/// pointing back at Frostgard's side: a branch of them points it at the way on, where the hero
/// comes in by the entrance 5.7.1 (`tpreinzHQ_5.7.1`, (5536, 2291)), whose exit leads on.
fn dungeon_way(records: &mut Vec<Record>) -> Result<(), String> {
    let both = |r: &Record| has(r, 0x49, &bit(6, "61")) && has(r, 0x49, &bit(9, "61"));
    if records.iter().any(both) {
        return Err("a chain without a branch of bits 6 and 9".to_owned());
    }
    let e = the(records, "ELSE", ELSE, |_| true)?;
    let test = new(ELSEIF, vec![op(0x49, bit(6, "61")), op(0x49, bit(9, "61"))]);
    records.splice(e..e, [test, compass(5536, 2291)]);
    Ok(())
}

/// Taking the element of the air ends by pointing 61's compass at the 5.7.4 mouth by Frostgard,
/// over the one that `ElementAbfrage` points at the mage once the four elements are held (bit 12
/// of `61`): it points so only where bit 12 is clear.
fn air_last(records: &mut Vec<Record>) -> Result<(), String> {
    let i = the(
        records,
        "QuestKompassPos of the 5.7.4 mouth",
        QUEST_KOMPASS_POS,
        |r| *r == compass(1093, 1750),
    )?;
    if i == 0 || records[i - 1].kind == IF {
        return Err("a QuestKompassPos of the 5.7.4 mouth outside a test".to_owned());
    }
    records.splice(i + 1..i + 1, [new(ELSE, Vec::new()), new(NOP, Vec::new())]);
    records.insert(i, new(IF, vec![op(0x4A, bit(12, "61"))]));
    Ok(())
}

/// Jacquaire 17523, as the box of `OMO9006rg6_goodie1_waldrand` makes him in Braverock once 9006's
/// step is done (bit 11 of `goodies`).
fn jacquaire() -> Record {
    new(
        CREATE_NPC,
        vec![
            op(0x01, text("res:17523")),
            op(0x02, Value::I32(689)),
            op(0x04, Value::Name("auftrag_goodie5".to_owned())),
            op(0x09, text("rghs_goodie1_auftrag")),
            op(0x0E, Value::Marker),
            op(0x46, Value::Marker),
            op(0x1F, Value::I32(20)),
            op(0x6B, Value::U16(1)),
            op(0x45, Value::Marker),
        ],
    )
}

/// 9006's step ends with `rg6_goodie1_start2`, which places the trigger whose 6 × 6 box alone
/// makes Jacquaire, so that a hero who leaves another way never makes him, and 9007 and 9011
/// wait on him: the step's end makes him, before it places the box.
fn jacquaire_made(records: &mut Vec<Record>) -> Result<(), String> {
    let i = the(
        records,
        "SetBaseTrigger of rg6_goodie1_waldrand",
        SET_BASE_TRIGGER,
        |r| has(r, 0x01, &text("rg6_goodie1_waldrand")),
    )?;
    records.insert(i, jacquaire());
    Ok(())
}

/// The box no longer makes Jacquaire, made at the step's end, so that it does not teleport him
/// back from where 9011 takes him (`CreateNPC` on a name held teleports its creature).
fn jacquaire_not_again(records: &mut Vec<Record>) -> Result<(), String> {
    let i = the(records, "CreateNPC of Jacquaire", CREATE_NPC, |r| {
        *r == jacquaire()
    })?;
    records.remove(i);
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
        let report = repair(&EDITS, &[], &[], "bin/NetScript", &mut tree);
        assert_eq!(report.made, ["quest 10260's item named (#957)"]);
        assert!(report.warnings.is_empty());
        assert_eq!(tree.functions[0].records, [vase(Some("res:4609"))]);
        assert_eq!(tree.functions[1].records, [vase(None)]);
        // Once named, the record is not what the repair expects: left as it is, with a warning.
        let report = repair(&EDITS, &[], &[], "bin/NetScript", &mut tree);
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
        let report = repair(&[], &rename, &[], "bin/NetScript", &mut good);
        assert_eq!(report.made, ["a test"]);
        assert_eq!(good.start, [death("attacke1401")]);
        assert_eq!(good.functions[0].records, [death("attacke1401")]);
        // Nothing named: nothing made, no warning.
        assert_eq!(
            repair(&[], &rename, &[], "bin/NetScript", &mut good),
            Report::default()
        );
        // The function meant not defined, or the misspelt one defined: a warning.
        for defined in ["attack1401", "attacke1402"] {
            let mut bad = tree(vec![
                function("Region13Init", vec![death("attack1401")]),
                function(defined, Vec::new()),
            ]);
            let report = repair(&[], &rename, &[], "bin/NetScript", &mut bad);
            assert!(report.made.is_empty());
            assert_eq!(report.warnings.len(), 1, "{defined}");
            assert_eq!(bad.functions[0].records, [death("attack1401")]);
        }
    }

    #[test]
    fn an_addition_joins_the_trees_that_hold_its_neighbour() {
        let mut tree = tree(vec![Function {
            quest: 7619,
            ..function("od_7619", Vec::new())
        }]);
        let report = repair(&[], &[], &ADDITIONS, "bin/NetScript", &mut tree);
        assert_eq!(
            report.made,
            ["quest 7619's follower's death after the win (#938)"]
        );
        assert_eq!(
            tree.functions[1],
            Function {
                quest: 7619,
                ..function(FOLLOWER_WON, Vec::new())
            }
        );
        // Defined already: a warning, nothing added.
        let report = repair(&[], &[], &ADDITIONS, "bin/NetScript", &mut tree);
        assert_eq!(report.warnings.len(), 1);
        assert_eq!(tree.functions.len(), 2);
    }

    #[test]
    fn an_addition_skips_a_tree_without_its_neighbour() {
        let mut other = tree(Vec::new());
        assert_eq!(
            repair(&[], &[], &ADDITIONS, "bin/NetScript", &mut other),
            Report::default()
        );
    }

    fn line(quest: i32, line: &str) -> Record {
        log_line(quest, line)
    }

    #[test]
    fn the_logs_lose_the_lines_that_contradict_them() {
        // 1303's timer: the completion line goes, the target's dialogue goes away.
        let mut records = vec![
            line(1303, "res:R13NQ03_Log_Qfail01"),
            line(1303, "res:NQ_Log_Qend"),
        ];
        goods_late(&mut records).unwrap();
        assert_eq!(records[0], line(1303, "res:R13NQ03_Log_Qfail01"));
        assert_eq!(records[1].kind, SET_NPC_STATE);
        assert_eq!(records.len(), 2);
        // 1401's timer: nothing left.
        let mut records = vec![line(1401, "res:NQ_Log_Qend")];
        snow_day(&mut records).unwrap();
        assert!(records.is_empty());
        assert!(snow_day(&mut records).is_err());
        // 10255's restart: the task again, once.
        let mut records = Vec::new();
        spiders_again(&mut records).unwrap();
        assert_eq!(records, [line(10255, "res:DQ_10255_LOG_HEADER")]);
        assert!(spiders_again(&mut records).is_err());
        // 670's loss: the failure line.
        let mut records = vec![line(670, "Res:NQ_Log_Qend")];
        glubba_lost(&mut records).unwrap();
        assert_eq!(records, [line(670, "Res:NQ_Log_QFAIL")]);
    }

    #[test]
    fn pigfaces_kill_line_takes_the_else() {
        let start = |first: Record, last: Record| {
            vec![
                new(IF, vec![op(0x4A, bit(20, "1001"))]),
                first,
                new(QUEST_KOMPASS_OBJ, vec![op(0x01, text("Res:17041"))]),
                new(ELSE, Vec::new()),
                last,
            ]
        };
        let kill = line(1001, "Res:R10NQ01_Log_Qziel01");
        let nop = new(NOP, Vec::new());
        let mut records = start(kill.clone(), nop.clone());
        pigface(&mut records).unwrap();
        assert_eq!(records, start(nop, kill));
        assert!(pigface(&mut records).is_err());
    }

    #[test]
    fn whiskys_win_comes_before_the_barrel() {
        let text_of = |s: &str| new(TEXT, vec![op(0x1E, text(s))]);
        let button = |s: &str| {
            new(
                SET_BUTTON,
                vec![op(0x01, text("res:1024")), op(0x01, text(s))],
            )
        };
        let barrel = |kind| {
            new(
                kind,
                vec![op(0x3A, Value::Strs("HERO".into(), "res:17713".into()))],
            )
        };
        let win = new(ELSEIF, vec![op(0x49, bit(25, "goodies"))]);
        let mut records = vec![
            barrel(IF),
            text_of("Wirt_07_1000"),
            win.clone(),
            text_of("GQ_RG22_ZIEL_SIEG"),
            button("rg22_goodie1_sieg"),
            new(ELSE, Vec::new()),
            new(NOP, Vec::new()),
        ];
        whisky(&mut records).unwrap();
        assert_eq!(
            records[..5],
            [
                new(
                    IF,
                    vec![op(0x49, bit(25, "goodies")), op(0x4A, bit(26, "goodies"))]
                ),
                text_of("GQ_RG22_ZIEL_SIEG"),
                button("rg22_goodie1_sieg"),
                barrel(ELSEIF),
                text_of("Wirt_07_1000"),
            ]
        );
        assert_eq!(records[5], win);
        assert!(whisky(&mut records).is_err());
    }

    #[test]
    fn the_quests_that_take_others_people_lose_them_first() {
        let removal = |group| {
            new(
                SET_GROUP_STATE,
                vec![op(0x11, Value::I32(group)), op(0x51, Value::Marker)],
            )
        };
        let mut records = vec![removal(3012), removal(301)];
        chapter_two(&mut records).unwrap();
        assert_eq!(records, [lose(301), lose(302), removal(3012), removal(301)]);
        let mut records = vec![removal(501), removal(551)];
        horn(&mut records).unwrap();
        assert_eq!(records, [lose(501), removal(501), removal(551)]);
        assert_eq!(nobles_lost()[1], log_line(501, "Res:NQ_Log_QFAIL"));
        let call = new(CALL_FUNKTION, vec![op(0x01, text("BadToGood"))]);
        let mut records = vec![call.clone(), new(NOP, Vec::new())];
        baron(&mut records).unwrap();
        assert_eq!(records[0], call);
        assert!(
            has(&records[1], 0x01, &text("res:17215")) && has(&records[1], 0x08, &Value::Marker)
        );
    }

    #[test]
    fn the_arena_and_the_dungeon_test_their_quests() {
        let mut records = vec![new(ELSEIF, vec![op(0x49, bit(0, "2002"))])];
        arena(&mut records).unwrap();
        assert_eq!(
            records,
            [new(
                ELSEIF,
                vec![op(0x49, bit(0, "2002")), op(0x49, bit(1, "2002"))]
            )]
        );
        assert!(arena(&mut records).is_err());
        let mut records = vec![
            new(IF, vec![op(0x49, bit(6, "61")), op(0x4A, bit(9, "61"))]),
            compass(5537, 2279),
            new(ELSE, Vec::new()),
            new(NOP, Vec::new()),
        ];
        dungeon_way(&mut records).unwrap();
        assert_eq!(records[2].kind, ELSEIF);
        assert_eq!(records[3], compass(5536, 2291));
        assert_eq!(records[4].kind, ELSE);
        assert!(dungeon_way(&mut records).is_err());
        let mut records = vec![
            new(NOP, Vec::new()),
            compass(1093, 1750),
            new(NOP, Vec::new()),
        ];
        air_last(&mut records).unwrap();
        assert_eq!(
            records,
            [
                new(NOP, Vec::new()),
                new(IF, vec![op(0x4A, bit(12, "61"))]),
                compass(1093, 1750),
                new(ELSE, Vec::new()),
                new(NOP, Vec::new()),
                new(NOP, Vec::new()),
            ]
        );
        assert!(air_last(&mut records).is_err());
    }

    #[test]
    fn jacquaire_comes_with_the_steps_end_not_the_box() {
        let trigger = new(
            SET_BASE_TRIGGER,
            vec![op(0x01, text("rg6_goodie1_waldrand"))],
        );
        let mut start = vec![trigger.clone()];
        jacquaire_made(&mut start).unwrap();
        assert_eq!(start, [jacquaire(), trigger]);
        let mut the_box = vec![new(NOP, Vec::new()), jacquaire()];
        jacquaire_not_again(&mut the_box).unwrap();
        assert_eq!(the_box, [new(NOP, Vec::new())]);
        assert!(jacquaire_not_again(&mut the_box).is_err());
    }

    #[test]
    fn the_quests_states_follow_their_ends() {
        // 2203's win enters the quest first.
        let test = new(IF, vec![op(0x49, bit(3, "2203a"))]);
        let mut records = vec![test.clone(), new(NOP, Vec::new())];
        pure_water(&mut records).unwrap();
        assert_eq!(
            records[1],
            new(TRIGGER_QUEST, vec![op(0x0B, Value::I32(2203))])
        );
        // 7619's follower dies with the empty function once the group is dead.
        let dialogue = |extra: Option<&str>| {
            let mut ops = vec![
                op(0x01, text("res:19545")),
                op(0x09, text("dlg_threepwood_7619_c")),
                op(0x39, Value::Marker),
            ];
            ops.extend(extra.map(|f| op(0x05, text(f))));
            new(SET_NPC_STATE, ops)
        };
        let mut records = vec![dialogue(None)];
        follower_won(&mut records).unwrap();
        assert_eq!(records, [dialogue(Some(FOLLOWER_WON))]);
        assert!(follower_won(&mut records).is_err());
        // 5005's mage gets his dialogue back before the loss.
        let mut records = vec![new(LOSE_QUEST, Vec::new())];
        pony_lost(&mut records).unwrap();
        assert_eq!(records[0].kind, SET_NPC_STATE);
        assert!(has(&records[0], 0x09, &text("NPC_Dialog_Magician")));
        assert_eq!(records[1].kind, LOSE_QUEST);
        // Glubba's death loses 670.
        let mut records = Vec::new();
        glubba_dies(&mut records).unwrap();
        assert_eq!(records, [new(LOSE_QUEST, Vec::new())]);
        assert!(glubba_dies(&mut records).is_err());
    }
}
