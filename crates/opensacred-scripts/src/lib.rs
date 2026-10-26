//! `opensacred-scripts`: the runtime of the scripts translated into Luau (#725, part of #720;
//! ADR-13, `opensacred-convert`'s `luau`).
//!
//! A [`Runtime`] holds one tree's functions: it loads a function's file from its bytecode as the
//! function is first called, through the tree's index (`opensacred_data::luau::Index`), and keeps
//! it. Luau runs sandboxed (`Lua::sandbox`): the built-ins are read-only, each file has an
//! environment of its own in which a global written fails, and no state outlives a call but the
//! files and the functions marked done. An interrupt bounds the work of a call and fails it,
//! naming the tree, the file and the function.
//!
//! The translator writes a record as a call of its type's name with its operands' table; the
//! runtime hands each to its [`Host`], which `opensacred-game` implements: an action done, a
//! condition's answer, or a record that has no meaning yet, which the runtime reports once
//! ([`Runtime::unknown`]) and answers with nothing, so that an action is skipped and a condition
//! fails. `call("name", …)` runs another function of the tree, `var("name")` reads a variable of
//! the host, and `truncated()` stands for a payload past its record's end. A call answers how its
//! function ended (`0x46BA90`, [`Outcome`]): run to its end, stopped, or marked done, never to
//! run again; one that asks to start again (`ToBegin`) runs again. The host hears each function
//! that runs begin and end ([`Host::begins`], [`Host::ends`]), as the runner's index of the
//! running function follows them (`0x94E07C`).

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use mlua::chunk::ChunkMode;
use mlua::{Function, Lua, MultiValue, Table, Value, VmState};
use opensacred_data::luau::Index;

mod record;

pub use record::record;

/// How a translated function's call ends (the walker's answer, `0x46BA90`; `opensacred-convert`'s
/// `flow::Ending`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// It ran to its end (`return true`).
    Ran,
    /// A record stopped it (`return false`).
    Stopped,
    /// It is marked done, never to run again (`return "done"`, `DeleteFunktionBlock`).
    Done,
    /// The runner refuses it by its name ([`refused`]), so that it never runs.
    Refused,
}

impl Outcome {
    /// Whether the function ran to its end.
    pub fn ran(self) -> bool {
        self == Self::Ran
    }
}

/// A record's operand as the host sees it: the translator's values (`opensacred-convert`'s
/// `luau`).
#[derive(Clone, Debug, PartialEq)]
pub enum Operand {
    Nil,
    Bool(bool),
    Number(f64),
    Text(String),
    /// A tuple, or the values of a tag that repeats.
    List(Vec<Operand>),
    /// A record's operands by their tags (`_01`, `_2A`).
    Tags(BTreeMap<String, Operand>),
}

/// What the host makes of a record's call.
#[derive(Clone, Debug, PartialEq)]
pub enum Handled {
    /// An action, done.
    Done,
    /// A condition's answer.
    Answer(bool),
    /// A record that has no meaning yet: reported once and answered with nothing.
    Unknown,
    /// The record failed, and the call with it.
    Failed(String),
    /// Run the function named, now, its records reaching the host, and hand the record to the
    /// host again with whether it ran to its end, false for a function the tree lacks: a quest's
    /// transition runs its section functions in the middle of its record (`SetUpQuest`,
    /// `TriggerQuest`, `opensacred_rules::quests`).
    Run(String),
}

/// What the scripts' calls reach: the game.
pub trait Host {
    /// The record `name` (its type's keyword in snake case) with its operands, and the answers
    /// of the functions it asked to run so far ([`Handled::Run`]), none at first.
    fn record(&mut self, name: &str, operands: &[Operand], ran: &[bool]) -> Handled;
    /// The value of the variable `name` (`var("name")`).
    fn var(&mut self, name: &str) -> i32;
    /// The function `function` begins to run, one neither done nor refused: the runner makes its
    /// index the running function's (`0x94E07C`, at `0x46BD0D`).
    fn begins(&mut self, _function: &str) {}
    /// The function that began last ends, however it ends: the runner leaves no function running
    /// (`0x94E07C` at −1, `0x46BDAD`, `0x46BF20`), until the next begins, though the function that
    /// called it runs on.
    fn ends(&mut self) {}
}

