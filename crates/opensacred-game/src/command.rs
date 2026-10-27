//! The game's commands (docs/10 §6): one set that scripts, the console and the remote protocol run
//! alike, each giving lines of output or an error. A line is split as a shell splits it and parsed
//! by clap as a REPL (`multicall`), so that a command takes the options of the command line and
//! has `help`; it runs between frames with the whole world at hand.
//!
//! A script is lines of commands: `--exec "A; B"`, `--script FILE` or the command `exec`. Its
//! `wait` holds the next line until a game is ready, the hero has arrived, or some frames or
//! seconds have passed; its output goes to the log.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Duration;

use bevy::prelude::*;
use clap::error::ErrorKind;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use opensacred_rules::weather::Event;

use crate::benchmark::Benchmark;
use crate::console::Console;
use crate::daynight::{self, Clock};
use crate::game::{AppState, Data, NewGame, Request, Shows};
use crate::help::Help;
use crate::hero::{Arrival, Body, Flies, Gait, Hero, Ready};
use crate::overlay::Probe;
use crate::pause::{Play, Still};
use crate::screenshot::{self, Recording};
use crate::sectors::Streamer;
use crate::settings::{Mode, Settings, SettingsFile};
use crate::terrain::{About, Heights, LANDING_REACH, Landing, NATURES};
use crate::{ai, atmosphere, crowd, debug, fight, iso, music, npcs, paths};

/// A line: one command.
#[derive(Parser, Debug)]
#[command(multicall = true)]
struct Line {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// End the game, if any, and start a new one, with the command line's options of a game
    NewGame(NewGame),
    /// Back to the main menu
    Menu,
    /// End the process
    Quit,
    /// Put the hero at the centre of sector X,Y of the 100 x 100 grid, or on the ground nearest
    /// it where it cannot stand and walk there
    Sector {
        #[arg(value_parser = parse_position)]
        at: (u32, u32),
    },
    /// Put the hero on global cell X,Y, or +DX,+DY cells from its own, whatever the cell: on the
    /// ground, or on floor FLOOR of a building there, 0 the ground even within its walls
    Cell {
        #[arg(value_parser = parse_at, allow_hyphen_values = true)]
        at: At,
        floor: Option<u8>,
    },
    /// What a cell of the loaded sectors is, the hero's by default (X,Y or +DX,+DY): its
    /// ground, the floors over it, what stands in it, and whether it is a pocket
    CellInfo {
        #[arg(value_parser = parse_at, allow_hyphen_values = true)]
        at: Option<At>,
    },
    /// Where `sector` would land the hero for a cell, the hero's by default (X,Y or +DX,+DY):
    /// the cell itself, a room of a building's floor, or the nearest cell of the ground that it
    /// can walk on from
    Nearest {
        #[arg(value_parser = parse_at, allow_hyphen_values = true)]
        at: Option<At>,
    },
    /// Send the hero to cell X,Y, or by +DX,+DY cells along X,Y (screen right-down, left-down),
    /// as a click does
    Go {
        #[arg(value_parser = parse_goal, allow_hyphen_values = true)]
        to: Goal,
    },
    /// Walk or run, as R
    Gait { gait: GaitArg },
    /// The camera's zoom, 0.5 to 2 as in the original: 2 shows twice the world
    Zoom { zoom: f32 },
    /// Set the clock, as HH:MM
    Time {
        #[arg(value_parser = daynight::parse_time)]
        time: Clock,
    },
    /// Start the rain of intensity N, 1 to 100, or stop it with 0, as the original's `setrain`
    Setrain { value: u32 },
    /// Start the fog of intensity N, 1 to 100, or stop it with 0, as `setfog`
    Setfog { value: u32 },
    /// Start the snow of intensity N, 1 to 100, or stop it with 0, as `setsnow`
    Setsnow { value: u32 },
    /// Shake the screen by a third of N pixels, 1 to 100, falling by 10 a second, as `setquake`
    Setquake { value: u32 },
    /// The music and the ambient loops
    Music { on: Switch },
    /// Pause the game without the pause's menu, for a still frame, or go on
    Pause { on: Switch },
    /// Save the next frame to FILE (PNG) and go on
    Screenshot { file: PathBuf },
    /// Save N frames to DIR/frame-NNNN.png and go on
    Record { dir: PathBuf, frames: u32 },
    /// Measure the frame rate for SECONDS, print it and go on
    Benchmark { seconds: f32 },
    /// Borderless full screen, or a window of WIDTH x HEIGHT; kept in the settings
    Window {
        mode: Mode,
        width: Option<u32>,
        height: Option<u32>,
    },
    /// Show or hide the debug overlay (F3); kept in the settings
    Overlay { on: Option<Switch> },
    /// Show or hide the log's panel (F4); kept in the settings
    Log { on: Option<Switch> },
    /// Open or close the console (the key under Esc)
    Console { on: Option<Switch> },
    /// Show or hide the keys' help (F1)
    Keys { on: Option<Switch> },
    /// The state of the process, the game, the hero, the clock and the frames
    State,
    /// The creatures within RADIUS cells of the hero (20 by default), the nearest first, as the
    /// original's driver lists them
    Creatures { radius: Option<u32> },
    /// A creature's fight values, the hero's by default; ID as `creatures` prints it
    Stats { id: Option<String> },
    /// Make COUNT creatures of type TYPE (as `creatures` prints it) on a cell and the rings about
    /// it, whatever the cells are but the hero's, as the original's driver makes them, the hero's
    /// cell by default, at their sector's level for the hero's or at LEVEL; hostile or peaceful
    /// by their class unless told
    Spawn {
        creature: u32,
        /// The cell, X,Y or +DX,+DY from the hero's
        #[arg(long, value_parser = parse_at, allow_hyphen_values = true)]
        at: Option<At>,
        #[arg(long, default_value_t = 1)]
        count: usize,
        #[arg(long)]
        level: Option<u32>,
        #[arg(long, value_enum)]
        rank: Option<RankArg>,
        #[arg(long, conflicts_with = "peaceful")]
        hostile: bool,
        #[arg(long)]
        peaceful: bool,
        /// They take no damage: a target for measuring the hero's hits
        #[arg(long)]
        immortal: bool,
    },
    /// Fell the hostile creatures within RADIUS cells of the hero (5 by default), as if it
    /// killed them: they drop and give it their experience
    Kill { radius: Option<u32> },
    /// Throw an item of type ITEM (an `items.pak` slot) about a cell, the hero's by default, as
    /// a death's drop is thrown
    Item {
        item: u32,
        /// The cell, X,Y or +DX,+DY from the hero's
        #[arg(long, value_parser = parse_at, allow_hyphen_values = true)]
        at: Option<At>,
        /// The level the item maker makes it at, the hero's by default
        #[arg(long)]
        level: Option<u32>,
        /// The pass's quality key, 0 to 17 (docs/re/items.md): 16 a plain item, 2 to 15 one of
        /// the maker's key 16 - KEY, 0 its 14
        #[arg(long, default_value_t = 16)]
        key: u8,
        /// Gold's pieces, a pile's by the level by default
        #[arg(long)]
        amount: Option<u32>,
    },
    /// The hero's level, its experience at the level's start: its health, values and points
    /// follow as at a level-up
    Level { level: u32 },
    /// Set the hero's experience to N, or add N to it with +N: its level follows as its kills
    /// raise it
    Experience { points: String },
    /// The hero attacks the hostile creature ID, or the nearest, as a left click on it does: it
    /// comes into its band and strikes once, or shoots with a bow or a crossbow; ID as
    /// `creatures` prints it
    Attack { id: Option<String> },
    /// The hero goes to take the item lying nearest it, as a left click on it does, and takes
    /// it once it stands within its reach
    Take,
    /// The hero goes to speak to the creature ID, or to the nearest of a dialogue, as a left click
    /// on it does; ID as `creatures` prints it
    Talk { id: Option<String> },
    /// The dialogue's window shown: its button N (from 1) chosen, as a click on it does, or its
    /// lines without N
    Answer { button: Option<usize> },
    /// A creature's health to its most, or to N (1 to its most), the hero's by default; ID as
    /// `creatures` prints it
    Heal {
        id: Option<String>,
        #[arg(long)]
        to: Option<u32>,
    },
    /// Give a creature, the hero by default, the effect EFFECT of the value VALUE for SECONDS
    /// (60), as the combat arts and the spells give them (`0x582D50`): its fight values follow
    /// from the next frame (`stats`); ID as `creatures` prints it
    Effect {
        effect: u32,
        #[arg(allow_hyphen_values = true)]
        value: f32,
        /// Its whole value
        #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
        whole: i32,
        #[arg(long, default_value_t = 60.0)]
        seconds: f32,
        /// The art it is keyed by
        #[arg(long, default_value_t = 0)]
        art: u32,
        /// The level it carries (the arrowproof's, which its deflection reads)
        #[arg(long, default_value_t = 0)]
        level: u16,
        #[arg(long)]
        id: Option<String>,
    },
    /// The hero takes no damage, or takes it again; without a switch, toggles
    God { on: Option<Switch> },
    /// The hero goes straight where it is sent, through walls and over water, on the ground's
    /// level, or walks again; without a switch, toggles
    Fly { on: Option<Switch> },
    /// Make the hostile creature nearest the hero drop, as if the hero killed it, TIMES times
    Drop { times: Option<u32> },
    /// Run the function FUNCTION of the hero's tree as the scripts run one, a function of a quest
    /// only while the quest is set up, the hero put where it teleports it; then list the
    /// scripts' timers
    Script { function: String },
    /// Set the quest QUEST up, or trigger it, entering it where its trigger runs through, as the
    /// scripts' records do, its sections run in turn; then list the scripts' timers
    Quest { action: QuestAction, quest: u32 },
    /// The hero's combat arts, as the original's driver lists them: each one's table, id, level
    /// and bonus, whether a rune taught it, its time, its factor and what is still to run, which
    /// runs down as they regenerate
    Arts {
        #[command(subcommand)]
        action: Option<ArtsAction>,
    },
    /// In a script: wait before the next command
    Wait {
        #[command(subcommand)]
        until: Wait,
    },
    /// Run a script: a command a line, # before a comment
    Exec { file: PathBuf },
}

