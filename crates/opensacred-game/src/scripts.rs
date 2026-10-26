//! The scripts run in Luau (#726, part of #720; ADR-13): the hero's tree's start script, its
//! sectors' `Init` and `Enter` functions, each sector's `Init` before its `Enter`, and its regions'
//! `Init` functions, each before the first `Init` of a sector of its region (#585, `0x492880`), as
//! the converter translates them (`netscript/<tree>/`), run by `opensacred_scripts`, their records
//! handed back as the records they were (`opensacred_scripts::record`) to the readers of `npcs`,
//! which place what they make as before. The game keeps the scripts' state
//! (`opensacred_rules::quests::Scripts`): the variables that `SetVar`, `SetVarBit`, `UnsetVarBit`,
//! `IncVar` and `RndVar` set, which an `IF`'s tests read (`0x48`, `0x49`, `0x4A`, `0x6D`, `0x6E`;
//! docs/re/netscript.md, "Variables and groups"), and the quests' flags, whose transitions
//! `SetUpQuest` and `TriggerQuest` run their sections in their middle, as the client's do
//! (`0x48F7F0`, `0x48F030`). A condition of anything else fails, an action the readers do not take
//! passes.
//!
//! The scripts run on through the play (#1002): the load leaves their state ([`Started`]), which a
//! runtime of the tree on the main thread takes on as the game starts ([`Live`], mlua's Luau being
//! no `Send`), with the places of `DefPos` and the base triggers of `SetBaseTrigger` and
//! `DelBaseTrigger`. As the hero's cell changes, the base trigger of its cell and level runs its
//! functions, `OMO<quest><name>`, each of a quest only while that quest is set up (`0x5941A0`,
//! `0x46BA90`), and their `Teleport`s of the hero put it where they name (`0x491D40`); their
//! `CreateNPC`s and `CreateObj`s go to `npcs` ([`WorldRecords`], #1014). The timers of `SetTimer`
//! and `DelTimer` count the game's minutes (#933, docs/decisions/2026-10-25-script-timers.md): at
//! each minute of the clock but the one that ends a day the timers due run their functions in turn,
//! but those of a finished quest, as the client's calendar runs them (`0x416F00`). `SetDrop` keeps
//! the scripts' drops by the creature type and the item (#930,
//! docs/decisions/2026-10-25-script-drops.md), which the deaths' drops take ([`ScriptDrops`],
//! `drops`). `SetObjState` bars, unbars, opens and closes the trigger of the object it names
//! ([`ObjStates`], which `npcs` carries out), the state kept by the trigger while the object is
//! gone (#952, docs/decisions/2026-10-25-stale-object-names.md). `SetHP` sets a creature's health
//! to a share of its most ([`Healths`], #1001).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use bevy::ecs::system::SystemParam;
use bevy::prelude::{
    Added, App, Commands, Entity, IntoScheduleConfigs, MessageReader, NonSendMut, OnEnter, OnExit,
    Plugin, Query, Res, ResMut, Resource, Transform, Update, With, Without, World,
};
use opensacred_data::luau::{BYTECODE, INDEX, Index};
use opensacred_data::netscript::{Function, Record, Tree};
use opensacred_rules::quests::{self, Counted, Transition};
use opensacred_rules::spawning::Rng;
use opensacred_scripts::{Handled, Host, Operand, Outcome, Runtime};

use crate::ai::Mind;
use crate::daynight::Clock;
use crate::fight::Health;
use crate::frame::{Playing, Step, Unpaused};
use crate::game::AppState;
use crate::hero::{Arrival, Body};
use crate::npcs::{Dies, OnDeath, ScriptName};
use crate::terrain::Heights;
use crate::{files, iso, npcs, paths};

/// The minutes of a day of the game's clock.
const DAY: u16 = 1_440;

/// The start script's function in a tree (`opensacred-convert`'s `luau::START`).
const START: &str = "StartCode";

/// The seed of the scripts' `rand()` in a new game, which `RndVar` draws from.
const SEED: usize = 0x5C;

/// What the scripts ran as a new game loads ([`run`]): their records, which `npcs` places, and
/// the state they left, which the play takes on.
pub struct Ran {
    pub tree: Tree,
    pub started: Started,
}

/// The scripts as the load leaves them, which [`Live`] takes on as the play starts: the tree, its
/// index, the scripts' state, the functions marked done, the scripts' `rand()` and the creatures'
/// types.
#[derive(Resource)]
pub struct Started {
    assets: PathBuf,
    tree: String,
    index: Index,
    state: quests::Scripts,
    done: Vec<String>,
    rng: Rng,
    creatures: HashSet<u32>,
}

/// The scripts' drops as the scripts last left them, which a death's drops take (`drops`).
#[derive(Resource, Default)]
pub struct ScriptDrops(pub quests::Drops);

/// A `SetObjState` (2, `0x48AE90`) of the object named `name` (its ASCII lower-cased, as `npcs`
/// keeps the objects' names): its trigger's bar (`0x8000`; `0x06` sets it, `0x07` clears it) and
/// open bit (bit 0; `0x0F` sets it, `0x10` clears it), each where the record gives it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjState {
    pub name: String,
    pub barred: Option<bool>,
    pub open: Option<bool>,
}

/// The `SetObjState`s that the scripts ran, in order, which `npcs` carries out on the objects'
/// triggers.
#[derive(Resource, Default)]
pub struct ObjStates(pub Vec<ObjState>);

/// A `SetHP` (52, `0x48E280`): the creature named `name` (its ASCII lower-cased), the hero for
/// none or `hero`, takes `share` per cent (0 to 100) of its most health, truncated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetHealth {
    pub name: Option<String>,
    pub share: u32,
}

/// The `SetHP`s that the scripts ran, in order, which `npcs` carries out on the creatures.
#[derive(Resource, Default)]
pub struct Healths(pub Vec<SetHealth>);

/// The hero's home, where it rises after its death (`+0x2BD` to `+0x2C5`, #974): the cell and the
/// floor that the play's `SetNPCState`s of the tag `0x4D` naming the hero and its `StartPosition`s
/// give, none before, the game's start then.
#[derive(Resource, Default)]
pub struct HeroHome(pub Option<Teleport>);

/// The records of the world that the scripts ran during the play, in order, which `npcs` places
/// (`CreateNPC`, `CreateObj` and the `DefPos`s and `SetNPCState`s with them, #1014).
#[derive(Resource, Default)]
pub struct WorldRecords(pub Vec<Record>);

/// What a dialogue's function gives its window (#1028, docs/re/netscript.md, "Dialogues"): its
/// texts' keys in order, its buttons' labels and functions, the sounds it plays, and whether it
/// takes its speaker's dialogue away (`DelDlg`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Window {
    pub texts: Vec<String>,
    pub buttons: Vec<(String, String)>,
    pub sounds: Vec<String>,
    pub untalks: bool,
}

/// How many buttons a dialogue keeps (`+0x9A00`); the window shows the first [`SHOWN`].
const BUTTONS: usize = 5;
pub const SHOWN: usize = 4;

/// What a run of the scripts did to the world beyond their state: the hero's teleports and the
/// objects' states, in order.
#[derive(Debug, Default, PartialEq)]
pub struct Effects {
    pub teleports: Vec<Teleport>,
    pub states: Vec<ObjState>,
    pub healths: Vec<SetHealth>,
    pub records: Vec<Record>,
    /// The hero's home, as the last record that sets it gives it.
    pub home: Option<Teleport>,
    /// What a dialogue's records gave its window.
    pub window: Window,
}

impl Effects {
    fn extend(&mut self, other: Effects) {
        self.teleports.extend(other.teleports);
        self.states.extend(other.states);
        self.healths.extend(other.healths);
        self.records.extend(other.records);
        self.home = other.home.or(self.home);
        self.window.texts.extend(other.window.texts);
        self.window.buttons.extend(other.window.buttons);
        self.window.sounds.extend(other.window.sounds);
        self.window.untalks |= other.window.untalks;
    }
}

