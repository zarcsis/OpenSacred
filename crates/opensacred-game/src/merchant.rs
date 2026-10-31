//! The merchant's window (#1618; docs/re/trade.md, "The merchant's window"): a creature of
//! `+0x200` bit `0x2000` (`quests::MERCHANT`) without a dialogue shows, as the hero speaks to it
//! (`talk`, `0x52AB70`), the inventory and the merchant's window (`cUI_Merchant`,
//! `UI_WND_MERCHANT`) at the screen's top left, 640 × 336: its panel tinted `0x9F000000` with the
//! merchant's portrait and the text 1197, and its tab sheet of three pages (`cUI_Merchandise`),
//! the weapons, the armour and the rest, each a grid of 16 × 9 cells of the stock
//! (`opensacred_rules::trade::stock`), which the window fills for the merchant as it opens: anew
//! for another merchant than the last, thinned and refilled for the same. The cells are tinted
//! as the backpack's, and yellow under an item that costs more than the hero's gold; a hovered
//! item's tooltip tells its price as bought (`tooltip`). The inventory's hide hides it.
//!
//! Not followed yet: buying and selling (#1619), the pieces of the frame (its panel's edges drawn
//! as the master's), the page's show's sound (14, 2, `0x1F`), the switch of each item to the
//! hero's class's variant as a page shows (`0x5C7370`), and the stock's keeping in the saves.

use bevy::prelude::*;
use opensacred_rules::drops::Dropped;
use opensacred_rules::trade::stock::{self, Buyer, COLUMNS, Page, ROWS, Stock};

use crate::backpack;
use crate::bar::{self, UiImages};
use crate::choice::Layer;
use crate::drops::Tables;
use crate::game::{AppState, FONT};
use crate::hud::Pointer;
use crate::inventory::{Shown, screen_point};
use crate::lines::Texts;
use crate::pickup::Pack;
use crate::tooltip::argb;

/// The window on the original's screen: its rectangle down to its pages' bottom, and its panel
/// (`UI_STATIC_BG`, 128 wide and 12 less than the window high) and its tint.
pub(crate) const WINDOW: Rect = Rect {
    min: Vec2::ZERO,
    max: Vec2::new(640.0, 340.0),
};
const PANEL: Rect = Rect {
    min: Vec2::new(512.0, 16.0),
    max: Vec2::new(640.0, 340.0),
};
const PANEL_TINT: Color = Color::srgba(0.0, 0.0, 0.0, 159.0 / 255.0);
/// The panel's edges (**estimate**, as the master's): a grey line of two pixels.
const EDGE: Color = Color::srgb(0.33, 0.33, 0.31);
/// The portrait's corner (`+0x17C`, `+0x180`), the title's line and colour (the text 1197 in
/// `0xFF0050FF`, **seen** centred in the panel at 214 to 229).
const PORTRAIT: Vec2 = Vec2::new(524.0, 42.0);
const TITLE: &str = "1197";
const TITLE_AT: Vec2 = Vec2::new(512.0, 208.0);
const TITLE_COLOUR: u32 = 0xFF00_50FF;
const FONT_SIZE: f32 = 20.0;
/// The tab sheet's corner (`UI_TABSHEET_MERCHANT`), where each page's grid starts, and its tabs'
/// texts (`UI_TABPAGE_WEAPONS`, `_ARMORS`, `_VARIOUS`).
pub(crate) const GRID: Vec2 = Vec2::new(0.0, 52.0);
const TABS: [&str; 3] = ["1057", "1058", "1059"];
/// The cells of an item that costs more than the hero's gold (`0xFFFF00` at `0x40`).
const DEAR_TINT: Color = backpack::tinted(0xFF, 0xFF, 0x00);
/// The render layer of the items' models.
const ITEMS: usize = 17;

/// The merchant whose window shows: its creature, its type (its portrait's) and its level.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Merchant {
    pub creature: Entity,
    kind: u32,
    level: u32,
    /// Whether its stock waits to be made for it.
    visiting: bool,
}

/// The merchants' stock, one for every merchant: the last one's.
#[derive(Resource, Default)]
pub struct Wares(pub Stock);

/// The page that the window shows.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Showing(pub Page);

