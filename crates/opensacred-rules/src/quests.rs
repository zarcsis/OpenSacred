//! The scripts' state (#724, part of #720; docs/re/netscript.md, "Quests" and "`SetNPCState`"),
//! apart from Luau and from the engine: the quests' flags and the transitions of `SetUpQuest`,
//! `TriggerQuest`, `ExitQuest`, `LoseQuest` and `UnTriggerQuest`, the variables by name, and the
//! dialogues' table that the start script's records of type 40 fill. A transition names the
//! section functions it runs for its caller, the scripts' runtime, to run, and takes each one's
//! answer, whether its body ran to its end (`0x46BA90`); all of it serializes, for the saves.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// A quest's flags (`+0x120` of its record of 0x124 bytes, `0xAABF18`): set up, finished,
/// entered, and while it is being set up.
pub const SET_UP: u32 = 1;
pub const FINISHED: u32 = 2;
pub const ENTERED: u32 = 4;
pub const SETTING_UP: u32 = 8;

/// A quest's section function (`0x465690`: the indices at `+0x10C` to `+0x11C`), named by the
/// compiler for its quest's number (`0x45A370`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Section {
    Trigger,
    OnSetUp,
    OnEnter,
    OnExit,
    OnLose,
}

impl Section {
    /// The name of the section function of the quest `quest`: `QIS_Trigger<n>` and the others.
    pub fn function(self, quest: u32) -> String {
        let name = match self {
            Self::Trigger => "QIS_Trigger",
            Self::OnSetUp => "QIS_OnSetUp",
            Self::OnEnter => "QIS_OnEnter",
            Self::OnExit => "QIS_OnExit",
            Self::OnLose => "QIS_OnLose",
        };
        format!("{name}{quest}")
    }
}

/// A quest's transition that runs its sections in the middle of its record, step by step
/// ([`Quests::step`]), for a runtime that runs each section and comes back with its answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Transition {
    /// `SetUpQuest` ([`Quests::set_up`]).
    SetUp,
    /// `TriggerQuest` ([`Quests::trigger`]).
    Trigger,
}

/// The quests' flags by number, and the quests in the quest log.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quests {
    flags: BTreeMap<u32, u32>,
    log: BTreeSet<u32>,
}

impl Quests {
    /// The flags of the quest `quest`, 0 for one never set up.
    pub fn flags(&self, quest: u32) -> u32 {
        self.flags.get(&quest).copied().unwrap_or(0)
    }

    /// Whether the quest `quest` is in the quest log.
    pub fn in_log(&self, quest: u32) -> bool {
        self.log.contains(&quest)
    }

    /// Runs a section of the quest `quest` through `run`, which answers whether its body ran to
    /// its end: none for a quest not set up (`0x46BA90`).
    fn run(&self, quest: u32, section: Section, run: &mut impl FnMut(&str) -> bool) -> bool {
        self.flags(quest) & SET_UP != 0 && run(&section.function(quest))
    }

    /// `SetUpQuest` (21, `0x48F7F0`, `0x46DB20`): once, the flags 1 and 8, its `OnSetUp`, its
    /// trigger, whose answer it ignores, then the flag 8 off; it does not enter the quest.
    pub fn set_up(&mut self, quest: u32, run: &mut impl FnMut(&str) -> bool) {
        self.steps(Transition::SetUp, quest, run);
    }

    /// `TriggerQuest` (20, `0x48F030`): for a quest set up, its trigger, and where that runs
    /// through, its entry (`0x46C160`): unless it is entered already, a quest log entry, the flag
    /// 4 and its `OnEnter`. Whether it entered the quest now.
    pub fn trigger(&mut self, quest: u32, run: &mut impl FnMut(&str) -> bool) -> bool {
        self.steps(Transition::Trigger, quest, run).len() > 1
    }

    /// A transition through, its sections run by `run`: their answers.
    fn steps(
        &mut self,
        transition: Transition,
        quest: u32,
        run: &mut impl FnMut(&str) -> bool,
    ) -> Vec<bool> {
        let mut ran = Vec::new();
        while let Some(section) = self.step(transition, quest, &ran) {
            ran.push(run(&section.function(quest)));
        }
        ran
    }

