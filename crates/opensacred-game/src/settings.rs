//! The game's settings, `settings.toml` in the user's configuration directory (docs/06 §6,
//! docs/10 §3): for now the display, borderless full screen on the monitor or a window of a given
//! size, which F11 switches between.
//!
//! The file is read once, at the start. A key the game sets is written back at once, alone, into
//! the document as it was read, so that the user's comments, order and unknown keys stay; a file
//! that does not read is never written.

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use bevy::prelude::*;
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode, WindowPosition};
use serde::{Deserialize, Serialize};
use toml_edit::{DocumentMut, Item, TableLike};

/// The settings file's name in the configuration directory.
const FILE: &str = "settings.toml";
/// The game's directory in the user's configuration directory.
const DIRECTORY: &str = "OpenSacred";

/// How the game shows on the screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Borderless full screen on the monitor the window is on, at its resolution.
    #[default]
    Borderless,
    /// A window of the display's width and height.
    Windowed,
}

impl Mode {
    /// The other mode, which F11 switches to.
    pub fn toggled(self) -> Self {
        match self {
            Mode::Borderless => Mode::Windowed,
            Mode::Windowed => Mode::Borderless,
        }
    }
}

/// `[display]`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Display {
    pub mode: Mode,
    /// The window's width in physical pixels, for [`Mode::Windowed`] only.
    pub width: u32,
    /// The window's height in physical pixels, for [`Mode::Windowed`] only.
    pub height: u32,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            mode: Mode::Borderless,
            width: 1920,
            height: 1080,
        }
    }
}

impl Display {
    /// Bevy's window mode for these settings, full screen on `monitor`: the primary one when the
    /// window is created, which has no monitor of its own yet, and its own after.
    pub fn window_mode(&self, monitor: MonitorSelection) -> WindowMode {
        match self.mode {
            Mode::Borderless => WindowMode::BorderlessFullscreen(monitor),
            Mode::Windowed => WindowMode::Windowed,
        }
    }
}

/// The settings in force: the file's, with the command line's for this run.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub display: Display,
}

/// The game's directory in the user's configuration directory: `%APPDATA%\OpenSacred` on Windows,
/// `$XDG_CONFIG_HOME/OpenSacred` (`~/.config/OpenSacred`) elsewhere.
pub fn config_dir() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        env_dir("APPDATA")
    } else {
        env_dir("XDG_CONFIG_HOME").or_else(|| env_dir("HOME").map(|home| home.join(".config")))
    };
    base.map(|base| base.join(DIRECTORY))
}

/// The absolute directory in the environment variable `name`.
fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

/// The settings file as read: the document that the settings' keys are written back into.
#[derive(Resource)]
pub struct SettingsFile {
    /// `None` when there is no configuration directory: nothing is written.
    path: Option<PathBuf>,
    document: DocumentMut,
    /// Whether the file read (or did not exist), so that writing it loses nothing.
    writable: bool,
    /// What was wrong with the file, logged once the log is up.
    warnings: Vec<String>,
}