/// What the scripts ran: the tree's start script's records, with those of the quests' sections
/// that its transitions ran in their middle, and each sector function's records and each region
/// function's (`Region<n>Init`) before the first sector of its region, as a tree, and the state
/// they left; `area` the area of a sector of the world (`world.ron`), which makes its region, and
/// `creatures` the types of the creatures, the `items.pak` slots of the categories 2, 3 and 32
/// (`0x426610`), by which `SetDrop` tells its creature from its item.
pub fn run(
    assets: &Path,
    tree: &str,
    area: impl Fn((u32, u32)) -> Option<u8>,
    creatures: HashSet<u32>,
) -> Result<Ran> {
    let started = Instant::now();
    let dir = assets.join("netscript").join(tree);
    let index_path = dir.join(INDEX);
    let index: Index = ron::from_str(
        &files::read_to_string(&index_path)
            .with_context(|| format!("reading {}", index_path.display()))?,
    )
    .with_context(|| format!("reading {}", index_path.display()))?;
    // Each sector's `Init`, then its `Enter`, as a sector that first comes near runs them
    // (docs/re/spawning.md).
    let mut sectors: Vec<((u32, u32), bool, String)> = index
        .functions
        .keys()
        .filter_map(|name| {
            let sector = npcs::sector_function(name)?;
            Some((sector, name.ends_with("Enter"), name.clone()))
        })
        .collect();
    sectors.sort();
    let runtime = runtime(assets, tree, index)?;
    let mut state = quests::Scripts::default();
    let mut rng = Rng::of(0, 0, SEED);
    let start = records(&runtime, (tree, &creatures), START, 0, &mut state, &mut rng);
    let mut functions = Vec::new();
    let mut regions = 0;
    for (sector, enter, name) in sectors {
        let region = quests::region(area(sector).unwrap_or(0));
        let init = format!("Region{region}Init");
        let entries = runtime.entries(&init);
        if !enter && entries > 0 && state.regions.init(region) {
            bevy::log::debug!("scripts: initRegion({region}) before {name}");
            let mut made = Vec::new();
            for entry in region_entries(entries) {
                made.extend(records(
                    &runtime,
                    (tree, &creatures),
                    &init,
                    entry,
                    &mut state,
                    &mut rng,
                ));
            }
            functions.push(Function {
                name: init,
                records: made,
                quest: -1,
                region: 0,
            });
            regions += 1;
        }
        functions.push(Function {
            records: records(&runtime, (tree, &creatures), &name, 0, &mut state, &mut rng),
            name,
            quest: -1,
            region: 0,
        });
    }
    bevy::log::info!(
        "scripts: {tree}'s start, {regions} region and {} sector functions ran in {:.2} s, {} \
         files loaded; the records without a meaning yet: {}",
        functions.len() - regions,
        started.elapsed().as_secs_f64(),
        runtime.files(),
        runtime.unknown().join(" "),
    );
    Ok(Ran {
        tree: Tree {
            start,
            quest: Vec::new(),
            functions,
            quests: Vec::new(),
            regions: Vec::new(),
        },
        started: Started {
            assets: assets.to_owned(),
            tree: tree.to_owned(),
            index: runtime.index().clone(),
            state,
            done: runtime.done(),
            rng,
            creatures,
        },
    })
}

/// The runtime of the tree `tree` of the index `index`, which loads its files' bytecode from
/// `assets` (`netscript/<tree>/`).
fn runtime(assets: &Path, tree: &str, index: Index) -> Result<Runtime> {
    let dir = assets.join("netscript").join(tree);
    Runtime::new(tree, index, move |file| {
        let stem = file.strip_suffix(".luau").unwrap_or(file);
        files::read(dir.join(format!("{stem}.{BYTECODE}"))).ok()
    })
    .with_context(|| format!("starting the scripts of {tree}"))
}

/// The scripts through the play: the runtime of the hero's tree, the scripts' state, their
/// `rand()`, the base triggers' functions by their triggers' names, and the hero's cell as last
/// seen.
pub struct Live {
    runtime: Runtime,
    tree: String,
    state: quests::Scripts,
    rng: Rng,
    /// Each base trigger's functions, by the trigger's name lower-cased: the functions
    /// `OMO<quest><name>` of the tree, which the client's cache of the triggers' functions names
    /// (the start script's records of type 39, `0x4918B0`; in every tree each of their names has
    /// the one function of that name, two in one where it is written twice).
    triggered: HashMap<String, Vec<String>>,
    at: Option<paths::Cell>,
    /// The minute of the clock's day last seen.
    minute: Option<u16>,
    creatures: HashSet<u32>,
    /// The scripts' groups that hold a living creature, wherever it stands, by which
    /// `GroupIsDead` answers (`0x48C860`, [`living_groups`]), taken before each run.
    living: HashSet<u32>,
}

/// A `Teleport` of the hero (`0x491D40`): the global cell and the layer it is put on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Teleport {
    pub cell: (i32, i32),
    pub layer: u8,
}

impl Live {
    /// The scripts of `runtime` taking on the state `started` left.
    fn new(runtime: Runtime, started: Started) -> Self {
        runtime.mark_done(started.done);
        let mut triggered: HashMap<String, Vec<String>> = HashMap::new();
        for name in runtime.index().functions.keys() {
            if let Some((_, trigger)) = quests::trigger_function(name) {
                triggered
                    .entry(trigger.to_lowercase())
                    .or_default()
                    .push(name.clone());
            }
        }
        Self {
            runtime,
            tree: started.tree,
            state: started.state,
            rng: started.rng,
            triggered,
            at: None,
            minute: None,
            creatures: started.creatures,
            living: HashSet::new(),
        }
    }

    /// The function of the name `name` as the client's search of its table finds it by name
    /// (`0x46BF40`): of a length from 1 to 63, the name itself, else the first of the same name
    /// case aside (`0x859690`).
    fn named(&self, name: &str) -> Option<String> {
        if name.is_empty() || name.len() >= 64 {
            return None;
        }
        if self.runtime.entries(name) > 0 {
            return Some(name.to_owned());
        }
        self.runtime
            .index()
            .functions
            .keys()
            .find(|key| key.eq_ignore_ascii_case(name))
            .cloned()
    }

    /// Runs the function `function` found by its name case aside (`0x46BF40`, `0x460590`), as
    /// the runner runs one (`0x46BA90`), a quest's only while the quest is set up: a creature's
    /// death's function, a count's: what it did to the world, nothing where the tree has no such
    /// function.
    pub fn run_named(&mut self, function: &str) -> Effects {
        match self.named(function) {
            Some(name) => self.run(&name, 0),
            None => {
                bevy::log::warn!("scripts: no function {function} in {}", self.tree);
                Effects::default()
            }
        }
    }

    /// Runs the dialogue `dialogue`'s function as a creature's talk runs it (`0x46B480`,
    /// `0x46BA90`; #1028): `Dialog:<name>`, else the name itself, found case aside; what it did,
    /// its window's contents among it; none where the tree has no such function.
    pub fn talk(&mut self, dialogue: &str) -> Option<Effects> {
        let function = self
            .named(&format!("Dialog:{dialogue}"))
            .or_else(|| self.named(dialogue))?;
        Some(self.run(&function, 0))
    }

    /// Counts a kill of a creature of the type `creature` (`SetOnKill`, `0x4AA630`; #1021): the
    /// function of its count once done, what it did to the world.
    pub fn kill(&mut self, creature: u32) -> Effects {
        match self.state.kills.count(creature) {
            Some(Counted::Done(function)) => {
                bevy::log::info!("scripts: the kills of {creature} are done and run {function}");
                self.run_named(&function)
            }
            Some(Counted::Goes { counted, asked }) => {
                bevy::log::info!("scripts: {counted} of {asked} kills of {creature}");
                Effects::default()
            }
            None => Effects::default(),
        }
    }

    /// Counts an item of the type `item` that the hero picks up (`SetOnCollect`, `0x4AAAE0`;
    /// #1021), the counts of a creature's type first taking the item of its `SetDrop`: what the
    /// count's function did to the world where one counts it, which keeps the item among the
    /// quest's, none where none counts it.
    pub fn collect(&mut self, item: u32) -> Option<Effects> {
        let quests::Scripts {
            collecting, drops, ..
        } = &mut self.state;
        let creatures = &self.creatures;
        collecting.retype(|kind| {
            creatures
                .contains(&kind)
                .then(|| drops.of(kind).next().map(|(item, _)| item))
                .flatten()
        });
        match self.state.collecting.count(item)? {
            Counted::Done(function) => {
                bevy::log::info!("scripts: the items {item} are collected and run {function}");
                Some(self.run_named(&function))
            }
            Counted::Goes { counted, asked } => {
                bevy::log::info!("scripts: {counted} of {asked} items {item} collected");
                Some(Effects::default())
            }
        }
    }

    /// The quest of the function `function`, the first of its name: the quest whose file of the
    /// tree holds it (`quests/<n>.luau`, the function's quest of `Vectoren.bin`), none for
    /// another.
    fn quest_of(&self, function: &str) -> Option<u32> {
        let entry = self.runtime.index().functions.get(function)?.first()?;
        entry
            .file
            .strip_prefix("quests/")?
            .strip_suffix(".luau")?
            .parse()
            .ok()
    }

