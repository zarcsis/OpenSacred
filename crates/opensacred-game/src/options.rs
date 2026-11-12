//! The options window (`UI_WND_OPTIONS`, `cUI_Options`; #2340, docs/re/menus.md, "The options
//! window"): the settings that the original's window holds, over the whole of its screen, in six
//! panels: the graphics' detail and the overview map's transparency, the three volumes, the
//! collecting, the sound's switch and quality, the other options and the multiplayer's data flow,
//! with Cancel and OK.
//!
//! The main menu's Options shows the menus' window ([`Mode::Menu`]); in a game the menu over it's
//! Options and the key O show the game's ([`Mode::Game`]), the game paused under it. Each show
//! sets the controls from the settings (`0x718660`); OK writes the window's keys into
//! `settings.toml` (`0x717C20`), while Cancel, Esc and the key O again write nothing; each goes
//! back to the main menu, or on with the game.

use bevy::picking::events::{PointerClick, PointerOut, PointerOver, PointerPress, PointerScroll};
use bevy::prelude::*;

use crate::bar::{self, UiImages};
use crate::busy_dialog;
use crate::choice::{self, Atlases};
use crate::game::FONT;
use crate::hud::Pointer;
use crate::menu::{Screen, Words};
use crate::pause::Play;
use crate::saves_window::{bar_of, lettering, stretched};
use crate::settings::{Collect, NetworkSpeed, Settings, SettingsFile};

/// Which of the two windows shows: the menus' (`+0x138`) or the game's (`+0xBC`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Menu,
    Game,
}

/// The background's textures (`UW_option_1_01.tga` to `UW_option_1_12.tga`), four to a row.
const TILES: usize = 12;
/// The background as the converter assembles it from its tiles (`opensacred-convert ui`, #2634).
const PICTURE: &str = "ui/screens/options.ktx2";

/// The rectangle of `size` at `at`.
const fn boxed(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect {
        min: Vec2::new(x, y),
        max: Vec2::new(x + width, y + height),
    }
}

/// The six panels' boxes, which the frames stand about, the fills grown by 20 about them.
const BOXES: [Rect; 6] = [
    boxed(64.0, 64.0, 384.0, 128.0),
    boxed(544.0, 64.0, 384.0, 128.0),
    boxed(64.0, 288.0, 384.0, 128.0),
    boxed(544.0, 288.0, 384.0, 128.0),
    boxed(64.0, 512.0, 384.0, 160.0),
    boxed(544.0, 512.0, 384.0, 64.0),
];
const FILL_GROWTH: f32 = 20.0;
/// The two separators under the collecting's and the sound's first rows: their left ends, their
/// width and their pieces (`UI_STATIC_TEXTBAR`'s).
const SEPARATORS: [Vec2; 2] = [Vec2::new(44.0, 310.0), Vec2::new(524.0, 310.0)];
const SEPARATOR_WIDTH: f32 = 424.0;
const SEPARATOR: [u32; 3] = [296, 294, 297];
/// The titles' bars, 200 wide over their panels' crests, their pieces, and their titles in the
/// font 7, white, centred.
const TITLE_BAR: [u32; 3] = [342, 343, 344];
const TITLE_WIDTH: f32 = 200.0;
const TITLES: [(Vec2, (&str, &str)); 6] = [
    (Vec2::new(152.0, 32.0), ("UI_CFG_GFX_SETTINGS", "Graphics")),
    (Vec2::new(632.0, 32.0), ("UI_CFG_SOUND_VOLUME", "Volume")),
    (Vec2::new(152.0, 256.0), ("UI_CFG_AUTOPICKUP", "Collecting")),
    (
        Vec2::new(632.0, 256.0),
        ("UI_CFG_SOUND_QUALITY_TITLE", "Sound"),
    ),
    (Vec2::new(152.0, 480.0), ("UI_CFG_VARIOUS", "Other")),
    (Vec2::new(632.0, 480.0), ("UI_CFG_WAN", "Multiplayer")),
];
/// The font 7's size, as the saves' window's labels.
const TITLE_SIZE: f32 = 13.0;
/// The font 3, AntiquaSSK 16, at the size the overview writes it, and how much lower than Bevy
/// sets them the original sets its letters (`overview`).
const LABEL_SIZE: f32 = 24.0;
const LABEL_DROP: f32 = 2.0;

/// How a label stands in its rectangle: from its left edge, or against its right one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Align {
    Left,
    Right,
}

