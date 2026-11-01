//! The blacksmith's window (#1662; docs/re/blacksmith.md): a creature of `+0x200` bit `0x1000`
//! (`quests::SMITH`) without a dialogue shows, as the hero speaks to it (`talk`, `0x52AB70`), the
//! inventory and the smith's window (`cUI_Blacksmith`, `UI_WND_BLACKSMITH`) at the screen's top
//! left, 640 × 352 (`0x6A4F30`, `0x6A33A0`): its panel tinted `0x9F000000` within its frame, the
//! text `UI_BLACKSMITH_MAKE`, a grid of 3 × 5 cells over the image 169 that takes one weapon,
//! shield or piece of armour, the smith's portrait, the three buttons of his upgrades (259, 260
//! and 261), the item's first four sockets (249 empty, 250 filled, the arrows 251 between them)
//! with the items in them or waiting for them as their models, an upgrade's 1.9 times its size,
//! the work's button (258, 256 lit) and the price under it, red where the gold falls short.
//!
//! Its presses (`0x6A4380`): an item of the cursor that the smith works goes into the grid at the
//! cell under it, the item there before onto the cursor; the grid's item onto the empty cursor;
//! either way what waited for the sockets goes back into the backpack, an upgrade lost (`0x6A3080`).
//! A button puts an upgrade of the smith's tier onto a cursor that holds nothing or another upgrade
//! (`opensacred_rules::smith::upgrade`), which no grid takes and a press on the window's ground
//! drops. A press on an empty socket swaps what waits there with the cursor's item where it may go
//! into a socket; a press on a filled one with the cursor empty takes its item out onto the cursor,
//! every socket emptied and what waited forgotten (`0x6A27B0`, `smith::take_out`). The work's
//! button, where something waits and the hero has the price, takes the gold and puts each waiting
//! item into the next socket by the smith's tier (`0x6A2660`, `smith::fill`). As the inventory
//! hides, the grid's item and what waits go back into the backpack, those without room dropped at
//! the smith's feet (`0x6A3080`).
//!
//! Not followed yet: the Dwarf smithing by his skill 33 (his button of the inventory, the third
//! button's picture 1606 and his checks), the item's check `0xE`, the window's sounds but its show's
//! (#1722), the tooltips of the item, the upgrades and the sockets' items and the waiting item's lines beside
//! the grid, the right button's press that works too, and a network game's events.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use opensacred_rules::drops::{Dropped, Kind};
use opensacred_rules::inventory::{Grid, held_at};
use opensacred_rules::smith::{self, Button, Smith, Waiting};

use crate::backpack;
use crate::bar::{self, UiImages};
use crate::choice::Layer;
use crate::drops::Tables;
use crate::game::{AppState, FONT};
use crate::hud::Pointer;
use crate::inventory::{Shown, screen_point};
use crate::lines::Texts;
use crate::pickup::{Held, Pack};
use crate::tooltip::{Rest, TipRoot, argb, spawn_tip};

