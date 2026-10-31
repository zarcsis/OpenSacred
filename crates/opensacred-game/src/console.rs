//! The console (docs/10 §7): the key under Esc, by its place in any layout (`~` in an English one,
//! `Ё` in a Russian one), drops it over the top half of the screen and raises it again; Esc
//! raises it too. It shows the log, the commands' output with it, and a line of input whose
//! Enter runs a command (`command`); Up and Down walk the history, kept beside the settings
//! (`history.txt`, the last 100 lines), Tab completes, Ctrl+V pastes from the system's clipboard
//! and Ctrl+C copies the line to it, PageUp, PageDown and the wheel scroll. While it is open the
//! player's keys and mouse do nothing else (`frame::Control`); F7, F10, F11 and F12 still work, and the
//! game goes on under it.
//!
//! The line of input is the console's own rather than Bevy's text input widget: a console needs
//! a line, a cursor and the history.

use std::fs;
use std::path::PathBuf;

use bevy::clipboard::Clipboard;
use bevy::input::ButtonState;
use bevy::input::InputSystems;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::log::tracing::Level;
use bevy::prelude::*;
use bevy::text::FontSize;

use crate::command;
use crate::frame::HoldsControl;
use crate::journal::Journal;
use crate::settings::SettingsFile;

/// The lines of the log the console shows at once.
const LINES: usize = 26;
/// The lines of the history kept.
const HISTORY: usize = 100;
/// The console's font size on the original's 768-pixel-high screen (`hud`).
const FONT_SIZE: f32 = 11.0;
/// The console's colours, in the frame's sRGB values (`hud`).
const BACK: Color = Color::linear_rgba(0.02, 0.02, 0.03, 0.88);
const TEXT: Color = Color::linear_rgb(0.85, 0.85, 0.85);
const INPUT: Color = Color::linear_rgb(0.86, 0.74, 0.45);
const WARNING: Color = Color::linear_rgb(0.95, 0.8, 0.3);
const ERROR: Color = Color::linear_rgb(1.0, 0.35, 0.3);

/// The console's state: whether it is open, the line of input and its cursor, the history and
/// where in it Up and Down are, and how far the view is scrolled up.
#[derive(Resource, Default)]
pub struct Console {
    open: bool,
    input: String,
    /// The cursor, in characters.
    cursor: usize,
    history: Vec<String>,
    /// The line of the history shown, counted from the newest.
    browsing: Option<usize>,
    /// Lines up from the newest.
    scroll: usize,
    /// Lines entered, to run.
    entered: Vec<String>,
    file: Option<PathBuf>,
}