/// The labels: their rectangles' corners, their widths, how they stand and their texts.
const LABELS: [(Vec2, f32, Align, (&str, &str)); 26] = [
    (
        Vec2::new(64.0, 116.0),
        64.0,
        Align::Left,
        ("UI_CFG_GFX_DETAIL", "Detail"),
    ),
    (
        Vec2::new(272.0, 60.0),
        128.0,
        Align::Right,
        ("UI_CFG_LOW", "Low"),
    ),
    (
        Vec2::new(272.0, 97.0),
        128.0,
        Align::Right,
        ("UI_CFG_MEDIUM", "Medium"),
    ),
    (
        Vec2::new(272.0, 134.0),
        128.0,
        Align::Right,
        ("UI_CFG_HIGH", "High"),
    ),
    (
        Vec2::new(66.0, 167.0),
        192.0,
        Align::Left,
        ("UI_CFG_MAPTRANS", "Map transparency"),
    ),
    (
        Vec2::new(504.0, 68.0),
        128.0,
        Align::Right,
        ("UI_CFG_SFX", "Effects"),
    ),
    (
        Vec2::new(504.0, 116.0),
        128.0,
        Align::Right,
        ("UI_CFG_SPEECH", "Speech"),
    ),
    (
        Vec2::new(504.0, 164.0),
        128.0,
        Align::Right,
        ("UI_CFG_SOUND", "Music"),
    ),
    (
        Vec2::new(80.0, 278.0),
        320.0,
        Align::Left,
        ("UI_CFG_ANIM", "Pick-up animation"),
    ),
    (
        Vec2::new(80.0, 368.0),
        320.0,
        Align::Left,
        ("UI_CFG_COLLECT", "Collect"),
    ),
    (
        Vec2::new(272.0, 334.0),
        128.0,
        Align::Right,
        ("UI_CFG_AUTO_ALL", "All"),
    ),
    (
        Vec2::new(272.0, 371.0),
        128.0,
        Align::Right,
        ("UI_CFG_AUTO_UNIQUE", "Valuables"),
    ),
    (
        Vec2::new(272.0, 408.0),
        128.0,
        Align::Right,
        ("UI_CFG_AUTO_GOLD", "Gold"),
    ),
    (
        Vec2::new(560.0, 278.0),
        320.0,
        Align::Left,
        ("UI_CFG_SOUNDPLAYBACK", "Sound"),
    ),
    (
        Vec2::new(560.0, 368.0),
        320.0,
        Align::Left,
        ("UI_CFG_SOUND_QUALITY", "Quality"),
    ),
    (
        Vec2::new(752.0, 334.0),
        128.0,
        Align::Right,
        ("UI_BTN_SNDQ_LOW", "Fewer effects"),
    ),
    (
        Vec2::new(752.0, 371.0),
        128.0,
        Align::Right,
        ("UI_BTN_SNDQ_MID", "Every effect"),
    ),
    (
        Vec2::new(752.0, 408.0),
        128.0,
        Align::Right,
        ("UI_LABEL_SNDQ_NO3D", "No 3D sound"),
    ),
    (
        Vec2::new(80.0, 511.0),
        320.0,
        Align::Left,
        ("UI_CFG_AUTOFOLLOW", "Follow the foe"),
    ),
    (
        Vec2::new(80.0, 548.0),
        320.0,
        Align::Left,
        ("UI_CFG_AUTOSAVE", "Autosave"),
    ),
    (
        Vec2::new(80.0, 585.0),
        320.0,
        Align::Left,
        ("UI_CFG_EXPLORE", "Fog of war"),
    ),
    (
        Vec2::new(80.0, 622.0),
        320.0,
        Align::Left,
        ("UI_CFG_VIOLENCE", "Violence"),
    ),
    (
        Vec2::new(80.0, 659.0),
        320.0,
        Align::Left,
        ("UI_CFG_FSAA_FILTER", "FSAA filter"),
    ),
    (
        Vec2::new(560.0, 535.0),
        320.0,
        Align::Left,
        ("UI_CFG_LANSPEED", "Data flow"),
    ),
    (
        Vec2::new(752.0, 516.0),
        128.0,
        Align::Right,
        ("UI_CFG_AB_DSL", "Fast"),
    ),
    (
        Vec2::new(752.0, 553.0),
        128.0,
        Align::Right,
        ("UI_CFG_AB_ISDN", "Slow"),
    ),
];

/// The ticks' and the round choices' pictures, ticked and not (`GUI_main_12.TGA`'s orbs).
const TICKED: u32 = 323;
const UNTICKED: u32 = 272;

/// A tick of the window: a setting on or off.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum Tick {
    Animation,
    Sound,
    Follow,
    Autosave,
    Explore,
    Violence,
    Filter,
}

impl Tick {
    /// The ticks and their corners.
    const ALL: [(Self, Vec2); 7] = [
        (Self::Animation, Vec2::new(416.0, 276.0)),
        (Self::Sound, Vec2::new(896.0, 276.0)),
        (Self::Follow, Vec2::new(416.0, 507.0)),
        (Self::Autosave, Vec2::new(416.0, 544.0)),
        (Self::Explore, Vec2::new(416.0, 581.0)),
        (Self::Violence, Vec2::new(416.0, 618.0)),
        (Self::Filter, Vec2::new(416.0, 655.0)),
    ];

    fn get(self, settings: &Settings) -> bool {
        match self {
            Self::Animation => settings.gameplay.pickup_animation,
            Self::Sound => settings.sound.on,
            Self::Follow => settings.interface.follow_foe,
            Self::Autosave => settings.gameplay.autosave,
            Self::Explore => settings.interface.explore_map,
            Self::Violence => settings.gameplay.violence,
            Self::Filter => settings.graphics.fsaa_filter,
        }
    }

