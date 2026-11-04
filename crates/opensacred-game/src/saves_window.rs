//! The saves' window (`UI_WND_SAVEGAME`, `cUI_Savegame`; #1990, docs/re/menus.md, "The saves'
//! window"): the slots' saves over the whole of the original's screen, the quick save's section
//! above and the list below, three rows shown at a time, each its picture (`save_picture`), its
//! name, its file and its date, with Load, Save, Delete and Back.
//!
//! The main menu's Load Game shows it for loading ([`Mode::Load`], the menus' own window, Save
//! greyed and no empty row); Load asks before it loads, Delete before it erases, and Back and Esc
//! go back to the main menu. Each window keeps its list's place between its openings, and each
//! opening selects the first row (`0x71D720`). The window plays no sound of its own, as the
//! original's buttons are not its children (seen).

use bevy::asset::RenderAssetUsages;
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::picking::events::{PointerClick, PointerOut, PointerOver, PointerScroll};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::bar::{self, UiImages};
use crate::busy_dialog::{self, word};
use crate::choice::{self, Atlases};
use crate::game::FONT;
use crate::lines::Texts;
use crate::menu::Screen;
use crate::saves::{self, Slot};

/// How the window shows: for loading from the menus (the original's mode 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Load,
}

/// The background's tiles (`UW_option_01.tga` to `UW_option_12.tga`), four to a row.
const TILES: usize = 12;
const TILE: f32 = 256.0;
/// The dark fills behind the two boxes (`0x9F000000`).
const FILLS: [Rect; 2] = [
    Rect::from_corners(Vec2::new(44.0, 44.0), Vec2::new(724.0, 180.0)),
    Rect::from_corners(Vec2::new(44.0, 300.0), Vec2::new(724.0, 692.0)),
];
const FILL: Color = Color::linear_rgba(0.0, 0.0, 0.0, 159.0 / 255.0);
/// The boxes that the frames stand about: the quick save's and the list's.
const BOXES: [Rect; 2] = [
    Rect::from_corners(Vec2::new(64.0, 64.0), Vec2::new(704.0, 160.0)),
    Rect::from_corners(Vec2::new(64.0, 320.0), Vec2::new(704.0, 672.0)),
];
/// The separators between the list's rows: their left end, middle and right end, their rows.
const SEPARATOR: [u32; 3] = [296, 294, 297];
const SEPARATORS: [f32; 2] = [419.0, 555.0];
/// The labels' bars (left, middle, right) and their places, the labels' texts over them in the
/// font 7, white.
const LABEL_BAR: [u32; 3] = [342, 343, 344];
const LABELS: [(f32, (&str, &str)); 2] = [
    (32.0, ("UI_SAVE_QUICKSAVE", "Quicksave")),
    (288.0, ("UI_SAVE_SAVEGAMES", "Savegames")),
];
const LABEL_WIDTH: f32 = 137.0;
const LABEL_SIZE: f32 = 13.0;
/// The groups: the quick save's (0) and the list's three rows, each its rectangle, which a click
/// selects, and its picture's corner (`+0x72C`, `+0x6FC`).
const GROUPS: [(Rect, Vec2); 4] = [
    (
        Rect::from_corners(Vec2::new(48.0, 48.0), Vec2::new(720.0, 176.0)),
        Vec2::new(72.0, 64.0),
    ),
    (
        Rect::from_corners(Vec2::new(48.0, 304.0), Vec2::new(720.0, 416.0)),
        Vec2::new(72.0, 312.0),
    ),
    (
        Rect::from_corners(Vec2::new(48.0, 439.0), Vec2::new(720.0, 551.0)),
        Vec2::new(72.0, 447.0),
    ),
    (
        Rect::from_corners(Vec2::new(48.0, 575.0), Vec2::new(720.0, 687.0)),
        Vec2::new(72.0, 583.0),
    ),
];
/// A group's three fields, from its picture's top: the name, the file's name and the date, each
/// 384 × 26 from x 264, its pieces' images (the read-only set's) and its text's inset.
const FIELD_X: f32 = 264.0;
const FIELD_SIZE: Vec2 = Vec2::new(384.0, 26.0);
const FIELD_STEP: f32 = 34.0;
const FIELD: [u32; 3] = [332, 333, 334];
const FIELD_INSET: Vec2 = Vec2::new(4.0, 1.0);
/// The font 1's size, of the fields' and the buttons' words.
const WORDS: f32 = 20.0;
/// The words' colours: white, the selected group's and a lit button's `0xFFFFFF80`, a button not
/// enabled `0xFF7F7F7F`, in the frame's sRGB values (`hud`), as the window's other colours.
const WHITE: Color = Color::WHITE;
const SELECTED: Color = Color::linear_rgb(1.0, 1.0, 128.0 / 255.0);
const DISABLED: Color = Color::linear_rgb(127.0 / 255.0, 127.0 / 255.0, 127.0 / 255.0);
/// An empty row's picture (`0x5555`: a grey of 85 at alpha 85).
const EMPTY_PICTURE: Color =
    Color::linear_rgba(85.0 / 255.0, 85.0 / 255.0, 85.0 / 255.0, 85.0 / 255.0);
