//! The menu over a game (#1410; docs/re/menus.md, "The menu over a game (Esc)"): Esc and the
//! taskbar's Options button pause the game (`pause`) under the original's window
//! `UI_WND_ESCMENU`, laid out on the original's screen centred along the window's width as the bar
//! is: its dark fill, the interface's standard frame (`choice::frame_about`) and its five items in
//! the menus' letter, the one under the cursor lit (`cUI_EscMenu`, `0x6BBBD0`). Options, Save Game
//! and Export do nothing yet and are greyed, as the main menu's items without their screens are
//! (#1096, #1095); Continue goes on, and Quit asks first in the original's dialog
//! (`DEFAULT_BUSYDLG`, `busy_dialog`): the screen darkened, its framed box, its question and the
//! buttons OK, which leaves the game for the main menu, and Cancel, which closes the dialog alone.
//! A still frame's pause (`pause on`) and the key P's (`busy_dialog`) show none of it.

use bevy::picking::events::{PointerClick, PointerOut, PointerOver};
use bevy::prelude::*;

use crate::bar::{self, UiImages};
use crate::busy_dialog::{self, screen, word};
use crate::choice::{self, Atlases};
use crate::game::AppState;
use crate::hud::Pointer;
use crate::lines::Texts;
use crate::menu;
use crate::pause::{Busy, Play, Still};

/// The window on the original's screen, its framed box, the items' rows from the window's top and
/// their height (`0x6BBBD0`), the items' size in the menus' letter, as the main menu's, and their
/// colours, pale and lit white under the cursor, in the frame's sRGB values (`hud`; **estimate**,
/// seen 2026-10-29: the letters (228, 212, 169), the lit (249, 249, 249)).
const WINDOW: Rect = Rect::from_corners(Vec2::new(380.0, 284.0), Vec2::new(644.0, 484.0));
const FRAMED: Rect = Rect::from_corners(Vec2::new(400.0, 304.0), Vec2::new(624.0, 464.0));
const ROWS: [f32; 5] = [8.0, 44.0, 80.0, 116.0, 152.0];
const ROW: f32 = 32.0;
const ITEM_SIZE: f32 = 29.0;
const PALE: Color = Color::linear_rgb(0.89, 0.83, 0.66);
const LIT: Color = Color::linear_rgb(0.98, 0.98, 0.98);

/// The window's items, in its order: their words' text names (the texts 1050, 1051, 1049, 1052
/// and 1053) and the words where the texts lack them.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Options,
    SaveGame,
    Export,
    Quit,
    Continue,
}

impl Entry {
    const ALL: [Self; 5] = [
        Self::Options,
        Self::SaveGame,
        Self::Export,
        Self::Quit,
        Self::Continue,
    ];

    fn text(self) -> (&'static str, &'static str) {
        match self {
            Self::Options => ("1050", "Options"),
            Self::SaveGame => ("1051", "Save Game"),
            Self::Export => ("1049", "Export"),
            Self::Quit => ("1052", "Quit"),
            Self::Continue => ("1053", "Continue"),
        }
    }

    /// Whether it does anything yet: the options wait for #1096, the saves for #1095, and the
    /// export, the multiplayer's character, for M9.
    fn works(self) -> bool {
        matches!(self, Self::Quit | Self::Continue)
    }
}

/// The dialog's buttons.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum Answer {
    Ok,
    Cancel,
}

/// The item under the cursor, which is lit.
#[derive(Resource, Default)]
struct Lit(Option<Entry>);

/// The window's root, and the dialog's.
#[derive(Component)]
struct Window;
#[derive(Component)]
struct Dialog;