    fn slot(self, settings: &mut Settings) -> &mut bool {
        match self {
            Self::Animation => &mut settings.gameplay.pickup_animation,
            Self::Sound => &mut settings.sound.on,
            Self::Follow => &mut settings.interface.follow_foe,
            Self::Autosave => &mut settings.gameplay.autosave,
            Self::Explore => &mut settings.interface.explore_map,
            Self::Violence => &mut settings.gameplay.violence,
            Self::Filter => &mut settings.graphics.fsaa_filter,
        }
    }
}

/// A round choice of one of the four groups, one of which always holds.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum Choice {
    Detail(u8),
    Collect(Collect),
    Quality(u8),
    Network(NetworkSpeed),
}

impl Choice {
    /// The choices shown and their corners; the 3D sound's (896, 404) is hidden, as the
    /// original's where its sound finds no 3D provider (`0x692B60`), which the game has not.
    const ALL: [(Self, Vec2); 10] = [
        (Self::Detail(0), Vec2::new(416.0, 56.0)),
        (Self::Detail(1), Vec2::new(416.0, 93.0)),
        (Self::Detail(2), Vec2::new(416.0, 130.0)),
        (Self::Collect(Collect::All), Vec2::new(416.0, 330.0)),
        (Self::Collect(Collect::Valuables), Vec2::new(416.0, 367.0)),
        (Self::Collect(Collect::Gold), Vec2::new(416.0, 404.0)),
        (Self::Quality(0), Vec2::new(896.0, 330.0)),
        (Self::Quality(1), Vec2::new(896.0, 367.0)),
        (Self::Network(NetworkSpeed::Fast), Vec2::new(896.0, 512.0)),
        (Self::Network(NetworkSpeed::Slow), Vec2::new(896.0, 549.0)),
    ];

    fn chosen(self, settings: &Settings) -> bool {
        match self {
            Self::Detail(detail) => settings.graphics.detail == detail,
            Self::Collect(collect) => settings.gameplay.collect == collect,
            Self::Quality(quality) => settings.sound.quality == quality,
            Self::Network(speed) => settings.network.speed == speed,
        }
    }

    fn choose(self, settings: &mut Settings) {
        match self {
            Self::Detail(detail) => settings.graphics.detail = detail,
            Self::Collect(collect) => settings.gameplay.collect = collect,
            Self::Quality(quality) => settings.sound.quality = quality,
            Self::Network(speed) => settings.network.speed = speed,
        }
    }
}

/// A slider of the window (`cUI_Scrollbar`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum Slider {
    Transparency,
    Effects,
    Speech,
    Music,
}

/// A slider's pieces (`0x752670`): its "−" button at its left edge, its "+" against its right
/// edge, its bar between them and its knob, and how far in from its left edge its knob's middle
/// runs from and how much narrower than it that run is (the bar less the knob's width).
const MINUS: u32 = 263;
const PLUS: u32 = 265;
const BAR: u32 = 264;
const KNOB: u32 = 262;
const MINUS_WIDTH: f32 = 37.0;
const PLUS_WIDTH: f32 = 40.0;
const RUN_FROM: f32 = 52.0;
const RUN_NARROWER: f32 = 107.0;
const KNOB_HALF: Vec2 = Vec2::new(15.0, 18.0);

impl Slider {
    /// The sliders and their rectangles.
    const ALL: [(Self, Rect); 4] = [
        (Self::Transparency, boxed(260.0, 167.0, 192.0, 32.0)),
        (Self::Effects, boxed(648.0, 64.0, 288.0, 32.0)),
        (Self::Speech, boxed(648.0, 112.0, 288.0, 32.0)),
        (Self::Music, boxed(648.0, 160.0, 288.0, 32.0)),
    ];

    /// Its number of positions (`0x753070`): 255 for the transparency, 126 for the volumes.
    fn positions(self) -> u8 {
        match self {
            Self::Transparency => 255,
            Self::Effects | Self::Speech | Self::Music => 126,
        }
    }

    fn slot(self, settings: &mut Settings) -> &mut u8 {
        match self {
            Self::Transparency => &mut settings.interface.map_alpha,
            Self::Effects => &mut settings.sound.effects,
            Self::Speech => &mut settings.sound.speech,
            Self::Music => &mut settings.sound.music,
        }
    }

    /// The volume it sets live as it moves, none for the transparency.
    fn kind(self) -> Option<crate::volumes::Kind> {
        match self {
            Self::Transparency => None,
            Self::Effects => Some(crate::volumes::Kind::Effects),
            Self::Speech => Some(crate::volumes::Kind::Speech),
            Self::Music => Some(crate::volumes::Kind::Music),
        }
    }

    fn get(self, settings: &Settings) -> u8 {
        match self {
            Self::Transparency => settings.interface.map_alpha,
            Self::Effects => settings.sound.effects,
            Self::Speech => settings.sound.speech,
            Self::Music => settings.sound.music,
        }
    }
}

/// One position along or back, within `0 .. positions`.
fn step(position: u8, positions: u8, up: bool) -> u8 {
    if up {
        position.saturating_add(1).min(positions.saturating_sub(1))
    } else {
        position.saturating_sub(1)
    }
}