    /// The next section that the transition `transition` of the quest `quest` runs, its sections
    /// so far having answered `ran`, and the flags it sets on the way; none once it is through.
    /// `SetUpQuest` sets a quest not yet set up up, the flags 1 and 8, runs its `OnSetUp` and its
    /// trigger whatever they answer, then takes the flag 8 off. `TriggerQuest` runs the trigger of
    /// a quest set up, and where that runs through, enters the quest unless it is entered already
    /// (`0x46C160`): a quest log entry, the flag 4 and its `OnEnter`.
    pub fn step(&mut self, transition: Transition, quest: u32, ran: &[bool]) -> Option<Section> {
        let set_up = self.flags(quest) & SET_UP != 0;
        match (transition, ran) {
            (Transition::SetUp, []) => {
                if set_up {
                    return None;
                }
                *self.flags.entry(quest).or_default() |= SET_UP | SETTING_UP;
                Some(Section::OnSetUp)
            }
            (Transition::SetUp, [_]) => Some(Section::Trigger),
            (Transition::SetUp, _) => {
                if let Some(flags) = self.flags.get_mut(&quest) {
                    *flags &= !SETTING_UP;
                }
                None
            }
            (Transition::Trigger, []) => set_up.then_some(Section::Trigger),
            (Transition::Trigger, [true]) => {
                if self.flags(quest) & ENTERED != 0 {
                    return None;
                }
                self.log.insert(quest);
                *self.flags.entry(quest).or_default() |= ENTERED;
                Some(Section::OnEnter)
            }
            (Transition::Trigger, _) => None,
        }
    }

    /// `ExitQuest` (15, `0x48EE20`, `0x46CBE0`): for a quest entered and not finished, its
    /// `OnExit` and the flag 2; whether it did.
    pub fn exit(&mut self, quest: u32, run: &mut impl FnMut(&str) -> bool) -> bool {
        self.finish(quest, Section::OnExit, run)
    }

    /// `LoseQuest` (54): as `ExitQuest`, its `OnLose`; whether it did.
    pub fn lose(&mut self, quest: u32, run: &mut impl FnMut(&str) -> bool) -> bool {
        self.finish(quest, Section::OnLose, run)
    }

    /// An exit or a loss: for a quest entered and not finished, its section and the flag 2.
    fn finish(&mut self, quest: u32, section: Section, run: &mut impl FnMut(&str) -> bool) -> bool {
        let flags = self.flags(quest);
        if flags & ENTERED == 0 || flags & FINISHED != 0 {
            return false;
        }
        self.run(quest, section, run);
        *self.flags.entry(quest).or_default() |= FINISHED;
        true
    }

    /// `UnTriggerQuest` (77, `0x48E600`): the flag 4 off and the quest log's entry out.
    pub fn untrigger(&mut self, quest: u32) {
        if let Some(flags) = self.flags.get_mut(&quest) {
            *flags &= !ENTERED;
        }
        self.log.remove(&quest);
    }
}

/// The longest name of a variable, its record's `char name[32]` with its end (`this+0x7550`).
pub const VARIABLE_NAME: usize = 31;

/// The scripts' variables by name (`this+0x7550`, records of `{char name[32]; i32 value}`,
/// docs/re/netscript.md, "Variable references"), a name kept to its first 31 bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variables(BTreeMap<String, i32>);

impl Variables {
    /// The name as its record keeps it.
    fn key(name: &str) -> String {
        let mut end = name.len().min(VARIABLE_NAME);
        while !name.is_char_boundary(end) {
            end -= 1;
        }
        name[..end].to_owned()
    }

    /// The value of the variable `name`, if one was set (**hypothesis**: a condition on a
    /// variable never set reads 0, [`Self::value`]).
    pub fn get(&self, name: &str) -> Option<i32> {
        self.0.get(&Self::key(name)).copied()
    }

