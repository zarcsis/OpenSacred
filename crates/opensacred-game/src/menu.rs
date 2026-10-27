//! The main menu, its submenus, the choice of the hero and the pause's menu (docs/10 §4). The main
//! menu is the original's (docs/re/menus.md, #1118): its picture of 12 tiles, the column of its
//! items in Carolingia, gold, lit under the cursor with a glow and fading in as a menu replaces
//! another, and the Multiplayer and Extras submenus; the items whose screens the game does not
//! have yet stand greyed (docs/decisions/2026-10-19-menus.md). The campaigns open their choices
//! of the hero (`choice`); in a paused game (`pause`), Continue, Main menu and Quit over
//! the frame dimmed. The mouse chooses by pointing and clicking, the keyboard with the arrows and
//! Enter; Esc goes back a screen.

use std::collections::HashMap;
use std::path::Path;

use crate::game::{AppState, FONT, GOLD};
use crate::hud;
use crate::lines::Texts;
use crate::pause::Play;
use bevy::picking::events::{PointerClick, PointerEnter};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::text::FontSize;

/// The menu's screen.
#[derive(SubStates, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[source(AppState = AppState::Menu)]
pub enum Screen {
    #[default]
    Main,
    Multiplayer,
    Extras,
    Heroes,
    Underworld,
}

/// What an item of a menu does.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Ancaria,
    Underworld,
    Multiplayer,
    Options,
    LoadGame,
    Extras,
    Quit,
    LocalNetwork,
    ClosedInternet,
    OpenInternet,
    Credits,
    Intro,
    UnderworldIntro,
    Back,
    Continue,
    MainMenu,
}

impl Item {
    /// The name of its text among the game's texts (docs/re/menus.md), a number's for `res:N`,
    /// and the word it shows without them.
    fn text(self) -> (&'static str, &'static str) {
        match self {
            Self::Ancaria => ("UI_MAINMENU_CAMPAIGN_SACRED", "Ancaria"),
            Self::Underworld => ("UI_MAINMENU_CAMPAIGN_ADDON", "Underworld"),
            Self::Multiplayer => ("1042", "Multiplayer"),
            Self::Options => ("1043", "Options"),
            Self::Extras => ("1044", "Extras"),
            Self::LoadGame => ("1045", "Load Game"),
            Self::Quit => ("1046", "Quit"),
            Self::LocalNetwork => ("UI_MAIN_LAN2", "Local Network"),
            Self::ClosedInternet => ("UI_MAIN_INET_CLOSED", "Closed Internet"),
            Self::OpenInternet => ("UI_NET_OPENNET", "Open Internet"),
            Self::Credits => ("UI_MAIN_CREDITS", "Credits"),
            Self::Intro => ("UI_MAIN_INTRO", "Intro"),
            Self::UnderworldIntro => ("UI_MAIN_INTRO_UW", "Underworld Intro"),
            Self::Back => ("UI_NET_BACK", "Back"),
            Self::Continue | Self::MainMenu => ("", ""),
        }
    }

    /// Whether it does anything yet: multiplayer waits for M9, the options for #1096, the saves for
    /// #1095, the credits and the movies for #1094.
    fn works(self) -> bool {
        !matches!(
            self,
            Self::Options
                | Self::LoadGame
                | Self::LocalNetwork
                | Self::ClosedInternet
                | Self::OpenInternet
                | Self::Credits
                | Self::Intro
                | Self::UnderworldIntro
        )
    }
}

/// The items of the main menu's screens, each with its row: the rows 32 pixels apart, the row 0's
/// centre on the row 287 of the original's screen, an empty row before the last item.
fn rows(screen: Screen) -> &'static [(Item, u8)] {
    match screen {
        Screen::Main => &[
            (Item::Ancaria, 0),
            (Item::Underworld, 1),
            (Item::Multiplayer, 2),
            (Item::Options, 3),
            (Item::LoadGame, 4),
            (Item::Extras, 5),
            (Item::Quit, 7),
        ],
        Screen::Multiplayer => &[
            (Item::LocalNetwork, 1),
            (Item::ClosedInternet, 2),
            (Item::OpenInternet, 3),
            (Item::Back, 5),
        ],
        Screen::Extras => &[
            (Item::Credits, 0),
            (Item::Intro, 1),
            (Item::UnderworldIntro, 2),
            (Item::Back, 4),
        ],
        Screen::Heroes | Screen::Underworld => &[],
    }
}

/// An item's place on its screen, in the order of the arrows.
#[derive(Component)]
struct Place(usize);

