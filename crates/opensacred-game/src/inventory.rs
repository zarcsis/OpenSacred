//! The inventory's windows (#1267; docs/re/inventory.md, §1, §2 and §4): the key `I` and the
//! bar's inventory button (`UI_TB_INVENTORY`) open the inventory (`cUI_Inventory3`, at (0, 388),
//! 640 × 256) and the equipment beside it (`cUI_Equipment`, at (656, 388), 256 × 256) together
//! (`0x6C5A50`), with `UI_INV_OPEN01`, and close them with `UI_INV_CLOSE01`; laid out on the
//! original's 1024 × 768 screen centred along the window's width, as the bar is (`bar`).
//!
//! The inventory's frame is the original's (`0x6C6490`): a top and a bottom edge of ten steps, the
//! first and the last pieces their own and the others at random, each standing on the window's
//! edge by its anchor, and a right edge of nine pieces; its three tabs above it, the items' first,
//! and its sort's button, which sorts the backpack (#1321). The equipment's ornament at its
//! alpha, framed as the original frames it
//! (`0x6BAC80`). Clicks over the windows do not reach the world.
//!
//! Not followed yet: the grid's cells and the items (#1268), the slots (#1270), the tabs' pages and
//! their buttons' pressed looks (the spells' and the combos' pages are #1090's), the sort's
//! button's other mode (#1321), and the camera's shift as the equipment shows. The statistics
//! window beside them is `statistics`' (#1521).

use bevy::input::InputSystems;
use bevy::prelude::*;
use opensacred_rules::spawning::Rng;

use crate::bar::{self, UiImages};
use crate::frame::HoldsControl;
use crate::game::{AppState, FONT};
use crate::hud::Pointer;
use crate::lines::Texts;
use crate::sfx;

/// The inventory's window and the equipment's on the original's screen (`0x6C5930`,
/// `0x6B6410`).
const INVENTORY: Rect = Rect {
    min: Vec2::new(0.0, 388.0),
    max: Vec2::new(640.0, 644.0),
};
pub(crate) const EQUIPMENT: Rect = Rect {
    min: Vec2::new(656.0, 388.0),
    max: Vec2::new(912.0, 644.0),
};
/// The sounds of the window shown and hidden (`(14, 2, 1)`, `UI_INV_OPEN01`, `UI_INV_CLOSE01`).
pub(crate) const OPENS: u32 = 7111;
pub(crate) const CLOSES: u32 = 7112;

/// The inventory's frame (`0x6C6490`, all of `GUI_main_03`): the top edge's first and last
/// pieces and the range of the others, the bottom edge's, and the right edge's, which starts 4
/// down and stands at its pieces' second anchors.
const TOP: (u32, u32, (u32, u32)) = (5, 12, (22, 5));
const BOTTOM: (u32, u32, (u32, u32)) = (3, 13, (15, 5));
const STEPS: usize = 10;
const RIGHT: (u32, u32, (u32, u32)) = (32, 33, (28, 3));
const RIGHT_PIECES: usize = 9;
const RIGHT_FROM: f32 = 4.0;
/// The tabs (`cUI_TabSheet`, `0x729980`): the image behind each, its bar over its two clasps,
/// stretched across the tab; the texts of the items', the spells' and the combos' pages (1155 to
/// 1157); their least width and the room about a text; their top, −20 less the font's height, and
/// the bar's height, which the text stands in the middle of; the gap between two and the text's
/// size (**estimate**, by the original's frames of #1251).
const TAB: u32 = 108;
const TAB_TEXTS: [&str; 3] = ["1155", "1156", "1157"];
const TAB_WIDTH: f32 = 128.0;
const TAB_ROOM: f32 = 12.0;
const TAB_TOP: f32 = -43.0;
const TAB_BAR: f32 = 24.0;
const TAB_GAP: f32 = 18.0;
const TAB_FONT: f32 = 15.0;
/// The sort's button (`cUI_Button2` at `+0x158`), at (−2, 254) of the window.
const SORT: (u32, Vec2) = (73, Vec2::new(-2.0, 254.0));
/// The equipment's ornament (`GUI_main_ornament`), drawn at an alpha of `0x9F` (`0x6BAC80`).
const ORNAMENT: u32 = 376;
const ORNAMENT_ALPHA: f32 = 159.0 / 255.0;
/// The equipment's frame (`0x6BAC80`): a right edge of eight pieces from 10 down, 34 or, in 41
/// cases of a hundred, 34 or 35; a top edge of four from −1 along, 50, 7, 7 and 51, the last 3
/// along; a bottom edge of 60, two of 15 to 19 and 61, the last 3 back.
const EQUIPMENT_RIGHT: usize = 8;
const EQUIPMENT_RIGHT_FROM: f32 = 10.0;
const EQUIPMENT_TOP: [u32; 4] = [50, 7, 7, 51];
const EQUIPMENT_BOTTOM: (u32, u32, (u32, u32)) = (60, 61, (15, 5));