/// A press at `point` on the slider of `rect`: "−" a position back, "+" one along, else the knob
/// at the point, its share of the run held below 1 (`0x752A60`, `0x7530C0`, `0x753430`).
fn pressed_at(position: u8, positions: u8, rect: Rect, point: Vec2) -> u8 {
    if point.x < rect.min.x + MINUS_WIDTH {
        return step(position, positions, false);
    }
    if point.x >= rect.max.x - PLUS_WIDTH {
        return step(position, positions, true);
    }
    let run = (rect.width() - RUN_NARROWER).max(1.0);
    let share = ((point.x - rect.min.x - RUN_FROM) / run).clamp(0.0, 0.9999);
    whole(share * f32::from(positions)).min(positions.saturating_sub(1))
}

/// A share's position, rounded down.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is held to 0..=255 first"
)]
fn whole(value: f32) -> u8 {
    value.floor().clamp(0.0, 255.0) as u8
}

/// The knob's left edge for `position` (**estimate**: its middle at the run's share, docs/re).
fn knob_x(position: u8, positions: u8, rect: Rect) -> f32 {
    let share = f32::from(position) / f32::from(positions.max(1));
    rect.min.x + RUN_FROM + share * (rect.width() - RUN_NARROWER) - KNOB_HALF.x
}

/// A slider's knob, which moves as its setting does.
#[derive(Component)]
struct Knob(Slider);

/// The foot's buttons.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum Foot {
    Cancel,
    Ok,
}

impl Foot {
    /// The buttons, their corners and their words.
    const ALL: [(Self, Vec2, (&'static str, &'static str)); 2] = [
        (
            Self::Cancel,
            Vec2::new(604.0, 644.0),
            ("UI_CANCEL", "Cancel"),
        ),
        (Self::Ok, Vec2::new(804.0, 644.0), ("UI_OK", "OK")),
    ];
}

/// The buttons' size (image 281's width times 2.5, its height), their pieces (the middle
/// stretched over it, the ends outside it), the font 1's size, as the saves' window's, and how
/// far up their words are set.
const BUTTON_SIZE: Vec2 = Vec2::new(95.0, 37.0);
const BUTTON: [u32; 3] = [280, 281, 282];
const WORDS: f32 = 20.0;
const BUTTON_RISE: f32 = 3.0;
/// The words' colours: white, and `0xFFFFFF80` under the cursor, in the frame's sRGB values.
const WHITE: Color = Color::WHITE;
const LIT: Color = Color::linear_rgb(1.0, 1.0, 128.0 / 255.0);

/// The keys of `settings.toml` that OK writes, those of `Settings.cfg` that the original's writes
/// (`0x717C20`).
const KEYS: [&str; 15] = [
    "graphics.detail",
    "graphics.fsaa_filter",
    "interface.map_alpha",
    "interface.explore_map",
    "interface.follow_foe",
    "sound.on",
    "sound.quality",
    "sound.effects",
    "sound.speech",
    "sound.music",
    "gameplay.autosave",
    "gameplay.violence",
    "gameplay.pickup_animation",
    "gameplay.collect",
    "network.speed",
];

/// The window's settings as its controls hold them, set from the settings in force as it shows,
/// its mode, and the button under the cursor.
#[derive(Resource)]
pub struct Draft {
    settings: Settings,
    mode: Mode,
    lit: Option<Foot>,
}

/// The settings that the window holds, from `draft` into `settings`.
fn confirm(draft: &Settings, settings: &mut Settings) {
    settings.graphics = draft.graphics.clone();
    settings.interface.map_alpha = draft.interface.map_alpha;
    settings.interface.explore_map = draft.interface.explore_map;
    settings.interface.follow_foe = draft.interface.follow_foe;
    settings.sound = draft.sound.clone();
    settings.gameplay.autosave = draft.gameplay.autosave;
    settings.gameplay.violence = draft.gameplay.violence;
    settings.gameplay.pickup_animation = draft.gameplay.pickup_animation;
    settings.gameplay.collect = draft.gameplay.collect;
    settings.network = draft.network.clone();
}

/// Asks for the game's window: the menu over a game's Options, or the key O.
#[derive(Resource, Default)]
pub struct Asked(pub bool);

/// The background's textures, found among the converted ones.
#[derive(Resource, Default)]
pub struct Images {
    background: crate::menu::Background,
}

impl Images {
    /// Finds the background's tiles.
    pub fn find(assets: &std::path::Path) -> Self {
        let names: Vec<String> = (1..=TILES).map(|n| format!("uw_option_1_{n:02}")).collect();
        Self {
            background: crate::menu::Background::find(assets, PICTURE, &names),
        }
    }
}

/// The window's root.
#[derive(Component)]
struct Root;

/// The names of the texts the window shows (`menu::Words`, which the menus and the game share).
pub fn names() -> impl Iterator<Item = &'static str> {
    TITLES
        .iter()
        .map(|(_, (name, _))| *name)
        .chain(LABELS.iter().map(|(_, _, _, (name, _))| *name))
        .chain(Foot::ALL.iter().map(|(_, _, (name, _))| *name))
}

/// The words of the text named `name`, else `word`.
fn word(words: &Words, (name, word): (&str, &str)) -> String {
    words.get(name).unwrap_or(word).to_owned()
}

