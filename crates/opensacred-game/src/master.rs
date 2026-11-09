//! The combat master's window (#1540; docs/re/arts.md, "The combat master"): a creature of
//! `+0x200` bit `0x4000` (`quests::WIZARD`) without a dialogue shows, as the hero speaks to it
//! (`talk`, `0x52AB70`), the inventory and the master's window (`cUI_Master`, `UI_WND_MASTER`) at
//! the screen's top left, 640 × 342: its two panels tinted `0x9F000000`, the master's portrait
//! and title (the text 1198), and its tab sheet's tabs COMBO and EXCHANGE over its COMBO page
//! (`cUI_MasterCombo`, `0x6D1DA0`), the inventory's combos page selected; the inventory's hide
//! hides it.
//!
//! On the COMBO page a combo dropped on the symbol's frame takes its place with its steps, the
//! spells page selected; a press on the frame gives it back. A left press on a step swaps its art
//! with the art on the cursor, at the hero's level of it with its bonus, a right press empties
//! it. Each changed step's price shows under it, the total under the button, in red where the
//! gold falls short, and the count of the steps that hold an art where more than one does; the
//! button lights while a step holds an art and the cost is the gold or less, and makes the combo
//! only where the gold is above the cost (`0x6D26D0`, `0x6D22A0`): the combo set, the gold taken,
//! the page cleared and the combos page selected (`opensacred_rules::combos`).
//!
//! The window and its combos page play the combo master's sound as they show (#1722).
//!
//! The EXCHANGE page (`cUI_MasterLotto`, #1895; `opensacred_rules::exchange`), shown by its tab
//! with the combo master's sound and the inventory's items page: four slots take runes off the
//! cursor; with two to four of them the button, where the gold pays the balance's price of the
//! count, takes it and makes a rune of every class's (two), of the hero's class's (three) or
//! the one of the hero's class's that the arrows beside the result's slot choose (four), the
//! slots emptied; the rune made waits in the result's slot until the left button takes it onto
//! the cursor or the right puts it into the backpack. As the inventory hides, the runes go back
//! into the backpack.
//!
//! Not followed: the buttons' sounds, the spells page's view under the master (the Vampiress's
//! arts not greyed), a combo on the page's tooltip, and a press on a slot with another item on
//! the cursor (not read).

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use opensacred_rules::combos::{self, STEPS, Step};
use opensacred_rules::drops::{Dropped, Kind};
use opensacred_rules::exchange;

use crate::art_slots::HeldArt;
use crate::arts::HeroArts;
use crate::bar::{self, UiImages};
use crate::choice::Layer;
use crate::combos::HeroCombos;
use crate::drops::Tables;
use crate::game::{AppState, FONT};
use crate::hud::Pointer;
use crate::inventory::{Page, Shown};
use crate::lines::Texts;
use crate::pickup::{Held, Pack};
use crate::tooltip::{Rest, TipRoot, argb, spawn_tip};

/// The window on the original's screen, and its two panels and their tint (`UI_STATIC_BG`,
/// `0x9F000000`).
pub(crate) const WINDOW: Rect = Rect {
    min: Vec2::ZERO,
    max: Vec2::new(640.0, 342.0),
};
const RIGHT_PANEL: Rect = Rect {
    min: Vec2::new(512.0, 16.0),
    max: Vec2::new(640.0, 340.0),
};
const LEFT_PANEL: Rect = Rect {
    min: Vec2::new(0.0, 50.0),
    max: Vec2::new(512.0, 342.0),
};
const PANEL: Color = Color::srgba(0.0, 0.0, 0.0, 159.0 / 255.0);
/// The panels' edges (**estimate**, by the original's frames): a grey line of two pixels.
const EDGE: Color = Color::srgb(0.33, 0.33, 0.31);
/// The portrait's corner in the right panel (the control `+0x158`, `0x6D1460`; **seen**: the
/// image 833 there to the pixel), the title's line and colour (`0xFF0050FF`).
const PORTRAIT: Vec2 = Vec2::new(524.0, 26.0);
/// The masters' types with an interface image of their own for a portrait (`0x6D1180`): the type
/// 317's (`TYPE_NPC_WIZARD`) the image 833, a bald old man (`gui_portraitx01`), and the type 84's
/// (`TYPE_NPC_ORC_CHAMP_2`) 896 (`gui_portraitx17`).
const OWN_PORTRAITS: [(u32, u32); 2] = [(317, 833), (84, 896)];
const TITLE: &str = "1198";
const TITLE_AT: Vec2 = Vec2::new(512.0, 192.0);
const TITLE_COLOUR: u32 = 0xFF00_50FF;
/// The tab sheet's corner (`UI_TABSHEET_MASTER`) and its tabs' texts, the COMBO page's first.
const SHEET: Vec2 = Vec2::new(0.0, 44.0);
const TABS: [&str; 2] = ["UI_MASTER_COMBO", "UI_MASTER_LOTTO"];
/// The COMBO page's title (`UI_COMBO_MAKE`) and its colour.
const MAKE_TITLE: &str = "UI_COMBO_MAKE";
const MAKE_TITLE_AT: Vec2 = Vec2::new(16.0, 62.0);
const YELLOW: u32 = 0xFFFF_FF80;
const WHITE: u32 = 0xFFFF_FFFF;
const RED: u32 = 0xFFFF_0020;
/// The symbol's frame (246, 247 while a combo is on the cursor) and the symbol in it.
const SYMBOL_FRAME: Rect = Rect {
    min: Vec2::new(16.0, 92.0),
    max: Vec2::new(105.0, 180.0),
};
const FRAME: u32 = 246;
const FRAME_LIT: u32 = 247;
const SYMBOL_AT: Vec2 = Vec2::new(27.0, 103.0);
const SYMBOL: f32 = 64.0;
/// The steps' frames (202), 88 apart from (16, 212), and the arrows between them (251).
const STEP_FRAME: u32 = 202;
const STEPS_FROM: Vec2 = Vec2::new(16.0, 212.0);
const STEP_APART: f32 = 88.0;
const ARROW: u32 = 251;
const ARROWS_FROM: Vec2 = Vec2::new(88.0, 236.0);
/// The button (`+0x884`): 255 grey, 253 lit.
const BUTTON: Rect = Rect {
    min: Vec2::new(432.0, 212.0),
    max: Vec2::new(493.0, 272.0),
};
const BUTTON_GREY: u32 = 255;
const BUTTON_LIT: u32 = 253;
/// The art lines (`name - level`), 24 apart from (144, 92).
const LINES_FROM: Vec2 = Vec2::new(144.0, 92.0);
const LINE_APART: f32 = 24.0;
const LEVEL: &str = "1070";
/// Each changed step's price under it, from (14, 276) 88 apart, the total under the button and
/// the count, with the word of the gold (1082) a line below.
const PRICES_FROM: Vec2 = Vec2::new(14.0, 276.0);
const PRICE_WIDTH: f32 = 68.0;
const TOTAL_AT: Vec2 = Vec2::new(429.0, 276.0);
const TOTAL_WIDTH: f32 = 64.0;
const COUNT_AT: Vec2 = Vec2::new(370.0, 280.0);
const GOLD: &str = "1082";
const GOLD_BELOW: f32 = 16.0;
const FONT_SIZE: f32 = 20.0;
const SMALL: f32 = 17.0;
/// The tooltips: the symbol's frame's while no combo stands in it, an empty step's and the
/// button's, centred 32 along and 80 below their controls' corners.
const TIP_FRAME: &str = "9936";
const TIP_STEP: &str = "9938";
const TIP_BUTTON: &str = "9939";
const TIP_FROM: Vec2 = Vec2::new(32.0, 80.0);