/// The windows made or taken away as they show and hide, which the grid's page follows
/// (`backpack`).
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Opened;

/// The equipment window, which the slots' frames go into (`equipment`, #1288).
#[derive(Component)]
pub(crate) struct EquipmentWindow;

/// Whether the inventory's windows show.
#[derive(Resource, Default)]
pub struct Shown(pub bool);

/// The page of the inventory's tab sheet that shows (`cUI_TabSheet`): 0 the items', 1 the
/// spells' (`spells_page`), 2 the combos'; the items' each time the windows show (`0x6C5A50`,
/// `0x729940(0)`).
#[derive(Resource, Default)]
pub struct Page(pub u8);

/// The page that the windows show on as they next show, the items' page where none (F and C,
/// #1596).
#[derive(Resource, Default)]
pub struct Opening(pub Option<u8>);

/// A tab of the tab sheet, by its page.
#[derive(Component)]
struct Tab(u8);

/// The sound of the spells' and the combos' pages as they show (`0x6C1E60`, `0x6BF3F0`: (14, 2,
/// 2) and (14, 2, 3), `UI_CLICK01`).
const PAGE_SHOW: u32 = 7000;

/// The container of the windows, the original's screen centred along the window's width.
#[derive(Component)]
struct Screen;

/// A piece of `count` at random from `first` (`rand()` / 32767 · count + first, truncated).
fn at_random(dice: &mut Rng, (first, count): (u32, u32)) -> u32 {
    first + dice.below(count)
}

/// The images and places of the inventory's frame: each piece's image and its place in the
/// window, by `anchors` and `size` of an image (`0x6C6490`).
fn inventory_frame(
    dice: &mut Rng,
    anchors: impl Fn(u32) -> [f32; 4],
    size: impl Fn(u32) -> Vec2,
) -> Vec<(u32, Vec2)> {
    let height = INVENTORY.height();
    let width = INVENTORY.width();
    let mut pieces = Vec::new();
    let pick =
        |dice: &mut Rng, step: usize, last: usize, (first, end, range): (u32, u32, (u32, u32))| {
            if step == 0 {
                first
            } else if step == last {
                end
            } else {
                at_random(dice, range)
            }
        };
    let mut x = 0.0;
    for step in 0..STEPS {
        let top = pick(dice, step, STEPS - 1, TOP);
        pieces.push((top, Vec2::new(x, -anchors(top)[1])));
        let bottom = pick(dice, step, STEPS - 1, BOTTOM);
        pieces.push((bottom, Vec2::new(x, height - anchors(bottom)[1])));
        x += size(bottom).x;
    }
    let mut y = RIGHT_FROM;
    for step in 0..RIGHT_PIECES {
        let piece = pick(dice, step, RIGHT_PIECES - 1, RIGHT);
        pieces.push((piece, Vec2::new(width - anchors(piece)[2] + 1.0, y)));
        y += size(piece).y;
    }
    pieces
}

