//! The game's keys (#1553, docs/10 §9): every action a key makes named once, with its keys, in
//! `controls.toml` beside `settings.toml`, the defaults the original's (its Quickstart's list and
//! `cEngine_receiveEvent`, `0x618090`), the game's own tools on the keys it leaves free (#1536).
//!
//! The file has a section for each place the keys work, `[world]`, `[interface]` and `[tools]`,
//! and in it an action to the list of its keys, each a key by its place on the keyboard (`I`, `1`,
//! `F1`, `Space`, `NumpadEnter`, …) with the modifiers held with it (`Shift+Space`). A key with
//! modifiers asks for them and lets the others be held too, but a key of more modifiers that are
//! held wins over it in its section. The file is written with the defaults where there is none;
//! an action it lacks takes its default, and a name it does not know is a warning, the default
//! kept. The editing of a line of text (the console's and the hero's name's) is not in it.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::LazyLock;

use anyhow::{Result, anyhow, bail};
use bevy::ecs::system::SystemParam;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use toml_edit::DocumentMut;

/// The file's name, beside the settings'.
const FILE: &str = "controls.toml";

/// Where an action's keys work: a section of the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    /// The game played in the world.
    World,
    /// The windows and the menus.
    Interface,
    /// The game's own tools, everywhere.
    Tools,
}

impl Section {
    const ALL: [Self; 3] = [Self::World, Self::Interface, Self::Tools];

    fn name(self) -> &'static str {
        match self {
            Self::World => "world",
            Self::Interface => "interface",
            Self::Tools => "tools",
        }
    }
}

/// An action that keys make.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    Walk,
    StrikeAtOnce,
    Labels,
    Inventory,
    QuestBook,
    WorldMap,
    Overview,
    WeaponSet1,
    WeaponSet2,
    WeaponSet3,
    WeaponSet4,
    WeaponSet5,
    ArtSlot1,
    ArtSlot2,
    ArtSlot3,
    ArtSlot4,
    ArtSlot5,
    ArtKey1,
    ArtKey2,
    ArtKey3,
    ArtKey4,
    ArtKey5,
    ArtKey6,
    BeltPotion,
    PurplePotion,
    BluePotion,
    GreenPotion,
    YellowPotion,
    FollowerPotion,
    Back,
    Confirm,
    Next,
    Previous,
    SendItem,
    Help,
    Overlay,
    Log,
    Window,
    Console,
}

/// The weapon sets' actions, the first to the fifth.
pub const WEAPON_SETS: [Action; 5] = [
    Action::WeaponSet1,
    Action::WeaponSet2,
    Action::WeaponSet3,
    Action::WeaponSet4,
    Action::WeaponSet5,
];
/// The art slots' actions, the first to the fifth.
pub const ART_SLOTS: [Action; 5] = [
    Action::ArtSlot1,
    Action::ArtSlot2,
    Action::ArtSlot3,
    Action::ArtSlot4,
    Action::ArtSlot5,
];
/// The arts' keys, the original's F1 to F6 (`0x6C1F30`).
pub const ART_KEYS: [Action; 6] = [
    Action::ArtKey1,
    Action::ArtKey2,
    Action::ArtKey3,
    Action::ArtKey4,
    Action::ArtKey5,
    Action::ArtKey6,
];