/// The quick save's plate where there is none (`ui_sl_quicksave1`'s halves and `ui_sl_quicksave2`).
const PLATE: [(u32, f32); 3] = [(1511, 48.0), (1512, 304.0), (1513, 560.0)];
/// The selection's corners: top left, top right, bottom left, bottom right (`0x71DCE0`).
const CORNERS: [u32; 4] = [278, 276, 279, 277];
/// The scroll bar (`0x752670`): the up arrow, its track, the down arrow and the thumb, and where
/// the thumb runs (**seen**: from the row 357 at the top to above the down arrow).
const UP: (u32, Vec2) = (287, Vec2::new(720.0, 336.0));
const DOWN: (u32, Vec2) = (289, Vec2::new(720.0, 635.0));
const TRACK: u32 = 288;
const THUMB: u32 = 286;
const THUMB_RUN: (f32, f32) = (357.0, 613.0);
/// The buttons, at x 820, 92 × 32, their pieces (the middle stretched over the rectangle, the
/// ends outside it) and their texts, the word four rows up.
const BUTTON_X: f32 = 820.0;
const BUTTON_SIZE: Vec2 = Vec2::new(92.0, 32.0);
const BUTTON: [u32; 3] = [280, 281, 282];
const BUTTON_RISE: f32 = 4.0;

/// A button of the window.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum Button {
    Load,
    Save,
    Erase,
    Back,
}

impl Button {
    const ALL: [Self; 4] = [Self::Load, Self::Save, Self::Erase, Self::Back];

    fn row(self) -> f32 {
        match self {
            Self::Load => 288.0,
            Self::Save => 352.0,
            Self::Erase => 416.0,
            Self::Back => 672.0,
        }
    }

    fn text(self) -> (&'static str, &'static str) {
        match self {
            Self::Load => ("UI_SAVE_LOAD", "Load"),
            Self::Save => ("UI_SAVE_SAVE", "Save"),
            Self::Erase => ("UI_SAVE_ERASE", "Delete"),
            Self::Back => ("UI_NET_BACK", "Back"),
        }
    }
}

/// The window's state: its mode, the slots' saves, the list's place (the scroll bar's, which may
/// run past the last place the view shows) and the selected group, 0 the quick save's.
#[derive(Resource)]
pub struct Listing {
    mode: Mode,
    quick: Option<Slot>,
    saves: Vec<Slot>,
    place: usize,
    selected: usize,
    /// The button under the cursor, whose word is lit.
    lit: Option<Button>,
    /// The user's short date's pattern, read as the window opens.
    pattern: String,
}

impl Listing {
    /// The first of the list's saves shown: the place, held at most at the last three
    /// (`0x71C2B0`: length − 3 for loading).
    fn first(&self) -> usize {
        let rows = match self.mode {
            Mode::Load => 3,
        };
        self.place.min(self.saves.len().saturating_sub(rows))
    }

    /// The save of the group `group`, 0 the quick save's.
    fn save_of(&self, group: usize) -> Option<&Slot> {
        match group {
            0 => self.quick.as_ref(),
            row => self.saves.get(self.first() + row - 1),
        }
    }