/// The pages of the tab sheet.
const COMBO_PAGE: u8 = 0;
const EXCHANGE_PAGE: u8 = 1;
/// The EXCHANGE page (`cUI_MasterLotto`, #1895; its layout `0x6D5B80`, its draw `0x6D49A0`):
/// its title (the text 9970) where the COMBO page's stands; four slots of the runes from
/// (16, 92), each the image 247 empty and 246 holding a rune, 16 apart; the result's slot (246)
/// at (168, 200) with the arrows 357 and 358 either side of it, the left one's right edge at
/// 169, the right one at the slot's width and 167, each centred down the slot; the button at
/// the COMBO page's; and a rune drawn at its slot's centre 1.5 times its size.
const EXCHANGE_TITLE: &str = "9970";
const SLOTS_FROM: Vec2 = Vec2::new(16.0, 92.0);
const SLOT_GAP: f32 = 16.0;
const SLOT_EMPTY: u32 = 247;
const SLOT_FILLED: u32 = 246;
const RESULT_AT: Vec2 = Vec2::new(168.0, 200.0);
const ARROW_LEFT: u32 = 357;
const ARROW_RIGHT: u32 = 358;
const ARROW_LEFT_EDGE: f32 = 169.0;
const ARROW_RIGHT_FROM: f32 = 167.0;
const RUNE_SCALE: f32 = 1.5;
/// The price where it is not 0: the word of the gold (1082) and the price under it, centred in
/// a rectangle left of the button, yellow or red where the gold falls short (**measured**
/// 2026-11-03: their letters from 229 and from 248, centred on 346).
const EXCHANGE_PRICE: Rect = Rect {
    min: Vec2::new(300.0, 225.0),
    max: Vec2::new(392.0, 244.0),
};
/// The hints (the style `0x209`) of an empty slot (9974), of the button (9975) and of the empty
/// result's slot (9976), centred 256 along and 108 below their controls' corners (**seen**), and
/// a rune's tooltip at its slot's corner and 92 along, 16 down (`0x6D49A0`).
const TIP_SLOT: &str = "9974";
const TIP_EXCHANGE: &str = "9975";
const TIP_RESULT: &str = "9976";
const HINT_FROM: Vec2 = Vec2::new(256.0, 108.0);
const RUNE_TIP_FROM: Vec2 = Vec2::new(92.0, 16.0);
/// The render layer of the runes' models.
const RUNES_LAYER: usize = 20;
/// The runes' category, the only items that the slots take (`0x6D5760`: the cursor's kind 1 of
/// the category `0x1C`).
const RUNES: u8 = 28;

/// The EXCHANGE page's runes: those in its four slots, as they came off the cursor, and the rune
/// made, waiting in the result's slot.
#[derive(Clone, Debug, Default, PartialEq)]
struct Exchange {
    slots: [Option<Held>; exchange::SLOTS],
    result: Option<Dropped>,
}

impl Exchange {
    /// How many runes the slots hold.
    fn count(&self) -> usize {
        self.slots.iter().flatten().count()
    }

    /// Takes every rune out: those of the slots and the result.
    fn take_all(&mut self) -> Vec<(Dropped, (u8, u8))> {
        let mut all: Vec<(Dropped, (u8, u8))> = self
            .slots
            .iter_mut()
            .filter_map(Option::take)
            .map(|held| (held.dropped, held.size))
            .collect();
        all.extend(self.result.take().map(|dropped| (dropped, (1, 1))));
        all
    }
}

/// The choice of the arrows, kept through the game (`0x17EA250`).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct RuneChoice(exchange::Choice);

/// The master whose window shows, and its pages: the COMBO page's combo dropped on it, its steps
/// as they stood and as they are made; the EXCHANGE page's runes; the page showing.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Master {
    pub creature: Entity,
    /// Its creature type, whose portrait the window shows (the event `0x2F` (8, id)).
    kind: u32,
    combo: Option<u16>,
    old: [Step; STEPS],
    steps: [Step; STEPS],
    /// Whether the window has just opened: the combos page is then selected.
    opened: bool,
    page: u8,
    exchange: Exchange,
}

