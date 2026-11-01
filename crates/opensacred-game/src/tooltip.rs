//! The items' tooltip (#1273; docs/re/inventory.md, §9; docs/re/interface.md, "The tooltip"): as
//! the pointer rests on an item of the backpack's grid, its tooltip shows at the item's right edge
//! and 16 and its top and 16 (`0x6C1480`, the manager's tooltip of the style `0xB`), kept 16
//! pixels within the screen (`0x6E7AF0`, the style's bit 8), until the pointer leaves the item.
//!
//! The window (`cUI_Tooltip`, `0x6E76C0`, `0x6E7AF0`, its border `0x727B30`): a black panel at
//! half alpha (**measured**), its text 4 pixels in, a line of 20 pixels, the font's height; its
//! width the text's and 8, its height the lines' and 4; and its border of `UI_STATIC_BORDER`s
//! about it: the image 21 (32 × 6) along its top from (4, −4) and its bottom from (4, H − 3), the
//! image 34 (6 × 32) down its left at −4 and its right at W, and the image 55 (38 × 18) at its
//! corners, the top right's at (W − 24, −11) as stored and the others turned about across, along
//! or both at (−13, −11), (W − 24, H − 8) and (−13, H − 8).

mod lines;

use bevy::prelude::*;
use opensacred_rules::drops::Dropped;

use crate::arts::HeroArts;
use crate::backpack::{CELL, GRID, cell_under, on_grid};
use crate::bar::{self, UiImages};
use crate::cursors::item_name;
use crate::drops::Tables;
use crate::fight::{Combat, Experience, Rules};
use crate::game::{AppState, FONT};
use crate::hero::Body;
use crate::hud::Pointer;
use crate::inventory::Shown;
use crate::lines::Texts;
use crate::pickup::Pack;

/// The panel's colour (**measured** 2026-10-30: black at half alpha), its text's inset and line,
/// and the font's size that fills the line (**estimate**).
pub(crate) const PANEL: Color = Color::linear_rgba(0.0, 0.0, 0.0, 0.5);
pub(crate) const INSET: f32 = 4.0;
const LINE: f32 = 20.0;
pub(crate) const FONT_SIZE: f32 = 20.0;
/// The tooltip's place from the item's top right, and how far it keeps within the screen.
const FROM_ITEM: Vec2 = Vec2::new(16.0, 16.0);
pub(crate) const MARGIN: f32 = 16.0;
/// The border's images: the edges along and down, and the corner.
const EDGE_ALONG: u32 = 21;
const EDGE_DOWN: u32 = 34;
const CORNER: u32 = 55;

/// A colour of an ARGB word, in the frame's sRGB values (`hud`).
pub(crate) fn argb(colour: u32) -> Color {
    let [a, r, g, b] = colour.to_be_bytes().map(|c| f32::from(c) / 255.0);
    Color::linear_rgba(r, g, b, a)
}

/// A piece of a tooltip's line: a text in its colour (`\c`, `\w`), or an image of the
/// interface tinted by a colour with its alpha (`\g`, the line advancing by its width).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Piece {
    Text(String, u32),
    Image(u32, u32),
}

/// A line of a tooltip: its pieces.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Line(pub Vec<Piece>);

impl Line {
    /// A line of one text in one colour.
    pub(crate) fn of(text: impl Into<String>, colour: u32) -> Self {
        Self(vec![Piece::Text(text.into(), colour)])
    }
}

/// What the tooltip weighs an item against: the hero's arts' table and skills, its level and its
/// fight's values, the items' tables, the fight's rules and the potions.
#[derive(bevy::ecs::system::SystemParam)]
struct Weighing<'w, 's> {
    tables: Option<Res<'w, Tables>>,
    rules: Res<'w, Rules>,
    arts: Option<Res<'w, HeroArts>>,
    experience: Res<'w, Experience>,
    potions: Option<Res<'w, crate::belt::Potions>>,
    heroes: Query<'w, 's, &'static Combat, With<Body>>,
}