/// Each action: its section, its name in the file, its default keys and what it does.
const ACTIONS: [(Action, Section, &str, &[&str], &str); 39] = [
    (
        Action::Walk,
        Section::World,
        "walk",
        &["Shift"],
        "held, walk rather than run",
    ),
    (
        Action::StrikeAtOnce,
        Section::World,
        "strike_at_once",
        &["Control"],
        "held with the left button, stop and strike",
    ),
    (
        Action::Labels,
        Section::World,
        "labels",
        &["Alt"],
        "held, the items' names",
    ),
    (
        Action::Inventory,
        Section::World,
        "inventory",
        &["I"],
        "the inventory",
    ),
    (
        Action::QuestBook,
        Section::World,
        "quest_book",
        &["L"],
        "the quest book",
    ),
    (
        Action::WorldMap,
        Section::World,
        "world_map",
        &["M"],
        "the world map",
    ),
    (
        Action::Overview,
        Section::World,
        "overview_map",
        &["Tab"],
        "held, the overview map",
    ),
    (
        Action::WeaponSet1,
        Section::World,
        "weapon_set_1",
        &["1"],
        "the first weapon set",
    ),
    (
        Action::WeaponSet2,
        Section::World,
        "weapon_set_2",
        &["2"],
        "the second weapon set",
    ),
    (
        Action::WeaponSet3,
        Section::World,
        "weapon_set_3",
        &["3"],
        "the third weapon set",
    ),
    (
        Action::WeaponSet4,
        Section::World,
        "weapon_set_4",
        &["4"],
        "the fourth weapon set",
    ),
    (
        Action::WeaponSet5,
        Section::World,
        "weapon_set_5",
        &["5"],
        "the fifth weapon set",
    ),
    (
        Action::ArtSlot1,
        Section::World,
        "art_slot_1",
        &["6"],
        "the first art slot",
    ),
    (
        Action::ArtSlot2,
        Section::World,
        "art_slot_2",
        &["7"],
        "the second art slot",
    ),
    (
        Action::ArtSlot3,
        Section::World,
        "art_slot_3",
        &["8"],
        "the third art slot",
    ),
    (
        Action::ArtSlot4,
        Section::World,
        "art_slot_4",
        &["9"],
        "the fourth art slot",
    ),
    (
        Action::ArtSlot5,
        Section::World,
        "art_slot_5",
        &["0"],
        "the fifth art slot",
    ),
    (
        Action::ArtKey1,
        Section::World,
        "art_key_1",
        &["F1"],
        "the art bound to it into the selected slot; over an art of the spells page, bind it",
    ),
    (
        Action::ArtKey2,
        Section::World,
        "art_key_2",
        &["F2"],
        "as art_key_1",
    ),
    (
        Action::ArtKey3,
        Section::World,
        "art_key_3",
        &["F3"],
        "as art_key_1",
    ),
    (
        Action::ArtKey4,
        Section::World,
        "art_key_4",
        &["F4"],
        "as art_key_1",
    ),
    (
        Action::ArtKey5,
        Section::World,
        "art_key_5",
        &["F5"],
        "as art_key_1, only while the spells page shows",
    ),
    (
        Action::ArtKey6,
        Section::World,
        "art_key_6",
        &["F6"],
        "as art_key_5",
    ),
    (
        Action::BeltPotion,
        Section::World,
        "belt_potion",
        &["Space"],
        "a healing potion, with Shift the yellow one, once a press",
    ),
    (
        Action::PurplePotion,
        Section::World,
        "purple_potion",
        &["Q"],
        "a purple potion, again while held",
    ),
    (
        Action::BluePotion,
        Section::World,
        "blue_potion",
        &["W"],
        "a blue potion, again while held",
    ),
    (
        Action::GreenPotion,
        Section::World,
        "green_potion",
        &["E"],
        "a green potion, again while held",
    ),
    (
        Action::YellowPotion,
        Section::World,
        "yellow_potion",
        &["R"],
        "a yellow potion, again while held",
    ),
    (
        Action::FollowerPotion,
        Section::World,
        "follower_potion",
        &["B"],
        "a healing potion for the weakest follower",
    ),
    (
        Action::Back,
        Section::Interface,
        "back",
        &["Escape"],
        "close a window or go back; in the game, the pause menu",
    ),
    (
        Action::Confirm,
        Section::Interface,
        "confirm",
        &["Enter", "NumpadEnter"],
        "do the menu's choice; end a dialogue's line",
    ),
    (
        Action::Next,
        Section::Interface,
        "next",
        &["ArrowDown", "ArrowRight"],
        "the menu's next item",
    ),
    (
        Action::Previous,
        Section::Interface,
        "previous",
        &["ArrowUp", "ArrowLeft"],
        "the menu's item before",
    ),
    (
        Action::SendItem,
        Section::Interface,
        "send_item",
        &["Shift"],
        "held with a click on an item: the stash's to the backpack, a potion to a follower",
    ),
    (
        Action::Help,
        Section::Tools,
        "help",
        &["F10"],
        "the keys' help",
    ),
    (
        Action::Overlay,
        Section::Tools,
        "overlay",
        &["F7"],
        "the debug overlay",
    ),
    (Action::Log, Section::Tools, "log", &["F12"], "the log"),
    (
        Action::Window,
        Section::Tools,
        "window",
        &["F11"],
        "borderless full screen or a window",
    ),
    (
        Action::Console,
        Section::Tools,
        "console",
        &["Backquote"],
        "the console (the key under Esc)",
    ),
];