impl SettingsFile {
    /// Reads `path`, or `settings.toml` in the configuration directory. The settings take their
    /// defaults where the file says nothing, and wholly when it does not read.
    pub fn read(path: Option<PathBuf>) -> (Self, Settings) {
        let mut file = Self {
            path: path.or_else(|| config_dir().map(|dir| dir.join(FILE))),
            document: DocumentMut::new(),
            writable: false,
            warnings: Vec::new(),
        };
        let Some(path) = file.path.clone() else {
            file.warnings
                .push("no configuration directory: the settings are not kept".into());
            return (file, Settings::default());
        };
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                file.writable = true;
                return (file, Settings::default());
            }
            Err(e) => {
                file.warnings.push(format!(
                    "reading {}: {e}; the defaults, and the file is left alone",
                    path.display()
                ));
                return (file, Settings::default());
            }
        };
        let read = text
            .parse::<DocumentMut>()
            .map_err(anyhow::Error::from)
            .and_then(|document| {
                let settings = toml_edit::de::from_document::<Settings>(document.clone())?;
                Ok((document, settings))
            });
        match read {
            Ok((document, settings)) => {
                file.warnings.extend(
                    unknown_keys(&document)
                        .into_iter()
                        .map(|key| format!("{}: unknown key {key}", path.display())),
                );
                file.document = document;
                file.writable = true;
                (file, settings)
            }
            Err(e) => {
                file.warnings.push(format!(
                    "{}: {e}; the defaults, and the file is left alone",
                    path.display()
                ));
                (file, Settings::default())
            }
        }
    }

    /// Writes the keys `keys` (`section.key`) of `settings` into the file, leaving everything else
    /// in it as it is.
    pub fn save(&mut self, settings: &Settings, keys: &[&str]) -> Result<()> {
        let Some(path) = &self.path else {
            bail!("no configuration directory");
        };
        if !self.writable {
            bail!("{} did not read, so it is not written", path.display());
        }
        let values = toml_edit::ser::to_document(settings)?;
        for key in keys {
            let (section, name) = key
                .split_once('.')
                .ok_or_else(|| anyhow!("{key}: not section.key"))?;
            let value = values
                .get(section)
                .and_then(|table| table.get(name))
                .and_then(Item::as_value)
                .ok_or_else(|| anyhow!("no setting {key}"))?
                .clone();
            let table = self
                .document
                .entry(section)
                .or_insert(toml_edit::table())
                .as_table_like_mut()
                .ok_or_else(|| anyhow!("{}: {section} is not a table", path.display()))?;
            match table.get_mut(name).and_then(Item::as_value_mut) {
                // The old value's comments stay with the new one.
                Some(old) => {
                    let decor = old.decor().clone();
                    *old = value;
                    *old.decor_mut() = decor;
                }
                None => {
                    table.insert(name, Item::Value(value));
                }
            }
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| anyhow!("creating {}: {e}", dir.display()))?;
        }
        // Written beside and renamed, so that a crash leaves the old file or the new one.
        let temporary = path.with_extension("toml.new");
        fs::write(&temporary, self.document.to_string())
            .map_err(|e| anyhow!("writing {}: {e}", temporary.display()))?;
        fs::rename(&temporary, path).map_err(|e| anyhow!("writing {}: {e}", path.display()))
    }
}