/// Shows the window as the game pauses, unless the pause is a still frame's or the key P's.
fn show(
    mut commands: Commands,
    (still, busy): (Option<Res<Still>>, Option<Res<Busy>>),
    (images, assets, texts): (Res<choice::Images>, Res<AssetServer>, Option<Res<Texts>>),
    mut lit: ResMut<Lit>,
) {
    if still.is_some_and(|still| still.0) || busy.is_some_and(|busy| busy.0) {
        return;
    }
    lit.0 = None;
    let atlases = Atlases::load(&images, &assets);
    let font: Handle<Font> = assets.load(menu::MENU_FONT);
    commands
        .spawn((
            Window,
            screen(),
            GlobalZIndex(20),
            Pickable::IGNORE,
            DespawnOnExit(Play::Paused),
        ))
        .with_children(|picture| {
            picture.spawn((
                choice::place(WINDOW),
                BackgroundColor(choice::SHADE),
                Pickable::IGNORE,
            ));
            choice::frame_about(picture, &atlases, FRAMED);
            for (entry, row) in Entry::ALL.into_iter().zip(ROWS) {
                let rect = Rect::from_corners(
                    WINDOW.min + Vec2::new(0.0, row),
                    Vec2::new(WINDOW.max.x, WINDOW.min.y + row + ROW),
                );
                picture
                    .spawn((
                        entry,
                        Node {
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..choice::place(rect)
                        },
                        Pickable::default(),
                    ))
                    .observe(move |_: On<PointerOver>, mut lit: ResMut<Lit>| {
                        lit.0 = Some(entry);
                    })
                    .observe(move |_: On<PointerOut>, mut lit: ResMut<Lit>| {
                        if lit.0 == Some(entry) {
                            lit.0 = None;
                        }
                    })
                    .observe(move |click: On<PointerClick>, mut commands: Commands| {
                        if click.button == PointerButton::Primary {
                            commands.run_system_cached_with(choose, entry);
                        }
                    })
                    .with_child((
                        Text::new(word(texts.as_deref(), entry.text())),
                        TextFont {
                            font: font.clone().into(),
                            font_size: FontSize::Px(ITEM_SIZE),
                            ..default()
                        },
                        TextColor(PALE),
                        TextShadow {
                            offset: Vec2::splat(1.0),
                            color: menu::OUTLINE,
                        },
                        Pickable::IGNORE,
                    ));
            }
        });
}

/// Lights the item under the cursor, the others gold and the ones that do nothing yet grey.
fn light(lit: Res<Lit>, entries: Query<(&Entry, &Children)>, mut texts: Query<&mut TextColor>) {
    for (entry, children) in &entries {
        let colour = if !entry.works() {
            menu::ITEM_GREY
        } else if lit.0 == Some(*entry) {
            LIT
        } else {
            PALE
        };
        for &child in children {
            if let Ok(mut text) = texts.get_mut(child) {
                text.set_if_neq(TextColor(colour));
            }
        }
    }
}

/// Does what the item `entry` does: Continue goes on, Quit asks first; the others nothing yet.
fn choose(
    In(entry): In<Entry>,
    mut commands: Commands,
    mut play: ResMut<NextState<Play>>,
    dialogs: Query<(), With<Dialog>>,
    (images, assets, texts): (Res<choice::Images>, Res<AssetServer>, Option<Res<Texts>>),
) {
    if !dialogs.is_empty() {
        return;
    }
    match entry {
        Entry::Continue => play.set(Play::Running),
        Entry::Quit => ask(&mut commands, &images, &assets, texts.as_deref()),
        Entry::Options | Entry::SaveGame | Entry::Export => {}
    }
}

/// The dialog that asks whether to quit the game (`DEFAULT_BUSYDLG`, `UI_BUSY_2MAINMENU`).
fn ask(
    commands: &mut Commands,
    images: &choice::Images,
    assets: &AssetServer,
    texts: Option<&Texts>,
) {
    let buttons = busy_dialog::spawn(
        commands,
        (images, assets, texts),
        (
            Dialog,
            ("UI_BUSY_2MAINMENU", "Do you want to quit the game?"),
        ),
        &[
            (Answer::Ok, busy_dialog::OK_BESIDE, busy_dialog::OK),
            (Answer::Cancel, busy_dialog::CANCEL, ("1025", "Cancel")),
        ],
    );
    for (button, answer) in buttons {
        commands
            .entity(button)
            .observe(move |click: On<PointerClick>, mut commands: Commands| {
                if click.button == PointerButton::Primary {
                    commands.run_system_cached_with(answered, answer);
                }
            });
    }
}