/// What `arts` does before it lists the hero's arts.
#[derive(Subcommand, Debug, Clone, Copy, PartialEq, Eq)]
enum ArtsAction {
    /// The hero learns ART, below 1000 a spell, or rises in it by LEVELS (1), as that many runes
    /// teach it, whatever its class
    Learn {
        art: u16,
        #[arg(value_parser = clap::value_parser!(u8).range(1..))]
        levels: Option<u8>,
    },
    /// Every art of the hero ready at once
    Ready,
    /// A use of ART, as the hero's making it starts the regeneration: every art of its kind
    /// takes its time to run
    Use { art: u16 },
    /// The art that the right button makes
    Select { art: u16 },
    /// The hero makes ART, the selected by default, on the hostile creature nearest it, as a
    /// right click on it does
    Make { art: Option<u16> },
}

/// What a script waits for.
#[derive(Subcommand, Debug, Clone, Copy, PartialEq)]
pub enum Wait {
    /// A game played, its hero animating and every sector near it in place
    Ready,
    /// The hero where it was sent
    Arrived,
    /// N frames
    Frames { n: u32 },
    /// S seconds of the real time
    Seconds { s: f32 },
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum Switch {
    On,
    Off,
}

/// A spawned creature's rank (`fight::Rank`).
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum RankArg {
    Champion,
    Boss,
}

/// A quest's transition (`opensacred_rules::quests::Transition`).
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum QuestAction {
    SetUp,
    Trigger,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum GaitArg {
    Walk,
    Run,
}

/// Where `go` sends the hero: a global cell, or an offset in cells.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Goal {
    Cell(u32, u32),
    By(f32, f32),
}

/// `X,Y` of a cell, or `+DX,+DY` of an offset: a sign makes it an offset.
fn parse_goal(text: &str) -> Result<Goal, String> {
    let (x, y) = text.split_once(',').ok_or("expected X,Y or +DX,+DY")?;
    if text.contains(['+', '-']) {
        let number = |s: &str| s.trim().parse::<f32>().map_err(|e| format!("{s}: {e}"));
        Ok(Goal::By(number(x)?, number(y)?))
    } else {
        let number = |s: &str| s.trim().parse::<u32>().map_err(|e| format!("{s}: {e}"));
        Ok(Goal::Cell(number(x)?, number(y)?))
    }
}

/// A cell that a command names: a global cell, or an offset in cells from the hero's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum At {
    Cell(u32, u32),
    By(i32, i32),
}

/// `X,Y` of a cell, or `+DX,+DY` of an offset from the hero's: a sign makes it an offset.
fn parse_at(text: &str) -> Result<At, String> {
    let (x, y) = text.split_once(',').ok_or("expected X,Y or +DX,+DY")?;
    if text.contains(['+', '-']) {
        let number = |s: &str| s.trim().parse::<i32>().map_err(|e| format!("{s}: {e}"));
        Ok(At::By(number(x)?, number(y)?))
    } else {
        let number = |s: &str| s.trim().parse::<u32>().map_err(|e| format!("{s}: {e}"));
        Ok(At::Cell(number(x)?, number(y)?))
    }
}

/// `x,y` of a grid position.
fn parse_position(text: &str) -> Result<(u32, u32), String> {
    let (x, y) = text.split_once(',').ok_or("expected X,Y")?;
    let number = |s: &str| s.trim().parse::<u32>().map_err(|e| format!("{s}: {e}"));
    Ok((number(x)?, number(y)?))
}

/// What a command gave: its lines, and for `wait` what a script waits for.
#[derive(Debug, Default, PartialEq)]
pub struct Outcome {
    pub lines: Vec<String>,
    pub wait: Option<Wait>,
}

impl Outcome {
    fn say(line: impl Into<String>) -> Self {
        Self {
            lines: vec![line.into()],
            wait: None,
        }
    }
}

/// Splits `line` into its words: blanks part them, a pair of single or double quotes keeps
/// blanks within a word, and a backslash is a plain character, so that a Windows path keeps its
/// separators (#368); `None` when a quote is left open.
fn words(line: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut word: Option<String> = None;
    let mut quote = None;
    for c in line.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => word.get_or_insert_default().push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                word.get_or_insert_default();
            }
            None if c.is_whitespace() => words.extend(word.take()),
            None => word.get_or_insert_default().push(c),
        }
    }
    if quote.is_some() {
        return None;
    }
    words.extend(word);
    Some(words)
}

/// Runs `line`: a command and its options. `help` and `--help` are output, not errors.
pub fn run(world: &mut World, line: &str) -> Result<Outcome, String> {
    let words = words(line).ok_or("unbalanced quotes")?;
    if words.is_empty() {
        return Ok(Outcome::default());
    }
    match Line::try_parse_from(&words) {
        Ok(parsed) => execute(world, parsed.command),
        Err(e) if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
            Ok(Outcome {
                lines: e
                    .render()
                    .to_string()
                    .trim_end()
                    .lines()
                    .map(Into::into)
                    .collect(),
                wait: None,
            })
        }
        Err(e) => Err(e.render().to_string().trim_end().to_owned()),
    }
}

/// Fails unless a game is played.
fn in_game(world: &World) -> Result<(), String> {
    match world.get_resource::<State<AppState>>().map(State::get) {
        Some(AppState::InGame) => Ok(()),
        _ => Err("no game is played".into()),
    }
}

/// The console's `setrain`, `setfog`, `setsnow` (`0x614230`, #1072) and `setquake` (#1073): the
/// weather takes the event of their kind and value, at most 100.
fn set_weather(world: &mut World, event: Event) -> Result<Outcome, String> {
    in_game(world)?;
    let mut weather = world
        .get_resource_mut::<crate::weather::Weather>()
        .ok_or("no weather in this game")?;
    weather.set(event);
    Ok(Outcome::say(format!("the weather takes {event:?}")))
}

fn set_state(world: &mut World, state: AppState) {
    world.resource_mut::<NextState<AppState>>().set(state);
}

