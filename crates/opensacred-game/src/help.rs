//! The keys' help (docs/10 §9): F10 shows and hides, over the middle of the screen, what each key
//! does everywhere, in the game, in the menus and in the console, the keys as `controls.toml` has
//! them (`controls`). Esc hides it too, without
//! pausing a game, after the console has had its Esc; the game goes on under it. The command
//! `keys on|off` shows and hides it from a script or from afar.

use bevy::input::InputSystems;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::text::FontSize;

use crate::controls::{ART_KEYS, ART_SLOTS, Action, Controls, KeysMut, WEAPON_SETS};

/// A row's keys: the keys of actions, the first of each where there are several, between words
/// before and after them; or words alone, for the mouse and the console's line.
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
        format!("{}{between}{names}{}", self.before, self.after)
    }
}

/// The keys and what they do, by where they work.
pub const KEYS: &[(&str, &[(Row, &str)])] = &[
    (
        "Everywhere",
        &[
            (of(&[Action::Help]), "this help"),
            (of(&[Action::Overlay]), "the debug overlay"),
            (of(&[Action::Log]), "the log"),
            (of(&[Action::Window]), "borderless full screen or a window"),
            (of(&[Action::Console]), "the console (the key under Esc)"),
        ],
    ),
    (
        "In the game",
        &[
            (words("Left button"), "run to the cursor"),
            (of(&[Action::Walk]), "walk rather than run, while held"),
            (
                words("Left button on a foe"),
                "strike it; hold to strike on",
            ),
            (
                Row {
                    before: "",
                    actions: &[Action::StrikeAtOnce],
                    after: "+left button",
                },
                "strike at once",
            ),
            (
                words("Right button on a foe"),
                "make the selected combat art",
            ),
            (of(&[Action::Labels]), "the items' names"),
            (of(&[Action::Inventory]), "the inventory"),
            (of(&[Action::QuestBook]), "the quest book"),
            (of(&[Action::WorldMap]), "the world map"),
            (of(&[Action::Overview]), "held, the overview map"),
            (of(&[Action::CombatArts]), "the inventory's combat arts"),
            (of(&[Action::Combos]), "the inventory's combos"),
            (
                of(&[Action::ZoomIn, Action::ZoomOut]),
                "held, the world closer or farther",
            ),
            (of(&[Action::ZoomBack]), "the zoom back to 1"),
            (of(&WEAPON_SETS), "the weapon sets"),
            (of(&ART_SLOTS), "the art slots"),
            (of(&ART_KEYS), "the art of the key into the selected slot"),
            (
                words("The same over an art"),
                "on the spells page: bind the key to it",
            ),
            (
                of(&[Action::BeltPotion]),
                "a healing potion (with Shift the yellow one)",
            ),
            (
                of(&[
                    Action::PurplePotion,
                    Action::BluePotion,
                    Action::GreenPotion,
                    Action::YellowPotion,
                ]),
                "a purple, blue, green or yellow potion",
            ),
            (
                of(&[Action::FollowerPotion]),
                "a healing potion for a follower",
            ),
            (of(&[Action::Collect]), "collect the items about the hero"),
            (
                Row {
                    before: "",
                    actions: &[Action::SendItem],
                    after: "+click on an item",
                },
                "the stash's to the backpack, a potion to a follower",
            ),
            (words("Wheel"), "zoom in and out"),
            (of(&[Action::Back]), "pause: Continue, Main menu, Quit"),
        ],
    ),
    (
        "In the menus",
        &[
            (
                Row {
                    before: "Mouse",
                    actions: &[Action::Next, Action::Previous],
                    after: "",
                },
                "choose",
            ),
            (
                Row {
                    before: "Click",
                    actions: &[Action::Confirm],
                    after: "",
                },
                "do it",
            ),
            (of(&[Action::Back]), "back"),
        ],
    ),
    (
        "In the console",
        &[
            (words("Enter"), "run the line (help lists the commands)"),
            (words("Up, Down"), "the history"),
            (words("Tab"), "complete"),
            (words("Ctrl+V, Ctrl+C"), "paste, copy the line"),
            (words("PageUp, PageDown"), "scroll"),
            (of(&[Action::Back]), "close"),
        ],
    ),
];

/// The text's size on the original's 768-pixel-high screen (`hud`).
const TITLE: f32 = 16.0;
const TEXT: f32 = 11.0;
/// How wide the keys' column is.
const KEY_WIDTH: f32 = 170.0;
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
                        padding: UiRect::all(Val::Px(14.0)),
                        row_gap: Val::Px(3.0),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(BACK),
                    BorderColor::all(GOLD),
                ))
                .with_children(|panel| {
                    panel.spawn(text(TITLE, GOLD, "Keys"));
                    for (place, keys) in KEYS {
                        panel.spawn((
                            text(TEXT, GOLD, place),
                            Node {
                                margin: UiRect::top(Val::Px(8.0)),
                                ..default()
                            },
                        ));
                        for (key, does) in *keys {
                            panel
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
                                    row.spawn(text(TEXT, WORDS, does));
                                });
                        }
                    }
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

    #[test]
    fn the_help_names_every_action_by_its_keys() {
        let controls = Controls::default();
        let rows: Vec<(String, &str)> = KEYS
            .iter()
            .flat_map(|(_, rows)| rows.iter().map(|(row, does)| (row.text(&controls), *does)))
            .collect();
        // Every action has its row.
        let shown: Vec<Action> = KEYS
            .iter()
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
        ] {
            assert!(rows.contains(&(keys.to_owned(), does)), "{keys}: {rows:#?}");
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