    /// The value of the variable `name`, 0 for one never set.
    pub fn value(&self, name: &str) -> i32 {
        self.get(name).unwrap_or(0)
    }

    /// `SetVar`: the variable `name` set to `value`, made where it is new.
    pub fn set(&mut self, name: &str, value: i32) {
        self.0.insert(Self::key(name), value);
    }
}

/// An entry of the dialogues' table (`this+0x755C`): its name, its number (`+0`, −1 in every
/// entry of the scripts), and the creature it is stamped on (`+0x4C`), if any.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dialogue {
    pub name: String,
    pub number: i32,
    pub creature: Option<u32>,
}

/// The dialogues' table that the start script's records of type 40 fill, in their order, its
/// first entry added twice (`0x476A05`), so that the `n`-th dialogue from 1 stands at the index
/// `n` (seen: the captain's `auftrag22` at 958).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dialogues(Vec<Dialogue>);

impl Dialogues {
    /// Adds the dialogue `name` of the number `number` at the table's end, the first one twice.
    pub fn add(&mut self, name: &str, number: i32) {
        let entry = Dialogue {
            name: name.to_owned(),
            number,
            creature: None,
        };
        if self.0.is_empty() {
            self.0.push(entry.clone());
        }
        self.0.push(entry);
    }

    /// The index of the first entry named `name`, case aside (`0x859690`).
    pub fn by_name(&self, name: &str) -> Option<usize> {
        self.0
            .iter()
            .position(|entry| entry.name.eq_ignore_ascii_case(name))
    }

    /// The index of the first entry of the number `number`.
    pub fn by_number(&self, number: i32) -> Option<usize> {
        self.0.iter().position(|entry| entry.number == number)
    }

    /// The dialogue a creature takes for an entry found at `index` (`SetNPCState`'s `0x09` and
    /// `0x0A`): an index above 0, which takes the creature, `creature`; the index 0, as the first
    /// name finds, or none found takes its dialogue away.
    pub fn stamp(&mut self, index: Option<usize>, creature: u32) -> Option<usize> {
        let index = index.filter(|&index| index > 0)?;
        let entry = self.0.get_mut(index)?;
        entry.creature = Some(creature);
        Some(index)
    }

    /// The entry at `index`.
    pub fn get(&self, index: usize) -> Option<&Dialogue> {
        self.0.get(index)
    }