/// Opens the menus' window, as the main menu's Options does.
fn open(world: &mut World) {
    open_in(world, Mode::Menu);
}

/// Opens the game's window where the key O asked for it, the game paused under it.
fn open_game(world: &mut World) {
    if !world.resource::<Asked>().0 || world.contains_resource::<Draft>() {
        return;
    }
    open_in(world, Mode::Game);
}

/// Opens the game's window in the menu over a game's place, the game still paused (its Options).
pub fn open_in_game(world: &mut World) {
    world.resource_mut::<Asked>().0 = true;
    open_in(world, Mode::Game);
}

/// Opens the window in `mode`, its controls set from the settings in force (`0x718660`).
fn open_in(world: &mut World, mode: Mode) {
    let settings = world
        .get_resource::<Settings>()
        .cloned()
        .unwrap_or_default();
    world.insert_resource(Draft {
        settings,
        mode,
        lit: None,
    });
    world.run_system_cached(spawn).ok();
}

/// The window: the background, the fills, the frames, the titles, the separators, the labels,
/// the ticks, the round choices, the sliders and the buttons.
fn spawn(
    mut commands: Commands,
    (images, ui, choices): (Res<Images>, Res<UiImages>, Res<choice::Images>),
    (assets, words, draft): (Res<AssetServer>, Res<Words>, Res<Draft>),
) {
    let atlases = Atlases::load(&choices, &assets);
    let font: Handle<Font> = assets.load(FONT);
    let texts = &*words;
    let mut root = commands.spawn((
        Root,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::BLACK),
        GlobalZIndex(15),
        Pickable::default(),
    ));
    match draft.mode {
        Mode::Menu => root.insert(DespawnOnExit(Screen::Options)),
        Mode::Game => root.insert(DespawnOnExit(Play::Paused)),
    };
    root.with_children(|root| {
        root.spawn((busy_dialog::screen(), Pickable::IGNORE))
            .with_children(|screen| {
                images.background.spawn(screen, &assets, Pickable::IGNORE);
                for framed in BOXES {
                    let fill = framed.inflate(FILL_GROWTH);
                    screen.spawn((
                        choice::place(fill),
                        BackgroundColor(choice::SHADE),
                        Pickable::IGNORE,
                    ));
                }
                for framed in BOXES {
                    choice::frame_about(screen, &atlases, framed);
                }
                for at in SEPARATORS {
                    bar_of(screen, (&ui, &assets), SEPARATOR, at, SEPARATOR_WIDTH);
                }
                for (at, text) in TITLES {
                    bar_of(screen, (&ui, &assets), TITLE_BAR, at, TITLE_WIDTH);
                    screen.spawn((
                        Node {
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..choice::place(boxed(at.x, at.y, TITLE_WIDTH, 16.0))
                        },
                        Pickable::IGNORE,
                        children![(
                            Text::new(word(texts, text)),
                            lettering(&font, TITLE_SIZE),
                            TextColor(WHITE),
                            TextLayout::no_wrap(),
                            Pickable::IGNORE,
                        )],
                    ));
                }
                for (at, width, align, text) in LABELS {
                    screen.spawn((
                        Node {
                            justify_content: match align {
                                Align::Left => JustifyContent::FlexStart,
                                Align::Right => JustifyContent::FlexEnd,
                            },
                            align_items: AlignItems::FlexStart,
                            ..choice::place(boxed(at.x, at.y + LABEL_DROP, width, LABEL_SIZE))
                        },
                        Pickable::IGNORE,
                        children![(
                            Text::new(word(texts, text)),
                            lettering(&font, LABEL_SIZE),
                            TextColor(WHITE),
                            TextLayout::no_wrap(),
                            Pickable::IGNORE,
                        )],
                    ));
                }
                for (tick, at) in Tick::ALL {
                    let index = orb(tick.get(&draft.settings));
                    screen
                        .spawn((
                            tick,
                            bar::piece(&ui, &assets, index, at, Vec2::ZERO),
                            crate::control_glow::Glows,
                            Pickable::default(),
                        ))
                        .observe(move |press: On<PointerPress>, mut commands: Commands| {
                            if press.button == PointerButton::Primary {
                                commands.run_system_cached_with(flip, tick);
                            }
                        });
                }
                for (choice, at) in Choice::ALL {
                    let index = orb(choice.chosen(&draft.settings));
                    screen
                        .spawn((
                            choice,
                            bar::piece(&ui, &assets, index, at, Vec2::ZERO),
                            crate::control_glow::Glows,
                            Pickable::default(),
                        ))
                        .observe(move |press: On<PointerPress>, mut commands: Commands| {
                            if press.button == PointerButton::Primary {
                                commands.run_system_cached_with(pick, choice);
                            }
                        });
                }
                for (slider, rect) in Slider::ALL {
                    slider_pieces(screen, (&ui, &assets), slider, rect, &draft.settings);
                }
                for (foot, at, text) in Foot::ALL {
                    button(screen, (&ui, &assets, &font), foot, at, word(texts, text));
                }
            });
    });
}

/// The ticked or the plain orb.
fn orb(ticked: bool) -> u32 {
    if ticked { TICKED } else { UNTICKED }
}