impl Action {
    /// Every action, in the file's order.
    pub fn all() -> impl Iterator<Item = Self> {
        ACTIONS.iter().map(|&(action, ..)| action)
    }

    fn row(self) -> (Section, &'static str, &'static [&'static str], &'static str) {
        ACTIONS
            .iter()
            .find(|&&(action, ..)| action == self)
            .map_or((Section::World, "", &[][..], ""), |&(_, s, n, k, d)| {
                (s, n, k, d)
            })
    }

    /// Its section.
    pub fn section(self) -> Section {
        self.row().0
    }

    /// Its name in the file.
    pub fn name(self) -> &'static str {
        self.row().1
    }

    /// What it does.
    pub fn does(self) -> &'static str {
        self.row().3
    }

    fn defaults(self) -> Vec<Binding> {
        self.row()
            .2
            .iter()
            .filter_map(|name| Binding::parse(name).ok())
            .collect()
    }

    fn named(section: Section, name: &str) -> Option<Self> {
        ACTIONS
            .iter()
            .find(|&&(_, s, n, ..)| s == section && n == name)
            .map(|&(action, ..)| action)
    }
}

/// The modifiers held.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
}

impl Modifiers {
    /// The modifiers that `keys` holds, either of each pair.
    pub fn held(keys: &ButtonInput<KeyCode>) -> Self {
        Self {
            shift: keys.any_pressed(Key::Shift.codes()),
            control: keys.any_pressed(Key::Control.codes()),
            alt: keys.any_pressed(Key::Alt.codes()),
        }
    }

    /// Whether every modifier of `asked` is held.
    fn covers(self, asked: Self) -> bool {
        (self.shift || !asked.shift) && (self.control || !asked.control) && (self.alt || !asked.alt)
    }

    fn count(self) -> usize {
        usize::from(self.shift) + usize::from(self.control) + usize::from(self.alt)
    }
}

/// A key of a binding: one by its place, or a modifier, either of its pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Key {
    Code(KeyCode),
    Shift,
    Control,
    Alt,
}

impl Key {
    fn codes(self) -> Vec<KeyCode> {
        match self {
            Self::Code(code) => vec![code],
            Self::Shift => vec![KeyCode::ShiftLeft, KeyCode::ShiftRight],
            Self::Control => vec![KeyCode::ControlLeft, KeyCode::ControlRight],
            Self::Alt => vec![KeyCode::AltLeft, KeyCode::AltRight],
        }
    }
}