    /// Runs the `entry`-th function of the name `function` as the client's runner does
    /// (`0x46BA90`), a function of a quest only while the quest is set up: what it did to the
    /// world.
    fn run(&mut self, function: &str, entry: usize) -> Effects {
        self.call(function, entry).0
    }

    /// Runs the `entry`-th function of the name `function` as [`Self::run`] does: what it did to
    /// the world and how it ended, none where its quest is not set up.
    fn call(&mut self, function: &str, entry: usize) -> (Effects, Option<Outcome>) {
        if let Some(quest) = self.quest_of(function)
            && self.state.quests.flags(quest) & quests::SET_UP == 0
        {
            bevy::log::debug!("scripts: {function} waits for its quest {quest}");
            return (Effects::default(), None);
        }
        let mut recorder = Recorder::new(
            (&self.tree, &self.creatures),
            &mut self.state,
            &mut self.rng,
        );
        recorder.living = Some(&self.living);
        let outcome = match self.runtime.call_entry(function, entry, &mut recorder) {
            Ok(outcome) => Some(outcome),
            Err(e) => {
                bevy::log::warn!("scripts: {e}");
                Some(Outcome::Stopped)
            }
        };
        let effects = Effects {
            teleports: recorder.teleports,
            states: recorder.states,
            healths: recorder.healths,
            records: recorder.records,
            home: recorder.home,
            window: recorder.window,
        };
        (effects, outcome)
    }

    /// A quest's transition, as its record runs it (`SetUpQuest`, `TriggerQuest`;
    /// `quests::Quests::step`), its sections run in turn: what they did to the world.
    pub fn transition(&mut self, transition: Transition, quest: u32) -> Effects {
        let mut ran = Vec::new();
        let mut effects = Effects::default();
        while let Some(section) = self.state.quests.step(transition, quest, &ran) {
            let function = section.function(quest);
            let through = self.runtime.entries(&function) > 0 && {
                let (done, outcome) = self.call(&function, 0);
                effects.extend(done);
                outcome.is_some_and(Outcome::ran)
            };
            ran.push(through);
        }
        effects
    }

    /// The game's clock at the minute `now` of its day (`0x416F00`): each minute completed since
    /// it was last seen counts on the timers, and at each but the one that ends a day the timers
    /// due run in turn, each the first due of the list as it stands, its function run unless the
    /// function's quest is finished (`0x41715F`), and every timer of its id taken out after;
    /// what they did to the world.
    pub fn clock(&mut self, now: u16) -> Effects {
        let Some(last) = self.minute.replace(now) else {
            return Effects::default();
        };
        let passed = (now + DAY - last % DAY) % DAY;
        let mut effects = Effects::default();
        for minute in 1..=passed {
            self.state.timers.tick();
            if (last + minute) % DAY == 0 {
                continue;
            }
            while let Some(timer) = self.state.timers.due().cloned() {
                let finished = self
                    .quest_of(&timer.function)
                    .is_some_and(|quest| self.state.quests.flags(quest) & quests::FINISHED != 0);
                if finished {
                    bevy::log::debug!(
                        "scripts: the timer {} of {}, its quest finished, runs nothing",
                        timer.id,
                        timer.function
                    );
                } else {
                    bevy::log::info!(
                        "scripts: the timer {} runs {} after {} minutes",
                        timer.id,
                        timer.function,
                        timer.time
                    );
                    effects.extend(self.run(&timer.function, 0));
                }
                self.state.timers.ran(timer.id);
            }
        }
        effects
    }

    /// The hero steps onto the global cell `cell` on the layer `layer` (`0x5941A0`, at
    /// `0x597E2C`): the base trigger that holds it runs its functions in order, each of a quest
    /// only while the quest is set up and each of a name written again as often (`0x46BA90`);
    /// the teleports of the hero they made, in order.
    pub fn step_on(&mut self, cell: (i32, i32), layer: u8) -> Effects {
        let Some(trigger) = self.state.triggers.at(cell, layer).map(str::to_owned) else {
            return Effects::default();
        };
        let functions = self
            .triggered
            .get(&trigger.to_lowercase())
            .cloned()
            .unwrap_or_default();
        let mut effects = Effects::default();
        for function in functions {
            bevy::log::info!(
                "scripts: the hero steps on {trigger} at {},{}: {function}",
                cell.0,
                cell.1
            );
            for entry in 0..self.runtime.entries(&function) {
                effects.extend(self.run(&function, entry));
            }
        }
        effects
    }
}

/// Which of the functions of a region's `Init`, `entries` of one name, run, in their order: of a
/// name written again, the last and then the first, the two that the client's cache of the
/// sectors' and the regions' names keeps (`0x46A5C3`: the first index in its low word, each later
/// one in its high), which the region's runner runs high word first (`0x492BFB`).
fn region_entries(entries: usize) -> Vec<usize> {
    match entries {
        0 => Vec::new(),
        1 => vec![0],
        _ => vec![entries - 1, 0],
    }
}

/// The records of the function `function` of the tree `tree`, the `entry`-th of its name, as it
/// runs, the scripts' state `state` with it; none where it fails, which the log tells.
fn records(
    runtime: &Runtime,
    of: (&str, &HashSet<u32>),
    function: &str,
    entry: usize,
    state: &mut quests::Scripts,
    rng: &mut Rng,
) -> Vec<Record> {
    let mut recorder = Recorder::new(of, state, rng);
    if let Err(e) = runtime.call_entry(function, entry, &mut recorder) {
        bevy::log::warn!("scripts: {e}");
    }
    recorder.records
}

/// The host of a run: the hero's tree, the creatures' types, the scripts' state, their `rand()`,
/// the records in the order run, the hero's teleports and the objects' states.
struct Recorder<'a> {
    tree: &'a str,
    creatures: &'a HashSet<u32>,
    state: &'a mut quests::Scripts,
    rng: &'a mut Rng,
    records: Vec<Record>,
    teleports: Vec<Teleport>,
    states: Vec<ObjState>,
    healths: Vec<SetHealth>,
    home: Option<Teleport>,
    /// The scripts' groups that hold a living creature, in the play; at the start, none known.
    living: Option<&'a HashSet<u32>>,
    window: Window,
}

