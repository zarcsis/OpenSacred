//! The scripts run in Luau (#726, part of #720; ADR-13): the hero's tree's start script and its
//! sectors' `Init` and `Enter` functions, as the converter translates them
//! (`netscript/<tree>/`), run by `opensacred_scripts`, their records handed back as the records
//! they were (`opensacred_scripts::record`) to the readers of `npcs`, which place what they make
//! as before. The game keeps the scripts' state (`opensacred_rules::quests::Scripts`): the
//! variables that `SetVar`, `SetVarBit`, `UnsetVarBit`, `IncVar` and `RndVar` set, which an
//! `IF`'s tests read (`0x48`, `0x49`, `0x4A`, `0x6D`, `0x6E`; docs/re/netscript.md, "Variables
//! and groups"), and the quests' flags, whose transitions `SetUpQuest` and `TriggerQuest` run
//! their sections in their middle, as the client's do (`0x48F7F0`, `0x48F030`). A condition of
//! anything else fails, an action the readers do not take passes.

use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use opensacred_data::luau::{BYTECODE, INDEX, Index};
use opensacred_data::netscript::{Function, Record, Tree};
use opensacred_rules::quests::{self, Transition};
use opensacred_rules::spawning::Rng;
use opensacred_scripts::{Handled, Host, Operand, Runtime};

use crate::{files, npcs};

/// The start script's function in a tree (`opensacred-convert`'s `luau::START`).
const START: &str = "StartCode";

/// The seed of the scripts' `rand()` in a new game, which `RndVar` draws from.
const SEED: usize = 0x5C;

/// What the scripts ran: the tree's start script's records, with those of the quests' sections
/// that its transitions ran in their middle, and each sector function's records, as a tree.
pub fn run(assets: &Path, tree: &str) -> Result<Tree> {
    let started = Instant::now();
    let dir = assets.join("netscript").join(tree);
    let index_path = dir.join(INDEX);
    let index: Index = ron::from_str(
        &files::read_to_string(&index_path)
            .with_context(|| format!("reading {}", index_path.display()))?,
    )
    .with_context(|| format!("reading {}", index_path.display()))?;
    let sectors: Vec<String> = index
        .functions
        .keys()
        .filter(|name| npcs::sector_function(name).is_some())
        .cloned()
        .collect();
    let runtime = Runtime::new(tree, index, move |file| {
        let stem = file.strip_suffix(".luau").unwrap_or(file);
        files::read(dir.join(format!("{stem}.{BYTECODE}"))).ok()
    })
    .with_context(|| format!("starting the scripts of {tree}"))?;
    let mut state = quests::Scripts::default();
    let mut rng = Rng::of(0, 0, SEED);
    let start = records(&runtime, tree, START, &mut state, &mut rng);
    let functions: Vec<Function> = sectors
        .into_iter()
        .map(|name| Function {
            records: records(&runtime, tree, &name, &mut state, &mut rng),
            name,
            quest: -1,
            region: 0,
        })
        .collect();
    bevy::log::info!(
        "scripts: {tree}'s start and {} sector functions ran in {:.2} s, {} files loaded; the \
         records without a meaning yet: {}",
        functions.len(),
        started.elapsed().as_secs_f64(),
        runtime.files(),
        runtime.unknown().join(" "),
    );
    Ok(Tree {
        start,
        quest: Vec::new(),
        functions,
        quests: Vec::new(),
        regions: Vec::new(),
    })
}

/// The records of the function `function` of the tree `tree` as it runs, the scripts' state
/// `state` with it; none where it fails, which the log tells.
fn records(
    runtime: &Runtime,
    tree: &str,
    function: &str,
    state: &mut quests::Scripts,
    rng: &mut Rng,
) -> Vec<Record> {
    let mut recorder = Recorder {
        tree,
        state,
        rng,
        records: Vec::new(),
    };
    if let Err(e) = runtime.call(function, &mut recorder) {
        bevy::log::warn!("scripts: {e}");
    }
    recorder.records
}