impl Default for Showing {
    fn default() -> Self {
        Self(Page::Weapons)
    }
}

/// The window's container.
#[derive(Component)]
struct Root;

/// The items' camera, light and models.
#[derive(Component, Clone)]
struct Scene;

/// A cell's tint, at its column and row.
#[derive(Component)]
struct Tint((u8, u8));

/// An item's model, by its key in the page.
#[derive(Component)]
struct Model(u32);

/// A tab of the sheet and its page.
#[derive(Component)]
struct Tab(Page);

/// Shows the merchant's window for `creature` with the inventory (`0x52AB70`), its stock made
/// for it.
pub fn open(world: &mut World, creature: Entity) {
    let kind = world
        .get::<crate::ai::Mind>(creature)
        .map_or(0, |mind| mind.creature);
    let level = world
        .get::<crate::fight::Health>(creature)
        .map_or(1, |health| health.level);
    info!("merchant: {creature}, a {kind} of level {level}, shows its window");
    world.insert_resource(Merchant {
        creature,
        kind,
        level,
        visiting: true,
    });
    if let Some(mut shown) = world.get_resource_mut::<Shown>()
        && !shown.0
    {
        shown.0 = true;
    }
}

/// The stock made for the merchant as its window opens (the pages' event `0x2E` 8): by its
/// creature's id and level, for the hero of its level, class, skills and difficulty.
fn visit(
    merchant: Option<ResMut<Merchant>>,
    (mut wares, mut dice): (ResMut<Wares>, ResMut<crate::drops::Dice>),
    (tables, experience, difficulty): (
        Option<Res<Tables>>,
        Res<crate::fight::Experience>,
        Option<Res<crate::game::Difficulty>>,
    ),
    heroes: Query<&crate::fight::Base, With<crate::hero::Body>>,
) {
    let (Some(mut merchant), Some(tables), Some(base)) = (merchant, tables, heroes.iter().next())
    else {
        return;
    };
    if !merchant.visiting {
        return;
    }
    merchant.visiting = false;
    let hero = crate::drops::hero(base, experience.0.level);
    let buyer = Buyer {
        hero: &hero,
        difficulty: difficulty.map(|d| d.0).unwrap_or_default(),
        weapons: tables.weapon_lists(),
    };
    let id = merchant.creature.to_bits();
    wares
        .0
        .visit(id, merchant.level, &buyer, &tables.supply(), &mut dice.0);
    let counts: Vec<usize> = wares.0.shelves.iter().map(|s| s.items.len()).collect();
    info!(
        "merchant: the stock for a merchant of level {}: {counts:?} items",
        merchant.level
    );
}

/// Takes the window away with the inventory.
fn follow(mut commands: Commands, shown: Res<Shown>, merchant: Option<Res<Merchant>>) {
    if merchant.is_some() && !shown.0 {
        commands.remove_resource::<Merchant>();
    }
}

/// What the window shows, made again as it changes.
#[derive(Clone, Debug, PartialEq)]
struct Look {
    merchant: Merchant,
    page: Page,
}

