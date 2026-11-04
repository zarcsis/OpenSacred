//! The keys' help (docs/10 §9): F10 shows and hides, over the middle of the screen, what each key
//! does everywhere, in the game, in the menus and in the console, the keys as `controls.toml` has
//! them (`controls`). Esc hides it too, without
//! pausing a game, after the console has had its Esc; the game goes on under it. The command
//! `keys on|off` shows and hides it from a script or from afar. Its words are the messages of
//! `text/en-US/help.ftl` (`own_texts`, #1986), named in its table.

use bevy::input::InputSystems;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::text::FontSize;

use crate::controls::{ART_KEYS, ART_SLOTS, Action, Controls, KeysMut, WEAPON_SETS};
use crate::own_texts;

/// A row's keys: the keys of actions, the first of each where there are several, between words
/// before and after them; or words alone, for the mouse and the console's line. The words are
/// messages' names, none where empty.
pub struct Row {
    before: &'static str,
    actions: &'static [Action],
    after: &'static str,
}

const fn of(actions: &'static [Action]) -> Row {
    Row {
        before: "",
        actions,
        after: "",
    }
}

const fn words(before: &'static str) -> Row {
    Row {
        before,
        actions: &[],
        after: "",
    }
}

impl Row {
    /// Its keys' text, by the controls in force.
    fn text(&self, controls: &Controls) -> String {
        let names = match self.actions {
            [action] => controls.names(*action),
            actions => actions
                .iter()
                .filter_map(|&action| controls.bindings(action).first().map(|b| b.name()))
                .collect::<Vec<_>>()
                .join(", "),
        };
        let between = if self.before.is_empty() || names.is_empty() {
            ""
        } else {
            ", "
        };
        format!(
            "{}{between}{names}{}",
            own_texts::text(self.before),
            own_texts::text(self.after)
        )
    }
}

