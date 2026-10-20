//! The main menu, the choice of the hero and the pause's menu (docs/10 §4), until the original's
//! interface of M5 (docs/decisions/2026-10-19-menus.md): the game's logo over New Game and Quit,
//! then the seven classes of `--class` with their portraits; in a paused game (`pause`),
//! Continue, Main menu and Quit over the frame dimmed. The mouse chooses by pointing and clicking,
//! the keyboard with the arrows and Enter; Esc goes back a screen. A hero starts a game with the
//! command line's other options (`game::Request`).

use std::path::Path;

use bevy::picking::events::{PointerClick, PointerEnter};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::text::FontSize;
use clap::ValueEnum;

use crate::game::{AppState, Class, FONT, GOLD, Request};
use crate::hud;
use crate::pause::Play;

/// The menu's screen.
#[derive(SubStates, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[source(AppState = AppState::Menu)]
pub enum Screen {
    #[default]
    Main,
    Heroes,
}

/// What an item of a menu does.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    NewGame,
    Quit,
    Hero(Class),
    Continue,
    MainMenu,
}

/// An item's place on its screen, in the order of the arrows.
#[derive(Component)]
struct Place(usize);

/// The place of the item chosen on the screen: the one the mouse points at, or the arrows moved
/// to.
#[derive(Resource, Default)]
struct Chosen(usize);

/// The menus' images, relative to the asset directory, if the converted textures have them.
#[derive(Resource, Default)]
pub struct Images {
    logo: Option<String>,
    portraits: Vec<Option<String>>,
}

impl Images {
    /// Finds the logo and the portraits among the converted textures.
    pub fn find(assets: &Path) -> Self {
        let mut names = vec!["gui_main_logo"];
        names.extend(classes().iter().map(|class| class.portrait()));
        let mut found = hud::find_textures(assets, &names);
        let logo = found.remove(0);
        Self {
            logo,
            portraits: found,
        }
    }
}

/// The size of the items' text on the original's 768-pixel-high screen (`hud`).
const ITEM: f32 = 40.0;
const NAME: f32 = 18.0;
/// The portraits' side.
const PORTRAIT: f32 = 112.0;
/// The colour of the items not chosen, in the frame's sRGB values (`hud`).
const DIM: Color = Color::linear_rgb(0.5, 0.42, 0.27);
const FRAME: Color = Color::linear_rgba(0.0, 0.0, 0.0, 0.0);
/// What the pause's menu lays over the frame.
const SHADE: Color = Color::linear_rgba(0.0, 0.0, 0.0, 0.6);

/// The classes in the order of the screen: `--class`'s.
fn classes() -> &'static [Class] {
    Class::value_variants()
}

/// An item's node, chosen when the mouse enters it and done when it is clicked.
fn item<'a>(
    parent: &'a mut ChildSpawnerCommands,
    item: Item,
    place: usize,
    node: impl Bundle,
) -> EntityCommands<'a> {
    let mut entity = parent.spawn((item, Place(place), node));
    entity
        .observe(move |_: On<PointerEnter>, mut chosen: ResMut<Chosen>| {
            chosen.0 = place;
        })
        .observe(move |click: On<PointerClick>, mut commands: Commands| {
            if click.button == PointerButton::Primary {
                commands.run_system_cached_with(act, item);
            }
        });
    entity
}

fn main_screen(
    mut commands: Commands,
    images: Res<Images>,
    assets: Res<AssetServer>,
    mut chosen: ResMut<Chosen>,
) {
    chosen.0 = 0;
    let font: Handle<Font> = assets.load(FONT);
    commands
        .spawn((
            DespawnOnExit(Screen::Main),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(16.0),
                ..default()
            },
        ))
        .with_children(|column| {
            if let Some(logo) = &images.logo {
                column.spawn((
                    ImageNode::new(hud::image(&assets, logo.clone())),
                    Node {
                        width: Val::Px(512.0),
                        height: Val::Px(128.0),
                        margin: UiRect::bottom(Val::Px(48.0)),
                        ..default()
                    },
                ));
            }
            words(
                column,
                &font,
                &[(Item::NewGame, "New Game"), (Item::Quit, "Quit")],
            );
        });
}

/// Items of a word each, in order.
fn words(column: &mut ChildSpawnerCommands, font: &Handle<Font>, entries: &[(Item, &str)]) {
    for (place, (entry, text)) in entries.iter().enumerate() {
        item(
            column,
            *entry,
            place,
            (
                Text::new(*text),
                TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(ITEM),
                    ..default()
                },
                TextColor(DIM),
            ),
        );
    }
}