impl Console {
    /// A console whose history is `file`, read from it if it is there.
    pub fn new(file: Option<PathBuf>) -> Self {
        let history = file
            .as_ref()
            .and_then(|file| fs::read_to_string(file).ok())
            .map(|text| text.lines().map(Into::into).collect())
            .unwrap_or_default();
        Self {
            history,
            file,
            ..Self::default()
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens or closes the console (the command `console`).
    pub fn set_open(&mut self, open: bool) {
        self.open = open;
    }

    /// The byte of the cursor in the line.
    fn at(&self) -> usize {
        self.input
            .char_indices()
            .nth(self.cursor)
            .map_or(self.input.len(), |(i, _)| i)
    }

    fn set(&mut self, line: String) {
        self.cursor = line.chars().count();
        self.input = line;
    }

    /// Takes the line entered into the history and the lines to run.
    fn enter(&mut self) {
        let line = std::mem::take(&mut self.input);
        self.cursor = 0;
        self.browsing = None;
        self.scroll = 0;
        if line.trim().is_empty() {
            return;
        }
        if self.history.last() != Some(&line) {
            self.history.push(line.clone());
            let extra = self.history.len().saturating_sub(HISTORY);
            self.history.drain(..extra);
            if let Some(file) = &self.file {
                // A history that cannot be kept is still the console's for this run.
                let _ = fs::write(file, self.history.join("\n") + "\n");
            }
        }
        self.entered.push(line);
    }

    /// Up (`older`) or Down through the history.
    fn browse(&mut self, older: bool) {
        let count = self.history.len();
        let next = match (self.browsing, older) {
            (None, true) if count > 0 => Some(0),
            (None, _) => None,
            (Some(i), true) => Some((i + 1).min(count - 1)),
            (Some(0), false) => None,
            (Some(i), false) => Some(i - 1),
        };
        self.browsing = next;
        match next {
            Some(i) => self.set(self.history[count - 1 - i].clone()),
            None => self.set(String::new()),
        }
    }

    /// Tab: completes the last word, as far as the candidates agree; when they part, they are
    /// logged.
    fn complete(&mut self) {
        let candidates = command::complete(&self.input);
        let Some(first) = candidates.first() else {
            return;
        };
        let common = candidates.iter().fold(first.clone(), |common, candidate| {
            common
                .chars()
                .zip(candidate.chars())
                .take_while(|(a, b)| a == b)
                .map(|(a, _)| a)
                .collect()
        });
        let start = self
            .input
            .rfind(char::is_whitespace)
            .map_or(0, |space| space + 1);
        let mut line = self.input[..start].to_owned();
        line.push_str(&common);
        if candidates.len() == 1 {
            line.push(' ');
        } else {
            info!(target: "console", "{}", candidates.join("  "));
        }
        self.set(line);
    }

    /// Puts `text` at the cursor, its lines and tabs as spaces: a command is a line.
    fn paste(&mut self, text: &str) {
        let text: String = text
            .replace(['\r', '\n', '\t'], " ")
            .chars()
            .filter(|c| !c.is_control())
            .collect();
        let text = text.trim();
        let at = self.at();
        self.input.insert_str(at, text);
        self.cursor += text.chars().count();
    }

    /// One key pressed while the console is open.
    fn key(&mut self, input: &KeyboardInput) {
        match &input.logical_key {
            Key::Enter => self.enter(),
            Key::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                let at = self.at();
                self.input.remove(at);
            }
            Key::Delete if self.cursor < self.input.chars().count() => {
                let at = self.at();
                self.input.remove(at);
            }
            Key::ArrowLeft => self.cursor = self.cursor.saturating_sub(1),
            Key::ArrowRight => self.cursor = (self.cursor + 1).min(self.input.chars().count()),
            Key::Home => self.cursor = 0,
            Key::End => self.cursor = self.input.chars().count(),
            Key::ArrowUp => self.browse(true),
            Key::ArrowDown => self.browse(false),
            Key::PageUp => self.scroll += LINES / 2,
            Key::PageDown => self.scroll = self.scroll.saturating_sub(LINES / 2),
            Key::Tab => self.complete(),
            _ => {
                if let Some(text) = &input.text
                    && text.chars().all(|c| !c.is_control())
                {
                    let at = self.at();
                    self.input.insert_str(at, text);
                    self.cursor += text.chars().count();
                }
            }
        }
    }
}

/// Whether the console is closed: the player's keys and mouse go to the game.
pub fn closed(console: Option<Res<Console>>) -> bool {
    console.is_none_or(|console| !console.open)
}

/// The key under Esc opens and closes the console, Esc closes it; while it is open, the keys
/// edit its line, and Ctrl+V and Ctrl+C, by their places, paste and copy through the system's
/// clipboard. It runs right after Bevy reads the input, and takes the keys it uses from the game:
/// Esc does not pause a game as it closes the console, nor hides the keys' help under it.
pub fn keys(
    mut console: ResMut<Console>,
    mut keys: crate::controls::KeysMut,
    mut input: MessageReader<KeyboardInput>,
    scroll: Res<AccumulatedMouseScroll>,
    mut clipboard: Option<ResMut<Clipboard>>,
) {
    let toggled = keys.just_pressed(crate::controls::Action::Console);
    let escaped = console.open && keys.just_pressed(crate::controls::Action::Back);
    if toggled || escaped {
        console.open = !console.open && !escaped;
        keys.clear(crate::controls::Action::Console);
        keys.clear(crate::controls::Action::Back);
        input.clear();
        return;
    }
    if !console.open {
        input.clear();
        return;
    }
    let control = keys.modifiers().control;
    for event in input.read() {
        if event.state != ButtonState::Pressed
            || keys.is_key(crate::controls::Action::Console, event.key_code)
        {
            continue;
        }
        match (control, event.key_code, clipboard.as_deref_mut()) {
            (true, KeyCode::KeyV, Some(clipboard)) => {
                match clipboard.fetch_text().poll_result() {
                    Some(Ok(text)) => console.paste(&text),
                    Some(Err(e)) => warn!(target: "console", "pasting: {e}"),
                    // A read the system has not answered yet (the web's): nothing to paste.
                    None => {}
                }
            }
            (true, KeyCode::KeyC, Some(clipboard)) => {
                if let Err(e) = clipboard.set_text(console.input.clone()) {
                    warn!(target: "console", "copying: {e}");
                }
            }
            _ => console.key(event),
        }
    }
    if scroll.delta.y > 0.0 {
        console.scroll += 3;
    } else if scroll.delta.y < 0.0 {
        console.scroll = console.scroll.saturating_sub(3);
    }
}

/// Runs the lines entered, their output and errors into the log, which the console shows.
fn run(world: &mut World) {
    let lines = std::mem::take(&mut world.resource_mut::<Console>().entered);
    for line in lines {
        info!(target: "console", "> {line}");
        match command::run(world, &line) {
            Ok(outcome) if outcome.wait.is_some() => {
                error!(target: "console", "{line}: wait is for scripts");
            }
            Ok(outcome) => {
                for line in outcome.lines {
                    info!(target: "console", "{line}");
                }
            }
            Err(e) => {
                for line in e.lines() {
                    error!(target: "console", "{line}");
                }
            }
        }
    }
}

/// The console's panel, its view of the log and its line of input.
#[derive(Component)]
struct Panel;

#[derive(Component)]
struct View;

#[derive(Component)]
struct Line;

fn spawn(mut commands: Commands) {
    commands
        .spawn((
            Panel,
            // The whole screen, so that the mouse reaches nothing under the console.
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            // Over everything, the keys' help too.
            GlobalZIndex(i32::MAX),
            Visibility::Hidden,
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(50.0),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::FlexEnd,
                        padding: UiRect::all(Val::Px(6.0)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(BACK),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        View,
                        Text::new(""),
                        TextFont {
                            font_size: FontSize::Px(FONT_SIZE),
                            ..default()
                        },
                        TextColor(TEXT),
                    ));
                    panel.spawn((
                        Line,
                        Text::new(""),
                        TextFont {
                            font_size: FontSize::Px(FONT_SIZE),
                            ..default()
                        },
                        TextColor(INPUT),
                    ));
                });
        });
}

