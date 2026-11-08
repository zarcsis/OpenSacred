//! The stables' horse window (#2300; docs/re/horses.md, "The stables and the horse traders",
//! "The horse window"): a talk to a stable's horse (its trader's stock, [`Stabled`]) or to a horse
//! trader (`quests::TRADER`, the talk stamp 180,000) opens `UI_WND_HORSE` (`cUI_Horse`,
//! `0x6BD2B0`) rather than a dialogue (`0x529930`).
//!
//! Its states (`0x6BE6E0`): on a horse, the Dwarf's refusal, the horse's sheet where the riding
//! test fails, else the sheet with the purchase; on a trader, the Dwarf's refusal, the hero
//! without a horse, the hero's horse more than 1,000 from the hero, else the hero's horse's sheet
//! with its sale and its healing. The sheet (`0x6BD3F0`): the owner's name over it, its portrait,
//! and the horse's class, level, pace, trample, health bonus, the hero's resistances, the riding
//! needed against the hero's, the healing's price and the price, as `riding::description` tells
//! them; its buttons, plain texts, yellow under the pointer (`0x754140`): the healing, the deal
//! and the leave.
//!
//! The purchase pays the asking price (`opensacred_rules::horses::asking`) and makes the horse
//! the hero's own, no stable's any longer; the sale gives the trader the hero's horse, a stable's
//! again, for two thirds of its asking price (`horses::sale`), a dead one for nothing; the healing
//! makes the horse whole for its price (`horses::healing`). The window closes by its leave and by
//! any deal; a press within it does not reach the world. It plays no sound of its own (its
//! events (9, `0x20`) to (9, `0x24`) have no case), the gold its coin.
//!
//! A stall that a purchase empties its trader fills again ([`restock`]) once the horse bought
//! is gone or stands 500 or more from it, the trader in the world.
//!
//! Not followed: the theft of another NPC's horse (the states 6 and 7, the effect 19), which no
//! NPC's horse in the game reaches yet (#2288, #2320).

use bevy::prelude::*;
use opensacred_rules::horses;

use crate::ai::Mind;
use crate::bar::{self, UiImages};
use crate::fight::Health;
use crate::game::{AppState, FONT};
use crate::hero::{Body, Hero};
use crate::npcs::{Dies, ScriptName};
use crate::pickup::Pack;
use crate::riding::{Horse, OwnHorse, Owned};
use crate::tooltip::argb;

/// A stable's horse, its trader's stock (`SetNPCState`'s or `CreateNPC`'s `0x63`, the talk stamp
/// 160,000 or 170,000): a talk to it opens the horse window rather than its mount; its trader by
/// its script name. Each horse of a stable keeps its trader here, where the original links the
/// trader with its last horse alone (docs/decisions/interface.md, "Horse window of its own
/// state").
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct Stabled {
    pub trader: String,
}

/// A stall that a purchase emptied, which its trader fills again (the stock's record,
/// `0xAACF80 + 0x79E8`): its trader's name, its horse's creature type, class and place, and
/// the horse bought from it.
#[derive(Clone, Debug, PartialEq)]
struct Stall {
    trader: String,
    kind: u32,
    class: u8,
    place: Vec3,
    horse: Entity,
}

/// The stalls that purchases emptied, until their traders fill them.
#[derive(Resource, Default, Debug)]
struct Emptied(Vec<Stall>);

/// How far the horse bought stands from its stall for the trader to fill it (`0x5382D8`).
const STALL_LEFT: f32 = 500.0;

/// The restock's rolls.
#[derive(Resource)]
struct Rolls(opensacred_rules::spawning::Rng);

impl Default for Rolls {
    fn default() -> Self {
        Self(crate::seed::dice(0x537270))
    }
}

/// The window's state (`0x6BE6E0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// The Dwarf, who never rides (16).
    Dwarf,
    /// A trader's, the hero's horse's sheet with its sale and its healing (1).
    Sell,
    /// A horse's sheet with its purchase (2).
    Buy,
    /// A trader's, the hero without a horse (3).
    NoHorse,
    /// A trader's, the hero's horse more than 1,000 from the hero (4).
    Far,
    /// A horse's sheet that the hero may not ride (5).
    CannotRide,
}