/// The host of a run: the hero's tree, the scripts' state, their `rand()`, and the records in
/// the order run.
struct Recorder<'a> {
    tree: &'a str,
    state: &'a mut quests::Scripts,
    rng: &'a mut Rng,
    records: Vec<Record>,
}

impl Host for Recorder<'_> {
    fn record(&mut self, name: &str, operands: &[Operand], ran: &[bool]) -> Handled {
        let tags = match operands.first() {
            Some(Operand::Tags(tags)) => Tags(Some(tags)),
            _ => Tags(None),
        };
        // A condition: every test of an `IF` holds (`0x4987B0`).
        if name == "test" {
            return tags.test(self.tree, &self.state.variables);
        }
        if ran.is_empty()
            && let Some(record) = opensacred_scripts::record(name, operands)
        {
            self.records.push(record);
        }
        let variables = &mut self.state.variables;
        match name {
            "set_up_quest" | "trigger_quest" => {
                let transition = if name == "set_up_quest" {
                    Transition::SetUp
                } else {
                    Transition::Trigger
                };
                let Some(quest) = tags
                    .numbers("_0B")
                    .first()
                    .and_then(|&q| u32::try_from(q).ok())
                else {
                    return Handled::Done;
                };
                match self.state.quests.step(transition, quest, ran) {
                    Some(section) => Handled::Run(section.function(quest)),
                    None => Handled::Done,
                }
            }
            "set_var" => {
                let names = tags.names("_01", variables);
                let value = match (tags.numbers("_0B").last(), names.get(1)) {
                    (Some(&value), _) => value,
                    (None, Some(other)) => variables.value(other),
                    (None, None) => return Handled::Done,
                };
                if let Some(name) = names.first() {
                    variables.set(name, value);
                }
                Handled::Done
            }
            "set_var_bit" => {
                if let Some(name) = tags.names("_01", variables).first() {
                    let mask = tags
                        .numbers("_0B")
                        .iter()
                        .fold(0_u32, |mask, &bit| mask | 1 << bit.rem_euclid(32));
                    let old = u32::from_ne_bytes(variables.value(name).to_ne_bytes());
                    variables.set(name, i32::from_ne_bytes((old | mask).to_ne_bytes()));
                }
                Handled::Done
            }
            "unset_var_bit" => {
                if let (Some(name), Some(&bit)) = (
                    tags.names("_01", variables).first(),
                    tags.numbers("_0B").last(),
                ) && let Some(old) = variables.get(name)
                {
                    let old = u32::from_ne_bytes(old.to_ne_bytes());
                    let new = old & !(1 << bit.rem_euclid(32));
                    variables.set(name, i32::from_ne_bytes(new.to_ne_bytes()));
                }
                Handled::Done
            }
            "inc_var" => {
                let names = tags.names("_01", variables);
                let step = match (tags.numbers("_0B").last(), names.get(1)) {
                    (Some(&step), _) => step,
                    (None, Some(other)) => variables.value(other),
                    (None, None) => return Handled::Done,
                };
                if let Some(name) = names.first() {
                    // The old value kept where the sum is smaller (`0x49C930`).
                    let value = match variables.get(name) {
                        Some(old) => old
                            .checked_add(step)
                            .filter(|&sum| sum >= old)
                            .unwrap_or(old),
                        None => step,
                    };
                    variables.set(name, value);
                }
                Handled::Done
            }
            "rnd_var" => {
                if let (Some(name), [low, high, ..]) = (
                    tags.names("_01", variables).first(),
                    tags.numbers("_0B").as_slice(),
                ) && let Ok(span) = u32::try_from(high - low + 1)
                    && span > 0
                {
                    let drawn = i32::try_from(self.rng.below(span)).unwrap_or(0);
                    variables.set(name, low + drawn);
                }
                Handled::Done
            }
            _ => Handled::Done,
        }
    }

    fn var(&mut self, name: &str) -> i32 {
        self.state.variables.value(name)
    }
}

/// A record's operands by their tags, none for a record of none.
struct Tags<'a>(Option<&'a std::collections::BTreeMap<String, Operand>>);

