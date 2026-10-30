//! The scripts' state (#724, part of #720; docs/re/netscript.md, "Quests" and "`SetNPCState`"),
//! apart from Luau and from the engine: the quests' flags and the transitions of `SetUpQuest`,
//! `TriggerQuest`, `ExitQuest`, `LoseQuest` and `UnTriggerQuest`, the variables by name, the
//! dialogues' table that the start script's records of type 40 fill, the regions that have run
//! their `Region<n>Init`, each of a sector's area ([`region`]), the places that `DefPos` names,
//! the icons over the creatures of a dialogue or a trade ([`icon`]), the base triggers' boxes, which run their functions as the hero steps on them (#1002), the
//! timers, which run theirs as the game's minutes pass (#933), and the scripts' drops of the
//! creatures' types (#930). A transition names the
//! section functions it runs for its caller, the scripts' runtime, to run, and takes each one's
//! answer, whether its body ran to its end (`0x46BA90`); all of it serializes, for the saves.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::spawning::Fill;

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
    /// `ExitQuest` ([`Quests::exit`]).
    Exit,
    /// `LoseQuest` ([`Quests::lose`]).
    Lose,
}

/// The first side quest's number: below it, from 1, the main quests (`0x46C160`).
pub const SIDE_FROM: u32 = 100;

/// The radius of a compass that names none (`0x49A4B0`, `0x49AC80`: 1,200 world units).
pub const COMPASS_RADIUS: u32 = 1200;

/// Where a quest's compass points (the log entry's `+0x10` and `+0x14`, docs/re/interface.md,
/// "The compass"): a cell, or an object of the world by its name, which the game finds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    Cell(i32, i32),
    Named(String),
}

/// A quest's compass: its target and its radius (`+0x20`), within which the side quest's pin
/// fades out ([`side_alpha`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Compass {
    pub target: Target,
    pub radius: u32,
}

/// The compass's markers that the quests set (`sdk_MARKER_RESOLVE`, `0x4A5980`): the needle, the
/// newest main quest's, and the inner pin, the newest side quest's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marker {
    Main,
    Side,
}

/// The side quest pin's alpha at `distance` world units from its target of `radius`
/// (`0x4A5980`): whole beyond the radius, fading to nothing over its last 255 units, none
/// nearer.
pub fn side_alpha(distance: u32, radius: u32) -> u8 {
    if distance >= radius {
        return u8::MAX;
    }
    u8::try_from((distance + 255).saturating_sub(radius)).unwrap_or(u8::MAX)
}

/// The lines of a quest log entry (`+0x28`, ten).
pub const LINES: usize = 10;

/// How a quest log entry stands (`+4`): entered (2, `0x46C160`), won (100) or lost (101), as
/// `ExitQuest` or `LoseQuest` ends it (`0x46CBE0`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Standing {
    #[default]
    Entered,
    Won,
    Lost,
}

/// A quest log entry (`0x174` bytes at the script manager's `+0x424`), as the quest book shows
/// it (docs/re/questbook.md): its quest (`+8`), its title (`+0x24`) and its
/// lines (`+0x28`) as `QuestBook` writes them, the texts' ids
/// (`opensacred_data::text::sacred_hash`), its chapter (`+0x16C`), how it stands, and whether
/// it was entered since the game began (`+0xC` bit 0, which no save keeps).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub quest: u32,
    pub title: Option<u32>,
    pub lines: Vec<u32>,
    pub chapter: i32,
    pub standing: Standing,
    #[serde(skip)]
    pub new: bool,
}

impl Entry {
    /// Whether its quest is a main one (`0x46C160`: 1 to 99, the entry's kind 3; else 4).
    pub fn main(&self) -> bool {
        (1..SIDE_FROM).contains(&self.quest)
    }
}

/// The quests' flags by number, and the quests in the quest log, and their compasses.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quests {
    flags: BTreeMap<u32, u32>,
    log: BTreeSet<u32>,
    /// The log's entries in the order the quests entered it, by which an entry has its index
    /// (the vector at the script manager's `+0x424`).
    #[serde(default)]
    book: Vec<Entry>,
    /// The compasses that `QuestKompassPos` and `QuestKompassObj` set, by the quest whose log
    /// entry holds them.
    #[serde(default)]
    compasses: BTreeMap<u32, Compass>,
    /// A compass set for a quest not in the log, which the next quest entered takes
    /// (`0xAB782C`).
    #[serde(default)]
    pending: Option<Compass>,
    /// The newest main quest and side quest entered, whose compasses the markers show (the
    /// quest manager's `+0x3A4` and `+0x3A8` by the player).
    #[serde(default)]
    newest: [Option<u32>; 2],
    /// Whether a marker was set anew since [`Quests::take_fresh`]: the taskbar's message `0x34`,
    /// by which the bar starts a sonar.
    #[serde(default)]
    fresh: bool,
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
    /// (`0x46C160`): a quest log entry, the flag 4 and its `OnEnter`. `ExitQuest` and `LoseQuest`
    /// run the `OnExit` or the `OnLose` of a quest entered and not finished, then set the flag 2
    /// (`0x46CBE0`).
    pub fn step(&mut self, transition: Transition, quest: u32, ran: &[bool]) -> Option<Section> {
        let flags = self.flags(quest);
        let set_up = flags & SET_UP != 0;
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
                // The entry (`0x46C160`) ends a compass waiting, whether it enters or not.
                if flags & ENTERED != 0 {
                    self.pending = None;
                    return None;
                }
                // The entry takes the chapter of the newest main quest's.
                if self.log.insert(quest) {
                    let chapter = self.newest[0]
                        .and_then(|main| self.book.iter().find(|entry| entry.quest == main))
                        .map_or(0, |entry| entry.chapter);
                    self.book.push(Entry {
                        quest,
                        chapter,
                        new: true,
                        ..Entry::default()
                    });
                }
                *self.flags.entry(quest).or_default() |= ENTERED;
                // The newest main or side quest, and the compass that waited for it, a main
                // quest's a new marker.
                let main = quest < SIDE_FROM;
                self.newest[usize::from(!main)] = Some(quest);
                if let Some(compass) = self.pending.take() {
                    self.compasses.insert(quest, compass);
                    self.fresh |= main;
                }
                Some(Section::OnEnter)
            }
            (Transition::Trigger, [true, _]) => {
                self.pending = None;
                None
            }
            (Transition::Trigger, _) => None,
            (Transition::Exit | Transition::Lose, []) => {
                if flags & ENTERED == 0 || flags & FINISHED != 0 {
                    return None;
                }
                Some(if transition == Transition::Exit {
                    Section::OnExit
                } else {
                    Section::OnLose
                })
            }
            (Transition::Exit | Transition::Lose, _) => {
                *self.flags.entry(quest).or_default() |= FINISHED;
                if let Some(entry) = self.book.iter_mut().find(|entry| entry.quest == quest) {
                    entry.standing = if transition == Transition::Exit {
                        Standing::Won
                    } else {
                        Standing::Lost
                    };
                }
                None
            }
        }
    }

    /// `ExitQuest` (15, `0x48EE20`, `0x46CBE0`): for a quest entered and not finished, its
    /// `OnExit` and the flag 2; whether it did.
    pub fn exit(&mut self, quest: u32, run: &mut impl FnMut(&str) -> bool) -> bool {
        !self.steps(Transition::Exit, quest, run).is_empty()
    }

    /// `LoseQuest` (54): as `ExitQuest`, its `OnLose`; whether it did.
    pub fn lose(&mut self, quest: u32, run: &mut impl FnMut(&str) -> bool) -> bool {
        !self.steps(Transition::Lose, quest, run).is_empty()
    }

    /// `UnTriggerQuest` (77, `0x48E600`): the flag 4 off and the quest log's entry out.
    pub fn untrigger(&mut self, quest: u32) {
        if let Some(flags) = self.flags.get_mut(&quest) {
            *flags &= !ENTERED;
        }
        self.log.remove(&quest);
        self.book.retain(|entry| entry.quest != quest);
        self.compasses.remove(&quest);
    }

    /// `QuestKompassPos` (63, `0x49A4B0`) and `QuestKompassObj` (64, `0x49AC80`): the compass of
    /// the quest `quest`'s log entry; for a quest not in the log, or none, the compass waits for
    /// the next quest entered. A main quest's compass, or the newest side quest's, that changed
    /// sets a new marker (the taskbar's message `0x34`).
    pub fn set_compass(&mut self, quest: Option<u32>, compass: Compass) {
        let Some(quest) = quest.filter(|quest| self.log.contains(quest)) else {
            self.pending = Some(compass);
            return;
        };
        let shown = quest < SIDE_FROM || self.newest.contains(&Some(quest));
        let changed = self.compasses.get(&quest).map(|old| &old.target) != Some(&compass.target);
        self.fresh |= shown && changed;
        self.compasses.insert(quest, compass);
    }

    /// The compass that the marker `marker` shows (`sdk_MARKER_RESOLVE`, `0x4A5980`): the newest
    /// main quest's, or the newest side quest's but those of 9000 to 9499; none for a quest out of
    /// the log or finished (**hypothesis**: the entry's `+0x4` from 100), or without a compass.
    pub fn marker(&self, marker: Marker) -> Option<&Compass> {
        let quest = match marker {
            Marker::Main => self.newest[0],
            Marker::Side => self.newest[1].filter(|quest| !(9000..9500).contains(quest)),
        }?;
        (self.log.contains(&quest) && self.flags(quest) & FINISHED == 0)
            .then(|| self.compasses.get(&quest))
            .flatten()
    }

    /// The quest of the marker `marker` with its compass, as [`Quests::marker`] gives it.
    pub fn marked(&self, marker: Marker) -> Option<(u32, &Compass)> {
        let quest = match marker {
            Marker::Main => self.newest[0],
            Marker::Side => self.newest[1],
        }?;
        self.marker(marker).map(|compass| (quest, compass))
    }

    /// The index of the quest `quest`'s entry in the log, the order the quests entered it.
    pub fn entry(&self, quest: u32) -> Option<usize> {
        self.book.iter().position(|entry| entry.quest == quest)
    }

    /// The log's entries in the order the quests entered it.
    pub fn book(&self) -> &[Entry] {
        &self.book
    }

    /// The compass of the quest `quest`'s entry.
    pub fn compass(&self, quest: u32) -> Option<&Compass> {
        self.compasses.get(&quest)
    }

    /// `QuestBook` (53, `0x496080`): the text `text` into the quest `quest`'s entry, its title
    /// for a `line` divisible by 10, written over the one before, else its first line of ten
    /// still free; nothing for no text, a line below 0, a quest out of the log, or ten lines
    /// taken.
    pub fn write(&mut self, quest: u32, line: i32, text: u32) {
        let Some(entry) = self.book.iter_mut().find(|entry| entry.quest == quest) else {
            return;
        };
        if text == 0 || line < 0 {
            return;
        }
        if line % 10 == 0 {
            entry.title = Some(text);
        } else if entry.lines.len() < LINES {
            entry.lines.push(text);
        }
    }

    /// `SetQuestInfo` (87, `0x4A6EA0`): the chapter of the quest `quest`'s entry, the quest of
    /// the function that runs.
    pub fn set_chapter(&mut self, quest: u32, chapter: i32) {
        if let Some(entry) = self.book.iter_mut().find(|entry| entry.quest == quest) {
            entry.chapter = chapter;
        }
    }

    /// The side quest `quest` made the one the inner pin follows, as a click on its row of the
    /// quest book makes it (`0x49DAB0`, the manager's `+0x3A8`): one in the log, not ended;
    /// whether it is.
    pub fn track(&mut self, quest: u32) -> bool {
        let tracks = self.book.iter().any(|entry| {
            entry.quest == quest && !entry.main() && entry.standing == Standing::Entered
        });
        if tracks {
            self.newest[1] = Some(quest);
        }
        tracks
    }

    /// The side quest that the inner pin follows, the newest entered or the one tracked.
    pub fn tracked(&self) -> Option<u32> {
        self.newest[1]
    }

    /// Whether a marker was set anew since the last call, for the bar's sonar.
    pub fn take_fresh(&mut self) -> bool {
        std::mem::take(&mut self.fresh)
    }
}