    /// Whether `button` may be used: Load and Delete where the selected group holds a save, Save
    /// not for loading, Back always (`0x71C360`).
    fn enabled(&self, button: Button) -> bool {
        match button {
            Button::Load | Button::Erase => self.save_of(self.selected).is_some(),
            Button::Save => false,
            Button::Back => true,
        }
    }

    /// One row up or down the list, the scroll bar's place within the list's length.
    fn scroll(&mut self, down: bool) {
        let last = self.saves.len().saturating_sub(1);
        self.place = if down {
            (self.place + 1).min(last)
        } else {
            self.place.saturating_sub(1)
        };
    }
}

/// The places that the windows keep between their openings: the menus' (#1990).
#[derive(Resource, Default)]
struct Kept {
    menu: usize,
}

/// The background's textures, found among the converted ones.
#[derive(Resource, Default)]
pub struct Images {
    tiles: Vec<Option<String>>,
}

impl Images {
    /// Finds the background's tiles.
    pub fn find(assets: &std::path::Path) -> Self {
        let names: Vec<String> = (1..=TILES).map(|n| format!("uw_option_{n:02}")).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        Self {
            tiles: crate::hud::find_textures(assets, &names),
        }
    }
}

/// The window's root, its part that its listing redraws, and a question it asks.
#[derive(Component)]
struct Root;
#[derive(Component)]
struct Rows;
#[derive(Component)]
struct Question;

/// A question's answer and what it answers.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum Answer {
    Load(bool),
    Erase(bool),
}

/// Opens the window for loading, as the main menu's Load Game does: its list read, its place
/// kept, its first row selected.
fn open(world: &mut World) {
    let (quick, saves) = saves::saves_directory(world)
        .map(|dir| saves::slots(&dir))
        .unwrap_or_default();
    let place = world.get_resource::<Kept>().map_or(0, |kept| kept.menu);
    let place = place.min(saves.len().saturating_sub(1));
    world.insert_resource(Listing {
        mode: Mode::Load,
        quick,
        saves,
        place,
        selected: 1,
        lit: None,
        pattern: short_date_pattern(),
    });
    world.run_system_cached(spawn).ok();
}