/// OK leaves the game for the main menu; Cancel closes the dialog, the menu staying.
fn answered(
    In(answer): In<Answer>,
    mut commands: Commands,
    mut state: ResMut<NextState<AppState>>,
    dialogs: Query<Entity, With<Dialog>>,
) {
    match answer {
        Answer::Ok => state.set(AppState::Menu),
        Answer::Cancel => {
            for dialog in &dialogs {
                commands.entity(dialog).despawn();
            }
        }
    }
}

/// Esc while the dialog asks closes it, as Cancel does, before the pause takes the key; and a click
/// on the taskbar's Options button pauses the game under the menu, or goes on (`UI_TB_OPTIONS`),
/// but under the key P's pause, whose window covers the screen.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn keys(
    mut commands: Commands,
    mut keys: crate::controls::KeysMut,
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&bevy::window::Window>),
    images: Res<UiImages>,
    (play, mut next): (Res<State<Play>>, ResMut<NextState<Play>>),
    busy: Res<Busy>,
    dialogs: Query<Entity, With<Dialog>>,
) {
    if keys.just_pressed(crate::controls::Action::Back) && !dialogs.is_empty() {
        keys.clear(crate::controls::Action::Back);
        for dialog in &dialogs {
            commands.entity(dialog).despawn();
        }
        return;
    }
    let clicked = buttons.just_pressed(MouseButton::Left)
        && dialogs.is_empty()
        && !busy.0
        && crate::inventory::screen_point(&pointer, &ui_scale, &windows).is_some_and(|at| {
            let (index, corner) = bar::OPTIONS_BUTTON;
            Rect::from_corners(corner, corner + images.size(index)).contains(at)
        });
    if clicked {
        next.set(match play.get() {
            Play::Running => Play::Paused,
            Play::Paused => Play::Running,
        });
    }
}

/// The menu over a game.
pub struct EscMenuPlugin;

impl Plugin for EscMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Lit>()
            .add_systems(OnEnter(Play::Paused), show)
            .add_systems(
                Update,
                (
                    keys.before(crate::pause::toggle),
                    light.run_if(in_state(Play::Paused)),
                )
                    .run_if(in_state(AppState::InGame)),
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
    fn esc_opens_the_menu_whose_quit_asks_before_it_leaves_for_the_main_menu() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::state::app::StatesPlugin,
            AssetPlugin::default(),
        ))
        .init_asset::<Font>()
        .init_asset::<Image>()
        .insert_state(AppState::InGame)
        .add_plugins((crate::pause::PausePlugin, EscMenuPlugin))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<choice::Images>()
        .init_resource::<UiImages>()
        .init_resource::<Pointer>()
        .init_resource::<UiScale>();
        app.update();
        let play = |app: &App| app.world().get_resource::<State<Play>>().map(|s| *s.get());
        let count = |app: &mut App| app.world_mut().query::<&Entry>().iter(app.world()).count();
        let asked = |app: &mut App| {
            app.world_mut()
                .query_filtered::<(), With<Dialog>>()
                .iter(app.world())
                .count()
        };
        press(&mut app, KeyCode::Escape);
        assert_eq!(play(&app), Some(Play::Paused));
        assert_eq!(count(&mut app), 5);
        // Continue goes on, the window gone.
        app.world_mut()
            .run_system_cached_with(choose, Entry::Continue)
            .unwrap();
        app.update();
        assert_eq!(play(&app), Some(Play::Running));
        assert_eq!(count(&mut app), 0);
        // Quit asks; Esc closes the dialog alone, the menu staying.
        press(&mut app, KeyCode::Escape);
        app.world_mut()
            .run_system_cached_with(choose, Entry::Quit)
            .unwrap();
        app.update();
        assert_eq!(asked(&mut app), 1);
        press(&mut app, KeyCode::Escape);
        assert_eq!(asked(&mut app), 0);
        assert_eq!(play(&app), Some(Play::Paused));
        // OK leaves for the main menu.
        app.world_mut()
            .run_system_cached_with(choose, Entry::Quit)
            .unwrap();
        app.update();
        app.world_mut()
            .run_system_cached_with(answered, Answer::Ok)
            .unwrap();
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Menu
        );
        assert_eq!(play(&app), None);
    }
}