/// An item of the main menu's screens, which the original's gold and glow light.
#[derive(Component)]
struct Gilded;

/// The place of the item chosen on the screen: the one the mouse points at, or the arrows moved
/// to.
#[derive(Resource, Default)]
struct Chosen(usize);

/// How long the screen has shown, in seconds, which its items fade in by.
#[derive(Resource, Default)]
struct Shown(f32);

/// The menus' images, relative to the asset directory, if the converted textures have them.
#[derive(Resource, Default)]
pub struct Images {
    /// The main menu's 12 tiles, four to a row from the top left.
    main: Vec<Option<String>>,
    /// The main menu's water: its bed, and the portal reflected in it.
    bed: Option<String>,
    reflection: Option<String>,
}

impl Images {
    /// Finds the main menu's tiles and water among the converted textures.
    pub fn find(assets: &Path) -> Self {
        let tiles: Vec<String> = (1..=TILES).map(|n| format!("gui_uw_option{n}")).collect();
        let mut names: Vec<&str> = tiles.iter().map(String::as_str).collect();
        names.extend(["ui_uw_main_center", "ui_uw_main_ref"]);
        let mut found = hud::find_textures(assets, &names);
        let reflection = found.pop().flatten();
        let bed = found.pop().flatten();
        Self {
            main: found,
            bed,
            reflection,
        }
    }
}

/// The menus' words from the game's texts, by their names.
#[derive(Resource, Default)]
pub struct Words(HashMap<&'static str, String>);

impl Words {
    /// The words of the menus' items and of the choice of the hero among the texts of `assets`.
    pub fn read(assets: &Path) -> Self {
        let texts = Texts::read(assets);
        let names = [Screen::Main, Screen::Multiplayer, Screen::Extras]
            .into_iter()
            .flat_map(|screen| rows(screen).iter().map(|&(item, _)| item.text().0))
            .chain(crate::choice::names());
        Self(
            names
                .filter_map(|name| Some((name, texts.named(name)?.to_owned())))
                .collect(),
        )
    }

    /// What `item` says.
    fn of(&self, item: Item) -> &str {
        let (name, word) = item.text();
        self.get(name).unwrap_or(word)
    }

    /// The text named `name`, if the game's texts have it.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }
}

/// The tiles of the main menu's picture, and their side.
const TILES: usize = 12;
const TILE: f32 = 256.0;
/// The original's screen, which the menus' pictures fill.
pub const SCREEN: Vec2 = Vec2::new(1024.0, 768.0);
/// How a menu's water shows: the row where its band at the picture's foot starts, the rows of
/// the bed's and the reflection's textures of 1024 × 1024 squeezed into it, the reflection's
/// opacity and the veil over both, matched by eye to the original's frames.
pub struct Water {
    pub top: f32,
    pub bed: (f32, f32),
    pub reflection: (f32, f32),
    pub reflection_alpha: f32,
    pub veil: Color,
}

/// The main menu's water.
const MAIN_WATER: Water = Water {
    top: 640.0,
    bed: (634.0, 914.0),
    reflection: (560.0, 1024.0),
    reflection_alpha: 0.65,
    veil: Color::linear_rgba(0.78, 0.84, 0.84, 0.08),
};
const WATER_TEXTURE: f32 = 1024.0;
/// The main menu's row 0's centre, the rows' pitch, and the items' size in Carolingia.
const FIRST_ROW: f32 = 287.0;
const ROW: f32 = 32.0;
const ITEM_SIZE: f32 = 29.0;
/// The main menu's font, Carolingia (`font/CAROLING.TTF`).
const MENU_FONT: &str = "ui/fonts/caroling.ttf";
/// The items' gold, an item lit, one that does nothing yet and their dark outline, in the frame's
/// sRGB values (`hud`): the middle of the original's gold, which darkens down the letters.
const ITEM_GOLD: Color = Color::linear_rgb(0.94, 0.70, 0.26);
const ITEM_LIT: Color = Color::linear_rgb(1.0, 0.96, 0.72);
const ITEM_GREY: Color = Color::linear_rgb(0.52, 0.48, 0.42);
const OUTLINE: Color = Color::linear_rgb(0.08, 0.0, 0.0);
/// The glow behind an item lit: pale, blurred at its edges.
const GLOW: Color = Color::linear_rgba(1.0, 0.95, 0.8, 0.45);
const GLOW_BLUR: f32 = 10.0;
const GLOW_SPREAD: f32 = 2.0;
/// How long a screen's items take to fade in, in seconds.
const FADE: f32 = 0.25;
/// The size of the version in the picture's bottom right corner.
const VERSION_SIZE: f32 = 12.0;
const VERSION: Color = Color::linear_rgb(0.85, 0.85, 0.85);

/// The size of the pause's items on the original's 768-pixel-high screen (`hud`), and their
/// colour not chosen, in the frame's sRGB values.
const TITLE: f32 = 40.0;
const DIM: Color = Color::linear_rgb(0.5, 0.42, 0.27);
/// What the pause's menu lays over the frame.
const SHADE: Color = Color::linear_rgba(0.0, 0.0, 0.0, 0.6);

/// An item's node, chosen when the mouse enters it and done when it is clicked, if it does
/// anything yet.
fn item<'a>(
    parent: &'a mut ChildSpawnerCommands,
    item: Item,
    place: usize,
    node: impl Bundle,
) -> EntityCommands<'a> {
    let mut entity = parent.spawn((item, Place(place), node));
    entity
        .observe(move |_: On<PointerEnter>, mut chosen: ResMut<Chosen>| {
            if item.works() {
                chosen.0 = place;
            }
        })
        .observe(move |click: On<PointerClick>, mut commands: Commands| {
            if click.button == PointerButton::Primary {
                commands.run_system_cached_with(act, item);
            }
        });
    entity
}