/// A rune of the type `rune` as the page makes it (`sdk_OBJ_CREATE_NPOS`): a plain item of it.
fn rune_item(rune: u32) -> Dropped {
    Dropped {
        kind: Kind::Rune,
        item: rune,
        level: 0,
        key: 16,
        quality: 0,
        amount: 1,
        values: None,
        named: None,
        socketed: Vec::new(),
        name_key: 0,
    }
}

/// The corner of the slot `k` of the runes, `width` the slot's image's.
fn slot_at(k: usize, width: f32) -> Vec2 {
    SLOTS_FROM
        + Vec2::new(
            (width + SLOT_GAP) * f32::from(u8::try_from(k).unwrap_or(0)),
            0.0,
        )
}

/// The rectangle of the image `index` at `at`.
fn control(images: &UiImages, index: u32, at: Vec2) -> Rect {
    Rect::from_corners(at, at + images.size(index))
}

/// The arrows' rectangles, the left's and the right's, either side of the result's slot.
fn arrows(images: &UiImages) -> (Rect, Rect) {
    let slot = images.size(SLOT_FILLED);
    let (left, right) = (images.size(ARROW_LEFT), images.size(ARROW_RIGHT));
    let down = |size: Vec2| RESULT_AT.y + (slot.y / 2.0).floor() - (size.y / 2.0).floor();
    let left_at = Vec2::new(ARROW_LEFT_EDGE - right.x, down(left));
    let right_at = Vec2::new(slot.x + ARROW_RIGHT_FROM, down(right));
    (
        Rect::from_corners(left_at, left_at + left),
        Rect::from_corners(right_at, right_at + right),
    )
}

/// The rune that four runes would make and the arrows choose, of the hero's class's list.
fn chosen(tables: &Tables, arts: &HeroArts, choice: exchange::Choice) -> Option<u32> {
    let list = exchange::list(Some(arts.kind), &arts.tables, |rune| tables.rune_art(rune));
    choice.of(&list)
}

/// The rune that the result's slot shows: the one made, else with four runes in the slots the
/// one the arrows choose (`0x6D3FD0`, the preview `+0x7B0`).
fn preview(
    master: &Master,
    tables: &Tables,
    arts: &HeroArts,
    choice: exchange::Choice,
) -> Option<u32> {
    master.exchange.result.as_ref().map(|d| d.item).or_else(|| {
        (master.exchange.count() == exchange::SLOTS)
            .then(|| chosen(tables, arts, choice))
            .flatten()
    })
}

/// The window's container.
#[derive(Component)]
struct WindowRoot;

/// The combo master's sound (7110), which the window and its combos page play as they show.
const SHOW_SOUND: u32 = 7110;

/// Shows the master's window for `creature` with the inventory (`0x52AB70`), with its sounds.
pub fn open(world: &mut World, creature: Entity) {
    let kind = world
        .get::<crate::ai::Mind>(creature)
        .map_or(0, |mind| mind.creature);
    info!("master: {creature}, a {kind}, shows its window");
    // The combo master's sound as the window shows (`0x6D1360`: the event (14, 2, `0x20`)) and
    // again as its combos page shows with it (`0x6D24A0`: (14, 2, `0x21`)), #1722.
    for _ in 0..2 {
        world.write_message(crate::sfx::Play {
            sound: crate::sfx::Of::Id(SHOW_SOUND),
            at: None,
            rate: (0.5, crate::sfx::Roll::None),
        });
    }
    world.insert_resource(Master {
        creature,
        kind,
        combo: None,
        old: [None; STEPS],
        steps: [None; STEPS],
        opened: true,
        page: COMBO_PAGE,
        exchange: Exchange::default(),
    });
    if let Some(mut shown) = world.get_resource_mut::<Shown>()
        && !shown.0
    {
        shown.0 = true;
    }
}

/// A text of `global.res` without the colour's tag it may start with (`<colorl=FFFFFF80>`), and
/// that colour, an ARGB word.
pub(crate) fn tagged(text: &str) -> (&str, Option<u32>) {
    let Some(rest) = text.strip_prefix("<color") else {
        return (text, None);
    };
    let Some((tag, words)) = rest.split_once('>') else {
        return (text, None);
    };
    let colour = tag
        .split_once('=')
        .and_then(|(_, hex)| u32::from_str_radix(hex, 16).ok());
    (words, colour)
}

/// The corner of the step `k`.
fn step_at(k: usize) -> Vec2 {
    STEPS_FROM + Vec2::new(STEP_APART * f32::from(u8::try_from(k).unwrap_or(0)), 0.0)
}

/// The step under the point `at`.
fn step_under(at: Vec2) -> Option<usize> {
    (0..STEPS).find(|&k| {
        let corner = step_at(k);
        Rect::from_corners(corner, corner + Vec2::splat(SYMBOL)).contains(at)
    })
}

/// The hero's level of the art `art` with its bonus (`0x580A10`), none for an art it lacks.
fn level_of(arts: &HeroArts, art: u16) -> Option<u8> {
    let entry = arts.arts.get(art)?;
    Some(entry.level.saturating_add(entry.bonus))
}

/// What the cost of `master`'s steps is, and whether the button lights, as a press makes the
/// combo, at `gold` (`0x6D23E0`, `combos::affordable`).
fn priced(master: &Master, gold: u32) -> (u32, bool) {
    let cost = combos::cost(&master.old, &master.steps);
    let lit = master.combo.is_some() && combos::affordable(&master.steps, cost, gold);
    (cost, lit)
}

