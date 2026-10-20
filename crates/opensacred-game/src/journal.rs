//! The log, readable in the game and after it (docs/10 §5). A `tracing` layer under Bevy's
//! `LogPlugin` keeps the last records in a ring, which F4 shows at the lower left and the console
//! and the remote protocol read, and writes every record to a file: a new one each start, the two
//! before kept, so that what came before a crash can be read after it.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{LineWriter, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use bevy::log::BoxedLayer;
use bevy::log::tracing::field::{Field, Visit};
use bevy::log::tracing::{Event, Level, Subscriber};
use bevy::log::tracing_subscriber::Layer;
use bevy::log::tracing_subscriber::layer::Context;
use bevy::prelude::*;
use bevy::text::FontSize;

use crate::settings::{Settings, SettingsFile};

/// The records the ring keeps.
const CAPACITY: usize = 1000;
/// The records the panel shows.
const SHOWN: usize = 15;
/// The panel's characters a line; the rest is cut.
const WIDTH: usize = 160;
/// The panel's font size on the original's 768-pixel-high screen (`hud`).
const FONT_SIZE: f32 = 10.0;
/// The panel's colours, in the frame's sRGB values (`hud`).
const BACK: Color = Color::linear_rgba(0.0, 0.0, 0.0, 0.6);
const TEXT: Color = Color::linear_rgb(0.85, 0.85, 0.85);
const WARNING: Color = Color::linear_rgb(0.95, 0.8, 0.3);
const ERROR: Color = Color::linear_rgb(1.0, 0.35, 0.3);

/// One record of the log.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    /// Its number since the start, which goes on past the ring's capacity.
    pub sequence: u64,
    /// Seconds since the start.
    pub seconds: f64,
    pub level: Level,
    pub target: String,
    pub message: String,
}

impl Record {
    /// The record as a line of the file: `   12.345 WARN  target: message`, a message's further
    /// lines indented under it.
    pub fn line(&self) -> String {
        format!(
            "{:9.3} {:<5} {}: {}",
            self.seconds,
            self.level,
            self.target,
            self.message.trim_end().replace('\n', "\n          ")
        )
    }

    /// The record as one line of the panel: shorter, the game's own targets without the crate,
    /// the message's lines and tabs as spaces.
    fn short(&self) -> String {
        let target = self
            .target
            .strip_prefix("opensacred_game::")
            .unwrap_or(&self.target);
        let message = self
            .message
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let line = format!("{:7.1} {:<5} {target}: {message}", self.seconds, self.level);
        // Three dots: the default font has no ellipsis.
        match line.char_indices().nth(WIDTH) {
            Some((end, _)) => format!("{}...", &line[..end]),
            None => line,
        }
    }
}

/// The last records, and the number of the next.
#[derive(Default)]
struct Ring {
    records: VecDeque<Record>,
    next: u64,
}

/// What the log shares with its `tracing` layer.
struct Shared {
    ring: Mutex<Ring>,
    file: Mutex<Option<LineWriter<File>>>,
    start: Instant,
    /// The file, if one was opened.
    path: Option<PathBuf>,
    /// Why the file could not be opened, logged once the log is up.
    problem: Option<String>,
}

/// The log: the ring of the last records and the file.
#[derive(Resource, Clone)]
pub struct Journal(Arc<Shared>);

impl Journal {
    /// A log writing to `path`, if any, after moving the files before it aside
    /// (`opensacred.log` to `opensacred.1.log`, that to `opensacred.2.log`).
    pub fn new(path: Option<PathBuf>) -> Self {
        let mut problem = None;
        let file = path.as_deref().and_then(|path| match open(path) {
            Ok(file) => Some(LineWriter::new(file)),
            Err(e) => {
                problem = Some(e);
                None
            }
        });
        Self(Arc::new(Shared {
            ring: Mutex::default(),
            path: file.as_ref().and(path),
            file: Mutex::new(file),
            start: Instant::now(),
            problem,
        }))
    }

    /// Adds a record to the ring and the file.
    pub fn push(&self, level: Level, target: &str, message: String) {
        let Ok(mut ring) = self.0.ring.lock() else {
            return;
        };
        let record = Record {
            sequence: ring.next,
            seconds: self.0.start.elapsed().as_secs_f64(),
            level,
            target: target.to_owned(),
            message,
        };
        if let Ok(mut file) = self.0.file.lock()
            && let Some(file) = file.as_mut()
        {
            // A record the file cannot take is still in the ring.
            let _ = writeln!(file, "{}", record.line());
        }
        ring.next += 1;
        ring.records.push_back(record);
        if ring.records.len() > CAPACITY {
            ring.records.pop_front();
        }
    }

    /// The number of the next record: it changes when one is added.
    pub fn next(&self) -> u64 {
        self.0.ring.lock().map(|ring| ring.next).unwrap_or_default()
    }

    /// The last `count` records, the oldest first.
    pub fn last(&self, count: usize) -> Vec<Record> {
        let Ok(ring) = self.0.ring.lock() else {
            return Vec::new();
        };
        let skip = ring.records.len().saturating_sub(count);
        ring.records.iter().skip(skip).cloned().collect()
    }