/// The pause's menu over the frame dimmed, unless the pause is a still frame's.
fn pause_screen(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut chosen: ResMut<Chosen>,
    still: Option<Res<crate::pause::Still>>,
) {
    if still.is_some_and(|still| still.0) {
        return;
    }
    chosen.0 = 0;
    let font: Handle<Font> = assets.load(FONT);
    commands
        .spawn((
            DespawnOnExit(Play::Paused),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(SHADE),
        ))
        .with_children(|column| {
            words(
                column,
                &font,
                &[
                    (Item::Continue, "Continue"),
                    (Item::MainMenu, "Main menu"),
                    (Item::Quit, "Quit"),
                ],
            );
        });
}

fn heroes_screen(
    mut commands: Commands,
    images: Res<Images>,
    assets: Res<AssetServer>,
    mut chosen: ResMut<Chosen>,
) {
    chosen.0 = 0;
    let font: Handle<Font> = assets.load(FONT);
    commands
        .spawn((
            DespawnOnExit(Screen::Heroes),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(32.0),
                ..default()
            },
        ))
        .with_children(|column| {
            column.spawn((
                Text::new("Choose your hero"),
                TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(ITEM),
                    ..default()
                },
                TextColor(GOLD),
            ));
            column
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(12.0),
                    ..default()
                })
                .with_children(|row| {
                    for (place, class) in classes().iter().enumerate() {
                        let portrait = images.portraits.get(place).cloned().flatten();
                        item(
                            row,
                            Item::Hero(*class),
                            place,
                            (
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    align_items: AlignItems::Center,
                                    padding: UiRect::all(Val::Px(4.0)),
                                    border: UiRect::all(Val::Px(3.0)),
                                    row_gap: Val::Px(6.0),
                                    ..default()
                                },
                                BorderColor::all(FRAME),
                            ),
                        )
                        .with_children(|card| {
                            if let Some(portrait) = portrait {
                                card.spawn((
                                    ImageNode::new(hud::image(&assets, portrait)),
                                    Node {
                                        width: Val::Px(PORTRAIT),
                                        height: Val::Px(PORTRAIT),
                                        ..default()
                                    },
                                ));
                            }
                            card.spawn((
                                Text::new(class.name()),
                                TextFont {
                                    font: font.clone().into(),
                                    font_size: FontSize::Px(NAME),
                                    ..default()
                                },
                                TextColor(DIM),
                            ));
                        });
                    }
                });
        });
}

/// Does what `item` does.
fn act(
    In(item): In<Item>,
    mut request: ResMut<Request>,
    mut screen: ResMut<NextState<Screen>>,
    mut state: ResMut<NextState<AppState>>,
    play: Option<ResMut<NextState<Play>>>,
    mut exit: MessageWriter<AppExit>,
) {
    match item {
        Item::NewGame => screen.set(Screen::Heroes),
        Item::Quit => {
            exit.write(AppExit::Success);
        }
        Item::Hero(class) => {
            request.0.class = Some(class);
            state.set(AppState::Loading);
        }
        Item::Continue => {
            if let Some(mut play) = play {
                play.set(Play::Running);
            }
        }
        Item::MainMenu => state.set(AppState::Menu),
    }
}

/// The arrows move the choice, Enter does it, Esc goes back from the heroes to the main menu
/// (the pause's Esc is the pause's, `pause`).
fn keys(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    screen: Option<Res<State<Screen>>>,
    mut next: ResMut<NextState<Screen>>,
    mut chosen: ResMut<Chosen>,
    items: Query<(&Item, &Place)>,
) {
    let count = items.iter().count();
    if count == 0 {
        return;
    }
    let pressed = |codes: &[KeyCode]| keys.any_just_pressed(codes.iter().copied());
    if pressed(&[KeyCode::ArrowDown, KeyCode::ArrowRight]) {
        chosen.0 = (chosen.0 + 1) % count;
    }
    if pressed(&[KeyCode::ArrowUp, KeyCode::ArrowLeft]) {
        chosen.0 = (chosen.0 + count - 1) % count;
    }
    if pressed(&[KeyCode::Enter, KeyCode::NumpadEnter])
        && let Some((item, _)) = items.iter().find(|(_, place)| place.0 == chosen.0)
    {
        commands.run_system_cached_with(act, *item);
    }
    if pressed(&[KeyCode::Escape]) && screen.is_some_and(|screen| *screen.get() == Screen::Heroes) {
        next.set(Screen::Main);
    }
}