/// The keys named in the file, beside the letters `A` to `Z`, the digits `0` to `9` and `F1` to
/// `F24`.
const NAMED: [(&str, KeyCode); 47] = [
    ("Space", KeyCode::Space),
    ("Escape", KeyCode::Escape),
    ("Enter", KeyCode::Enter),
    ("Tab", KeyCode::Tab),
    ("Backspace", KeyCode::Backspace),
    ("Backquote", KeyCode::Backquote),
    ("Minus", KeyCode::Minus),
    ("Equal", KeyCode::Equal),
    ("BracketLeft", KeyCode::BracketLeft),
    ("BracketRight", KeyCode::BracketRight),
    ("Backslash", KeyCode::Backslash),
    ("Semicolon", KeyCode::Semicolon),
    ("Quote", KeyCode::Quote),
    ("Comma", KeyCode::Comma),
    ("Period", KeyCode::Period),
    ("Slash", KeyCode::Slash),
    ("Insert", KeyCode::Insert),
    ("Delete", KeyCode::Delete),
    ("Home", KeyCode::Home),
    ("End", KeyCode::End),
    ("PageUp", KeyCode::PageUp),
    ("PageDown", KeyCode::PageDown),
    ("ArrowUp", KeyCode::ArrowUp),
    ("ArrowDown", KeyCode::ArrowDown),
    ("ArrowLeft", KeyCode::ArrowLeft),
    ("ArrowRight", KeyCode::ArrowRight),
    ("CapsLock", KeyCode::CapsLock),
    ("Pause", KeyCode::Pause),
    ("PrintScreen", KeyCode::PrintScreen),
    ("ScrollLock", KeyCode::ScrollLock),
    ("Numpad0", KeyCode::Numpad0),
    ("Numpad1", KeyCode::Numpad1),
    ("Numpad2", KeyCode::Numpad2),
    ("Numpad3", KeyCode::Numpad3),
    ("Numpad4", KeyCode::Numpad4),
    ("Numpad5", KeyCode::Numpad5),
    ("Numpad6", KeyCode::Numpad6),
    ("Numpad7", KeyCode::Numpad7),
    ("Numpad8", KeyCode::Numpad8),
    ("Numpad9", KeyCode::Numpad9),
    ("NumpadEnter", KeyCode::NumpadEnter),
    ("NumpadAdd", KeyCode::NumpadAdd),
    ("NumpadSubtract", KeyCode::NumpadSubtract),
    ("NumpadMultiply", KeyCode::NumpadMultiply),
    ("NumpadDivide", KeyCode::NumpadDivide),
    ("NumpadDecimal", KeyCode::NumpadDecimal),
    ("NumLock", KeyCode::NumLock),
];
const LETTERS: [KeyCode; 26] = [
    KeyCode::KeyA,
    KeyCode::KeyB,
    KeyCode::KeyC,
    KeyCode::KeyD,
    KeyCode::KeyE,
    KeyCode::KeyF,
    KeyCode::KeyG,
    KeyCode::KeyH,
    KeyCode::KeyI,
    KeyCode::KeyJ,
    KeyCode::KeyK,
    KeyCode::KeyL,
    KeyCode::KeyM,
    KeyCode::KeyN,
    KeyCode::KeyO,
    KeyCode::KeyP,
    KeyCode::KeyQ,
    KeyCode::KeyR,
    KeyCode::KeyS,
    KeyCode::KeyT,
    KeyCode::KeyU,
    KeyCode::KeyV,
    KeyCode::KeyW,
    KeyCode::KeyX,
    KeyCode::KeyY,
    KeyCode::KeyZ,
];
const DIGITS: [KeyCode; 10] = [
    KeyCode::Digit0,
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];
const FUNCTIONS: [KeyCode; 24] = [
    KeyCode::F1,
    KeyCode::F2,
    KeyCode::F3,
    KeyCode::F4,
    KeyCode::F5,
    KeyCode::F6,
    KeyCode::F7,
    KeyCode::F8,
    KeyCode::F9,
    KeyCode::F10,
    KeyCode::F11,
    KeyCode::F12,
    KeyCode::F13,
    KeyCode::F14,
    KeyCode::F15,
    KeyCode::F16,
    KeyCode::F17,
    KeyCode::F18,
    KeyCode::F19,
    KeyCode::F20,
    KeyCode::F21,
    KeyCode::F22,
    KeyCode::F23,
    KeyCode::F24,
];

/// The key of the name `name`.
fn key_named(name: &str) -> Option<Key> {
    match name {
        "Shift" => return Some(Key::Shift),
        "Control" | "Ctrl" => return Some(Key::Control),
        "Alt" => return Some(Key::Alt),
        _ => {}
    }
    if let Some(&(_, code)) = NAMED.iter().find(|&&(n, _)| n == name) {
        return Some(Key::Code(code));
    }
    let mut chars = name.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if c.is_ascii_uppercase() {
            let i = u32::from(c) - u32::from('A');
            return LETTERS
                .get(usize::try_from(i).ok()?)
                .map(|&code| Key::Code(code));
        }
        if let Some(i) = c.to_digit(10) {
            return DIGITS
                .get(usize::try_from(i).ok()?)
                .map(|&code| Key::Code(code));
        }
    }
    let n: usize = name.strip_prefix('F')?.parse().ok()?;
    FUNCTIONS
        .get(n.checked_sub(1)?)
        .map(|&code| Key::Code(code))
}

/// The name of the key `key`.
fn name_of(key: Key) -> String {
    let code = match key {
        Key::Shift => return "Shift".into(),
        Key::Control => return "Control".into(),
        Key::Alt => return "Alt".into(),
        Key::Code(code) => code,
    };
    if let Some(&(name, _)) = NAMED.iter().find(|&&(_, c)| c == code) {
        return name.into();
    }
    if let Some(i) = LETTERS.iter().position(|&c| c == code) {
        return char::from(b'A' + u8::try_from(i).unwrap_or(0)).to_string();
    }
    if let Some(i) = DIGITS.iter().position(|&c| c == code) {
        return i.to_string();
    }
    if let Some(i) = FUNCTIONS.iter().position(|&c| c == code) {
        return format!("F{}", i + 1);
    }
    format!("{code:?}")
}