/// A call's failure, naming its tree, its file and its function.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{tree}: no function {function} in its index")]
    NoFunction { tree: String, function: String },
    #[error("{tree}: no bytecode of {file}")]
    NoFile { tree: String, file: String },
    #[error("{tree}: {file}: {function}: interrupted after {steps} of the interrupt's checks")]
    Interrupted {
        tree: String,
        file: String,
        function: String,
        steps: u64,
    },
    #[error("{tree}: {function}: started again more than {AGAIN_MOST} times")]
    Again { tree: String, function: String },
    #[error("{tree}: {file}: {function}: {message}")]
    Failed {
        tree: String,
        file: String,
        function: String,
        message: String,
    },
}

/// The bytecode of the Luau `source`, as the converter's stage B compiles a tree's sources (for
/// the tests of a host, which write their scripts of their own).
pub fn compile(source: &str) -> Result<Vec<u8>, String> {
    mlua::chunk::Compiler::new()
        .compile(source)
        .map_err(|e| e.to_string())
}

/// How many of the interrupt's checks (at the calls and the loops' back edges) a call may take
/// before it is interrupted (**estimate**: far above any function of the scripts).
pub const STEPS: u64 = 1_000_000;
/// How many times a function may start again (`ToBegin`) within a call: the original sets no
/// bound, and the scripts' loops run so up to 195 times (`grabsteindialogvars`, a start script's,
/// #726); the interrupt bounds a call's work besides.
pub const AGAIN_MOST: u32 = 10_000;

/// The names that read the built-ins rather than a record's call.
const BUILT_INS: [&str; 12] = [
    "math", "string", "table", "type", "tostring", "tonumber", "pairs", "ipairs", "select", "next",
    "error", "assert",
];

/// What loads a file's bytecode by its name.
type Loader = Box<dyn Fn(&str) -> Option<Vec<u8>>>;

/// What the runtime's Rust and Luau sides share.
struct Inner {
    tree: String,
    index: Index,
    load: Loader,
    files: RefCell<BTreeMap<String, Table>>,
    done: RefCell<BTreeSet<String>>,
    unknown: RefCell<BTreeSet<String>>,
    steps: Rc<Cell<u64>>,
    budget: Rc<Cell<u64>>,
    /// The table whose `call` the calls of a running call reach, set for each call.
    bridge: Table,
    /// The files' environments' metatable.
    environment: Table,
}

/// One tree's scripts in Luau.
pub struct Runtime {
    lua: Lua,
    inner: Rc<Inner>,
}

impl Runtime {
    /// The runtime of the tree `tree` of the index `index`, which loads a file's bytecode by its
    /// name through `load`.
    pub fn new(
        tree: &str,
        index: Index,
        load: impl Fn(&str) -> Option<Vec<u8>> + 'static,
    ) -> Result<Self, Error> {
        let failed = |e: mlua::Error| Error::Failed {
            tree: tree.to_owned(),
            file: String::new(),
            function: String::new(),
            message: e.to_string(),
        };
        let lua = Lua::new();
        lua.sandbox(true).map_err(failed)?;
        let bridge = lua.create_table().map_err(failed)?;
        let environment = lua.create_table().map_err(failed)?;
        let globals = lua.globals();
        // A name of the built-ins reads them; any other is a record's call through the bridge.
        let reads = {
            let bridge = bridge.clone();
            lua.create_function(move |lua, (_, name): (Value, String)| {
                if BUILT_INS.contains(&name.as_str()) {
                    return globals.raw_get::<Value>(name.as_str());
                }
                let bridge = bridge.clone();
                let call = lua.create_function(move |_, args: MultiValue| {
                    let call: Function = bridge.raw_get("call")?;
                    call.call::<Value>((name.clone(), args))
                })?;
                Ok(Value::Function(call))
            })
            .map_err(failed)?
        };
        let writes = lua
            .create_function(|_, (_, name): (Value, Value)| -> mlua::Result<()> {
                Err(mlua::Error::runtime(format!("a global written: {name:?}")))
            })
            .map_err(failed)?;
        environment.raw_set("__index", reads).map_err(failed)?;
        environment.raw_set("__newindex", writes).map_err(failed)?;
        let steps = Rc::new(Cell::new(0));
        let budget = Rc::new(Cell::new(STEPS));
        {
            let (steps, budget) = (Rc::clone(&steps), Rc::clone(&budget));
            lua.set_interrupt(move |_| {
                let taken = steps.get() + 1;
                steps.set(taken);
                if taken > budget.get() {
                    Err(mlua::Error::runtime("interrupted"))
                } else {
                    Ok(VmState::Continue)
                }
            });
        }
        Ok(Self {
            lua,
            inner: Rc::new(Inner {
                tree: tree.to_owned(),
                index,
                load: Box::new(load),
                files: RefCell::new(BTreeMap::new()),
                done: RefCell::new(BTreeSet::new()),
                unknown: RefCell::new(BTreeSet::new()),
                steps,
                budget,
                bridge,
                environment,
            }),
        })
    }