/// Makes the window as the merchant shows it and as its page changes, and takes it away as it
/// goes.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn show(
    mut commands: Commands,
    (merchant, showing): (Option<Res<Merchant>>, Res<Showing>),
    (images, assets, texts): (Res<UiImages>, Res<AssetServer>, Option<Res<Texts>>),
    portraits: Option<Res<crate::talk::Portraits>>,
    (mut targets, mut layers): (ResMut<Assets<Image>>, ResMut<Assets<Layer>>),
    window: Option<Single<&Window>>,
    olds: Query<Entity, Or<(With<Root>, With<Scene>)>>,
    mut was: Local<Option<Look>>,
) {
    let now = merchant.as_deref().map(|merchant| Look {
        merchant: merchant.clone(),
        page: showing.0,
    });
    if *was == now {
        return;
    }
    *was = now.clone();
    for old in &olds {
        commands.entity(old).despawn();
    }
    let Some(look) = now else {
        return;
    };
    let said = |key: &str| {
        texts
            .as_deref()
            .and_then(|texts| texts.named(key))
            .unwrap_or(key)
            .to_owned()
    };
    let font: Handle<Font> = assets.load(FONT);
    let root = commands
        .spawn((Root, backpack::screen(), GlobalZIndex(-1)))
        .id();
    let mut panel = bar::placed(PANEL.min, PANEL.size());
    panel.border = UiRect::all(Val::Px(2.0));
    let panel = commands
        .spawn((
            panel,
            BackgroundColor(PANEL_TINT),
            BorderColor::all(EDGE),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(root).add_child(panel);
    // The merchant's portrait by its type, and the title.
    if let Some((file, rect)) = portraits.as_deref().and_then(|p| p.of(look.merchant.kind)) {
        let [x, y, w, h] = rect.map(f32::from);
        let mut image = ImageNode::new(crate::hud::image(&assets, file.to_owned()));
        image.rect = Some(Rect::new(x, y, x + w, y + h));
        let portrait = commands
            .spawn((
                bar::placed(PORTRAIT, Vec2::new(w, h)),
                image,
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(root).add_child(portrait);
    }
    let mut title = bar::placed(TITLE_AT, Vec2::new(PANEL.width(), FONT_SIZE + 4.0));
    title.justify_content = JustifyContent::Center;
    let title = commands
        .spawn((title, Pickable::IGNORE))
        .with_child((
            Text::new(said(TITLE)),
            TextFont {
                font: font.clone().into(),
                font_size: FontSize::Px(FONT_SIZE),
                ..default()
            },
            TextLayout {
                linebreak: LineBreak::NoWrap,
                ..default()
            },
            TextColor(argb(TITLE_COLOUR)),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(root).add_child(title);
    // The tab sheet's tabs, and the page's grid.
    let labels: Vec<String> = TABS.iter().map(|key| said(key)).collect();
    let sheet = commands
        .spawn((bar::placed(GRID, Vec2::new(640.0, 288.0)), Pickable::IGNORE))
        .id();
    commands.entity(root).add_child(sheet);
    let (tabs, each) =
        crate::inventory::spawn_tabs(&mut commands, (&images, &assets), &font, &labels);
    commands.entity(sheet).add_child(tabs);
    for (tab, page) in each.into_iter().zip(Page::ALL) {
        commands.entity(tab).insert(Tab(page));
    }
    let scale = window.map_or(1.0, |window| window.height() / bar::SCREEN.y);
    backpack::spawn_grid(
        &mut commands,
        (&images, &assets),
        (&mut targets, &mut layers),
        scale,
        (root, GRID, (COLUMNS, ROWS)),
        (ITEMS, Tint, Scene),
    );
}

/// The page's shelf of `wares` that the window shows.
fn shelf(wares: &Wares, page: Page) -> &stock::Shelf {
    let index = Page::ALL.iter().position(|&p| p == page).unwrap_or(0);
    &wares.0.shelves[index]
}

/// Lays the page's items' models out while the window shows.
fn lay_out(
    mut commands: Commands,
    (merchant, showing, wares): (Option<Res<Merchant>>, Res<Showing>, Res<Wares>),
    (tables, assets): (Option<Res<Tables>>, Res<AssetServer>),
    mut models: Query<(Entity, &Model, &mut Transform)>,
    added: Query<(), Added<Tint>>,
) {
    if merchant.is_none() || !(wares.is_changed() || showing.is_changed() || !added.is_empty()) {
        return;
    }
    let Some(tables) = tables else {
        return;
    };
    let shelf = shelf(&wares, showing.0);
    let laid = models
        .iter_mut()
        .map(|(entity, model, transform)| (entity, model.0, transform))
        .collect();
    backpack::lay_models(
        &mut commands,
        (&tables, &assets),
        (&shelf.items, &shelf.grid),
        laid,
        (ITEMS, |key| (Scene, Model(key))),
    );
}

/// What an item of the stock costs the hero (`0x5DEE60(item, 1)`): its price as bought by its
/// level, its trading and its charisma.
pub(crate) fn cost(tables: &Tables, dropped: &Dropped, hero: (u32, u32, f32)) -> i64 {
    let (level, trading, charisma) = hero;
    opensacred_rules::trade::shown(
        tables.trade_price(dropped),
        level,
        trading,
        f64::from(charisma),
        true,
        1.0,
    )
}

/// The hero's level, trading and charisma, by which the trade prices.
pub(crate) fn trader(
    experience: &crate::fight::Experience,
    arts: Option<&crate::arts::HeroArts>,
    combat: Option<&crate::fight::Combat>,
) -> (u32, u32, f32) {
    let trading = arts
        .and_then(|arts| {
            arts.skills
                .iter()
                .find(|&&(skill, _)| skill == opensacred_rules::trade::TRADING)
        })
        .map_or(0, |&(_, level)| level);
    let charisma = combat.map_or(0.0, |combat| combat.0.attributes[5]);
    (experience.0.level, trading, charisma)
}

/// Tints the page's cells: an item's blue, red where the hero may not use it, yellow where it
/// costs more than the hero's gold.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn tint(
    (merchant, showing, wares, pack): (Option<Res<Merchant>>, Res<Showing>, Res<Wares>, Res<Pack>),
    (hero, experience, rules, tables, arts): (
        Option<Res<crate::hero::Hero>>,
        Res<crate::fight::Experience>,
        Res<crate::fight::Rules>,
        Option<Res<Tables>>,
        Option<Res<crate::arts::HeroArts>>,
    ),
    combats: Query<&crate::fight::Combat, With<crate::hero::Body>>,
    mut tints: Query<(&Tint, &mut BackgroundColor)>,
) {
    if merchant.is_none() {
        return;
    }
    let shelf = shelf(&wares, showing.0);
    let buyer = trader(&experience, arts.as_deref(), combats.iter().next());
    for (tint, mut colour) in &mut tints {
        let wanted = shelf
            .grid
            .at(tint.0)
            .and_then(|placed| shelf.items.iter().find(|(k, _)| *k == placed.key))
            .map_or(Color::NONE, |(_, dropped)| {
                let ours = (hero.as_deref(), &*experience, &*rules, tables.as_deref());
                if backpack::refuses(ours, dropped) {
                    backpack::REFUSED_TINT
                } else if tables
                    .as_deref()
                    .is_some_and(|t| cost(t, dropped, buyer) > i64::from(pack.gold))
                {
                    DEAR_TINT
                } else {
                    backpack::ITEM_TINT
                }
            });
        colour.set_if_neq(BackgroundColor(wanted));
    }
}

/// A left click on a tab shows its page (`cUI_TabSheet`, `0x729980`).
fn select(
    buttons: Res<ButtonInput<MouseButton>>,
    (merchant, mut showing): (Option<Res<Merchant>>, ResMut<Showing>),
    (pointer, windows): (Res<Pointer>, Query<&Window>),
    tabs: Query<(&Tab, &ComputedNode, &UiGlobalTransform)>,
) {
    if merchant.is_none() || !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let (Some(at), Ok(window)) = (pointer.0, windows.single()) else {
        return;
    };
    let point = at * window.scale_factor();
    if let Some(page) = tabs
        .iter()
        .find(|(_, node, transform)| node.contains_point(**transform, point))
        .map(|(tab, ..)| tab.0)
    {
        showing.set_if_neq(Showing(page));
    }
}

/// The item of the page that the window shows under the pointer, with where its grid starts:
/// its key, its place and its item, for the tooltip.
pub(crate) fn hovered(
    (merchant, showing, wares): (Option<&Merchant>, &Showing, &Wares),
    (pointer, ui_scale, windows): (&Pointer, &UiScale, &Query<&Window>),
) -> Option<(opensacred_rules::inventory::Placed, Dropped)> {
    merchant?;
    let at = screen_point(pointer, ui_scale, windows)? - GRID;
    let cell = backpack::cell_in(at, (COLUMNS, ROWS))?;
    let shelf = shelf(wares, showing.0);
    let placed = *shelf.grid.at(cell)?;
    let dropped = shelf.items.iter().find(|(k, _)| *k == placed.key)?.1;
    Some((placed, dropped))
}

/// The merchant's window.
pub struct MerchantPlugin;

impl Plugin for MerchantPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Wares>()
            .init_resource::<Showing>()
            .add_systems(OnEnter(AppState::InGame), |mut commands: Commands| {
                commands.insert_resource(Wares::default());
            })
            .add_systems(
                Update,
                (visit, follow, show, select, lay_out, tint)
                    .chain()
                    .after(crate::inventory::Opened)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