/// The longest name of a variable, its record's `char name[32]` with its end (`this+0x7550`).
pub const VARIABLE_NAME: usize = 31;

/// The scripts' variables by name (`this+0x7550`, records of `{char name[32]; i32 value}`,
/// docs/re/netscript.md, "Variable references"), a name kept to its first 31 bytes and found
/// without regard to case (`0x859690`, #1054).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variables(BTreeMap<String, i32>);

impl Variables {
    /// The name as its record keeps it, lower-cased, as it is found case aside.
    fn key(name: &str) -> String {
        let mut end = name.len().min(VARIABLE_NAME);
        while !name.is_char_boundary(end) {
            end -= 1;
        }
        name[..end].to_ascii_lowercase()
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

/// The trades of a creature (`+0x200`, docs/re/ai.md, "The roll"): a smith (`CreateNPC`'s tag
/// `0x23`), a merchant (`0x24`), a wizard (`0x25`) and the trader of the tag `0x64`, whose talk
/// the tag stamps [`TRADER_TALK`] (`0x4843AF`, `SetNPCState`'s `0x462487`).
pub const SMITH: u32 = 0x1000;
pub const MERCHANT: u32 = 0x2000;
pub const WIZARD: u32 = 0x4000;
pub const TRADER: u32 = 0x400_0000;
pub const TRADER_TALK: u32 = 180_000;

/// The icon over a creature of a talk `talk` (`+0x245`, a dialogue's index or a stamp), of the
/// trades `trades`, whose dialogue's entry holds the icon `entry` (`+0x48`, the type-40 body's
/// second value, as `SetIcon` changes it), `0x499E90`: 4 for a stamp of 160,000 or more (the
/// trader of the tag `0x64`, the scripts' hires), else a trade's own (`0x20` to `0x23`), else the
/// entry's, else 0.
pub fn icon(talk: u32, trades: u32, entry: Option<i32>) -> i32 {
    if talk >= 160_000 {
        return 4;
    }
    let traded = [
        (SMITH, 0x20),
        (MERCHANT, 0x21),
        (WIZARD, 0x22),
        (TRADER, 0x23),
    ];
    match traded.iter().find(|(bit, _)| trades & bit != 0) {
        Some(&(_, icon)) => icon,
        None => entry.unwrap_or(0),
    }
}

/// The region of each area, the table of 69 signed bytes at `0x95CBFC`, constant data of the
/// client: the areas 0 to 49 by the list, 50 to 68 the regions 51 to 69 (docs/re/sound.md, "The
/// region's music").
const REGIONS: [u8; 69] = [
    0, 1, 2, 3, 4, 7, 10, 9, 8, 111, 6, 5, 0, 25, 26, 24, 24, 24, 24, 0, 15, 12, 13, 0, 110, 0, 0,
    25, 0, 0, 13, 17, 16, 18, 19, 0, 19, 20, 21, 23, 0, 0, 22, 22, 25, 27, 14, 28, 20, 26, 51, 52,
    53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69,
];

/// The region of a sector of the area `area` (the world record's `+0x2C0`, `world.ron`'s `area`),
/// as `0x602250` turns one into the other; 0, no region, past the table, which the client reads
/// on without a bound (no area of the world is past it).
pub fn region(area: u8) -> u16 {
    REGIONS.get(usize::from(area)).copied().map_or(0, u16::from)
}

/// The regions whose `Region<n>Init` has run (the interpreter's tree at `+0x350`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Regions(BTreeSet<u16>);

impl Regions {
    /// Whether the `Init` of the region `region`, which the tree holds, runs as a sector of the
    /// region runs its own `Init` (`0x492880`, which the sector's runner `0x493680` calls before
    /// it): the first time alone, the region marked run then. In a game of one player a region
    /// runs its `Init` once, whichever of its areas' sectors comes first; a region whose function
    /// the tree lacks is never marked.
    pub fn init(&mut self, region: u16) -> bool {
        self.0.insert(region)
    }

    /// Whether the region `region` has run its `Init`.
    pub fn ran(&self, region: u16) -> bool {
        self.0.contains(&region)
    }
}

/// The entries of the areas' table (`0x61D450`, 256 of 24 bytes) that stand pacified, `+2` above
/// `+3` (`0x61DB10`): those that the scripts' `RegionFrei` (124, `0x475C70`) set to 100
/// (`0x61D540(N, 1, 100.0, 1)`), by their number, which the hero's remark at an area's border
/// looks up by the sector's area (docs/re/sound.md, "The hero's comments"; **hypothesis**: `+3`,
/// which nothing is found to write, is 0).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pacified(BTreeSet<u8>);

impl Pacified {
    /// `RegionFrei`'s number `n`, held to 0 to 255, pacified: the entry's number.
    pub fn pacify(&mut self, n: i32) -> u8 {
        let entry = u8::try_from(n.clamp(0, 255)).unwrap_or(u8::MAX);
        self.0.insert(entry);
        entry
    }

    /// Whether the entry `entry` stands pacified.
    pub fn holds(&self, entry: u8) -> bool {
        self.0.contains(&entry)
    }
}

/// The places that the scripts name (`DefPos`, 23, `0x478780`): a name and up to four numbers,
/// the first two a global cell, the third the layer where a record takes the place as a cell
/// (`SetBaseTrigger`, `Teleport`); a name defined again replaces its entry. The names are kept
/// lower-cased, as `opensacred-game` looks them up.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Positions(BTreeMap<String, Vec<i32>>);

impl Positions {
    /// `DefPos`: the place `name` at the first four of `numbers`.
    pub fn define(&mut self, name: &str, numbers: &[i32]) {
        let numbers = numbers.iter().take(4).copied().collect();
        self.0.insert(name.to_lowercase(), numbers);
    }

    /// The numbers of the place `name`, whatever its case.
    pub fn get(&self, name: &str) -> Option<&[i32]> {
        self.0.get(&name.to_lowercase()).map(Vec::as_slice)
    }
}

/// The cells of a sector's side, by which the client splits a base trigger's box (`>> 6`).
const SIDE: i32 = 64;

/// A part of a base trigger's box within one sector (`0x622E80`: an entry of 0x4C bytes of the
/// sector's list at `+0x204`): its name, its layer, and its corners, the lower first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerBox {
    pub name: String,
    pub layer: u8,
    pub from: (i32, i32),
    pub to: (i32, i32),
}

/// The layer as a base trigger's lookup takes it: the layers 0 and 1, the ground and a
/// building's ground floor, alike (`0x6230C0`).
fn trigger_layer(layer: u8) -> u8 {
    if layer < 2 { 0 } else { layer }
}

/// The base triggers (`SetBaseTrigger`, 4, `0x4819A0`), each sector's parts of their boxes in the
/// order they were set (`0x63C970`, `0x622E80`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Triggers(BTreeMap<(i32, i32), Vec<TriggerBox>>);

impl Triggers {
    /// `SetBaseTrigger`: the trigger `name` on the box from the corner `first`, a global cell and
    /// a layer, to the cell `second`, one cell where it is missing; the corners ordered, the
    /// layer held to 0..16, and the box split at the sectors' edges, each sector's part appended
    /// to its list.
    pub fn set(&mut self, name: &str, first: (i32, i32, i32), second: Option<(i32, i32)>) {
        let (x0, y0, layer) = first;
        let (x1, y1) = second.unwrap_or((x0, y0));
        let layer = u8::try_from(layer).ok().filter(|&l| l <= 16).unwrap_or(0);
        let (left, right) = (x0.min(x1), x0.max(x1));
        let (top, bottom) = (y0.min(y1), y0.max(y1));
        for sx in left.div_euclid(SIDE)..=right.div_euclid(SIDE) {
            for sy in top.div_euclid(SIDE)..=bottom.div_euclid(SIDE) {
                let from = (left.max(sx * SIDE), top.max(sy * SIDE));
                let to = (
                    right.min(sx * SIDE + SIDE - 1),
                    bottom.min(sy * SIDE + SIDE - 1),
                );
                self.0.entry((sx, sy)).or_default().push(TriggerBox {
                    name: name.to_owned(),
                    layer,
                    from,
                    to,
                });
            }
        }
    }