impl Weighing<'_, '_> {
    /// The tooltip's lines of `dropped`, named `name`, of the texts `texts`, its price as the hero
    /// would buy it where `buying` (a merchant's, #1618), else as it would sell it.
    fn lines(
        &self,
        dropped: &Dropped,
        name: &str,
        texts: Option<&Texts>,
        buying: bool,
    ) -> Vec<Line> {
        let Some(tables) = self.tables.as_deref() else {
            return vec![Line::of(name, WHITE)];
        };
        let record = tables.record_of(dropped);
        let values = tables.values(dropped);
        let made = dropped.values.is_some();
        // A made item's levels by its own values (`0x5D3AB0` at `0x5D56FA`), another's as its
        // record holds them: a potion's none, a scroll's level none (seen, #1273); with its
        // sockets filled, at least theirs, the level asked of it (`0x5C0810`, #1661).
        let own = if made {
            opensacred_rules::fight::item_level(&tables.own_values(dropped), &self.rules)
        } else {
            u32::from(record.level)
        };
        let level = own.max(tables.sockets_level(dropped, &self.rules));
        let asked = if made || !dropped.socketed.is_empty() {
            opensacred_rules::equipment::asked_level(level)
        } else {
            u32::from(record.asked)
        };
        let quality = if dropped.quality != 0 {
            dropped.quality
        } else {
            u8::try_from((record.word >> 16) & 0xF).unwrap_or(0)
        };
        let item = lines::Item {
            item: dropped.item,
            name,
            category: tables.category(dropped.item),
            record,
            price: tables.trade_price(dropped),
            buying,
            quality,
            values,
            level,
            asked,
            heals: self
                .potions
                .as_deref()
                .and_then(|potions| potions.0.heals(dropped.item)),
        };
        let skills = self.arts.as_deref().map_or(&[][..], |arts| &arts.skills);
        let hero = self.heroes.iter().next().map(|combat| lines::Hero {
            creature: self.arts.as_deref().map_or(1, |arts| arts.kind),
            level: self.experience.0.level,
            attributes: combat.attributes,
            skills,
            multipliers: combat.multipliers,
        });
        let text = |name: &str| texts.and_then(|texts| texts.named(name)).map(str::to_owned);
        let art = |id: u32| {
            let arts = self.arts.as_deref()?;
            let id = u16::try_from(id).ok()?;
            let name = arts
                .tables
                .spells
                .iter()
                .find(|spell| spell.id == id)
                .map(|spell| spell.name)
                .or_else(|| {
                    arts.tables
                        .weapon_arts
                        .iter()
                        .find(|art| art.id == id)
                        .map(|art| art.name)
                })?;
            texts?.id(name).map(str::to_owned)
        };
        let words = lines::Words {
            text: &text,
            art: &art,
        };
        lines::lines(&item, hero.as_ref(), &words)
    }
}

/// White, the lines' default (`0xFFFFFFFF`).
pub(crate) const WHITE: u32 = 0xFFFF_FFFF;

/// The tooltips' container: the original's screen centred along the window's width, over the
/// windows.
#[derive(Component)]
struct Root;

/// Where the item that a tooltip shows lies: the backpack's grid, a page of a merchant's (#1618),
/// or a weapon set's slot of the bar (#1689).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Backpack,
    Merchant(opensacred_rules::trade::stock::Page),
    WeaponSet,
}

/// A weapon set's slot's item's tooltip's corner from the slot's, as the taskbar shows it
/// (`0x6E4560`): its middle less 64 along, 128 up.
const FROM_SLOT: Vec2 = Vec2::new(32.0 - 64.0, -128.0);

/// The tooltip shown: what it shows (the grid's item by its key), its place on the original's
/// screen before it is kept within it.
#[derive(Component)]
struct Panel {
    source: Source,
    key: u32,
    at: Vec2,
}

/// The merchant's window's page and its stock, whose items show their tooltips too.
type Merchant<'w> = (
    Option<Res<'w, crate::merchant::Merchant>>,
    Res<'w, crate::merchant::Showing>,
    Res<'w, crate::merchant::Wares>,
);