/// Keeps the window with the inventory: the combos page selected as it opens, the master gone
/// as the inventory hides, the EXCHANGE page's runes back into the backpack (**seen**: the runes
/// of the slots and the result's), those without room dropped at the master's feet (not seen).
fn follow(
    mut commands: Commands,
    shown: Res<Shown>,
    master: Option<ResMut<Master>>,
    (mut page, mut pack): (ResMut<Page>, ResMut<Pack>),
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
    let Some(mut master) = master else {
        return;
    };
    if !shown.0 {
        let runes = master.exchange.take_all();
        let left: Vec<Dropped> = runes
            .into_iter()
            .filter_map(|(dropped, size)| (!pack.take(dropped.clone(), size)).then_some(dropped))
            .collect();
        if let (Some(tables), Ok(at), Some(body)) = (
            tables.as_deref(),
            creatures.get(master.creature),
            heroes.iter().next(),
        ) {
            let cell = crate::paths::cell_of(at.translation.x, at.translation.z);
            for dropped in left {
                info!("master: no room for the rune {}, dropped", dropped.item);
                crate::drops::drop_at(
                    &mut commands,
                    (&assets, tables, &mut dice),
                    (cell, body.level()),
                    dropped,
                );
            }
        }
        commands.remove_resource::<Master>();
        return;
    }
    if master.page != COMBO_PAGE {
        return;
    }
    // The inventory's show selects its items' page: the master's selects the combos' after it.
    if master.opened || shown.is_changed() {
        master.opened = false;
        page.0 = crate::combos_page::PAGE;
    }
}

/// The presses on the COMBO page.
fn press(
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    master: Option<ResMut<Master>>,
    mut held: ResMut<HeldArt>,
    mut page: ResMut<Page>,
    (arts, mut combos): (Option<Res<HeroArts>>, ResMut<HeroCombos>),
    mut pack: ResMut<Pack>,
) {
    let (left, right) = (
        buttons.just_pressed(MouseButton::Left),
        buttons.just_pressed(MouseButton::Right),
    );
    let (Some(mut master), Some(arts)) = (master, arts) else {
        return;
    };
    if !(left || right) || master.page != COMBO_PAGE {
        return;
    }
    let Some(at) = crate::inventory::screen_point(&pointer, &ui_scale, &windows) else {
        return;
    };
    if left && SYMBOL_FRAME.contains(at) {
        match (held.0, master.combo) {
            // A combo dropped on the frame takes its place with its steps, the spells page
            // selected.
            (Some(id), _) if crate::combos::is_combo(id) => {
                let steps = combos.get(id).map_or([None; STEPS], |combo| combo.steps);
                master.combo = Some(id);
                master.old = steps;
                master.steps = steps;
                held.0 = None;
                page.0 = crate::spells_page::PAGE;
            }
            // A press on the symbol gives it back.
            (None, Some(id)) => {
                held.0 = Some(id);
                master.combo = None;
                master.old = [None; STEPS];
                master.steps = [None; STEPS];
            }
            _ => {}
        }
        return;
    }
    if let Some(k) = step_under(at).filter(|_| master.combo.is_some()) {
        if right {
            master.steps[k] = None;
        } else if held.0.is_none_or(|art| !crate::combos::is_combo(art)) {
            // The step's art onto the cursor, the cursor's into the step at the hero's level.
            let into = held.0.and_then(|art| Some((art, level_of(&arts, art)?)));
            if held.0.is_some() && into.is_none() {
                return;
            }
            held.0 = master.steps[k].map(|(art, _)| art);
            master.steps[k] = into;
        }
        return;
    }
    if left && BUTTON.contains(at) && held.0.is_none() {
        let (cost, lit) = priced(&master, pack.gold);
        let Some(index) = master.combo.and_then(combos::index_of).filter(|_| lit) else {
            return;
        };
        let steps = master.steps;
        combos.0[index].set(steps, &arts.tables);
        pack.add_gold(-i64::from(cost));
        info!(
            "master: combo {} made of {:?} for {cost} gold, {} left",
            index + 1,
            steps,
            pack.gold
        );
        master.combo = None;
        master.old = [None; STEPS];
        master.steps = [None; STEPS];
        page.0 = crate::combos_page::PAGE;
    }
}

/// A tab of the master's tab sheet, by its page.
#[derive(Component)]
struct MasterTab(u8);

/// A left press on a tab shows its page (`cUI_TabSheet`, `0x729980`) with its show's sound, the
/// combo master's (**seen**: the EXCHANGE page's (14, 2, `0x22`)), and selects the inventory's
/// page that goes with it, the combos' or the items' (whose own sound follows, **seen**).
fn select_tab(
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, windows): (Res<Pointer>, Query<&Window>),
    (master, mut page): (Option<ResMut<Master>>, ResMut<Page>),
    tabs: Query<(&MasterTab, &ComputedNode, &UiGlobalTransform)>,
    mut sounds: MessageWriter<crate::sfx::Play>,
) {
    let Some(mut master) = master else {
        return;
    };
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
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
    if master.page == tab {
        return;
    }
    master.page = tab;
    crate::inventory::play(&mut sounds, SHOW_SOUND);
    page.0 = if tab == EXCHANGE_PAGE {
        0
    } else {
        crate::combos_page::PAGE
    };
}