impl Tags<'_> {
    /// The values of the tag `key`: one, or each of a tag that repeats.
    fn values(&self, key: &str) -> Vec<&Operand> {
        match self.0.and_then(|tags| tags.get(key)) {
            Some(Operand::List(list))
                if list
                    .iter()
                    .all(|v| matches!(v, Operand::Number(_) | Operand::Text(_))) =>
            {
                list.iter().collect()
            }
            Some(one) => vec![one],
            None => Vec::new(),
        }
    }

    /// The whole numbers of the tag `key`.
    fn numbers(&self, key: &str) -> Vec<i32> {
        self.values(key)
            .into_iter()
            .filter_map(|v| match v {
                Operand::Number(n) => int(*n),
                _ => None,
            })
            .collect()
    }

    /// The variables' names of the tag `key`, as the engine expands them ([`expand`]).
    fn names(&self, key: &str, variables: &quests::Variables) -> Vec<String> {
        self.texts(key)
            .into_iter()
            .map(|name| expand(name, variables))
            .collect()
    }

    /// The texts of the tag `key`.
    fn texts(&self, key: &str) -> Vec<&str> {
        self.values(key)
            .into_iter()
            .filter_map(|v| match v {
                Operand::Text(text) => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    /// An `IF`'s answer, every test of it to hold (`0x4987B0`): `0x48` a variable equal to the
    /// number, `0x6D` greater, `0x6E` less, `0x49` its bit set and `0x4A` clear, each false for
    /// a missing variable; `0x52` the hero's tree, `tree`, named (**hypothesis**: the hero's
    /// type, by the name of its script, `TYPE_NPC_GLADIATOR`, which the quests' `OnEnter` ask
    /// before they place a class's own NPCs); a test of any other tag has no meaning yet.
    fn test(&self, tree: &str, variables: &quests::Variables) -> Handled {
        let Some(tags) = self.0 else {
            return Handled::Unknown;
        };
        for (key, operand) in tags {
            if key == "_52" {
                match operand {
                    Operand::Text(name) if name.eq_ignore_ascii_case(tree) => continue,
                    Operand::Text(_) => return Handled::Answer(false),
                    _ => return Handled::Unknown,
                }
            }
            let tests: Vec<&Operand> = match operand {
                Operand::List(list) if list.iter().all(|t| matches!(t, Operand::List(_))) => {
                    list.iter().collect()
                }
                one => vec![one],
            };
            for test in tests {
                let Operand::List(pair) = test else {
                    return Handled::Unknown;
                };
                let [Operand::Number(n), Operand::Text(name)] = pair.as_slice() else {
                    return Handled::Unknown;
                };
                let (Some(n), Some(value)) = (int(*n), variables.get(&expand(name, variables)))
                else {
                    return Handled::Answer(false);
                };
                let bits = u32::from_ne_bytes(value.to_ne_bytes());
                let bit = 1_u32 << n.rem_euclid(32);
                let holds = match key.as_str() {
                    "_48" => value == n,
                    "_6D" => value > n,
                    "_6E" => value < n,
                    "_49" => bits & bit != 0,
                    "_4A" => bits & bit == 0,
                    _ => return Handled::Unknown,
                };
                if !holds {
                    return Handled::Answer(false);
                }
            }
        }
        Handled::Answer(true)
    }
}

/// A name as the engine expands it as it runs: each `+Var(NAME)` the value of the variable `NAME`
/// (`grabstein_dialog_+Var(grvarNr)`, docs/02 §7.6).
fn expand(name: &str, variables: &quests::Variables) -> String {
    const MARK: &str = "+var(";
    let mut out = String::new();
    let mut rest = name;
    while let Some(at) = rest.to_ascii_lowercase().find(MARK) {
        out.push_str(&rest[..at]);
        let after = &rest[at + MARK.len()..];
        let Some(end) = after.find(')') else {
            out.push_str(&rest[at..]);
            return out;
        };
        out.push_str(&variables.value(&after[..end]).to_string());
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// A whole number of the scripts, if it is one an `i32` holds.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a number of the scripts is whole and within an i32, which the test before it checks"
)]
fn int(n: f64) -> Option<i32> {
    (n.fract() == 0.0 && n >= f64::from(i32::MIN) && n <= f64::from(i32::MAX)).then_some(n as i32)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use opensacred_data::luau::Entry;
    use opensacred_data::netscript::Body;

    use super::*;

    /// A runtime of the Luau files `sources` (name, source), each function indexed by its key.
    fn runtime(sources: &[(&str, &str)]) -> Runtime {
        let mut index = Index::default();
        let mut bytecode = BTreeMap::new();
        for &(file, source) in sources {
            bytecode.insert(
                file.to_owned(),
                opensacred_scripts::compile(source).expect("the test's source compiles"),
            );
            for line in source.lines() {
                if let Some(key) = line
                    .trim()
                    .strip_prefix("F[\"")
                    .and_then(|rest| rest.split_once('"'))
                    .map(|(key, _)| key)
                {
                    index
                        .functions
                        .entry(key.to_owned())
                        .or_default()
                        .push(Entry {
                            file: file.to_owned(),
                            key: key.to_owned(),
                        });
                }
            }
        }
        Runtime::new("TEST", index, move |file| bytecode.get(file).cloned())
            .expect("the runtime starts")
    }

    /// The records' types.
    fn kinds(records: &[Record]) -> Vec<u16> {
        records.iter().map(|r| r.kind).collect()
    }

    #[test]
    fn the_start_runs_the_quests_sections_in_the_middle_of_their_transitions() {
        let runtime = runtime(&[
            (
                "start.luau",
                "local F = {}\n\
                 F[\"StartCode\"] = function()\n\
                 def_pos { _01 = \"POS_AUFTRAG_15004\", _0B = { 3370, 2524 } }\n\
                 set_up_quest { _0B = 15004 }\n\
                 set_up_quest { _0B = 7 }\n\
                 set_up_quest { _0B = 15004 }\n\
                 set_up_quest { _0B = 1 }\n\
                 trigger_quest { _0B = 1 }\n\
                 trigger_quest { _0B = 2 }\n\
                 return true\n\
                 end\n\
                 return F",
            ),
            (
                "quests.luau",
                "local F = {}\n\
                 F[\"QIS_OnSetUp15004\"] = function()\n\
                 create_npc { _02 = 288, _04 = \"POS_AUFTRAG_15004\", _0E = true }\n\
                 return true\n\
                 end\n\
                 F[\"QIS_Trigger1\"] = function()\n\
                 return true\n\
                 end\n\
                 F[\"QIS_OnEnter1\"] = function()\n\
                 create_npc { _02 = 679, _04 = \"novizin1\" }\n\
                 return true\n\
                 end\n\
                 F[\"QIS_OnEnter2\"] = function()\n\
                 create_npc { _02 = 679, _04 = \"novizin2\" }\n\
                 return true\n\
                 end\n\
                 return F",
            ),
        ]);
        let mut state = quests::Scripts::default();
        let mut rng = Rng::of(0, 0, SEED);
        let records = records(&runtime, "TEST", START, &mut state, &mut rng);
        // Quest 15004's giver after its first set-up alone; quest 1, set up and triggered, its
        // trigger through, entered with its novice; quest 2, never set up, not.
        assert_eq!(kinds(&records), [23, 21, 1, 21, 21, 21, 20, 1, 20]);
        let Body::Operands(novice) = &records[7].body else {
            panic!("the novice has operands");
        };
        assert!(
            novice
                .iter()
                .any(|o| o.tag == 0x02 && o.value == opensacred_data::netscript::Value::I32(679))
        );
        assert!(state.quests.in_log(1) && !state.quests.in_log(2));
    }

    #[test]
    fn the_variables_hold_what_the_scripts_set_and_the_tests_read_them() {
        let runtime = runtime(&[(
            "start.luau",
            "local F = {}\n\
             F[\"StartCode\"] = function()\n\
             set_var { _01 = \"SoldatCnt\", _0B = 565943 }\n\
             set_var_bit { _01 = \"bits\", _0B = { 0, 3 } }\n\
             unset_var_bit { _01 = \"bits\", _0B = 0 }\n\
             inc_var { _01 = \"count\", _0B = 2 }\n\
             inc_var { _01 = \"count\", _0B = -5 }\n\
             rnd_var { _01 = \"pool\", _0B = { 0, 3 } }\n\
             if test { _6D = { 0, \"SoldatCnt\" } } then def_pos { _01 = \"a\", _0B = { 1, 1 } } end\n\
             if test { _49 = { 3, \"bits\" } } then def_pos { _01 = \"b\", _0B = { 1, 1 } } end\n\
             if test { _49 = { 0, \"bits\" } } then def_pos { _01 = \"c\", _0B = { 1, 1 } } end\n\
             if test { _48 = { 2, \"count\" } } then def_pos { _01 = \"d\", _0B = { 1, 1 } } end\n\
             if test { _48 = { 0, \"missing\" } } then def_pos { _01 = \"e\", _0B = { 1, 1 } } end\n\
             if test { _A0 = true } then def_pos { _01 = \"f\", _0B = { 1, 1 } } end\n\
             if test { _52 = \"TYPE_NPC_GLADIATOR\" } then def_pos { _01 = \"g\", _0B = { 1, 1 } } end\n\
             if test { _52 = \"TYPE_NPC_SERAPHIM\" } then def_pos { _01 = \"h\", _0B = { 1, 1 } } end\n\
             return true\n\
             end\n\
             return F",
        )]);
        let mut state = quests::Scripts::default();
        let mut rng = Rng::of(0, 0, SEED);
        let records = records(&runtime, "TYPE_NPC_GLADIATOR", START, &mut state, &mut rng);
        let variables = &state.variables;
        assert_eq!(variables.get("bits"), Some(8));
        // A negative step that would lower it keeps the old value (`0x49C930`).
        assert_eq!(variables.get("count"), Some(2));
        assert!((0..=3).contains(&variables.value("pool")));
        // `a`, `b` and `d` held, and `g` the hero's tree; `c` a bit cleared, `e` a missing
        // variable, `f` no meaning yet, `h` another tree.
        let named: Vec<String> = records
            .iter()
            .filter(|r| r.kind == 23)
            .filter_map(|r| match &r.body {
                Body::Operands(ops) => ops.iter().find_map(|o| match &o.value {
                    opensacred_data::netscript::Value::Str(name) => Some(name.clone()),
                    _ => None,
                }),
                _ => None,
            })
            .collect();
        assert_eq!(named, ["a", "b", "d", "g"]);
        assert_eq!(runtime.unknown(), ["test"]);
    }

    #[test]
    fn a_loop_of_the_start_runs_through_its_names_built_as_it_runs() {
        // `grabsteindialogvars` of the start scripts: 194 variables, each named by the loop's.
        let runtime = runtime(&[(
            "start.luau",
            "local F = {}\n\
             F[\"StartCode\"] = function()\n\
             set_var { _01 = \"grvarNr\", _0B = 0 }\n\
             call(\"grabsteindialogvars\", {})\n\
             return true\n\
             end\n\
             F[\"grabsteindialogvars\"] = function()\n\
             if test { _6E = { 194, \"grvarNr\" } } then\n\
             set_var { _01 = \"grabstein_dialog_+Var(grvarNr)\", _0B = 7 }\n\
             inc_var { _01 = \"grvarNr\", _0B = 1 }\n\
             return \"again\"\n\
             else\n\
             return true\n\
             end\n\
             end\n\
             return F",
        )]);
        let mut state = quests::Scripts::default();
        let mut rng = Rng::of(0, 0, SEED);
        records(&runtime, "TEST", START, &mut state, &mut rng);
        assert_eq!(state.variables.get("grvarNr"), Some(194));
        assert_eq!(state.variables.get("grabstein_dialog_0"), Some(7));
        assert_eq!(state.variables.get("grabstein_dialog_193"), Some(7));
        assert_eq!(state.variables.get("grabstein_dialog_194"), None);
    }
}