/// What the sheet shows of the horse ([`crate::riding::description`]'s facts) and its prices.
#[derive(Clone, Debug)]
struct Sheet {
    class: u8,
    level: u32,
    pace: u32,
    trample: horses::Trample,
    bonus: u32,
    resistances: [f32; 4],
    /// The riding skill needed, the hero's, and whether the hero may ride the horse.
    riding: (u32, u32, bool),
    asking: u32,
    healing: u32,
}

/// The open window: the horse and the trader it was opened on, its state, its title, the type of
/// its portrait's creature and the horse's sheet.
#[derive(Resource, Clone, Debug)]
pub(crate) struct Opened {
    horse: Option<Entity>,
    trader: Option<Entity>,
    state: State,
    title: String,
    portrait: Option<u32>,
    sheet: Option<Sheet>,
}

/// The window's root.
#[derive(Component)]
struct WindowRoot;

/// A button's text, by its index: the healing, the deal, the leave.
#[derive(Component, Clone, Copy)]
struct ButtonText(usize);

/// The window (`0x6BED90`): its panel, black at the alpha 159; its two separators; its title's
/// top; the portrait's place and size; the sheet's labels' left, its values' rect, its rows' first
/// top and their step, the healing's and the price's rows; the other states' text's rect.
const PANEL: Rect = Rect {
    min: Vec2::new(256.0, 204.0),
    max: Vec2::new(768.0, 568.0),
};
const PANEL_COLOUR: u32 = 0x9F00_0000;
const SEPARATORS: [f32; 2] = [236.0, 544.0];
const SEPARATOR_COLOUR: u32 = 0xFFAC_8A67;
const TITLE_TOP: f32 = 208.0;
const PORTRAIT_AT: Vec2 = Vec2::new(272.0, 268.0);
const PORTRAIT_SIZE: Vec2 = Vec2::new(96.0, 128.0);
const LABEL_X: f32 = 381.0;
const VALUE_X: f32 = 556.0;
const VALUE_WIDTH: f32 = 256.0;
const ROWS_FROM: f32 = 238.0;
const ROW_STEP: f32 = 24.0;
const HEALING_ROW: f32 = 486.0;
const PRICE_ROW: f32 = 510.0;
const TEXT_RECT: Rect = Rect {
    min: Vec2::new(381.0, 238.0),
    max: Vec2::new(731.0, 534.0),
};
/// The buttons' rects: the healing, the deal, the leave.
const BUTTONS: [Rect; 3] = [
    Rect {
        min: Vec2::new(266.0, 544.0),
        max: Vec2::new(386.0, 566.0),
    },
    Rect {
        min: Vec2::new(442.0, 544.0),
        max: Vec2::new(582.0, 566.0),
    },
    Rect {
        min: Vec2::new(648.0, 544.0),
        max: Vec2::new(748.0, 566.0),
    },
];
const WHITE: u32 = 0xFFFF_FFFF;
const YELLOW: u32 = 0xFFFF_FF00;
const RED: u32 = 0xFFFF_0000;
const FONT_SIZE: f32 = 20.0;
/// The resistances' images, the physical, fire, magic and poison.
const RESISTANCES: [u32; 4] = [209, 210, 211, 212];
/// The texts: the class's label and names from `9069 + 1`, the level's, the pace's, the riding's,
/// the healing's and the price's labels; the buttons' (the healing, the purchase, the sale, the
/// leave, the leave of the states without a sheet); the states' texts.
const CLASS_NAMES: u32 = 9069;
const CLASS_LABEL: &str = "9069";
const LEVEL_LABEL: &str = "1070";
const PACE_LABEL: &str = "1098";
const RIDING_LABEL: &str = "9421";
const HEALING_LABEL: &str = "9068";
const PRICE_LABEL: &str = "9067";
const HEAL: &str = "9066";
const BUY: &str = "9050";
const SELL: &str = "9051";
const LEAVE: &str = "9053";
const LEAVE_ALONE: &str = "1028";
const NO_HORSE: &str = "9063";
const FAR: &str = "9061";
const DWARF_TEXT: &str = "UI_DWARF_CANTRIDE";
/// How far from the hero its horse may stand for a trader to deal in it (`0x6BE775`).
const DEALS_WITHIN: f32 = 1_000.0;
/// The trading and the riding skills.
const TRADING: u8 = 21;
const RIDING: u8 = 22;
/// The balance's `HorseOverrideTime` where it lacks it.
const OVERRIDE_TIME: f32 = 10.0;