    /// The interrupt's checks a call may take, [`STEPS`] by default.
    pub fn set_budget(&mut self, steps: u64) {
        self.inner.budget.set(steps);
    }

    /// Runs the function `function`, its records reaching `host`: how it ended.
    pub fn call(&self, function: &str, host: &mut dyn Host) -> Result<Outcome, Error> {
        self.call_entry(function, 0, host)
    }

    /// How many functions of the name `function` the tree holds, in their order: more than one
    /// where the compiler wrote a name again (`Region2Init` and `Region2Init#2`), 0 for none.
    pub fn entries(&self, function: &str) -> usize {
        self.inner.index.functions.get(function).map_or(0, Vec::len)
    }

    /// Runs the function of the name `function` that stands `entry`-th of those of its name
    /// ([`Self::entries`]), its records reaching `host`: how it ended. A call by the name alone
    /// runs the first, as the client's search of its table finds it (`0x859690`).
    pub fn call_entry(
        &self,
        function: &str,
        entry: usize,
        host: &mut dyn Host,
    ) -> Result<Outcome, Error> {
        let inner = &self.inner;
        inner.steps.set(0);
        let host = RefCell::new(host);
        let ran = self.lua.scope(|scope| {
            let reach = scope.create_function(|lua, (name, args): (String, MultiValue)| {
                dispatch(lua, inner, &host, &name, args)
            })?;
            inner.bridge.raw_set("call", reach)?;
            let ran = run(&self.lua, inner, &host, function, entry);
            inner.bridge.raw_set("call", Value::Nil)?;
            Ok(ran)
        });
        match ran {
            Ok(ran) => ran,
            Err(e) => Err(Error::Failed {
                tree: inner.tree.clone(),
                file: String::new(),
                function: function.to_owned(),
                message: e.to_string(),
            }),
        }
    }

    /// The functions marked done, never to run again, by their keys: part of the scripts' state,
    /// which a runtime made anew takes on ([`Self::mark_done`]).
    pub fn done(&self) -> Vec<String> {
        self.inner.done.borrow().iter().cloned().collect()
    }

    /// Marks the functions of the keys `keys` done, as another runtime of the tree left them.
    pub fn mark_done(&self, keys: impl IntoIterator<Item = String>) {
        self.inner.done.borrow_mut().extend(keys);
    }

    /// The index of the tree's functions.
    pub fn index(&self) -> &Index {
        &self.inner.index
    }

    /// The records that had no meaning, each reported once.
    pub fn unknown(&self) -> Vec<String> {
        self.inner.unknown.borrow().iter().cloned().collect()
    }

    /// How many files are loaded.
    pub fn files(&self) -> usize {
        self.inner.files.borrow().len()
    }
}