/// The main menu or a submenu: the original's picture with the column of the screen's items, the
/// first that does anything chosen, and the game's version in its corner.
fn main_screen(
    mut commands: Commands,
    screen: Res<State<Screen>>,
    images: Res<Images>,
    words: Res<Words>,
    assets: Res<AssetServer>,
    mut chosen: ResMut<Chosen>,
    mut shown: ResMut<Shown>,
) {
    let screen = *screen.get();
    let items = rows(screen);
    chosen.0 = items.iter().position(|(item, _)| item.works()).unwrap_or(0);
    shown.0 = 0.0;
    let font: Handle<Font> = assets.load(MENU_FONT);
    let small: Handle<Font> = assets.load(FONT);
    commands
        .spawn((
            DespawnOnExit(screen),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::BLACK),
        ))
        .with_children(|root| {
            root.spawn(Node {
                width: Val::Px(SCREEN.x),
                height: Val::Px(SCREEN.y),
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|picture| {
                water(
                    picture,
                    [&images.bed, &images.reflection],
                    &assets,
                    &MAIN_WATER,
                );
                tiles(picture, &images.main, &assets);
                for (place, &(entry, row)) in items.iter().enumerate() {
                    picture
                        .spawn(Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(0.0),
                            top: Val::Px(FIRST_ROW + ROW * f32::from(row) - ROW / 2.0),
                            width: Val::Px(SCREEN.x),
                            height: Val::Px(ROW),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|line| {
                            item(
                                line,
                                entry,
                                place,
                                (
                                    Gilded,
                                    Text::new(words.of(entry)),
                                    TextFont {
                                        font: font.clone().into(),
                                        font_size: FontSize::Px(ITEM_SIZE),
                                        ..default()
                                    },
                                    TextColor(Color::NONE),
                                    TextShadow {
                                        offset: Vec2::splat(1.0),
                                        color: Color::NONE,
                                    },
                                    Node {
                                        padding: UiRect::horizontal(Val::Px(6.0)),
                                        ..default()
                                    },
                                    BoxShadow::new(
                                        Color::NONE,
                                        Val::Px(0.0),
                                        Val::Px(0.0),
                                        Val::Px(GLOW_SPREAD),
                                        Val::Px(GLOW_BLUR),
                                    ),
                                ),
                            );
                        });
                }
                picture.spawn((
                    Text::new(env!("OPENSACRED_VERSION")),
                    TextFont {
                        font: small.into(),
                        font_size: FontSize::Px(VERSION_SIZE),
                        ..default()
                    },
                    TextColor(VERSION),
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(6.0),
                        bottom: Val::Px(4.0),
                        ..default()
                    },
                ));
            });
        });
}

/// A picture of 12 tiles, four to a row from the top left.
pub fn tiles(picture: &mut ChildSpawnerCommands, tiles: &[Option<String>], assets: &AssetServer) {
    let places = (0..3u8).flat_map(|row| (0..4u8).map(move |column| (column, row)));
    for (tile, (column, row)) in tiles.iter().zip(places) {
        let Some(tile) = tile else {
            continue;
        };
        picture.spawn((
            ImageNode::new(hud::image(assets, tile.clone())),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(TILE * f32::from(column)),
                top: Val::Px(TILE * f32::from(row)),
                width: Val::Px(TILE),
                height: Val::Px(TILE),
                ..default()
            },
        ));
    }
}