/// The window's still parts: the background, the fills, the frames, the separators, the labels,
/// the scroll bar's arrows and the buttons; the rows apart ([`redraw`]).
fn spawn(
    mut commands: Commands,
    (images, ui, choices): (Res<Images>, Res<UiImages>, Res<choice::Images>),
    (assets, texts): (Res<AssetServer>, Option<Res<Texts>>),
) {
    let atlases = Atlases::load(&choices, &assets);
    let font: Handle<Font> = assets.load(FONT);
    let texts = texts.as_deref();
    commands
        .spawn((
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
            DespawnOnExit(Screen::Load),
        ))
        .observe(wheel)
        .with_children(|root| {
            root.spawn((busy_dialog::screen(), Pickable::IGNORE))
                .with_children(|screen| {
                    for (k, tile) in images.tiles.iter().enumerate() {
                        let Some(file) = tile else { continue };
                        let at = Vec2::new(
                            TILE * f32::from(u8::try_from(k % 4).unwrap_or(0)),
                            TILE * f32::from(u8::try_from(k / 4).unwrap_or(0)),
                        );
                        screen.spawn((
                            ImageNode::new(crate::hud::image(&assets, file.clone())),
                            bar::placed(at, Vec2::splat(TILE)),
                            Pickable::IGNORE,
                        ));
                    }
                    for fill in FILLS {
                        screen.spawn((
                            choice::place(fill),
                            BackgroundColor(FILL),
                            Pickable::IGNORE,
                        ));
                    }
                    for framed in BOXES {
                        choice::frame_about(screen, &atlases, framed);
                    }
                    for y in SEPARATORS {
                        bar_of(screen, (&ui, &assets), SEPARATOR, Vec2::new(44.0, y), 680.0);
                    }
                    for (y, text) in LABELS {
                        bar_of(
                            screen,
                            (&ui, &assets),
                            LABEL_BAR,
                            Vec2::new(64.0, y),
                            LABEL_WIDTH,
                        );
                        screen.spawn((
                            Node {
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..choice::place(Rect::new(64.0, y, 64.0 + LABEL_WIDTH, y + 16.0))
                            },
                            Pickable::IGNORE,
                            children![(
                                Text::new(word(texts, text)),
                                lettering(&font, LABEL_SIZE),
                                TextColor(WHITE),
                                Pickable::IGNORE,
                            )],
                        ));
                    }
                    // The scroll bar's track between its arrows, and the arrows.
                    let up_size = ui.size(UP.0);
                    screen.spawn((
                        stretched(
                            (&ui, &assets),
                            TRACK,
                            Vec2::new(UP.1.x, UP.1.y + up_size.y),
                            Vec2::new(ui.size(TRACK).x, DOWN.1.y - UP.1.y - up_size.y),
                        ),
                        Pickable::IGNORE,
                    ));
                    for (index, at, down) in [(UP.0, UP.1, false), (DOWN.0, DOWN.1, true)] {
                        screen
                            .spawn((
                                bar::piece(&ui, &assets, index, at, Vec2::ZERO),
                                Pickable::default(),
                            ))
                            .observe(
                                move |click: On<PointerClick>, mut listing: ResMut<Listing>| {
                                    if click.button == PointerButton::Primary {
                                        listing.scroll(down);
                                    }
                                },
                            );
                    }
                    for button in Button::ALL {
                        let at = Vec2::new(BUTTON_X, button.row());
                        let [left, middle, right] = BUTTON;
                        screen.spawn((
                            bar::piece(
                                &ui,
                                &assets,
                                left,
                                at - Vec2::new(ui.size(left).x, 0.0),
                                Vec2::ZERO,
                            ),
                            Pickable::IGNORE,
                        ));
                        screen.spawn((
                            stretched((&ui, &assets), middle, at, BUTTON_SIZE),
                            Pickable::IGNORE,
                        ));
                        screen.spawn((
                            bar::piece(
                                &ui,
                                &assets,
                                right,
                                at + Vec2::new(BUTTON_SIZE.x, 0.0),
                                Vec2::ZERO,
                            ),
                            Pickable::IGNORE,
                        ));
                        screen
                            .spawn((
                                button,
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
                                    Text::new(word(texts, button.text())),
                                    lettering(&font, WORDS),
                                    TextColor(WHITE),
                                    Pickable::IGNORE,
                                )],
                            ))
                            .observe(move |_: On<PointerOver>, mut listing: ResMut<Listing>| {
                                listing.lit = Some(button);
                            })
                            .observe(move |_: On<PointerOut>, mut listing: ResMut<Listing>| {
                                if listing.lit == Some(button) {
                                    listing.lit = None;
                                }
                            })
                            .observe(move |click: On<PointerClick>, mut commands: Commands| {
                                if click.button == PointerButton::Primary {
                                    commands.run_system_cached_with(pressed, button);
                                }
                            });
                    }
                    screen.spawn((
                        Rows,
                        Node {
                            position_type: PositionType::Absolute,
                            ..choice::place(Rect::new(0.0, 0.0, bar::SCREEN.x, bar::SCREEN.y))
                        },
                        Pickable::IGNORE,
                    ));
                });
        });
}

/// A bar of three pieces from `at`, `width` wide: its left end, its middle repeated at its width
/// and its right end at the bar's end, each at its natural size.
fn bar_of(
    parent: &mut ChildSpawnerCommands,
    (ui, assets): (&UiImages, &AssetServer),
    [left, middle, right]: [u32; 3],
    at: Vec2,
    width: f32,
) {
    let end = at.x + width - ui.size(right).x;
    parent.spawn((
        bar::piece(ui, assets, left, at, Vec2::ZERO),
        Pickable::IGNORE,
    ));
    let step = ui.size(middle).x.max(1.0);
    let mut x = at.x + ui.size(left).x;
    while x < end {
        parent.spawn((
            bar::piece(ui, assets, middle, Vec2::new(x, at.y), Vec2::ZERO),
            Pickable::IGNORE,
        ));
        x += step;
    }
    parent.spawn((
        bar::piece(ui, assets, right, Vec2::new(end, at.y), Vec2::ZERO),
        Pickable::IGNORE,
    ));
}

/// The image `index` stretched over `size` at `at`.
fn stretched(
    (ui, assets): (&UiImages, &AssetServer),
    index: u32,
    at: Vec2,
    size: Vec2,
) -> (ImageNode, Node) {
    let (mut image, _) = bar::piece(ui, assets, index, at, Vec2::ZERO);
    image.image_mode = NodeImageMode::Stretch;
    (image, bar::placed(at, size))
}