/// A place's name and its rows: each row's keys and what they do, the words messages' names.
type Place = (&'static str, &'static [(Row, &'static str)]);

/// The keys and what they do, by where they work, the places in two columns side by side, so
/// that the help keeps within the screen's height (#1952): everywhere and in the game, then in the
/// menus and in the console.
pub const KEYS: [&[Place]; 2] = [
    &[
        (
            "help-everywhere",
            &[
                (of(&[Action::Help]), "help-help"),
                (of(&[Action::Overlay]), "help-overlay"),
                (of(&[Action::Log]), "help-log"),
                (of(&[Action::Window]), "help-window"),
                (of(&[Action::Console]), "help-console"),
                (of(&[Action::SkipMovie]), "help-skip-movie"),
            ],
        ),
        (
            "help-in-the-game",
            &[
                (words("help-left-button"), "help-run"),
                (of(&[Action::Walk]), "help-walk"),
                (words("help-left-button-on-a-foe"), "help-strike"),
                (
                    Row {
                        before: "",
                        actions: &[Action::StrikeAtOnce],
                        after: "help-and-left-button",
                    },
                    "help-strike-at-once",
                ),
                (words("help-right-button-on-a-foe"), "help-make-art"),
                (of(&[Action::Labels]), "help-labels"),
                (of(&[Action::Inventory]), "help-inventory"),
                (of(&[Action::QuestBook]), "help-quest-book"),
                (of(&[Action::WorldMap]), "help-world-map"),
                (of(&[Action::Overview]), "help-overview"),
                (of(&[Action::CombatArts]), "help-combat-arts"),
                (of(&[Action::Combos]), "help-combos"),
                (of(&[Action::ZoomIn, Action::ZoomOut]), "help-zoom-keys"),
                (of(&[Action::ZoomBack]), "help-zoom-back"),
                (of(&WEAPON_SETS), "help-weapon-sets"),
                (of(&ART_SLOTS), "help-art-slots"),
                (of(&ART_KEYS), "help-art-keys"),
                (words("help-the-same-over-an-art"), "help-bind"),
                (of(&[Action::BeltPotion]), "help-belt-potion"),
                (
                    of(&[
                        Action::PurplePotion,
                        Action::BluePotion,
                        Action::GreenPotion,
                        Action::YellowPotion,
                    ]),
                    "help-potions",
                ),
                (of(&[Action::FollowerPotion]), "help-follower-potion"),
                (of(&[Action::Collect]), "help-collect"),
                (
                    Row {
                        before: "",
                        actions: &[Action::SendItem],
                        after: "help-and-click-on-an-item",
                    },
                    "help-send-item",
                ),
                (words("help-wheel"), "help-zoom-wheel"),
                (of(&[Action::Pause]), "help-pause"),
                (of(&[Action::QuickSave]), "help-quick-save"),
                (of(&[Action::QuickLoad]), "help-quick-load"),
                (of(&[Action::Back]), "help-pause-menu"),
            ],
        ),
    ],
    &[
        (
            "help-in-the-menus",
            &[
                (
                    Row {
                        before: "help-mouse",
                        actions: &[Action::Next, Action::Previous],
                        after: "",
                    },
                    "help-choose",
                ),
                (
                    Row {
                        before: "help-click",
                        actions: &[Action::Confirm],
                        after: "",
                    },
                    "help-do-it",
                ),
                (of(&[Action::Back]), "help-back"),
            ],
        ),
        (
            "help-in-the-console",
            &[
                (words("help-enter"), "help-run-line"),
                (words("help-up-down"), "help-history"),
                (words("help-tab"), "help-complete"),
                (words("help-paste-copy-keys"), "help-paste-copy"),
                (words("help-page-keys"), "help-scroll"),
                (of(&[Action::Back]), "help-close"),
            ],
        ),
    ],
];

/// The text's size on the original's 768-pixel-high screen (`hud`).
const TITLE: f32 = 16.0;
const TEXT: f32 = 11.0;
/// How wide the keys' column is.
const KEY_WIDTH: f32 = 170.0;
/// The panel's padding, the space between its rows, above a place's name and between its two
/// columns.
const PADDING: f32 = 14.0;
const GAP: f32 = 3.0;
const PLACE_MARGIN: f32 = 8.0;
const COLUMN_GAP: f32 = 28.0;
/// The panel's colours, in the frame's sRGB values (`hud`).
const BACK: Color = Color::linear_rgba(0.02, 0.02, 0.03, 0.9);
const GOLD: Color = Color::linear_rgb(0.86, 0.74, 0.45);
const WORDS: Color = Color::linear_rgb(0.85, 0.85, 0.85);

/// Whether the help shows.
#[derive(Resource, Default)]
pub struct Help {
    pub shown: bool,
}

/// The help's panel.
#[derive(Component)]
struct Panel;

fn spawn(mut commands: Commands, controls: Option<Res<Controls>>) {
    let defaults = Controls::default();
    let controls = controls.as_deref().unwrap_or(&defaults);
    let text = |size: f32, colour: Color, words: &str| {
        (
            Text::new(words),
            TextFont {
                font_size: FontSize::Px(size),
                ..default()
            },
            TextColor(colour),
        )
    };
    commands
        .spawn((
            Panel,
            // The whole screen, to centre the panel, letting the mouse through around it.
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Pickable::IGNORE,
            GlobalZIndex(i32::MAX - 1),
            Visibility::Hidden,
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(PADDING)),
                        row_gap: Val::Px(GAP),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(BACK),
                    BorderColor::all(GOLD),
                ))
                .with_children(|panel| {
                    panel.spawn(text(TITLE, GOLD, own_texts::text("help-title")));
                    panel
                        .spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(COLUMN_GAP),
                            ..default()
                        })
                        .with_children(|columns| {
                            for places in KEYS {
                                columns
                                    .spawn(Node {
                                        flex_direction: FlexDirection::Column,
                                        row_gap: Val::Px(GAP),
                                        ..default()
                                    })
                                    .with_children(|column| {
                                        for (place, keys) in places {
                                            column.spawn((
                                                text(TEXT, GOLD, own_texts::text(place)),
                                                Node {
                                                    margin: UiRect::top(Val::Px(PLACE_MARGIN)),
                                                    ..default()
                                                },
                                            ));
                                            for (key, does) in *keys {
                                                column
                                                    .spawn(Node {
                                                        flex_direction: FlexDirection::Row,
                                                        ..default()
                                                    })
                                                    .with_children(|row| {
                                                        row.spawn((
                                                            text(TEXT, WORDS, &key.text(controls)),
                                                            Node {
                                                                width: Val::Px(KEY_WIDTH),
                                                                ..default()
                                                            },
                                                        ));
                                                        row.spawn(text(
                                                            TEXT,
                                                            WORDS,
                                                            own_texts::text(does),
                                                        ));
                                                    });
                                            }
                                        }
                                    });
                            }
                        });
                });
        });
}

/// F10 shows and hides the help; Esc hides it, taken from the game so that it does not pause it.
fn keys(mut keys: KeysMut, mut help: ResMut<Help>) {
    if keys.take(Action::Help) {
        help.shown = !help.shown;
    } else if help.shown && keys.take(Action::Back) {
        help.shown = false;
    }
}

fn show(help: Res<Help>, mut panel: Single<&mut Visibility, With<Panel>>) {
    let visibility = if help.shown {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if **panel != visibility {
        **panel = visibility;
    }
}

/// The keys' help on F10.
pub struct HelpPlugin;

impl Plugin for HelpPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Help>()
            .add_systems(Startup, spawn)
            .add_systems(
                PreUpdate,
                keys.after(InputSystems).after(crate::console::keys),
            )
            .add_systems(Update, show);
    }
}