/// Opens the window on `creature`, a stable's horse or a horse trader (`0x529930`, the UI event
/// `0x11`): on a horse the horse itself and its trader its owner; on a trader the hero's own
/// horse.
pub(crate) fn open(world: &mut World, creature: Entity) {
    let horse = world.get::<Horse>(creature).is_some();
    let (horse, trader) = if horse {
        (Some(creature), None)
    } else {
        let owned = world.get_resource::<Owned>().and_then(|owned| owned.0);
        (owned, Some(creature))
    };
    let opened = opened(world, horse, trader);
    info!("stables: the horse window opens in {:?}", opened.state);
    world.insert_resource(opened);
}

/// The window of the horse `horse` and the trader `trader` as they stand now.
fn opened(world: &mut World, horse: Option<Entity>, trader: Option<Entity>) -> Opened {
    let hero_kind = world.get_resource::<Hero>().map_or(0, |hero| hero.creature);
    let skill = |skill: u8| {
        world
            .get_resource::<crate::arts::HeroArts>()
            .and_then(|arts| arts.skills.iter().find(|&&(s, _)| s == skill))
            .map_or(0, |&(_, level)| level)
    };
    let (trading, riding) = (skill(TRADING), skill(RIDING));
    let hero_level = world
        .get_resource::<crate::fight::Experience>()
        .map_or(1, |experience| experience.level);
    let mut heroes = world.query_filtered::<(&Transform, &crate::fight::Combat), With<Body>>();
    let (hero_at, resistances) = heroes
        .iter(world)
        .next()
        .map_or((Vec3::ZERO, [0.0; 4]), |(at, combat)| {
            (at.translation, combat.0.resistance)
        });
    // A horse alive in the world, its class, level, health and place.
    let horse_facts = horse.and_then(|horse| {
        let entity = world.get_entity(horse).ok()?;
        if entity.contains::<Dies>() {
            return None;
        }
        Some((
            entity.get::<Horse>()?.class,
            entity.get::<Mind>()?.level,
            *entity.get::<Health>()?,
            entity.get::<Transform>()?.translation,
            entity
                .get::<Stabled>()
                .map(|stabled| stabled.trader.clone()),
        ))
    });
    let named = |world: &mut World, name: &str| {
        let mut named = world.query::<(Entity, &ScriptName)>();
        named
            .iter(world)
            .find(|(_, script)| script.0.eq_ignore_ascii_case(name))
            .map(|(entity, _)| entity)
    };
    // The window's owner, whose name titles it and whose face it shows: the trader opened on, else
    // the horse's stable's trader.
    let owner = trader.or_else(|| {
        let stable = horse_facts.as_ref()?.4.clone()?;
        named(world, &stable)
    });
    let row = |class: u8| {
        world
            .get_resource::<crate::npcs::Npcs>()
            .map(|npcs| npcs.horse_row(class))
            .unwrap_or_default()
    };
    let balance = world
        .get_resource::<crate::game::Data>()
        .and_then(|data| data.0.tables.balance().ok());
    let prices = horses::Prices::of(balance);
    let time = balance
        .and_then(|balance| balance.f32("HorseOverrideTime"))
        .unwrap_or(OVERRIDE_TIME);
    let sheet = horse_facts.as_ref().map(|&(class, level, health, ..)| {
        let row = row(class);
        let asking = horses::asking(prices.price(class, level), trading);
        Sheet {
            class,
            level,
            pace: row.pace,
            trample: horses::trample(&row, (level, hero_level, riding), time),
            bonus: horses::health_bonus(&row, level, hero_level),
            resistances,
            riding: (row.riding, riding, horses::rides(hero_kind, riding, &row)),
            asking,
            healing: horses::healing(asking, (health.current, health.most)),
        }
    });
    let state = match (trader, &horse_facts, &sheet) {
        _ if hero_kind == horses::DWARF => State::Dwarf,
        (Some(_), None, _) => State::NoHorse,
        (Some(_), Some((.., at, _)), _) if (*at - hero_at).xz().length() > DEALS_WITHIN => {
            State::Far
        }
        (Some(_), Some(_), _) => State::Sell,
        (None, _, Some(sheet)) if !sheet.riding.2 => State::CannotRide,
        (None, ..) => State::Buy,
    };
    // The owner's name, its script name's text (`res:` and the text's number, `sdk_REFKEY`).
    let title = owner
        .and_then(|owner| world.get::<ScriptName>(owner))
        .and_then(|name| {
            let prefix = name.0.get(..4)?;
            prefix
                .eq_ignore_ascii_case("res:")
                .then(|| name.0[4..].to_owned())
        })
        .and_then(|key| {
            world
                .get_resource::<crate::lines::Texts>()
                .and_then(|texts| texts.named(&key).map(str::to_owned))
        })
        .unwrap_or_default();
    let portrait = owner.and_then(|owner| world.get::<Mind>(owner).map(|mind| mind.creature));
    Opened {
        horse,
        trader,
        state,
        title,
        portrait,
        sheet: matches!(state, State::Sell | State::Buy | State::CannotRide)
            .then_some(sheet)
            .flatten(),
    }
}