/// A key and the modifiers asked with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    key: Key,
    with: Modifiers,
}

impl Binding {
    /// A key's name with its modifiers before it, joined by `+`: `F1`, `Shift+Space`.
    fn parse(text: &str) -> Result<Self> {
        let mut parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let last = parts.pop().unwrap_or_default();
        let key = key_named(last).ok_or_else(|| anyhow!("no key {last:?}"))?;
        let mut with = Modifiers::default();
        for part in parts {
            match key_named(part) {
                Some(Key::Shift) => with.shift = true,
                Some(Key::Control) => with.control = true,
                Some(Key::Alt) => with.alt = true,
                _ => bail!("{part:?} is no modifier"),
            }
        }
        Ok(Self { key, with })
    }

    /// Its name, as the file writes it.
    pub fn name(&self) -> String {
        let mut name = String::new();
        for (held, modifier) in [
            (self.with.control, "Control+"),
            (self.with.alt, "Alt+"),
            (self.with.shift, "Shift+"),
        ] {
            if held {
                name.push_str(modifier);
            }
        }
        name.push_str(&name_of(self.key));
        name
    }

    /// Whether `code` is its key.
    fn is(&self, code: KeyCode) -> bool {
        self.key.codes().contains(&code)
    }
}

/// The keys of every action.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Controls {
    keys: HashMap<Action, Vec<Binding>>,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            keys: Action::all()
                .map(|action| (action, action.defaults()))
                .collect(),
        }
    }
}

impl Controls {
    /// The keys of `action`.
    pub fn bindings(&self, action: Action) -> &[Binding] {
        self.keys.get(&action).map_or(&[], Vec::as_slice)
    }