fn lettering(font: &Handle<Font>, size: f32) -> TextFont {
    TextFont {
        font: font.clone().into(),
        font_size: bevy::text::FontSize::Px(size),
        ..default()
    }
}

/// Draws the rows anew as the listing changes: the quick save's group or its plate, the three
/// rows from the first shown, the selection's corners, the scroll bar's thumb and the buttons'
/// words' colours.
fn redraw(
    mut commands: Commands,
    listing: Option<Res<Listing>>,
    rows: Query<Entity, With<Rows>>,
    (ui, assets, texts): (Res<UiImages>, Res<AssetServer>, Option<Res<Texts>>),
    mut pictures: ResMut<Assets<Image>>,
    buttons: Query<(&Button, &Children)>,
    mut words: Query<&mut TextColor>,
) {
    let Some(listing) = listing.filter(|listing| listing.is_changed()) else {
        return;
    };
    let Ok(rows) = rows.single() else {
        return;
    };
    for (button, children) in &buttons {
        let colour = if !listing.enabled(*button) {
            DISABLED
        } else if listing.lit == Some(*button) {
            SELECTED
        } else {
            WHITE
        };
        for &child in children {
            if let Ok(mut word) = words.get_mut(child) {
                word.0 = colour;
            }
        }
    }
    let font: Handle<Font> = assets.load(FONT);
    let texts = texts.as_deref();
    let empty = word(texts, ("UI_SAVE_EMPTY", "<empty>"));
    commands
        .entity(rows)
        .despawn_children()
        .with_children(|rows| {
            for (group, &(rect, picture_at)) in GROUPS.iter().enumerate() {
                let save = listing.save_of(group);
                if group == 0 && save.is_none() {
                    for (index, x) in PLATE {
                        rows.spawn((
                            bar::piece(&ui, &assets, index, Vec2::new(x, rect.min.y), Vec2::ZERO),
                            Pickable::IGNORE,
                        ));
                    }
                } else {
                    match save.and_then(|save| save.picture.as_ref()) {
                        Some(picture) => {
                            let image = pictures.add(Image::new(
                                Extent3d {
                                    width: crate::save_picture::WIDTH,
                                    height: crate::save_picture::HEIGHT,
                                    depth_or_array_layers: 1,
                                },
                                TextureDimension::D2,
                                picture.rgba(),
                                TextureFormat::Rgba8Unorm,
                                RenderAssetUsages::RENDER_WORLD,
                            ));
                            rows.spawn((
                                ImageNode::new(image),
                                bar::placed(picture_at, PICTURE),
                                Pickable::IGNORE,
                            ));
                        }
                        None if save.is_none() => {
                            rows.spawn((
                                bar::placed(picture_at, PICTURE),
                                BackgroundColor(EMPTY_PICTURE),
                                Pickable::IGNORE,
                            ));
                        }
                        None => {}
                    }
                    let colour = if listing.selected == group {
                        SELECTED
                    } else {
                        WHITE
                    };
                    let lines = match save {
                        Some(save) => [
                            save.header.name.clone(),
                            format!("{}.sav", save.stem),
                            date_line(texts, save.header.saved, &listing.pattern),
                        ],
                        None => [empty.clone(), empty.clone(), empty.clone()],
                    };
                    for (k, line) in lines.into_iter().enumerate() {
                        let y = picture_at.y + FIELD_STEP * f32::from(u8::try_from(k).unwrap_or(0));
                        field(rows, (&ui, &assets), Vec2::new(FIELD_X, y));
                        rows.spawn((
                            Text::new(line),
                            lettering(&font, WORDS),
                            TextColor(colour),
                            TextLayout::no_wrap(),
                            Node {
                                overflow: Overflow::clip(),
                                ..bar::placed(
                                    Vec2::new(FIELD_X, y) + FIELD_INSET,
                                    FIELD_SIZE - FIELD_INSET,
                                )
                            },
                            Pickable::IGNORE,
                        ));
                    }
                }
                // The group's rectangle, which a click selects, a double press on a save loads.
                rows.spawn((choice::place(rect), Pickable::default()))
                    .observe(
                        move |click: On<PointerClick>,
                              mut listing: ResMut<Listing>,
                              mut commands: Commands| {
                            if click.button != PointerButton::Primary {
                                return;
                            }
                            listing.selected = group;
                            if click.count >= 2 && listing.enabled(Button::Load) {
                                commands.run_system_cached_with(pressed, Button::Load);
                            }
                        },
                    );
            }
            let (rect, _) = GROUPS[listing.selected.min(GROUPS.len() - 1)];
            for (k, index) in CORNERS.into_iter().enumerate() {
                let size = ui.size(index);
                let x = if k % 2 == 0 {
                    rect.min.x
                } else {
                    rect.max.x - size.x
                };
                let y = if k < 2 {
                    rect.min.y
                } else {
                    rect.max.y - size.y
                };
                rows.spawn((
                    bar::piece(&ui, &assets, index, Vec2::new(x, y), Vec2::ZERO),
                    Pickable::IGNORE,
                ));
            }
            let last = listing.saves.len().saturating_sub(1).max(1);
            let share = f32::from(u16::try_from(listing.place.min(last)).unwrap_or(0))
                / f32::from(u16::try_from(last).unwrap_or(1));
            let thumb_x = UP.1.x + (ui.size(UP.0).x - ui.size(THUMB).x) / 2.0;
            let thumb_y = THUMB_RUN.0 + (THUMB_RUN.1 - THUMB_RUN.0) * share;
            rows.spawn((
                bar::piece(&ui, &assets, THUMB, Vec2::new(thumb_x, thumb_y), Vec2::ZERO),
                Pickable::IGNORE,
            ));
        });
}