/// Whether the window shows the button `i` in its state, of the hero's gold `gold`
/// (`0x6BE6E0`, `0x6BEB90`): the healing on the trader's sheet where its price and the gold are
/// above 0; the deal to buy where the gold pays the price, and to sell; the leave always.
fn shown(opened: &Opened, i: usize, gold: u32) -> bool {
    let sheet = opened.sheet.as_ref();
    match i {
        0 => opened.state == State::Sell && sheet.is_some_and(|s| s.healing > 0) && gold > 0,
        1 => match opened.state {
            State::Buy => sheet.is_some_and(|s| s.asking <= gold),
            State::Sell => true,
            _ => false,
        },
        _ => true,
    }
}

/// The button `i`'s text in the window's state.
fn button_text(opened: &Opened, i: usize) -> &'static str {
    match i {
        0 => HEAL,
        1 if opened.state == State::Sell => SELL,
        1 => BUY,
        _ => match opened.state {
            State::Dwarf | State::NoHorse | State::Far | State::CannotRide => LEAVE_ALONE,
            State::Sell | State::Buy => LEAVE,
        },
    }
}

/// Draws the window anew as it opens or changes (`0x6BD3F0`): its panel, its separators, its
/// title, its sheet or its state's text, its buttons and the portrait.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn draw(
    mut commands: Commands,
    opened: Option<Res<Opened>>,
    roots: Query<Entity, With<WindowRoot>>,
    (images, assets): (Res<UiImages>, Res<AssetServer>),
    texts: Option<Res<crate::lines::Texts>>,
    portraits: Option<Res<crate::talk::Portraits>>,
    pack: Res<Pack>,
    mut was_open: Local<bool>,
) {
    let changed = opened.as_ref().is_some_and(|opened| opened.is_changed());
    if !changed && opened.is_some() == *was_open {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    *was_open = opened.is_some();
    let Some(opened) = opened else {
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
        .spawn((WindowRoot, crate::backpack::screen(), GlobalZIndex(1)))
        .id();
    let child = |commands: &mut Commands, bundle: (Node, BackgroundColor)| {
        let entity = commands.spawn((bundle, Pickable::IGNORE)).id();
        commands.entity(root).add_child(entity);
    };
    child(
        &mut commands,
        (
            bar::placed(PANEL.min, PANEL.size()),
            BackgroundColor(argb(PANEL_COLOUR)),
        ),
    );
    for y in SEPARATORS {
        child(
            &mut commands,
            (
                bar::placed(Vec2::new(PANEL.min.x, y), Vec2::new(PANEL.width(), 1.0)),
                BackgroundColor(argb(SEPARATOR_COLOUR)),
            ),
        );
    }
    let write = |commands: &mut Commands,
                 pieces: Vec<Piece>,
                 (at, width): (Vec2, f32),
                 justify: JustifyContent|
     -> Entity {
        let mut node = bar::placed(at, Vec2::new(width, FONT_SIZE + 4.0));
        node.flex_direction = FlexDirection::Row;
        node.justify_content = justify;
        node.align_items = AlignItems::Center;
        let row = commands.spawn((node, Pickable::IGNORE)).id();
        for piece in pieces {
            let entity = match piece {
                Piece::Text(text, colour) => commands
                    .spawn((
                        Text::new(text),
                        TextFont {
                            font: font.clone().into(),
                            font_size: FontSize::Px(FONT_SIZE),
                            ..default()
                        },
                        TextLayout {
                            linebreak: LineBreak::NoWrap,
                            ..default()
                        },
                        TextColor(argb(colour)),
                        Pickable::IGNORE,
                    ))
                    .id(),
                Piece::Image(index) => {
                    let (image, mut node) =
                        bar::piece(&images, &assets, index, Vec2::ZERO, Vec2::ZERO);
                    node.position_type = PositionType::Relative;
                    commands.spawn((image, node, Pickable::IGNORE)).id()
                }
            };
            commands.entity(row).add_child(entity);
        }
        commands.entity(root).add_child(row);
        row
    };
    let label = |text: String, colour: u32| vec![Piece::Text(text, colour)];
    match &opened.sheet {
        Some(sheet) => {
            write(
                &mut commands,
                label(opened.title.clone(), WHITE),
                (Vec2::new(PANEL.min.x, TITLE_TOP), PANEL.width()),
                JustifyContent::Center,
            );
            let whole = |x: f32| x.max(0.0).trunc().to_string();
            let (needed, skill, rides) = sheet.riding;
            let riding_colour = if rides { WHITE } else { RED };
            let resisted = |k: usize| {
                let mut pieces = Vec::new();
                for i in [k, k + 1] {
                    pieces.push(Piece::Image(RESISTANCES[i]));
                    pieces.push(Piece::Text(whole(sheet.resistances[i]), WHITE));
                }
                pieces
            };
            let rows: [(String, Vec<Piece>, u32); 10] = [
                (
                    said(CLASS_LABEL),
                    label(
                        said(&(CLASS_NAMES + u32::from(sheet.class)).to_string()),
                        WHITE,
                    ),
                    WHITE,
                ),
                (
                    said(LEVEL_LABEL),
                    label(sheet.level.to_string(), WHITE),
                    WHITE,
                ),
                (
                    said(PACE_LABEL),
                    label(sheet.pace.to_string(), WHITE),
                    WHITE,
                ),
                (
                    said("UI_TT_OVERRIDETIME"),
                    label(format!("{}s", whole(sheet.trample.seconds)), WHITE),
                    WHITE,
                ),
                (
                    said("UI_TT_OVERRIDEDAMAGE"),
                    label(whole(sheet.trample.damage), WHITE),
                    WHITE,
                ),
                (
                    said("UI_TT_OVERRIDEREGENERATION"),
                    label(format!("{}s", whole(sheet.trample.regeneration)), WHITE),
                    WHITE,
                ),
                (
                    said("UI_TT_OVERRIDEHPBONUS"),
                    label(format!("{}%", sheet.bonus), WHITE),
                    WHITE,
                ),
                (said("UI_TT_RESISTANCES"), resisted(0), WHITE),
                (String::new(), resisted(2), WHITE),
                (
                    said(RIDING_LABEL),
                    label(format!("{needed}/{skill}"), riding_colour),
                    riding_colour,
                ),
            ];
            for ((name, value, colour), y) in rows
                .into_iter()
                .zip((0_u8..).map(|k| ROWS_FROM + ROW_STEP * f32::from(k)))
            {
                write(
                    &mut commands,
                    label(name, colour),
                    (Vec2::new(LABEL_X, y), VALUE_WIDTH),
                    JustifyContent::FlexStart,
                );
                write(
                    &mut commands,
                    value,
                    (Vec2::new(VALUE_X, y), VALUE_WIDTH),
                    JustifyContent::Center,
                );
            }
            if opened.state == State::Sell {
                for (text, at) in [
                    (said(HEALING_LABEL), LABEL_X),
                    (sheet.healing.to_string(), VALUE_X),
                ] {
                    let justify = if at == LABEL_X {
                        JustifyContent::FlexStart
                    } else {
                        JustifyContent::Center
                    };
                    write(
                        &mut commands,
                        label(text, RED),
                        (Vec2::new(at, HEALING_ROW), VALUE_WIDTH),
                        justify,
                    );
                }
            }
            // The price: the sale's two thirds of the asking price on the trader's sheet, else the
            // asking price, red where the gold falls short (`0x6BDE64`).
            let (price, colour) = if opened.state == State::Sell {
                (horses::sale(sheet.asking), YELLOW)
            } else if sheet.asking > pack.gold {
                (sheet.asking, RED)
            } else {
                (sheet.asking, YELLOW)
            };
            write(
                &mut commands,
                label(said(PRICE_LABEL), colour),
                (Vec2::new(LABEL_X, PRICE_ROW), VALUE_WIDTH),
                JustifyContent::FlexStart,
            );
            write(
                &mut commands,
                label(price.to_string(), colour),
                (Vec2::new(VALUE_X, PRICE_ROW), VALUE_WIDTH),
                JustifyContent::Center,
            );
        }
        None => {
            let text = match opened.state {
                State::Dwarf => said(DWARF_TEXT),
                State::NoHorse => said(NO_HORSE),
                State::Far => said(FAR),
                _ => String::new(),
            };
            let entity = commands
                .spawn((
                    bar::placed(TEXT_RECT.min, TEXT_RECT.size()),
                    Text::new(text),
                    TextFont {
                        font: font.clone().into(),
                        font_size: FontSize::Px(FONT_SIZE),
                        ..default()
                    },
                    TextLayout::justify(Justify::Justified),
                    TextColor(argb(WHITE)),
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(root).add_child(entity);
        }
    }
    for (i, rect) in BUTTONS.iter().enumerate() {
        if !shown(&opened, i, pack.gold) {
            continue;
        }
        let row = write(
            &mut commands,
            label(said(button_text(&opened, i)), WHITE),
            (rect.min, rect.width()),
            JustifyContent::Center,
        );
        commands.entity(row).insert(ButtonText(i));
    }
    // The portrait of the window's owner by its type, as the dialogues' window shows it.
    if let Some((file, rect)) = opened
        .portrait
        .and_then(|kind| portraits.as_deref()?.of(kind))
    {
        let [x, y, w, h] = rect.map(f32::from);
        let mut image = ImageNode::new(crate::hud::image(&assets, file.to_owned()));
        image.rect = Some(Rect::new(x, y, x + w, y + h));
        let entity = commands
            .spawn((
                bar::placed(PORTRAIT_AT, PORTRAIT_SIZE),
                image,
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(root).add_child(entity);
    }
}

/// A piece of a row: a text in its colour, or an interface image.
enum Piece {
    Text(String, u32),
    Image(u32),
}

/// The buttons' texts yellow under the pointer, white else (`0x754140`).
fn hover(
    (pointer, ui_scale, windows): (Res<crate::hud::Pointer>, Res<UiScale>, Query<&Window>),
    buttons: Query<(&ButtonText, &Children)>,
    mut colours: Query<&mut TextColor>,
) {
    let at = crate::inventory::screen_point(&pointer, &ui_scale, &windows);
    for (button, children) in &buttons {
        let lit = at.is_some_and(|at| BUTTONS[button.0].contains(at));
        let colour = argb(if lit { YELLOW } else { WHITE });
        for &child in children {
            if let Ok(mut text) = colours.get_mut(child) {
                text.set_if_neq(TextColor(colour));
            }
        }
    }
}

/// A left press on a shown button runs it (`0x6BE340`, `0x6BE400`).
fn press(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale, windows): (Res<crate::hud::Pointer>, Res<UiScale>, Query<&Window>),
    opened: Option<Res<Opened>>,
    pack: Res<Pack>,
) {
    let Some(opened) = opened else {
        return;
    };
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(at) = crate::inventory::screen_point(&pointer, &ui_scale, &windows) else {
        return;
    };
    let Some(i) =
        (0..BUTTONS.len()).find(|&i| BUTTONS[i].contains(at) && shown(&opened, i, pack.gold))
    else {
        return;
    };
    commands.queue(move |world: &mut World| act(world, i));
}

/// Runs the button `i` (`0x6BE400`): the healing, the deal by the state, or the leave.
fn act(world: &mut World, i: usize) {
    let Some(opened) = world.get_resource::<Opened>().cloned() else {
        return;
    };
    match i {
        0 => heal(world, &opened),
        1 => {
            // The window hides first, then deals.
            world.remove_resource::<Opened>();
            match opened.state {
                State::Buy => buy(world, &opened),
                State::Sell => sell(world, &opened),
                _ => {}
            }
        }
        _ => {
            world.remove_resource::<Opened>();
        }
    }
}

/// The healing: the horse made whole for its price, where the gold pays it, the window drawn
/// again (its button gone with the price).
fn heal(world: &mut World, opened: &Opened) {
    let (Some(horse), Some(sheet)) = (opened.horse, opened.sheet.as_ref()) else {
        return;
    };
    let price = sheet.healing;
    let Some(mut pack) = world.get_resource_mut::<Pack>() else {
        return;
    };
    if pack.gold < price {
        return;
    }
    pack.add_gold(-i64::from(price));
    if let Some(mut health) = world.get_mut::<Health>(horse) {
        health.current = health.most;
    }
    info!("stables: {horse} healed for {price}");
    let again = self::opened(world, opened.horse, opened.trader);
    world.insert_resource(again);
}

/// The purchase: the asking price paid, the horse the hero's own and no stable's.
fn buy(world: &mut World, opened: &Opened) {
    let (Some(horse), Some(sheet)) = (opened.horse, opened.sheet.as_ref()) else {
        return;
    };
    if let Some(mut pack) = world.get_resource_mut::<Pack>() {
        pack.add_gold(-i64::from(sheet.asking));
    }
    // Its stall waits for its trader to fill it again (`0x538199`).
    let stall = world.get_entity(horse).ok().and_then(|entity| {
        Some(Stall {
            trader: entity.get::<Stabled>()?.trader.clone(),
            kind: entity.get::<Mind>()?.creature,
            class: entity.get::<Horse>()?.class,
            place: entity.get::<Transform>()?.translation,
            horse,
        })
    });
    if let Some(stall) = stall {
        world.get_resource_or_init::<Emptied>().0.push(stall);
    }
    if let Ok(mut entity) = world.get_entity_mut(horse) {
        entity.remove::<Stabled>();
    }
    crate::riding::own(world, horse);
    info!("stables: the hero buys {horse} for {}", sheet.asking);
}

/// The sale: the hero's horse let go and the trader's stable's again, its sector's once more, for
/// two thirds of its asking price; a dead one lost for nothing.
fn sell(world: &mut World, opened: &Opened) {
    let (Some(horse), Some(trader)) = (opened.horse, opened.trader) else {
        return;
    };
    if let Some(mut owned) = world.get_resource_mut::<Owned>() {
        owned.0 = None;
    }
    let alive = world
        .get_entity(horse)
        .is_ok_and(|entity| !entity.contains::<Dies>());
    let name = world.get::<ScriptName>(trader).map(|name| name.0.clone());
    let at = world.get::<Transform>(horse).map(|at| at.translation);
    if let Ok(mut entity) = world.get_entity_mut(horse) {
        entity.remove::<OwnHorse>();
        if alive && let Some(trader) = name {
            entity.insert(Stabled { trader });
        }
    }
    if let Some(at) = at {
        let sector = crate::music::sector_of(at.x, at.z);
        world.resource_scope(|world, mut streamer: Mut<crate::sectors::Streamer>| {
            let mut commands = world.commands();
            streamer.adopt(&mut commands, sector, horse);
        });
        world.flush();
    }
    let paid = opened
        .sheet
        .as_ref()
        .filter(|_| alive)
        .map_or(0, |sheet| horses::paid_for(sheet.asking));
    if let Some(mut pack) = world.get_resource_mut::<Pack>() {
        pack.add_gold(i64::from(paid));
    }
    info!("stables: the hero sells {horse} for {paid}");
}

/// Fills again each stall that a purchase emptied (the trader's update, `0x5375C0` at
/// `0x538199`–`0x53867D`), at a tick of the world, where its trader is in the world and the
/// horse bought is gone or dead or stands [`STALL_LEFT`] or more from it: a new horse of its type
/// at its place, of its class, its level the hero's held within the sector's
/// (`Npcs::restock_level`, `0x537270`), its trader's stock. A horse sold back joins no stall, as
/// the original's joins no record. Not followed: the new horse's gear (its slot 1's item) and
/// gold; the stalls are not saved.
fn restock(world: &mut World) {
    if world
        .get_resource::<crate::frame::WorldTick>()
        .is_none_or(|tick| tick.seconds.is_none())
    {
        return;
    }
    let Some(stalls) = world
        .get_resource::<Emptied>()
        .map(|emptied| emptied.0.clone())
    else {
        return;
    };
    if stalls.is_empty() {
        return;
    }
    let mut traders = world.query::<&ScriptName>();
    let present: Vec<String> = traders.iter(world).map(|name| name.0.clone()).collect();
    let hero = world
        .get_resource::<crate::fight::Experience>()
        .map_or(1, |experience| experience.level);
    let mut kept = Vec::new();
    for stall in stalls {
        let horse = world.get_entity(stall.horse).ok();
        let gone = horse.is_none_or(|horse| horse.contains::<Dies>());
        let far = horse
            .and_then(|horse| horse.get::<Transform>())
            .is_none_or(|at| (at.translation - stall.place).xz().length() >= STALL_LEFT);
        if !(gone || far) || !present.contains(&stall.trader) {
            kept.push(stall);
            continue;
        }
        let roll = world.get_resource_or_init::<Rolls>().0.below(2);
        let level = world
            .get_resource::<crate::npcs::Npcs>()
            .map_or(hero, |npcs| npcs.restock_level(hero, stall.place, roll));
        let (x, z) = crate::paths::cell_of(stall.place.x, stall.place.z);
        let (Ok(x), Ok(z)) = (u32::try_from(x), u32::try_from(z)) else {
            continue;
        };
        let request = crate::npcs::Spawn {
            creature: stall.kind,
            cells: vec![(x, z)],
            floor: 0,
            level: Some(level),
            rank: crate::fight::Rank::Common,
            hostile: Some(false),
            immortal: false,
            roams: false,
        };
        match world.run_system_cached_with(crate::npcs::spawn_creatures, request) {
            Ok(Ok(made)) => {
                for &horse in &made {
                    world.entity_mut(horse).insert((
                        Horse { class: stall.class },
                        Stabled {
                            trader: stall.trader.clone(),
                        },
                    ));
                    info!(
                        "stables: {} fills its stall with {horse} of level {level}",
                        stall.trader
                    );
                }
            }
            Ok(Err(error)) => warn!("stables: {} fills no stall: {error}", stall.trader),
            Err(error) => warn!("stables: {} fills no stall: {error}", stall.trader),
        }
    }
    if let Some(mut emptied) = world.get_resource_mut::<Emptied>() {
        emptied.0 = kept;
    }
}

/// Whether the pointer is off the open window, whose presses do not reach the world.
fn off_the_window(
    opened: Option<Res<Opened>>,
    (pointer, ui_scale, windows): (Res<crate::hud::Pointer>, Res<UiScale>, Query<&Window>),
) -> bool {
    opened.is_none()
        || !crate::inventory::screen_point(&pointer, &ui_scale, &windows)
            .is_some_and(|at| PANEL.contains(at))
}

/// The horse window.
pub struct StablesPlugin;

impl Plugin for StablesPlugin {
    fn build(&self, app: &mut App) {
        use crate::frame::HoldsControl;
        app.hold_control(off_the_window)
            .add_systems(OnExit(AppState::InGame), |mut commands: Commands| {
                commands.remove_resource::<Opened>();
                commands.insert_resource(Emptied::default());
            })
            .add_systems(
                Update,
                restock
                    .in_set(crate::frame::Unpaused)
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                Update,
                (press, draw, hover)
                    .chain()
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