/// The presses on the EXCHANGE page (`0x6D5760`): a slot's swaps its rune with a rune on the
/// cursor, or gives it onto the empty cursor; an arrow's chooses the next or the rune before of
/// the hero's class's list; the button's, with no rune made yet and the gold for the count's
/// price, takes the price and makes the count's rune of the slots' runes; the result's slot's
/// gives the rune made onto the empty cursor by the left button, into the backpack by the right.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn press_exchange(
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    (master, mut pack, held): (Option<ResMut<Master>>, ResMut<Pack>, Res<HeldArt>),
    (tables, arts, rules, images): (
        Option<Res<Tables>>,
        Option<Res<HeroArts>>,
        Res<crate::fight::Rules>,
        Res<UiImages>,
    ),
    (mut choice, mut dice): (ResMut<RuneChoice>, ResMut<crate::drops::Dice>),
) {
    let (Some(mut master), Some(tables), Some(arts)) = (master, tables, arts) else {
        return;
    };
    let (left, right) = (
        buttons.just_pressed(MouseButton::Left),
        buttons.just_pressed(MouseButton::Right),
    );
    if !(left || right) || master.page != EXCHANGE_PAGE || held.0.is_some() {
        return;
    }
    let Some(at) = crate::inventory::screen_point(&pointer, &ui_scale, &windows) else {
        return;
    };
    let width = images.size(SLOT_FILLED).x;
    let runes = &mut master.exchange;
    if let Some(k) = (0..exchange::SLOTS)
        .find(|&k| control(&images, SLOT_FILLED, slot_at(k, width)).contains(at))
    {
        let rune_held = pack
            .held
            .as_ref()
            .is_some_and(|held| tables.category(held.dropped.item) == RUNES);
        if pack.held.is_some() && !rune_held {
            return;
        }
        let out = runes.slots[k].take();
        runes.slots[k] = pack.held.take();
        pack.held = out;
        return;
    }
    let result = control(&images, SLOT_FILLED, RESULT_AT);
    if result.contains(at) {
        if pack.held.is_some() {
            return;
        }
        let Some(dropped) = runes.result.take() else {
            return;
        };
        let size = tables.size(dropped.item);
        if left {
            pack.hold(dropped, size);
        } else if !pack.take(dropped.clone(), size) {
            runes.result = Some(dropped);
        }
        return;
    }
    let (back, next) = arrows(&images);
    if runes.count() == exchange::SLOTS
        && runes.result.is_none()
        && (back.contains(at) || next.contains(at))
    {
        let count =
            exchange::list(Some(arts.kind), &arts.tables, |rune| tables.rune_art(rune)).len();
        choice.0 = if next.contains(at) {
            choice.0.next(count)
        } else {
            choice.0.previous(count)
        };
        return;
    }
    if !BUTTON.contains(at) || runes.result.is_some() {
        return;
    }
    let count = runes.count();
    let price = rules.0.exchange_price(count);
    if price > pack.gold {
        return;
    }
    // The gold's handler's sound whatever the price, 0 below two runes (`0x57FD80`).
    pack.add_gold(-i64::from(price));
    let art_of = |rune: u32| tables.rune_art(rune);
    let made = match exchange::made(count) {
        exchange::Made::Nothing => None,
        exchange::Made::AnyClass => Some(exchange::draw(
            &exchange::list(None, &arts.tables, art_of),
            &mut dice.0,
        )),
        exchange::Made::OwnClass => Some(exchange::draw(
            &exchange::list(Some(arts.kind), &arts.tables, art_of),
            &mut dice.0,
        )),
        exchange::Made::Chosen => chosen(&tables, &arts, choice.0),
    };
    let Some(rune) = made else {
        return;
    };
    info!("master: {count} runes exchanged for {rune}, {price} gold");
    runes.slots = Default::default();
    runes.result = Some(rune_item(rune));
}

/// What the window shows, made again as it changes.
#[derive(Clone, Debug, PartialEq)]
struct Look {
    master: Master,
    gold: u32,
    combo_held: bool,
    /// The rune that the result's slot shows, made or chosen.
    preview: Option<u32>,
}