/// The picture's size.
const PICTURE: Vec2 = Vec2::new(128.0, 96.0);

/// A field's frame at `at`: its left end right against it, its middle stretched over it and its
/// right end at its end, as wide as the middle (the original's quirk, `0x7382F0`).
fn field(parent: &mut ChildSpawnerCommands, (ui, assets): (&UiImages, &AssetServer), at: Vec2) {
    let [left, middle, right] = FIELD;
    parent.spawn((
        bar::piece(
            ui,
            assets,
            left,
            at - Vec2::new(ui.size(left).x, 0.0),
            Vec2::ZERO,
        ),
        Pickable::IGNORE,
    ));
    parent.spawn((
        stretched((ui, assets), middle, at, FIELD_SIZE),
        Pickable::IGNORE,
    ));
    parent.spawn((
        stretched(
            (ui, assets),
            right,
            at + Vec2::new(FIELD_SIZE.x, 0.0),
            Vec2::new(ui.size(middle).x, FIELD_SIZE.y),
        ),
        Pickable::IGNORE,
    ));
}

/// A save's date line (`0x71E600`): `UI_DATE_DATE`, the date, ` - `, `UI_DATE_TIME` and the time
/// of the computer's clock as it saved, in its time zone, the date in the user's short form
/// (`GetDateFormatA` of `LOCALE_USER_DEFAULT` and `DATE_SHORTDATE`), the time `hh:mm`.
fn date_line(texts: Option<&Texts>, saved: u64, pattern: &str) -> String {
    let date = word(texts, ("UI_DATE_DATE", "Date"));
    let time = word(texts, ("UI_DATE_TIME", "Time"));
    let when = i64::try_from(saved)
        .ok()
        .and_then(|seconds| jiff::Timestamp::from_second(seconds).ok())
        .map(|stamp| stamp.to_zoned(jiff::tz::TimeZone::system()));
    match when {
        Some(when) => format!(
            "{date} {} - {time} {:02}:{:02}",
            short_date(pattern, i32::from(when.year()), when.month(), when.day()),
            when.hour(),
            when.minute()
        ),
        None => format!("{date} - {time}"),
    }
}

/// The user's short date's pattern (`sShortDate` of `HKCU\Control Panel\International`, which
/// `GetDateFormatA` takes for `DATE_SHORTDATE`), the US one where there is none.
fn short_date_pattern() -> String {
    #[cfg(windows)]
    {
        if let Some(pattern) = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey(r"Control Panel\International")
            .ok()
            .and_then(|key| key.get_value::<String, _>("sShortDate").ok())
            .filter(|pattern| !pattern.trim().is_empty())
        {
            return pattern;
        }
    }
    "M/d/yyyy".to_owned()
}