#[cfg(test)]
mod tests {
    use bevy::input::ButtonState;
    use bevy::input::keyboard::{Key, KeyboardInput};

    use super::*;
    use crate::frame::Control;

    /// Every place, the columns' in turn.
    fn places() -> impl Iterator<Item = &'static Place> {
        KEYS.iter().flat_map(|column| column.iter())
    }

    #[test]
    fn the_help_names_every_action_by_its_keys() {
        let controls = Controls::default();
        let rows: Vec<(String, &str)> = places()
            .flat_map(|(_, rows)| {
                rows.iter()
                    .map(|(row, does)| (row.text(&controls), own_texts::text(does)))
            })
            .collect();
        // Every action has its row.
        let shown: Vec<Action> = places()
            .flat_map(|(_, rows)| rows.iter().flat_map(|(row, _)| row.actions.iter().copied()))
            .collect();
        for action in Action::all() {
            assert!(shown.contains(&action), "{action:?}");
        }
        for (keys, does) in [
            ("F10", "this help"),
            ("Shift", "walk rather than run, while held"),
            ("Control+left button", "strike at once"),
            ("1, 2, 3, 4, 5", "the weapon sets"),
            (
                "F1, F2, F3, F4, F5, F6",
                "the art of the key into the selected slot",
            ),
            ("Q, W, E, R", "a purple, blue, green or yellow potion"),
            ("Mouse, ArrowDown, ArrowUp", "choose"),
            ("Click, Enter, NumpadEnter", "do it"),
            ("Ctrl+V, Ctrl+C", "paste, copy the line"),
            ("Escape, Space", "held, as either button, stop a movie"),
        ] {
            assert!(rows.contains(&(keys.to_owned(), does)), "{keys}: {rows:#?}");
        }
    }

    #[test]
    fn the_table_names_every_message_of_the_helps_file_and_no_other() {
        let mut named = vec!["help-title"];
        for (place, rows) in places() {
            named.push(place);
            for (row, does) in *rows {
                named.extend(
                    [row.before, row.after, does]
                        .into_iter()
                        .filter(|n| !n.is_empty()),
                );
            }
        }
        for name in &named {
            assert!(own_texts::has(name), "{name}");
        }
        for name in own_texts::names().filter(|name| name.starts_with("help-")) {
            assert!(named.contains(&name), "{name} is named by no row");
        }
    }

    #[test]
    fn each_column_keeps_within_the_screens_height() {
        // A line of Bevy's text is 1.2 of its size high, and the panel's gap follows each.
        let line = |size: f32| size * 1.2 + GAP;
        let count = |n: usize| f32::from(u16::try_from(n).unwrap());
        for places in KEYS {
            let rows: usize = places.iter().map(|(_, rows)| rows.len()).sum();
            let height = 2.0 * (PADDING + 1.0)
                + line(TITLE)
                + count(places.len()) * (line(TEXT) + PLACE_MARGIN)
                + count(rows) * line(TEXT);
            // Room for a few rows more within the screen.
            assert!(height + 4.0 * line(TEXT) < crate::bar::SCREEN.y, "{height}");
        }
    }

    #[test]
    fn f10_shows_the_help_and_esc_hides_it_without_reaching_the_game() {
        #[derive(Resource, Default)]
        struct Escaped(u32);
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::input::InputPlugin, HelpPlugin))
            .init_resource::<Escaped>()
            .add_systems(
                Update,
                (|keys: Res<ButtonInput<KeyCode>>, mut escaped: ResMut<Escaped>| {
                    if keys.just_pressed(KeyCode::Escape) {
                        escaped.0 += 1;
                    }
                })
                .in_set(Control),
            );
        app.update();
        let press = |app: &mut App, key: KeyCode, logical: Key| {
            for state in [ButtonState::Pressed, ButtonState::Released] {
                app.world_mut().write_message(KeyboardInput {
                    key_code: key,
                    logical_key: logical.clone(),
                    state,
                    text: None,
                    repeat: false,
                    window: Entity::PLACEHOLDER,
                });
                app.update();
            }
        };
        let shown = |app: &mut App| {
            let mut panels = app.world_mut().query_filtered::<&Visibility, With<Panel>>();
            *panels.single(app.world()).unwrap() != Visibility::Hidden
        };
        assert!(!shown(&mut app));
        press(&mut app, KeyCode::F10, Key::F10);
        assert!(shown(&mut app));
        press(&mut app, KeyCode::Escape, Key::Escape);
        assert!(!shown(&mut app));
        assert_eq!(app.world().resource::<Escaped>().0, 0);
        press(&mut app, KeyCode::Escape, Key::Escape);
        assert_eq!(app.world().resource::<Escaped>().0, 1);
        press(&mut app, KeyCode::F10, Key::F10);
        press(&mut app, KeyCode::F10, Key::F10);
        assert!(!shown(&mut app));
    }
}