    /// The records numbered `after` and later that the ring still holds, the oldest first.
    pub fn since(&self, after: u64) -> Vec<Record> {
        let Ok(ring) = self.0.ring.lock() else {
            return Vec::new();
        };
        ring.records
            .iter()
            .filter(|record| record.sequence >= after)
            .cloned()
            .collect()
    }

    /// The `tracing` layer that feeds the log.
    pub fn layer(&self) -> BoxedLayer {
        Box::new(Capture(self.clone()))
    }
}

/// `LogPlugin::custom_layer`: the layer of the app's [`Journal`], which `main` inserts before
/// Bevy's plugins.
pub fn layer(app: &mut App) -> Option<BoxedLayer> {
    app.world().get_resource::<Journal>().map(Journal::layer)
}

/// Opens `path` for a new log, the files before it moved aside.
fn open(path: &Path) -> Result<File, String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    }
    let (older, old) = (numbered(path, 2), numbered(path, 1));
    // Missing files are as good as moved.
    let _ = fs::remove_file(&older);
    let _ = fs::rename(&old, &older);
    let _ = fs::rename(path, &old);
    File::create(path).map_err(|e| format!("creating {}: {e}", path.display()))
}

/// `name.N.ext` of `name.ext`.
fn numbered(path: &Path, n: u32) -> PathBuf {
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_else(|| "log".into());
    path.with_extension(format!("{n}.{extension}"))
}

/// The `tracing` layer: every event that Bevy's filter lets through becomes a record.
struct Capture(Journal);

impl<S: Subscriber> Layer<S> for Capture {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        let metadata = event.metadata();
        let target = fields.target.as_deref().unwrap_or(metadata.target());
        self.0.push(*metadata.level(), target, fields.message);
    }
}

/// An event's message, its other fields as `name=value` after it, and the target of a record of
/// the `log` crate, which comes as fields `log.*`.
#[derive(Default)]
struct Fields {
    message: String,
    target: Option<String>,
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "message" => self.message.insert_str(0, value),
            "log.target" => self.target = Some(value.to_owned()),
            name if name.starts_with("log.") => {}
            name => {
                let _ = write!(self.message, " {name}={value}");
            }
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "message" => self.message.insert_str(0, &format!("{value:?}")),
            name if name.starts_with("log.") => {}
            name => {
                let _ = write!(self.message, " {name}={value:?}");
            }
        }
    }
}

/// The panel of the last records.
#[derive(Component)]
struct Panel;

/// The panel's line `.0`, from the top.
#[derive(Component)]
struct Line(usize);

/// Logs where the log goes.
fn report(journal: Res<Journal>) {
    if let Some(path) = &journal.0.path {
        info!("log: {}", path.display());
    }
    if let Some(problem) = &journal.0.problem {
        warn!("log: {problem}; the log stays in the game");
    }
}

fn spawn_panel(mut commands: Commands, settings: Res<Settings>) {
    commands
        .spawn((
            Panel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(6.0),
                bottom: Val::Px(6.0),
                max_width: Val::Percent(75.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(BACK),
            GlobalZIndex(i32::MAX - 2),
            visibility(settings.debug.log),
        ))
        .with_children(|panel| {
            for i in 0..SHOWN {
                panel.spawn((
                    Line(i),
                    Text::new(""),
                    TextFont {
                        font_size: FontSize::Px(FONT_SIZE),
                        ..default()
                    },
                    TextColor(TEXT),
                ));
            }
        });
}

fn visibility(shown: bool) -> Visibility {
    if shown {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}

/// F4: the panel shown or hidden, kept in the settings.
fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<Settings>,
    mut file: ResMut<SettingsFile>,
) {
    if !keys.just_pressed(KeyCode::F4) {
        return;
    }
    settings.debug.log = !settings.debug.log;
    if let Err(e) = file.save(&settings, &["debug.log"]) {
        warn!("settings: {e:#}");
    }
}

/// Shows the panel as the settings say, and the last records in it when they change.
fn draw(
    settings: Res<Settings>,
    journal: Res<Journal>,
    mut drawn: Local<Option<u64>>,
    mut panel: Single<&mut Visibility, With<Panel>>,
    mut lines: Query<(&Line, &mut Text, &mut TextColor)>,
) {
    if settings.is_changed() {
        **panel = visibility(settings.debug.log);
        *drawn = None;
    }
    let next = journal.next();
    if !settings.debug.log || *drawn == Some(next) {
        return;
    }
    *drawn = Some(next);
    let records = journal.last(SHOWN);
    for (line, mut text, mut color) in &mut lines {
        let (content, tint) = match records.get(line.0) {
            Some(record) => (
                record.short(),
                match record.level {
                    Level::ERROR => ERROR,
                    Level::WARN => WARNING,
                    _ => TEXT,
                },
            ),
            None => (String::new(), TEXT),
        };
        if text.0 != content {
            text.0 = content;
        }
        color.0 = tint;
    }
}

/// The log's panel on F4; the [`Journal`] comes from `main`.
pub struct JournalPlugin;

impl Plugin for JournalPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (report, spawn_panel))
            .add_systems(Update, (toggle, draw).chain());
    }
}

