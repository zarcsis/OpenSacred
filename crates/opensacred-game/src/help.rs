//! The keys' help (docs/10 §9): F1 shows and hides, over the middle of the screen, what each key
//! does everywhere, in the game, in the menus and in the console. Esc hides it too, without
//! pausing a game, after the console has had its Esc; the game goes on under it. The command
//! `keys on|off` shows and hides it from a script or from afar.

use bevy::input::InputSystems;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::text::FontSize;

/// The keys and what they do, by where they work.
pub const KEYS: &[(&str, &[(&str, &str)])] = &[
    (
        "Everywhere",
        &[
            ("F1", "this help"),
            ("F3", "the debug overlay"),
            ("F4", "the log"),
            ("F11", "borderless full screen or a window"),
            ("~", "the console (the key under Esc)"),
        ],
    ),
    (
        "In the game",
        &[
            ("Left button", "walk or run to the cursor"),
            ("Left button on a foe", "strike it; hold to strike on"),
            ("Ctrl+left button", "strike at once"),
            ("Right button on a foe", "make the selected combat art"),
            ("R", "walk or run"),
            ("I", "the inventory"),
            ("Wheel", "zoom in and out"),
            ("Esc", "pause: Continue, Main menu, Quit"),
        ],
    ),
    (
        "In the menus",
        &[
            ("Mouse, arrows", "choose"),
            ("Click, Enter", "do it"),
            ("Esc", "back"),
        ],
    ),
    (
        "In the console",
        &[
            ("Enter", "run the line (help lists the commands)"),
            ("Up, Down", "the history"),
            ("Tab", "complete"),
            ("Ctrl+V, Ctrl+C", "paste, copy the line"),
            ("PageUp, PageDown", "scroll"),
            ("Esc", "close"),
        ],
    ),
];

/// The text's size on the original's 768-pixel-high screen (`hud`).
const TITLE: f32 = 16.0;
const TEXT: f32 = 11.0;
/// How wide the keys' column is.
const KEY_WIDTH: f32 = 140.0;
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

fn spawn(mut commands: Commands) {
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
                                        text(TEXT, WORDS, key),
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

/// F1 shows and hides the help; Esc hides it, taken from the game so that it does not pause it.
fn keys(mut keys: ResMut<ButtonInput<KeyCode>>, mut help: ResMut<Help>) {
    if keys.just_pressed(KeyCode::F1) {
        help.shown = !help.shown;
        keys.clear_just_pressed(KeyCode::F1);
    } else if help.shown && keys.just_pressed(KeyCode::Escape) {
        help.shown = false;
        keys.clear_just_pressed(KeyCode::Escape);
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

/// The keys' help on F1.
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
    fn the_help_names_every_key_of_the_game() {
        let keys: Vec<&str> = KEYS
            .iter()
            .flat_map(|(_, keys)| keys.iter().map(|(key, _)| *key))
            .collect();
        for key in [
            "F1",
            "F3",
            "F4",
            "F11",
            "~",
            "R",
            "Esc",
            "Tab",
            "Ctrl+V, Ctrl+C",
            "Ctrl+left button",
        ] {
            assert!(keys.contains(&key), "{key}");
        }
    }

    #[test]
    fn f1_shows_the_help_and_esc_hides_it_without_reaching_the_game() {
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
        press(&mut app, KeyCode::F1, Key::F1);
        assert!(shown(&mut app));
        press(&mut app, KeyCode::Escape, Key::Escape);
        assert!(!shown(&mut app));
        assert_eq!(app.world().resource::<Escaped>().0, 0);
        press(&mut app, KeyCode::Escape, Key::Escape);
        assert_eq!(app.world().resource::<Escaped>().0, 1);
        press(&mut app, KeyCode::F1, Key::F1);
        press(&mut app, KeyCode::F1, Key::F1);
        assert!(!shown(&mut app));
    }
}