impl<'a> Recorder<'a> {
    fn new(
        (tree, creatures): (&'a str, &'a HashSet<u32>),
        state: &'a mut quests::Scripts,
        rng: &'a mut Rng,
    ) -> Self {
        Self {
            tree,
            creatures,
            state,
            rng,
            records: Vec::new(),
            teleports: Vec::new(),
            states: Vec::new(),
            healths: Vec::new(),
            home: None,
            living: None,
            window: Window::default(),
        }
    }
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
        // A guard: whether no creature of the group lives, wherever it stands (`0x48C860`); a
        // group never made is dead. At the start, before the world, not known.
        if name == "group_is_dead" {
            return match (self.living, tags.numbers("_0B").last()) {
                (Some(living), Some(&group)) => Handled::Answer(
                    u32::try_from(group)
                        .ok()
                        .is_none_or(|group| !living.contains(&(npcs::SCRIPT_GROUP | group))),
                ),
                _ => Handled::Unknown,
            };
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
            "def_pos" => {
                if let Some(name) = tags.texts("_01").first() {
                    self.state.positions.define(name, &tags.numbers("_0B"));
                }
                Handled::Done
            }
            "set_base_trigger" => {
                let positions = &self.state.positions;
                let first = tags
                    .spot("_0C", positions)
                    .or_else(|| tags.spot("_04", positions));
                let second = tags.spot("_0D", positions).map(|(x, y, _)| (x, y));
                if let (Some(name), Some(first)) = (tags.texts("_01").first(), first) {
                    self.state.triggers.set(name, first, second);
                }
                Handled::Done
            }
            "del_base_trigger" => {
                let positions = &self.state.positions;
                // The lower of the corners given (`0x4817F0`).
                let about = [tags.spot("_0C", positions), tags.spot("_0D", positions)]
                    .into_iter()
                    .flatten()
                    .map(|(x, y, _)| (x, y))
                    .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1)));
                if let Some(name) = tags.texts("_01").first() {
                    self.state.triggers.delete(name, about);
                }
                Handled::Done
            }
            "teleport" => {
                // The hero, named `hero` or not at all; another creature is not followed yet.
                let hero = tags
                    .texts("_01")
                    .first()
                    .is_none_or(|name| name.eq_ignore_ascii_case("hero"));
                let positions = &self.state.positions;
                let spot = ["_04", "_0C", "_0D"]
                    .iter()
                    .find_map(|key| tags.spot(key, positions));
                match spot {
                    Some((x, y, layer)) if hero => {
                        self.teleports.push(Teleport {
                            cell: (x, y),
                            layer: u8::try_from(layer).unwrap_or(0),
                        });
                        Handled::Done
                    }
                    _ => Handled::Unknown,
                }
            }
            "set_timer" => {
                // Its id the last `0B`; a record without its time, `38`, makes none (the
                // client's carries whatever its stack held, docs/re/netscript.md).
                if let (Some(function), Some(&id), Some(&time)) = (
                    tags.texts("_01").first(),
                    tags.numbers("_0B").last(),
                    tags.numbers("_38").last(),
                ) {
                    self.state.timers.set(id, function, time);
                }
                Handled::Done
            }
            "del_timer" => {
                if let Some(&id) = tags.numbers("_0B").last() {
                    self.state.timers.delete(id);
                }
                Handled::Done
            }
            "set_obj_state" => {
                let has = |key: &str| matches!(tags.values(key).first(), Some(Operand::Bool(true)));
                let barred = if has("_06") {
                    Some(true)
                } else if has("_07") {
                    Some(false)
                } else {
                    None
                };
                let open = if has("_0F") {
                    Some(true)
                } else if has("_10") {
                    Some(false)
                } else {
                    None
                };
                if let Some(name) = tags.texts("_01").first()
                    && (barred.is_some() || open.is_some())
                {
                    self.states.push(ObjState {
                        name: name.to_ascii_lowercase(),
                        barred,
                        open,
                    });
                }
                Handled::Done
            }
            // The hero's home: a `SetNPCState` of the tag `0x4D` naming the hero (`0x461540`,
            // `+0x2B9`), and a `StartPosition`, which teleports the hero in a game of one player
            // and copies its position into its home (45, `0x491B30`; #974).
            "set_npc_state" => {
                if tags
                    .texts("_01")
                    .first()
                    .is_some_and(|name| name.eq_ignore_ascii_case("hero"))
                    && let [x, y, rest @ ..] = tags.numbers("_4D").as_slice()
                {
                    self.home = Some(Teleport {
                        cell: (*x, *y),
                        layer: rest
                            .first()
                            .and_then(|&l| u8::try_from(l).ok())
                            .unwrap_or(0),
                    });
                }
                Handled::Done
            }
            "start_position" => {
                if let [x, y, rest @ ..] = tags.numbers("_0B").as_slice() {
                    let spot = Teleport {
                        cell: (*x, *y),
                        layer: rest
                            .first()
                            .and_then(|&l| u8::try_from(l).ok())
                            .unwrap_or(0),
                    };
                    self.teleports.push(spot);
                    self.home = Some(spot);
                }
                Handled::Done
            }
            // The counts of the kills and of the items collected (`0x4AF190`, `0x4AF8C0`; #1021):
            // their first type and count, and their function.
            "set_on_kill" | "set_on_collect" => {
                let kind = tags
                    .numbers("_02")
                    .first()
                    .and_then(|&k| u32::try_from(k).ok());
                let count = tags
                    .numbers("_0B")
                    .first()
                    .and_then(|&c| u32::try_from(c).ok());
                let function = tags
                    .texts("_05")
                    .first()
                    .or(tags.texts("_41").first())
                    .copied();
                if let (Some(kind), Some(count)) = (kind, count) {
                    let counts = if name == "set_on_kill" {
                        &mut self.state.kills
                    } else {
                        &mut self.state.collecting
                    };
                    counts.set(kind, count, function);
                }
                Handled::Done
            }
            // The groups' changes and the removals, which `npcs` carries out (#1021).
            "set_group_state" | "del_npc" => Handled::Done,
            // A dialogue's window (#1028): a text replaces the window's, or with `0x7C` adds to
            // it (`0x80EAA0`); a button its label and function, five kept (`0x499BA0`); a sound
            // played as the window opens (`0x4A9730`).
            "text" => {
                if let Some(key) = tags.texts("_1E").first() {
                    if tags.values("_7C").is_empty() {
                        self.window.texts.clear();
                    }
                    self.window.texts.push((*key).to_owned());
                }
                Handled::Done
            }
            "set_button" => {
                if let [label, function, ..] = tags.texts("_01").as_slice()
                    && label.len() >= 2
                    && self.window.buttons.len() < BUTTONS
                {
                    self.window
                        .buttons
                        .push(((*label).to_owned(), (*function).to_owned()));
                }
                Handled::Done
            }
            "play_sound" => {
                if let Some(sound) = tags.texts("_01").first() {
                    self.window.sounds.push((*sound).to_owned());
                }
                Handled::Done
            }
            // The speaker's dialogue taken away, and the function stopped (`0x477CA7`).
            "del_dlg" => {
                self.window.untalks = true;
                Handled::Answer(false)
            }
            "exit_dlg" => Handled::Done,
            "set_hp" => {
                // Its last name and number, the number held to 0..100 (`0x48E280`).
                let name = tags
                    .texts("_01")
                    .last()
                    .filter(|name| !name.eq_ignore_ascii_case("hero"))
                    .map(|name| name.to_ascii_lowercase());
                if let Some(&share) = tags.numbers("_0B").last() {
                    self.healths.push(SetHealth {
                        name,
                        share: u32::try_from(share.clamp(0, 100)).unwrap_or(0),
                    });
                }
                Handled::Done
            }
            "set_drop" => {
                // The two types, the creature first: the one of a creature's category
                // (`0x426610`) where the other is not (`0x4AFFF0`).
                if let ([a, b, ..], Some(&chance)) =
                    (tags.numbers("_02").as_slice(), tags.numbers("_0B").first())
                    && let (Ok(a), Ok(b)) = (u32::try_from(*a), u32::try_from(*b))
                {
                    let (creature, item) =
                        if self.creatures.contains(&b) && !self.creatures.contains(&a) {
                            (b, a)
                        } else {
                            (a, b)
                        };
                    self.state
                        .drops
                        .set(creature, item, u32::try_from(chance).unwrap_or(0));
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

    /// The cell of the tag `key`: its numbers `(x, y)` or `(x, y, layer)`, or the first three of
    /// the place its `DefPos` name names, as the operand reader gives a cell (`+0xA860` to
    /// `+0xA868`), the layer 0 where it is missing.
    fn spot(&self, key: &str, positions: &quests::Positions) -> Option<(i32, i32, i32)> {
        let numbers = match (self.numbers(key).as_slice(), self.texts(key).as_slice()) {
            (numbers @ [_, _, ..], _) => numbers.to_vec(),
            ([], [name]) => positions.get(name)?.to_vec(),
            _ => return None,
        };
        match numbers[..] {
            [x, y] => Some((x, y, 0)),
            [x, y, layer, ..] => Some((x, y, layer)),
            _ => None,
        }
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

/// Takes on the scripts' state that the load left, on the main thread, as the play starts.
fn begin(world: &mut World) {
    let Some(started) = world.remove_resource::<Started>() else {
        return;
    };
    world.insert_resource(HeroHome::default());
    world.insert_resource(ScriptDrops(started.state.drops.clone()));
    match runtime(&started.assets, &started.tree, started.index.clone()) {
        Ok(runtime) => world.insert_non_send(Live::new(runtime, started)),
        Err(e) => bevy::log::warn!("scripts: {e:#}; the scripts do not run on"),
    }
}

/// Drops the scripts of the game left.
fn end(world: &mut World) {
    world.remove_non_send::<Live>();
    world.remove_resource::<ScriptDrops>();
}

/// Hands the scripts' drops to the deaths' drops as the scripts change them.
fn share(live: Option<NonSendMut<Live>>, shared: Option<ResMut<ScriptDrops>>) {
    if let (Some(live), Some(mut shared)) = (live, shared)
        && shared.0 != live.state.drops
    {
        shared.0 = live.state.drops.clone();
    }
}

/// Runs the base trigger of the hero's cell as the cell changes on its way, not by a teleport,
/// and puts the hero where its functions teleport it, on its level (`Arrival::On`, as the command
/// `cell` does, as the scripts' `Teleport` puts a player).
fn step_on(
    live: Option<NonSendMut<Live>>,
    heights: Option<Res<Heights>>,
    mut heroes: Query<(&mut Transform, &mut Body)>,
    (mut states, mut healths, mut world, mut home): (
        ResMut<ObjStates>,
        ResMut<Healths>,
        ResMut<WorldRecords>,
        ResMut<HeroHome>,
    ),
    (npcs, minds): (Option<Res<npcs::Npcs>>, Query<&Mind, Without<Dies>>),
) {
    let (Some(mut live), Some(heights)) = (live, heights) else {
        return;
    };
    let Ok((mut transform, mut body)) = heroes.single_mut() else {
        return;
    };
    let cell = paths::cell_of(transform.translation.x, transform.translation.z);
    let before = live.at.replace(cell);
    // A teleport onto a trigger's cell runs nothing (seen: the original's `cell` onto one called
    // no lookup of `0x6230C0`, which every step of a walk calls).
    if body.landing() || before.is_none_or(|before| before == cell) {
        return;
    }
    let layer = heights.floor(body.level()).map_or(0, |(_, floor)| floor);
    live.living = living_groups(npcs.as_deref(), minds.iter());
    let effects = live.step_on(cell, layer);
    put(&mut live, &effects.teleports, &mut transform, &mut body);
    states.0.extend(effects.states);
    healths.0.extend(effects.healths);
    world.0.extend(effects.records);
    if let Some(spot) = effects.home {
        home.0 = Some(spot);
    }
}

/// Counts the clock's minutes on the timers, which run their functions as they are due, and
/// puts the hero where they teleport it.
fn timers(
    live: Option<NonSendMut<Live>>,
    clock: Option<Res<Clock>>,
    mut heroes: Query<(&mut Transform, &mut Body)>,
    (mut states, mut healths, mut world, mut home): (
        ResMut<ObjStates>,
        ResMut<Healths>,
        ResMut<WorldRecords>,
        ResMut<HeroHome>,
    ),
    (npcs, minds): (Option<Res<npcs::Npcs>>, Query<&Mind, Without<Dies>>),
) {
    let (Some(mut live), Some(clock)) = (live, clock) else {
        return;
    };
    if live.minute != Some(clock.minute()) {
        live.living = living_groups(npcs.as_deref(), minds.iter());
    }
    let effects = live.clock(clock.minute());
    if let Ok((mut transform, mut body)) = heroes.single_mut() {
        put(&mut live, &effects.teleports, &mut transform, &mut body);
    }
    states.0.extend(effects.states);
    healths.0.extend(effects.healths);
    world.0.extend(effects.records);
    if let Some(spot) = effects.home {
        home.0 = Some(spot);
    }
}

/// Runs the function `function` of the hero's tree as the scripts run one (`Live::run`, the
/// command `script`), the hero put where it teleports it: a line of what it did, and one of each
/// timer, its id, its function and the minutes it has still to run.
pub fn run_function(world: &mut World, function: &str) -> Result<Vec<String>, String> {
    let run = |live: &mut Live| {
        if live.runtime.entries(function) == 0 {
            return Err(format!("no function {function} in {}", live.tree));
        }
        let (effects, outcome) = live.call(function, 0);
        let line = match (outcome, live.quest_of(function)) {
            (Some(outcome), _) => format!("{function} ran: {outcome:?}"),
            (None, Some(quest)) => format!("{function} waits for its quest {quest}, not set up"),
            (None, None) => format!("{function} did not run"),
        };
        Ok((effects, line))
    };
    carry_out(world, run)
}

/// Sets the quest `quest` up or triggers it, as its records do, its sections run in turn (the
/// command `quest`): lines as [`run_function`] gives them, the quest's flags first.
pub fn run_transition(
    world: &mut World,
    transition: Transition,
    quest: u32,
) -> Result<Vec<String>, String> {
    let run = |live: &mut Live| {
        let effects = live.transition(transition, quest);
        let flags = live.state.quests.flags(quest);
        let line = format!(
            "quest {quest}: flags {flags:#x}{}",
            if live.state.quests.in_log(quest) {
                ", in the log"
            } else {
                ""
            }
        );
        Ok((effects, line))
    };
    carry_out(world, run)
}

/// Runs `run` on the scripts, the hero put where it teleports it and the objects' states handed
/// to `npcs`: its line, the hero's teleport, and a line of each timer, its id, its function and
/// the minutes it has still to run.
fn carry_out(
    world: &mut World,
    run: impl FnOnce(&mut Live) -> Result<(Effects, String), String>,
) -> Result<Vec<String>, String> {
    let (effects, first, timers) = {
        let living = living_in(world);
        let mut live = world
            .get_non_send_mut::<Live>()
            .ok_or("the scripts do not run")?;
        live.living = living;
        let (effects, first) = run(&mut live)?;
        let now = live.state.timers.minutes();
        let timers: Vec<String> = live
            .state
            .timers
            .list()
            .iter()
            .map(|t| {
                let due = (t.set + u64::try_from(t.time).unwrap_or(0)).saturating_sub(now);
                format!("timer {} runs {} in {due} minutes", t.id, t.function)
            })
            .collect();
        (effects, first, timers)
    };
    let mut lines = vec![first];
    lines.extend(hand_world(world, effects));
    lines.extend(timers);
    Ok(lines)
}

/// The scripts' groups that hold a living creature, of the world `world` ([`living_groups`]).
fn living_in(world: &mut World) -> HashSet<u32> {
    let mut minds = world.query_filtered::<&Mind, Without<Dies>>();
    let minds: Vec<Mind> = minds.iter(world).cloned().collect();
    living_groups(world.get_resource::<npcs::Npcs>(), minds.iter())
}

/// Hands `effects` on to the modules that carry them out, the hero put where the last teleport
/// names: a line of the teleport, if any.
fn hand_world(world: &mut World, effects: Effects) -> Option<String> {
    let Effects {
        teleports,
        states,
        healths,
        records,
        home,
        ..
    } = effects;
    world.resource_mut::<ObjStates>().0.extend(states);
    world.resource_mut::<Healths>().0.extend(healths);
    world.resource_mut::<WorldRecords>().0.extend(records);
    if let Some(spot) = home {
        world.resource_mut::<HeroHome>().0 = Some(spot);
    }
    let &Teleport {
        cell: (x, y),
        layer,
    } = teleports.last()?;
    let mut heroes = world.query::<(&mut Transform, &mut Body)>();
    let line = heroes
        .iter_mut(world)
        .next()
        .map(|(mut transform, mut body)| {
            body.put(
                &mut transform,
                iso::ground(x as f32 + 0.5, y as f32 + 0.5),
                Arrival::On(Some(layer)),
            );
            format!("the hero teleports to {x},{y} on layer {layer}")
        });
    if let Some(mut live) = world.get_non_send_mut::<Live>() {
        live.at = Some((x, y));
    }
    line
}

/// Runs the dialogue `dialogue`'s function for a talk (#1028, [`Live::talk`]), what it did to the
/// world carried out: its window's contents, none where the scripts do not run or have no such
/// function.
pub fn speak(world: &mut World, dialogue: &str) -> Option<Window> {
    let living = living_in(world);
    let effects = {
        let mut live = world.get_non_send_mut::<Live>()?;
        live.living = living;
        live.talk(dialogue)?
    };
    let window = effects.window.clone();
    hand_world(world, effects);
    Some(window)
}

/// Runs a dialogue's button's function `function`, found by its name case aside, with no
/// creature (`0x499DA0`, `0x46BA90(index, 0, 0)`; #1028), what it did to the world carried out.
pub fn answer(world: &mut World, function: &str) {
    let living = living_in(world);
    let effects = {
        let Some(mut live) = world.get_non_send_mut::<Live>() else {
            return;
        };
        live.living = living;
        live.run_named(function)
    };
    hand_world(world, effects);
}

/// The queues that hand what the scripts did to the world on to the modules that carry it out:
/// the objects' states, the healths, the records of the world and the hero's home.
#[derive(SystemParam)]
struct Queues<'w> {
    states: ResMut<'w, ObjStates>,
    healths: ResMut<'w, Healths>,
    world: ResMut<'w, WorldRecords>,
    home: ResMut<'w, HeroHome>,
}

impl Queues<'_> {
    /// Hands `effects` on, but the hero's teleports, which the caller carries out.
    fn hand(&mut self, effects: Effects) {
        self.states.0.extend(effects.states);
        self.healths.0.extend(effects.healths);
        self.world.0.extend(effects.records);
        if let Some(spot) = effects.home {
            self.home.0 = Some(spot);
        }
    }
}

/// The scripts' groups that hold a living creature, wherever it stands (`GroupIsDead`,
/// `0x48C860`): the world's creatures alive, but the raised dead (`+0x1F4` bit `0x2000000`), and
/// those that live away from it (`npcs::Npcs::living_groups`).
fn living_groups<'a>(
    npcs: Option<&npcs::Npcs>,
    minds: impl Iterator<Item = &'a Mind>,
) -> HashSet<u32> {
    let mut living = npcs.map(npcs::Npcs::living_groups).unwrap_or_default();
    living.extend(
        minds
            .filter(|mind| mind.flags & crate::followers::RAISED == 0)
            .filter_map(|mind| mind.group),
    );
    living
}