#[cfg(test)]
mod tests {
    use bevy::log::tracing;
    use bevy::log::tracing_subscriber::Registry;
    use bevy::log::tracing_subscriber::layer::SubscriberExt;

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "opensacred-game-journal-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_ring_keeps_the_last_records_and_numbers_on() {
        let journal = Journal::new(None);
        for i in 0..CAPACITY + 5 {
            journal.push(Level::INFO, "test", format!("record {i}"));
        }
        assert_eq!(journal.next(), u64::try_from(CAPACITY).unwrap() + 5);
        let last = journal.last(3);
        assert_eq!(
            last.iter().map(|r| r.sequence).collect::<Vec<_>>(),
            [1002, 1003, 1004]
        );
        assert_eq!(last[2].message, "record 1004");
        // The first five went.
        let all = journal.last(usize::MAX);
        assert_eq!(all.len(), CAPACITY);
        assert_eq!(all[0].sequence, 5);
    }

    #[test]
    fn the_layer_records_the_events_and_the_file_keeps_them() {
        let dir = scratch("layer");
        let path = dir.join("opensacred.log");
        let journal = Journal::new(Some(path.clone()));
        let subscriber = Registry::default().with(journal.layer());
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("hello {}", 7);
            tracing::warn!(target: "opensacred_game::sectors", sector = 12, "slow");
        });
        let records = journal.last(10);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].level, Level::INFO);
        assert_eq!(records[0].message, "hello 7");
        assert_eq!(records[1].target, "opensacred_game::sectors");
        assert_eq!(records[1].message, "slow sector=12");
        assert!(
            records[1]
                .short()
                .ends_with(" WARN  sectors: slow sector=12")
        );
        let text = fs::read_to_string(&path).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[1].ends_with(" WARN  opensacred_game::sectors: slow sector=12"));
        drop(journal);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_message_of_several_lines_is_one_line_on_the_panel_and_indented_in_the_file() {
        let record = Record {
            sequence: 0,
            seconds: 1.25,
            level: Level::WARN,
            target: "gilrs::gamepad".into(),
            message: "No mapping\n\tDefault mapping will be used.\n".into(),
        };
        assert_eq!(
            record.short(),
            "    1.2 WARN  gilrs::gamepad: No mapping Default mapping will be used."
        );
        assert_eq!(
            record.line(),
            "    1.250 WARN  gilrs::gamepad: No mapping\n          \tDefault mapping will be used."
        );
        let long = Record {
            message: "x".repeat(300),
            ..record
        };
        assert_eq!(long.short().chars().count(), WIDTH + 3);
        assert!(long.short().ends_with("x..."));
    }

    #[test]
    fn a_new_log_moves_the_two_before_aside() {
        let dir = scratch("rotate");
        let path = dir.join("opensacred.log");
        for run in ["first", "second", "third", "fourth"] {
            let journal = Journal::new(Some(path.clone()));
            journal.push(Level::INFO, "test", run.into());
        }
        let message = |name: &str| {
            let text = fs::read_to_string(dir.join(name)).unwrap();
            text.split(": ").last().unwrap().trim().to_owned()
        };
        assert_eq!(message("opensacred.log"), "fourth");
        assert_eq!(message("opensacred.1.log"), "third");
        assert_eq!(message("opensacred.2.log"), "second");
        assert!(!dir.join("opensacred.3.log").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn f4_shows_the_last_records_and_the_settings_keep_it() {
        let dir = scratch("panel");
        let path = dir.join("settings.toml");
        let (file, settings) = SettingsFile::read(Some(path.clone()));
        let journal = Journal::new(None);
        let mut app = App::new();
        app.add_plugins(JournalPlugin)
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(file)
            .insert_resource(settings)
            .insert_resource(journal.clone());
        journal.push(Level::INFO, "opensacred_game::music", "peace".into());
        journal.push(Level::ERROR, "opensacred_game::npcs", "no model".into());
        app.update();
        let shown = |app: &mut App| {
            let mut panel = app.world_mut().query_filtered::<&Visibility, With<Panel>>();
            *panel.single(app.world()).unwrap() != Visibility::Hidden
        };
        assert!(!shown(&mut app));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F4);
        app.update();
        assert!(shown(&mut app));
        assert_eq!(fs::read_to_string(&path).unwrap(), "[debug]\nlog = true\n");
        let mut lines = app
            .world_mut()
            .query::<(&Line, &Text, &TextColor)>()
            .iter(app.world())
            .map(|(line, text, color)| (line.0, text.0.clone(), color.0))
            .collect::<Vec<_>>();
        lines.sort_by_key(|line| line.0);
        assert!(lines[0].1.ends_with("INFO  music: peace"), "{:?}", lines[0]);
        assert!(
            lines[1].1.ends_with("ERROR npcs: no model"),
            "{:?}",
            lines[1]
        );
        assert_eq!(lines[1].2, ERROR);
        assert!(lines[2].1.is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }
}