/// The tooltip of the grid's item under the pointer while the items' page shows, or of a
/// merchant's page's item with its price as bought (`0x6DAA70`), made as the pointer comes to an
/// item and taken away as it leaves it (`0x6C1480`); and of a weapon set's slot's item of the
/// bar, the hand by the half the pointer is on, whether the inventory shows or not (`0x6E4560`,
/// #1689).
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn grid(
    mut commands: Commands,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    (shown, page): (Option<Res<Shown>>, Res<crate::inventory::Page>),
    (pack, (merchant, showing, wares)): (Res<Pack>, Merchant),
    (sets, slots): (
        Option<Res<crate::sets::WeaponSets>>,
        Option<Res<bar::Shown>>,
    ),
    texts: Option<Res<Texts>>,
    weighing: Weighing,
    (images, assets): (Res<UiImages>, Res<AssetServer>),
    panels: Query<(Entity, &Panel)>,
    roots: Query<Entity, With<Root>>,
) {
    let shown = shown.is_some_and(|shown| shown.0);
    // Each source's item under the pointer: where it is, its key there, its tooltip's corner.
    let at_item = |origin: Vec2, placed: opensacred_rules::inventory::Placed| {
        origin
            + Vec2::new(
                f32::from(placed.at.0 + placed.size.0),
                f32::from(placed.at.1),
            ) * CELL
            + FROM_ITEM
    };
    let sold = || {
        let (placed, dropped) = crate::merchant::hovered(
            (merchant.as_deref(), &showing, &wares),
            (&pointer, &ui_scale, &windows),
        )?;
        Some((
            Source::Merchant(showing.0),
            placed.key,
            at_item(crate::merchant::GRID, placed),
            dropped,
        ))
    };
    let carried = || {
        let at = on_grid(&pointer, &ui_scale, &windows, &page)?;
        let placed = *pack.grid.at(cell_under(at)?)?;
        let dropped = pack
            .items
            .iter()
            .find(|(key, _)| *key == placed.key)?
            .1
            .clone();
        Some((Source::Backpack, placed.key, at_item(GRID, placed), dropped))
    };
    let held_by_set = || {
        let at = crate::inventory::screen_point(&pointer, &ui_scale, &windows)?;
        let count = slots.as_deref()?.0;
        let (set, half, corner, dropped) = crate::sets::hovered(sets.as_deref()?, count, at)?;
        let key = u32::try_from(set * 2 + half).ok()?;
        Some((Source::WeaponSet, key, corner + FROM_SLOT, dropped))
    };
    let under = shown
        .then(|| sold().or_else(carried))
        .flatten()
        .or_else(held_by_set);
    let current = panels.iter().next();
    if current.map(|(_, panel)| (panel.source, panel.key))
        == under.as_ref().map(|(source, key, ..)| (*source, *key))
    {
        return;
    }
    if let Some((entity, _)) = current {
        commands.entity(entity).despawn();
    }
    let Some((source, key, corner, dropped)) = under else {
        return;
    };
    let name = texts
        .as_deref()
        .and_then(|texts| item_name(texts, weighing.tables.as_deref(), &dropped, None))
        .unwrap_or_default();
    let buying = matches!(source, Source::Merchant(_));
    let lines = weighing.lines(&dropped, name, texts.as_deref(), buying);
    let root = roots
        .iter()
        .next()
        .unwrap_or_else(|| commands.spawn(root()).id());
    let panel = spawn(&mut commands, &images, &assets, corner, &lines);
    commands.entity(panel).insert(Panel {
        source,
        key,
        at: corner,
    });
    commands.entity(root).add_child(panel);
}

/// The tooltips' container.
fn root() -> impl Bundle {
    (
        Root,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Px(0.0),
            margin: UiRect::left(Val::Px(-bar::SCREEN.x / 2.0)),
            width: Val::Px(bar::SCREEN.x),
            height: Val::Px(bar::SCREEN.y),
            ..default()
        },
        GlobalZIndex(2),
        Pickable::IGNORE,
        DespawnOnExit(AppState::InGame),
    )
}