/// The images and places of the equipment's frame (`0x6BAC80`).
fn equipment_frame(
    dice: &mut Rng,
    anchors: impl Fn(u32) -> [f32; 4],
    size: impl Fn(u32) -> Vec2,
) -> Vec<(u32, Vec2)> {
    let side = EQUIPMENT.width();
    let mut pieces = Vec::new();
    let mut y = EQUIPMENT_RIGHT_FROM;
    for _ in 0..EQUIPMENT_RIGHT {
        let piece = if dice.below(101) > 40 {
            34
        } else {
            at_random(dice, (34, 2))
        };
        pieces.push((piece, Vec2::new(side - anchors(piece)[2], y)));
        y += size(piece).y;
    }
    let mut x = -1.0;
    let (first, last, range) = EQUIPMENT_BOTTOM;
    for (step, &top) in EQUIPMENT_TOP.iter().enumerate() {
        if step == 3 {
            x += 3.0;
        }
        pieces.push((top, Vec2::new(x, -anchors(top)[1])));
        let bottom = match step {
            0 => first,
            3 => {
                x -= 3.0;
                last
            }
            _ => at_random(dice, range),
        };
        pieces.push((bottom, Vec2::new(x, side - anchors(bottom)[1])));
        x += size(bottom).x;
    }
    pieces
}

/// The sort's button, its side.
const SORT_SIDE: f32 = 34.0;

/// A left click on the sort's button while the windows show sorts the backpack's grid
/// (`cInventoryEntry::autoSortItems`, `0x5DB6B0`, #1321): by the items' categories and areas, at
/// their first fits by columns, the button's other mode (by rows, its `+0x168` bit 4) not read;
/// the grid as it was where the sort leaves an item out.
fn sort(
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    shown: Res<Shown>,
    mut pack: ResMut<crate::pickup::Pack>,
    tables: Option<Res<crate::drops::Tables>>,
) {
    if !shown.0 || !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let corner = INVENTORY.min + SORT.1;
    let on_button = screen_point(&pointer, &ui_scale, &windows)
        .is_some_and(|at| Rect::from_corners(corner, corner + Vec2::splat(SORT_SIDE)).contains(at));
    let Some(tables) = tables.filter(|_| on_button) else {
        return;
    };
    let category = |key: u32| {
        pack.items
            .iter()
            .find(|(k, _)| *k == key)
            .map_or(0, |(_, dropped)| tables.category(dropped.item))
    };
    match pack.grid.sorted(category, false) {
        Some(sorted) => {
            info!("inventory: the backpack sorted");
            pack.grid = sorted;
        }
        None => info!("inventory: the backpack left as it was: the sort leaves an item out"),
    }
}

/// `I` and the bar's inventory button show and hide the windows (`0x6C5A50`), and Esc hides
/// them, with their sounds; F and C show them on the combat arts' and the combos' pages, the
/// original's Quickstart's windows, or hide them from those pages (#1596); the keys taken from
/// the game; not while the world map shows (`0x757840`).
pub(crate) fn toggle(
    mut keys: crate::controls::KeysMut,
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    images: Res<UiImages>,
    (mut shown, map): (ResMut<Shown>, Res<crate::world_map::Map>),
    (mut page, mut opening): (ResMut<Page>, ResMut<Opening>),
    mut sounds: MessageWriter<sfx::Play>,
) {
    if map.shown {
        return;
    }
    let wanted = [
        (
            crate::controls::Action::CombatArts,
            crate::spells_page::PAGE,
        ),
        (crate::controls::Action::Combos, crate::combos_page::PAGE),
    ]
    .into_iter()
    .find(|&(action, _)| keys.take(action))
    .map(|(_, page)| page);
    if let Some(wanted) = wanted {
        if shown.0 && page.0 != wanted {
            page.0 = wanted;
            return;
        }
        shown.0 = !shown.0;
        opening.0 = Some(wanted);
        sounds.write(sfx::Play {
            sound: sfx::Of::Id(if shown.0 { OPENS } else { CLOSES }),
            at: None,
            rate: (0.5, sfx::Roll::None),
        });
        return;
    }
    let pressed = keys.take(crate::controls::Action::Inventory);
    let clicked = buttons.just_pressed(MouseButton::Left)
        && screen_point(&pointer, &ui_scale, &windows).is_some_and(|at| {
            let (index, corner) = bar::INVENTORY_BUTTON;
            Rect::from_corners(corner, corner + images.size(index)).contains(at)
        });
    // Esc closes the windows after the world map and the quest book, which take it first, and
    // before the pause's menu (`cUI_Manager::handleEsc`, `0x7572C0`, #1592).
    let escaped = shown.0 && keys.take(crate::controls::Action::Back);
    if !(pressed || clicked || escaped) {
        return;
    }
    shown.0 = !shown.0;
    sounds.write(sfx::Play {
        sound: sfx::Of::Id(if shown.0 { OPENS } else { CLOSES }),
        at: None,
        rate: (0.5, sfx::Roll::None),
    });
}