/// The date of `year`, `month` and `day` in the Windows pattern `pattern`: `d` and `dd` the day,
/// `M` and `MM` the month, `yy` and `yyyy` the year, text between single quotes as it is, every
/// other character as it is.
fn short_date(pattern: &str, year: i32, month: i8, day: i8) -> String {
    let mut out = String::new();
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\'' {
            for c in chars.by_ref() {
                if c == '\'' {
                    break;
                }
                out.push(c);
            }
            continue;
        }
        let mut run = 1;
        while chars.peek() == Some(&c) {
            chars.next();
            run += 1;
        }
        match (c, run) {
            ('d', 1) => out.push_str(&day.to_string()),
            ('d', 2) => out.push_str(&format!("{day:02}")),
            ('M', 1) => out.push_str(&month.to_string()),
            ('M', 2) => out.push_str(&format!("{month:02}")),
            ('y', 1 | 2) => out.push_str(&format!("{:02}", year.rem_euclid(100))),
            ('y', _) => out.push_str(&year.to_string()),
            (c, run) => out.extend(std::iter::repeat_n(c, run)),
        }
    }
    out
}

/// The list one row up or down by the wheel over the window, a row an event whatever its delta.
fn wheel(_: On<PointerScroll>, scroll: Res<AccumulatedMouseScroll>, mut listing: ResMut<Listing>) {
    if scroll.delta.y != 0.0 {
        listing.scroll(scroll.delta.y < 0.0);
    }
}

/// A button pressed, or its key: Load asks first, Delete asks first, Back goes back to the main
/// menu (`0x71CC90`).
fn pressed(
    In(button): In<Button>,
    mut commands: Commands,
    listing: Res<Listing>,
    (choices, assets, texts): (Res<choice::Images>, Res<AssetServer>, Option<Res<Texts>>),
    questions: Query<(), With<Question>>,
    mut screen: ResMut<NextState<Screen>>,
) {
    if !listing.enabled(button) || !questions.is_empty() {
        return;
    }
    let (line, yes, no) = match button {
        Button::Load => (
            ("UI_BUSY_LOAD_SURE", "Load the game?"),
            Answer::Load(true),
            Answer::Load(false),
        ),
        Button::Erase => (
            ("UI_BUSY_SAVE_ERASE", "Delete the game?"),
            Answer::Erase(true),
            Answer::Erase(false),
        ),
        Button::Save => return,
        Button::Back => {
            screen.set(Screen::Main);
            return;
        }
    };
    let buttons = busy_dialog::spawn(
        &mut commands,
        (&choices, &assets, texts.as_deref()),
        ((Question, DespawnOnExit(Screen::Load)), line),
        &[
            (yes, busy_dialog::OK_BESIDE, ("UI_NET_OK", "OK")),
            (no, busy_dialog::CANCEL, ("UI_NET_CANCEL", "Cancel")),
        ],
    );
    for (entity, answer) in buttons {
        commands
            .entity(entity)
            .observe(move |click: On<PointerClick>, mut commands: Commands| {
                if click.button == PointerButton::Primary {
                    commands.run_system_cached_with(answered, answer);
                }
            });
    }
}

/// A question answered: OK loads the selected save or erases it, its list read anew; either
/// answer closes the question.
fn answered(
    In(answer): In<Answer>,
    mut commands: Commands,
    questions: Query<Entity, With<Question>>,
) {
    for question in &questions {
        commands.entity(question).despawn();
    }
    match answer {
        Answer::Load(true) => commands.queue(|world: &mut World| {
            let Some(stem) = selected_stem(world) else {
                return;
            };
            if let Err(e) = saves::load(world, &stem) {
                warn!("saves window: {e:#}");
            }
        }),
        Answer::Erase(true) => commands.queue(|world: &mut World| {
            let (Some(stem), Some(dir)) = (selected_stem(world), saves::saves_directory(world))
            else {
                return;
            };
            if let Err(e) = saves::erase(&dir, &stem) {
                warn!("saves window: {e:#}");
            }
            let (quick, list) = saves::slots(&dir);
            let mut listing = world.resource_mut::<Listing>();
            listing.quick = quick;
            listing.saves = list;
            listing.place = listing.place.min(listing.saves.len().saturating_sub(1));
        }),
        Answer::Load(false) | Answer::Erase(false) => {}
    }
}