    /// The names of the keys of `action`, joined by commas.
    pub fn names(&self, action: Action) -> String {
        self.bindings(action)
            .iter()
            .map(Binding::name)
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Whether `binding` of `action` counts with the modifiers `held`: its own held, and no key of
    /// more modifiers held on the same key in its section.
    fn counts(&self, action: Action, binding: &Binding, held: Modifiers) -> bool {
        held.covers(binding.with)
            && !self.keys.iter().any(|(&other, bindings)| {
                other.section() == action.section()
                    && bindings.iter().any(|b| {
                        b.key == binding.key
                            && b.with.count() > binding.with.count()
                            && b.with.covers(binding.with)
                            && held.covers(b.with)
                    })
            })
    }

    fn any(
        &self,
        action: Action,
        keys: &ButtonInput<KeyCode>,
        state: impl Fn(&ButtonInput<KeyCode>, KeyCode) -> bool,
    ) -> bool {
        let held = Modifiers::held(keys);
        self.bindings(action).iter().any(|binding| {
            binding
                .key
                .codes()
                .into_iter()
                .any(|code| state(keys, code))
                && self.counts(action, binding, held)
        })
    }

    /// Whether a key of `action` is held.
    pub fn pressed(&self, action: Action, keys: &ButtonInput<KeyCode>) -> bool {
        self.any(action, keys, ButtonInput::pressed)
    }

    /// Whether a key of `action` went down this frame.
    pub fn just_pressed(&self, action: Action, keys: &ButtonInput<KeyCode>) -> bool {
        self.any(action, keys, ButtonInput::just_pressed)
    }

    /// Whether a key of `action` came up this frame.
    pub fn just_released(&self, action: Action, keys: &ButtonInput<KeyCode>) -> bool {
        self.any(action, keys, ButtonInput::just_released)
    }

    /// Takes the keys of `action` that went down this frame from the systems after.
    pub fn clear_just_pressed(&self, action: Action, keys: &mut ButtonInput<KeyCode>) {
        for binding in self.bindings(action) {
            for code in binding.key.codes() {
                keys.clear_just_pressed(code);
            }
        }
    }

    /// Whether the key message `input`, a press or its repeat, is a key of `action` with the
    /// modifiers `held`.
    pub fn is(&self, action: Action, input: &KeyboardInput, held: Modifiers) -> bool {
        self.bindings(action)
            .iter()
            .any(|binding| binding.is(input.key_code) && self.counts(action, binding, held))
    }

    /// The file's text of these keys: a section each, an action a line under a comment of what
    /// it does.
    pub fn text(&self) -> String {
        let mut text = String::from(
            "# OpenSacred's keys (docs/10 §9): each action and its keys, by their places on the\n\
             # keyboard (I, 1, F1, Space, NumpadEnter, ...), with the modifiers held with them\n\
             # (Shift+Space). An action left out takes its default; delete the file for them all.\n",
        );
        for section in Section::ALL {
            let _ = write!(text, "\n[{}]\n", section.name());
            for action in Action::all().filter(|action| action.section() == section) {
                let keys: Vec<String> = self
                    .bindings(action)
                    .iter()
                    .map(|binding| format!("\"{}\"", binding.name()))
                    .collect();
                let _ = writeln!(text, "# {}", action.does());
                let _ = writeln!(text, "{} = [{}]", action.name(), keys.join(", "));
            }
        }
        text
    }

    /// Reads the keys from the document `text`, the defaults where it says nothing, with what was
    /// wrong in it.
    fn parse(text: &str) -> Result<(Self, Vec<String>)> {
        let document: DocumentMut = text.parse()?;
        let mut controls = Self::default();
        let mut warnings = Vec::new();
        for (section_name, item) in document.iter() {
            let Some(section) = Section::ALL.into_iter().find(|s| s.name() == section_name) else {
                warnings.push(format!("unknown section [{section_name}]"));
                continue;
            };
            let Some(table) = item.as_table_like() else {
                warnings.push(format!("[{section_name}] is no table"));
                continue;
            };
            for (name, value) in table.iter() {
                let Some(action) = Action::named(section, name) else {
                    warnings.push(format!("unknown action {section_name}.{name}"));
                    continue;
                };
                let read = value
                    .as_array()
                    .ok_or_else(|| anyhow!("not a list of keys"))
                    .and_then(|array| {
                        array
                            .iter()
                            .map(|key| {
                                key.as_str()
                                    .ok_or_else(|| anyhow!("{key} is no key's name"))
                                    .and_then(Binding::parse)
                            })
                            .collect::<Result<Vec<_>>>()
                    });
                match read {
                    Ok(bindings) => {
                        controls.keys.insert(action, bindings);
                    }
                    Err(e) => warnings.push(format!(
                        "{section_name}.{name}: {e}; its default {}",
                        Self::default().names(action)
                    )),
                }
            }
        }
        warnings.extend(controls.conflicts());
        Ok((controls, warnings))
    }

    /// The keys that two actions of one section share.
    fn conflicts(&self) -> Vec<String> {
        let mut found = Vec::new();
        let actions: Vec<Action> = Action::all().collect();
        for (i, &a) in actions.iter().enumerate() {
            for &b in actions.iter().skip(i + 1) {
                if a.section() != b.section() {
                    continue;
                }
                for binding in self.bindings(a) {
                    if self.bindings(b).contains(binding) {
                        found.push(format!(
                            "{} and {} share {} in [{}]",
                            a.name(),
                            b.name(),
                            binding.name(),
                            a.section().name()
                        ));
                    }
                }
            }
        }
        found
    }

    /// Reads `controls.toml` beside the settings' file `settings`, or in the configuration
    /// directory, and writes it with the defaults where there is none; with what was wrong.
    pub fn read(settings: Option<&Path>) -> (Self, Vec<String>) {
        let path = settings
            .and_then(Path::parent)
            .map(|dir| dir.join(FILE))
            .or_else(|| crate::settings::config_dir().map(|dir| dir.join(FILE)));
        let Some(path) = path else {
            return (Self::default(), vec!["no configuration directory".into()]);
        };
        match fs::read_to_string(&path) {
            Ok(text) => match Self::parse(&text) {
                Ok((controls, warnings)) => (
                    controls,
                    warnings
                        .into_iter()
                        .map(|warning| format!("{}: {warning}", path.display()))
                        .collect(),
                ),
                Err(e) => (
                    Self::default(),
                    vec![format!("{}: {e}; the defaults", path.display())],
                ),
            },
            Err(e) if e.kind() == ErrorKind::NotFound => {
                let controls = Self::default();
                let warnings = write(&path, &controls.text())
                    .err()
                    .map(|e| format!("{e:#}"))
                    .into_iter()
                    .collect();
                (controls, warnings)
            }
            Err(e) => (
                Self::default(),
                vec![format!("reading {}: {e}; the defaults", path.display())],
            ),
        }
    }
}

/// Writes `text` to `path`, its directory made.
fn write(path: &Path, text: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| anyhow!("creating {}: {e}", dir.display()))?;
    }
    fs::write(path, text).map_err(|e| anyhow!("writing {}: {e}", path.display()))
}