/// A tooltip's window of `lines` at `at` of the original's screen: its panel, its lines and its
/// border.
fn spawn(
    commands: &mut Commands,
    images: &UiImages,
    assets: &AssetServer,
    at: Vec2,
    lines: &[Line],
) -> Entity {
    let font = assets.load(FONT);
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(at.x),
                top: Val::Px(at.y),
                padding: UiRect::axes(Val::Px(INSET), Val::Px(INSET / 2.0)),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(PANEL),
            Pickable::IGNORE,
        ))
        .id();
    // Each line a row of its texts and its images, an image as wide as it is and as high as the
    // line at most.
    for line in lines {
        let row = commands
            .spawn((
                Node {
                    height: Val::Px(LINE),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .id();
        for piece in &line.0 {
            let child = match piece {
                Piece::Text(text, colour) => commands
                    .spawn((
                        Text::new(text.clone()),
                        TextFont {
                            font: font.clone().into(),
                            font_size: FontSize::Px(FONT_SIZE),
                            ..default()
                        },
                        TextLayout {
                            linebreak: LineBreak::NoWrap,
                            ..default()
                        },
                        TextColor(argb(*colour)),
                        Pickable::IGNORE,
                    ))
                    .id(),
                Piece::Image(index, colour) => {
                    let (image, mut node) =
                        bar::piece(images, assets, *index, Vec2::ZERO, Vec2::ZERO);
                    node.position_type = PositionType::Relative;
                    commands
                        .spawn((
                            ImageNode {
                                color: argb(*colour),
                                ..image
                            },
                            node,
                            Pickable::IGNORE,
                        ))
                        .id()
                }
            };
            commands.entity(row).add_child(child);
        }
        commands.entity(panel).add_child(row);
    }
    add_border(commands, (images, assets), panel);
    panel
}

/// Puts the tooltip's border about `panel` (`0x727B30`), as the purchase window's too.
pub(crate) fn add_border(
    commands: &mut Commands,
    (images, assets): (&UiImages, &AssetServer),
    panel: Entity,
) {
    for (index, place, turned) in border() {
        let Some((file, rect, _)) = images.turned_part(index) else {
            continue;
        };
        let piece = commands
            .spawn((
                ImageNode {
                    rect: Some(rect),
                    flip_x: turned[0],
                    flip_y: turned[1],
                    image_mode: NodeImageMode::Stretch,
                    ..ImageNode::new(crate::hud::image(assets, file.to_owned()))
                },
                place,
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(panel).add_child(piece);
    }
}

/// The border's pieces: each image, its node about the panel, and whether it is turned about
/// across and along (`0x727B30`).
fn border() -> [(u32, Node, [bool; 2]); 8] {
    let node = |left: Option<f32>, right: Option<f32>, top: Option<f32>, bottom: Option<f32>| {
        let px = |v: Option<f32>| v.map_or(Val::Auto, Val::Px);
        Node {
            position_type: PositionType::Absolute,
            left: px(left),
            right: px(right),
            top: px(top),
            bottom: px(bottom),
            ..default()
        }
    };
    let sized = |mut node: Node, width: Option<f32>, height: Option<f32>| {
        if let Some(width) = width {
            node.width = Val::Px(width);
        }
        if let Some(height) = height {
            node.height = Val::Px(height);
        }
        node
    };
    [
        // The top and the bottom from 4 to the right edge, 6 high.
        (
            EDGE_ALONG,
            sized(
                node(Some(4.0), Some(0.0), Some(-4.0), None),
                None,
                Some(6.0),
            ),
            [false, false],
        ),
        (
            EDGE_ALONG,
            sized(
                node(Some(4.0), Some(0.0), None, Some(-3.0)),
                None,
                Some(6.0),
            ),
            [false, false],
        ),
        // The left and the right down the whole height, 6 wide.
        (
            EDGE_DOWN,
            sized(
                node(Some(-4.0), None, Some(0.0), Some(0.0)),
                Some(6.0),
                None,
            ),
            [false, false],
        ),
        (
            EDGE_DOWN,
            sized(
                node(None, Some(-6.0), Some(0.0), Some(0.0)),
                Some(6.0),
                None,
            ),
            [false, false],
        ),
        // The corners, 38 × 18: the top right as stored, the others turned about.
        (
            CORNER,
            sized(
                node(None, Some(-14.0), Some(-11.0), None),
                Some(38.0),
                Some(18.0),
            ),
            [false, false],
        ),
        (
            CORNER,
            sized(
                node(Some(-13.0), None, Some(-11.0), None),
                Some(38.0),
                Some(18.0),
            ),
            [true, false],
        ),
        (
            CORNER,
            sized(
                node(None, Some(-14.0), None, Some(-10.0)),
                Some(38.0),
                Some(18.0),
            ),
            [false, true],
        ),
        (
            CORNER,
            sized(
                node(Some(-13.0), None, None, Some(-10.0)),
                Some(38.0),
                Some(18.0),
            ),
            [true, true],
        ),
    ]
}

/// Each tooltip kept 16 pixels within the original's screen by its size as laid out
/// (`0x6E7AF0`, the style's bit 8).
fn keep_within(mut panels: Query<(&Panel, &ComputedNode, &mut Node)>) {
    for (panel, computed, mut node) in &mut panels {
        let size = computed.size() * computed.inverse_scale_factor();
        let at = kept(panel.at, size);
        if node.left != Val::Px(at.x) {
            node.left = Val::Px(at.x);
        }
        if node.top != Val::Px(at.y) {
            node.top = Val::Px(at.y);
        }
    }
}

/// The place `at` of a window of `size` kept 16 pixels within the original's screen: no nearer
/// its left and top, and moved back from its right and bottom.
fn kept(at: Vec2, size: Vec2) -> Vec2 {
    let mut at = at.max(Vec2::splat(MARGIN));
    if at.x + size.x + MARGIN > bar::SCREEN.x {
        at.x = bar::SCREEN.x - MARGIN - size.x;
    }
    if at.y + size.y + MARGIN > bar::SCREEN.y {
        at.y = bar::SCREEN.y - MARGIN - size.y;
    }
    at
}

/// A control's tooltip of the style `0x209` (the combos page's symbols, the master's controls):
/// its window, and the centre of its top on the original's screen.
#[derive(Component)]
struct Tip(Vec2);

/// A control's tooltip's container.
#[derive(Component)]
pub(crate) struct TipRoot;

/// The rest before a control's tooltip shows (`0x7327B0`, `0x9E52AC`).
pub(crate) const TIP_AFTER: f32 = 1.0 / 3.0;

/// The control that the pointer rests on, the time it came onto it and whether its tooltip
/// shows.
#[derive(Default)]
pub(crate) struct Rest(Option<(u32, f32, bool)>);

impl Rest {
    /// The pointer over the control `over` at `now`: whether the tooltip that shows goes (the
    /// control left or another's), and the control whose tooltip is due, once, after
    /// [`TIP_AFTER`].
    pub(crate) fn step(&mut self, over: Option<u32>, now: f32) -> (bool, Option<u32>) {
        let gone = match (over, self.0) {
            (Some(control), Some((was, ..))) if control == was => false,
            (Some(control), _) => {
                self.0 = Some((control, now, false));
                true
            }
            (None, rest) => {
                self.0 = None;
                rest.is_some()
            }
        };
        let due = match self.0 {
            Some((control, since, false)) if now - since >= TIP_AFTER => {
                self.0 = Some((control, since, true));
                Some(control)
            }
            _ => None,
        };
        (gone, due)
    }
}

/// Where a control's tooltip stands: on the original's screen centred along the window, as the
/// bar and the windows lie, or as the portraits' column lies, at the window's right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Frame {
    Screen,
    Column,
}

/// A control's tooltip of `text` on the original's screen centred on `centre` (the style `0x209`,
/// whose point is its centre), a panel of the items' tooltip's look without its border, kept 16
/// pixels within the screen by [`centre_tips`].
pub(crate) fn spawn_tip(commands: &mut Commands, assets: &AssetServer, text: &str, centre: Vec2) {
    spawn_tip_in(commands, assets, (text, centre), Frame::Screen);
}

/// [`spawn_tip`] in the frame `frame`, the text's colours' tags ([`coloured`]) followed.
pub(crate) fn spawn_tip_in(
    commands: &mut Commands,
    assets: &AssetServer,
    (text, centre): (&str, Vec2),
    frame: Frame,
) {
    let font = TextFont {
        font: assets.load(FONT).into(),
        font_size: FontSize::Px(FONT_SIZE),
        ..default()
    };
    // A run of spaces draws as one, as the original's padded numbers show ("0/ 300" of
    // `%6d/%6d`), while the panel keeps the width of the text as written, each space left out
    // widening it by the original's space (**hypothesis** and **estimate** by the frames of the
    // health's and the experience's tooltips).
    let (drawn, left_out) = one_space(text);
    let widened = INSET + SPACE * f32::from(u16::try_from(left_out).unwrap_or(u16::MAX));
    let panel = commands
        .spawn((
            Tip(centre),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(centre.x),
                top: Val::Px(centre.y),
                padding: UiRect {
                    left: Val::Px(INSET),
                    right: Val::Px(widened),
                    top: Val::Px(INSET / 2.0),
                    bottom: Val::Px(INSET / 2.0),
                },
                ..default()
            },
            BackgroundColor(PANEL),
            Pickable::IGNORE,
        ))
        .with_children(|panel| {
            panel
                .spawn((
                    Text::default(),
                    font.clone(),
                    TextLayout {
                        linebreak: LineBreak::NoWrap,
                        ..default()
                    },
                    TextColor(argb(WHITE)),
                    Pickable::IGNORE,
                ))
                .with_children(|line| {
                    for (words, colour) in coloured(&drawn) {
                        line.spawn((TextSpan::new(words), font.clone(), TextColor(argb(colour))));
                    }
                });
        })
        .id();
    let root = match frame {
        Frame::Screen => commands
            .spawn((TipRoot, crate::backpack::screen(), GlobalZIndex(2)))
            .id(),
        Frame::Column => commands
            .spawn((
                TipRoot,
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Px(bar::SCREEN.x),
                    height: Val::Px(bar::SCREEN.y),
                    ..default()
                },
                Pickable::IGNORE,
                DespawnOnExit(AppState::InGame),
                GlobalZIndex(2),
            ))
            .id(),
    };
    commands.entity(root).add_child(panel);
}

/// The width that the original's tooltip measures a space at (**estimate**).
const SPACE: f32 = 6.5;

/// `text` with each run of spaces as one, and how many spaces it left out.
fn one_space(text: &str) -> (String, usize) {
    let mut drawn = String::with_capacity(text.len());
    let mut left_out = 0;
    for c in text.chars() {
        if c == ' ' && drawn.ends_with(' ') {
            left_out += 1;
        } else {
            drawn.push(c);
        }
    }
    (drawn, left_out)
}

/// A text of `global.res` as its colours' tags paint it: `<colorw=AARRGGBB>` the word after it,
/// `<color=…>` and `<colorl=…>` the rest of its line, white else; `<n>` a line's break, the other
/// tags left out.
pub(crate) fn coloured(text: &str) -> Vec<(String, u32)> {
    let mut runs: Vec<(String, u32)> = Vec::new();
    let mut push = |words: &str, colour: u32| {
        if words.is_empty() {
            return;
        }
        match runs.last_mut() {
            Some((last, was)) if *was == colour => last.push_str(words),
            _ => runs.push((words.to_owned(), colour)),
        }
    };
    let (mut word, mut line) = (None, None);
    let mut rest = text;
    while !rest.is_empty() {
        let open = rest.find('<').unwrap_or(rest.len());
        let mut words = &rest[..open];
        if let Some(colour) = word.take() {
            let end = words.find(char::is_whitespace).unwrap_or(words.len());
            push(&words[..end], colour);
            words = &words[end..];
        }
        push(words, line.unwrap_or(WHITE));
        let Some(close) = rest[open..].find('>') else {
            break;
        };
        let tag = &rest[open + 1..open + close];
        rest = &rest[open + close + 1..];
        let colour = |prefix: &str| {
            tag.strip_prefix(prefix)
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        };
        if tag == "n" {
            push("\n", WHITE);
            line = None;
        } else if let Some(colour) = colour("colorw=") {
            word = Some(colour);
        } else if let Some(colour) = colour("colorl=").or_else(|| colour("color=")) {
            line = Some(colour);
        }
    }
    runs
}

/// Each control's tooltip centred on its place by its size as laid out, kept 16 pixels within
/// the screen.
fn centre_tips(mut tips: Query<(&Tip, &ComputedNode, &mut Node)>) {
    for (tip, computed, mut node) in &mut tips {
        let size = computed.size() * computed.inverse_scale_factor();
        let corner = tip_corner(tip.0, size);
        if node.left != Val::Px(corner.x) {
            node.left = Val::Px(corner.x);
        }
        if node.top != Val::Px(corner.y) {
            node.top = Val::Px(corner.y);
        }
    }
}

/// The top left of a tooltip of `size` centred on `centre`, kept 16 pixels within the screen.
fn tip_corner(centre: Vec2, size: Vec2) -> Vec2 {
    (centre - size / 2.0)
        .min(bar::SCREEN - Vec2::splat(MARGIN) - size)
        .max(Vec2::splat(MARGIN))
}

/// The items' tooltip.
pub struct TooltipPlugin;

impl Plugin for TooltipPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (grid, keep_within, centre_tips)
                .chain()
                .run_if(in_state(AppState::InGame)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_controls_tooltip_is_centred_and_kept_on_the_screen() {
        let line = Vec2::new(200.0, 24.0);
        assert_eq!(
            tip_corner(Vec2::new(400.0, 300.0), line),
            Vec2::new(300.0, 288.0)
        );
        // The combos page's first symbol's, as wide as the original's line, moved to the margin.
        assert_eq!(
            tip_corner(Vec2::new(80.0, 300.0), Vec2::new(340.0, 24.0)),
            Vec2::new(16.0, 288.0)
        );
        // The portrait's health over the top's margin, the experience's off the right.
        assert_eq!(
            tip_corner(Vec2::new(868.0, 16.0), line),
            Vec2::new(768.0, 16.0)
        );
        assert_eq!(
            tip_corner(Vec2::new(916.0, 122.0), line).x,
            1024.0 - 16.0 - 200.0
        );
    }

    #[test]
    fn a_texts_tags_colour_a_word_or_the_rest_of_its_line() {
        const YELLOW: u32 = 0xFFFF_FF80;
        assert_eq!(
            coloured("<colorw=FFFFFF80>Inventory [I]"),
            [("Inventory".to_owned(), YELLOW), (" [I]".to_owned(), WHITE)]
        );
        assert_eq!(
            coloured("<colorw=FFFFFF80>Collect <colorw=FFFFFF80>all [A]"),
            [
                ("Collect".to_owned(), YELLOW),
                (" ".to_owned(), WHITE),
                ("all".to_owned(), YELLOW),
                (" [A]".to_owned(), WHITE),
            ]
        );
        assert_eq!(
            coloured("<color=FFFFFF80> Day<n>Night <unknown>x"),
            [(" Day".to_owned(), YELLOW), ("\nNight x".to_owned(), WHITE)]
        );
        assert_eq!(coloured("plain"), [("plain".to_owned(), WHITE)]);
        assert_eq!(
            one_space("Experience:      0/   300"),
            ("Experience: 0/ 300".to_owned(), 7)
        );
    }

    #[test]
    fn a_tooltip_shows_once_after_its_rest() {
        let mut rest = Rest::default();
        assert_eq!(rest.step(Some(1), 0.0), (true, None));
        assert_eq!(rest.step(Some(1), 0.2), (false, None));
        assert_eq!(rest.step(Some(1), 0.4), (false, Some(1)));
        assert_eq!(rest.step(Some(1), 0.9), (false, None));
        // Another control's starts its own rest, the one shown going.
        assert_eq!(rest.step(Some(2), 1.0), (true, None));
        assert_eq!(rest.step(None, 1.1), (true, None));
        assert_eq!(rest.step(None, 1.2), (false, None));
    }

    #[test]
    fn a_tooltip_keeps_16_pixels_within_the_screen() {
        let size = Vec2::new(280.0, 84.0);
        assert_eq!(kept(Vec2::new(48.0, 408.0), size), Vec2::new(48.0, 408.0));
        assert_eq!(kept(Vec2::new(4.0, 2.0), size), Vec2::new(16.0, 16.0));
        assert_eq!(
            kept(Vec2::new(900.0, 700.0), size),
            Vec2::new(1024.0 - 16.0 - 280.0, 768.0 - 16.0 - 84.0)
        );
    }

    #[test]
    fn a_colour_is_its_word_as_the_frame_holds_it() {
        assert_eq!(
            argb(0xFFAF_AFAF),
            Color::linear_rgba(175.0 / 255.0, 175.0 / 255.0, 175.0 / 255.0, 1.0)
        );
    }
}