/// Puts the hero at world point `position`: it lands on the cell there as `arrival` says once
/// the sectors about it are in (`hero::land`).
fn teleport(world: &mut World, mut position: Vec3, arrival: Arrival) -> Result<(), String> {
    in_game(world)?;
    if let Some(y) = world
        .get_resource::<Heights>()
        .and_then(|heights| heights.standing(position.x, position.z, None))
    {
        position.y = y;
    }
    let mut bodies = world.query::<(&mut Transform, &mut Body)>();
    let (mut transform, mut body) = bodies
        .iter_mut(world)
        .next()
        .ok_or("the hero is not in the world yet")?;
    body.put(&mut transform, position, arrival);
    Ok(())
}

/// Sets the hero's experience to `points`, or adds them to it where `add`, its level and points
/// as its kills would give them (`Experience::add`); `fight::level_up` then rebuilds its health
/// and values. A line of what it has.
fn experience(world: &mut World, points: u64, add: bool) -> Result<String, String> {
    let rules = world
        .get_resource::<fight::Rules>()
        .ok_or("no fight is played")?
        .clone();
    let mut experience = world
        .get_resource_mut::<fight::Experience>()
        .ok_or("no fight is played")?;
    if !add {
        *experience = fight::Experience::default();
    }
    experience.add(points, &rules);
    Ok(format!(
        "the hero is level {} with {} experience, {} attribute and {} skill points",
        experience.level, experience.points, experience.attribute_points, experience.skill_points
    ))
}

/// The hero makes `art` on the hostile creature nearest it (`making::Making`), or a boon on
/// itself, its attack ending; which creature, as a line.
fn make_art(world: &mut World, art: u16) -> Result<String, String> {
    let mut heroes =
        world.query_filtered::<(Entity, &Transform), (With<Body>, Without<npcs::Dies>)>();
    let (hero, at) = heroes
        .iter(world)
        .next()
        .map(|(hero, transform)| (hero, transform.translation))
        .ok_or("the hero is not in the world yet")?;
    // An art under way keeps its order, as the right button leaves it (`making::aim`).
    if world.get::<crate::making::Playing>(hero).is_some() {
        return Err("the hero is still making an art".into());
    }
    // A boon and the changeling are made on the hero, as is an art on a friend without one (the
    // launch type 2) and a spell on itself (5).
    let launch = crate::making::launch(world.resource::<crate::arts::HeroArts>(), art);
    if matches!(
        opensacred_rules::arts::style(art),
        Some(
            opensacred_rules::arts::Style::Boon
                | opensacred_rules::arts::Style::Frenzy
                | opensacred_rules::arts::Style::Bolts
                | opensacred_rules::arts::Style::Glance
                | opensacred_rules::arts::Style::Changeling
        )
    ) || matches!(launch, Some(2 | 5))
    {
        world
            .entity_mut(hero)
            .remove::<(crate::attack::Attack, crate::attack::Striking)>()
            .insert(crate::making::Making {
                art,
                target: hero,
                point: None,
            });
        return Ok(format!("the hero makes {art} on itself"));
    }
    // An art of the launch type 4 is made on the nearest dead creature (`0x51D840`, case 4).
    if launch == Some(4) {
        let mut corpses = world.query_filtered::<(Entity, &GlobalTransform), With<npcs::Corpse>>();
        let (corpse, distance) = corpses
            .iter(world)
            .map(|(entity, place)| (entity, (place.translation() - at).xz().length()))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .ok_or("no dead creature is about")?;
        world
            .entity_mut(hero)
            .remove::<(crate::attack::Attack, crate::attack::Striking)>()
            .insert(crate::making::Making {
                art,
                target: corpse,
                point: None,
            });
        return Ok(format!(
            "the hero makes {art} on the dead {corpse} at {distance:.0}"
        ));
    }
    let mut creatures = world.query_filtered::<
        (Entity, &GlobalTransform, &ai::Mind, &fight::Health),
        Without<npcs::Dies>,
    >();
    let (target, distance) = creatures
        .iter(world)
        .filter(|(_, _, mind, health)| mind.hostile() && health.current > 0)
        .map(|(entity, place, ..)| (entity, (place.translation() - at).xz().length()))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .ok_or("no hostile creature is about")?;
    // An art aimed at a point takes the creature's place (the launch type 3, and the dash).
    let aimed = launch == Some(3)
        || opensacred_rules::arts::style(art) == Some(opensacred_rules::arts::Style::Dash);
    let point = aimed.then(|| {
        let mut places = world.query::<&GlobalTransform>();
        places
            .get(world, target)
            .map_or(at, GlobalTransform::translation)
    });
    world
        .entity_mut(hero)
        .remove::<(crate::attack::Attack, crate::attack::Striking)>()
        .insert(crate::making::Making { art, target, point });
    Ok(format!("the hero makes {art} on {target} at {distance:.0}"))
}

/// The hero's attack on the hostile creature `id`, or the nearest alive, as a left click on it
/// orders it (`attack::Attack::ordered`).
fn attack(world: &mut World, id: Option<Entity>) -> Result<String, String> {
    let mut heroes =
        world.query_filtered::<(Entity, &Transform), (With<Body>, Without<npcs::Dies>)>();
    let (hero, at) = heroes
        .iter(world)
        .next()
        .map(|(hero, transform)| (hero, transform.translation))
        .ok_or("the hero is not in the world yet")?;
    let mut creatures = world.query_filtered::<
        (Entity, &GlobalTransform, &ai::Mind, &fight::Health),
        Without<npcs::Dies>,
    >();
    let (target, distance) = creatures
        .iter(world)
        .filter(|&(entity, _, mind, health)| {
            mind.hostile() && health.current > 0 && id.is_none_or(|id| id == entity)
        })
        .map(|(entity, place, ..)| (entity, (place.translation() - at).xz().length()))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .ok_or("no hostile creature alive to attack")?;
    world
        .entity_mut(hero)
        .remove::<(crate::making::Making, crate::attack::Striking)>()
        .insert(crate::attack::Attack::ordered(target));
    Ok(format!("the hero attacks {target} at {distance:.0}"))
}

/// Gives the hero a marker `M` (`fight::Immortal`, `hero::Flies`) or takes it away, as `on`
/// says or else the other way round; whether it has it now.
fn hero_switch<M: Component + Default>(
    world: &mut World,
    on: Option<Switch>,
) -> Result<bool, String> {
    let mut heroes = world.query_filtered::<(Entity, Has<M>), With<Body>>();
    let (hero, has) = heroes
        .iter(world)
        .next()
        .ok_or("the hero is not in the world yet")?;
    let on = match on {
        Some(Switch::On) => true,
        Some(Switch::Off) => false,
        None => !has,
    };
    if on {
        world.entity_mut(hero).insert(M::default());
    } else {
        world.entity_mut(hero).remove::<M>();
    }
    Ok(on)
}

/// The hero's global cell.
fn hero_cell(world: &mut World) -> Result<(i32, i32), String> {
    let mut bodies = world.query_filtered::<&Transform, With<Body>>();
    let transform = bodies
        .iter(world)
        .next()
        .ok_or("the hero is not in the world yet")?;
    Ok(paths::cell_of(
        transform.translation.x,
        transform.translation.z,
    ))
}

/// The global cell that `at` names, the hero's when none.
fn cell_at(world: &mut World, at: Option<At>) -> Result<(i32, i32), String> {
    let cell = match at {
        Some(At::Cell(x, y)) => (
            i32::try_from(x).map_err(|e| e.to_string())?,
            i32::try_from(y).map_err(|e| e.to_string())?,
        ),
        Some(At::By(dx, dy)) => {
            let (x, y) = hero_cell(world)?;
            (x + dx, y + dy)
        }
        None => hero_cell(world)?,
    };
    if cell.0 < 0 || cell.1 < 0 {
        return Err(format!("{},{} lies off the world", cell.0, cell.1));
    }
    Ok(cell)
}

/// The loaded sectors' heights, or why there are none.
fn heights(world: &World) -> Result<&Heights, String> {
    world
        .get_resource::<Heights>()
        .ok_or_else(|| "no world is loaded".into())
}

/// Why a command cannot read cell `(x, y)`: it lies outside the loaded sectors.
fn not_loaded((x, y): (i32, i32)) -> String {
    format!("{x},{y} lies outside the loaded sectors, the 3 x 3 about the hero")
}

/// The building at its anchor, as the lines name it.
fn building((x, y): crate::buildings::Anchor) -> String {
    format!("the building at {x},{y}")
}