/// The window on the original's screen, whose presses do not reach the world.
pub(crate) const WINDOW: Rect = Rect {
    min: Vec2::ZERO,
    max: Vec2::new(640.0, 352.0),
};
/// The panel (`UI_STATIC_BG`, from 16 to 28 above the window's bottom) and its tint.
const PANEL: Rect = Rect {
    min: Vec2::new(0.0, 16.0),
    max: Vec2::new(640.0, 340.0),
};
const PANEL_TINT: Color = Color::srgba(0.0, 0.0, 0.0, 159.0 / 255.0);
/// The frame (`0x6A4F30`): ten pieces of the image 34 down the divider, each at its third anchor
/// right of 502, from 22; twenty along the top at 32 apart, the first 54, the last 55 and the rest
/// 21, each at its second anchor above 18; twenty of the image 14 along the bottom at 340; and ten
/// down each side from 20, the image 34 and the last 56 on the left at its third anchor less one,
/// 34 and the last 57 on the right at its third anchor from the window's right edge.
const EDGE: u32 = 34;
const DIVIDER_AT: f32 = 502.0;
const DIVIDER_FROM: f32 = 22.0;
const TOP: (u32, u32, u32) = (54, 21, 55);
const TOP_AT: f32 = 18.0;
const BOTTOM: u32 = 14;
const BOTTOM_AT: f32 = 340.0;
const SIDES_LAST: (u32, u32) = (56, 57);
const SIDES_FROM: f32 = 20.0;
const PIECES: u8 = 10;
const ALONG: u8 = 20;
const ALONG_APART: f32 = 32.0;
/// The title (`UI_BLACKSMITH_MAKE` in the font 1), in its rectangle (**seen**: its letters from
/// 37 to 53 down).
const MAKE: &str = "UI_BLACKSMITH_MAKE";
const TITLE: Rect = Rect {
    min: Vec2::new(16.0, 32.0),
    max: Vec2::new(272.0, 56.0),
};
const FONT_SIZE: f32 = 20.0;
const WHITE: u32 = 0xFFFF_FFFF;
/// The grid (the inventory entry `0x5DEC90`): its corner, its columns and rows, and its picture.
const GRID: Vec2 = Vec2::new(16.0, 64.0);
const COLUMNS: u8 = 3;
const ROWS: u8 = 5;
const GRID_IMAGE: u32 = 169;
/// The smith's portrait by his type, else the image 839.
const PORTRAIT: Vec2 = Vec2::new(524.0, 42.0);
const PORTRAIT_DEFAULT: u32 = 839;
/// The three buttons, each with the picture of the upgrade it gives.
const BUTTONS: [(Button, Vec2, u32); 3] = [
    (Button::Grinding, Vec2::new(512.0, 192.0), 259),
    (Button::Hardening, Vec2::new(576.0, 192.0), 260),
    (Button::Refining, Vec2::new(544.0, 256.0), 261),
];
/// The work's button (258, 256 lit) and the price under it (`0xFFFFFF80`, `0xFFFF0020` short).
const WORK: Vec2 = Vec2::new(432.0, 232.0);
const WORK_DARK: u32 = 258;
const WORK_LIT: u32 = 256;
const PRICE: Rect = Rect {
    min: Vec2::new(429.0, 296.0),
    max: Vec2::new(493.0, 320.0),
};
const YELLOW: u32 = 0xFFFF_FF80;
const RED: u32 = 0xFFFF_0020;
const SMALL: f32 = 17.0;
/// The first four sockets, 88 apart from (16, 232), each 249 empty or 250 filled, the arrows 251
/// between them, and an item in one drawn at its centre, 32 along and down.
const SOCKETS: usize = 4;
const SOCKETS_FROM: Vec2 = Vec2::new(16.0, 232.0);
const SOCKET_APART: f32 = 88.0;
const EMPTY: u32 = 249;
const FILLED: u32 = 250;
const ARROW: u32 = 251;
const ARROWS_FROM: Vec2 = Vec2::new(88.0, 256.0);
const SOCKET_CENTRE: Vec2 = Vec2::new(32.0, 32.0);
/// An upgrade's model in a socket, times its size (the float `0x3FF33333`).
const UPGRADE_SCALE: f32 = 1.9;
/// The hints (the style `0x209`): the empty grid's (`0x26DC`), an empty socket's (`0x26DD`) and
/// the work's button's (`0x26D8`), each at its point from its control's corner.
const TIP_GRID: (&str, Vec2) = ("9948", Vec2::new(256.0, 92.0));
const TIP_SOCKET: (&str, Vec2) = ("9949", Vec2::new(48.0, 80.0));
const TIP_WORK: (&str, Vec2) = ("9944", Vec2::new(32.0, 80.0));
/// The render layer of the items' models.
const ITEMS: usize = 18;

/// What waits for an empty socket: an item from the cursor, or an upgrade of a type.
#[derive(Clone, Debug, PartialEq)]
enum Wait {
    Item(Held),
    Upgrade(u32),
}

/// The smith whose window shows, and what it holds: the item in its grid, by its key and place,
/// and what waits for each of the item's first four sockets.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Blacksmith {
    pub creature: Entity,
    /// Its creature type, whose portrait the window shows (the event `0x30` (8, id)).
    kind: u32,
    /// Its tier (`0x6A2230`), 0 to 5.
    tier: u8,
    items: Vec<(u32, Dropped)>,
    grid: Grid,
    waiting: [Option<Wait>; SOCKETS],
    /// What went back from the sockets and found no room in the backpack, to drop at the smith's
    /// feet (`0x6A3080`).
    fallen: Vec<Dropped>,
}

impl Blacksmith {
    /// The item in the grid.
    fn item(&self) -> Option<&Dropped> {
        self.items.first().map(|(_, dropped)| dropped)
    }

    /// The smith, a creature of its type.
    fn smith(&self) -> Smith {
        Smith::Creature { kind: self.kind }
    }

    /// Whether anything waits for the sockets (`0x6A2DF0`).
    fn waits(&self) -> bool {
        self.waiting.iter().any(Option::is_some)
    }

    /// The price of putting what waits into the item's sockets (`0x6A2B20`), none where nothing
    /// waits or the grid is empty.
    fn price(&self, tables: &Tables) -> Option<i64> {
        let item = self.item().filter(|_| self.waits())?;
        let waiting: Vec<Waiting> = self
            .waiting
            .iter()
            .flatten()
            .map(|wait| match wait {
                Wait::Item(held) if tables.category(held.dropped.item) == RUNES => Waiting::Rune,
                Wait::Item(held) => Waiting::Worth(tables.worth(&held.dropped)),
                Wait::Upgrade(kind) => Waiting::Worth(tables.price(*kind)),
            })
            .collect();
        Some(smith::price(
            self.smith(),
            tables.worth(item),
            tables.sockets(item),
            &waiting,
        ))
    }