/// A menu's water under the bottom tiles, which leave the main menu's clear below the row 652:
/// its bed, what stands over it reflected and a pale veil, still, as the original's plane of water
/// looks from the screen (`ui_uw_main_center`, `ui_uw_main_ref` and the choice of the hero's;
/// docs/decisions/2026-10-19-menus.md).
pub fn water(
    picture: &mut ChildSpawnerCommands,
    [bed, reflection]: [&Option<String>; 2],
    assets: &AssetServer,
    water: &Water,
) {
    let band = Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(water.top),
        width: Val::Px(SCREEN.x),
        height: Val::Px(SCREEN.y - water.top),
        ..default()
    };
    for (texture, rows, alpha) in [
        (bed, water.bed, 1.0),
        (reflection, water.reflection, water.reflection_alpha),
    ] {
        if let Some(texture) = texture {
            let mut image = ImageNode::new(hud::image(assets, texture.clone()))
                .with_color(Color::WHITE.with_alpha(alpha));
            image.rect = Some(Rect::new(0.0, rows.0, WATER_TEXTURE, rows.1));
            image.image_mode = NodeImageMode::Stretch;
            picture.spawn((image, band.clone()));
        }
    }
    picture.spawn((BackgroundColor(water.veil), band));
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
                    font_size: FontSize::Px(TITLE),
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

/// Does what `item` does; an item that does nothing yet does nothing.
fn act(
    In(item): In<Item>,
    mut screen: ResMut<NextState<Screen>>,
    mut state: ResMut<NextState<AppState>>,
    play: Option<ResMut<NextState<Play>>>,
    mut exit: MessageWriter<AppExit>,
) {
    match item {
        Item::Ancaria => screen.set(Screen::Heroes),
        Item::Underworld => screen.set(Screen::Underworld),
        Item::Multiplayer => screen.set(Screen::Multiplayer),
        Item::Extras => screen.set(Screen::Extras),
        Item::Back => screen.set(Screen::Main),
        Item::Quit => {
            exit.write(AppExit::Success);
        }
        Item::Continue => {
            if let Some(mut play) = play {
                play.set(Play::Running);
            }
        }
        Item::MainMenu => state.set(AppState::Menu),
        Item::Options
        | Item::LoadGame
        | Item::LocalNetwork
        | Item::ClosedInternet
        | Item::OpenInternet
        | Item::Credits
        | Item::Intro
        | Item::UnderworldIntro => {}
    }
}

/// The arrows move the choice among the items that do anything, Enter does it, Esc goes back from
/// a submenu or the heroes to the main menu (the pause's Esc is the pause's, `pause`).
fn keys(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    screen: Option<Res<State<Screen>>>,
    mut next: ResMut<NextState<Screen>>,
    mut chosen: ResMut<Chosen>,
    items: Query<(&Item, &Place)>,
) {
    let pressed = |codes: &[KeyCode]| keys.any_just_pressed(codes.iter().copied());
    if pressed(&[KeyCode::Escape]) && screen.is_some_and(|screen| *screen.get() != Screen::Main) {
        next.set(Screen::Main);
        return;
    }
    let mut places: Vec<usize> = items
        .iter()
        .filter(|(item, _)| item.works())
        .map(|(_, place)| place.0)
        .collect();
    if places.is_empty() {
        return;
    }
    places.sort_unstable();
    let at = places.iter().position(|&place| place == chosen.0);
    let count = places.len();
    if pressed(&[KeyCode::ArrowDown, KeyCode::ArrowRight]) {
        chosen.0 = places[at.map_or(0, |at| (at + 1) % count)];
    }
    if pressed(&[KeyCode::ArrowUp, KeyCode::ArrowLeft]) {
        chosen.0 = places[at.map_or(count - 1, |at| (at + count - 1) % count)];
    }
    if pressed(&[KeyCode::Enter, KeyCode::NumpadEnter])
        && let Some((item, _)) = items
            .iter()
            .find(|(item, place)| place.0 == chosen.0 && item.works())
    {
        commands.run_system_cached_with(act, *item);
    }
}