    /// `DelBaseTrigger` (5, `0x4817F0`, `0x63CB10`): the boxes of the trigger `name`, whatever its
    /// case, taken out of every sector, or with a corner `about` (the lower of its record's two)
    /// out of that corner's sector and its eight neighbours; each taken out as the client takes
    /// it, the sector's last box moved into its place (`0x622FA0`).
    pub fn delete(&mut self, name: &str, about: Option<(i32, i32)>) {
        let sectors: Vec<(i32, i32)> = match about {
            None => self.0.keys().copied().collect(),
            Some((x, y)) => {
                let (sx, sy) = (x.div_euclid(SIDE), y.div_euclid(SIDE));
                (sx - 1..=sx + 1)
                    .flat_map(|x| (sy - 1..=sy + 1).map(move |y| (x, y)))
                    .collect()
            }
        };
        for sector in sectors {
            if let Some(boxes) = self.0.get_mut(&sector) {
                while let Some(at) = boxes.iter().position(|b| b.name.eq_ignore_ascii_case(name)) {
                    boxes.swap_remove(at);
                }
            }
        }
    }

    /// The trigger of the global cell `cell` on the layer `layer`: the first of its sector's
    /// boxes that holds it on its layer, the layers 0 and 1 alike (`0x6230C0`).
    pub fn at(&self, (x, y): (i32, i32), layer: u8) -> Option<&str> {
        let layer = trigger_layer(layer);
        self.0
            .get(&(x.div_euclid(SIDE), y.div_euclid(SIDE)))?
            .iter()
            .find(|b| {
                trigger_layer(b.layer) == layer
                    && (b.from.0..=b.to.0).contains(&x)
                    && (b.from.1..=b.to.1).contains(&y)
            })
            .map(|b| b.name.as_str())
    }
}

/// The quest and the trigger of a base trigger's function (`OMO<quest><name>`, the compiler's
/// `OMO%d%s` at `0x45C192`): the quest −1 for none.
pub fn trigger_function(name: &str) -> Option<(i32, &str)> {
    let rest = name.strip_prefix("OMO")?;
    let digits = rest.strip_prefix('-').unwrap_or(rest);
    let end = digits
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(digits.len());
    if end == 0 {
        return None;
    }
    let split = rest.len() - digits.len() + end;
    let quest = rest[..split].parse().ok()?;
    Some((quest, &rest[split..]))
}

/// A scripts' timer (`SetTimer`, 56, `0x4982D0`; an entry of 0x4C bytes of the calendar's list):
/// its id, the function it runs, its time in the game's minutes, and the minute it was set, of
/// the game's minutes elapsed since the new game (docs/decisions/netscript.md, "Scripts' timers in
/// elapsed minutes").
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timer {
    pub id: i32,
    pub function: String,
    pub time: i32,
    pub set: u64,
}

/// The scripts' timers in their order, and the game's minutes elapsed since the new game, which
/// they count by (the client counts by its calendar, whose months drift, #933).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timers {
    list: Vec<Timer>,
    minutes: u64,
}

impl Timers {
    /// `SetTimer`: the timer `id` (0 or more) runs `function` (a name) after `time` minutes
    /// (above 0), counted from now; a timer of that id already set takes the new function, time
    /// and moment in its place, which starts it anew (`0x417710`), and the others are appended.
    pub fn set(&mut self, id: i32, function: &str, time: i32) {
        if id < 0 || time <= 0 || function.is_empty() {
            return;
        }
        let timer = Timer {
            id,
            function: function.to_owned(),
            time,
            set: self.minutes,
        };
        match self.list.iter_mut().find(|t| t.id == id) {
            Some(old) => *old = timer,
            None => self.list.push(timer),
        }
    }

    /// `DelTimer` (57, `0x4985E0`, `0x4177E0`): the first timer of the id `id` taken out, the
    /// list's last moved into its place.
    pub fn delete(&mut self, id: i32) {
        if let Some(at) = self.list.iter().position(|t| t.id == id) {
            self.list.swap_remove(at);
        }
    }

    /// A minute of the game's clock completed.
    pub fn tick(&mut self) {
        self.minutes += 1;
    }

    /// The game's minutes elapsed since the new game.
    pub fn minutes(&self) -> u64 {
        self.minutes
    }

    /// The first timer of the list whose time has passed since it was set (`0x416F00`), which
    /// runs its function; [`Self::ran`] takes it out after.
    pub fn due(&self) -> Option<&Timer> {
        self.list.iter().find(|t| {
            u64::try_from(t.time).is_ok_and(|time| t.set.saturating_add(time) <= self.minutes)
        })
    }

    /// Every timer of the id `id` taken out, as one of it has run, each as the list's last
    /// moved into its place (`0x4171B0`).
    pub fn ran(&mut self, id: i32) {
        while let Some(at) = self.list.iter().position(|t| t.id == id) {
            self.list.swap_remove(at);
        }
    }

    /// The timers in their order.
    pub fn list(&self) -> &[Timer] {
        &self.list
    }
}

/// A scripts' drop (`SetDrop`): the creature type that drops the item, and its chance in per
/// cent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptDrop {
    pub creature: u32,
    pub item: u32,
    pub chance: u32,
}

/// The scripts' drops (`SetDrop`, 137, `0x4AFFF0`; docs/re/items.md, "The scripts' drops"), kept
/// by the creature type and the item in the order they were set, where the client keeps one
/// entry per creature type (docs/decisions/netscript.md, "Scripts' drops per creature and item",
/// #930).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Drops(Vec<ScriptDrop>);

impl Drops {
    /// `SetDrop`: the creature type `creature` drops the item `item` at `chance` per cent; a pair
    /// set again takes its new chance in its place, and a chance of 0 takes the pair out.
    pub fn set(&mut self, creature: u32, item: u32, chance: u32) {
        let at = self
            .0
            .iter()
            .position(|d| d.creature == creature && d.item == item);
        match (at, chance) {
            (Some(at), 0) => {
                self.0.remove(at);
            }
            (Some(at), chance) => self.0[at].chance = chance,
            (None, 0) => {}
            (None, chance) => self.0.push(ScriptDrop {
                creature,
                item,
                chance,
            }),
        }
    }

    /// The items that the creature type `creature` drops and their chances, in their order.
    pub fn of(&self, creature: u32) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.0
            .iter()
            .filter(move |d| d.creature == creature)
            .map(|d| (d.item, d.chance))
    }
}

/// The prefixes of the names in the script manager's list (`+0x358`) that are no creature's death
/// function: an item's taking and use, and a sight (`0x4A71F0`, `0x94E1CC`, `0x94E1C4`,
/// `0x94E1D4`).
const NOT_DEATHS: [&str; 3] = ["Take:", "Use:", SIGHT];
/// The prefix of a sight function's entry, which `CreateNPC`'s, `SetNPCState`'s and
/// `SetGroupState`'s `0x60` keep (`0x94E1D4`, #1025).
const SIGHT: &str = "See:";
/// The prefix of the names that a death or a `SetNPCState` leaves in the list (`0x94E1BC`).
const KEPT: &str = "wegw";

/// A creature's functions in the script manager's list (`+0x358`, entries of its id and a name
/// of 64 bytes; docs/re/netscript.md, "The deaths' functions"), in their order: those of its
/// `CreateNPC`'s and its `SetNPCState`s' tags `0x05`, of which its death runs one, and its sight
/// functions of their `0x60`, which it does not.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeathFunctions(Vec<String>);

impl DeathFunctions {
    /// `CreateNPC`'s `0x05`: the death's function `name` added (`0x4B9740`).
    pub fn add(&mut self, name: &str) {
        self.0.push(name.to_owned());
    }

    /// `CreateNPC`'s and `SetNPCState`'s `0x60`: the sight function `name` added, its entry's
    /// name prefixed `See:` (`0x94E1D4`), so that no death runs it (#1025).
    pub fn see(&mut self, name: &str) {
        self.0.push(format!("{SIGHT}{name}"));
    }

    /// `SetNPCState`'s `0x05`: the entries that a death takes out taken out first
    /// ([`Self::take_out`], `0x464330(id, 0, 1)` at `0x461FBC`), then `name` added.
    pub fn set(&mut self, name: &str) {
        self.take_out();
        self.add(name);
    }