    /// The sockets the window shows of the item, its first four (`0x5BE9C0`).
    fn shown(&self, tables: &Tables) -> usize {
        self.item()
            .map_or(0, |item| tables.sockets(item).min(SOCKETS))
    }

    /// Whether the work's button lights (`0x6A33A0`): the item has a socket, something waits and
    /// the hero has its price.
    fn lit(&self, tables: &Tables, gold: u32) -> bool {
        self.shown(tables) > 0
            && self
                .price(tables)
                .is_some_and(|price| price <= i64::from(gold))
    }

    /// Takes what waits for the sockets away: the items, the upgrades lost.
    fn forget(&mut self) -> Vec<Held> {
        self.waiting
            .iter_mut()
            .filter_map(Option::take)
            .filter_map(|wait| match wait {
                Wait::Item(held) => Some(held),
                Wait::Upgrade(_) => None,
            })
            .collect()
    }
}

/// The runes' category, which counts 2,000 in the price as it waits.
const RUNES: u8 = 28;

/// An upgrade of the type `kind` as an item, which the window makes as it goes into a socket.
fn upgrade_item(kind: u32) -> Dropped {
    Dropped {
        kind: Kind::Other,
        item: kind,
        level: 0,
        key: 16,
        quality: 0,
        amount: 1,
        values: None,
        named: None,
        socketed: Vec::new(),
    }
}

/// Whether the cursor holds an upgrade (the cursor's kind 5), which only the smith's sockets take.
pub(crate) fn holds_upgrade(pack: &Pack, tables: &Tables) -> bool {
    pack.held
        .as_ref()
        .is_some_and(|held| tables.category(held.dropped.item) == smith::UPGRADES)
}

/// The smith's sound (7108), which the window plays as it shows.
const SHOW_SOUND: u32 = 7108;

/// Shows the smith's window for `creature` with the inventory (`0x52AB70`), his tier by his type
/// and the difficulty, with its sound.
pub fn open(world: &mut World, creature: Entity) {
    let kind = world
        .get::<crate::ai::Mind>(creature)
        .map_or(0, |mind| mind.creature);
    let difficulty = world
        .get_resource::<crate::game::Difficulty>()
        .map(|d| d.0)
        .unwrap_or_default();
    let tier = Smith::Creature { kind }.tier(u8::try_from(difficulty.index()).unwrap_or(0));
    info!("blacksmith: {creature}, a {kind}, shows its window, of the tier {tier}");
    // The smith's sound as the window shows (`0x6A41F0`: the event (14, 2, `0x1E`), #1722).
    world.write_message(crate::sfx::Play {
        sound: crate::sfx::Of::Id(SHOW_SOUND),
        at: None,
        rate: (0.5, crate::sfx::Roll::None),
    });
    world.insert_resource(Blacksmith {
        creature,
        kind,
        tier,
        items: Vec::new(),
        grid: Grid::new(COLUMNS, ROWS),
        waiting: [const { None }; SOCKETS],
        fallen: Vec::new(),
    });
    if let Some(mut shown) = world.get_resource_mut::<Shown>()
        && !shown.0
    {
        shown.0 = true;
    }
}

/// Puts `items` into the backpack at their first fits, giving back those without room.
fn into_backpack(pack: &mut Pack, items: Vec<Held>) -> Vec<Dropped> {
    items
        .into_iter()
        .filter_map(|held| {
            let size = held.size;
            (!pack.take(held.dropped.clone(), size)).then_some(held.dropped)
        })
        .collect()
}

/// Takes the window away with the inventory (`0x6A3080`): the grid's item and what waits back
/// into the backpack, an upgrade lost and the cursor's too, those without room dropped at the
/// smith's feet.
fn follow(
    mut commands: Commands,
    shown: Res<Shown>,
    smith: Option<ResMut<Blacksmith>>,
    mut pack: ResMut<Pack>,
    (tables, assets, mut dice): (
        Option<Res<Tables>>,
        Res<AssetServer>,
        ResMut<crate::drops::Dice>,
    ),
    (creatures, heroes): (
        Query<&Transform>,
        Query<&crate::hero::Body, Without<crate::npcs::Dies>>,
    ),
) {
    let (Some(mut smith), Some(tables)) = (smith, tables) else {
        return;
    };
    if !shown.0 {
        let smith = &mut *smith;
        let mut back: Vec<Held> = std::mem::take(&mut smith.items)
            .into_iter()
            .map(|(key, dropped)| {
                let size = smith.grid.remove(key).map_or((1, 1), |placed| placed.size);
                Held { key, dropped, size }
            })
            .collect();
        back.extend(smith.forget());
        if holds_upgrade(&pack, &tables) {
            pack.held = None;
        }
        let left = into_backpack(&mut pack, back);
        smith.fallen.extend(left);
        commands.remove_resource::<Blacksmith>();
    }
    if smith.fallen.is_empty() {
        return;
    }
    let fallen = std::mem::take(&mut smith.fallen);
    let (Ok(at), Some(body)) = (creatures.get(smith.creature), heroes.iter().next()) else {
        return;
    };
    let cell = crate::paths::cell_of(at.translation.x, at.translation.z);
    for dropped in fallen {
        info!("blacksmith: no room for {}, dropped", dropped.item);
        crate::drops::drop_at(
            &mut commands,
            (&assets, &tables, &mut dice),
            (cell, body.level()),
            dropped,
        );
    }
}