/// A pause's item, and its text.
type Lit = (&'static Item, &'static Place, &'static mut TextColor);

/// A main menu's item, its text's outline and its glow.
type Gilt = (
    &'static Item,
    &'static Place,
    &'static mut TextColor,
    &'static mut TextShadow,
    &'static mut BoxShadow,
);

/// Lights the item chosen: a main menu's in near white with a glow behind it, the others gold
/// and the ones that do nothing yet grey, all fading in as their screen shows; a pause's item in
/// gold.
fn light(
    time: Res<Time>,
    chosen: Res<Chosen>,
    mut shown: ResMut<Shown>,
    mut gilded: Query<Gilt, With<Gilded>>,
    items: Query<Lit, Without<Gilded>>,
) {
    shown.0 += time.delta_secs();
    let alpha = (shown.0 / FADE).clamp(0.0, 1.0);
    for (item, place, mut text, mut outline, mut glow) in &mut gilded {
        let on = place.0 == chosen.0 && item.works();
        let colour = if !item.works() {
            ITEM_GREY
        } else if on {
            ITEM_LIT
        } else {
            ITEM_GOLD
        };
        text.0 = colour.with_alpha(alpha);
        outline.color = OUTLINE.with_alpha(alpha);
        if let Some(style) = glow.0.first_mut() {
            style.color = if on {
                GLOW.with_alpha(GLOW.alpha() * alpha)
            } else {
                Color::NONE
            };
        }
    }
    for (_, place, mut text) in items {
        text.0 = if place.0 == chosen.0 { GOLD } else { DIM };
    }
}

/// Whether a menu shows: the main menu's, or the pause's.
fn shown(app: Option<Res<State<AppState>>>, play: Option<Res<State<Play>>>) -> bool {
    app.is_some_and(|app| *app.get() == AppState::Menu)
        || play.is_some_and(|play| *play.get() == Play::Paused)
}

/// The main menu, its submenus, the pause's menu and their keys.
pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_sub_state::<Screen>()
            .init_resource::<Chosen>()
            .init_resource::<Shown>()
            .init_resource::<Images>()
            .init_resource::<Words>()
            .add_systems(OnEnter(Screen::Main), main_screen)
            .add_systems(OnEnter(Screen::Multiplayer), main_screen)
            .add_systems(OnEnter(Screen::Extras), main_screen)
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
    use crate::game::Request;

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

    fn menu_app() -> App {
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
        app
    }

    fn screen(app: &App) -> Screen {
        *app.world().resource::<State<Screen>>().get()
    }

    fn items(app: &mut App) -> Vec<Item> {
        let mut items: Vec<(usize, Item)> = app
            .world_mut()
            .query::<(&Item, &Place)>()
            .iter(app.world())
            .map(|(item, place)| (place.0, *item))
            .collect();
        items.sort_unstable_by_key(|(place, _)| *place);
        items.into_iter().map(|(_, item)| item).collect()
    }

    #[test]
    fn the_campaign_opens_the_choice_of_the_hero_and_esc_goes_back() {
        let mut app = menu_app();
        assert_eq!(screen(&app), Screen::Main);
        assert_eq!(
            items(&mut app),
            [
                Item::Ancaria,
                Item::Underworld,
                Item::Multiplayer,
                Item::Options,
                Item::LoadGame,
                Item::Extras,
                Item::Quit
            ]
        );
        press(&mut app, KeyCode::Enter);
        assert_eq!(screen(&app), Screen::Heroes);
        press(&mut app, KeyCode::Escape);
        assert_eq!(screen(&app), Screen::Main);
    }

    #[test]
    fn the_arrows_pass_over_the_items_that_do_nothing_yet_into_the_submenus() {
        let mut app = menu_app();
        // Down from the Ancaria campaign is the Underworld's, whose choice Esc leaves.
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::Enter);
        assert_eq!(screen(&app), Screen::Underworld);
        press(&mut app, KeyCode::Escape);
        assert_eq!(screen(&app), Screen::Main);
        // Down twice is Multiplayer.
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::Enter);
        assert_eq!(screen(&app), Screen::Multiplayer);
        assert_eq!(
            items(&mut app),
            [
                Item::LocalNetwork,
                Item::ClosedInternet,
                Item::OpenInternet,
                Item::Back
            ]
        );
        // Back is the one item of the submenu that does anything.
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::Enter);
        assert_eq!(screen(&app), Screen::Main);
        // Up from the Ancaria campaign is Quit; up again, Extras, past the options and the saves.
        press(&mut app, KeyCode::ArrowUp);
        press(&mut app, KeyCode::ArrowUp);
        press(&mut app, KeyCode::Enter);
        assert_eq!(screen(&app), Screen::Extras);
        assert_eq!(items(&mut app).last(), Some(&Item::Back));
        press(&mut app, KeyCode::Escape);
        assert_eq!(screen(&app), Screen::Main);
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