/// A slider's pieces at `rect`, its knob at its setting's position, and the rectangle that takes
/// the wheel.
fn slider_pieces(
    screen: &mut ChildSpawnerCommands,
    (ui, assets): (&UiImages, &AssetServer),
    slider: Slider,
    rect: Rect,
    settings: &Settings,
) {
    screen.spawn((
        stretched(
            (ui, assets),
            BAR,
            Vec2::new(rect.min.x + MINUS_WIDTH, rect.min.y),
            Vec2::new(
                rect.width() - MINUS_WIDTH - PLUS_WIDTH,
                ui.size(BAR).y.max(1.0),
            ),
        ),
        Pickable::IGNORE,
    ));
    screen.spawn((
        bar::piece(ui, assets, MINUS, rect.min, Vec2::ZERO),
        Pickable::IGNORE,
    ));
    screen.spawn((
        bar::piece(
            ui,
            assets,
            PLUS,
            Vec2::new(rect.max.x - PLUS_WIDTH, rect.min.y),
            Vec2::ZERO,
        ),
        Pickable::IGNORE,
    ));
    let x = knob_x(slider.get(settings), slider.positions(), rect);
    let y = rect.center().y - KNOB_HALF.y;
    screen.spawn((
        Knob(slider),
        bar::piece(ui, assets, KNOB, Vec2::new(x, y), Vec2::ZERO),
        Pickable::IGNORE,
    ));
    screen
        .spawn((slider, choice::place(rect), Pickable::default()))
        .observe(
            move |scroll: On<PointerScroll>,
                  mut draft: ResMut<Draft>,
                  levels: Option<ResMut<crate::volumes::Levels>>| {
                if scroll.y == 0.0 {
                    return;
                }
                let position = slider.get(&draft.settings);
                let moved = step(position, slider.positions(), scroll.y > 0.0);
                if moved != position {
                    *slider.slot(&mut draft.settings) = moved;
                }
                // Heard at once, as a press moves it (docs/decisions/sound.md).
                if let (Some(kind), Some(mut levels)) = (slider.kind(), levels) {
                    levels.moved(kind, moved);
                }
            },
        );
}

/// A button of the foot: its pieces and its words, which a click presses.
fn button(
    screen: &mut ChildSpawnerCommands,
    (ui, assets, font): (&UiImages, &AssetServer, &Handle<Font>),
    foot: Foot,
    at: Vec2,
    text: String,
) {
    let [left, middle, right] = BUTTON;
    screen.spawn((
        bar::piece(
            ui,
            assets,
            left,
            at - Vec2::new(ui.size(left).x, 0.0),
            Vec2::ZERO,
        ),
        Pickable::IGNORE,
    ));
    screen.spawn((
        stretched((ui, assets), middle, at, BUTTON_SIZE),
        Pickable::IGNORE,
    ));
    screen.spawn((
        bar::piece(
            ui,
            assets,
            right,
            at + Vec2::new(BUTTON_SIZE.x, 0.0),
            Vec2::ZERO,
        ),
        Pickable::IGNORE,
    ));
    screen
        .spawn((
            foot,
            Node {
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..choice::place(Rect::from_corners(
                    at - Vec2::new(0.0, BUTTON_RISE),
                    at + BUTTON_SIZE - Vec2::new(0.0, BUTTON_RISE),
                ))
            },
            Pickable::default(),
            children![(
                Text::new(text),
                lettering(font, WORDS),
                TextColor(WHITE),
                TextLayout::no_wrap(),
                Pickable::IGNORE,
            )],
        ))
        .observe(move |_: On<PointerOver>, mut draft: ResMut<Draft>| {
            draft.lit = Some(foot);
        })
        .observe(move |_: On<PointerOut>, mut draft: ResMut<Draft>| {
            if draft.lit == Some(foot) {
                draft.lit = None;
            }
        })
        .observe(move |click: On<PointerClick>, mut commands: Commands| {
            if click.button == PointerButton::Primary {
                commands.run_system_cached_with(pressed, foot);
            }
        });
}

/// A tick pressed: its setting turned over, with its click (`0x72B560`); the sound's at once,
/// whatever closes the window after (`0x717696`).
fn flip(
    In(tick): In<Tick>,
    mut draft: ResMut<Draft>,
    mut sounds: MessageWriter<crate::sfx::Play>,
    levels: Option<ResMut<crate::volumes::Levels>>,
) {
    crate::inventory::play(&mut sounds, crate::inventory::CLICK);
    let slot = tick.slot(&mut draft.settings);
    *slot = !*slot;
    if tick == Tick::Sound
        && let Some(mut levels) = levels
    {
        levels.on = *slot;
    }
}

/// A round choice pressed: it holds and the others of its group do not, with its click; a press
/// on the one that holds leaves it (`0x7172F0`).
fn pick(
    In(choice): In<Choice>,
    mut draft: ResMut<Draft>,
    mut sounds: MessageWriter<crate::sfx::Play>,
) {
    crate::inventory::play(&mut sounds, crate::inventory::CLICK);
    if !choice.chosen(&draft.settings) {
        choice.choose(&mut draft.settings);
    }
}

/// The pace of a held button's presses: the original's one a drawn frame, at its 60 frames a
/// second.
const HELD_STEP: f32 = 1.0 / 60.0;