/// The corner of the socket `k`.
fn socket_at(k: usize) -> Vec2 {
    SOCKETS_FROM + Vec2::new(SOCKET_APART * f32::from(u8::try_from(k).unwrap_or(0)), 0.0)
}

/// The rectangle of a control of the image `index` at `at`.
fn control(images: &UiImages, index: u32, at: Vec2) -> Rect {
    Rect::from_corners(at, at + images.size(index))
}

/// The socket of the `shown` under the point `at`.
fn socket_under(images: &UiImages, at: Vec2, shown: usize) -> Option<usize> {
    (0..shown).find(|&k| control(images, EMPTY, socket_at(k)).contains(at))
}

/// The presses on the window (`0x6A4380`), in its order: the grid, the sockets, the buttons, the
/// work's button, the window's ground.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn press(
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    (smith, mut pack, tables, images): (
        Option<ResMut<Blacksmith>>,
        ResMut<Pack>,
        Option<Res<Tables>>,
        Res<UiImages>,
    ),
    asking: Option<Res<crate::purchase::Asking>>,
) {
    let (Some(mut smith), Some(tables)) = (smith, tables) else {
        return;
    };
    if asking.is_some() || !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(at) = screen_point(&pointer, &ui_scale, &windows).filter(|&at| WINDOW.contains(at))
    else {
        return;
    };
    let smith = &mut *smith;
    if let Some(cell) = backpack::cell_in(at - GRID, (COLUMNS, ROWS)) {
        press_grid(smith, &mut pack, &tables, (at - GRID, cell));
        return;
    }
    if let Some(k) = socket_under(&images, at, smith.shown(&tables)) {
        press_socket(smith, &mut pack, &tables, k);
        return;
    }
    if let Some(&(button, ..)) = BUTTONS
        .iter()
        .find(|(_, corner, index)| control(&images, *index, *corner).contains(at))
    {
        // An upgrade of the smith's tier onto a cursor that holds nothing or another upgrade.
        if holds_upgrade(&pack, &tables) {
            pack.held = None;
        }
        let kind = smith::upgrade(button, smith.tier, smith.smith());
        if pack.hold(upgrade_item(kind), tables.size(kind)) {
            debug!("blacksmith: the upgrade {kind} onto the cursor");
        }
        return;
    }
    if control(&images, WORK_DARK, WORK).contains(at) {
        if smith
            .price(&tables)
            .is_some_and(|price| price <= i64::from(pack.gold))
        {
            work(smith, &mut pack, &tables);
        }
    } else if holds_upgrade(&pack, &tables) {
        // A press on the window's ground drops the upgrade off the cursor.
        pack.held = None;
    }
}

/// A press on the grid at the point `at` of its cell `cell`: the cursor's item that the smith
/// works into the grid where it fits at the cell, the item there before onto the cursor; else the
/// grid's item onto the empty cursor; either way what waited goes back into the backpack.
fn press_grid(
    smith: &mut Blacksmith,
    pack: &mut Pack,
    tables: &Tables,
    (at, cell): (Vec2, (u8, u8)),
) {
    let Some(held) = pack.held.clone() else {
        if smith.grid.at(cell).is_none() {
            return;
        }
        if let Some((key, dropped)) = smith.items.pop() {
            let size = smith.grid.remove(key).map_or((1, 1), |placed| placed.size);
            debug!("blacksmith: {} out of the grid", dropped.item);
            pack.held = Some(Held { key, dropped, size });
            let waited = smith.forget();
            let left = into_backpack(pack, waited);
            smith.fallen.extend(left);
        }
        return;
    };
    if !smith::worked(tables.category(held.dropped.item)) {
        return;
    }
    let (column, row) = held_at((at.x, at.y), held.size);
    let (Ok(column), Ok(row)) = (u8::try_from(column), u8::try_from(row)) else {
        return;
    };
    let mut grid = Grid::new(COLUMNS, ROWS);
    if !grid.place(held.key, (column, row), held.size) {
        return;
    }
    let old = smith.items.pop().map(|(key, dropped)| {
        let size = smith.grid.remove(key).map_or((1, 1), |placed| placed.size);
        Held { key, dropped, size }
    });
    let waited = smith.forget();
    let left = into_backpack(pack, waited);
    smith.fallen.extend(left);
    info!(
        "blacksmith: {} into the grid, of {} sockets",
        held.dropped.item,
        tables.sockets(&held.dropped)
    );
    smith.grid = grid;
    smith.items.push((held.key, held.dropped));
    pack.held = old;
}