/// Runs the death's function of each creature that dies (the AI's state 9, `0x53E7C0`, and the
/// death's event at `0x52F769`; #656): the first of its list's that is no item's, the others of
/// its kind taken out ([`quests::DeathFunctions::take`]), by its name ([`Live::die`]), after its
/// drops and its killer's experience, the hero put where it teleports it. A creature whose
/// function gives it health back, its `SetHP` above 0, rises at that health (`0x53E460`) before
/// it is laid down (`npcs::die`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn deaths(
    mut commands: Commands,
    live: Option<NonSendMut<Live>>,
    mut dying: Query<
        (
            Entity,
            Option<&mut OnDeath>,
            Option<&ScriptName>,
            &mut Health,
            &Dies,
            Option<&Mind>,
        ),
        Added<Dies>,
    >,
    mut heroes: Query<(&mut Transform, &mut Body), With<Body>>,
    mut queues: Queues,
    (npcs, minds): (Option<Res<npcs::Npcs>>, Query<&Mind, Without<Dies>>),
) {
    let Some(mut live) = live else {
        return;
    };
    let mut taken = false;
    for (entity, on_death, name, mut health, dies, mind) in &mut dying {
        if !taken {
            live.living = living_groups(npcs.as_deref(), minds.iter());
            taken = true;
        }
        // A kill of a type that a `SetOnKill` counts, whoever killed it, but not a death at
        // nobody's hands nor one of class 5 (`0x534795`, `0x534F60`, `0x5337AB`; #1021).
        let kill = mind
            .filter(|mind| dies.killed && mind.class != 5)
            .map(|mind| mind.creature);
        let Some(function) = on_death.and_then(|mut on_death| on_death.0.take()) else {
            if let Some(creature) = kill {
                let effects = live.kill(creature);
                if let Ok((mut transform, mut body)) = heroes.single_mut() {
                    put(&mut live, &effects.teleports, &mut transform, &mut body);
                }
                queues.hand(effects);
            }
            continue;
        };
        bevy::log::info!(
            "scripts: {} dies and runs {function}",
            name.map_or("a creature", |name| name.0.as_str())
        );
        let mut effects = live.run_named(&function);
        // The last `SetHP` of its own name decides its health (`0x53E93C`).
        let rises = name
            .and_then(|name| {
                effects
                    .healths
                    .iter()
                    .rev()
                    .find(|set| set.name.as_deref() == Some(name.0.as_str()))
            })
            .filter(|set| set.share > 0);
        if let Some(set) = rises {
            health.current = (health.most * set.share / 100).max(1);
            commands.entity(entity).remove::<Dies>();
            bevy::log::info!(
                "scripts: {} rises at {}/{}",
                name.map_or("a creature", |name| name.0.as_str()),
                health.current,
                health.most
            );
        } else if let Some(creature) = kill {
            // One that rises is no kill: its event's handler ends there (`0x52F7A8`).
            let counted = live.kill(creature);
            effects.extend(counted);
        }
        if let Ok((mut transform, mut body)) = heroes.single_mut() {
            put(&mut live, &effects.teleports, &mut transform, &mut body);
        }
        queues.hand(effects);
    }
}