/// The lines of `cell-info` for `cell`, which is `about`, a pocket of `pocket` places or none,
/// with `occupants` standing in it.
fn describe(
    (x, y): (i32, i32),
    about: &About,
    pocket: Option<usize>,
    occupants: Vec<String>,
) -> Vec<String> {
    let side = i32::try_from(opensacred_data::world::SIDE).unwrap_or(64);
    let mut lines = vec![format!(
        "cell {x},{y}  sector {},{}  cell {},{} of it",
        x / side,
        y / side,
        x % side,
        y % side
    )];
    let corners = |values: [f32; 4]| {
        values
            .iter()
            .map(|v| format!("{v:.0}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut ground = vec![
        format!(
            "ground {} (0x{:02X}, flags 0x{:02X})",
            NATURES[usize::from(about.ground >> 4)],
            about.ground,
            about.flags
        ),
        if about.walkable {
            "walkable"
        } else {
            "not walkable"
        }
        .to_owned(),
    ];
    for (holds, what) in [
        (about.flown_over, "flown over"),
        (about.entrance, "an entrance"),
        (about.bridged, "a bridge"),
        (about.closed, "closed by a trigger"),
    ] {
        if holds {
            ground.push(what.to_owned());
        }
    }
    lines.push(ground.join(", "));
    lines.push(format!(
        "heights {}  standing {}",
        corners(about.heights),
        corners(about.standing)
    ));
    lines.push(match about.building {
        Some(anchor) => format!("of {}", building(anchor)),
        None => "of no building".into(),
    });
    for inside in &about.inside {
        let mut line = vec![format!(
            "{}{}",
            crate::overlay::place(Some(inside.level)),
            match inside.floor {
                Some((anchor, floor)) => format!(", floor {floor} of {}", building(anchor)),
                None => String::new(),
            }
        )];
        line.push(
            if inside.walkable {
                "walkable"
            } else {
                "not walkable"
            }
            .to_owned(),
        );
        if inside.room {
            line.push("a room".into());
        }
        if inside.exit {
            line.push("an exit".into());
        }
        if let Some((floors, [dx, dy])) = inside.stair {
            let way = if floors > 0 { "up" } else { "down" };
            line.push(format!("a stair {way} {} by {dx:+},{dy:+}", floors.abs()));
        }
        if inside.flown_over {
            line.push("flown over".into());
        }
        if inside.closed {
            line.push("closed by a trigger".into());
        }
        lines.push(line.join(", "));
    }
    lines.push(match pocket {
        Some(places) => format!("a pocket: {places} places reached from it"),
        None => "no pocket: the hero walks on from it".into(),
    });
    if occupants.is_empty() {
        lines.push("nothing stands in it".into());
    } else {
        lines.extend(occupants.into_iter().map(|line| format!("in it: {line}")));
    }
    lines
}

/// What stands in `cell`: the hero, the creatures and the objects there, a line each.
fn occupants(world: &mut World, cell: (i32, i32)) -> Vec<String> {
    let server = world.get_resource::<AssetServer>().cloned();
    let mut found = world.query::<(
        Entity,
        &Transform,
        Has<Body>,
        Option<&ai::Mind>,
        Option<&fight::Health>,
        Option<&npcs::Object>,
        Option<&npcs::Door>,
        Option<&crowd::InTheWay>,
        Option<&WorldAssetRoot>,
    )>();
    let mut lines = Vec::new();
    for (entity, transform, hero, mind, health, object, door, way, root) in found.iter(world) {
        let at = object.map_or(transform.translation, |object| object.at);
        if paths::cell_of(at.x, at.z) != cell || !(hero || mind.is_some() || object.is_some()) {
            continue;
        }
        let mut line = vec![if hero {
            "the hero".to_owned()
        } else {
            entity.to_string()
        }];
        if let Some(mind) = mind {
            line.push(format!("type {} level {}", mind.creature, mind.level));
            line.push(
                if mind.hostile() {
                    "hostile"
                } else {
                    "peaceful"
                }
                .to_owned(),
            );
        } else if object.is_some() {
            line.push("an object".into());
        }
        if let Some(health) = health {
            line.push(format!("health {}/{}", health.current, health.most));
        }
        if let Some(door) = door {
            line.push(
                if door.open {
                    "an open door"
                } else {
                    "a closed door"
                }
                .to_owned(),
            );
        }
        if let Some(way) = way
            && way.level.is_some()
        {
            line.push(crate::overlay::place(way.level));
        }
        let model = root
            .zip(server.as_ref())
            .and_then(|(root, server)| server.get_path(root.0.id()))
            .and_then(|path| {
                path.path()
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
            });
        if let Some(model) = model {
            line.push(model);
        }
        lines.push(line.join("  "));
    }
    lines.sort();
    lines
}

/// Saves `keys` of the settings, as they are now.
fn save(world: &mut World, keys: &[&str]) -> Result<(), String> {
    let settings = world.resource::<Settings>().clone();
    world
        .resource_mut::<SettingsFile>()
        .save(&settings, keys)
        .map_err(|e| format!("{e:#}"))
}

/// Sets a debug panel's setting, or toggles it, and keeps it.
fn panel(world: &mut World, on: Option<Switch>, key: &str, log: bool) -> Result<Outcome, String> {
    let mut settings = world.resource_mut::<Settings>();
    let shown = if log {
        &mut settings.debug.log
    } else {
        &mut settings.debug.overlay
    };
    *shown = match on {
        Some(Switch::On) => true,
        Some(Switch::Off) => false,
        None => !*shown,
    };
    let shown = *shown;
    save(world, &[key])?;
    Ok(Outcome::say(if shown { "shown" } else { "hidden" }))
}

/// Does `command`.
fn execute(world: &mut World, command: Command) -> Result<Outcome, String> {
    let side = opensacred_data::world::SIDE as f32;
    match command {
        Command::NewGame(game) => {
            let game = world
                .get_resource::<Shows>()
                .copied()
                .unwrap_or_default()
                .fill(game);
            world.insert_resource(Request(game));
            set_state(world, AppState::Loading);
            Ok(Outcome::say("a new game loads"))
        }
        Command::Menu => {
            set_state(world, AppState::Menu);
            Ok(Outcome::say("to the main menu"))
        }
        Command::Quit => {
            world.write_message(AppExit::Success);
            Ok(Outcome::say("quitting"))
        }
        Command::Sector { at: (x, y) } => {
            teleport(
                world,
                iso::ground((x as f32 + 0.5) * side, (y as f32 + 0.5) * side),
                Arrival::Nearest,
            )?;
            Ok(Outcome::say("the hero lands there once its sectors are in"))
        }
        Command::Cell { at, floor } => {
            in_game(world)?;
            let (x, y) = cell_at(world, Some(at))?;
            // A floor that no building has at a cell of the loaded sectors; farther, `hero::land`
            // tells it once the sectors are in.
            if let (Some(floor @ 1..), Ok(heights)) = (floor, heights(world))
                && heights.about((x, y)).is_some()
                && heights
                    .building_on_floor((x, y), usize::from(floor))
                    .is_none()
            {
                return Err(format!("no building's floor {floor} holds {x},{y}"));
            }
            teleport(
                world,
                iso::ground(x as f32 + 0.5, y as f32 + 0.5),
                Arrival::On(floor),
            )?;
            let on = match floor {
                None => String::new(),
                Some(0) => " on the ground".into(),
                Some(floor) => format!(" on floor {floor}"),
            };
            Ok(Outcome::say(format!(
                "the hero stands on {x},{y}{on} once its sectors are in"
            )))
        }
        Command::CellInfo { at } => {
            in_game(world)?;
            let cell = cell_at(world, at)?;
            let heights = heights(world)?;
            let about = heights.about(cell).ok_or_else(|| not_loaded(cell))?;
            let level = if about.walkable {
                None
            } else {
                heights.level_of(cell)
            };
            let pocket = heights.pocket(cell, level);
            let occupants = occupants(world, cell);
            Ok(Outcome {
                lines: describe(cell, &about, pocket, occupants),
                wait: None,
            })
        }
        Command::Nearest { at } => {
            in_game(world)?;
            let (x, y) = cell_at(world, at)?;
            let heights = heights(world)?;
            if heights.about((x, y)).is_none() {
                return Err(not_loaded((x, y)));
            }
            Ok(Outcome::say(match heights.landing((x, y)) {
                Some(Landing::On(None)) => format!("{x},{y} itself, on the ground"),
                Some(Landing::On(level)) => match heights.floor(level) {
                    Some((anchor, floor)) => {
                        format!(
                            "{x},{y} itself, in a room of floor {floor} of {}",
                            building(anchor)
                        )
                    }
                    None => format!("{x},{y} itself, {}", crate::overlay::place(level)),
                },
                Some(Landing::Near((nx, ny))) => format!(
                    "{nx},{ny} on the ground, {:+},{:+} from {x},{y}",
                    nx - x,
                    ny - y
                ),
                None => format!("no cell within {LANDING_REACH} of {x},{y} can be walked on from"),
            }))
        }
        Command::Go { to } => {
            in_game(world)?;
            let mut bodies = world.query::<(&Transform, &mut Body)>();
            let (transform, mut body) = bodies
                .iter_mut(world)
                .next()
                .ok_or("the hero is not in the world yet")?;
            let target = match to {
                Goal::Cell(x, y) => iso::ground(x as f32 + 0.5, y as f32 + 0.5),
                Goal::By(dx, dy) => transform.translation + iso::ground(dx, dy),
            };
            body.send(target);
            Ok(Outcome::say("the hero goes"))
        }
        Command::Gait { gait } => {
            in_game(world)?;
            let mut hero = world.get_resource_mut::<Hero>().ok_or("no hero")?;
            hero.gait = match gait {
                GaitArg::Walk => Gait::Walk,
                GaitArg::Run => Gait::Run,
            };
            Ok(Outcome::say(format!("{gait:?}").to_lowercase()))
        }
        Command::Zoom { zoom } => {
            let zoom = zoom.clamp(*iso::ZOOM.start(), *iso::ZOOM.end());
            let mut cameras = world.query_filtered::<&mut Projection, With<Camera3d>>();
            for mut projection in cameras.iter_mut(world) {
                if let Projection::Orthographic(ortho) = &mut *projection {
                    ortho.scale = zoom;
                }
            }
            Ok(Outcome::say(format!("zoom {zoom:.2}")))
        }
        Command::Time { time } => {
            in_game(world)?;
            world.insert_resource(time);
            Ok(Outcome::say("the clock is set"))
        }
        Command::Setrain { value } => set_weather(world, Event::Rain(value)),
        Command::Setfog { value } => set_weather(world, Event::Fog(value)),
        Command::Setsnow { value } => set_weather(world, Event::Snow(value)),
        Command::Setquake { value } => set_weather(world, Event::Quake(value)),
        Command::Music { on } => {
            in_game(world)?;
            match on {
                Switch::On if world.contains_resource::<music::Music>() => {
                    Ok(Outcome::say("the music is on"))
                }
                Switch::On => {
                    let shared = world.resource::<Data>().0.clone();
                    let sound = world.resource::<crate::sound::Index>().clone();
                    let music = music::Music::new(&shared.world, &sound.0);
                    let atmosphere = atmosphere::Atmosphere::new(&sound.0);
                    world.insert_resource(music);
                    world.insert_resource(atmosphere);
                    Ok(Outcome::say("the music is on"))
                }
                Switch::Off => {
                    world
                        .run_system_cached(music::leave)
                        .map_err(|e| e.to_string())?;
                    world
                        .run_system_cached(atmosphere::leave)
                        .map_err(|e| e.to_string())?;
                    world.remove_resource::<music::Music>();
                    world.remove_resource::<atmosphere::Atmosphere>();
                    Ok(Outcome::say("the music is off"))
                }
            }
        }
        Command::Pause { on } => {
            in_game(world)?;
            let play = match on {
                Switch::On => {
                    world.resource_mut::<Still>().0 = true;
                    Play::Paused
                }
                Switch::Off => Play::Running,
            };
            world.resource_mut::<NextState<Play>>().set(play);
            Ok(Outcome::say(format!("{play:?}").to_lowercase()))
        }
        Command::Screenshot { file } => {
            screenshot::save(world, file.clone());
            Ok(Outcome::say(format!("saving {}", file.display())))
        }
        Command::Record { dir, frames } => {
            in_game(world)?;
            std::fs::create_dir_all(&dir)
                .map_err(|e| format!("creating {}: {e}", dir.display()))?;
            let line = format!("recording {frames} frames to {}", dir.display());
            world.insert_resource(Recording::new(dir, 0, frames, false));
            Ok(Outcome::say(line))
        }
        Command::Benchmark { seconds } => {
            in_game(world)?;
            world.insert_resource(Benchmark::new(seconds, false));
            Ok(Outcome::say(format!("measuring for {seconds} s")))
        }
        Command::Window {
            mode,
            width,
            height,
        } => {
            let mut keys = vec!["display.mode"];
            let mut settings = world.resource_mut::<Settings>();
            settings.display.mode = mode;
            if let Some(width) = width {
                settings.display.width = width;
                keys.push("display.width");
            }
            if let Some(height) = height {
                settings.display.height = height;
                keys.push("display.height");
            }
            save(world, &keys)?;
            Ok(Outcome::say(format!("{mode:?}").to_lowercase()))
        }
        Command::Overlay { on } => panel(world, on, "debug.overlay", false),
        Command::Log { on } => panel(world, on, "debug.log", true),
        Command::Keys { on } => {
            let mut help = world.get_resource_mut::<Help>().ok_or("no help")?;
            help.shown = match on {
                Some(Switch::On) => true,
                Some(Switch::Off) => false,
                None => !help.shown,
            };
            Ok(Outcome::say(if help.shown { "shown" } else { "hidden" }))
        }
        Command::Console { on } => {
            let mut console = world.get_resource_mut::<Console>().ok_or("no console")?;
            let open = match on {
                Some(Switch::On) => true,
                Some(Switch::Off) => false,
                None => !console.is_open(),
            };
            console.set_open(open);
            Ok(Outcome::say(if open { "open" } else { "closed" }))
        }
        Command::Creatures { radius } => {
            in_game(world)?;
            Ok(Outcome {
                lines: debug::creatures(world, radius.unwrap_or(20))?,
                wait: None,
            })
        }
        Command::Stats { id } => {
            in_game(world)?;
            let id = id.map(|id| debug::creature(world, &id)).transpose()?;
            Ok(Outcome {
                lines: debug::stats(world, id)?,
                wait: None,
            })
        }
        Command::Spawn {
            creature,
            at,
            count,
            level,
            rank,
            hostile,
            peaceful,
            immortal,
        } => {
            in_game(world)?;
            let centre = cell_at(world, at)?;
            let hero = hero_cell(world)?;
            // About the hero, on its floor; at a cell it names, on the ground.
            let on = match at {
                None | Some(At::By(..)) => world
                    .query::<&Body>()
                    .iter(world)
                    .next()
                    .and_then(Body::level),
                Some(At::Cell(..)) => None,
            };
            let heights = heights(world)?;
            let floor = heights.floor(on).map_or(0, |(_, floor)| i32::from(floor));
            let cells: Vec<(u32, u32)> = debug::rings(centre, count, hero)
                .into_iter()
                .filter_map(|(x, y)| Some((u32::try_from(x).ok()?, u32::try_from(y).ok()?)))
                .collect();
            let request = npcs::Spawn {
                creature,
                cells,
                floor,
                level,
                rank: match rank {
                    None => fight::Rank::Common,
                    Some(RankArg::Champion) => fight::Rank::Champion,
                    Some(RankArg::Boss) => fight::Rank::Boss,
                },
                hostile: match (hostile, peaceful) {
                    (true, _) => Some(true),
                    (_, true) => Some(false),
                    _ => None,
                },
                immortal,
                roams: false,
            };
            let made = world
                .run_system_cached_with(npcs::spawn_creatures, request)
                .map_err(|e| e.to_string())??;
            let ids: Vec<String> = made.iter().map(ToString::to_string).collect();
            Ok(Outcome::say(format!(
                "{} creatures of type {creature} about {},{}: {}",
                made.len(),
                centre.0,
                centre.1,
                ids.join(" ")
            )))
        }
        Command::Kill { radius } => {
            in_game(world)?;
            hero_cell(world)?;
            let lines = world
                .run_system_cached_with(fight::kill, radius.unwrap_or(5))
                .map_err(|e| e.to_string())??;
            Ok(Outcome { lines, wait: None })
        }
        Command::Item {
            item,
            at,
            level,
            key,
            amount,
        } => {
            in_game(world)?;
            let (x, y) = cell_at(world, at)?;
            // About the hero, on its floor; at a cell it names, on the ground.
            let on = match at {
                None | Some(At::By(..)) => world
                    .query::<&Body>()
                    .iter(world)
                    .next()
                    .and_then(Body::level),
                Some(At::Cell(..)) => None,
            };
            let mut point = iso::ground(x as f32 + 0.5, y as f32 + 0.5);
            if let Some(height) = heights(world)?.standing(point.x, point.z, on) {
                point.y = height;
            }
            let throw = crate::drops::Throw {
                item,
                at: (point, on),
                level,
                key,
                amount,
            };
            let line = world
                .run_system_cached_with(crate::drops::throw_item, throw)
                .map_err(|e| e.to_string())??;
            Ok(Outcome::say(line))
        }
        Command::Level { level } => {
            in_game(world)?;
            let start = match level {
                0 | 1 => 0,
                level => opensacred_rules::fight::level_end(level - 1),
            };
            Ok(Outcome::say(experience(world, start, false)?))
        }
        Command::Experience { points } => {
            in_game(world)?;
            let (add, number) = match points.strip_prefix('+') {
                Some(number) => (true, number),
                None => (false, points.as_str()),
            };
            let number: u64 = number
                .parse()
                .map_err(|e| format!("{points}: {e} (N or +N)"))?;
            Ok(Outcome::say(experience(world, number, add)?))
        }
        Command::Attack { id } => {
            in_game(world)?;
            let id = id.map(|id| debug::creature(world, &id)).transpose()?;
            Ok(Outcome::say(attack(world, id)?))
        }
        Command::Take => {
            in_game(world)?;
            Ok(Outcome::say(crate::pickup::take_nearest(world)?))
        }
        Command::Talk { id } => {
            in_game(world)?;
            let id = id.map(|id| debug::creature(world, &id)).transpose()?;
            Ok(Outcome::say(crate::talk::talk_to(world, id)?))
        }
        Command::Answer { button } => {
            in_game(world)?;
            Ok(Outcome::say(crate::talk::answer(world, button)?))
        }
        Command::Heal { id, to } => {
            in_game(world)?;
            let id = id.map(|id| debug::creature(world, &id)).transpose()?;
            Ok(Outcome::say(debug::heal(world, id, to)?))
        }
        Command::Effect {
            effect,
            value,
            whole,
            seconds,
            art,
            level,
            id,
        } => {
            in_game(world)?;
            let id = id.map(|id| debug::creature(world, &id)).transpose()?;
            let effect = opensacred_rules::fight::Effect {
                id: effect,
                whole,
                value,
                art,
                level,
            };
            Ok(Outcome::say(debug::effect(world, id, effect, seconds)?))
        }
        Command::God { on } => {
            in_game(world)?;
            let on = hero_switch::<fight::Immortal>(world, on)?;
            Ok(Outcome::say(if on {
                "the hero takes no damage"
            } else {
                "the hero takes damage"
            }))
        }
        Command::Fly { on } => {
            in_game(world)?;
            let on = hero_switch::<Flies>(world, on)?;
            Ok(Outcome::say(if on {
                "the hero flies"
            } else {
                "the hero walks"
            }))
        }
        Command::Drop { times } => {
            in_game(world)?;
            let line = crate::drops::drop_nearest(world, times.unwrap_or(1))?;
            Ok(Outcome::say(line))
        }
        Command::Script { function } => {
            in_game(world)?;
            Ok(Outcome {
                lines: crate::scripts::run_function(world, &function)?,
                wait: None,
            })
        }
        Command::Quest { action, quest } => {
            in_game(world)?;
            let transition = match action {
                QuestAction::SetUp => opensacred_rules::quests::Transition::SetUp,
                QuestAction::Trigger => opensacred_rules::quests::Transition::Trigger,
            };
            Ok(Outcome {
                lines: crate::scripts::run_transition(world, transition, quest)?,
                wait: None,
            })
        }
        Command::Arts { action } => {
            in_game(world)?;
            let mut first = Vec::new();
            {
                let mut arts = world
                    .get_resource_mut::<crate::arts::HeroArts>()
                    .ok_or("no game is played")?;
                match action {
                    Some(ArtsAction::Learn { art, levels }) => {
                        arts.learn(art, levels.unwrap_or(1))?;
                    }
                    Some(ArtsAction::Ready) => arts.ready(),
                    Some(ArtsAction::Use { art }) => arts.start(art),
                    Some(ArtsAction::Select { .. } | ArtsAction::Make { .. }) | None => {}
                }
            }
            match action {
                Some(ArtsAction::Select { art }) => {
                    let arts = world.resource::<crate::arts::HeroArts>();
                    if arts.arts.get(art).is_none() || opensacred_rules::arts::style(art).is_none()
                    {
                        return Err(format!("the hero holds no art {art} whose action is read"));
                    }
                    world.resource_mut::<crate::making::Selected>().0 = Some(art);
                }
                Some(ArtsAction::Make { art }) => {
                    let art = art
                        .or(world.resource::<crate::making::Selected>().0)
                        .ok_or("no art is selected")?;
                    let mut heroes =
                        world.query_filtered::<Option<&crate::effects::Effects>, With<Body>>();
                    let effects = heroes.iter(world).next().flatten().cloned();
                    // The art of the hero's form for the one asked, as the right button takes it
                    // (`0x552370`).
                    let kind = world.resource::<crate::arts::HeroArts>().kind;
                    let art = opensacred_rules::arts::right_button(art, kind, |key| {
                        effects
                            .as_ref()
                            .is_some_and(|effects| effects.holds_any(&[key]))
                    });
                    crate::making::may_make(
                        world.resource::<crate::arts::HeroArts>(),
                        art,
                        effects.as_ref(),
                    )?;
                    first.push(make_art(world, art)?);
                }
                _ => {}
            }
            let mut lines = first;
            lines.extend(world.resource::<crate::arts::HeroArts>().lines());
            if let Some(art) = world.resource::<crate::making::Selected>().0 {
                lines.push(format!("selected {art}"));
            }
            Ok(Outcome { lines, wait: None })
        }
        Command::State => {
            let snapshot = world
                .run_system_cached(|probe: Probe| probe.snapshot())
                .map_err(|e| e.to_string())?;
            Ok(Outcome {
                lines: snapshot.lines(),
                wait: None,
            })
        }
        Command::Wait { until } => Ok(Outcome {
            lines: Vec::new(),
            wait: Some(until),
        }),
        Command::Exec { file } => {
            let text = std::fs::read_to_string(&file)
                .map_err(|e| format!("reading {}: {e}", file.display()))?;
            let lines = script_lines(&text);
            let count = lines.len();
            let mut script = world.resource_mut::<Script>();
            for line in lines.into_iter().rev() {
                script.lines.push_front(line);
            }
            Ok(Outcome::say(format!(
                "running {}: {count} commands",
                file.display()
            )))
        }
    }
}

/// What the last word of `input` may become: the commands' names for the first word, then the
/// command's `--options`, or the values its arguments take (the classes, on and off, …).
pub fn complete(input: &str) -> Vec<String> {
    let line = Line::command();
    let words: Vec<&str> = input.split_whitespace().collect();
    let last = if input.ends_with(char::is_whitespace) {
        ""
    } else {
        words.last().copied().unwrap_or_default()
    };
    let mut found: Vec<String> = if words.len() <= 1 && !input.ends_with(char::is_whitespace) {
        line.get_subcommands()
            .map(|command| command.get_name().to_owned())
            .filter(|name| name.starts_with(last))
            .collect()
    } else {
        let Some(command) = words.first().and_then(|name| line.find_subcommand(name)) else {
            return Vec::new();
        };
        if last.starts_with('-') {
            command
                .get_arguments()
                .filter_map(|arg| arg.get_long())
                .map(|long| format!("--{long}"))
                .filter(|long| long.starts_with(last))
                .collect()
        } else {
            let wanted = words
                .iter()
                .rev()
                .nth(usize::from(!last.is_empty()))
                .copied()
                .unwrap_or_default();
            // The values of the option before the word, or of any argument of the command.
            let values = |arg: &clap::Arg| -> Vec<String> {
                arg.get_possible_values()
                    .iter()
                    .map(|value| value.get_name().to_owned())
                    .collect()
            };
            let option = wanted.strip_prefix("--").and_then(|long| {
                command
                    .get_arguments()
                    .find(|arg| arg.get_long() == Some(long))
            });
            match option {
                Some(arg) => values(arg),
                None => command
                    .get_arguments()
                    .filter(|arg| arg.is_positional())
                    .flat_map(values)
                    .collect(),
            }
            .into_iter()
            .filter(|value| value.starts_with(last))
            .collect()
        }
    };
    found.sort();
    found.dedup();
    found
}

/// The commands of a script's text: a line each, without comments and blank lines.
pub fn script_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| line.split('#').next().unwrap_or_default().trim())
        .filter(|line| !line.is_empty())
        .map(Into::into)
        .collect()
}