/// What the console showed last: the log's record count, the line, the cursor, the scroll.
#[derive(Default, PartialEq)]
struct Shown(Option<(u64, String, usize, usize)>);

/// Shows the console as it is: its last records of the log, a span a record in its level's
/// colour, and the line with its cursor.
fn draw(
    mut commands: Commands,
    console: Res<Console>,
    journal: Res<Journal>,
    mut shown: Local<Shown>,
    mut panel: Single<&mut Visibility, With<Panel>>,
    view: Single<Entity, With<View>>,
    mut line: Single<&mut Text, With<Line>>,
) {
    let visibility = if console.open {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if **panel != visibility {
        **panel = visibility;
    }
    if !console.open {
        shown.0 = None;
        return;
    }
    let now = Shown(Some((
        journal.next(),
        console.input.clone(),
        console.cursor,
        console.scroll,
    )));
    if *shown == now {
        return;
    }
    *shown = now;
    let mut input = console.input.clone();
    input.insert(console.at(), '|');
    line.0 = format!("> {input}");
    // The records' lines, the newest last, up to the scroll.
    let records = journal.last(LINES + console.scroll);
    let mut lines: Vec<(String, Color)> = Vec::new();
    for record in &records {
        let colour = match record.level {
            Level::ERROR => ERROR,
            Level::WARN => WARNING,
            _ => TEXT,
        };
        let prefix = if record.target == "console" {
            String::new()
        } else {
            format!("{:<5} ", record.level)
        };
        for part in record
            .message
            .lines()
            .filter(|part| !part.trim().is_empty())
        {
            // The default font has no tab.
            lines.push((format!("{prefix}{}", part.replace('\t', "    ")), colour));
        }
    }
    let end = lines.len().saturating_sub(console.scroll);
    let start = end.saturating_sub(LINES);
    commands
        .entity(*view)
        .despawn_children()
        .with_children(|view| {
            for (text, colour) in &lines[start..end] {
                view.spawn((
                    TextSpan::new(format!("{text}\n")),
                    TextFont {
                        font_size: FontSize::Px(FONT_SIZE),
                        ..default()
                    },
                    TextColor(*colour),
                ));
            }
        });
}

/// The console: its key, its line, its view of the log and the commands it runs.
pub struct ConsolePlugin;

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        app.hold_control(closed)
            .add_systems(Startup, spawn)
            .add_systems(PreUpdate, keys.after(InputSystems))
            .add_systems(Update, (run, draw).chain());
    }
}