/// Counts each item that the hero picks up on the scripts' collecting (`SetOnCollect`,
/// `0x4AAAE0`; #1021): one that a count counts goes from its pack among the main quest's items
/// (docs/re/items.md, "What becomes of the item"), and a count done runs its function.
fn collect(
    live: Option<NonSendMut<Live>>,
    mut taken: MessageReader<crate::pickup::Taken>,
    mut pack: ResMut<crate::pickup::Pack>,
    mut heroes: Query<(&mut Transform, &mut Body)>,
    mut queues: Queues,
    (npcs, minds): (Option<Res<npcs::Npcs>>, Query<&Mind, Without<Dies>>),
) {
    let Some(mut live) = live else {
        taken.clear();
        return;
    };
    for &crate::pickup::Taken(item) in taken.read() {
        live.living = living_groups(npcs.as_deref(), minds.iter());
        let Some(effects) = live.collect(item) else {
            continue;
        };
        pack.keep_for_quest(item);
        if let Ok((mut transform, mut body)) = heroes.single_mut() {
            put(&mut live, &effects.teleports, &mut transform, &mut body);
        }
        queues.hand(effects);
    }
}

/// Puts the hero where the last of `teleports` names, on its level.
fn put(live: &mut Live, teleports: &[Teleport], transform: &mut Transform, body: &mut Body) {
    if let Some(&Teleport {
        cell: (x, y),
        layer,
    }) = teleports.last()
    {
        bevy::log::info!("scripts: the hero teleports to {x},{y} on layer {layer}");
        body.put(
            transform,
            iso::ground(x as f32 + 0.5, y as f32 + 0.5),
            Arrival::On(Some(layer)),
        );
        live.at = Some((x, y));
    }
}

/// The scripts through the play.
pub struct ScriptsPlugin;