/// The commands of `--exec`: between semicolons.
pub fn exec_lines(text: &str) -> Vec<String> {
    text.split(';')
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(Into::into)
        .collect()
}

/// What a script waits for now.
enum Waiting {
    Ready,
    Arrived,
    Frames(u32),
    /// Until the real time is this far on.
    Until(Duration),
}

/// The script running: the lines still to run, and what the next waits for.
#[derive(Resource, Default)]
pub struct Script {
    lines: VecDeque<String>,
    waiting: Option<Waiting>,
}

impl Script {
    /// Adds `lines` at the end.
    pub fn push(&mut self, lines: impl IntoIterator<Item = String>) {
        self.lines.extend(lines);
    }
}

/// Whether what the script waits for has come; it counts a frame of a wait for frames. A script
/// also waits for the screenshots it asked for to be written, so that a `quit` or a teleport
/// after a `screenshot` cannot come before its frame (#202).
fn waited(world: &mut World) -> bool {
    if world
        .get_resource::<screenshot::Saving>()
        .is_some_and(screenshot::Saving::pending)
    {
        return false;
    }
    let done = match &world.resource::<Script>().waiting {
        None => return true,
        Some(Waiting::Ready) => {
            // A hero that a command put somewhere has landed there (`hero::land`).
            let hero = world
                .query::<(&Transform, &Body)>()
                .iter(world)
                .next()
                .filter(|(_, body)| !body.landing())
                .map(|(transform, _)| transform.translation);
            in_game(world).is_ok()
                && world.get_resource::<Ready>().is_some_and(|ready| ready.0)
                && hero
                    .zip(world.get_resource::<Streamer>())
                    .is_some_and(|(hero, streamer)| streamer.settled(hero))
        }
        Some(Waiting::Arrived) => {
            let mut bodies = world.query::<&Body>();
            bodies.iter(world).next().is_some_and(|body| {
                body.target().is_none() && !body.ordered() && body.moving().is_none()
            })
        }
        Some(Waiting::Frames(n)) => *n == 0,
        Some(Waiting::Until(at)) => world.resource::<Time<Real>>().elapsed() >= *at,
    };
    let mut script = world.resource_mut::<Script>();
    if done {
        script.waiting = None;
    } else if let Some(Waiting::Frames(n)) = &mut script.waiting {
        *n -= 1;
    }
    done
}