/// A press on the socket `k`: a filled one gives its item back onto the empty cursor, every
/// socket emptied and what waited forgotten (`0x6A27B0`); an empty one swaps what waits there with
/// the cursor's item where it may go into a socket.
fn press_socket(smith: &mut Blacksmith, pack: &mut Pack, tables: &Tables, k: usize) {
    let filled = smith.item().map_or(0, |item| item.socketed.len());
    if k < filled {
        if pack.held.is_some() {
            return;
        }
        let Some((_, item)) = smith.items.first_mut() else {
            return;
        };
        let taken = smith::take_out(item, k);
        smith.waiting = [const { None }; SOCKETS];
        info!(
            "blacksmith: the sockets emptied, {:?} back",
            taken.as_ref().map(|d| d.item)
        );
        if let Some(taken) = taken.filter(|d| tables.category(d.item) != smith::UPGRADES) {
            let size = tables.size(taken.item);
            pack.hold(taken, size);
        }
        return;
    }
    let into = match pack.held.clone() {
        Some(held) => {
            let category = tables.category(held.dropped.item);
            if !smith::socketable(held.dropped.item, category) {
                return;
            }
            Some(if category == smith::UPGRADES {
                Wait::Upgrade(held.dropped.item)
            } else {
                Wait::Item(held)
            })
        }
        None => None,
    };
    let out = std::mem::replace(&mut smith.waiting[k], into);
    pack.held = None;
    match out {
        Some(Wait::Item(held)) => pack.held = Some(held),
        Some(Wait::Upgrade(kind)) => {
            pack.hold(upgrade_item(kind), tables.size(kind));
        }
        None => {}
    }
}

/// The work (`0x6A2660`): the price taken, each waiting item into the item's next socket by the
/// smith's tier.
fn work(smith: &mut Blacksmith, pack: &mut Pack, tables: &Tables) {
    let Some(price) = smith.price(tables) else {
        return;
    };
    let waiting: Vec<Dropped> = smith
        .waiting
        .iter_mut()
        .filter_map(Option::take)
        .map(|wait| match wait {
            Wait::Item(held) => held.dropped,
            Wait::Upgrade(kind) => upgrade_item(kind),
        })
        .collect();
    let tier = smith.tier;
    let Some((_, item)) = smith.items.first_mut() else {
        return;
    };
    pack.gold = u32::try_from(i64::from(pack.gold) - price).unwrap_or(0);
    for socket in waiting {
        if let Some(refused) = tables.fill(item, socket, tier) {
            info!("blacksmith: {} refused by the sockets", refused.item);
        }
    }
    info!(
        "blacksmith: {} worked for {price} gold, {} left, its sockets {:?}",
        item.item,
        pack.gold,
        item.socketed
            .iter()
            .map(|socket| socket.item.item)
            .collect::<Vec<_>>()
    );
}

/// The window's container.
#[derive(Component)]
struct Root;

/// The items' camera, light and models.
#[derive(Component, Clone)]
struct Scene;

/// A grid cell's tint, at its column and row.
#[derive(Component)]
struct Tint((u8, u8));

/// What the window shows, made again as it changes.
#[derive(Clone, Debug, PartialEq)]
struct Look {
    smith: Blacksmith,
    gold: u32,
}

/// The images and places of the window's frame (`0x6A4F30`), by `anchors` and `size` of an image.
fn frame(anchors: impl Fn(u32) -> [f32; 4], size: impl Fn(u32) -> Vec2) -> Vec<(u32, Vec2)> {
    let mut pieces = Vec::new();
    let mut y = DIVIDER_FROM;
    for _ in 0..PIECES {
        pieces.push((EDGE, Vec2::new(DIVIDER_AT + anchors(EDGE)[2], y)));
        y += size(EDGE).y;
    }
    let mut x = 0.0;
    for k in 0..ALONG {
        let piece = match k {
            0 => TOP.0,
            k if k + 1 == ALONG => TOP.2,
            _ => TOP.1,
        };
        pieces.push((piece, Vec2::new(x, TOP_AT - anchors(piece)[1])));
        pieces.push((BOTTOM, Vec2::new(x, BOTTOM_AT - anchors(BOTTOM)[1])));
        x += ALONG_APART;
    }
    let mut y = SIDES_FROM;
    for k in 0..PIECES {
        let (left, right) = if k + 1 == PIECES {
            SIDES_LAST
        } else {
            (EDGE, EDGE)
        };
        pieces.push((left, Vec2::new(anchors(left)[2] - 1.0, y)));
        pieces.push((right, Vec2::new(WINDOW.width() - anchors(right)[2], y)));
        y += size(right).y;
    }
    pieces
}