    /// The function that the creature's death runs, taken out with the others of its kind: the
    /// first entry not of an item's prefixes (`0x4A71F0` with an empty name), then each entry of
    /// no `:` or of `See:` but those of [`KEPT`] (`0x464330(id, 1, 1)`); none where the list holds
    /// no such entry (`0x53E915`, `0x52F769`).
    pub fn take(&mut self) -> Option<String> {
        let first = self
            .0
            .iter()
            .find(|name| !NOT_DEATHS.iter().any(|prefix| name.starts_with(prefix)))
            .cloned()?;
        self.take_out();
        Some(first)
    }

    /// Takes out the entries of no `:` or of `See:`, but those of [`KEPT`] (`0x464330` with its
    /// fourth argument 1).
    fn take_out(&mut self) {
        self.0.retain(|name| {
            let taken = !name.contains(':') || name.starts_with(SIGHT);
            !taken || name.starts_with(KEPT)
        });
    }

    /// Whether it holds no function.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A count of the scripts (`SetOnKill`, `SetOnCollect`; entries of 76 bytes): a creature's or an
/// item's type, the count left and the count asked, and the function that runs once it is done.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Count {
    pub kind: u32,
    pub left: u32,
    pub asked: u32,
    pub function: String,
}

/// What a count does as a type is counted ([`Counts::count`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Counted {
    /// Its count goes on: so many counted of so many asked (the engine's event `0xB` with the
    /// line of the type's name and the counts, not followed).
    Goes { counted: u32, asked: u32 },
    /// It is done: its function runs, the entry taken out.
    Done(String),
}

/// The scripts' counts by type (`SetOnKill`, 135, `0x4AF190`, at `+0x7614`; `SetOnCollect`, 136,
/// `0x4AF8C0`, at `+0x7620`; docs/re/netscript.md, "`SetOnKill` and `SetOnCollect`"), in the order
/// set.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts(Vec<Count>);

impl Counts {
    /// A record's count of `count` for the type `kind`, with the function `function` where it
    /// names one: a type counted already starts again at the new count, and takes the new
    /// function if any; a count of 0 takes it out; a new type is kept only with a function.
    pub fn set(&mut self, kind: u32, count: u32, function: Option<&str>) {
        let at = self.0.iter().position(|c| c.kind == kind);
        match (at, count, function) {
            (Some(at), 0, _) => {
                self.0.remove(at);
            }
            (Some(at), count, function) => {
                let entry = &mut self.0[at];
                entry.left = count;
                entry.asked = count;
                if let Some(function) = function {
                    function.clone_into(&mut entry.function);
                }
            }
            (None, 0, _) | (None, _, None) => {}
            (None, count, Some(function)) => self.0.push(Count {
                kind,
                left: count,
                asked: count,
                function: function.to_owned(),
            }),
        }
    }

    /// Counts one of the type `kind` on the first entry of that type (`0x4AA630`, `0x4AAAE0`);
    /// none where no entry counts it.
    pub fn count(&mut self, kind: u32) -> Option<Counted> {
        let at = self.0.iter().position(|c| c.kind == kind)?;
        let entry = &mut self.0[at];
        entry.left = entry.left.saturating_sub(1);
        if entry.left > 0 {
            return Some(Counted::Goes {
                counted: entry.asked - entry.left,
                asked: entry.asked,
            });
        }
        Some(Counted::Done(self.0.remove(at).function))
    }

    /// Gives each entry whose type `item` maps its item's type in its place: `SetOnCollect`'s
    /// entries of a creature's type count the item of its `SetDrop` (`0x4AAAE0`).
    pub fn retype(&mut self, item: impl Fn(u32) -> Option<u32>) {
        for entry in &mut self.0 {
            if let Some(item) = item(entry.kind) {
                entry.kind = item;
            }
        }
    }
}

/// An entry of `ReplaceSpawn` (96, `0x4A79D0`; docs/re/netscript.md, "`ReplaceSpawn` and
/// `RegionChange`"): the fills of an area that it changes, its pairs of types, each a type
/// replaced and its replacement, and the side it gives the runs it changes, hostile with `0x08`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Replacement {
    pub area: u8,
    pub pairs: Vec<(u32, u32)>,
    pub hostile: bool,
}

/// The entries of the scripts' `ReplaceSpawn`s, in order (the interpreter's vector `+0x394`),
/// which the fills of the sectors read as they come in (#55). Nothing takes one out.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Replacements(Vec<Replacement>);

impl Replacements {
    /// Adds `entry`, as `0x4A79D0` does: every entry of its area and its types first takes its
    /// place, and it is appended all the same.
    pub fn add(&mut self, entry: Replacement) {
        let types = |entry: &Replacement| {
            entry
                .pairs
                .iter()
                .map(|&(from, _)| from)
                .collect::<Vec<_>>()
        };
        let replaced = types(&entry);
        for kept in &mut self.0 {
            if kept.area == entry.area && types(kept) == replaced {
                *kept = entry.clone();
            }
        }
        self.0.push(entry);
    }

    /// The fill `fill` of a sector of the area `area` as the entries change it, in their order
    /// (`0x47C610`, while an `Enter` runs): each line of a type that an entry of the area replaces
    /// takes the new type, its weight and hostility kept, and a run of such a line takes the
    /// entry's side for all its lines (**hypothesis**: only a run where a line was replaced).
    pub fn fill(&self, area: u8, fill: &Fill) -> Fill {
        let mut fill = fill.clone();
        for entry in self.0.iter().filter(|entry| entry.area == area) {
            let mut changed = false;
            for line in &mut fill.entries {
                if let Some(&(_, to)) = entry.pairs.iter().find(|&&(from, _)| from == line.creature)
                {
                    line.creature = to;
                    changed = true;
                }
            }
            if changed {
                for line in &mut fill.entries {
                    line.hostile = entry.hostile;
                }
            }
        }
        fill
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The areas whose lists `RegionChange` keeps (`+0x1048`, 50 lists of `0xAC` bytes): 0 to 49.
pub const AREAS: u8 = 50;

/// A pair of a `RegionChange`'s list (`0x4ABFE0`): the type turned, its new type, and the low
/// half of its record's flags word, which the list keeps in the new type's high half.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Turn {
    pub from: u32,
    pub to: u32,
    pub flags: u16,
}

impl Turn {
    /// The type and the flags word of a `CreateNPC` of the flags word `word` that the pair turns
    /// (`0x482F41`): its new type, and where the pair's flags are not 0, the word's low half
    /// replaced by them, their top bit spread over the high half (`SAR 0x10`).
    pub fn of(self, word: u32) -> (u32, u32) {
        let word = match self.flags {
            0 => word,
            flags => {
                let spread = i32::from(i16::from_ne_bytes(flags.to_ne_bytes()));
                word & 0xFFFF_0000 | u32::from_ne_bytes(spread.to_ne_bytes())
            }
        };
        (self.to, word)
    }
}

/// The lists of `RegionChange` (116, `0x4ABB60`; docs/re/netscript.md, "`ReplaceSpawn` and
/// `RegionChange`"), one by area (`+0x1048`), which a later `CreateNPC` of a cell of the area
/// reads where it carries `0x6B` of 0 (`0x482F41`, #1063, #1102). Only the interpreter's reset
/// empties them (`0x461040`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreaTypes(BTreeMap<u8, Vec<Turn>>);

impl AreaTypes {
    /// A `RegionChange` of the area `area`, its types `types` taken in pairs (an odd last one
    /// left) and its flags word `word` (`0x4ABFE0`): while a function runs, its record replaces
    /// the area's list; once a function that it called has ended, the runner's index of the
    /// running function at −1 (`0x94E07C`; `appends`), it adds its pairs to the list, past the
    /// list's 20 slots too, where the original writes on over the next area's list. An area
    /// above 49 keeps nothing.
    pub fn add(&mut self, area: u8, types: &[u32], word: u32, appends: bool) {
        if area >= AREAS {
            return;
        }
        let flags = u16::try_from(word & 0xFFFF).unwrap_or(0);
        let pairs = types.as_chunks::<2>().0.iter().map(|&[from, to]| Turn {
            from,
            to: to & 0xFFFF,
            flags,
        });
        let list = self.0.entry(area).or_default();
        if !appends {
            list.clear();
        }
        list.extend(pairs);
    }

    /// The pair of the area `area`'s list that turns a `CreateNPC` of the type `creature`, its
    /// first of that type (`0x482F41`), none where the list holds none.
    pub fn turn(&self, area: u8, creature: u32) -> Option<Turn> {
        self.0
            .get(&area)?
            .iter()
            .find(|turn| turn.from == creature)
            .copied()
    }

    /// The areas whose lists turn the type `creature`, each with its pair ([`Self::turn`]).
    pub fn turns(&self, creature: u32) -> impl Iterator<Item = (u8, Turn)> + '_ {
        self.0
            .keys()
            .filter_map(move |&area| Some((area, self.turn(area, creature)?)))
    }

    /// Whether no list holds a pair.
    pub fn is_empty(&self) -> bool {
        self.0.values().all(Vec::is_empty)
    }
}

/// The flags word of a `RegionChange`'s operand-less tags `tags`, in order (`0x4ABB60`): each
/// tag adds its bit, unlike `CreateNPC`'s modes, and `0x14` clears the low half.
pub fn region_change_word(tags: impl IntoIterator<Item = u8>) -> u32 {
    tags.into_iter().fold(0, |word, tag| match tag {
        0x12 => word | 0x1,
        0x13 => word | 0x2,
        0x14 => word & 0xFFFF_0000,
        0x2B => word | 0x8,
        0x2E => word | 0x10,
        0x30 => word | 0x20,
        0x2F | 0x31 => word | 0x40,
        0x32 => word | 0x80,
        0x42 => word | 0x400,
        0x43 => word | 0x800,
        0x44 => word | 0x1000,
        0x45 => word | 0x2000,
        0x46 => word | 0x4000,
        0x4C => word | 0x8000,
        0x50 => word | 0x1_0000,
        0x74 => word | 0x800_0000,
        0x85 => word | 0x200_0000,
        _ => word,
    })
}