/// The sliders under the cursor pressed at it: once by the press itself and once by its frame's
/// draw, then, while the left button is held, by every drawn frame at the original's pace, so
/// that a held "−" or "+" moves a position a frame and a held press on the bar drags the knob
/// after the cursor (`0x752A60`, `0x716E00`; seen: a click of one frame moves two positions, and
/// a held one a position every frame).
///
/// A volume's slider so pressed sets its volume at once, over 126 (`0x716E00`), which neither
/// Cancel nor Esc sets back.
fn drag(
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    (time, mut held): (Res<Time<Real>>, Local<f32>),
    mut draft: ResMut<Draft>,
    mut levels: Option<ResMut<crate::volumes::Levels>>,
) {
    let presses = if buttons.just_pressed(MouseButton::Left) {
        *held = 0.0;
        2
    } else if buttons.pressed(MouseButton::Left) {
        *held += time.delta_secs();
        let mut presses = 0;
        while *held >= HELD_STEP {
            *held -= HELD_STEP;
            presses += 1;
        }
        presses
    } else {
        return;
    };
    let Some(at) = crate::inventory::screen_point(&pointer, &ui_scale, &windows) else {
        return;
    };
    for (slider, rect) in Slider::ALL {
        if !rect.contains(at) {
            continue;
        }
        let position = slider.get(&draft.settings);
        let moved = (0..presses).fold(position, |position, _| {
            pressed_at(position, slider.positions(), rect, at)
        });
        if moved != position {
            *slider.slot(&mut draft.settings) = moved;
        }
        if presses > 0
            && let (Some(kind), Some(levels)) = (slider.kind(), levels.as_deref_mut())
        {
            levels.moved(kind, moved);
        }
    }
}

/// Puts the controls' pictures and the knobs at the window's settings as they change, and lights
/// the button under the cursor.
fn redraw(
    draft: Res<Draft>,
    (ui, assets): (Res<UiImages>, Res<AssetServer>),
    mut ticks: Query<(&Tick, &mut ImageNode), Without<Choice>>,
    mut choices: Query<(&Choice, &mut ImageNode), Without<Tick>>,
    mut knobs: Query<(&Knob, &mut Node)>,
    feet: Query<(&Foot, &Children)>,
    mut words: Query<&mut TextColor>,
) {
    if !draft.is_changed() {
        return;
    }
    let picture = |ticked: bool| bar::piece(&ui, &assets, orb(ticked), Vec2::ZERO, Vec2::ZERO).0;
    for (tick, mut image) in &mut ticks {
        let wanted = picture(tick.get(&draft.settings));
        if image.rect != wanted.rect {
            *image = wanted;
        }
    }
    for (choice, mut image) in &mut choices {
        let wanted = picture(choice.chosen(&draft.settings));
        if image.rect != wanted.rect {
            *image = wanted;
        }
    }
    for (knob, mut node) in &mut knobs {
        let slider = knob.0;
        let rect = Slider::ALL
            .iter()
            .find(|(of, _)| *of == slider)
            .map_or(Rect::default(), |&(_, rect)| rect);
        let x = Val::Px(knob_x(
            slider.get(&draft.settings),
            slider.positions(),
            rect,
        ));
        if node.left != x {
            node.left = x;
        }
    }
    for (foot, children) in &feet {
        let colour = if draft.lit == Some(*foot) { LIT } else { WHITE };
        for &child in children {
            if let Ok(mut word) = words.get_mut(child) {
                word.set_if_neq(TextColor(colour));
            }
        }
    }
}

/// Cancel or OK pressed, with its click: OK writes the window's settings into the settings in
/// force and their keys into the file (`0x717C20`); either goes back to the main menu, or on with
/// the game.
fn pressed(
    In(foot): In<Foot>,
    draft: Res<Draft>,
    (mut settings, mut file): (ResMut<Settings>, ResMut<SettingsFile>),
    screen: Option<ResMut<NextState<Screen>>>,
    play: Option<ResMut<NextState<Play>>>,
    mut sounds: MessageWriter<crate::sfx::Play>,
) {
    crate::inventory::play(&mut sounds, crate::inventory::CLICK);
    if foot == Foot::Ok {
        confirm(&draft.settings, &mut settings);
        if let Err(e) = file.save(&settings, &KEYS) {
            warn!("options: {e:#}");
        }
    }
    leave(draft.mode, screen, play);
}

/// Back to the main menu, or on with the game.
fn leave(
    mode: Mode,
    screen: Option<ResMut<NextState<Screen>>>,
    play: Option<ResMut<NextState<Play>>>,
) {
    match mode {
        Mode::Menu => {
            if let Some(mut screen) = screen {
                screen.set(Screen::Main);
            }
        }
        Mode::Game => {
            if let Some(mut play) = play {
                play.set(Play::Running);
            }
        }
    }
}

/// The key O in a game (`cUI_Manager_receiveEvent`, `0x758024`): the game's window, the game
/// paused under it with no menu over it.
fn o_key(
    mut keys: crate::controls::KeysMut,
    (mut asked, mut play): (ResMut<Asked>, ResMut<NextState<Play>>),
) {
    if keys.take(crate::controls::Action::Options) {
        asked.0 = true;
        play.set(Play::Paused);
    }
}