impl Plugin for ScriptsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ObjStates>()
            .init_resource::<Healths>()
            .init_resource::<WorldRecords>()
            .init_resource::<HeroHome>()
            .add_systems(OnEnter(AppState::InGame), begin)
            .add_systems(OnExit(AppState::InGame), end)
            .add_systems(
                Update,
                (step_on, timers, share)
                    .chain()
                    .after(Step::HeroMove)
                    .before(Step::Follow)
                    .in_set(Playing)
                    .in_set(Unpaused),
            )
            .add_systems(
                Update,
                deaths
                    .after(crate::fight::resolve)
                    .after(npcs::script_healths)
                    .before(npcs::die)
                    .in_set(Playing),
            )
            .add_systems(
                Update,
                collect
                    .after(crate::pickup::take)
                    .in_set(Playing)
                    .in_set(Unpaused),
            );
    }
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
        let records = records(
            &runtime,
            ("TEST", &HashSet::new()),
            START,
            0,
            &mut state,
            &mut rng,
        );
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
        let records = records(
            &runtime,
            ("TYPE_NPC_GLADIATOR", &HashSet::new()),
            START,
            0,
            &mut state,
            &mut rng,
        );
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
    fn a_death_runs_its_function_by_its_name_once_its_group_is_dead() {
        let runtime = runtime(&[(
            "start.luau",
            "local F = {}\n\
             F[\"StartCode\"] = function()\n\
             return true\n\
             end\n\
             F[\"tot1201\"] = function()\n\
             if not group_is_dead { _0B = 1201 } then\n\
             return false\n\
             end\n\
             set_hp { _01 = \"res:17631\", _0B = 50 }\n\
             return true\n\
             end\n\
             return F",
        )]);
        let mut live = live(runtime);
        // Another of the group lives, somewhere: the function stops at its guard.
        live.living = HashSet::from([npcs::SCRIPT_GROUP | 1201]);
        assert!(live.run_named("TOT1201").healths.is_empty());
        // The last of it dies: the function runs on, found by its name case aside.
        live.living = HashSet::new();
        let effects = live.run_named("TOT1201");
        assert_eq!(
            effects.healths,
            [SetHealth {
                name: Some("res:17631".into()),
                share: 50
            }]
        );
        assert!(live.run_named("no_such_function").healths.is_empty());
    }

    #[test]
    fn a_dialogue_gives_its_window_its_texts_buttons_and_sounds() {
        let runtime = runtime(&[
            (
                "quests/22.luau",
                "local F = {}\n\
             F[\"Dialog:auftrag22\"] = function()\n\
             text { _1E = \"first\" }\n\
             text { _1E = \"replaced\" }\n\
             text { _1E = \"added\", _7C = true }\n\
             play_sound { _01 = \"replaced\" }\n\
             set_button { _01 = { \"Res:1024\", \"trigger22\" } }\n\
             set_button { _01 = { \"x\", \"nothing\" } }\n\
             return true\n\
             end\n\
             F[\"Dialog:notmage\"] = function()\n\
             if not del_dlg {} then\n\
             return false\n\
             end\n\
             text { _1E = \"never\" }\n\
             return true\n\
             end\n\
             return F",
            ),
            (
                "start.luau",
                "local F = {}\n\
             F[\"StartCode\"] = function()\n\
             return true\n\
             end\n\
             return F",
            ),
        ]);
        let mut live = live(runtime);
        // A quest's dialogue waits for its quest.
        assert!(
            live.talk("auftrag22")
                .is_some_and(|e| e.window.texts.is_empty())
        );
        live.state.quests.set_up(22, &mut |_| true);
        let window = live.talk("AUFTRAG22").expect("the dialogue runs").window;
        assert_eq!(window.texts, ["replaced", "added"]);
        assert_eq!(window.sounds, ["replaced"]);
        assert_eq!(window.buttons, [("Res:1024".into(), "trigger22".into())]);
        assert!(!window.untalks);
        // `DelDlg` takes the dialogue away and stops the function.
        let window = live.talk("notmage").expect("the dialogue runs").window;
        assert!(window.untalks && window.texts.is_empty());
        assert!(live.talk("no_such").is_none());
    }

    #[test]
    fn the_kills_and_the_items_collected_run_their_functions_once_counted() {
        let runtime = runtime(&[(
            "start.luau",
            "local F = {}\n\
             F[\"StartCode\"] = function()\n\
             return true\n\
             end\n\
             F[\"bounty\"] = function()\n\
             set_on_kill { _02 = 691, _05 = \"ok_10252\", _0B = 2 }\n\
             set_drop { _02 = { 38, 6005 }, _0B = 75 }\n\
             set_on_collect { _02 = 38, _0B = 1, _41 = \"oc_10250\" }\n\
             return true\n\
             end\n\
             F[\"OK_10252\"] = function()\n\
             set_hp { _0B = 100 }\n\
             return true\n\
             end\n\
             F[\"oc_10250\"] = function()\n\
             set_hp { _0B = 50 }\n\
             return true\n\
             end\n\
             return F",
        )]);
        let mut live = live(runtime);
        live.creatures = HashSet::from([38, 691]);
        live.run_named("bounty");
        // The first kill counts, the second runs the function by its name case aside.
        assert!(live.kill(691).healths.is_empty());
        assert!(live.kill(190).healths.is_empty());
        assert_eq!(live.kill(691).healths.len(), 1);
        assert!(live.kill(691).healths.is_empty());
        // The drop of the creature 38 counts as its item is picked up; another item does not.
        assert!(live.collect(1).is_none());
        let collected = live.collect(6005).expect("6005 is counted");
        assert_eq!(collected.healths.len(), 1);
        assert!(live.collect(6005).is_none());
    }

    #[test]
    fn a_creature_that_its_deaths_function_heals_rises() {
        let runtime = runtime(&[(
            "start.luau",
            "local F = {}\n\
             F[\"StartCode\"] = function()\n\
             return true\n\
             end\n\
             F[\"tot2203\"] = function()\n\
             set_hp { _01 = \"res:17260\", _0B = 50 }\n\
             return true\n\
             end\n\
             F[\"tot2\"] = function()\n\
             set_hp { _01 = \"res:17260\", _0B = 0 }\n\
             return true\n\
             end\n\
             return F",
        )]);
        let mut app = bevy::app::App::new();
        app.world_mut().insert_non_send(live(runtime));
        app.init_resource::<ObjStates>()
            .init_resource::<Healths>()
            .init_resource::<WorldRecords>()
            .init_resource::<HeroHome>()
            .add_systems(Update, deaths);
        let dying = |app: &mut App, function: &str| {
            let mut functions = quests::DeathFunctions::default();
            functions.add(function);
            app.world_mut()
                .spawn((
                    OnDeath(functions),
                    ScriptName("res:17260".into()),
                    Health {
                        current: 0,
                        most: 300,
                        level: 10,
                    },
                    Dies::new(0),
                ))
                .id()
        };
        // Its function gives it half its health: it rises, its function spent.
        let target = dying(&mut app, "tot2203");
        // Its function leaves it at 0: it stays dead.
        let other = dying(&mut app, "tot2");
        app.update();
        let world = app.world();
        assert!(!world.entity(target).contains::<Dies>());
        assert_eq!(world.get::<Health>(target).map(|h| h.current), Some(150));
        assert!(world.get::<OnDeath>(target).is_some_and(|d| d.0.is_empty()));
        assert!(world.entity(other).contains::<Dies>());
        assert_eq!(world.resource::<Healths>().0.len(), 2);
    }

    #[test]
    fn the_scripts_move_the_heros_home() {
        let runtime = runtime(&[(
            "start.luau",
            "local F = {}\n\
             F[\"StartCode\"] = function()\n\
             return true\n\
             end\n\
             F[\"OMO-1leave_rg1\"] = function()\n\
             set_npc_state { _01 = \"res:17459\", _39 = true, _4D = { 3230, 2766, 0 } }\n\
             set_npc_state { _01 = \"HERO\", _4D = { 3230, 2769, 0 } }\n\
             return true\n\
             end\n\
             F[\"start_again\"] = function()\n\
             start_position { _0B = { 3236, 2511, 1 } }\n\
             return true\n\
             end\n\
             return F",
        )]);
        let mut live = live(runtime);
        // Another creature's home is not the hero's.
        assert_eq!(
            live.run("OMO-1leave_rg1", 0).home,
            Some(Teleport {
                cell: (3230, 2769),
                layer: 0
            })
        );
        // A `StartPosition` puts the hero there and makes it its home.
        let start = Teleport {
            cell: (3236, 2511),
            layer: 1,
        };
        let effects = live.run("start_again", 0);
        assert_eq!(
            (effects.teleports, effects.home),
            (vec![start], Some(start))
        );
    }

    #[test]
    fn a_set_hp_names_its_creature_or_the_hero() {
        let runtime = runtime(&[(
            "start.luau",
            "local F = {}\n\
             F[\"StartCode\"] = function()\n\
             return true\n\
             end\n\
             F[\"tot1202\"] = function()\n\
             set_hp { _01 = \"Res:17163\", _0B = 0 }\n\
             set_hp { _01 = \"HERO\", _0B = 150 }\n\
             set_hp { _0B = 25 }\n\
             return true\n\
             end\n\
             return F",
        )]);
        let mut live = live(runtime);
        let set = |name: Option<&str>, share| SetHealth {
            name: name.map(str::to_owned),
            share,
        };
        assert_eq!(
            live.run("tot1202", 0).healths,
            [set(Some("res:17163"), 0), set(None, 100), set(None, 25)]
        );
    }

    #[test]
    fn a_quests_transitions_run_its_sections_and_their_objects_states() {
        let runtime = runtime(&[(
            "quests/2411.luau",
            "local F = {}\n\
             F[\"StartCode\"] = function()\n\
             return true\n\
             end\n\
             F[\"QIS_Trigger2411\"] = function()\n\
             return true\n\
             end\n\
             F[\"QIS_OnEnter2411\"] = function()\n\
             set_obj_state { _01 = \"R24NQ11_Tür\", _07 = true }\n\
             set_obj_state { _01 = \"gate\", _06 = true, _10 = true, marks = { 0x06, 0x10 } }\n\
             set_obj_state { _01 = \"well\", _41 = \"take\", _75 = 1 }\n\
             return true\n\
             end\n\
             return F",
        )]);
        let mut live = live(runtime);
        // Not set up, a trigger enters nothing.
        assert_eq!(
            live.transition(Transition::Trigger, 2411),
            Effects::default()
        );
        assert_eq!(live.transition(Transition::SetUp, 2411), Effects::default());
        let entered = live.transition(Transition::Trigger, 2411);
        assert!(live.state.quests.in_log(2411));
        let state = |name: &str, barred, open| ObjState {
            name: name.into(),
            barred,
            open,
        };
        // The bar and the open bit alone; a record of neither sets nothing of them.
        assert_eq!(
            entered.states,
            [
                state("r24nq11_tür", Some(false), None),
                state("gate", Some(true), Some(false))
            ]
        );
    }

    #[test]
    fn the_hero_on_a_base_trigger_runs_its_functions_and_is_teleported() {
        let runtime = runtime(&[
            (
                "start.luau",
                "local F = {}\n\
                 F[\"StartCode\"] = function()\n\
                 def_pos { _01 = \"tpreinz2.3\", _0B = { 4083, 83 } }\n\
                 set_base_trigger { _01 = \"tprein2.3\", _0C = { 3046, 2892, 0 }, _0D = { 3047, 2893, 0 } }\n\
                 set_base_trigger { _01 = \"Wolfattack\", _0C = { 3205, 2902, 0 }, _0D = { 3205, 2916, 0 } }\n\
                 set_base_trigger { _01 = \"gone\", _0C = \"tpreinz2.3\" }\n\
                 del_base_trigger { _01 = \"GONE\" }\n\
                 return true\n\
                 end\n\
                 return F",
            ),
            (
                "triggers.luau",
                "local F = {}\n\
                 F[\"OMO-1tprein2.3\"] = function()\n\
                 teleport { _03 = 45, _04 = \"tpreinz2.3\" }\n\
                 return true\n\
                 end\n\
                 F[\"OMO-1gone\"] = function()\n\
                 teleport { _04 = { 1, 1, 0 } }\n\
                 return true\n\
                 end\n\
                 return F",
            ),
            (
                "quests/201.luau",
                "local F = {}\n\
                 F[\"OMO201Wolfattack\"] = function()\n\
                 set_var { _01 = \"wolves\", _0B = 1 }\n\
                 teleport { _01 = \"res:18008\", _04 = { 3205, 2910, 0 } }\n\
                 return true\n\
                 end\n\
                 return F",
            ),
        ]);
        let mut state = quests::Scripts::default();
        let mut rng = Rng::of(0, 0, SEED);
        records(
            &runtime,
            ("TEST", &HashSet::new()),
            START,
            0,
            &mut state,
            &mut rng,
        );
        let started = Started {
            assets: PathBuf::new(),
            tree: "TEST".into(),
            index: runtime.index().clone(),
            state,
            done: runtime.done(),
            rng,
            creatures: HashSet::new(),
        };
        let mut live = Live::new(runtime, started);
        // The cave's entrance puts the hero at its target, on the ground.
        assert_eq!(
            live.step_on((3047, 2893), 1).teleports,
            [Teleport {
                cell: (4083, 83),
                layer: 0
            }]
        );
        // Quest 201's function waits for its quest; another creature's teleport is not followed.
        assert!(live.step_on((3205, 2910), 0).teleports.is_empty());
        assert_eq!(live.state.variables.get("wolves"), None);
        live.state.quests.set_up(201, &mut |_| true);
        assert!(live.step_on((3205, 2910), 0).teleports.is_empty());
        assert_eq!(live.state.variables.get("wolves"), Some(1));
        // A trigger taken out runs nothing, and a cell of none neither.
        assert!(live.step_on((4083, 83), 0).teleports.is_empty());
        assert!(live.step_on((3048, 2893), 0).teleports.is_empty());
    }

    /// The scripts of `runtime` as they go on after its start script ran.
    fn live(runtime: Runtime) -> Live {
        let mut state = quests::Scripts::default();
        let mut rng = Rng::of(0, 0, SEED);
        records(
            &runtime,
            ("TEST", &HashSet::new()),
            START,
            0,
            &mut state,
            &mut rng,
        );
        let started = Started {
            assets: PathBuf::new(),
            tree: "TEST".into(),
            index: runtime.index().clone(),
            state,
            done: runtime.done(),
            rng,
            creatures: HashSet::new(),
        };
        Live::new(runtime, started)
    }

    #[test]
    fn the_timers_run_their_functions_as_the_clocks_minutes_pass() {
        let runtime = runtime(&[
            (
                "start.luau",
                "local F = {}\n\
                 F[\"StartCode\"] = function()\n\
                 set_up_quest { _0B = 1202 }\n\
                 set_timer { _01 = \"zeit1202\", _0B = 1202, _38 = 4320 }\n\
                 set_timer { _01 = \"tick\", _0B = 1, _38 = 1 }\n\
                 set_timer { _01 = \"gone\", _0B = 2, _38 = 1 }\n\
                 del_timer { _0B = 2 }\n\
                 set_timer { _01 = \"no time\", _0B = { 1401, 4320 } }\n\
                 return true\n\
                 end\n\
                 F[\"tick\"] = function()\n\
                 inc_var { _01 = \"ticks\", _0B = 1 }\n\
                 set_timer { _01 = \"tick\", _0B = 1, _38 = 1 }\n\
                 return true\n\
                 end\n\
                 F[\"gone\"] = function()\n\
                 set_var { _01 = \"gone\", _0B = 1 }\n\
                 return true\n\
                 end\n\
                 return F",
            ),
            (
                "quests/1202.luau",
                "local F = {}\n\
                 F[\"zeit1202\"] = function()\n\
                 set_var { _01 = \"late\", _0B = 1 }\n\
                 teleport { _04 = { 3236, 2511, 1 } }\n\
                 return true\n\
                 end\n\
                 return F",
            ),
        ]);
        let mut live = live(runtime);
        assert_eq!(live.state.timers.list().len(), 2);
        // The first sight of the clock counts nothing.
        assert!(live.clock(23 * 60 + 58).teleports.is_empty());
        // 23:59: the timer of one minute runs its function, which sets it again, and then every
        // timer of its id is taken out, the one set again with them (`0x4171B0`).
        live.clock(23 * 60 + 59);
        assert_eq!(live.state.variables.get("ticks"), Some(1));
        assert_eq!(live.state.timers.list().len(), 1);
        // 00:00 ends a day and runs nothing; 00:01 neither, nothing being due.
        live.state.timers.set(1, "tick", 1);
        live.clock(0);
        assert_eq!(live.state.variables.get("ticks"), Some(1));
        live.clock(1);
        assert_eq!(live.state.variables.get("ticks"), Some(2));
        assert_eq!(live.state.timers.minutes(), 3);
        assert_eq!(live.state.variables.get("gone"), None);
        // Three days on, quest 1202's timer runs its function, which teleports the hero: 4,320
        // minutes after the start, at 23:58 again, the hour by hour of the clock between.
        let mut now = 1;
        while live.state.timers.minutes() + 60 < 4_320 {
            now = (now + 60) % DAY;
            assert!(live.clock(now).teleports.is_empty());
        }
        let left = u16::try_from(4_320 - 1 - live.state.timers.minutes()).expect("an hour");
        assert!(live.clock((now + left) % DAY).teleports.is_empty());
        assert_eq!(
            live.clock((now + left + 1) % DAY).teleports,
            [Teleport {
                cell: (3236, 2511),
                layer: 1
            }]
        );
        assert_eq!(live.state.variables.get("late"), Some(1));
        assert!(live.state.timers.list().is_empty());
    }

    #[test]
    fn a_timer_of_a_finished_quest_runs_nothing() {
        let runtime = runtime(&[(
            "quests/7.luau",
            "local F = {}\n\
             F[\"StartCode\"] = function()\n\
             return true\n\
             end\n\
             F[\"late7\"] = function()\n\
             set_var { _01 = \"late\", _0B = 1 }\n\
             return true\n\
             end\n\
             return F",
        )]);
        let mut live = live(runtime);
        live.state.quests.set_up(7, &mut |_| true);
        live.state.quests.trigger(7, &mut |_| true);
        live.state.timers.set(7, "late7", 1);
        live.state.quests.exit(7, &mut |_| true);
        live.clock(600);
        live.clock(601);
        assert_eq!(live.state.variables.get("late"), None);
        assert!(live.state.timers.list().is_empty());
    }

    #[test]
    fn the_scripts_drops_take_their_creature_by_its_category() {
        let runtime = runtime(&[(
            "start.luau",
            "local F = {}
             F[\"StartCode\"] = function()
             set_drop { _02 = { 38, 6005 }, _0B = 75 }
             set_drop { _02 = { 38, 4931 }, _0B = 65 }
             set_drop { _02 = { 1355, 99 }, _0B = 75 }
             set_drop { _02 = { 38, 4931 }, _0B = 0 }
             return true
             end
             return F",
        )]);
        let mut state = quests::Scripts::default();
        let mut rng = Rng::of(0, 0, SEED);
        // Quest 15067's antidote names its item first (docs/re/netscript.md).
        let creatures = HashSet::from([38, 99]);
        records(
            &runtime,
            ("TEST", &creatures),
            START,
            0,
            &mut state,
            &mut rng,
        );
        assert_eq!(state.drops.of(38).collect::<Vec<_>>(), [(6005, 75)]);
        assert_eq!(state.drops.of(99).collect::<Vec<_>>(), [(1355, 75)]);
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
        records(
            &runtime,
            ("TEST", &HashSet::new()),
            START,
            0,
            &mut state,
            &mut rng,
        );
        assert_eq!(state.variables.get("grvarNr"), Some(194));
        assert_eq!(state.variables.get("grabstein_dialog_0"), Some(7));
        assert_eq!(state.variables.get("grabstein_dialog_193"), Some(7));
        assert_eq!(state.variables.get("grabstein_dialog_194"), None);
    }
}