/// A `RegionChange` (116, `0x4ABB60`) as its record reads: its area (`0x69`'s number, 0 without
/// one), its types (`0x02`), its flags word ([`region_change_word`]) and its side, hostile with
/// `0x08` and peaceful with `0x0E`, the last of them, else neither. It changes the creatures that
/// exist in its area (`0x4ABFE0`, #1064; docs/re/netscript.md, "The NPCs that later events
/// remove") and keeps its pairs for the later `CreateNPC`s ([`AreaTypes`], #1063).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AreaChange {
    pub area: u8,
    pub types: Vec<u32>,
    pub word: u32,
    pub hostile: Option<bool>,
}

impl AreaChange {
    /// The `RegionChange` of the operands `ops`, none where its area is not one of the 50
    /// ([`AREAS`]): the applier then does nothing.
    pub fn of(ops: &[opensacred_data::netscript::Operand]) -> Option<Self> {
        use opensacred_data::netscript::Value;
        let mut area = 0;
        let mut types = Vec::new();
        let mut marks = Vec::new();
        let mut hostile = None;
        for op in ops {
            match (op.tag, &op.value) {
                (0x02, Value::I32(creature)) => types.extend(u32::try_from(*creature).ok()),
                // The number of its text, as the C library reads one, 0 for none.
                (0x69, Value::Str(text) | Value::Name(text)) => {
                    area = text.trim().parse::<i32>().unwrap_or(0);
                }
                (0x69, Value::I32(number)) => area = *number,
                (tag, Value::Marker) => {
                    match tag {
                        0x08 => hostile = Some(true),
                        0x0E => hostile = Some(false),
                        _ => {}
                    }
                    marks.push(tag);
                }
                _ => {}
            }
        }
        let area = u8::try_from(area).ok().filter(|&area| area < AREAS)?;
        Some(Self {
            area,
            types,
            word: region_change_word(marks),
            hostile,
        })
    }

    /// Its pairs of types, an odd last one left.
    pub fn pairs(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.types
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&[from, to]| (from, to))
    }

    /// The type that a creature of the type `creature` takes (`0x4AC1A0` to `0x4AC2B5`): each
    /// pair in order whose new type is not 0 turns the type it has then, and how many turned it;
    /// none where none did.
    pub fn creature(&self, creature: u32) -> Option<(u32, usize)> {
        let (mut now, mut turned) = (creature, 0);
        for (from, to) in self.pairs() {
            if to & 0xFFFF != 0 && from == now {
                now = to & 0xFFFF;
                turned += 1;
            }
        }
        (turned > 0).then_some((now, turned))
    }

    /// The type that an item of the type `item` of a creature turned `times` times takes: at each
    /// turn, each pair in order turns the type it has then (`0x4AC2D0` to `0x4AC3CB`).
    pub fn item(&self, item: u32, times: usize) -> u32 {
        let mut now = item;
        for _ in 0..times {
            for (from, to) in self.pairs() {
                if now == from {
                    now = to & 0xFFFF;
                }
            }
        }
        now
    }

    /// The flags word that a turned creature of the word `word` takes: the record's, the old
    /// word's `0x2000` kept, where the record's is not 0.
    pub fn word(&self, word: u32) -> u32 {
        match self.word {
            0 => word,
            changed => word & 0x2000 | changed,
        }
    }

    /// The side that a turned creature of the side `side` and the new type `creature` takes
    /// (`sdk_STANCE`): a hostile record's makes one not hostile to the heroes hostile, 2, or 14
    /// of the side 7; a peaceful record's makes a hostile one peaceful, 3, or 7 where the
    /// record's word has the guards' mode `0x4000`; any other keeps its side.
    pub fn side(&self, side: u8, creature: u32) -> u8 {
        let against = crate::minds::hostile_to_heroes(side, creature);
        match self.hostile {
            Some(true) if !against => {
                if side == 7 {
                    14
                } else {
                    2
                }
            }
            Some(false) if against => {
                if self.word & 0x4000 != 0 {
                    7
                } else {
                    3
                }
            }
            _ => side,
        }
    }
}

/// The scripts' whole state, as a save keeps it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scripts {
    pub quests: Quests,
    pub variables: Variables,
    pub dialogues: Dialogues,
    pub regions: Regions,
    pub positions: Positions,
    pub triggers: Triggers,
    pub timers: Timers,
    pub drops: Drops,
    /// The fills' replacements of the `ReplaceSpawn`s (#55).
    pub replacements: Replacements,
    /// The lists of the `RegionChange`s, by area (#1063).
    pub area_types: AreaTypes,
    /// The kills counted (`SetOnKill`) and the items collected (`SetOnCollect`).
    pub kills: Counts,
    pub collecting: Counts,
    /// The areas' entries that the scripts' `RegionFrei`s pacified (#1247).
    #[serde(default)]
    pub pacified: Pacified,
    /// The local hero's list of the main quest's items, by which `0x3A` finds an item (#1257).
    #[serde(default)]
    pub items: QuestItems,
    /// The game's difficulty, 0 bronze to 4 niobium (`0x182EE44`), which the `IF`s' tags `0x96`
    /// to `0x9A` test (`0x4987B0`; docs/re/fight.md, "The difficulty"; #1125).
    #[serde(default)]
    pub difficulty: u8,
    /// The pools of places that `AddPoolPos` fills (#1518).
    #[serde(default)]
    pub pools: Pools,
    /// The hidden stashes that `MachVersteck` lays and the hero has not searched yet (#1518).
    #[serde(default)]
    pub stashes: Stashes,
}

/// The scripts' pools of places (`AddPoolPos`, 108; the interpreter's list at `+0x334`, entries
/// of 100 bytes of the kind 2; docs/re/containers.md, "The hidden stashes"): each pool's cells in
/// the order added.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pools(BTreeMap<String, Vec<(i32, i32)>>);

/// The most places of a pool that a pick weighs (`0x4B0C00`).
const MOST_CANDIDATES: usize = 100;

impl Pools {
    /// `AddPoolPos`: the cell `cell` into the pool `name`.
    pub fn add(&mut self, name: &str, cell: (i32, i32)) {
        self.0.entry(name.to_owned()).or_default().push(cell);
    }

    /// One of the pool `name`'s first 100 places at random (`rand() % n`), which leaves the pool;
    /// none of an empty pool.
    pub fn take(
        &mut self,
        name: &str,
        dice: &mut impl crate::spawning::Dice,
    ) -> Option<(i32, i32)> {
        let places = self.0.get_mut(name)?;
        let candidates = places.len().min(MOST_CANDIDATES);
        if candidates == 0 {
            return None;
        }
        let at = if candidates == 1 {
            0
        } else {
            usize::try_from(dice.below(u32::try_from(candidates).unwrap_or(1))).unwrap_or(0)
        };
        Some(places.remove(at))
    }

    /// The places left in the pool `name`.
    pub fn len(&self, name: &str) -> usize {
        self.0.get(name).map_or(0, Vec::len)
    }
}

/// A hidden stash (`MachVersteck`, a mouse event of `0x4AB160`): its cell, the function its
/// search runs, and the name of the text the cursor shows over it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stash {
    pub cell: (i32, i32),
    pub function: String,
    pub text: String,
}

/// The hidden stashes not searched yet, in the order laid.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stashes(pub Vec<Stash>);

impl Stashes {
    /// `MachVersteck` (140, `0x4B0C00`): `count` places taken at random from the pool `pool`, each
    /// a stash of the function `function` and the text `text` (a `res:` name's key), as many as
    /// the pool holds.
    pub fn lay(
        &mut self,
        pools: &mut Pools,
        (text, pool, function): (&str, &str, &str),
        count: u32,
        dice: &mut impl crate::spawning::Dice,
    ) {
        let text = text
            .strip_prefix("res:")
            .or_else(|| text.strip_prefix("RES:"))
            .unwrap_or(text);
        for _ in 0..count {
            let Some(cell) = pools.take(pool, dice) else {
                break;
            };
            self.0.push(Stash {
                cell,
                function: function.to_owned(),
                text: text.to_owned(),
            });
        }
    }

    /// The stash at `cell`, which its search takes away (the mouse event's function run once).
    pub fn search(&mut self, cell: (i32, i32)) -> Option<Stash> {
        let at = self.0.iter().position(|stash| stash.cell == cell)?;
        Some(self.0.remove(at))
    }
}

/// The local hero's list of the main quest's items (its inventory entry's, `0x5D8280`): pairs
/// of an item type and a key, the key of the object's script name, which the test `0x3A` asks
/// for (`0x4987B0`; docs/re/netscript.md, "The log's quest items"; #1257). An empty key stands
/// for the key 0.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QuestItems(Vec<(u32, String)>);

impl QuestItems {
    /// Adds `item` of the key `key` (`cInventoryEntry::addMainQuestItem`, `0x5D8280`): unless an
    /// entry has its type already, or any entry has the key 0: whether it did.
    pub fn add(&mut self, item: u32, key: &str) -> bool {
        if self
            .0
            .iter()
            .any(|(held, held_key)| *held == item || held_key.is_empty())
        {
            return false;
        }
        self.0.push((item, key.to_ascii_lowercase()));
        true
    }