/// Makes the window as the smith shows it and as it changes, and takes it away as it goes.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn show(
    mut commands: Commands,
    (smith, pack, tables): (Option<Res<Blacksmith>>, Res<Pack>, Option<Res<Tables>>),
    (images, assets, texts): (Res<UiImages>, Res<AssetServer>, Option<Res<Texts>>),
    portraits: Option<Res<crate::talk::Portraits>>,
    (mut targets, mut layers): (ResMut<Assets<Image>>, ResMut<Assets<Layer>>),
    window: Option<Single<&Window>>,
    olds: Query<Entity, Or<(With<Root>, With<Scene>)>>,
    mut was: Local<Option<Look>>,
) {
    let now = smith.as_deref().map(|smith| Look {
        smith: smith.clone(),
        gold: pack.gold,
    });
    if *was == now {
        return;
    }
    *was = now.clone();
    for old in &olds {
        commands.entity(old).despawn();
    }
    let (Some(look), Some(tables)) = (now, tables) else {
        return;
    };
    let smith = &look.smith;
    let font: Handle<Font> = assets.load(FONT);
    let root = commands
        .spawn((Root, backpack::screen(), GlobalZIndex(-1)))
        .id();
    let panel = commands
        .spawn((
            bar::placed(PANEL.min, PANEL.size()),
            BackgroundColor(PANEL_TINT),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(root).add_child(panel);
    let add = |commands: &mut Commands, index: u32, at: Vec2| {
        let piece = commands
            .spawn((
                bar::piece(&images, &assets, index, at, Vec2::ZERO),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(root).add_child(piece);
    };
    for (index, at) in frame(|i| images.anchors(i), |i| images.size(i)) {
        add(&mut commands, index, at);
    }
    // A line of text in its rectangle, centred down it.
    let write = |commands: &mut Commands,
                 (text, rect): (&str, Rect),
                 (colour, size, justify): (u32, f32, JustifyContent)| {
        let mut node = bar::placed(rect.min, rect.size());
        node.justify_content = justify;
        node.align_items = AlignItems::Center;
        let child = commands
            .spawn((node, Pickable::IGNORE))
            .with_child((
                Text::new(text),
                TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(size),
                    ..default()
                },
                TextLayout {
                    linebreak: LineBreak::NoWrap,
                    ..default()
                },
                TextColor(argb(colour)),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(root).add_child(child);
    };
    // The title in its text's own colour.
    let title = texts
        .as_deref()
        .and_then(|texts| texts.named(MAKE))
        .unwrap_or(MAKE);
    let (title, colour) = crate::master::tagged(title);
    write(
        &mut commands,
        (title, TITLE),
        (
            colour.unwrap_or(WHITE),
            FONT_SIZE,
            JustifyContent::FlexStart,
        ),
    );
    // The smith's portrait by his type, else the default.
    match portraits.as_deref().and_then(|p| p.of(smith.kind)) {
        Some((file, rect)) => {
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
        None => add(&mut commands, PORTRAIT_DEFAULT, PORTRAIT),
    }
    for (_, at, index) in BUTTONS {
        add(&mut commands, index, at);
    }
    // The grid's picture and its cells' tints.
    add(&mut commands, GRID_IMAGE, GRID);
    for column in 0..COLUMNS {
        for row in 0..ROWS {
            let at = GRID + Vec2::new(f32::from(column), f32::from(row)) * backpack::CELL;
            let cell = commands
                .spawn((
                    bar::placed(at, Vec2::splat(backpack::CELL)),
                    BackgroundColor(Color::NONE),
                    Tint((column, row)),
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(root).add_child(cell);
        }
    }
    // The sockets, the arrows between them, the work's button and the price.
    let shown = smith.shown(&tables);
    let filled = smith.item().map_or(0, |item| item.socketed.len());
    for k in 0..shown {
        add(
            &mut commands,
            if k < filled { FILLED } else { EMPTY },
            socket_at(k),
        );
        if k + 1 < shown {
            let arrow = ARROWS_FROM
                + Vec2::new(SOCKET_APART * f32::from(u8::try_from(k).unwrap_or(0)), 0.0);
            add(&mut commands, ARROW, arrow);
        }
    }
    // The work's button with an item in the grid alone.
    if smith.item().is_some() {
        let lit = smith.lit(&tables, look.gold);
        add(&mut commands, if lit { WORK_LIT } else { WORK_DARK }, WORK);
    }
    if let Some(price) = smith.price(&tables) {
        let colour = if price <= i64::from(look.gold) {
            YELLOW
        } else {
            RED
        };
        write(
            &mut commands,
            (&price.to_string(), PRICE),
            (colour, SMALL, JustifyContent::Center),
        );
    }
    // The items' picture over the whole window.
    let scale = window.map_or(1.0, |window| window.height() / bar::SCREEN.y);
    let picture = commands
        .spawn((bar::placed(Vec2::ZERO, WINDOW.size()), Pickable::IGNORE))
        .id();
    commands.entity(root).add_child(picture);
    backpack::spawn_picture(
        &mut commands,
        (&mut targets, &mut layers),
        scale,
        (picture, WINDOW.size()),
        (ITEMS, Scene),
    );
    // The grid's item and the sockets' items, filled or waiting, as their models.
    let mut models: Vec<(u32, Transform)> = Vec::new();
    for placed in &smith.grid.items {
        let Some((_, dropped)) = smith.items.iter().find(|(key, _)| *key == placed.key) else {
            continue;
        };
        if let Some((.., pose)) = tables.look(dropped.item) {
            let mut at = backpack::placement(placed.at, placed.size, pose);
            at.translation += Vec3::new(GRID.x, -GRID.y, 0.0);
            models.push((dropped.item, at));
        }
    }
    let socketed = smith
        .item()
        .map(|item| {
            item.socketed
                .iter()
                .map(|s| s.item.item)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for k in 0..shown {
        let kind = socketed.get(k).copied().or(match &smith.waiting[k] {
            Some(Wait::Item(held)) => Some(held.dropped.item),
            Some(Wait::Upgrade(kind)) => Some(*kind),
            None => None,
        });
        let Some(kind) = kind else {
            continue;
        };
        let Some((.., pose)) = tables.look(kind) else {
            continue;
        };
        let centre = socket_at(k) + SOCKET_CENTRE;
        let mut at = backpack::posed(Vec3::new(centre.x, -centre.y, 0.0), pose);
        if tables.category(kind) == smith::UPGRADES {
            at.scale *= UPGRADE_SCALE;
        }
        models.push((kind, at));
    }
    for (kind, at) in models {
        let Some((model, texture, _)) = tables.look(kind) else {
            continue;
        };
        let mut entity = commands.spawn((
            Scene,
            backpack::Matte,
            crate::models::scene(&assets, model),
            at,
            RenderLayers::layer(ITEMS),
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(texture) = texture {
            entity.insert(crate::npcs::Retexture::load(&assets, texture));
        }
    }
}

/// Tints the grid's cells as the backpack's: under an item blue, or red where the hero may not
/// use it.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn tint(
    smith: Option<Res<Blacksmith>>,
    (hero, experience, rules, tables): (
        Option<Res<crate::hero::Hero>>,
        Res<crate::fight::Experience>,
        Res<crate::fight::Rules>,
        Option<Res<Tables>>,
    ),
    mut tints: Query<(&Tint, &mut BackgroundColor)>,
) {
    let Some(smith) = smith else {
        return;
    };
    let refused = |key: u32| {
        smith
            .items
            .iter()
            .find(|(k, _)| *k == key)
            .is_some_and(|(_, dropped)| {
                backpack::refuses(
                    (hero.as_deref(), &experience, &rules, tables.as_deref()),
                    dropped,
                )
            })
    };
    for (tint, mut colour) in &mut tints {
        let wanted = backpack::cell_tint(&smith.grid, None, tint.0, refused);
        colour.set_if_neq(BackgroundColor(wanted));
    }
}

/// The controls' hints (the style `0x209`): the empty grid's, an empty socket's with nothing
/// waiting, and the work's button's.
fn tooltip(
    mut commands: Commands,
    time: Res<Time<Real>>,
    (smith, tables, images): (Option<Res<Blacksmith>>, Option<Res<Tables>>, Res<UiImages>),
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    (assets, texts): (Res<AssetServer>, Option<Res<Texts>>),
    tips: Query<Entity, With<TipRoot>>,
    mut rest: Local<Rest>,
) {
    let at = smith
        .as_deref()
        .and_then(|_| screen_point(&pointer, &ui_scale, &windows));
    // The controls by number: 0 the grid, 1 the work's button, 2 and on the sockets.
    let over = smith
        .as_deref()
        .zip(tables.as_deref())
        .zip(at)
        .and_then(|((smith, tables), at)| {
            if smith.item().is_none() && backpack::cell_in(at - GRID, (COLUMNS, ROWS)).is_some() {
                Some(0)
            } else if control(&images, WORK_DARK, WORK).contains(at) {
                Some(1)
            } else {
                let filled = smith.item().map_or(0, |item| item.socketed.len());
                socket_under(&images, at, smith.shown(tables))
                    .filter(|&k| k >= filled && smith.waiting[k].is_none())
                    .and_then(|k| u32::try_from(k + 2).ok())
            }
        });
    let (gone, due) = rest.step(over, time.elapsed_secs());
    if gone {
        for tip in &tips {
            commands.entity(tip).despawn();
        }
    }
    let Some(control) = due else {
        return;
    };
    let ((key, from), corner) = match control {
        0 => (TIP_GRID, GRID),
        1 => (TIP_WORK, WORK),
        k => (TIP_SOCKET, socket_at(usize::try_from(k - 2).unwrap_or(0))),
    };
    if let Some(text) = texts.as_deref().and_then(|texts| texts.named(key)) {
        spawn_tip(&mut commands, &assets, text, corner + from);
    }
}

/// The picture of the button that gives the upgrade of the type `kind` (`0x6A4380`: the cursor of
/// the kind 5 takes the button's image), none for an upgrade that no button gives.
fn picture_of(kind: u32) -> Option<u32> {
    match kind {
        8000..=8005 => Some(BUTTONS[0].2),
        8020..=8025 => Some(BUTTONS[1].2),
        8030..=8035 => Some(BUTTONS[2].2),
        8042..=8047 => Some(DWARF_REFINING),
        _ => None,
    }
}

/// The Dwarf's refining button's picture (`0xA0DDA8`).
const DWARF_REFINING: u32 = 1606;
/// The cursor's upgrade's picture, its centre on the pointer (the hot spot at −32, −32).
const CURSOR_HOT_SPOT: Vec2 = Vec2::new(32.0, 32.0);

/// The cursor's upgrade's picture.
#[derive(Component)]
struct CursorPicture(u32);

/// Shows an upgrade on the cursor as its button's picture, centred on the pointer, over every
/// window; takes it away as the cursor lets it go.
fn cursor_picture(
    mut commands: Commands,
    (pack, tables, images, assets): (
        Res<Pack>,
        Option<Res<Tables>>,
        Res<UiImages>,
        Res<AssetServer>,
    ),
    (pointer, ui_scale): (Res<Pointer>, Res<UiScale>),
    mut pictures: Query<(Entity, &CursorPicture, &mut Node, &mut Visibility)>,
) {
    let wanted = tables
        .as_deref()
        .filter(|tables| holds_upgrade(&pack, tables))
        .and_then(|_| pack.held.as_ref())
        .and_then(|held| picture_of(held.dropped.item));
    for (entity, picture, mut node, mut visibility) in &mut pictures {
        if Some(picture.0) != wanted {
            commands.entity(entity).despawn();
            continue;
        }
        match pointer.0 {
            Some(at) => {
                let corner = at / ui_scale.0.max(0.01) - CURSOR_HOT_SPOT;
                node.left = Val::Px(corner.x);
                node.top = Val::Px(corner.y);
                visibility.set_if_neq(Visibility::Inherited);
            }
            None => {
                visibility.set_if_neq(Visibility::Hidden);
            }
        }
    }
    let Some(index) = wanted.filter(|&index| pictures.iter().all(|(_, p, ..)| p.0 != index)) else {
        return;
    };
    commands.spawn((
        CursorPicture(index),
        bar::piece(&images, &assets, index, Vec2::ZERO, Vec2::ZERO),
        Visibility::Hidden,
        GlobalZIndex(crate::held::OVER_ALL),
        Pickable::IGNORE,
        DespawnOnExit(AppState::InGame),
    ));
}

/// The blacksmith's window.
pub struct BlacksmithPlugin;

impl Plugin for BlacksmithPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (follow, press, show, tint, tooltip, cursor_picture)
                .chain()
                .after(crate::inventory::Opened)
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(OnExit(AppState::InGame), |mut commands: Commands| {
            commands.remove_resource::<Blacksmith>();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frame_runs_round_the_window_and_down_its_divider() {
        let pieces = frame(|_| [0.0, 4.0, 8.0, 0.0], |_| Vec2::new(32.0, 32.0));
        assert_eq!(pieces.len(), 10 + 40 + 20);
        // The divider of 34 from 22 down at 502 and its third anchor.
        assert_eq!(pieces[0], (34, Vec2::new(510.0, 22.0)));
        assert_eq!(pieces[9], (34, Vec2::new(510.0, 310.0)));
        // The top's first 54 and the bottom's 14 at 32 apart, the top's last 55.
        assert_eq!(pieces[10], (54, Vec2::new(0.0, 14.0)));
        assert_eq!(pieces[11], (14, Vec2::new(0.0, 336.0)));
        assert_eq!(pieces[12], (21, Vec2::new(32.0, 14.0)));
        assert_eq!(pieces[48], (55, Vec2::new(608.0, 14.0)));
        // The sides from 20, the last 56 and 57.
        assert_eq!(pieces[50], (34, Vec2::new(7.0, 20.0)));
        assert_eq!(pieces[51], (34, Vec2::new(632.0, 20.0)));
        assert_eq!(pieces[68], (56, Vec2::new(7.0, 308.0)));
        assert_eq!(pieces[69], (57, Vec2::new(632.0, 308.0)));
    }

    #[test]
    fn the_sockets_stand_88_apart() {
        assert_eq!(socket_at(0), Vec2::new(16.0, 232.0));
        assert_eq!(socket_at(3), Vec2::new(280.0, 232.0));
    }
}
