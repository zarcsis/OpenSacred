//! The packaged game's first start (#1498, #1494): when the player's assets are not there, or a
//! converter of another version made them, a window of its own runs the converter that the
//! release carries beside the game: `locate --json` finds the installation, `install --json`
//! converts it, its stages shown as a bar with the time left; with none found, or one refused,
//! the player picks the installation's folder; a failure says what failed. Once the assets are
//! made, the game starts again on them, as one process makes one window's event loop alone.

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, ExitCode, Stdio};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use bevy::log::{DEFAULT_FILTER, LogPlugin};
use bevy::picking::events::PointerClick;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::window::WindowResolution;
use opensacred_data::converted::{Converted, MARK};
use serde_json::Value;

/// The converter's run on the development machine, in seconds (docs/03 §2, #1497), by which the
/// bar moves before the run's own pace is known.
const RUN_SECONDS: f32 = 483.0;

/// Why the assets are to be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Need {
    /// They are there, of this version.
    Nothing,
    /// They are not there, or not whole.
    First,
    /// A converter of another version made them: the game was updated.
    Update { from: String },
}

/// What the assets in `assets` need for the game of `version`, by their mark.
pub fn need(assets: &Path, version: &str) -> Need {
    let Ok(text) = std::fs::read_to_string(assets.join(MARK)) else {
        return Need::First;
    };
    match ron::from_str::<Converted>(&text) {
        Ok(mark) if mark.version == version => Need::Nothing,
        Ok(mark) => Need::Update { from: mark.version },
        Err(_) => Need::First,
    }
}

/// What the converter's run tells.
#[derive(Debug, Clone, PartialEq)]
pub enum Told {
    /// `locate`: the installation that passed, if any.
    Located(Option<PathBuf>),
    /// `install`: a stage starts, its shares of the run at its start and end.
    Stage {
        stage: String,
        step: u64,
        steps: u64,
        done: f32,
        until: f32,
    },
    /// `install`: the assets are made.
    Done,
    /// The run failed, and why.
    Failed(String),
    /// A line of the converter's own, for the log.
    Said(String),
}

/// What a line of the converter's stdout tells: its JSON lines by their key, the others as said.
pub fn told(line: &str) -> Told {
    let Some(value) = line
        .starts_with('{')
        .then(|| serde_json::from_str::<Value>(line).ok())
        .flatten()
    else {
        return Told::Said(line.to_owned());
    };
    let share =
        |stage: &Value, key: &str| serde_json::from_value::<f32>(stage[key].clone()).unwrap_or(0.0);
    if let Some(stage) = value.get("stage") {
        return Told::Stage {
            stage: stage["stage"].as_str().unwrap_or_default().to_owned(),
            step: stage["step"].as_u64().unwrap_or(0),
            steps: stage["steps"].as_u64().unwrap_or(0),
            done: share(stage, "done"),
            until: share(stage, "until"),
        };
    }
    if value.get("done").is_some() {
        return Told::Done;
    }
    if let Some(error) = value.get("error") {
        let message = error["message"].as_str().unwrap_or("the conversion failed");
        return Told::Failed(message.to_owned());
    }
    if let Some(found) = value.get("installation") {
        return Told::Located(found.as_str().map(PathBuf::from));
    }
    Told::Said(line.to_owned())
}

/// The share of the run behind which its pace is trusted: the first stages are short, and the
/// check of the installation is quick once the system has read its files.
const PACE_FROM: f32 = 0.2;

/// The bar's share and the seconds left of a stage that runs from `done` to `until` and started
/// `in_stage` seconds ago, `elapsed` seconds into the run: the run's pace so far once
/// [`PACE_FROM`] of it is behind, the development machine's before; a stage's bar stops short of
/// its end.
pub fn estimate(done: f32, until: f32, in_stage: f32, elapsed: f32) -> (f32, f32) {
    let per_share = if done >= PACE_FROM && elapsed > in_stage {
        (elapsed - in_stage) / done
    } else {
        RUN_SECONDS
    };
    let stage_seconds = ((until - done) * per_share).max(1.0);
    let within = (in_stage / stage_seconds).min(0.95);
    let share = done + (until - done) * within;
    let left = ((1.0 - share) * per_share).max(0.0);
    (share, left)
}