/// An item's place, its children, and its text or frame.
type Lit = (
    &'static Place,
    Option<&'static Children>,
    Option<&'static mut TextColor>,
    Option<&'static mut BorderColor>,
);

/// Lights the item chosen: its text, or a hero's frame and name.
fn light(
    chosen: Res<Chosen>,
    items: Query<Lit, With<Item>>,
    mut texts: Query<&mut TextColor, Without<Item>>,
) {
    for (place, children, text, border) in items {
        let on = place.0 == chosen.0;
        let colour = if on { GOLD } else { DIM };
        if let Some(mut text) = text {
            text.0 = colour;
        }
        if let Some(mut border) = border {
            *border = BorderColor::all(if on { GOLD } else { FRAME });
        }
        for child in children.into_iter().flatten() {
            if let Ok(mut text) = texts.get_mut(*child) {
                text.0 = colour;
            }
        }
    }
}

/// Whether a menu shows: the main menu's, or the pause's.
fn shown(app: Option<Res<State<AppState>>>, play: Option<Res<State<Play>>>) -> bool {
    app.is_some_and(|app| *app.get() == AppState::Menu)
        || play.is_some_and(|play| *play.get() == Play::Paused)
}

/// The main menu, the choice of the hero, the pause's menu and their keys.
pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_sub_state::<Screen>()
            .init_resource::<Chosen>()
            .init_resource::<Images>()
            .add_systems(OnEnter(Screen::Main), main_screen)
            .add_systems(OnEnter(Screen::Heroes), heroes_screen)
            .add_systems(OnEnter(Play::Paused), pause_screen)
            .add_systems(
                Update,
                (keys.in_set(crate::frame::Control), light)
                    .chain()
                    .run_if(shown),
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
    fn new_game_then_a_hero_loads_a_game_of_that_class_and_esc_goes_back() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::state::app::StatesPlugin,
            AssetPlugin::default(),
        ))
        .init_asset::<Font>()
        .init_asset::<Image>()
        .init_state::<AppState>()
        .add_plugins(MenuPlugin)
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<Request>();
        app.update();
        let screen = |app: &App| *app.world().resource::<State<Screen>>().get();
        let items = |app: &mut App| app.world_mut().query::<&Item>().iter(app.world()).count();
        assert_eq!(screen(&app), Screen::Main);
        assert_eq!(items(&mut app), 2);
        press(&mut app, KeyCode::Enter);
        assert_eq!(screen(&app), Screen::Heroes);
        assert_eq!(items(&mut app), 7);
        press(&mut app, KeyCode::Escape);
        assert_eq!(screen(&app), Screen::Main);
        press(&mut app, KeyCode::Enter);
        // Left of the first hero is the last, the Dwarf; three right of it, the Mage.
        press(&mut app, KeyCode::ArrowLeft);
        for _ in 0..3 {
            press(&mut app, KeyCode::ArrowRight);
        }
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Loading
        );
        assert_eq!(app.world().resource::<Request>().0.class, Some(Class::Mage));
    }

    #[test]
    fn the_pauses_menu_continues_or_leaves_for_the_main_menu() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::state::app::StatesPlugin,
            AssetPlugin::default(),
        ))
        .init_asset::<Font>()
        .init_asset::<Image>()
        .insert_state(AppState::InGame)
        .add_plugins((MenuPlugin, crate::pause::PausePlugin))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<Request>();
        app.update();
        let play = |app: &App| app.world().get_resource::<State<Play>>().map(|s| *s.get());
        let items = |app: &mut App| app.world_mut().query::<&Item>().iter(app.world()).count();
        assert_eq!(items(&mut app), 0);
        press(&mut app, KeyCode::Escape);
        assert_eq!(play(&app), Some(Play::Paused));
        assert_eq!(items(&mut app), 3);
        // Continue.
        press(&mut app, KeyCode::Enter);
        assert_eq!(play(&app), Some(Play::Running));
        assert_eq!(items(&mut app), 0);
        press(&mut app, KeyCode::Escape);
        press(&mut app, KeyCode::ArrowDown);
        // Main menu.
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Menu
        );
        assert_eq!(play(&app), None);
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
    }
}
