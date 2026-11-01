//! The interface's busy dialog (`DEFAULT_BUSYDLG`, `cUI_BusyDlg`; docs/re/menus.md, "The busy
//! dialog"): the screen darkened, its framed box, a line in white in its middle and its buttons
//! under it, laid out on the original's screen centred along the window's width as the bar is.
//! The menu over a game's Quit asks in it (`esc_menu`), and since #1687 the key P pauses the game
//! under it (`0x618090`): the line `UI_BUSY_PAUSE` over an OK alone, the OK or P again going on,
//! Esc doing nothing (seen).

use bevy::picking::events::PointerClick;
use bevy::prelude::*;

use crate::bar;
use crate::choice::{self, Atlases};
use crate::controls::{Action, Keys};
use crate::game::{AppState, FONT};
use crate::lines::Texts;
use crate::pause::{Busy, Play};

/// The dialog's dark box and its framed box, its line's row and the size of its line's and its
/// buttons' letters.
const BOX: Rect = Rect::from_corners(Vec2::new(288.0, 288.0), Vec2::new(736.0, 480.0));
const FRAMED: Rect = Rect::from_corners(Vec2::new(320.0, 320.0), Vec2::new(704.0, 448.0));
const LINE_ROW: f32 = 367.0;
const LINE_SIZE: f32 = 19.0;
const BUTTON_SIZE: f32 = 18.0;
/// The buttons' middles, their ends outside them: OK and Cancel side by side (**estimate**, seen
/// 2026-10-29: 157 wide from the ends' outer edges, about the row 440), and an OK alone centred
/// along the box (`0x7208F0`'s mode 1; seen 2026-11-01 under the pause's line).
pub(crate) const OK_BESIDE: Rect =
    Rect::from_corners(Vec2::new(389.0, 422.0), Vec2::new(455.0, 459.0));
pub(crate) const CANCEL: Rect =
    Rect::from_corners(Vec2::new(570.0, 422.0), Vec2::new(636.0, 459.0));
const OK_ALONE: Rect = Rect::from_corners(Vec2::new(479.0, 422.0), Vec2::new(545.0, 459.0));
/// The darkening of the screen behind the dialog (**estimate**, seen: the world dimmed by about
/// half).
const DARKENED: Color = Color::linear_rgba(0.0, 0.0, 0.0, 0.5);
/// The pause's line (`UI_BUSY_PAUSE`) and the OK's text (1024), with the words where the texts
/// lack them.
const PAUSE: (&str, &str) = ("UI_BUSY_PAUSE", "Pause");
pub(crate) const OK: (&str, &str) = ("1024", "OK");

/// The words of the text named `name`, else `word`.
pub(crate) fn word(texts: Option<&Texts>, (name, word): (&str, &str)) -> String {
    texts
        .and_then(|texts| texts.named(name))
        .unwrap_or(word)
        .to_owned()
}

/// A node of the original's screen, centred along the window's width as the bar is.
pub(crate) fn screen() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(50.0),
        top: Val::Px(0.0),
        margin: UiRect::left(Val::Px(-bar::SCREEN.x / 2.0)),
        width: Val::Px(bar::SCREEN.x),
        height: Val::Px(bar::SCREEN.y),
        ..default()
    }
}

/// Makes the dialog, `marker` on its root, of the line `line` and the buttons `buttons`, each
/// carrying its answer at its rectangle with its text, while the game is paused: the buttons and
/// their answers, to observe.
pub(crate) fn spawn<B: Component + Copy>(
    commands: &mut Commands,
    (images, assets, texts): (&choice::Images, &AssetServer, Option<&Texts>),
    (marker, line): (impl Bundle, (&str, &str)),
    buttons: &[(B, Rect, (&str, &str))],
) -> Vec<(Entity, B)> {
    let atlases = Atlases::load(images, assets);
    let font: Handle<Font> = assets.load(FONT);
    let lettering = |size: f32| TextFont {
        font: font.clone().into(),
        font_size: FontSize::Px(size),
        ..default()
    };
    let mut spawned = Vec::new();
    commands
        .spawn((
            marker,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(DARKENED),
            GlobalZIndex(21),
            Pickable::default(),
            DespawnOnExit(Play::Paused),
        ))
        .with_children(|dark| {
            dark.spawn((screen(), Pickable::IGNORE))
                .with_children(|picture| {
                    picture.spawn((
                        choice::place(BOX),
                        BackgroundColor(choice::SHADE),
                        Pickable::IGNORE,
                    ));
                    choice::frame_about(picture, &atlases, FRAMED);
                    picture.spawn((
                        Node {
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..choice::place(Rect::new(
                                FRAMED.min.x,
                                LINE_ROW - LINE_SIZE,
                                FRAMED.max.x,
                                LINE_ROW + LINE_SIZE,
                            ))
                        },
                        Pickable::IGNORE,
                        children![(
                            Text::new(word(texts, line)),
                            lettering(LINE_SIZE),
                            TextColor(Color::WHITE),
                            Pickable::IGNORE,
                        )],
                    ));
                    for &(answer, rect, text) in buttons {
                        let whole = choice::button_pieces(picture, &atlases, rect);
                        let button = picture
                            .spawn((
                                answer,
                                Node {
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..choice::place(whole)
                                },
                                Pickable::default(),
                                children![(
                                    Text::new(word(texts, text)),
                                    lettering(BUTTON_SIZE),
                                    TextColor(Color::WHITE),
                                    Pickable::IGNORE,
                                )],
                            ))
                            .id();
                        spawned.push((button, answer));
                    }
                });
        });
    spawned
}