/// The selected group's save's file.
fn selected_stem(world: &World) -> Option<String> {
    let listing = world.get_resource::<Listing>()?;
    listing
        .save_of(listing.selected)
        .map(|save| save.stem.clone())
}

/// Enter does the selected row's Load (`0x71D890`'s action 11, for loading).
fn keys(mut commands: Commands, keys: crate::controls::Keys, listing: Option<Res<Listing>>) {
    if listing.is_some_and(|listing| listing.enabled(Button::Load))
        && keys.just_pressed(crate::controls::Action::Confirm)
    {
        commands.run_system_cached_with(pressed, Button::Load);
    }
}

/// Keeps the list's place as the window closes.
fn close(mut commands: Commands, listing: Option<Res<Listing>>, mut kept: ResMut<Kept>) {
    if let Some(listing) = listing {
        kept.menu = listing.place;
    }
    commands.remove_resource::<Listing>();
}

/// The saves' window.
pub struct SavesWindowPlugin;

impl Plugin for SavesWindowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Kept>()
            .add_systems(OnEnter(Screen::Load), open)
            .add_systems(OnExit(Screen::Load), close)
            .add_systems(
                Update,
                (keys, redraw).chain().run_if(in_state(Screen::Load)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(stem: &str) -> Slot {
        Slot {
            stem: stem.to_owned(),
            header: saves::Header {
                version: saves::VERSION,
                name: stem.to_owned(),
                class: crate::game::Class::Dwarf,
                hero: "Dwarf".to_owned(),
                difficulty: opensacred_rules::drops::Difficulty::Bronze,
                level: 1,
                clock: 0.0,
                saved: 0,
            },
            picture: None,
        }
    }

    fn listing(saves: usize) -> Listing {
        Listing {
            mode: Mode::Load,
            quick: None,
            saves: (1..=saves).map(|n| slot(&format!("GAME{n:02}"))).collect(),
            place: 0,
            selected: 1,
            lit: None,
            pattern: "dd/MM/yyyy".to_owned(),
        }
    }

    #[test]
    fn the_menus_list_ends_on_its_last_save_and_its_bar_runs_past_it() {
        let mut listing = listing(10);
        for _ in 0..12 {
            listing.scroll(true);
        }
        // The bar at the list's end, the view at its last three.
        assert_eq!(listing.place, 9);
        assert_eq!(listing.first(), 7);
        assert_eq!(listing.save_of(3).map(|s| s.stem.as_str()), Some("GAME10"));
        // Two rows up change nothing, the third shows GAME07 first (seen).
        listing.scroll(false);
        listing.scroll(false);
        assert_eq!(listing.first(), 7);
        listing.scroll(false);
        assert_eq!(listing.first(), 6);
    }

    #[test]
    fn a_date_takes_the_users_short_pattern() {
        assert_eq!(short_date("dd/MM/yyyy", 2026, 11, 4), "04/11/2026");
        assert_eq!(short_date("M/d/yyyy", 2026, 11, 4), "11/4/2026");
        assert_eq!(short_date("yyyy-MM-dd", 2026, 3, 9), "2026-03-09");
        assert_eq!(short_date("dd.MM.yy", 2026, 11, 4), "04.11.26");
        assert_eq!(short_date("'Day' d", 2026, 11, 4), "Day 4");
    }

    #[test]
    fn load_and_delete_need_a_save_and_save_is_greyed_for_loading() {
        let mut listing = listing(1);
        assert!(listing.enabled(Button::Load) && listing.enabled(Button::Erase));
        assert!(!listing.enabled(Button::Save) && listing.enabled(Button::Back));
        listing.selected = 2;
        assert!(!listing.enabled(Button::Load) && !listing.enabled(Button::Erase));
        listing.selected = 0;
        assert!(!listing.enabled(Button::Load));
        listing.quick = Some(slot("GAME00"));
        assert!(listing.enabled(Button::Load));
    }
}