/// The pointer on the original's screen, centred along the window's width as the bar is.
pub(crate) fn screen_point(
    pointer: &Pointer,
    ui_scale: &UiScale,
    windows: &Query<&Window>,
) -> Option<Vec2> {
    let at = pointer.0?;
    let window = windows.single().ok()?;
    let scale = ui_scale.0.max(0.01);
    let width = window.width() / scale;
    let at = at / scale;
    Some(Vec2::new(at.x - (width - bar::SCREEN.x) / 2.0, at.y))
}

/// Makes the windows as they show and takes them away as they hide.
fn show(
    mut commands: Commands,
    (shown, mut page, mut opening): (Res<Shown>, ResMut<Page>, ResMut<Opening>),
    screens: Query<Entity, With<Screen>>,
    (images, assets, texts): (Res<UiImages>, Res<AssetServer>, Option<Res<Texts>>),
    mut dice: Local<Option<Rng>>,
) {
    if !shown.is_changed() {
        return;
    }
    for screen in &screens {
        commands.entity(screen).despawn();
    }
    let wanted = opening.0.take();
    if !shown.0 {
        return;
    }
    page.0 = wanted.unwrap_or(0);
    let dice = dice.get_or_insert_with(|| crate::seed::dice(1267));
    let anchors = |index| images.anchors(index);
    let size = |index| images.size(index);
    let piece = |index: u32, at: Vec2| bar::piece(&images, &assets, index, at, Vec2::ZERO);
    let font: Handle<Font> = assets.load(FONT);
    let screen = commands
        .spawn((
            Screen,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Px(0.0),
                margin: UiRect::left(Val::Px(-bar::SCREEN.x / 2.0)),
                width: Val::Px(bar::SCREEN.x),
                height: Val::Px(bar::SCREEN.y),
                ..default()
            },
            Pickable::IGNORE,
            DespawnOnExit(AppState::InGame),
        ))
        .id();
    let window = |rect: Rect| (bar::placed(rect.min, rect.size()), Pickable::IGNORE);
    let inventory = commands.spawn(window(INVENTORY)).id();
    let equipment = commands.spawn((window(EQUIPMENT), EquipmentWindow)).id();
    commands
        .entity(screen)
        .add_children(&[inventory, equipment]);
    // The equipment's ornament under its frame.
    let (mut ornament, node) = piece(ORNAMENT, Vec2::ZERO);
    ornament.color = Color::srgba(1.0, 1.0, 1.0, ORNAMENT_ALPHA);
    let ornament = commands.spawn((ornament, node, Pickable::IGNORE)).id();
    commands.entity(equipment).add_child(ornament);
    for (index, at) in equipment_frame(dice, anchors, size) {
        let child = commands.spawn((piece(index, at), Pickable::IGNORE)).id();
        commands.entity(equipment).add_child(child);
    }
    for (index, at) in inventory_frame(dice, anchors, size) {
        let child = commands.spawn((piece(index, at), Pickable::IGNORE)).id();
        commands.entity(inventory).add_child(child);
    }
    let sort = commands
        .spawn((
            piece(SORT.0, SORT.1),
            Pickable::IGNORE,
            crate::control_glow::Glows,
        ))
        .id();
    commands.entity(inventory).add_child(sort);
    // The tabs above the window, each its text on the image 108 stretched to it.
    let labels: Vec<String> = TAB_TEXTS
        .iter()
        .map(|key| {
            texts
                .as_deref()
                .and_then(|texts| texts.named(key))
                .unwrap_or(key)
                .to_owned()
        })
        .collect();
    let (tabs, each) = spawn_tabs(&mut commands, (&images, &assets), &font, &labels);
    commands.entity(inventory).add_child(tabs);
    for (index, tab) in (0_u8..).zip(each) {
        commands.entity(tab).insert(Tab(index));
    }
}

