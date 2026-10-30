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
//! Not followed: the EXCHANGE page (`cUI_MasterLotto`), the window's sounds, the spells page's
//! view under the master (the Vampiress's arts not greyed), and a combo on the page's tooltip.

use bevy::prelude::*;
use opensacred_rules::combos::{self, STEPS, Step};

use crate::art_slots::HeldArt;
use crate::arts::HeroArts;
use crate::bar::{self, UiImages};
use crate::combos::HeroCombos;
use crate::game::{AppState, FONT};
use crate::hud::Pointer;
use crate::inventory::{Page, Shown};
use crate::lines::Texts;
use crate::pickup::Pack;
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
/// The portrait's corner in the right panel, the title's line and colour (`0xFF0050FF`).
const PORTRAIT: Vec2 = Vec2::new(528.0, 20.0);
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

/// The master whose window shows, and its COMBO page: the combo dropped on it, its steps as they
/// stood and as they are made.
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
}

/// The window's container.
#[derive(Component)]
struct WindowRoot;

/// Shows the master's window for `creature` with the inventory (`0x52AB70`).
pub fn open(world: &mut World, creature: Entity) {
    let kind = world
        .get::<crate::ai::Mind>(creature)
        .map_or(0, |mind| mind.creature);
    info!("master: {creature}, a {kind}, shows its window");
    world.insert_resource(Master {
        creature,
        kind,
        combo: None,
        old: [None; STEPS],
        steps: [None; STEPS],
        opened: true,
    });
    if let Some(mut shown) = world.get_resource_mut::<Shown>()
        && !shown.0
    {
        shown.0 = true;
    }
}

/// A text of `global.res` without the colour's tag it may start with (`<colorl=FFFFFF80>`), and
/// that colour, an ARGB word.
fn tagged(text: &str) -> (&str, Option<u32>) {
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

/// What the cost of `master`'s steps is, whether the button lights, and whether a press makes
/// the combo, at `gold` (`0x6D23E0`, `0x6D26D0`, `0x6D22A0`).
fn priced(master: &Master, gold: u32) -> (u32, bool, bool) {
    let cost = combos::cost(&master.old, &master.steps);
    let lit = master.combo.is_some() && combos::affordable(&master.steps, cost, gold);
    (cost, lit, lit && combos::makes(cost, gold))
}

/// Keeps the window with the inventory: the combos page selected as it opens, the master gone
/// as the inventory hides.
fn follow(
    mut commands: Commands,
    shown: Res<Shown>,
    master: Option<ResMut<Master>>,
    mut page: ResMut<Page>,
) {
    let Some(mut master) = master else {
        return;
    };
    if !shown.0 {
        commands.remove_resource::<Master>();
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
    if !(left || right) {
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
        let (cost, _, makes) = priced(&master, pack.gold);
        let Some(index) = master.combo.and_then(combos::index_of).filter(|_| makes) else {
            return;
        };
        let steps = master.steps;
        combos.0[index].set(steps, &arts.tables);
        pack.gold -= cost;
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

/// What the window shows, made again as it changes.
#[derive(Clone, Debug, PartialEq)]
struct Look {
    master: Master,
    gold: u32,
    combo_held: bool,
}

/// Makes the window as the master shows it and as it changes, and takes it away as it goes.
#[expect(
    clippy::too_many_lines,
    reason = "the window's pieces read best in their order"
)]
fn show(
    mut commands: Commands,
    (master, pack, held): (Option<Res<Master>>, Res<Pack>, Res<HeldArt>),
    (images, assets, texts): (Res<UiImages>, Res<AssetServer>, Option<Res<Texts>>),
    (arts, portraits): (Option<Res<HeroArts>>, Option<Res<crate::talk::Portraits>>),
    roots: Query<Entity, With<WindowRoot>>,
    mut was: Local<Option<Look>>,
) {
    let now = master.as_deref().map(|master| Look {
        master: master.clone(),
        gold: pack.gold,
        combo_held: held.0.is_some_and(crate::combos::is_combo),
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
    // The master's portrait by its type, and its title.
    if let Some((file, rect)) = portraits.as_deref().and_then(|p| p.of(master.kind)) {
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
    let (tabs, _) = crate::inventory::spawn_tabs(&mut commands, (&images, &assets), &font, &labels);
    commands.entity(sheet).add_child(tabs);
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
    let (cost, lit, _) = priced(master, look.gold);
    add(
        &mut commands,
        piece(if lit { BUTTON_LIT } else { BUTTON_GREY }, BUTTON.min),
    );
    let held_steps = master.steps.iter().flatten().count();
    if held_steps > 0 {
        let colour = if cost <= look.gold { YELLOW } else { RED };
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

/// The controls' tooltips (the style `0x209`): the symbol's frame's while no combo stands in it,
/// an empty step's and the button's.
fn tooltip(
    mut commands: Commands,
    time: Res<Time<Real>>,
    master: Option<Res<Master>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    (assets, texts): (Res<AssetServer>, Option<Res<Texts>>),
    tips: Query<Entity, With<TipRoot>>,
    mut rest: Local<Rest>,
) {
    let at = master
        .as_deref()
        .and_then(|_| crate::inventory::screen_point(&pointer, &ui_scale, &windows));
    // The controls by number: 0 the frame, 1 the button, 2 and on the steps.
    let over = master.as_deref().zip(at).and_then(|(master, at)| {
        if SYMBOL_FRAME.contains(at) && master.combo.is_none() {
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
    let (key, corner) = match control {
        0 => (TIP_FRAME, SYMBOL_FRAME.min),
        1 => (TIP_BUTTON, BUTTON.min),
        k => (TIP_STEP, step_at(usize::try_from(k - 2).unwrap_or(0))),
    };
    if let Some(text) = texts.as_deref().and_then(|texts| texts.named(key)) {
        spawn_tip(&mut commands, &assets, text, corner + TIP_FROM);
    }
}

/// The combat master's window.
pub struct MasterPlugin;

impl Plugin for MasterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (follow, press, show, tooltip)
                .chain()
                .after(crate::inventory::Opened)
                .run_if(in_state(AppState::InGame)),
        )
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
        }
    }

    #[test]
    fn the_button_lights_at_the_cost_and_makes_above_it() {
        let made = master(
            [None; STEPS],
            [Some((1070, 3)), Some((18, 2)), Some((1020, 1)), None],
        );
        // Three steps, each changed: 3 · (300 + 200 + 100) = 1,800 (seen).
        assert_eq!(priced(&made, 5_000), (1_800, true, true));
        assert_eq!(priced(&made, 1_800), (1_800, true, false));
        assert_eq!(priced(&made, 1_799), (1_800, false, false));
        let empty = master([None; STEPS], [None; STEPS]);
        assert_eq!(priced(&empty, 5_000), (0, false, false));
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