    /// Whether the list holds the key `name`, case aside, or any entry of the key 0 (`0x4987B0`'s
    /// `0x3A` on the list, `0x4988B4`).
    pub fn holds(&self, name: &str) -> bool {
        let name = name.to_ascii_lowercase();
        self.0.iter().any(|(_, key)| *key == name || key.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_quest_items_list_refuses_a_type_twice_and_all_after_the_key_0() {
        let mut items = QuestItems::default();
        assert!(items.add(1741, "Res:17640"));
        assert!(items.holds("res:17640") && !items.holds("res:17630"));
        // A second item of a type held already is refused (quest 154's).
        assert!(!items.add(1741, "res:17630"));
        assert!(!items.holds("res:17630"));
        // An entry of the key 0 refuses every later one, and answers every name.
        assert!(items.add(4609, ""));
        assert!(!items.add(6029, "res:6029"));
        assert!(items.holds("res:17630"));
    }

    /// A runtime that records the functions it ran and answers `through` for each.
    fn runtime(ran: &mut Vec<String>, through: bool) -> impl FnMut(&str) -> bool + '_ {
        move |name| {
            ran.push(name.to_owned());
            through
        }
    }

    #[test]
    fn the_logs_entries_keep_the_order_the_quests_entered_it() {
        let mut quests = Quests::default();
        let mut ran = Vec::new();
        for quest in [150, 1, 2] {
            quests.set_up(quest, &mut runtime(&mut ran, true));
            assert!(quests.trigger(quest, &mut runtime(&mut ran, true)));
        }
        assert_eq!(
            [150, 1, 2, 3].map(|quest| quests.entry(quest)),
            [Some(0), Some(1), Some(2), None]
        );
        // The newest main quest's marker with its quest, once it has a compass.
        assert_eq!(quests.marked(Marker::Main), None);
        let compass = Compass {
            target: Target::Cell(3240, 2511),
            radius: 0,
        };
        quests.set_compass(Some(2), compass.clone());
        assert_eq!(quests.marked(Marker::Main), Some((2, &compass)));
    }

    /// The quest book's entries (#1560): `QuestBook`'s title and ten lines, `SetQuestInfo`'s
    /// chapter, which a new entry takes from the newest main quest's, how an entry stands, and
    /// the side quest tracked.
    #[test]
    fn the_logs_entries_take_their_titles_lines_and_chapters() {
        let mut quests = Quests::default();
        let mut ran = Vec::new();
        let enter = |quests: &mut Quests, quest: u32, ran: &mut Vec<String>| {
            quests.set_up(quest, &mut runtime(ran, true));
            quests.trigger(quest, &mut runtime(ran, true));
        };
        // Nothing for a quest out of the log.
        quests.write(1, 0, 11);
        enter(&mut quests, 1, &mut ran);
        assert_eq!(quests.book()[0].title, None);
        quests.write(1, 0, 11);
        quests.write(1, 10, 12);
        for text in 20..32 {
            quests.write(1, 1, text);
        }
        quests.write(1, -1, 40);
        quests.write(1, 1, 0);
        let entry = &quests.book()[0];
        assert_eq!(entry.title, Some(12));
        assert_eq!(entry.lines, (20..30).collect::<Vec<_>>());
        assert!(entry.main() && entry.new && entry.chapter == 0);
        // The chapter set, and taken by the next entry from the newest main quest.
        quests.set_chapter(1, 2);
        enter(&mut quests, 150, &mut ran);
        enter(&mut quests, 151, &mut ran);
        assert_eq!(
            quests
                .book()
                .iter()
                .map(|entry| entry.chapter)
                .collect::<Vec<_>>(),
            [2, 2, 2]
        );
        // The newest side quest tracked, another tracked by a click, not a main one or one
        // ended.
        assert_eq!(quests.tracked(), Some(151));
        assert!(quests.track(150) && quests.tracked() == Some(150));
        assert!(!quests.track(1));
        quests.exit(151, &mut runtime(&mut ran, true));
        quests.lose(1, &mut runtime(&mut ran, true));
        assert!(!quests.track(151));
        assert_eq!(
            quests
                .book()
                .iter()
                .map(|entry| entry.standing)
                .collect::<Vec<_>>(),
            [Standing::Lost, Standing::Entered, Standing::Won]
        );
        // An untrigger takes the entry out, the later ones moving up.
        quests.untrigger(1);
        assert_eq!(quests.entry(150), Some(0));
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

    /// The compasses (#1462): set on a quest in the log, or waiting for the next quest entered;
    /// the needle the newest main quest's, the inner pin the newest side quest's but 9000 to
    /// 9499's; a new marker for a main quest, or the newest side quest, whose compass changed.
    #[test]
    fn the_markers_show_the_newest_quests_compasses() {
        let mut quests = Quests::default();
        let mut ran = Vec::new();
        let enter = |quests: &mut Quests, quest: u32, ran: &mut Vec<String>| {
            quests.set_up(quest, &mut runtime(ran, true));
            quests.trigger(quest, &mut runtime(ran, true));
        };
        let cell = |x, y| Compass {
            target: Target::Cell(x, y),
            radius: COMPASS_RADIUS,
        };
        // A compass for a quest not in the log waits for the next one entered.
        quests.set_compass(Some(1), cell(10, 20));
        assert_eq!(quests.marker(Marker::Main), None);
        enter(&mut quests, 1, &mut ran);
        assert_eq!(quests.marker(Marker::Main), Some(&cell(10, 20)));
        assert!(quests.take_fresh());
        assert!(!quests.take_fresh());
        // The same target again is no new marker; another is.
        quests.set_compass(Some(1), cell(10, 20));
        assert!(!quests.take_fresh());
        quests.set_compass(Some(1), cell(11, 20));
        assert!(quests.take_fresh());
        // A side quest's compass is the pin's, and a new marker as it is the newest.
        enter(&mut quests, 120, &mut ran);
        quests.set_compass(Some(120), cell(5, 5));
        assert_eq!(quests.marker(Marker::Side), Some(&cell(5, 5)));
        assert!(quests.take_fresh());
        // An older side quest's changes no marker.
        enter(&mut quests, 130, &mut ran);
        quests.set_compass(Some(120), cell(6, 6));
        assert!(!quests.take_fresh());
        assert_eq!(quests.marker(Marker::Side), None);
        // 9000 to 9499 show no pin, and a finished quest no marker.
        enter(&mut quests, 9001, &mut ran);
        quests.set_compass(Some(9001), cell(7, 7));
        assert_eq!(quests.marker(Marker::Side), None);
        quests.exit(1, &mut runtime(&mut ran, true));
        assert_eq!(quests.marker(Marker::Main), None);
        // A compass that waited is gone with the next entry that enters nothing.
        quests.set_compass(None, cell(8, 8));
        quests.trigger(1, &mut runtime(&mut ran, true));
        enter(&mut quests, 2, &mut ran);
        assert_eq!(quests.marker(Marker::Main), None);
    }

    #[test]
    fn the_side_pin_fades_within_its_radius() {
        assert_eq!(side_alpha(5000, 1200), 255);
        assert_eq!(side_alpha(1200, 1200), 255);
        assert_eq!(side_alpha(1199, 1200), 254);
        assert_eq!(side_alpha(945, 1200), 0);
        assert_eq!(side_alpha(100, 1200), 0);
    }

    #[test]
    fn an_exit_runs_its_section_before_it_finishes_the_quest() {
        let mut quests = Quests::default();
        let mut ran = Vec::new();
        quests.set_up(4, &mut runtime(&mut ran, true));
        quests.trigger(4, &mut runtime(&mut ran, true));
        assert_eq!(quests.step(Transition::Exit, 4, &[]), Some(Section::OnExit));
        // Not yet finished while the section runs.
        assert_eq!(quests.flags(4) & FINISHED, 0);
        assert_eq!(quests.step(Transition::Exit, 4, &[false]), None);
        assert_eq!(quests.flags(4) & FINISHED, FINISHED);
        assert_eq!(quests.step(Transition::Lose, 4, &[]), None);
    }

    #[test]
    fn variables_are_set_by_name_kept_to_31_bytes() {
        let mut variables = Variables::default();
        assert_eq!((variables.get("gold"), variables.value("gold")), (None, 0));
        variables.set("gold", 5);
        variables.set("gold", 7);
        // One variable whatever the case of its name: the start's `Regionchange` and the
        // quests' `RegionChange`.
        variables.set("Regionchange", 0);
        variables.set("RegionChange", 128);
        assert_eq!(variables.get("REGIONCHANGE"), Some(128));
        assert_eq!(variables.value("gold"), 7);
        let long = "a".repeat(40);
        variables.set(&long, 1);
        assert_eq!(variables.get(&"a".repeat(31)), Some(1));
        assert_eq!(variables.get(&"a".repeat(35)), Some(1));
    }

    #[test]
    fn a_replace_spawn_swaps_the_types_of_its_areas_fills() {
        use crate::spawning::Entry;
        let line = |creature, hostile| Entry {
            creature,
            weight: 100,
            hostile,
        };
        let fill = Fill {
            per_area: (1, 2, 100),
            entries: vec![line(279, false), line(35, false)],
        };
        let mut replacements = Replacements::default();
        // `Init58`'s: the soldier 279 becomes the skeleton warrior 32, hostile, in area 1.
        replacements.add(Replacement {
            area: 1,
            pairs: vec![(279, 32)],
            hostile: true,
        });
        let changed = replacements.fill(1, &fill);
        assert_eq!(changed.entries, [line(32, true), line(35, true)]);
        // Another area's fill, and a fill of none of its types, stay.
        assert_eq!(replacements.fill(2, &fill), fill);
        let other = Fill {
            per_area: (1, 2, 100),
            entries: vec![line(35, false)],
        };
        assert_eq!(replacements.fill(1, &other), other);
        // A later entry swaps back; the same types again take the first's place and append.
        replacements.add(Replacement {
            area: 1,
            pairs: vec![(32, 279)],
            hostile: false,
        });
        assert_eq!(
            replacements.fill(1, &fill).entries,
            [line(279, false), line(35, false)]
        );
        replacements.add(Replacement {
            area: 1,
            pairs: vec![(279, 33)],
            hostile: true,
        });
        assert_eq!(replacements.0.len(), 3);
        assert_eq!(replacements.0[0].pairs, [(279, 33)]);
    }

    #[test]
    fn a_region_change_replaces_its_areas_list_within_a_function_and_adds_after_a_call() {
        let mut lists = AreaTypes::default();
        assert!(lists.is_empty());
        // `GoodToBad`'s two records of area 5, in one function: the second replaces the first.
        let hostile = region_change_word([0x08, 0x12]);
        assert_eq!(hostile, 0x1);
        lists.add(5, &[279, 276, 297, 296], hostile, false);
        lists.add(5, &[1211, 1209, 1210, 1209], 0, false);
        assert_eq!(lists.turn(5, 279), None);
        assert_eq!(
            lists.turn(5, 1210),
            Some(Turn {
                from: 1210,
                to: 1209,
                flags: 0
            })
        );
        // `WillbursTot45`'s, after its `call("GoodToBad")`, adds to it; an odd last type is left.
        lists.add(5, &[281, 274, 297, 296, 1208], 0, true);
        assert_eq!(lists.turn(5, 297).map(|turn| turn.to), Some(296));
        assert_eq!(lists.turn(5, 1208), None);
        assert_eq!(lists.turn(4, 297), None);
        // The first pair of a type counts, in every area that holds one.
        lists.add(4, &[297, 296, 297, 279], hostile, false);
        assert_eq!(lists.turn(4, 297).map(|turn| turn.to), Some(296));
        let areas: Vec<u8> = lists.turns(297).map(|(area, _)| area).collect();
        assert_eq!(areas, [4, 5]);
        // An area above 49 keeps nothing.
        lists.add(50, &[1, 2], 0, false);
        assert_eq!(lists.turn(50, 1), None);
    }

    #[test]
    fn a_region_change_turns_a_creature_its_items_its_word_and_its_side() {
        use opensacred_data::netscript::{Operand, Value};
        let op = |tag, value| Operand { tag, value };
        let marker = |tag| op(tag, Value::Marker);
        // `GoodToBad`'s record of area 4: hostile, mode 1; `BadToGood`'s, peaceful.
        let good_to_bad = AreaChange::of(&[
            op(0x02, Value::I32(279)),
            op(0x02, Value::I32(276)),
            op(0x02, Value::I32(1211)),
            op(0x02, Value::I32(1209)),
            marker(0x08),
            marker(0x12),
            op(0x69, Value::Str("4".into())),
        ])
        .expect("an area of the 50");
        assert_eq!(good_to_bad.area, 4);
        assert_eq!(good_to_bad.word, 0x1);
        assert_eq!(good_to_bad.hostile, Some(true));
        assert_eq!(good_to_bad.creature(279), Some((276, 1)));
        assert_eq!(good_to_bad.creature(286), None);
        assert_eq!(good_to_bad.item(1211, 1), 1209);
        assert_eq!(good_to_bad.item(1729, 1), 1729);
        // The word replaced but its `0x2000`; a record of no tags keeps it.
        assert_eq!(good_to_bad.word(0x4000 | 0x2000), 0x2001);
        let bare =
            AreaChange::of(&[op(0x02, Value::I32(1)), op(0x02, Value::I32(2))]).expect("area 0");
        assert_eq!((bare.area, bare.word, bare.hostile), (0, 0, None));
        assert_eq!(bare.word(0x4000), 0x4000);
        // A guard of side 7, a friend of the heroes, turns hostile as 14; another as 2.
        assert_eq!(good_to_bad.side(7, 276), 14);
        assert_eq!(good_to_bad.side(3, 276), 2);
        // A hostile record leaves a hostile one; a peaceful one turns it back, 7 in the guards'
        // mode.
        assert_eq!(good_to_bad.side(14, 276), 14);
        let bad_to_good = AreaChange {
            area: 4,
            types: vec![276, 279],
            word: region_change_word([0x0E, 0x46]),
            hostile: Some(false),
        };
        assert_eq!(bad_to_good.side(14, 279), 7);
        assert_eq!(bad_to_good.side(7, 279), 7);
        // Each pair in order turns the type it has then; a pair whose new type is 0 none.
        let chain = AreaChange {
            area: 0,
            types: vec![1, 2, 2, 3, 3, 0],
            word: 0,
            hostile: None,
        };
        assert_eq!(chain.creature(1), Some((3, 2)));
        assert_eq!(chain.item(1, 2), 0);
        // An area above 49 changes nothing.
        assert_eq!(AreaChange::of(&[op(0x69, Value::Str("50".into()))]), None);
    }

    #[test]
    fn a_turned_create_npc_takes_the_pairs_flags_in_its_words_low_half() {
        let turn = |flags| Turn {
            from: 279,
            to: 276,
            flags,
        };
        // No flags: the word stays.
        assert_eq!(turn(0).of(0x0200_4000), (276, 0x0200_4000));
        // `0x12`'s 1 replaces the mode, the high half kept.
        assert_eq!(turn(0x1).of(0x0200_4000), (276, 0x0200_0001));
        // `0x4C`'s top bit spreads over the high half.
        assert_eq!(turn(0x8000).of(0x0200_4000), (276, 0xFFFF_8000));
        // `0x14` clears the bits before it, not those after.
        assert_eq!(region_change_word([0x12, 0x14, 0x46]), 0x4000);
        assert_eq!(region_change_word([0x85, 0x14]), 0x200_0000);
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
    fn a_creatures_icon_is_its_stamps_its_trades_or_its_dialogues() {
        // The novice's dialogue's icon, 0, and one that `SetIcon` made 13.
        assert_eq!(icon(5, 0, Some(0)), 0);
        assert_eq!(icon(5, 0, Some(13)), 13);
        assert_eq!(icon(0, 0, None), 0);
        // A smith with a dialogue shows the smith's; the trader of `0x64` its stamp's 4.
        assert_eq!(icon(5, SMITH, Some(10)), 0x20);
        assert_eq!(icon(0, MERCHANT | WIZARD, None), 0x21);
        assert_eq!(icon(TRADER_TALK, TRADER, Some(0)), 4);
        assert_eq!(icon(0, TRADER, None), 0x23);
    }

    #[test]
    fn a_region_runs_its_init_once_whichever_of_its_areas_comes_first() {
        // The starting sector 50,39 is of the area 1, Adelina's 50,45 of the area 2; the areas
        // 15 to 18 all make the region 24, and 50 to 68 the regions 51 to 69 (`0x95CBFC`).
        assert_eq!(region(1), 1);
        assert_eq!(region(2), 2);
        assert_eq!(region(9), 111);
        assert_eq!([15, 16, 17, 18].map(region), [24; 4]);
        assert_eq!(region(50), 51);
        assert_eq!(region(68), 69);
        assert_eq!(region(69), 0);
        let mut regions = Regions::default();
        assert!(regions.init(region(15)));
        assert!(!regions.init(region(16)));
        assert!(regions.ran(24) && !regions.ran(2));
    }

    #[test]
    fn a_base_trigger_holds_its_box_split_at_the_sectors_edges() {
        let mut triggers = Triggers::default();
        // Quest 201's line of cells by the wolves, and a box across four sectors.
        triggers.set("Wolfattack", (3205, 2902, 0), Some((3205, 2916)));
        triggers.set("wide", (130, 130, 3), Some((126, 126)));
        assert_eq!(triggers.at((3205, 2910), 0), Some("Wolfattack"));
        // The ground's floor 1 is the ground's to a trigger, floor 2 another layer.
        assert_eq!(triggers.at((3205, 2910), 1), Some("Wolfattack"));
        assert_eq!(triggers.at((3205, 2910), 2), None);
        assert_eq!(triggers.at((3206, 2910), 0), None);
        for cell in [(126, 126), (127, 128), (128, 127), (130, 130)] {
            assert_eq!(triggers.at(cell, 3), Some("wide"), "{cell:?}");
        }
        assert_eq!(triggers.0.len(), 5);
        // One cell without a second corner; a layer past 16 is the ground's.
        triggers.set("one", (10, 20, 40), None);
        assert_eq!(triggers.at((10, 20), 0), Some("one"));
        assert_eq!(triggers.at((10, 21), 0), None);
    }

    #[test]
    fn a_base_trigger_removed_by_name_goes_from_every_sector_or_about_a_corner() {
        let mut triggers = Triggers::default();
        triggers.set("a", (10, 10, 0), Some((12, 12)));
        triggers.set("b", (11, 11, 0), None);
        triggers.set("A", (200, 200, 0), None);
        triggers.set("c", (10, 10, 0), None);
        // The first set holds the cell.
        assert_eq!(triggers.at((11, 11), 0), Some("a"));
        // About a far corner, the near boxes stay.
        triggers.delete("a", Some((1000, 1000)));
        assert_eq!(triggers.at((11, 11), 0), Some("a"));
        triggers.delete("a", Some((70, 70)));
        assert_eq!(triggers.at((11, 11), 0), Some("b"));
        assert_eq!(triggers.at((200, 200), 0), Some("A"));
        // The last box moved into the place of the one taken out comes before the others.
        let mut swapped = Triggers::default();
        for name in ["x", "y", "z"] {
            swapped.set(name, (5, 5, 0), None);
        }
        swapped.delete("X", None);
        assert_eq!(swapped.at((5, 5), 0), Some("z"));
        triggers.delete("a", None);
        assert_eq!(triggers.at((200, 200), 0), None);
    }

    #[test]
    fn a_base_triggers_function_names_its_quest_and_its_trigger() {
        assert_eq!(
            trigger_function("OMO201Wolfattack"),
            Some((201, "Wolfattack"))
        );
        assert_eq!(
            trigger_function("OMO-1tpreinHQ_5.7.4"),
            Some((-1, "tpreinHQ_5.7.4"))
        );
        assert_eq!(trigger_function("OMO-1"), Some((-1, "")));
        assert_eq!(trigger_function("OMO"), None);
        assert_eq!(trigger_function("QIS_OnEnter1"), None);
    }

    #[test]
    fn a_place_defined_again_replaces_its_numbers() {
        let mut positions = Positions::default();
        positions.define("pos_posTochter", &[3244, 2928]);
        positions.define("tpreinzHQ_5.7.4", &[5536, 2291, 0, 7, 9]);
        positions.define("POS_POSTOCHTER", &[1, 2, 3]);
        assert_eq!(positions.get("pos_postochter"), Some(&[1, 2, 3][..]));
        assert_eq!(
            positions.get("tpreinzhq_5.7.4"),
            Some(&[5536, 2291, 0, 7][..])
        );
        assert_eq!(positions.get("nowhere"), None);
    }

    #[test]
    fn a_timer_runs_its_time_in_minutes_after_it_is_set_whatever_the_date() {
        let mut timers = Timers::default();
        // Quest 1202's three days, and a timer that a day's end does not hurry.
        timers.set(1202, "zeit1202", 4_320);
        for _ in 0..100 {
            timers.tick();
        }
        timers.set(7, "seven", 1_440);
        // Neither a negative id, nor a time of 0, nor a function without a name.
        timers.set(-1, "never", 1);
        timers.set(8, "never", 0);
        timers.set(9, "", 1);
        assert_eq!(timers.list().len(), 2);
        let mut run = Vec::new();
        for _ in 100..4_320 + 100 {
            timers.tick();
            while let Some(timer) = timers.due().cloned() {
                run.push((timers.minutes(), timer.function.clone()));
                timers.ran(timer.id);
            }
        }
        assert_eq!(
            run,
            [(1_540, "seven".to_owned()), (4_320, "zeit1202".to_owned())]
        );
        assert!(timers.list().is_empty());
    }

    #[test]
    fn a_timer_set_again_starts_anew_in_its_place_and_one_deleted_goes() {
        let mut timers = Timers::default();
        timers.set(1, "a", 10);
        timers.set(2, "b", 10);
        timers.set(3, "c", 10);
        for _ in 0..5 {
            timers.tick();
        }
        // Set again, the first takes the new function and starts anew where it stands.
        timers.set(1, "a2", 10);
        let functions = |t: &Timers| {
            t.list()
                .iter()
                .map(|t| t.function.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(functions(&timers), ["a2", "b", "c"]);
        // Deleted, the last takes the first's place.
        timers.delete(1);
        assert_eq!(functions(&timers), ["c", "b"]);
        for _ in 0..5 {
            timers.tick();
        }
        assert_eq!(timers.due().map(|t| t.function.as_str()), Some("c"));
    }

    #[test]
    fn two_quests_drops_of_one_creature_type_stand_side_by_side() {
        let mut drops = Drops::default();
        // Quests 15015 and 10259 on the creature type 38 (docs/re/netscript.md).
        drops.set(38, 6005, 75);
        drops.set(38, 4931, 65);
        drops.set(99, 1355, 75);
        assert_eq!(drops.of(38).collect::<Vec<_>>(), [(6005, 75), (4931, 65)]);
        // The later quest counted, its chance 0 takes its own pair out alone.
        drops.set(38, 4931, 0);
        assert_eq!(drops.of(38).collect::<Vec<_>>(), [(6005, 75)]);
        drops.set(38, 6005, 50);
        assert_eq!(drops.of(38).collect::<Vec<_>>(), [(6005, 50)]);
        drops.set(40, 1, 0);
        assert_eq!(drops.of(40).count(), 0);
        assert_eq!(drops.of(99).collect::<Vec<_>>(), [(1355, 75)]);
    }

    #[test]
    fn a_death_runs_its_first_function_and_takes_the_others_out() {
        let mut deaths = DeathFunctions::default();
        assert_eq!(deaths.take(), None);
        // `CreateNPC`'s `0x05` and `0x60`, in their order; a sight function runs at no death, and
        // the death takes it out.
        deaths.add("tot2203");
        deaths.see("schmuggler1402");
        assert_eq!(deaths.take().as_deref(), Some("tot2203"));
        assert!(deaths.is_empty());
        let mut sights = DeathFunctions::default();
        sights.see("schmugler1402");
        assert_eq!(sights.take(), None);
        assert!(!sights.is_empty());
        // An item's entries are no death's, and stay; a `wegw` entry runs and stays.
        let mut deaths = DeathFunctions::default();
        deaths.add("Take:take1404");
        deaths.add("wegweiser");
        deaths.add("od_5002");
        assert_eq!(deaths.take().as_deref(), Some("wegweiser"));
        assert_eq!(deaths.take().as_deref(), Some("wegweiser"));
        // `SetNPCState`'s `0x05` replaces the deaths' and the sight functions, its `0x60` adds a
        // sight function.
        let mut deaths = DeathFunctions::default();
        deaths.add("first");
        deaths.see("look");
        deaths.set("lose670");
        deaths.see("second");
        assert_eq!(deaths.take().as_deref(), Some("lose670"));
        assert_eq!(deaths.take(), None);
    }

    #[test]
    fn a_count_runs_its_function_once_done_and_starts_again_when_set_again() {
        let mut kills = Counts::default();
        // A new type is kept only with a function, and a count of 0 takes one out.
        kills.set(691, 3, None);
        assert_eq!(kills.count(691), None);
        kills.set(691, 2, Some("ok_10252"));
        assert_eq!(
            kills.count(691),
            Some(Counted::Goes {
                counted: 1,
                asked: 2
            })
        );
        // Set again, it starts again at its new count.
        kills.set(691, 3, None);
        assert_eq!(
            kills.count(691),
            Some(Counted::Goes {
                counted: 1,
                asked: 3
            })
        );
        kills.count(691);
        assert_eq!(kills.count(691), Some(Counted::Done("ok_10252".into())));
        assert_eq!(kills.count(691), None);
        kills.set(190, 5, Some("ok_10253"));
        kills.set(190, 0, None);
        assert_eq!(kills.count(190), None);
        // A creature's type counts the item of its drop.
        let mut collecting = Counts::default();
        collecting.set(38, 1, Some("oc_10250"));
        collecting.retype(|kind| (kind == 38).then_some(6005));
        assert_eq!(collecting.count(38), None);
        assert_eq!(
            collecting.count(6005),
            Some(Counted::Done("oc_10250".into()))
        );
    }

    #[test]
    fn the_scripts_state_round_trips() {
        let mut scripts = Scripts::default();
        let mut ran = Vec::new();
        scripts.quests.set_up(1, &mut runtime(&mut ran, true));
        scripts.quests.trigger(1, &mut runtime(&mut ran, true));
        scripts.variables.set("quest1_done", 1);
        scripts.dialogues.add("first", -1);
        scripts.regions.init(2);
        scripts.positions.define("novizin1", &[3237, 2514]);
        scripts
            .triggers
            .set("Wolfattack", (3205, 2902, 0), Some((3205, 2916)));
        scripts.timers.set(1202, "zeit1202", 4_320);
        scripts.drops.set(38, 6005, 75);
        let bytes = postcard::to_allocvec(&scripts).expect("the state serializes");
        let back: Scripts = postcard::from_bytes(&bytes).expect("the state reads back");
        // A loaded game's entries are not new: no save keeps `+0xC`.
        assert!(scripts.quests.book[0].new && !back.quests.book[0].new);
        scripts.quests.book[0].new = false;
        assert_eq!(back, scripts);
    }
    #[test]
    fn a_stash_takes_a_place_from_its_pool_and_a_search_takes_it_away() {
        /// Dice that answer each `below` with its largest number.
        struct Last;
        impl crate::spawning::Dice for Last {
            fn below(&mut self, n: u32) -> u32 {
                n.saturating_sub(1)
            }
        }
        let mut pools = Pools::default();
        for cell in [(1, 1), (2, 2), (3, 3)] {
            pools.add("Versteck_1", cell);
        }
        let mut stashes = Stashes::default();
        stashes.lay(
            &mut pools,
            ("res:VERSTECKMOUSEOVER", "Versteck_1", "VERSTECK"),
            2,
            &mut Last,
        );
        let cells: Vec<_> = stashes.0.iter().map(|stash| stash.cell).collect();
        assert_eq!(cells, [(3, 3), (2, 2)]);
        assert_eq!(pools.len("Versteck_1"), 1);
        assert_eq!(stashes.0[0].text, "VERSTECKMOUSEOVER");
        // More than the pool holds lays what it holds.
        stashes.lay(&mut pools, ("t", "Versteck_1", "f"), 5, &mut Last);
        assert_eq!(stashes.0.len(), 3);
        assert_eq!(
            stashes.search((2, 2)).map(|stash| stash.function),
            Some("VERSTECK".into())
        );
        assert_eq!(stashes.search((2, 2)), None);
    }
}