/// The pause's window, and its OK.
#[derive(Component)]
struct PauseWindow;
#[derive(Component, Clone, Copy)]
struct GoOn;

/// P pauses a running game under the pause's window, and again goes on from it (`0x618090`,
/// `0x721380`); it leaves the menu over a game alone.
fn pause_key(
    keys: Keys,
    play: Res<State<Play>>,
    mut next: ResMut<NextState<Play>>,
    mut busy: ResMut<Busy>,
) {
    if !keys.just_pressed(Action::Pause) {
        return;
    }
    match play.get() {
        Play::Running => {
            busy.0 = true;
            next.set(Play::Paused);
        }
        Play::Paused if busy.0 => next.set(Play::Running),
        Play::Paused => {}
    }
}

/// Shows the pause's window as P pauses the game (the event `0x39` to `DEFAULT_BUSYDLG` of the
/// mode 1, its OK's reaction 2 going on, `0x720A80`).
fn show(
    mut commands: Commands,
    busy: Res<Busy>,
    (images, assets, texts): (Res<choice::Images>, Res<AssetServer>, Option<Res<Texts>>),
) {
    if !busy.0 {
        return;
    }
    let buttons = spawn(
        &mut commands,
        (&images, &assets, texts.as_deref()),
        (PauseWindow, PAUSE),
        &[(GoOn, OK_ALONE, OK)],
    );
    for (button, _) in buttons {
        commands.entity(button).observe(
            |click: On<PointerClick>, mut next: ResMut<NextState<Play>>| {
                if click.button == PointerButton::Primary {
                    next.set(Play::Running);
                }
            },
        );
    }
}

/// The busy dialog's pause.
pub struct BusyDialogPlugin;

impl Plugin for BusyDialogPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Play::Paused), show).add_systems(
            Update,
            pause_key
                .before(crate::pause::toggle)
                .run_if(in_state(AppState::InGame))
                .in_set(crate::frame::Control),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release(key);
        keys.clear();
        app.update();
    }

    #[test]
    fn p_pauses_under_the_pause_window_esc_leaves_it_and_p_goes_on() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::state::app::StatesPlugin,
            AssetPlugin::default(),
        ))
        .init_asset::<Font>()
        .init_asset::<Image>()
        .insert_state(AppState::InGame)
        .add_plugins((
            crate::pause::PausePlugin,
            crate::esc_menu::EscMenuPlugin,
            BusyDialogPlugin,
        ))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<choice::Images>()
        .init_resource::<bar::UiImages>()
        .init_resource::<crate::hud::Pointer>()
        .init_resource::<UiScale>();
        app.update();
        let play = |app: &App| *app.world().resource::<State<Play>>().get();
        let windows = |app: &mut App| {
            app.world_mut()
                .query_filtered::<(), With<PauseWindow>>()
                .iter(app.world())
                .count()
        };
        let menus = |app: &mut App| {
            app.world_mut()
                .query::<&crate::esc_menu::Entry>()
                .iter(app.world())
                .count()
        };
        press(&mut app, KeyCode::KeyP);
        assert_eq!(play(&app), Play::Paused);
        assert_eq!((windows(&mut app), menus(&mut app)), (1, 0));
        // Esc neither goes on nor opens the menu.
        press(&mut app, KeyCode::Escape);
        assert_eq!(play(&app), Play::Paused);
        assert_eq!((windows(&mut app), menus(&mut app)), (1, 0));
        press(&mut app, KeyCode::KeyP);
        assert_eq!(play(&app), Play::Running);
        assert_eq!(windows(&mut app), 0);
        // Under the menu over a game P does nothing, and Esc pauses with the menu alone.
        press(&mut app, KeyCode::Escape);
        press(&mut app, KeyCode::KeyP);
        assert_eq!(play(&app), Play::Paused);
        assert_eq!((windows(&mut app), menus(&mut app)), (0, 5));
    }
}