/// The keys of `document` that the settings do not have, as `section.key`.
fn unknown_keys(document: &DocumentMut) -> Vec<String> {
    fn walk(table: &dyn TableLike, known: &dyn TableLike, prefix: &str, out: &mut Vec<String>) {
        for (key, item) in table.iter() {
            let path = if prefix.is_empty() {
                key.to_owned()
            } else {
                format!("{prefix}.{key}")
            };
            match known.get(key) {
                None => out.push(path),
                Some(known) => {
                    if let (Some(table), Some(known)) =
                        (item.as_table_like(), known.as_table_like())
                    {
                        walk(table, known, &path, out);
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    if let Ok(known) = toml_edit::ser::to_document(&Settings::default()) {
        walk(document.as_table(), known.as_table(), "", &mut out);
    }
    out
}

/// Logs what was wrong with the settings file.
fn report(file: Res<SettingsFile>) {
    for warning in &file.warnings {
        warn!("settings: {warning}");
    }
}

/// F11: borderless full screen or the window, kept in the file.
fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<Settings>,
    mut file: ResMut<SettingsFile>,
) {
    if !keys.just_pressed(KeyCode::F11) {
        return;
    }
    settings.display.mode = settings.display.mode.toggled();
    if let Err(e) = file.save(&settings, &["display.mode"]) {
        warn!("settings: {e:#}");
    }
}

/// Puts the display's settings on the window when they change: the mode, and for a window its
/// size, centred on its monitor.
fn display(settings: Res<Settings>, mut window: Single<&mut Window, With<PrimaryWindow>>) {
    if !settings.is_changed() {
        return;
    }
    let display = &settings.display;
    let size = (display.width, display.height);
    let sized = (
        window.resolution.physical_width(),
        window.resolution.physical_height(),
    ) == size;
    let borderless = matches!(window.mode, WindowMode::BorderlessFullscreen(_));
    match display.mode {
        Mode::Borderless if borderless => return,
        Mode::Windowed if window.mode == WindowMode::Windowed && sized => return,
        _ => {}
    }
    window.mode = display.window_mode(MonitorSelection::Current);
    match display.mode {
        Mode::Borderless => info!("display: borderless full screen"),
        Mode::Windowed => {
            window.resolution.set_physical_resolution(size.0, size.1);
            window.position = WindowPosition::Centered(MonitorSelection::Current);
            info!("display: a window of {} x {}", size.0, size.1);
        }
    }
}

/// F11 and the display's settings on the window; `Settings` and `SettingsFile` come from `main`.
pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, report)
            .add_systems(Update, (toggle, display).chain());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory for one test's file.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "opensacred-game-settings-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn no_file_gives_the_defaults_and_is_written_on_the_first_change() {
        let dir = scratch("none");
        let path = dir.join("nested").join(FILE);
        let (mut file, mut settings) = SettingsFile::read(Some(path.clone()));
        assert_eq!(settings, Settings::default());
        assert_eq!(settings.display.mode, Mode::Borderless);
        assert!(file.warnings.is_empty());
        assert!(!path.exists());
        settings.display.mode = Mode::Windowed;
        file.save(&settings, &["display.mode"]).unwrap();
        // Only the key that changed.
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "[display]\nmode = \"windowed\"\n"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_settings_read_back_as_written() {
        let dir = scratch("round");
        let path = dir.join(FILE);
        let (mut file, _) = SettingsFile::read(Some(path.clone()));
        let settings = Settings {
            display: Display {
                mode: Mode::Windowed,
                width: 1280,
                height: 720,
            },
        };
        file.save(
            &settings,
            &["display.mode", "display.width", "display.height"],
        )
        .unwrap();
        let (file, read) = SettingsFile::read(Some(path));
        assert_eq!(read, settings);
        assert!(file.warnings.is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_that_does_not_read_is_never_written() {
        let dir = scratch("broken");
        let path = dir.join(FILE);
        for text in ["[display\nmode = 1", "[display]\nmode = \"sideways\"\n"] {
            fs::write(&path, text).unwrap();
            let (mut file, mut settings) = SettingsFile::read(Some(path.clone()));
            assert_eq!(settings, Settings::default());
            assert_eq!(file.warnings.len(), 1, "{:?}", file.warnings);
            settings.display.mode = Mode::Windowed;
            assert!(file.save(&settings, &["display.mode"]).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), text);
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn writing_a_key_keeps_the_comments_and_the_unknown_keys() {
        let dir = scratch("keep");
        let path = dir.join(FILE);
        let text = "# My settings\n\n[display]\n# How it shows\nmode = \"borderless\" # F11\nheight = 900\nbrightness = 2\n\n[other]\nanswer = 42\n";
        fs::write(&path, text).unwrap();
        let (mut file, mut settings) = SettingsFile::read(Some(path.clone()));
        assert_eq!(settings.display.height, 900);
        assert_eq!(settings.display.width, 1920);
        assert_eq!(file.warnings.len(), 2);
        assert!(file.warnings[0].ends_with("unknown key display.brightness"));
        assert!(file.warnings[1].ends_with("unknown key other"));
        settings.display.mode = settings.display.mode.toggled();
        file.save(&settings, &["display.mode"]).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            text.replace("\"borderless\"", "\"windowed\"")
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn f11_switches_the_window_and_the_file_keeps_its_mode() {
        let dir = scratch("f11");
        let path = dir.join(FILE);
        let (file, settings) = SettingsFile::read(Some(path.clone()));
        let mut app = App::new();
        app.add_plugins(SettingsPlugin)
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(file);
        let window = app
            .world_mut()
            .spawn((
                Window {
                    mode: settings.display.window_mode(MonitorSelection::Primary),
                    resolution: bevy::window::WindowResolution::new(3840, 2160)
                        .with_scale_factor_override(1.0),
                    ..default()
                },
                PrimaryWindow,
            ))
            .id();
        app.insert_resource(settings);
        app.update();
        let press = |app: &mut App| {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::F11);
            app.update();
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.release(KeyCode::F11);
            keys.clear();
            app.update();
        };
        let mode = |app: &App| app.world().get::<Window>(window).unwrap().mode;
        assert!(matches!(mode(&app), WindowMode::BorderlessFullscreen(_)));
        press(&mut app);
        assert_eq!(mode(&app), WindowMode::Windowed);
        let resolution = &app.world().get::<Window>(window).unwrap().resolution;
        assert_eq!(
            (resolution.physical_width(), resolution.physical_height()),
            (1920, 1080)
        );
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "[display]\nmode = \"windowed\"\n"
        );
        press(&mut app);
        assert!(matches!(mode(&app), WindowMode::BorderlessFullscreen(_)));
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "[display]\nmode = \"borderless\"\n"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_mode_is_bevys_window_mode() {
        let mut display = Display::default();
        assert_eq!(
            display.window_mode(MonitorSelection::Primary),
            WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
        );
        display.mode = display.mode.toggled();
        assert_eq!(
            display.window_mode(MonitorSelection::Primary),
            WindowMode::Windowed
        );
    }
}