/// What the screen shows.
#[derive(Debug, Clone, PartialEq)]
enum Phase {
    Locating,
    Choosing(String),
    Converting {
        stage: String,
        step: u64,
        steps: u64,
        done: f32,
        until: f32,
        since: Instant,
    },
    Failed(String),
    Made,
}

/// The run of the converter: its process, and what it tells.
#[derive(Resource)]
struct Run {
    child: Option<Child>,
    told: Mutex<Receiver<Told>>,
    started: Instant,
}

impl Drop for Run {
    /// A window closed or the game quit stops the conversion.
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[derive(Resource)]
struct Screen {
    phase: Phase,
    need: Need,
    shot: Option<PathBuf>,
}

#[derive(Component)]
struct Headline;
#[derive(Component)]
struct Detail;
#[derive(Component)]
struct Bar;
#[derive(Component)]
struct Fill;
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Action {
    Choose,
    Quit,
}

/// The converter beside the game.
fn converter() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let path = exe.with_file_name(format!(
        "opensacred-convert{}",
        std::env::consts::EXE_SUFFIX
    ));
    path.is_file().then_some(path)
}

/// Starts the converter with `args`, its stdout's lines told on a channel, its stderr gathered
/// into the failure it ends with.
fn start(args: &[OsString]) -> Result<Run, String> {
    let program = converter().ok_or_else(|| {
        "the converter, opensacred-convert, is not beside the game: unpack the release again"
            .to_owned()
    })?;
    let mut command = Command::new(&program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        /// `CREATE_NO_WINDOW` of `CreateProcess`.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("cannot run {}: {e}", program.display()))?;
    let (sender, receiver) = channel();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    std::thread::spawn(move || read(stdout, stderr, &sender));
    info!("first start: {} {:?}", program.display(), args);
    Ok(Run {
        child: Some(child),
        told: Mutex::new(receiver),
        started: Instant::now(),
    })
}

/// Tells each line of `stdout`, then, at its end, the failure that `stderr` holds, if any.
fn read(
    stdout: Option<ChildStdout>,
    stderr: Option<std::process::ChildStderr>,
    sender: &Sender<Told>,
) {
    let errors = std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(mut stderr) = stderr {
            let _ = stderr.read_to_string(&mut text);
        }
        text
    });
    let mut ended = false;
    if let Some(stdout) = stdout {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let told = told(line.trim_end());
            ended |= matches!(told, Told::Done | Told::Failed(_) | Told::Located(_));
            if sender.send(told).is_err() {
                return;
            }
        }
    }
    let errors = errors.join().unwrap_or_default();
    if !ended {
        let message = errors
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("the converter stopped")
            .trim_start_matches("Error: ")
            .to_owned();
        let _ = sender.send(Told::Failed(message));
    }
}

/// Starts the conversion of the installation `dir`.
fn install(dir: &Path) -> Result<Run, String> {
    start(&[
        "install".into(),
        "--json".into(),
        "--game-dir".into(),
        dir.as_os_str().to_owned(),
    ])
}

/// The words of a stage for the screen.
fn stage_words(stage: &str) -> &str {
    match stage {
        "verify" => "Checking the installation",
        "textures" => "Reading the textures",
        "models" => "Reading the models",
        "world" => "Reading the world",
        "sound" => "Reading the sound and the music",
        "text" => "Reading the texts",
        "video" => "Reading the cutscenes",
        "ui" => "Reading the interface",
        "netscript" => "Reading the scripts",
        "tables" => "Reading the tables",
        "heroes" => "Reading the heroes",
        "build" => "Building the game's assets",
        other => other,
    }
}

/// An error's message, which starts low and ends without a stop as the code's errors do, as a
/// sentence for the screen.
fn sentence(message: &str) -> String {
    let mut chars = message.trim().chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut text: String = first.to_uppercase().chain(chars).collect();
    if !text.ends_with(['.', '!', '?']) {
        text.push('.');
    }
    text
}

fn minutes(seconds: f32) -> String {
    let minutes = (seconds / 60.0).ceil();
    if minutes <= 1.0 {
        "About a minute left".to_owned()
    } else {
        format!("About {minutes:.0} minutes left")
    }
}