/// A record's call from a running function: `call`, `var` and `truncated` the runtime's, any
/// other the host's.
fn dispatch(
    lua: &Lua,
    inner: &Inner,
    host: &RefCell<&mut dyn Host>,
    name: &str,
    args: MultiValue,
) -> mlua::Result<Value> {
    let first = || args.front().cloned().unwrap_or(Value::Nil);
    match name {
        "call" => {
            let callee = match first() {
                Value::String(callee) => callee.to_string_lossy(),
                other => {
                    return Err(mlua::Error::runtime(format!(
                        "call of no function's name: {other:?}"
                    )));
                }
            };
            let outcome = run(lua, inner, host, &callee, 0).map_err(mlua::Error::external)?;
            Ok(Value::Boolean(outcome.ran()))
        }
        "var" => {
            let var = match first() {
                Value::String(var) => var.to_string_lossy(),
                _ => String::new(),
            };
            Ok(Value::Number(f64::from(host.borrow_mut().var(&var))))
        }
        "truncated" => Ok(Value::Nil),
        _ => {
            let operands: Vec<Operand> = args.iter().map(operand).collect();
            // The host's borrow ends with each answer, so that a function it asks to run reaches
            // it in turn.
            let mut ran = Vec::new();
            loop {
                let handled = host.borrow_mut().record(name, &operands, &ran);
                match handled {
                    Handled::Run(function) => {
                        let through = inner.index.functions.contains_key(&function)
                            && run(lua, inner, host, &function, 0)
                                .map_err(mlua::Error::external)?
                                .ran();
                        ran.push(through);
                    }
                    Handled::Done => return Ok(Value::Nil),
                    Handled::Answer(answer) => return Ok(Value::Boolean(answer)),
                    Handled::Unknown => {
                        inner.unknown.borrow_mut().insert(name.to_owned());
                        return Ok(Value::Nil);
                    }
                    Handled::Failed(message) => return Err(mlua::Error::runtime(message)),
                }
            }
        }
    }
}

