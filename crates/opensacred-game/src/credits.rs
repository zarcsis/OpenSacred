//! The credits of the main menu's Extras (#2929; docs/re/menus.md, "The credits"): the main
//! menu's state 3 (`cUI_MainMenu +0x154`), its picture and water with no items, the lines of
//! `credits2.txt` (`0x715F10`) climbing over them from the screen's foot at 50 pixels a second in
//! the font 3, centred, 25 pixels apart, a heading white and a name `0xFFFFFF80`, each line's
//! text the game's text its key names, empty where none (`0x7128D0`); once the lines and four
//! more have passed the screen's top the Extras come back, as a click on the Cancel button at the
//! bottom right (`+0x18C`, images 280 to 282) or Esc brings them at once (`0x7125B0`).

use bevy::picking::events::{PointerClick, PointerOut, PointerOver};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;

use crate::bar::{self, UiImages};
use crate::game::FONT;
use crate::lines::Texts;
use crate::menu::{SCREEN, Screen};
use crate::saves_window::{lettering, stretched};

/// The credits' file among the converted assets: `Sacred.exe` reads `credits2.txt` alone
/// (docs/02 §11).
const FILE: &str = "ui/texts/credits2.txt";
/// Where the lines' top starts, the screen's foot, and how fast it climbs, in pixels a second
/// (`+0x17C` set to 768.0 as the screen shows, `0x711DA0`; less the frame's seconds times 50,
/// `0x7128D0`).
const START: f32 = 768.0;
const PACE: f32 = 50.0;
/// A line's height, the font 3's of `A`, and the lines past the last whose height the top
/// climbs above the screen's before the Extras come back (`0x7128D0`).
const LINE: f32 = 25.0;
const PAST: f32 = 4.0;
/// The font 3, AntiquaSSK 16, at the size the overview writes it, and how far down its line its
/// letters stand (`overview`).
const SIZE: f32 = 24.0;
const DROP: f32 = 2.0;
/// The colours of a heading (`TYPE`) and of a name (`SUB`) (`0x715F10`: -1 and `0xFFFFFF80`).
const HEADING: u32 = 0xFFFF_FFFF;
const NAME: u32 = 0xFFFF_FF80;
/// The Cancel button: its middle's corner and size (image 281's width twice, its height), its
/// pieces, its word's size and how far up it is set, as the options window's buttons
/// (`0x715DFA`), and its word's colours, white and `0xFFFFFF80` under the cursor.
const CANCEL_AT: Vec2 = Vec2::new(844.0, 704.0);
const CANCEL_SIZE: Vec2 = Vec2::new(76.0, 37.0);
const BUTTON: [u32; 3] = [280, 281, 282];
const WORDS: f32 = 20.0;
const RISE: f32 = 3.0;
const LIT: u32 = 0xFFFF_FF80;

/// A line of the credits' file: a heading's or a name's text by its key, or an empty line.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Line {
    Heading(String),
    Name(String),
    Empty,
}

/// The lines of a credits' file (`0x715F10`): its words, each `//` taking the rest of its line
/// away, `TYPE = KEY` a heading, `SUB = KEY` a name, `NEWLINE` an empty line, any other word
/// passed over.
fn parse(text: &str) -> Vec<Line> {
    let mut words = text.lines().flat_map(|line| {
        let line = line.split("//").next().unwrap_or_default();
        line.replace('=', " = ")
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    });
    let mut lines = Vec::new();
    while let Some(word) = words.next() {
        let line: fn(String) -> Line = match word.as_str() {
            "TYPE" => Line::Heading,
            "SUB" => Line::Name,
            "NEWLINE" => {
                lines.push(Line::Empty);
                continue;
            }
            _ => continue,
        };
        // The word after the keyword, its `=`, passed over; the next its key.
        let _ = words.next();
        if let Some(key) = words.next() {
            lines.push(line(key));
        }
    }
    lines
}

/// The lines' top after `seconds` more from `top`.
fn climbed(top: f32, seconds: f32) -> f32 {
    top - seconds * PACE
}

/// Whether the lines' top `top` of `count` lines has climbed past their end: their height and
/// four lines' more above the screen's top.
fn ended(top: f32, count: usize) -> bool {
    let count = u16::try_from(count).map_or(f32::from(u16::MAX), f32::from);
    top < -(LINE * (count + PAST))
}

/// The credits shown: their lines' count and their top.
#[derive(Resource, Debug, Default)]
struct Shown {
    count: usize,
    top: f32,
}

/// The column of the lines, which climbs.
#[derive(Component)]
struct Column;

/// The Cancel button's word, lit under the cursor.
#[derive(Component)]
struct Cancel;