/// A tab sheet's tabs (`cUI_TabSheet`, `0x729980`) of `labels`, in a row standing above its
/// sheet's top left: each its label in capitals on the image 108 stretched to it; the row and
/// each tab.
pub(crate) fn spawn_tabs(
    commands: &mut Commands,
    (images, assets): (&UiImages, &AssetServer),
    font: &Handle<Font>,
    labels: &[String],
) -> (Entity, Vec<Entity>) {
    let tabs = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(TAB_TOP),
                column_gap: Val::Px(TAB_GAP),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    let mut each = Vec::new();
    for label in labels {
        let words = label.to_uppercase();
        let (mut image, _) = bar::piece(images, assets, TAB, Vec2::ZERO, Vec2::ZERO);
        image.image_mode = NodeImageMode::Stretch;
        // The image over the whole tab, the text's room in a node of its own: an image is drawn
        // within its node's padding.
        let tab = commands
            .spawn((
                image,
                Node {
                    min_width: Val::Px(TAB_WIDTH),
                    height: Val::Px(images.size(TAB).y),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::FlexStart,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|tab| {
                tab.spawn((
                    Node {
                        height: Val::Px(TAB_BAR),
                        padding: UiRect::horizontal(Val::Px(TAB_ROOM)),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .with_child((
                    Text::new(words),
                    TextFont {
                        font: font.clone().into(),
                        font_size: FontSize::Px(TAB_FONT),
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.82, 0.7)),
                    Pickable::IGNORE,
                ));
            })
            .id();
        commands.entity(tabs).add_child(tab);
        each.push(tab);
    }
    (tabs, each)
}

/// A left click on a tab shows its page (`cUI_TabSheet`, `0x729980`).
fn select(
    buttons: Res<ButtonInput<MouseButton>>,
    (shown, mut page): (Res<Shown>, ResMut<Page>),
    (pointer, windows): (Res<Pointer>, Query<&Window>),
    tabs: Query<(&Tab, &ComputedNode, &UiGlobalTransform)>,
) {
    if !shown.0 || !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    // The pointer in the window's physical pixels, as the interface's nodes lie.
    let (Some(at), Ok(window)) = (pointer.0, windows.single()) else {
        return;
    };
    let point = at * window.scale_factor();
    let Some(tab) = tabs
        .iter()
        .find(|(_, node, transform)| node.contains_point(**transform, point))
        .map(|(tab, ..)| tab.0)
    else {
        return;
    };
    if page.0 == tab {
        return;
    }
    page.0 = tab;
}

/// The pages' sounds as they show, whichever way they come (a tab, a key, the windows shown on
/// them): the spells' (`0x6C1E60`, (14, 2, 2)) and the combos' (`0x6BF3F0`, (14, 2, 3)),
/// `UI_CLICK01` both (#1722).
fn page_sounds(
    shown: Res<Shown>,
    page: Res<Page>,
    mut sounds: MessageWriter<sfx::Play>,
    mut last: Local<Option<u8>>,
) {
    let now = shown.0.then_some(page.0);
    if *last == now {
        return;
    }
    *last = now;
    if now.is_some_and(|page| page == crate::spells_page::PAGE || page == crate::combos_page::PAGE)
    {
        sounds.write(sfx::Play {
            sound: sfx::Of::Id(PAGE_SHOW),
            at: None,
            rate: (0.5, sfx::Roll::None),
        });
    }
}

/// The traders' and the stashes' windows that show with the inventory: a town stash's, a combat
/// master's, a merchant's and a smith's.
type BesideWindows<'w> = (
    Option<Res<'w, crate::town_stash::Opened>>,
    Option<Res<'w, crate::master::Master>>,
    Option<Res<'w, crate::merchant::Merchant>>,
    Option<Res<'w, crate::blacksmith::Blacksmith>>,
);

/// Whether the pointer is off the windows that show, whose clicks do not reach the world: the
/// inventory with its tabs above it, the equipment and the statistics (#1521), and a stash's or
/// a trader's window beside them.
fn off_the_windows(
    shown: Option<Res<Shown>>,
    images: Res<UiImages>,
    pointer: Res<Pointer>,
    ui_scale: Res<UiScale>,
    windows: Query<&Window>,
    (stash, master, merchant, smith): BesideWindows,
) -> bool {
    if !shown.is_some_and(|shown| shown.0) {
        return true;
    }
    let Some(at) = screen_point(&pointer, &ui_scale, &windows) else {
        return true;
    };
    let over_stash =
        stash.is_some_and(|stash| stash.0.is_some()) && crate::town_stash::WINDOW.contains(at);
    let over_master = master.is_some() && crate::master::WINDOW.contains(at);
    let over_merchant = merchant.is_some() && crate::merchant::WINDOW.contains(at);
    let over_smith = smith.is_some() && crate::blacksmith::WINDOW.contains(at);
    !(over_windows(at, crate::statistics::close_button(&images))
        || over_stash
        || over_master
        || over_merchant
        || over_smith)
}

/// Whether `at` is over the windows that show with the inventory: the inventory with its tabs
/// above it, its sort's button, which stands below it but its top two rows, the equipment, and
/// the statistics window with its close button `close`, which stands out of its right edge
/// (#1593).
fn over_windows(at: Vec2, close: Rect) -> bool {
    let tabs = Rect {
        min: INVENTORY.min + Vec2::new(0.0, TAB_TOP),
        max: INVENTORY.max,
    };
    let sort = INVENTORY.min + SORT.1;
    tabs.contains(at)
        || Rect::from_corners(sort, sort + Vec2::splat(SORT_SIDE)).contains(at)
        || EQUIPMENT.contains(at)
        || crate::statistics::WINDOW.contains(at)
        || close.contains(at)
}

/// The windows hidden at each game's start.
fn hide(mut shown: ResMut<Shown>) {
    shown.0 = false;
}

/// The inventory's windows.
pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Shown>()
            .init_resource::<Page>()
            .init_resource::<Opening>()
            .hold_control(off_the_windows)
            .add_systems(OnEnter(AppState::InGame), hide)
            .add_systems(
                PreUpdate,
                toggle
                    .after(InputSystems)
                    .after(crate::console::keys)
                    // The book takes Esc before the inventory (`0x7572C0`).
                    .after(crate::quest_book::toggle)
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                Update,
                (
                    show.in_set(Opened),
                    select.in_set(Opened).after(show),
                    page_sounds.after(Opened),
                    sort.after(Opened),
                )
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pieces' anchors and sizes of the table (`tables/ui_images.ron`): each of the top's
    /// stands 8 over its line, the bottom's 4, and the right's 1 from its left.
    fn anchors(index: u32) -> [f32; 4] {
        match index {
            5 => [102.0, 9.0, 0.0, 0.0],
            12 => [0.0, 9.0, 0.0, 0.0],
            22..=26 | 16 | 17 | 19 => [0.0, 8.0, 0.0, 0.0],
            30 => [0.0, 0.0, 11.0, 0.0],
            28 | 29 | 32 | 33 | 34 | 35 => [0.0, 0.0, 1.0, 0.0],
            _ => [0.0; 4],
        }
    }

    fn size(index: u32) -> Vec2 {
        match index {
            5 => Vec2::new(103.0, 20.0),
            32 => Vec2::new(16.0, 16.0),
            33 => Vec2::new(16.0, 14.0),
            28..=30 => Vec2::new(16.0, 32.0),
            34 => Vec2::new(6.0, 32.0),
            35 => Vec2::new(19.0, 32.0),
            50 => Vec2::new(64.0, 19.0),
            60 => Vec2::new(64.0, 23.0),
            61 => Vec2::new(70.0, 18.0),
            _ => Vec2::new(64.0, 20.0),
        }
    }

    #[test]
    fn the_inventorys_frame_stands_on_its_edges() {
        let mut dice = Rng::of(0, 0, 1);
        let pieces = inventory_frame(&mut dice, anchors, size);
        assert_eq!(pieces.len(), 2 * STEPS + RIGHT_PIECES);
        // The top's first piece raised by its anchor, the bottom's first on the window's foot.
        assert_eq!(pieces[0], (5, Vec2::new(0.0, -9.0)));
        assert_eq!(pieces[1], (3, Vec2::new(0.0, 256.0)));
        // The steps go by the bottom pieces' widths; the last ones their own.
        assert_eq!(pieces[2].1.x, 64.0);
        assert_eq!(pieces[18].0, 12);
        assert_eq!(pieces[19].0, 13);
        assert!(
            pieces[2..18]
                .iter()
                .step_by(2)
                .all(|(i, _)| (22..=26).contains(i))
        );
        assert!(
            pieces[3..18]
                .iter()
                .step_by(2)
                .all(|(i, _)| (15..=19).contains(i))
        );
        // The right edge from 4 down at the pieces' second anchors.
        assert_eq!(pieces[20], (32, Vec2::new(640.0, 4.0)));
        assert_eq!(pieces[21].1.y, 20.0);
        assert_eq!(pieces.last().map(|p| p.0), Some(33));
    }

    #[test]
    fn the_equipments_frame_closes_its_square() {
        let mut dice = Rng::of(0, 0, 2);
        let pieces = equipment_frame(&mut dice, anchors, size);
        assert_eq!(pieces.len(), EQUIPMENT_RIGHT + 8);
        assert!(
            pieces[..EQUIPMENT_RIGHT]
                .iter()
                .all(|(i, _)| *i == 34 || *i == 35)
        );
        assert_eq!(pieces[0].1, Vec2::new(255.0, 10.0));
        // The top from −1, its last 3 along, and the bottom's last 3 back.
        let top: Vec<u32> = pieces[EQUIPMENT_RIGHT..]
            .iter()
            .step_by(2)
            .map(|(i, _)| *i)
            .collect();
        assert_eq!(top, EQUIPMENT_TOP);
        assert_eq!(pieces[EQUIPMENT_RIGHT].1.x, -1.0);
        let last_top = pieces[EQUIPMENT_RIGHT + 6];
        let last_bottom = pieces[EQUIPMENT_RIGHT + 7];
        assert_eq!(last_top.1.x, last_bottom.1.x + 3.0);
        assert_eq!(last_bottom.0, 61);
    }

    #[test]
    fn the_buttons_out_of_the_windows_hold_their_clicks() {
        // The statistics window's close button, 34 wide at (898, 10), out of its right edge.
        let close = Rect::from_corners(Vec2::new(898.0, 10.0), Vec2::new(932.0, 44.0));
        let sort = INVENTORY.min + SORT.1;
        for at in [
            sort + Vec2::splat(1.0),
            sort + Vec2::splat(SORT_SIDE - 1.0),
            Vec2::new(930.0, 26.0),
        ] {
            assert!(over_windows(at, close), "{at}");
        }
        // The world beside them.
        assert!(!over_windows(
            sort + Vec2::new(SORT_SIDE + 1.0, 30.0),
            close
        ));
        assert!(!over_windows(Vec2::new(935.0, 26.0), close));
    }
}