/// The console's history beside the settings file.
pub fn history_file(settings: &SettingsFile) -> Option<PathBuf> {
    settings
        .path()
        .and_then(|path| path.parent())
        .map(|dir| dir.join("history.txt"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(console: &mut Console, key: Key, text: Option<&str>) {
        console.key(&KeyboardInput {
            key_code: KeyCode::KeyA,
            logical_key: key,
            state: ButtonState::Pressed,
            text: text.map(Into::into),
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }

    fn write(console: &mut Console, text: &str) {
        for c in text.chars() {
            let s = c.to_string();
            typed(console, Key::Character(s.as_str().into()), Some(&s));
        }
    }

    #[test]
    fn the_line_edits_and_the_history_walks() {
        let mut console = Console::default();
        write(&mut console, "zom 1.5");
        typed(&mut console, Key::Home, None);
        for _ in 0..2 {
            typed(&mut console, Key::ArrowRight, None);
        }
        write(&mut console, "o");
        assert_eq!(console.input, "zoom 1.5");
        typed(&mut console, Key::End, None);
        typed(&mut console, Key::Backspace, None);
        write(&mut console, "ё");
        assert_eq!(console.input, "zoom 1.ё");
        typed(&mut console, Key::Backspace, None);
        write(&mut console, "2");
        typed(&mut console, Key::Enter, None);
        assert_eq!(console.entered, ["zoom 1.2"]);
        assert!(console.input.is_empty());
        write(&mut console, "state");
        typed(&mut console, Key::Enter, None);
        typed(&mut console, Key::ArrowUp, None);
        assert_eq!(console.input, "state");
        typed(&mut console, Key::ArrowUp, None);
        assert_eq!(console.input, "zoom 1.2");
        typed(&mut console, Key::ArrowUp, None);
        assert_eq!(console.input, "zoom 1.2");
        typed(&mut console, Key::ArrowDown, None);
        typed(&mut console, Key::ArrowDown, None);
        assert_eq!(console.input, "");
    }

    #[test]
    fn a_paste_goes_in_at_the_cursor_as_one_line() {
        let mut console = Console::default();
        write(&mut console, "new-game  --time 20:00");
        for _ in 0.." --time 20:00".chars().count() {
            typed(&mut console, Key::ArrowLeft, None);
        }
        console.paste("--class\r\n\tdwarf\n");
        assert_eq!(console.input, "new-game --class   dwarf --time 20:00");
        write(&mut console, "!");
        assert_eq!(console.input, "new-game --class   dwarf! --time 20:00");
    }

    #[test]
    fn tab_completes_as_far_as_the_candidates_agree() {
        let mut console = Console::default();
        write(&mut console, "zo");
        typed(&mut console, Key::Tab, None);
        assert_eq!(console.input, "zoom ");
        // The Dark Elf, the Daemon and the Dwarf agree on the d alone.
        console.set("new-game --class d".into());
        typed(&mut console, Key::Tab, None);
        assert_eq!(console.input, "new-game --class d");
        console.set("new-game --class w".into());
        typed(&mut console, Key::Tab, None);
        assert_eq!(console.input, "new-game --class wood-elf ");
    }

    #[test]
    fn the_history_is_kept_between_runs() {
        let file = std::env::temp_dir().join(format!(
            "opensacred-game-history-{}.txt",
            std::process::id()
        ));
        let _ = fs::remove_file(&file);
        let mut console = Console::new(Some(file.clone()));
        for i in 0..HISTORY + 3 {
            console.set(format!("zoom {i}"));
            console.enter();
        }
        let console = Console::new(Some(file.clone()));
        assert_eq!(console.history.len(), HISTORY);
        assert_eq!(console.history[0], "zoom 3");
        fs::remove_file(&file).unwrap();
    }

    #[test]
    fn the_open_console_holds_the_keys_back_and_esc_closes_it_without_a_pause() {
        use crate::frame::Control;
        #[derive(Resource, Default)]
        struct Controlled(u32);
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::input::InputPlugin));
        crate::frame::held_back(&mut app);
        app.add_plugins(ConsolePlugin)
            .insert_resource(Console::default())
            .insert_resource(Journal::new(None))
            .init_resource::<Controlled>()
            .add_systems(
                Update,
                (|keys: Res<ButtonInput<KeyCode>>, mut controlled: ResMut<Controlled>| {
                    if keys.just_pressed(KeyCode::Escape) {
                        controlled.0 += 1;
                    }
                })
                .in_set(Control),
            );
        app.update();
        let press = |app: &mut App, key: KeyCode| {
            app.world_mut().write_message(KeyboardInput {
                key_code: key,
                logical_key: Key::Escape,
                state: ButtonState::Pressed,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
            app.update();
            app.world_mut().write_message(KeyboardInput {
                key_code: key,
                logical_key: Key::Escape,
                state: ButtonState::Released,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
            app.update();
        };
        press(&mut app, KeyCode::Backquote);
        assert!(app.world().resource::<Console>().open);
        press(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<Console>().open);
        assert_eq!(app.world().resource::<Controlled>().0, 0);
        press(&mut app, KeyCode::Escape);
        assert_eq!(app.world().resource::<Controlled>().0, 1);
    }
}