/// Whether the runner refuses the function `name` (`0x46BA90`, at `0x46BB72` to `0x46BC0A`;
/// #1051): a name that begins with `InitRg`, case aside, and holds `Pools` or `(DQ_Quest)`, and
/// one that begins with `DQTrgRegion` or `DQStartRegion`, case aside, the functions that would
/// start the dynamic quests, which never run in the release.
pub fn refused(name: &str) -> bool {
    let begins = |prefix: &str| {
        name.get(..prefix.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
    };
    (begins("InitRg") && (name.contains("Pools") || name.contains("(DQ_Quest)")))
        || begins("DQTrgRegion")
        || begins("DQStartRegion")
}

/// Runs the `nth` function of the name `function` of the tree: its file loaded as it is first
/// called, a function marked done not run, one that starts again run again, one of a name that
/// the runner refuses never. Each is marked done by its key in its file, the name itself for the
/// first of its name. The host hears that a function that runs begins and ends
/// ([`Host::begins`]).
fn run(
    lua: &Lua,
    inner: &Inner,
    host: &RefCell<&mut dyn Host>,
    function: &str,
    nth: usize,
) -> Result<Outcome, Error> {
    if refused(function) {
        return Ok(Outcome::Refused);
    }
    let entry = inner
        .index
        .functions
        .get(function)
        .and_then(|entries| entries.get(nth))
        .ok_or_else(|| Error::NoFunction {
            tree: inner.tree.clone(),
            function: function.to_owned(),
        })?;
    if inner.done.borrow().contains(&entry.key) {
        return Ok(Outcome::Done);
    }
    host.borrow_mut().begins(function);
    let outcome = run_entry(lua, inner, entry, function);
    host.borrow_mut().ends();
    outcome
}

/// Runs the function `function` of the index's entry `entry` ([`run`]).
fn run_entry(
    lua: &Lua,
    inner: &Inner,
    entry: &opensacred_data::luau::Entry,
    function: &str,
) -> Result<Outcome, Error> {
    let failed = |e: mlua::Error| {
        if inner.steps.get() > inner.budget.get() {
            Error::Interrupted {
                tree: inner.tree.clone(),
                file: entry.file.clone(),
                function: function.to_owned(),
                steps: inner.budget.get(),
            }
        } else {
            Error::Failed {
                tree: inner.tree.clone(),
                file: entry.file.clone(),
                function: function.to_owned(),
                message: e.to_string(),
            }
        }
    };
    let table = file(lua, inner, &entry.file).map_err(failed)?;
    let code: Function = table.get(entry.key.as_str()).map_err(failed)?;
    for _ in 0..=AGAIN_MOST {
        match code.call::<Value>(()).map_err(failed)? {
            Value::Boolean(false) => return Ok(Outcome::Stopped),
            Value::String(end) if end.to_string_lossy() == "done" => {
                inner.done.borrow_mut().insert(entry.key.clone());
                return Ok(Outcome::Done);
            }
            Value::String(end) if end.to_string_lossy() == "again" => {}
            _ => return Ok(Outcome::Ran),
        }
    }
    Err(Error::Again {
        tree: inner.tree.clone(),
        function: function.to_owned(),
    })
}

/// The table of the file `name`'s functions, loaded from its bytecode in an environment of its
/// own as first asked for, and kept.
fn file(lua: &Lua, inner: &Inner, name: &str) -> mlua::Result<Table> {
    if let Some(table) = inner.files.borrow().get(name) {
        return Ok(table.clone());
    }
    let bytes =
        (inner.load)(name).ok_or_else(|| mlua::Error::runtime(format!("no bytecode of {name}")))?;
    let environment = lua.create_table()?;
    environment.set_metatable(Some(inner.environment.clone()))?;
    let table: Table = lua
        .load(&bytes[..])
        .set_mode(ChunkMode::Binary)
        .set_name(name)
        .set_environment(environment)
        .eval()?;
    inner
        .files
        .borrow_mut()
        .insert(name.to_owned(), table.clone());
    Ok(table)
}

/// A Luau value as an operand.
fn operand(value: &Value) -> Operand {
    match value {
        Value::Boolean(b) => Operand::Bool(*b),
        Value::Number(n) => Operand::Number(*n),
        Value::Integer(n) => Operand::Number(f64::from(i32::try_from(*n).unwrap_or(i32::MAX))),
        Value::String(text) => Operand::Text(text.to_string_lossy()),
        Value::Table(table) if table.raw_len() > 0 => Operand::List(
            table
                .sequence_values::<Value>()
                .filter_map(Result::ok)
                .map(|value| operand(&value))
                .collect(),
        ),
        Value::Table(table) => Operand::Tags(
            table
                .pairs::<String, Value>()
                .filter_map(Result::ok)
                .map(|(key, value)| (key, operand(&value)))
                .collect(),
        ),
        _ => Operand::Nil,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use opensacred_data::luau::Entry;

    use super::*;

    /// A host that records what it is asked, answers a condition `ok` true, counts a variable
    /// `n` up, and knows no `mystery`; `set_up` runs `OnSetUp`, then `Trigger`, then, where the
    /// trigger ran through, `OnEnter`, as a quest's transitions do, and keeps their answers.
    #[derive(Default)]
    struct Recorder {
        records: Vec<(String, Vec<Operand>)>,
        n: i32,
        answers: Vec<Vec<bool>>,
        /// The records' names, and the functions' beginnings (`+Name`) and ends (`-`), in order.
        events: Vec<String>,
    }

    impl Host for Recorder {
        fn record(&mut self, name: &str, operands: &[Operand], ran: &[bool]) -> Handled {
            if name == "set_up" {
                return match ran {
                    [] => Handled::Run("OnSetUp".to_owned()),
                    [_] => Handled::Run("Trigger".to_owned()),
                    [_, true] => Handled::Run("OnEnter".to_owned()),
                    _ => {
                        self.answers.push(ran.to_vec());
                        Handled::Done
                    }
                };
            }
            self.records.push((name.to_owned(), operands.to_vec()));
            self.events.push(name.to_owned());
            match name {
                "ok" => Handled::Answer(true),
                "broken" => Handled::Failed("it broke".to_owned()),
                "mystery" => Handled::Unknown,
                _ => Handled::Done,
            }
        }

        fn var(&mut self, name: &str) -> i32 {
            if name == "n" {
                self.n += 1;
            }
            self.n
        }

        fn begins(&mut self, function: &str) {
            self.events.push(format!("+{function}"));
        }

        fn ends(&mut self) {
            self.events.push("-".to_owned());
        }
    }

    /// A runtime of the files `sources` (name, Luau), each function indexed by its key (`t.Name`
    /// or `t["Name#2"]`, filed under its name before the `#`), and how many times each file was
    /// loaded.
    fn runtime(sources: &[(&str, &str)]) -> (Runtime, Rc<RefCell<BTreeMap<String, u32>>>) {
        let mut index = Index::default();
        let mut bytecode = BTreeMap::new();
        for &(file, source) in sources {
            bytecode.insert(
                file.to_owned(),
                mlua::chunk::Compiler::new()
                    .compile(source)
                    .expect("the test's source compiles"),
            );
            for key in source.lines().filter_map(|line| {
                let line = line.trim();
                match line.strip_prefix("t[\"") {
                    Some(rest) => rest.split_once('"').map(|(key, _)| key),
                    None => line
                        .strip_prefix("t.")
                        .and_then(|rest| rest.split_once(" ="))
                        .map(|(key, _)| key.trim()),
                }
            }) {
                let name = key.split_once('#').map_or(key, |(name, _)| name);
                index
                    .functions
                    .entry(name.to_owned())
                    .or_default()
                    .push(Entry {
                        file: file.to_owned(),
                        key: key.to_owned(),
                    });
            }
        }
        let loads = Rc::new(RefCell::new(BTreeMap::new()));
        let counted = Rc::clone(&loads);
        let runtime = Runtime::new("TEST", index, move |file| {
            *counted.borrow_mut().entry(file.to_owned()).or_insert(0) += 1;
            bytecode.get(file).cloned()
        })
        .expect("the runtime starts");
        (runtime, loads)
    }

    #[test]
    fn a_call_runs_its_records_through_the_host() {
        let (runtime, _) = runtime(&[(
            "start.luau",
            "local t = {}\n\
             t.Start = function()\n\
               create_npc({ _01 = \"novice\", _2A = { 3237, 2514, 0 } })\n\
               if ok({}) then go({ n = var(\"gold\") }) end\n\
               return true\n\
             end\n\
             return t",
        )]);
        let mut host = Recorder::default();
        assert_eq!(runtime.call("Start", &mut host).ok(), Some(Outcome::Ran));
        let names: Vec<&str> = host.records.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, ["create_npc", "ok", "go"]);
        let Operand::Tags(tags) = &host.records[0].1[0] else {
            panic!("the operands are a table of tags");
        };
        assert_eq!(tags.get("_01"), Some(&Operand::Text("novice".to_owned())));
        assert_eq!(
            tags.get("_2A"),
            Some(&Operand::List(vec![
                Operand::Number(3237.0),
                Operand::Number(2514.0),
                Operand::Number(0.0)
            ]))
        );
    }

    #[test]
    fn the_host_hears_each_function_that_runs_begin_and_end() {
        let (runtime, _) = runtime(&[(
            "start.luau",
            "local t = {}\n\
             t.Outer = function()\n  before({})\n  call(\"Inner\")\n  after({})\n  return true\nend\n\
             t.Inner = function()\n  inside({})\n  return \"done\"\nend\n\
             return t",
        )]);
        let mut host = Recorder::default();
        assert_eq!(runtime.call("Outer", &mut host).ok(), Some(Outcome::Ran));
        assert_eq!(
            host.events,
            ["+Outer", "before", "+Inner", "inside", "-", "after", "-"]
        );
        // A function marked done neither begins nor ends.
        host.events.clear();
        assert_eq!(runtime.call("Inner", &mut host).ok(), Some(Outcome::Done));
        assert!(host.events.is_empty());
    }

    #[test]
    fn a_record_runs_the_functions_its_host_asks_for_in_its_middle() {
        let (quest, _) = runtime(&[
            (
                "start.luau",
                "local t = {}\nt.Start = function()\n  before({})\n  set_up({})\n  after({})\n  return true\nend\nreturn t",
            ),
            (
                "quest.luau",
                "local t = {}\n\
                 t.OnSetUp = function() made({}) return true end\n\
                 t.Trigger = function() if ok({}) then return true end return false end\n\
                 t.OnEnter = function() entered({}) return true end\n\
                 return t",
            ),
        ]);
        let mut host = Recorder::default();
        assert_eq!(quest.call("Start", &mut host).ok(), Some(Outcome::Ran));
        // The sections' records in the middle of the transition's, in the order run.
        let names: Vec<&str> = host.records.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, ["before", "made", "ok", "entered", "after"]);
        assert_eq!(host.answers, [vec![true, true, true]]);
        // A section the tree lacks answers false, and the transition goes on without it.
        let (lacking, _) = runtime(&[(
            "start.luau",
            "local t = {}\nt.Start = function()\n  set_up({})\n  return true\nend\nreturn t",
        )]);
        let mut host = Recorder::default();
        assert_eq!(lacking.call("Start", &mut host).ok(), Some(Outcome::Ran));
        assert_eq!(host.answers, [vec![false, false]]);
    }

    #[test]
    fn a_call_of_another_file_loads_it_once() {
        let (runtime, loads) = runtime(&[
            (
                "a.luau",
                "local t = {}\nt.First = function()\n  call(\"Second\", {})\n  return true\nend\nreturn t",
            ),
            (
                "b.luau",
                "local t = {}\nt.Second = function()\n  touch({})\n  return true\nend\nreturn t",
            ),
        ]);
        let mut host = Recorder::default();
        for _ in 0..3 {
            assert_eq!(runtime.call("First", &mut host).ok(), Some(Outcome::Ran));
        }
        assert_eq!(host.records.len(), 3);
        assert_eq!(loads.borrow().get("b.luau"), Some(&1));
        assert_eq!(runtime.files(), 2);
    }

    /// A runtime of a region's `Init` written twice, the first marking itself done.
    fn runtime_of_region() -> (Runtime, Rc<RefCell<BTreeMap<String, u32>>>) {
        runtime(&[(
            "regions/2.luau",
            "local t = {}\n\
             t[\"Region2Init\"] = function() first({}) return \"done\" end\n\
             t[\"Region2Init#2\"] = function() second({}) return true end\n\
             return t",
        )])
    }

    #[test]
    fn a_name_written_again_calls_each_of_its_functions_by_its_place() {
        let (runtime, _) = runtime_of_region();
        let mut host = Recorder::default();
        assert_eq!(runtime.entries("Region2Init"), 2);
        assert_eq!(runtime.entries("Region3Init"), 0);
        assert_eq!(
            runtime.call_entry("Region2Init", 1, &mut host).ok(),
            Some(Outcome::Ran)
        );
        assert_eq!(
            runtime.call("Region2Init", &mut host).ok(),
            Some(Outcome::Done)
        );
        // The first marked done, the second still runs.
        assert_eq!(
            runtime.call("Region2Init", &mut host).ok(),
            Some(Outcome::Done)
        );
        assert_eq!(
            runtime.call_entry("Region2Init", 1, &mut host).ok(),
            Some(Outcome::Ran)
        );
        let names: Vec<&str> = host.records.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, ["second", "first", "second"]);
        assert!(runtime.call_entry("Region2Init", 2, &mut host).is_err());
        // A runtime made anew takes on the functions marked done.
        assert_eq!(runtime.done(), ["Region2Init"]);
        let (again, _) = runtime_of_region();
        again.mark_done(runtime.done());
        assert_eq!(
            again.call("Region2Init", &mut host).ok(),
            Some(Outcome::Done)
        );
    }

    #[test]
    fn a_global_written_fails_and_names_its_function() {
        let (runtime, _) = runtime(&[(
            "f.luau",
            "local t = {}\nt.Writes = function()\n  x = 1\n  return true\nend\nreturn t",
        )]);
        let error = runtime
            .call("Writes", &mut Recorder::default())
            .expect_err("a global written fails");
        let text = error.to_string();
        assert!(text.contains("TEST") && text.contains("f.luau") && text.contains("Writes"));
        assert!(text.contains("a global written"), "{text}");
    }

    #[test]
    fn a_runaway_loop_is_interrupted() {
        let (mut runtime, _) = runtime(&[(
            "loop.luau",
            "local t = {}\nt.Forever = function()\n  while true do end\n  return true\nend\nreturn t",
        )]);
        runtime.set_budget(10_000);
        let error = runtime
            .call("Forever", &mut Recorder::default())
            .expect_err("the loop is interrupted");
        assert!(matches!(error, Error::Interrupted { .. }), "{error}");
        assert!(error.to_string().contains("Forever"));
    }

    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn the_seraphims_start_and_her_starting_sector_load_and_run() {
        // The converter's Luau of the Seraphim's tree (`opensacred-convert netscript`).
        let tree = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets-src/netscript/type_npc_seraphim");
        let index: Index = ron::from_str(
            &std::fs::read_to_string(tree.join(opensacred_data::luau::INDEX))
                .expect("the tree's index is converted"),
        )
        .expect("the index reads");
        let compiling = Rc::new(Cell::new(std::time::Duration::ZERO));
        let spent = Rc::clone(&compiling);
        let runtime = Runtime::new("TYPE_NPC_SERAPHIM", index, move |file| {
            let source = std::fs::read_to_string(tree.join(file)).ok()?;
            let started = std::time::Instant::now();
            let bytes = mlua::chunk::Compiler::new().compile(source).ok();
            spent.set(spent.get() + started.elapsed());
            bytes
        })
        .expect("the runtime starts");
        /// A host that does every action and answers every condition false.
        struct Quiet(u32);
        impl Host for Quiet {
            fn record(&mut self, _: &str, _: &[Operand], _: &[bool]) -> Handled {
                self.0 += 1;
                Handled::Answer(false)
            }
            fn var(&mut self, _: &str) -> i32 {
                0
            }
        }
        for function in ["StartCode", "Sector5039Init", "Sector5039Enter"] {
            let mut host = Quiet(0);
            let started = std::time::Instant::now();
            let ended = runtime.call(function, &mut host);
            let took = started.elapsed();
            println!(
                "{function}: {ended:?}, {} records in {:.1} ms with {:.1} ms of compiling",
                host.0,
                took.as_secs_f64() * 1000.0,
                compiling.take().as_secs_f64() * 1000.0
            );
            assert!(ended.is_ok(), "{function}: {ended:?}");
        }
    }

    #[test]
    fn a_call_answers_how_its_function_ended() {
        let (runtime, _) = runtime(&[(
            "ends.luau",
            "local t = {}\n\
             t.Stops = function() return false end\n\
             t.Once = function() touch({}) return \"done\" end\n\
             t.Thrice = function() if var(\"n\") < 3 then return \"again\" end return true end\n\
             t.Fails = function() broken({}) return true end\n\
             t.Asks = function() mystery({}) if mystery({}) then return true end return false end\n\
             return t",
        )]);
        let mut host = Recorder::default();
        assert_eq!(
            runtime.call("Stops", &mut host).ok(),
            Some(Outcome::Stopped)
        );
        assert_eq!(runtime.call("Once", &mut host).ok(), Some(Outcome::Done));
        // Marked done, it runs no more.
        assert_eq!(runtime.call("Once", &mut host).ok(), Some(Outcome::Done));
        assert_eq!(host.records.len(), 1);
        // It starts again until its variable reaches 3.
        assert_eq!(runtime.call("Thrice", &mut host).ok(), Some(Outcome::Ran));
        assert_eq!(host.n, 3);
        assert!(runtime.call("Fails", &mut host).is_err());
        // A record without a meaning: an action skipped, a condition failing, reported once.
        assert_eq!(runtime.call("Asks", &mut host).ok(), Some(Outcome::Stopped));
        assert_eq!(runtime.unknown(), ["mystery"]);
        assert!(runtime.call("Missing", &mut host).is_err());
    }

    #[test]
    fn the_runner_refuses_the_dynamic_quests_functions() {
        let (runtime, _) = runtime(&[(
            "dq.luau",
            "local t = {}\n\
             t.DQStartRegion1 = function() touch({}) return true end\n\
             t.InitRg01Pools = function() touch({}) return true end\n\
             t.Region1Enter = function() if call(\"dqtrgregion1\") then touch({}) end return true end\n\
             return t",
        )]);
        let mut host = Recorder::default();
        assert_eq!(
            runtime.call("DQStartRegion1", &mut host).ok(),
            Some(Outcome::Refused)
        );
        assert_eq!(
            runtime.call("InitRg01Pools", &mut host).ok(),
            Some(Outcome::Refused)
        );
        // A call of one, missing or not, answers that it did not run.
        assert_eq!(
            runtime.call("Region1Enter", &mut host).ok(),
            Some(Outcome::Ran)
        );
        assert!(host.records.is_empty());
        assert!(refused("InitRg+VAR(DQ_Quest)+ZielEskortiereNPC"));
        assert!(refused("initrg5Pools"));
        // `Pools` and `(DQ_Quest)` are found as they are written.
        assert!(!refused("InitRg5pools"));
        assert!(!refused("InitRg12"));
        assert!(!refused("Region1Init"));
    }
}