    /// How many entries the table holds, the first counted twice.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the table holds none.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The scripts' whole state, as a save keeps it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scripts {
    pub quests: Quests,
    pub variables: Variables,
    pub dialogues: Dialogues,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A runtime that records the functions it ran and answers `through` for each.
    fn runtime(ran: &mut Vec<String>, through: bool) -> impl FnMut(&str) -> bool + '_ {
        move |name| {
            ran.push(name.to_owned());
            through
        }
    }

    #[test]
    fn a_quest_triggered_before_it_is_set_up_does_nothing() {
        let mut quests = Quests::default();
        let mut ran = Vec::new();
        assert!(!quests.trigger(1, &mut runtime(&mut ran, true)));
        assert!(ran.is_empty());
        assert_eq!(quests.flags(1), 0);
        assert!(!quests.in_log(1));
    }

    #[test]
    fn a_quest_set_up_runs_its_set_up_and_its_trigger_but_does_not_enter() {
        let mut quests = Quests::default();
        let mut ran = Vec::new();
        quests.set_up(7, &mut runtime(&mut ran, true));
        assert_eq!(ran, ["QIS_OnSetUp7", "QIS_Trigger7"]);
        assert_eq!(quests.flags(7), SET_UP);
        // Set up once.
        quests.set_up(7, &mut runtime(&mut ran, true));
        assert_eq!(ran.len(), 2);
    }

    #[test]
    fn a_quest_set_up_and_triggered_enters_once() {
        let mut quests = Quests::default();
        let mut ran = Vec::new();
        quests.set_up(1, &mut runtime(&mut ran, true));
        ran.clear();
        assert!(quests.trigger(1, &mut runtime(&mut ran, true)));
        assert_eq!(ran, ["QIS_Trigger1", "QIS_OnEnter1"]);
        assert!(quests.in_log(1));
        assert_eq!(quests.flags(1), SET_UP | ENTERED);
        // A second trigger runs the trigger alone.
        ran.clear();
        assert!(!quests.trigger(1, &mut runtime(&mut ran, true)));
        assert_eq!(ran, ["QIS_Trigger1"]);
    }

    #[test]
    fn a_trigger_that_does_not_run_through_does_not_enter() {
        let mut quests = Quests::default();
        let mut ran = Vec::new();
        quests.set_up(3, &mut runtime(&mut ran, true));
        ran.clear();
        assert!(!quests.trigger(3, &mut runtime(&mut ran, false)));
        assert_eq!(ran, ["QIS_Trigger3"]);
        assert_eq!(quests.flags(3) & ENTERED, 0);
    }

    #[test]
    fn an_exit_after_a_loss_does_nothing_and_an_untrigger_takes_the_log_entry() {
        let mut quests = Quests::default();
        let mut ran = Vec::new();
        // Neither before the quest is entered.
        quests.set_up(2, &mut runtime(&mut ran, true));
        assert!(!quests.lose(2, &mut runtime(&mut ran, true)));
        quests.trigger(2, &mut runtime(&mut ran, true));
        ran.clear();
        assert!(quests.lose(2, &mut runtime(&mut ran, true)));
        assert_eq!(ran, ["QIS_OnLose2"]);
        assert!(!quests.exit(2, &mut runtime(&mut ran, true)));
        assert_eq!(ran.len(), 1);
        assert_eq!(quests.flags(2), SET_UP | ENTERED | FINISHED);
        quests.untrigger(2);
        assert!(!quests.in_log(2));
        assert_eq!(quests.flags(2), SET_UP | FINISHED);
    }

    #[test]
    fn variables_are_set_by_name_kept_to_31_bytes() {
        let mut variables = Variables::default();
        assert_eq!((variables.get("gold"), variables.value("gold")), (None, 0));
        variables.set("gold", 5);
        variables.set("gold", 7);
        assert_eq!(variables.value("gold"), 7);
        let long = "a".repeat(40);
        variables.set(&long, 1);
        assert_eq!(variables.get(&"a".repeat(31)), Some(1));
        assert_eq!(variables.get(&"a".repeat(35)), Some(1));
    }

    #[test]
    fn the_dialogues_table_doubles_its_first_entry() {
        let mut dialogues = Dialogues::default();
        for name in ["first", "second", "auftrag22"] {
            dialogues.add(name, -1);
        }
        assert_eq!(dialogues.len(), 4);
        // The n-th from 1 stands at the index n; the first name finds the index 0, which takes
        // the dialogue away.
        assert_eq!(dialogues.by_name("AUFTRAG22"), Some(3));
        assert_eq!(dialogues.stamp(dialogues.by_name("first"), 38), None);
        assert_eq!(dialogues.stamp(dialogues.by_name("auftrag22"), 38), Some(3));
        assert_eq!(dialogues.get(3).and_then(|entry| entry.creature), Some(38));
        // Every entry's number is −1, so a number of 0 finds none.
        assert_eq!(dialogues.stamp(dialogues.by_number(0), 38), None);
        assert_eq!(dialogues.stamp(dialogues.by_name("nowhere"), 38), None);
    }

    #[test]
    fn the_scripts_state_round_trips() {
        let mut scripts = Scripts::default();
        let mut ran = Vec::new();
        scripts.quests.set_up(1, &mut runtime(&mut ran, true));
        scripts.quests.trigger(1, &mut runtime(&mut ran, true));
        scripts.variables.set("quest1_done", 1);
        scripts.dialogues.add("first", -1);
        let bytes = postcard::to_allocvec(&scripts).expect("the state serializes");
        let back: Scripts = postcard::from_bytes(&bytes).expect("the state reads back");
        assert_eq!(back, scripts);
    }
}