/// Makes the window as the master shows it and as it changes, and takes it away as it goes.
#[expect(
    clippy::too_many_lines,
    reason = "the window's pieces read best in their order"
)]
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn show(
    mut commands: Commands,
    (master, pack, held): (Option<Res<Master>>, Res<Pack>, Res<HeldArt>),
    (images, assets, texts): (Res<UiImages>, Res<AssetServer>, Option<Res<Texts>>),
    (arts, portraits): (Option<Res<HeroArts>>, Option<Res<crate::talk::Portraits>>),
    (tables, choice, rules): (
        Option<Res<Tables>>,
        Res<RuneChoice>,
        Res<crate::fight::Rules>,
    ),
    (mut targets, mut layers, window): (
        ResMut<Assets<Image>>,
        ResMut<Assets<Layer>>,
        Option<Single<&Window>>,
    ),
    roots: Query<Entity, Or<(With<WindowRoot>, With<RuneScene>)>>,
    mut was: Local<Option<Look>>,
) {
    let now = master.as_deref().map(|master| Look {
        master: master.clone(),
        gold: pack.gold,
        combo_held: held.0.is_some_and(crate::combos::is_combo),
        preview: tables
            .as_deref()
            .zip(arts.as_deref())
            .and_then(|(tables, arts)| preview(master, tables, arts, choice.0)),
    });
    if *was == now {
        return;
    }
    *was = now.clone();
    for root in &roots {
        commands.entity(root).despawn();
    }
    let Some(look) = now else {
        return;
    };
    let master = &look.master;
    let said = |key: &str| {
        texts
            .as_deref()
            .and_then(|texts| texts.named(key))
            .unwrap_or(key)
            .to_owned()
    };
    let font: Handle<Font> = assets.load(FONT);
    let root = commands
        .spawn((WindowRoot, crate::backpack::screen(), GlobalZIndex(-1)))
        .id();
    let add = |commands: &mut Commands, bundle: (Node, ImageNode)| {
        let child = commands.spawn((bundle, Pickable::IGNORE)).id();
        commands.entity(root).add_child(child);
    };
    for panel in [RIGHT_PANEL, LEFT_PANEL] {
        let mut node = bar::placed(panel.min, panel.size());
        node.border = UiRect::all(Val::Px(2.0));
        let child = commands
            .spawn((
                node,
                BackgroundColor(PANEL),
                BorderColor::all(EDGE),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(root).add_child(child);
    }
    // The master's portrait by its type (`0x6D1180`): an interface image of its own, else its
    // dialogue's portrait; and its title.
    if let Some(&(_, index)) = OWN_PORTRAITS.iter().find(|(kind, _)| *kind == master.kind) {
        let (image, node) = bar::piece(&images, &assets, index, PORTRAIT, Vec2::ZERO);
        add(&mut commands, (node, image));
    } else if let Some((file, rect)) = portraits.as_deref().and_then(|p| p.of(master.kind)) {
        let [x, y, w, h] = rect.map(f32::from);
        let mut image = ImageNode::new(crate::hud::image(&assets, file.to_owned()));
        image.rect = Some(Rect::new(x, y, x + w, y + h));
        add(
            &mut commands,
            (bar::placed(PORTRAIT, Vec2::new(w, h)), image),
        );
    }
    let write = |commands: &mut Commands,
                 (text, at, width): (&str, Vec2, f32),
                 (colour, size, justify): (u32, f32, JustifyContent)| {
        let mut node = bar::placed(at, Vec2::new(width, size + 4.0));
        node.flex_direction = FlexDirection::Row;
        node.justify_content = justify;
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
    let (left, centre) = (JustifyContent::FlexStart, JustifyContent::Center);
    write(
        &mut commands,
        (&said(TITLE), TITLE_AT, RIGHT_PANEL.width()),
        (TITLE_COLOUR, FONT_SIZE, centre),
    );
    // The tab sheet's tabs, the COMBO page's showing.
    let labels: Vec<String> = TABS.iter().map(|key| said(key)).collect();
    let sheet = commands
        .spawn((
            bar::placed(SHEET, Vec2::new(640.0, 256.0)),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(root).add_child(sheet);
    let (tabs, each) =
        crate::inventory::spawn_tabs(&mut commands, (&images, &assets), &font, &labels);
    commands.entity(sheet).add_child(tabs);
    for (tab, page) in each.into_iter().zip(0_u8..) {
        commands.entity(tab).insert(MasterTab(page));
    }
    if master.page == EXCHANGE_PAGE {
        let scale = window.map_or(1.0, |window| window.height() / bar::SCREEN.y);
        let page = ExchangeLook {
            look: &look,
            tables: tables.as_deref(),
            price: rules.0.exchange_price(master.exchange.count()),
        };
        show_exchange(
            &mut commands,
            (root, &font, scale),
            (&images, &assets, texts.as_deref()),
            (&mut targets, &mut layers),
            &page,
        );
        return;
    }
    // The COMBO page, its title in its text's own colour.
    let make_title = said(MAKE_TITLE);
    let (make_title, colour) = tagged(&make_title);
    write(
        &mut commands,
        (make_title, MAKE_TITLE_AT, 256.0),
        (colour.unwrap_or(YELLOW), FONT_SIZE, left),
    );
    let piece = |index: u32, at: Vec2| {
        let (image, node) = bar::piece(&images, &assets, index, at, Vec2::ZERO);
        (node, image)
    };
    let frame = if look.combo_held { FRAME_LIT } else { FRAME };
    add(&mut commands, piece(frame, SYMBOL_FRAME.min));
    let Some(id) = master.combo else {
        return;
    };
    if let Some([_, _, vivid]) = crate::combos::sprites(id) {
        let (mut node, mut image) = piece(vivid, SYMBOL_AT);
        node.width = Val::Px(SYMBOL);
        node.height = Val::Px(SYMBOL);
        image.image_mode = NodeImageMode::Stretch;
        add(&mut commands, (node, image));
    }
    let gold = said(GOLD);
    for k in 0..STEPS {
        let at = step_at(k);
        add(&mut commands, piece(STEP_FRAME, at));
        let step = master.steps[k];
        if let Some(sprites) = step.and_then(|(art, _)| {
            arts.as_deref()
                .and_then(|arts| crate::spells_page::sprites(arts, art))
        }) {
            let (mut node, mut image) = piece(sprites[2], at);
            node.width = Val::Px(SYMBOL);
            node.height = Val::Px(SYMBOL);
            image.image_mode = NodeImageMode::Stretch;
            add(&mut commands, (node, image));
        }
        if k + 1 < STEPS {
            let arrow =
                ARROWS_FROM + Vec2::new(STEP_APART * f32::from(u8::try_from(k).unwrap_or(0)), 0.0);
            add(&mut commands, piece(ARROW, arrow));
        }
        // A changed step's price under it, 100 times its level.
        if let Some((_, level)) = step.filter(|_| step != master.old[k]) {
            let place =
                PRICES_FROM + Vec2::new(STEP_APART * f32::from(u8::try_from(k).unwrap_or(0)), 0.0);
            let price = (100 * u32::from(level)).to_string();
            write(
                &mut commands,
                (&price, place, PRICE_WIDTH),
                (WHITE, SMALL, centre),
            );
            write(
                &mut commands,
                (&gold, place + Vec2::new(0.0, GOLD_BELOW), PRICE_WIDTH),
                (WHITE, SMALL, centre),
            );
        }
    }
    // The art lines.
    if let (Some(arts), Some(texts)) = (arts.as_deref(), texts.as_deref()) {
        let level = said(LEVEL);
        for (row, &(art, step_level)) in master.steps.iter().flatten().enumerate() {
            let name = crate::combos::art_name(art, arts, texts)
                .map_or_else(|| art.to_string(), str::to_owned);
            let line = format!("{name} - {level} {step_level}");
            let y = LINE_APART * f32::from(u8::try_from(row).unwrap_or(0));
            write(
                &mut commands,
                (&line, LINES_FROM + Vec2::new(0.0, y), 340.0),
                (WHITE, FONT_SIZE, left),
            );
        }
    }
    // The button, the total and the count.
    let (cost, lit) = priced(master, look.gold);
    add(
        &mut commands,
        piece(if lit { BUTTON_LIT } else { BUTTON_GREY }, BUTTON.min),
    );
    let held_steps = master.steps.iter().flatten().count();
    if held_steps > 0 {
        // Yellow where a press makes the combo, where the original's is yellow at the cost too.
        let colour = if combos::makes(cost, look.gold) {
            YELLOW
        } else {
            RED
        };
        write(
            &mut commands,
            (&cost.to_string(), TOTAL_AT, TOTAL_WIDTH),
            (colour, SMALL, centre),
        );
        write(
            &mut commands,
            (&gold, TOTAL_AT + Vec2::new(0.0, GOLD_BELOW), TOTAL_WIDTH),
            (colour, SMALL, centre),
        );
    }
    if held_steps > 1 {
        write(
            &mut commands,
            (&format!("x{held_steps}"), COUNT_AT, 32.0),
            (WHITE, SMALL, left),
        );
    }
}

/// The runes' camera, light and models.
#[derive(Component, Clone)]
struct RuneScene;

/// What the EXCHANGE page shows: the window's look, the items' tables and the count's price.
struct ExchangeLook<'a> {
    look: &'a Look,
    tables: Option<&'a Tables>,
    price: u32,
}

/// A line of `runs` of text and colour in a row from `at`, `width` wide, its texts `justify`d.
fn write_runs(
    commands: &mut Commands,
    (root, font): (Entity, &Handle<Font>),
    (runs, at, width): (&[(String, u32)], Vec2, f32),
    (size, justify): (f32, JustifyContent),
) {
    let mut node = bar::placed(at, Vec2::new(width, size + 4.0));
    node.flex_direction = FlexDirection::Row;
    node.justify_content = justify;
    let line = commands.spawn((node, Pickable::IGNORE)).id();
    for (text, colour) in runs {
        let child = commands
            .spawn((
                Text::new(text.clone()),
                TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(size),
                    ..default()
                },
                TextLayout {
                    linebreak: LineBreak::NoWrap,
                    ..default()
                },
                TextColor(argb(*colour)),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(line).add_child(child);
    }
    commands.entity(root).add_child(line);
}

/// Makes the EXCHANGE page (`0x6D49A0`): its title in its text's colours, the slots with their
/// runes, the result's slot with the rune made or chosen, the arrows where four runes wait and
/// none is made, the button lit where a press makes a rune (`0x6D4610`: none made, two runes or
/// more, the price within the gold), and the price where it is not 0.
fn show_exchange(
    commands: &mut Commands,
    (root, font, scale): (Entity, &Handle<Font>, f32),
    (images, assets, texts): (&UiImages, &AssetServer, Option<&Texts>),
    (targets, layers): (&mut Assets<Image>, &mut Assets<Layer>),
    page: &ExchangeLook,
) {
    let master = &page.look.master;
    let runes = &master.exchange;
    let add = |commands: &mut Commands, index: u32, at: Vec2| {
        let piece = commands
            .spawn((
                bar::piece(images, assets, index, at, Vec2::ZERO),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(root).add_child(piece);
    };
    if let Some(title) = texts.and_then(|texts| texts.named(EXCHANGE_TITLE)) {
        write_runs(
            commands,
            (root, font),
            (
                &crate::tooltip::coloured(title),
                MAKE_TITLE_AT,
                RIGHT_PANEL.min.x,
            ),
            (FONT_SIZE, JustifyContent::FlexStart),
        );
    }
    let width = images.size(SLOT_FILLED).x;
    let mut models: Vec<(u32, Vec2)> = Vec::new();
    for (k, slot) in runes.slots.iter().enumerate() {
        let at = slot_at(k, width);
        add(
            commands,
            if slot.is_some() {
                SLOT_FILLED
            } else {
                SLOT_EMPTY
            },
            at,
        );
        if let Some(held) = slot {
            models.push((held.dropped.item, control(images, SLOT_FILLED, at).center()));
        }
    }
    add(commands, SLOT_FILLED, RESULT_AT);
    if let Some(rune) = page.look.preview {
        models.push((rune, control(images, SLOT_FILLED, RESULT_AT).center()));
    }
    if runes.count() == exchange::SLOTS && runes.result.is_none() {
        let (back, next) = arrows(images);
        add(commands, ARROW_LEFT, back.min);
        add(commands, ARROW_RIGHT, next.min);
    }
    let gold = page.look.gold;
    let lit = runes.result.is_none() && runes.count() > 1 && page.price <= gold;
    add(
        commands,
        if lit { BUTTON_LIT } else { BUTTON_GREY },
        BUTTON.min,
    );
    if page.price != 0 {
        let colour = if page.price <= gold { YELLOW } else { RED };
        let word = texts
            .and_then(|texts| texts.named(GOLD))
            .unwrap_or(GOLD)
            .to_owned();
        for (line, at) in [
            (word, EXCHANGE_PRICE.min),
            (
                page.price.to_string(),
                EXCHANGE_PRICE.min + Vec2::new(0.0, EXCHANGE_PRICE.height()),
            ),
        ] {
            write_runs(
                commands,
                (root, font),
                (&[(line, colour)], at, EXCHANGE_PRICE.width()),
                (SMALL, JustifyContent::Center),
            );
        }
    }
    let Some(tables) = page.tables.filter(|_| !models.is_empty()) else {
        return;
    };
    // The runes' models over the whole window, each at its slot's centre.
    let picture = commands
        .spawn((bar::placed(Vec2::ZERO, WINDOW.size()), Pickable::IGNORE))
        .id();
    commands.entity(root).add_child(picture);
    crate::backpack::spawn_picture(
        commands,
        (targets, layers),
        scale,
        (picture, WINDOW.size()),
        (RUNES_LAYER, RuneScene),
    );
    for (rune, centre) in models {
        let Some((model, texture, pose)) = tables.look(rune) else {
            continue;
        };
        let mut at = crate::backpack::posed(Vec3::new(centre.x, -centre.y, 0.0), pose);
        at.scale *= RUNE_SCALE;
        let mut entity = commands.spawn((
            RuneScene,
            crate::backpack::Matte,
            crate::models::scene(assets, model),
            at,
            RenderLayers::layer(RUNES_LAYER),
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(texture) = texture {
            entity.insert(crate::npcs::Retexture::load(assets, texture));
        }
    }
}

/// The rune under the point `at` whose tooltip the EXCHANGE page shows (`0x6D49A0`): a slot's
/// rune, or the result's slot's rune made or chosen, each at its slot's corner and 92 along, 16
/// down: its key there, the corner and the rune.
pub(crate) fn hovered(
    master: &Master,
    (tables, arts, choice): (&Tables, &HeroArts, &RuneChoice),
    images: &UiImages,
    at: Vec2,
) -> Option<(u32, Vec2, Dropped)> {
    if master.page != EXCHANGE_PAGE {
        return None;
    }
    let width = images.size(SLOT_FILLED).x;
    for (k, slot) in (0_u32..).zip(&master.exchange.slots) {
        let corner = slot_at(usize::try_from(k).ok()?, width);
        if control(images, SLOT_FILLED, corner).contains(at) {
            return slot
                .as_ref()
                .map(|held| (k, corner + RUNE_TIP_FROM, held.dropped.clone()));
        }
    }
    if control(images, SLOT_FILLED, RESULT_AT).contains(at) {
        let rune = master
            .exchange
            .result
            .clone()
            .or_else(|| preview(master, tables, arts, choice.0).map(rune_item))?;
        return Some((0x10 + rune.item, RESULT_AT + RUNE_TIP_FROM, rune));
    }
    None
}

/// The controls' tooltips (the style `0x209`): the symbol's frame's while no combo stands in it,
/// an empty step's and the button's.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn tooltip(
    mut commands: Commands,
    time: Res<Time<Real>>,
    master: Option<Res<Master>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    (assets, texts, images): (Res<AssetServer>, Option<Res<Texts>>, Res<UiImages>),
    (tables, arts, choice): (Option<Res<Tables>>, Option<Res<HeroArts>>, Res<RuneChoice>),
    tips: Query<Entity, With<TipRoot>>,
    mut rest: Local<Rest>,
) {
    let at = master
        .as_deref()
        .and_then(|_| crate::inventory::screen_point(&pointer, &ui_scale, &windows));
    // The EXCHANGE page's: an empty slot's, the button's and the empty result's slot's, by
    // number from 10; a slot's rune's and the result's tooltip are the items' (`tooltip`).
    let exchange_hint = |master: &Master, at: Vec2| {
        let width = images.size(SLOT_FILLED).x;
        if BUTTON.contains(at) {
            return Some(10);
        }
        if control(&images, SLOT_FILLED, RESULT_AT).contains(at) {
            let shown = master.exchange.result.is_some()
                || tables
                    .as_deref()
                    .zip(arts.as_deref())
                    .and_then(|(tables, arts)| preview(master, tables, arts, choice.0))
                    .is_some();
            return (!shown).then_some(11);
        }
        (0..exchange::SLOTS)
            .find(|&k| control(&images, SLOT_FILLED, slot_at(k, width)).contains(at))
            .filter(|&k| master.exchange.slots[k].is_none())
            .and_then(|k| u32::try_from(k + 12).ok())
    };
    // The controls by number: 0 the frame, 1 the button, 2 and on the steps.
    let over = master.as_deref().zip(at).and_then(|(master, at)| {
        if master.page == EXCHANGE_PAGE {
            exchange_hint(master, at)
        } else if SYMBOL_FRAME.contains(at) && master.combo.is_none() {
            Some(0)
        } else if BUTTON.contains(at) {
            Some(1)
        } else {
            step_under(at)
                .filter(|&k| master.combo.is_some() && master.steps[k].is_none())
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
    let width = images.size(SLOT_FILLED).x;
    let (key, corner) = match control {
        0 => (TIP_FRAME, SYMBOL_FRAME.min + TIP_FROM),
        1 => (TIP_BUTTON, BUTTON.min + TIP_FROM),
        10 => (TIP_EXCHANGE, BUTTON.min + HINT_FROM),
        11 => (TIP_RESULT, RESULT_AT + HINT_FROM),
        k @ 12.. => (
            TIP_SLOT,
            slot_at(usize::try_from(k - 12).unwrap_or(0), width) + HINT_FROM,
        ),
        k => (
            TIP_STEP,
            step_at(usize::try_from(k - 2).unwrap_or(0)) + TIP_FROM,
        ),
    };
    if let Some(text) = texts.as_deref().and_then(|texts| texts.named(key)) {
        spawn_tip(&mut commands, &assets, text, corner);
    }
}

/// The combat master's window.
pub struct MasterPlugin;

impl Plugin for MasterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (follow, select_tab, press, press_exchange, show, tooltip)
                .chain()
                .after(crate::inventory::Opened)
                .run_if(in_state(AppState::InGame)),
        )
        .init_resource::<RuneChoice>()
        .add_systems(OnExit(AppState::InGame), |mut commands: Commands| {
            commands.remove_resource::<Master>();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn master(old: [Step; STEPS], steps: [Step; STEPS]) -> Master {
        Master {
            creature: Entity::PLACEHOLDER,
            kind: 317,
            combo: Some(0x200),
            old,
            steps,
            opened: false,
            page: COMBO_PAGE,
            exchange: Exchange::default(),
        }
    }

    #[test]
    fn the_button_lights_where_a_press_makes_the_combo_above_the_cost() {
        let made = master(
            [None; STEPS],
            [Some((1070, 3)), Some((18, 2)), Some((1020, 1)), None],
        );
        // Three steps, each changed: 3 · (300 + 200 + 100) = 1,800 (seen). The gold equal to
        // the cost makes nothing, so the button stays grey (#1743), where the original's lights.
        assert_eq!(priced(&made, 5_000), (1_800, true));
        assert_eq!(priced(&made, 1_801), (1_800, true));
        assert_eq!(priced(&made, 1_800), (1_800, false));
        let empty = master([None; STEPS], [None; STEPS]);
        assert_eq!(priced(&empty, 5_000), (0, false));
    }

    #[test]
    fn a_texts_colour_tag_gives_its_colour() {
        assert_eq!(
            tagged("<colorl=FFFFFF80>Words"),
            ("Words", Some(0xFFFF_FF80))
        );
        assert_eq!(tagged("Words"), ("Words", None));
    }

    #[test]
    fn the_steps_stand_88_apart() {
        assert_eq!(step_under(Vec2::new(20.0, 220.0)), Some(0));
        assert_eq!(step_under(Vec2::new(280.0, 270.0)), Some(3));
        assert_eq!(step_under(Vec2::new(85.0, 220.0)), None);
    }
}