/// Runs the first start's window for `need`; once the assets are made, starts the game again
/// with the same arguments.
pub fn run(need: Need, hidden: bool, log_file: Option<PathBuf>, shot: Option<PathBuf>) -> ExitCode {
    let mut app = App::new();
    app.insert_resource(crate::journal::Journal::new(log_file));
    app.add_plugins(
        DefaultPlugins
            .set(LogPlugin {
                filter: format!("{DEFAULT_FILTER},gilrs::gamepad=error"),
                custom_layer: crate::journal::layer,
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "OpenSacred".into(),
                    resolution: WindowResolution::new(960, 540),
                    visible: !hidden,
                    ..default()
                }),
                ..default()
            }),
    )
    .insert_resource(ClearColor(Color::srgb(0.06, 0.05, 0.04)))
    .insert_resource(Screen {
        phase: Phase::Locating,
        need,
        shot,
    })
    .add_systems(Startup, (spawn, locate))
    .add_systems(Update, (pump, show, keys, photograph));
    let exit = app.run();
    // The window and the converter are gone with the app.
    drop(app);
    match exit {
        AppExit::Success => relaunch(),
        AppExit::Error(_) => ExitCode::FAILURE,
    }
}

/// Starts the game again with this run's arguments, on the assets now made.
fn relaunch() -> ExitCode {
    let Ok(exe) = std::env::current_exe() else {
        return ExitCode::FAILURE;
    };
    match Command::new(exe).args(std::env::args_os().skip(1)).spawn() {
        Ok(_) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

fn locate(mut commands: Commands, mut screen: ResMut<Screen>) {
    match start(&["locate".into(), "--json".into()]) {
        Ok(run) => {
            commands.insert_resource(run);
        }
        Err(message) => screen.phase = Phase::Failed(message),
    }
}

fn spawn(mut commands: Commands) {
    commands.spawn(Camera2d);
    let text = |size: f32| TextFont {
        font_size: FontSize::Px(size),
        ..default()
    };
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(18.0),
            ..default()
        })
        .with_children(|page| {
            page.spawn((Text::new("OpenSacred"), text(40.0), TextColor(GOLD)));
            page.spawn((Headline, Text::new(""), text(22.0), TextColor(PALE)));
            page.spawn((
                Bar,
                Node {
                    width: Val::Px(640.0),
                    height: Val::Px(14.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.18, 0.16, 0.13)),
            ))
            .with_child((
                Fill,
                Node {
                    width: Val::Percent(0.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(GOLD),
            ));
            page.spawn((
                Detail,
                Text::new(""),
                text(16.0),
                TextColor(DIM),
                TextLayout::justify(Justify::Center),
                Node {
                    max_width: Val::Px(820.0),
                    ..default()
                },
            ));
            page.spawn(Node {
                column_gap: Val::Px(24.0),
                ..default()
            })
            .with_children(|row| {
                for (action, words) in [
                    (Action::Choose, "Choose the folder of Sacred Gold"),
                    (Action::Quit, "Quit"),
                ] {
                    row.spawn((
                        action,
                        Node {
                            padding: UiRect::axes(Val::Px(18.0), Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.24, 0.2, 0.14)),
                    ))
                    .observe(move |click: On<PointerClick>, mut commands: Commands| {
                        if click.button == PointerButton::Primary {
                            commands.run_system_cached_with(act, action);
                        }
                    })
                    .with_child((
                        Text::new(words),
                        text(18.0),
                        TextColor(PALE),
                    ));
                }
            });
        });
}

const GOLD: Color = Color::srgb(0.85, 0.7, 0.35);
const PALE: Color = Color::srgb(0.92, 0.89, 0.82);
const DIM: Color = Color::srgb(0.62, 0.58, 0.52);

/// A button's action: the folder's choice, or the end.
fn act(
    In(action): In<Action>,
    mut commands: Commands,
    mut screen: ResMut<Screen>,
    mut exit: MessageWriter<AppExit>,
) {
    match action {
        Action::Quit => {
            exit.write(AppExit::error());
        }
        Action::Choose => {
            let Some(dir) = pick_folder() else {
                return;
            };
            info!("first start: the player picked {}", dir.display());
            match install(&dir) {
                Ok(run) => {
                    commands.insert_resource(run);
                    screen.phase = Phase::Locating;
                }
                Err(message) => screen.phase = Phase::Failed(message),
            }
        }
    }
}

/// The installation's folder, as the player picks it.
#[cfg(windows)]
fn pick_folder() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("The folder of Sacred Gold 2.28")
        .pick_folder()
}

#[cfg(not(windows))]
fn pick_folder() -> Option<PathBuf> {
    None
}

/// Takes what the converter told.
fn pump(
    mut commands: Commands,
    run: Option<Res<Run>>,
    mut screen: ResMut<Screen>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(run) = run else {
        return;
    };
    let told: Vec<Told> = run
        .told
        .lock()
        .map(|receiver| receiver.try_iter().collect())
        .unwrap_or_default();
    for told in told {
        match told {
            Told::Said(line) => info!("convert: {line}"),
            Told::Located(Some(dir)) => {
                info!("first start: the installation in {}", dir.display());
                match install(&dir) {
                    Ok(next) => commands.insert_resource(next),
                    Err(message) => screen.phase = Phase::Failed(message),
                }
            }
            Told::Located(None) => {
                screen.phase = Phase::Choosing(
                    "No installation of Sacred Gold 2.28 (English) from GOG or Steam was found."
                        .to_owned(),
                );
            }
            Told::Stage {
                stage,
                step,
                steps,
                done,
                until,
            } => {
                info!("first start: {stage} ({step} of {steps})");
                screen.phase = Phase::Converting {
                    stage,
                    step,
                    steps,
                    done,
                    until,
                    since: Instant::now(),
                };
            }
            Told::Done => {
                info!(
                    "first start: the assets are made in {:.0} s",
                    run.started.elapsed().as_secs_f32()
                );
                screen.phase = Phase::Made;
                exit.write(AppExit::Success);
            }
            Told::Failed(message) => {
                warn!("first start: {message}");
                screen.phase = Phase::Failed(message);
            }
        }
    }
}

/// Shows the phase: its words, the bar and the buttons.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries name their filters"
)]
fn show(
    screen: Res<Screen>,
    run: Option<Res<Run>>,
    mut texts: ParamSet<(
        Query<&mut Text, With<Headline>>,
        Query<&mut Text, With<Detail>>,
    )>,
    mut fill: Query<&mut Node, (With<Fill>, Without<Bar>, Without<Action>)>,
    mut bar: Query<&mut Node, (With<Bar>, Without<Fill>, Without<Action>)>,
    mut buttons: Query<(&Action, &mut Node), (Without<Fill>, Without<Bar>)>,
) {
    let elapsed = run.map_or(0.0, |run| run.started.elapsed().as_secs_f32());
    let reason = match &screen.need {
        Need::Update { from } => {
            format!("The game was updated from {from}: its assets are made again from your copy.")
        }
        _ => "The game makes its assets from your copy of Sacred Gold 2.28, once.".to_owned(),
    };
    let (headline, detail, share, choose) = match &screen.phase {
        Phase::Locating => (
            "Looking for Sacred Gold 2.28...".to_owned(),
            reason,
            None,
            false,
        ),
        Phase::Choosing(why) => (
            why.clone(),
            "Choose the folder where Sacred Gold 2.28 is installed.".to_owned(),
            None,
            true,
        ),
        Phase::Converting {
            stage,
            step,
            steps,
            done,
            until,
            since,
        } => {
            let (share, left) = estimate(*done, *until, since.elapsed().as_secs_f32(), elapsed);
            (
                format!("{} ({step} of {steps})", stage_words(stage)),
                format!("{reason} {}.", minutes(left)),
                Some(share),
                false,
            )
        }
        Phase::Failed(message) => (
            "The assets could not be made.".to_owned(),
            sentence(message),
            None,
            true,
        ),
        Phase::Made => (
            "Starting the game...".to_owned(),
            String::new(),
            Some(1.0),
            false,
        ),
    };
    if let Ok(mut text) = texts.p0().single_mut()
        && text.0 != headline
    {
        text.0 = headline;
    }
    if let Ok(mut text) = texts.p1().single_mut()
        && text.0 != detail
    {
        text.0 = detail;
    }
    if let Ok(mut node) = bar.single_mut() {
        node.display = if share.is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let (Ok(mut node), Some(share)) = (fill.single_mut(), share) {
        node.width = Val::Percent(share * 100.0);
    }
    for (action, mut node) in &mut buttons {
        let shown = match action {
            Action::Choose => choose,
            Action::Quit => true,
        };
        node.display = if shown { Display::Flex } else { Display::None };
    }
}

/// Esc ends the first start, and the conversion with it.
fn keys(keys: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::error());
    }
}