/// Runs the script's lines until one waits, at most a few hundred a frame, so that a script that
/// runs itself cannot hang the game.
fn script(world: &mut World) {
    for _ in 0..256 {
        if !waited(world) {
            return;
        }
        let Some(line) = world.resource_mut::<Script>().lines.pop_front() else {
            return;
        };
        info!("> {line}");
        match run(world, &line) {
            Ok(outcome) => {
                for line in &outcome.lines {
                    info!("{line}");
                }
                let Some(wait) = outcome.wait else {
                    continue;
                };
                let elapsed = world.resource::<Time<Real>>().elapsed();
                world.resource_mut::<Script>().waiting = Some(match wait {
                    Wait::Ready => Waiting::Ready,
                    Wait::Arrived => Waiting::Arrived,
                    Wait::Frames { n } => Waiting::Frames(n),
                    Wait::Seconds { s } => {
                        Waiting::Until(elapsed + Duration::from_secs_f32(s.max(0.0)))
                    }
                });
                // The wait starts with the next frame: `wait frames 3` lets three render.
                return;
            }
            Err(e) => error!("{line}: {e}"),
        }
    }
}

/// The scripts.
pub struct CommandPlugin;

impl Plugin for CommandPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Script>().add_systems(Update, script);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Class;

    fn parse(line: &str) -> Result<Command, clap::Error> {
        Line::try_parse_from(words(line).unwrap()).map(|line| line.command)
    }

    #[test]
    fn the_commands_take_the_command_lines_options() {
        let Ok(Command::NewGame(game)) =
            parse("new-game --class wood-elf --sector 50,39 --time 20:00 --walking --no-music")
        else {
            panic!("not new-game");
        };
        assert_eq!(game.class, Some(Class::WoodElf));
        assert_eq!(game.sector, Some((50, 39)));
        assert!(game.walking && game.no_music);
        assert!(matches!(
            parse("go 3236,2510"),
            Ok(Command::Go {
                to: Goal::Cell(3236, 2510)
            })
        ));
        assert!(matches!(
            parse("go +10,-4"),
            Ok(Command::Go {
                to: Goal::By(10.0, -4.0)
            })
        ));
        assert!(matches!(
            parse("go -3,4"),
            Ok(Command::Go {
                to: Goal::By(-3.0, 4.0)
            })
        ));
        assert!(matches!(
            parse("sector 50,39"),
            Ok(Command::Sector { at: (50, 39) })
        ));
        assert!(matches!(
            parse("gait walk"),
            Ok(Command::Gait {
                gait: GaitArg::Walk
            })
        ));
        assert!(matches!(
            parse("window windowed 1280 720"),
            Ok(Command::Window {
                mode: Mode::Windowed,
                width: Some(1280),
                height: Some(720)
            })
        ));
        assert!(matches!(
            parse("overlay"),
            Ok(Command::Overlay { on: None })
        ));
        assert!(matches!(
            parse("wait frames 3"),
            Ok(Command::Wait {
                until: Wait::Frames { n: 3 }
            })
        ));
        assert!(matches!(
            parse("screenshot 'a b.png'"),
            Ok(Command::Screenshot { file }) if file == std::path::Path::new("a b.png")
        ));
        assert!(parse("time 25:00").is_err());
        assert!(parse("teleport 1,2").is_err());
    }

    #[test]
    fn a_windows_path_keeps_its_backslashes() {
        assert!(matches!(
            parse(r"screenshot C:\Temp\shot.png"),
            Ok(Command::Screenshot { file }) if file == std::path::Path::new(r"C:\Temp\shot.png")
        ));
        assert_eq!(
            words(r#"record "C:\My shots" 10"#).unwrap(),
            ["record", r"C:\My shots", "10"]
        );
        assert_eq!(words("a  '' \"b c\"").unwrap(), ["a", "", "b c"]);
        assert_eq!(words("  ").unwrap(), Vec::<String>::new());
        assert_eq!(words("say 'open"), None);
    }

    #[test]
    fn tab_completes_the_commands_their_options_and_values() {
        assert_eq!(complete("ze"), Vec::<String>::new());
        assert_eq!(complete("zo"), ["zoom"]);
        assert_eq!(
            complete("s"),
            [
                "screenshot",
                "script",
                "sector",
                "setfog",
                "setquake",
                "setrain",
                "setsnow",
                "spawn",
                "state",
                "stats"
            ]
        );
        assert_eq!(complete("gait "), ["run", "walk"]);
        assert_eq!(complete("music o"), ["off", "on"]);
        assert_eq!(complete("new-game --cl"), ["--class"]);
        assert_eq!(complete("new-game --class wo"), ["wood-elf"]);
        assert_eq!(complete("window b"), ["borderless"]);
        assert!(complete("nothing ").is_empty());
    }

    #[test]
    fn a_cell_is_named_or_an_offset_from_the_heros() {
        assert_eq!(parse_at("3253,2868"), Ok(At::Cell(3253, 2868)));
        assert_eq!(parse_at("+3,-2"), Ok(At::By(3, -2)));
        assert_eq!(parse_at("-1,0"), Ok(At::By(-1, 0)));
        assert!(parse_at("3253").is_err());
        let Line {
            command: Command::Cell { at, floor },
        } = Line::try_parse_from(["cell", "-4,+1", "2"]).unwrap()
        else {
            panic!("not a cell");
        };
        assert_eq!((at, floor), (At::By(-4, 1), Some(2)));
    }

    #[test]
    fn a_cell_tells_its_ground_floors_pocket_and_occupants() {
        let about = About {
            ground: 0x41,
            flags: 0x03,
            heights: [10.0, 12.4, 11.0, 9.0],
            standing: [0.0; 4],
            walkable: false,
            flown_over: false,
            entrance: false,
            bridged: false,
            building: Some((3250, 2860)),
            closed: false,
            inside: vec![crate::terrain::Inside {
                level: (50, 44, 2),
                floor: Some(((3250, 2860), 1)),
                walkable: true,
                flown_over: false,
                exit: false,
                stair: Some((1, [2, -1])),
                closed: true,
                room: true,
            }],
        };
        let lines = describe(
            (3253, 2867),
            &about,
            Some(37),
            vec!["464v1  hostile".into()],
        );
        assert_eq!(
            lines,
            [
                "cell 3253,2867  sector 50,44  cell 53,51 of it",
                "ground GRASS (0x41, flags 0x03), not walkable",
                "heights 10 12 11 9  standing 0 0 0 0",
                "of the building at 3250,2860",
                "inside 50,44:2, floor 1 of the building at 3250,2860, walkable, a room, a stair up \
                 1 by +2,-1, closed by a trigger",
                "a pocket: 37 places reached from it",
                "in it: 464v1  hostile",
            ]
        );
        let open = describe(
            (1, 1),
            &About {
                inside: Vec::new(),
                building: None,
                ..about
            },
            None,
            Vec::new(),
        );
        assert_eq!(open[3], "of no building");
        assert_eq!(
            open[4..],
            [
                "no pocket: the hero walks on from it",
                "nothing stands in it"
            ]
        );
    }

    #[test]
    fn scripts_split_into_commands() {
        assert_eq!(
            exec_lines("zoom 2;  wait frames 3 ;; zoom 0.5"),
            ["zoom 2", "wait frames 3", "zoom 0.5"]
        );
        assert_eq!(
            script_lines("# a script\nzoom 2\n\n  state # and a comment\n"),
            ["zoom 2", "state"]
        );
    }

    /// An app of the commands, its settings in a directory of the test's `name` and the
    /// process's id, emptied first: an earlier run whose process had the same id left its
    /// settings there (#484).
    fn app(name: &str) -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::state::app::StatesPlugin,
            CommandPlugin,
            crate::overlay::OverlayPlugin,
        ))
        .insert_state(AppState::InGame)
        .add_plugins(crate::pause::PausePlugin)
        .init_resource::<ButtonInput<KeyCode>>();
        let dir = std::env::temp_dir().join(format!(
            "opensacred-game-command-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let (file, settings) = SettingsFile::read(Some(dir.join("settings.toml")));
        app.insert_resource(file).insert_resource(settings);
        app.world_mut().spawn((
            Camera3d::default(),
            Projection::Orthographic(OrthographicProjection::default_3d()),
        ));
        app.insert_resource(Clock::default());
        app.update();
        app
    }

    fn zoom(app: &mut App) -> f32 {
        let mut cameras = app.world_mut().query::<&Projection>();
        match cameras.iter(app.world()).next() {
            Some(Projection::Orthographic(ortho)) => ortho.scale,
            _ => 0.0,
        }
    }

    #[test]
    fn commands_change_the_game_and_say_so() {
        let mut app = app("say");
        let world = app.world_mut();
        assert_eq!(run(world, "zoom 1.5").unwrap(), Outcome::say("zoom 1.50"));
        assert_eq!(run(world, "zoom 9").unwrap(), Outcome::say("zoom 2.00"));
        assert_eq!(
            run(world, "time 20:30").unwrap(),
            Outcome::say("the clock is set")
        );
        assert!((world.resource::<Clock>().minutes() - (20.0 * 60.0 + 30.0)).abs() < 1e-6);
        assert_eq!(run(world, "log on").unwrap(), Outcome::say("shown"));
        assert!(world.resource::<Settings>().debug.log);
        assert_eq!(run(world, "overlay").unwrap(), Outcome::say("shown"));
        assert!(
            run(world, "go 1,2")
                .unwrap_err()
                .contains("not in the world")
        );
        for line in [
            "cell 1,2",
            "cell +1,+2 0",
            "cell-info",
            "nearest",
            "spawn 76 --count 2",
            "kill",
            "item 1712",
            "heal",
            "effect 1 0.2",
            "god",
            "fly on",
        ] {
            assert!(
                run(world, line).unwrap_err().contains("not in the world"),
                "{line}"
            );
        }
        assert!(run(world, "nearest 5,5").unwrap_err().contains("no world"));
        assert!(
            run(world, "script StartCode")
                .unwrap_err()
                .contains("scripts do not run")
        );
        let help = run(world, "help").unwrap();
        assert!(help.lines.iter().any(|line| line.contains("new-game")));
        let state = run(world, "state").unwrap();
        assert!(state.lines[1].ends_with(" game"), "{state:?}");
        let error = run(world, "zoom twice").unwrap_err();
        assert!(error.contains("twice"), "{error}");
        assert_eq!(zoom(&mut app), 2.0);
    }

    #[test]
    fn a_new_game_keeps_how_the_command_line_shows_it() {
        let mut app = app("shows");
        let world = app.world_mut();
        let Ok(Command::NewGame(line)) = parse("new-game --zoom 1.5 --no-music") else {
            panic!("not new-game");
        };
        world.insert_resource(Shows::of(&line));
        run(world, "new-game --class mage").unwrap();
        let game = &world.resource::<Request>().0;
        assert_eq!(game.class, Some(Class::Mage));
        assert_eq!((game.zoom, game.no_music), (Some(1.5), true));
        run(world, "new-game --zoom 2").unwrap();
        assert_eq!(world.resource::<Request>().0.zoom, Some(2.0));
    }

    #[test]
    fn the_heros_level_and_experience_are_set_as_its_kills_would() {
        use opensacred_rules::fight::level_end;
        let mut app = app("experience");
        let world = app.world_mut();
        assert!(run(world, "level 5").unwrap_err().contains("no fight"));
        world.insert_resource(fight::Rules::default());
        world.insert_resource(fight::Experience::default());
        run(world, "level 5").unwrap();
        let experience = *world.resource::<fight::Experience>();
        assert_eq!((experience.level, experience.points), (5, level_end(4)));
        assert_eq!(experience.attribute_points, 4);
        // Down again, from the start: the points that level 5 gave are gone.
        run(world, "level 2").unwrap();
        let experience = *world.resource::<fight::Experience>();
        assert_eq!((experience.level, experience.attribute_points), (2, 1));
        // Added to, a level's end crossed; set again.
        run(
            world,
            &format!("experience +{}", level_end(2) - level_end(1)),
        )
        .unwrap();
        assert_eq!(world.resource::<fight::Experience>().level, 3);
        let line = run(world, "experience 0").unwrap();
        assert_eq!(world.resource::<fight::Experience>().level, 1);
        assert!(line.lines[0].starts_with("the hero is level 1 with 0 experience"));
        assert!(run(world, "experience -5").is_err());
    }

    #[test]
    fn a_script_waits_between_its_commands() {
        let mut app = app("script");
        app.world_mut()
            .resource_mut::<Script>()
            .push(exec_lines("zoom 2; wait frames 3; zoom 0.5"));
        app.update();
        assert_eq!(zoom(&mut app), 2.0);
        for _ in 0..3 {
            app.update();
            assert_eq!(zoom(&mut app), 2.0);
        }
        app.update();
        assert_eq!(zoom(&mut app), 0.5);
    }
}
