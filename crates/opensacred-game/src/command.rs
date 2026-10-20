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

use crate::benchmark::Benchmark;
use crate::console::Console;
use crate::daynight::{self, Clock};
use crate::game::{AppState, Data, NewGame, Request};
use crate::hero::{Body, Gait, Hero, Ready};
use crate::overlay::Probe;
use crate::pause::{Play, Still};
use crate::screenshot::{self, Recording};
use crate::sectors::Streamer;
use crate::settings::{Mode, Settings, SettingsFile};
use crate::terrain::Heights;
use crate::{atmosphere, iso, music};

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
    /// Put the hero at the centre of sector X,Y of the 100 x 100 grid
    Sector {
        #[arg(value_parser = parse_position)]
        at: (u32, u32),
    },
    /// Put the hero on global cell X,Y
    Cell {
        #[arg(value_parser = parse_position)]
        at: (u32, u32),
    },
    /// Send the hero to cell X,Y, or by +DX,+DY cells along X,Y (screen right-down, left-down),
    /// as a click does
    Go {
        #[arg(value_parser = parse_goal, allow_hyphen_values = true)]
        to: Goal,
    },
    /// Walk or run, as R
    Gait { gait: GaitArg },
    /// The camera's zoom, 0.5 to 2 as in the original
    Zoom { zoom: f32 },
    /// Set the clock, as HH:MM
    Time {
        #[arg(value_parser = daynight::parse_time)]
        time: Clock,
    },
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
    /// The state of the process, the game, the hero, the clock and the frames
    State,
    /// In a script: wait before the next command
    Wait {
        #[command(subcommand)]
        until: Wait,
    },
    /// Run a script: a command a line, # before a comment
    Exec { file: PathBuf },
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

/// Runs `line`: a command and its options. `help` and `--help` are output, not errors.
pub fn run(world: &mut World, line: &str) -> Result<Outcome, String> {
    let words = shlex::split(line).ok_or("unbalanced quotes")?;
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

fn set_state(world: &mut World, state: AppState) {
    world.resource_mut::<NextState<AppState>>().set(state);
}

/// Puts the hero at world point `position`, on the ground there once it is in.
fn teleport(world: &mut World, mut position: Vec3) -> Result<Outcome, String> {
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
    body.teleport(&mut transform, position);
    Ok(Outcome::say("the hero is there"))
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
        Command::Sector { at: (x, y) } => teleport(
            world,
            iso::ground((x as f32 + 0.5) * side, (y as f32 + 0.5) * side),
        ),
        Command::Cell { at: (x, y) } => {
            teleport(world, iso::ground(x as f32 + 0.5, y as f32 + 0.5))
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
                    ortho.scale = 1.0 / zoom;
                }
            }
            Ok(Outcome::say(format!("zoom {zoom:.2}")))
        }
        Command::Time { time } => {
            in_game(world)?;
            world.insert_resource(time);
            Ok(Outcome::say("the clock is set"))
        }
        Command::Music { on } => {
            in_game(world)?;
            match on {
                Switch::On if world.contains_resource::<music::Music>() => {
                    Ok(Outcome::say("the music is on"))
                }
                Switch::On => {
                    let assets = world.resource::<Data>().0.assets.clone();
                    let music = music::Music::read(&assets).map_err(|e| format!("{e:#}"))?;
                    let atmosphere =
                        atmosphere::Atmosphere::read(&assets).map_err(|e| format!("{e:#}"))?;
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

/// Whether what the script waits for has come; it counts a frame of a wait for frames.
fn waited(world: &mut World) -> bool {
    let done = match &world.resource::<Script>().waiting {
        None => return true,
        Some(Waiting::Ready) => {
            in_game(world).is_ok()
                && world.get_resource::<Ready>().is_some_and(|ready| ready.0)
                && world.get_resource::<Streamer>().is_some_and(Streamer::idle)
        }
        Some(Waiting::Arrived) => {
            let mut bodies = world.query::<&Body>();
            bodies
                .iter(world)
                .next()
                .is_some_and(|body| body.target().is_none() && body.moving().is_none())
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
        Line::try_parse_from(shlex::split(line).unwrap()).map(|line| line.command)
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
    fn tab_completes_the_commands_their_options_and_values() {
        assert_eq!(complete("ze"), Vec::<String>::new());
        assert_eq!(complete("zo"), ["zoom"]);
        assert_eq!(complete("s"), ["screenshot", "sector", "state"]);
        assert_eq!(complete("gait "), ["run", "walk"]);
        assert_eq!(complete("music o"), ["off", "on"]);
        assert_eq!(complete("new-game --cl"), ["--class"]);
        assert_eq!(complete("new-game --class wo"), ["wood-elf"]);
        assert_eq!(complete("window b"), ["borderless"]);
        assert!(complete("nothing ").is_empty());
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

    fn app() -> App {
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
        let dir =
            std::env::temp_dir().join(format!("opensacred-game-command-{}", std::process::id()));
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
            Some(Projection::Orthographic(ortho)) => 1.0 / ortho.scale,
            _ => 0.0,
        }
    }

    #[test]
    fn commands_change_the_game_and_say_so() {
        let mut app = app();
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
        let help = run(world, "help").unwrap();
        assert!(help.lines.iter().any(|line| line.contains("new-game")));
        let state = run(world, "state").unwrap();
        assert!(state.lines[1].ends_with(" game"), "{state:?}");
        let error = run(world, "zoom twice").unwrap_err();
        assert!(error.contains("twice"), "{error}");
        assert_eq!(zoom(&mut app), 2.0);
    }

    #[test]
    fn a_script_waits_between_its_commands() {
        let mut app = app();
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