/// With `--screenshot`, a frame of the screen once the conversion has run for a while, beside
/// the screenshot's file, for the checks (#1498).
fn photograph(mut commands: Commands, mut screen: ResMut<Screen>, time: Res<Time>) {
    let due = matches!(
        screen.phase,
        Phase::Converting { .. } | Phase::Choosing(_) | Phase::Failed(_)
    ) && time.elapsed() > Duration::from_secs(4);
    if !due {
        return;
    }
    if let Some(path) = screen.shot.take() {
        let name = format!(
            "{}-first-start.png",
            path.file_stem().unwrap_or_default().to_string_lossy()
        );
        commands
            .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
            .observe(bevy::render::view::screenshot::save_to_disk(
                path.with_file_name(name),
            ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_converters_lines_tell_their_events() {
        assert_eq!(
            told(r#"{"stage":{"stage":"sound","step":5,"steps":12,"done":0.25,"until":0.5}}"#),
            Told::Stage {
                stage: "sound".into(),
                step: 5,
                steps: 12,
                done: 0.25,
                until: 0.5
            }
        );
        assert_eq!(told(r#"{"done":{"assets":"C:\\a"}}"#), Told::Done);
        assert_eq!(
            told(r#"{"error":{"message":"no room"}}"#),
            Told::Failed("no room".into())
        );
        assert_eq!(
            told(r#"{"installation":"c:\\games\\sacred gold","places":[]}"#),
            Told::Located(Some(PathBuf::from(r"c:\games\sacred gold")))
        );
        assert_eq!(
            told(r#"{"installation":null,"places":[]}"#),
            Told::Located(None)
        );
        assert_eq!(
            told("textures: 25534 textures"),
            Told::Said("textures: 25534 textures".into())
        );
        assert_eq!(told("{ not json"), Told::Said("{ not json".into()));
    }

    #[test]
    fn an_error_shows_as_a_sentence() {
        assert_eq!(sentence("no room on c:"), "No room on c:.");
        assert_eq!(sentence(" it failed. "), "It failed.");
        assert_eq!(sentence(""), "");
    }

    #[test]
    fn the_bar_moves_within_a_stage_and_stops_short_of_its_end() {
        // Before any pace is known, by the development machine's run.
        let (share, left) = estimate(0.0, 0.1, 0.0, 0.0);
        // A quick first stage does not set the pace.
        let (_, early) = estimate(0.05, 0.1, 0.0, 1.0);
        assert!((early - 0.95 * RUN_SECONDS).abs() < 1.0);
        assert_eq!(share, 0.0);
        assert!((left - RUN_SECONDS).abs() < 1.0);
        let (share, _) = estimate(0.0, 0.1, 1000.0, 1000.0);
        assert!((share - 0.095).abs() < 1e-4);
        // A run half done in 100 s goes at 200 s the run.
        let (share, left) = estimate(0.5, 0.9, 40.0, 140.0);
        assert!((share - 0.7).abs() < 1e-4);
        assert!((left - 60.0).abs() < 1e-3);
    }

    #[test]
    fn the_assets_need_by_their_mark() {
        let dir =
            std::env::temp_dir().join(format!("opensacred-first-start-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(need(&dir, "1.0.0"), Need::First);
        std::fs::write(dir.join(MARK), "(version: \"1.0.0\", release: \"gog\")").unwrap();
        assert_eq!(need(&dir, "1.0.0"), Need::Nothing);
        assert_eq!(
            need(&dir, "1.1.0"),
            Need::Update {
                from: "1.0.0".into()
            }
        );
        std::fs::write(dir.join(MARK), "not ron").unwrap();
        assert_eq!(need(&dir, "1.0.0"), Need::First);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