/// The credits' lines over the main menu's picture, and the Cancel button.
fn show(
    mut commands: Commands,
    (assets, ui, texts): (Res<AssetServer>, Option<Res<UiImages>>, Option<Res<Texts>>),
) {
    let lines = crate::files::read_to_string(FILE)
        .map(|text| parse(&text))
        .unwrap_or_else(|e| {
            warn!("credits: reading {FILE}: {e}");
            Vec::new()
        });
    commands.insert_resource(Shown {
        count: lines.len(),
        top: START,
    });
    let font: Handle<Font> = assets.load(FONT);
    let text = |key: &str| {
        texts
            .as_deref()
            .and_then(|texts| texts.named(key))
            .unwrap_or_default()
            .to_owned()
    };
    let words = text("UI_CANCEL");
    commands
        .spawn((
            DespawnOnExit(Screen::Credits),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            GlobalZIndex(1),
            Pickable::IGNORE,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(SCREEN.x),
                    height: Val::Px(SCREEN.y),
                    flex_shrink: 0.0,
                    overflow: Overflow::clip(),
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|picture| {
                picture
                    .spawn((
                        Column,
                        Node {
                            flex_direction: FlexDirection::Column,
                            ..bar::placed(Vec2::new(0.0, START), Vec2::new(SCREEN.x, 0.0))
                        },
                        Pickable::IGNORE,
                    ))
                    .with_children(|column| {
                        for line in &lines {
                            let (words, colour) = match line {
                                Line::Heading(key) => (text(key), HEADING),
                                Line::Name(key) => (text(key), NAME),
                                Line::Empty => (String::new(), HEADING),
                            };
                            column.spawn((
                                Node {
                                    width: Val::Px(SCREEN.x),
                                    height: Val::Px(LINE),
                                    flex_shrink: 0.0,
                                    justify_content: JustifyContent::Center,
                                    padding: UiRect::top(Val::Px(DROP)),
                                    ..default()
                                },
                                Pickable::IGNORE,
                                children![(
                                    Text::new(words),
                                    lettering(&font, SIZE),
                                    TextLayout::no_wrap(),
                                    TextColor(crate::tooltip::argb(colour)),
                                    Pickable::IGNORE,
                                )],
                            ));
                        }
                    });
                if let Some(ui) = ui.as_deref() {
                    cancel(picture, (ui, &assets, &font), words);
                }
            });
        });
}

/// The Cancel button: its pieces, the middle stretched over its size, and its word, which a click
/// presses.
fn cancel(
    picture: &mut ChildSpawnerCommands,
    (ui, assets, font): (&UiImages, &AssetServer, &Handle<Font>),
    words: String,
) {
    let [left, middle, right] = BUTTON;
    picture.spawn((
        bar::piece(
            ui,
            assets,
            left,
            CANCEL_AT - Vec2::new(ui.size(left).x, 0.0),
            Vec2::ZERO,
        ),
        Pickable::IGNORE,
    ));
    picture.spawn((
        stretched((ui, assets), middle, CANCEL_AT, CANCEL_SIZE),
        Pickable::IGNORE,
    ));
    picture.spawn((
        bar::piece(
            ui,
            assets,
            right,
            CANCEL_AT + Vec2::new(CANCEL_SIZE.x, 0.0),
            Vec2::ZERO,
        ),
        Pickable::IGNORE,
    ));
    picture
        .spawn((
            Node {
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..crate::choice::place(Rect::from_corners(
                    CANCEL_AT - Vec2::new(0.0, RISE),
                    CANCEL_AT + CANCEL_SIZE - Vec2::new(0.0, RISE),
                ))
            },
            Pickable::default(),
            children![(
                Cancel,
                Text::new(words),
                lettering(font, WORDS),
                TextColor(Color::WHITE),
                TextLayout::no_wrap(),
                Pickable::IGNORE,
            )],
        ))
        .observe(
            |_: On<PointerOver>, mut words: Query<&mut TextColor, With<Cancel>>| {
                for mut colour in &mut words {
                    colour.0 = crate::tooltip::argb(LIT);
                }
            },
        )
        .observe(
            |_: On<PointerOut>, mut words: Query<&mut TextColor, With<Cancel>>| {
                for mut colour in &mut words {
                    colour.0 = Color::WHITE;
                }
            },
        )
        .observe(
            |click: On<PointerClick>,
             mut screen: ResMut<NextState<Screen>>,
             mut sounds: MessageWriter<crate::sfx::Play>| {
                if click.button == PointerButton::Primary {
                    crate::inventory::play(&mut sounds, crate::inventory::CLICK);
                    screen.set(Screen::Extras);
                }
            },
        );
}

/// The lines climb by the frame's time; past their end the Extras come back.
fn climb(
    time: Res<Time>,
    mut shown: ResMut<Shown>,
    mut columns: Query<&mut Node, With<Column>>,
    mut screen: ResMut<NextState<Screen>>,
) {
    shown.top = climbed(shown.top, time.delta_secs());
    for mut node in &mut columns {
        node.top = Val::Px(shown.top);
    }
    if ended(shown.top, shown.count) {
        screen.set(Screen::Extras);
    }
}

/// The credits of the Extras.
pub struct CreditsPlugin;

impl Plugin for CreditsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Shown>()
            .add_systems(OnEnter(Screen::Credits), show)
            .add_systems(Update, climb.run_if(in_state(Screen::Credits)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_gives_headings_names_and_empty_lines() {
        let text = "// a comment\r\nNEWLINE\r\nTYPE=HEAD_A\r\n\tSUB = NAME_B // a note\r\n\
                    SUB=NAME_C//a note\r\nOTHER\r\nNEWLINE\r\n";
        assert_eq!(
            parse(text),
            [
                Line::Empty,
                Line::Heading("HEAD_A".to_owned()),
                Line::Name("NAME_B".to_owned()),
                Line::Name("NAME_C".to_owned()),
                Line::Empty,
            ]
        );
    }

    #[test]
    fn the_lines_climb_until_four_more_have_passed() {
        // 50 pixels a second from the foot.
        assert_eq!(climbed(START, 2.0), 668.0);
        // Three lines end once their top is above -(25 * 7).
        assert!(!ended(-175.0, 3));
        assert!(ended(-175.5, 3));
        // No line ends after four lines' height.
        assert!(ended(-100.5, 0));
        assert!(!ended(-100.0, 0));
    }
}