/// What was wrong with the file, logged once the log is up.
#[derive(Resource, Default)]
pub struct Warnings(pub Vec<String>);

fn report(warnings: Res<Warnings>) {
    for warning in &warnings.0 {
        warn!("controls: {warning}");
    }
}

/// The defaults, for a system run where no file was read (a module's tests).
static DEFAULTS: LazyLock<Controls> = LazyLock::new(Controls::default);

/// A system's keys, read through the controls.
#[derive(SystemParam)]
pub struct Keys<'w> {
    input: Res<'w, ButtonInput<KeyCode>>,
    controls: Option<Res<'w, Controls>>,
}

impl Keys<'_> {
    /// The controls in force.
    pub fn controls(&self) -> &Controls {
        self.controls.as_deref().unwrap_or(&DEFAULTS)
    }

    /// Whether a key of `action` is held.
    pub fn pressed(&self, action: Action) -> bool {
        self.controls().pressed(action, &self.input)
    }

    /// Whether a key of `action` went down this frame.
    pub fn just_pressed(&self, action: Action) -> bool {
        self.controls().just_pressed(action, &self.input)
    }

    /// Whether a key of `action` came up this frame.
    pub fn just_released(&self, action: Action) -> bool {
        self.controls().just_released(action, &self.input)
    }

    /// The modifiers held.
    pub fn modifiers(&self) -> Modifiers {
        Modifiers::held(&self.input)
    }
}

/// A system's keys, read through the controls, which takes the keys it acts on from the systems
/// after it.
#[derive(SystemParam)]
pub struct KeysMut<'w> {
    input: ResMut<'w, ButtonInput<KeyCode>>,
    controls: Option<Res<'w, Controls>>,
}

impl KeysMut<'_> {
    fn controls(&self) -> &Controls {
        self.controls.as_deref().unwrap_or(&DEFAULTS)
    }

    /// Whether a key of `action` went down this frame.
    pub fn just_pressed(&self, action: Action) -> bool {
        self.controls().just_pressed(action, &self.input)
    }

    /// The modifiers held.
    pub fn modifiers(&self) -> Modifiers {
        Modifiers::held(&self.input)
    }

    /// Whether `code` is a key of `action`.
    pub fn is_key(&self, action: Action, code: KeyCode) -> bool {
        self.controls()
            .bindings(action)
            .iter()
            .any(|binding| binding.is(code))
    }

    /// Takes the keys of `action` that went down this frame from the systems after.
    pub fn clear(&mut self, action: Action) {
        let controls = self.controls.as_deref().unwrap_or(&DEFAULTS);
        controls.clear_just_pressed(action, &mut self.input);
    }

    /// Whether a key of `action` went down this frame, then taken from the systems after.
    pub fn take(&mut self, action: Action) -> bool {
        let pressed = self.just_pressed(action);
        if pressed {
            self.clear(action);
        }
        pressed
    }
}