/// The key O again while the game's window shows: it closes, writing nothing (`0x758798`).
fn o_again(
    mut keys: crate::controls::KeysMut,
    draft: Res<Draft>,
    play: Option<ResMut<NextState<Play>>>,
) {
    if draft.mode == Mode::Game && keys.take(crate::controls::Action::Options) {
        leave(Mode::Game, None, play);
    }
}

/// The window's settings forgotten as it closes.
fn close(mut commands: Commands) {
    commands.remove_resource::<Draft>();
}

/// The game's window closes with the pause.
fn close_game(commands: Commands, mut asked: ResMut<Asked>) {
    asked.0 = false;
    close(commands);
}

/// The options window.
pub struct OptionsPlugin;

impl Plugin for OptionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Asked>()
            .add_systems(OnEnter(Screen::Options), open)
            .add_systems(OnExit(Screen::Options), close)
            .add_systems(OnEnter(Play::Paused), open_game)
            .add_systems(OnExit(Play::Paused), close_game)
            .add_systems(
                Update,
                (
                    o_key.run_if(in_state(Play::Running)),
                    (o_again, drag, redraw)
                        .chain()
                        .run_if(resource_exists::<Draft>),
                ),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slider_steps_by_its_buttons_and_puts_its_knob_at_a_press() {
        let (_, rect) = Slider::ALL[1];
        let n = Slider::Effects.positions();
        // "−" and "+" a position each, within 0 and 125.
        assert_eq!(pressed_at(64, n, rect, Vec2::new(650.0, 80.0)), 63);
        assert_eq!(pressed_at(0, n, rect, Vec2::new(650.0, 80.0)), 0);
        assert_eq!(pressed_at(64, n, rect, Vec2::new(930.0, 80.0)), 65);
        assert_eq!(pressed_at(125, n, rect, Vec2::new(930.0, 80.0)), 125);
        // The bar: the run from x 700, 181 wide; its ends held to 0 and the last.
        assert_eq!(pressed_at(64, n, rect, Vec2::new(690.0, 80.0)), 0);
        assert_eq!(pressed_at(64, n, rect, Vec2::new(700.0 + 90.5, 80.0)), 63);
        assert_eq!(pressed_at(0, n, rect, Vec2::new(894.0, 80.0)), 125);
        // The transparency's 255 positions, 0 to 254.
        let (_, rect) = Slider::ALL[0];
        assert_eq!(pressed_at(0, 255, rect, Vec2::new(440.0, 180.0)), 1);
        assert_eq!(pressed_at(254, 255, rect, Vec2::new(440.0, 180.0)), 254);
        assert_eq!(pressed_at(0, 255, rect, Vec2::new(405.0, 180.0)), 254);
    }

    #[test]
    fn a_knob_runs_over_the_bar_less_its_width() {
        let (_, rect) = Slider::ALL[1];
        // The frame's knob at the volume 0: its left edge at 685.
        assert_eq!(knob_x(0, 126, rect), 685.0);
        let (_, rect) = Slider::ALL[0];
        assert!(
            (knob_x(127, 255, rect) - (260.0 + 52.0 + 127.0 / 255.0 * 85.0 - 15.0)).abs() < 1e-4
        );
    }

    #[test]
    fn the_controls_set_their_settings_and_ok_takes_the_windows_alone() {
        let mut draft = Settings::default();
        for (tick, _) in Tick::ALL {
            let before = tick.get(&draft);
            *tick.slot(&mut draft) = !before;
            assert_eq!(tick.get(&draft), !before, "{tick:?}");
        }
        Choice::Collect(Collect::Gold).choose(&mut draft);
        Choice::Detail(0).choose(&mut draft);
        Choice::Network(NetworkSpeed::Slow).choose(&mut draft);
        assert!(Choice::Detail(0).chosen(&draft) && !Choice::Detail(2).chosen(&draft));
        *Slider::Music.slot(&mut draft) = 3;
        draft.display.width = 640;
        draft.interface.warning_level = 90;
        let mut settings = Settings::default();
        confirm(&draft, &mut settings);
        assert_eq!(settings.gameplay, draft.gameplay);
        assert_eq!(settings.sound, draft.sound);
        assert_eq!(settings.graphics, draft.graphics);
        assert_eq!(settings.network, draft.network);
        assert_eq!(settings.interface.map_alpha, draft.interface.map_alpha);
        // What the window does not hold stays.
        assert_eq!(settings.display, Settings::default().display);
        assert_eq!(settings.interface.warning_level, 25);
    }

    #[test]
    fn ok_writes_the_windows_keys_and_they_read_back() {
        let dir =
            std::env::temp_dir().join(format!("opensacred-game-options-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("settings.toml");
        let (mut file, mut settings) = SettingsFile::read(Some(path.clone()));
        let mut draft = settings.clone();
        draft.sound.effects = 0;
        draft.gameplay.collect = Collect::Valuables;
        confirm(&draft, &mut settings);
        file.save(&settings, &KEYS).unwrap();
        let (_, read) = SettingsFile::read(Some(path));
        assert_eq!(read, settings);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