/// The game's keys: the defaults until `main` puts the file's in.
pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Controls>()
            .init_resource::<Warnings>()
            .add_systems(Startup, report);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use bevy::input::ButtonState;
    use bevy::input::keyboard::Key as Logical;

    use super::*;

    /// A fresh directory for one test's file.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "opensacred-game-controls-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn keys(held: &[KeyCode], pressed: KeyCode) -> ButtonInput<KeyCode> {
        let mut keys = ButtonInput::default();
        for &key in held {
            keys.press(key);
        }
        keys.clear();
        keys.press(pressed);
        keys
    }

    #[test]
    fn the_defaults_are_the_original_s_and_read_back_as_written() {
        let controls = Controls::default();
        assert_eq!(controls.names(Action::Inventory), "I");
        assert_eq!(controls.names(Action::ArtKey1), "F1");
        assert_eq!(controls.names(Action::Confirm), "Enter, NumpadEnter");
        assert_eq!(controls.names(Action::Walk), "Shift");
        let dir = scratch("defaults");
        let settings = dir.join("settings.toml");
        let (read, warnings) = Controls::read(Some(&settings));
        assert_eq!((read, warnings), (Controls::default(), Vec::new()));
        // The first start wrote them, and they read back.
        let text = fs::read_to_string(dir.join(FILE)).unwrap();
        assert!(
            text.contains("[world]\n# held, walk rather than run"),
            "{text}"
        );
        assert!(!text.contains('\r'));
        let (again, warnings) = Controls::read(Some(&settings));
        assert_eq!((again, warnings), (Controls::default(), Vec::new()));
        // Every action has a key, and no section shares one.
        assert!(Action::all().all(|action| !controls.bindings(action).is_empty()));
        assert!(controls.conflicts().is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_key_moved_in_the_file_moves_the_action() {
        let (controls, warnings) =
            Controls::parse("[tools]\nhelp = [\"F1\", \"Shift+H\"]\n").unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(controls.names(Action::Help), "F1, Shift+H");
        assert!(controls.just_pressed(Action::Help, &keys(&[], KeyCode::F1)));
        assert!(!controls.just_pressed(Action::Help, &keys(&[], KeyCode::F10)));
        assert!(!controls.just_pressed(Action::Help, &keys(&[], KeyCode::KeyH)));
        assert!(controls.just_pressed(Action::Help, &keys(&[KeyCode::ShiftRight], KeyCode::KeyH)));
        // The rest keep their defaults.
        assert_eq!(controls.names(Action::Overlay), "F7");
    }

    #[test]
    fn an_unknown_name_or_a_shared_key_warns_and_the_default_stays() {
        let (controls, warnings) = Controls::parse(
            "[world]\ninventory = [\"Nothing\"]\nfly = [\"F\"]\nlabels = [\"I\"]\n[menus]\n",
        )
        .unwrap();
        assert_eq!(controls.names(Action::Inventory), "I");
        assert_eq!(warnings.len(), 4, "{warnings:?}");
        assert!(
            warnings[0].contains("no key \"Nothing\"; its default I"),
            "{warnings:?}"
        );
        assert!(warnings[1].contains("unknown action world.fly"));
        assert!(warnings[2].contains("unknown section [menus]"));
        assert!(
            warnings[3].contains("labels and inventory share I"),
            "{warnings:?}"
        );
    }

    #[test]
    fn a_key_of_more_modifiers_held_wins_over_the_key_alone() {
        let (controls, _) = Controls::parse(
            "[world]\nbelt_potion = [\"Space\"]\nyellow_potion = [\"Shift+Space\"]\n",
        )
        .unwrap();
        let alone = keys(&[], KeyCode::Space);
        let shifted = keys(&[KeyCode::ShiftLeft], KeyCode::Space);
        assert!(controls.just_pressed(Action::BeltPotion, &alone));
        assert!(!controls.just_pressed(Action::YellowPotion, &alone));
        assert!(!controls.just_pressed(Action::BeltPotion, &shifted));
        assert!(controls.just_pressed(Action::YellowPotion, &shifted));
        // A key without one asked lets the modifiers be held: Q with Shift held, as walking.
        let defaults = Controls::default();
        let input = KeyboardInput {
            key_code: KeyCode::KeyQ,
            logical_key: Logical::Character("q".into()),
            state: ButtonState::Pressed,
            text: None,
            repeat: true,
            window: Entity::PLACEHOLDER,
        };
        let walking = Modifiers {
            shift: true,
            ..default()
        };
        assert!(defaults.is(Action::PurplePotion, &input, walking));
        assert!(defaults.pressed(Action::Walk, &keys(&[KeyCode::ShiftRight], KeyCode::KeyQ)));
    }

    #[test]
    fn no_module_reads_a_key_but_through_the_controls() {
        // The console's line editing and remote control's keys alone name keys of their own.
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut found = Vec::new();
        for entry in fs::read_dir(&src).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if !name.ends_with(".rs") || ["controls.rs", "remote.rs"].contains(&name.as_str()) {
                continue;
            }
            let text = fs::read_to_string(&path).unwrap();
            let code = text.split("#[cfg(test)]").next().unwrap();
            for (i, line) in code.lines().enumerate() {
                let line = line.trim();
                if line.starts_with("//") || !line.contains("KeyCode::") {
                    continue;
                }
                let editing = name == "console.rs"
                    && (line.contains("KeyCode::KeyV") || line.contains("KeyCode::KeyC"));
                if !editing {
                    found.push(format!("{name}:{}: {line}", i + 1));
                }
            }
        }
        assert!(found.is_empty(), "{found:#?}");
    }
}
