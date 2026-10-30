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
const PANEL: Color = Color::linear_rgba(0.0, 0.0, 0.0, 0.5);
const INSET: f32 = 4.0;
const LINE: f32 = 20.0;
const FONT_SIZE: f32 = 20.0;
/// The tooltip's place from the item's top right, and how far it keeps within the screen.
const FROM_ITEM: Vec2 = Vec2::new(16.0, 16.0);
const MARGIN: f32 = 16.0;
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
    /// The tooltip's lines of `dropped`, named `name`, of the texts `texts`.
    fn lines(&self, dropped: &Dropped, name: &str, texts: Option<&Texts>) -> Vec<Line> {
        let Some(tables) = self.tables.as_deref() else {
            return vec![Line::of(name, WHITE)];
        };
        let record = tables.record(dropped.item);
        let values = tables.values(dropped);
        let made = dropped.values.is_some();
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
            quality,
            values,
            // A made item's levels by its values (`0x5D3AB0` at `0x5D56FA`), another's as its
            // record holds them: a potion's none, a scroll's level none (seen, #1273).
            level: if made {
                opensacred_rules::fight::item_level(&values, &self.rules)
            } else {
                u32::from(record.level)
            },
            asked: if made {
                tables.asks(dropped, &self.rules).level
            } else {
                u32::from(record.asked)
            },
            made,
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

/// The tooltip shown: what it shows (the grid's item by its key), its place on the original's
/// screen before it is kept within it.
#[derive(Component)]
struct Panel {
    key: u32,
    at: Vec2,
}

/// The tooltip of the grid's item under the pointer while the items' page shows, made as the
/// pointer comes to an item and taken away as it leaves it (`0x6C1480`).
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn grid(
    mut commands: Commands,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    (shown, page): (Option<Res<Shown>>, Res<crate::inventory::Page>),
    pack: Res<Pack>,
    texts: Option<Res<Texts>>,
    weighing: Weighing,
    (images, assets): (Res<UiImages>, Res<AssetServer>),
    panels: Query<(Entity, &Panel)>,
    roots: Query<Entity, With<Root>>,
) {
    let under = shown
        .is_some_and(|shown| shown.0)
        .then(|| on_grid(&pointer, &ui_scale, &windows, &page))
        .flatten()
        .and_then(cell_under)
        .and_then(|cell| pack.grid.at(cell).copied());
    let current = panels.iter().next();
    if current.map(|(_, panel)| panel.key) == under.map(|placed| placed.key) {
        return;
    }
    if let Some((entity, _)) = current {
        commands.entity(entity).despawn();
    }
    let Some(placed) = under else {
        return;
    };
    let Some(dropped) = pack
        .items
        .iter()
        .find(|(key, _)| *key == placed.key)
        .map(|(_, dropped)| *dropped)
    else {
        return;
    };
    let name = texts
        .as_deref()
        .and_then(|texts| item_name(texts, weighing.tables.as_deref(), &dropped, None))
        .unwrap_or_default();
    let lines = weighing.lines(&dropped, name, texts.as_deref());
    let corner = GRID
        + Vec2::new(
            f32::from(placed.at.0 + placed.size.0),
            f32::from(placed.at.1),
        ) * CELL
        + FROM_ITEM;
    let root = roots
        .iter()
        .next()
        .unwrap_or_else(|| commands.spawn(root()).id());
    let panel = spawn(&mut commands, &images, &assets, corner, &lines);
    commands.entity(panel).insert(Panel {
        key: placed.key,
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
    panel
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

/// The items' tooltip.
pub struct TooltipPlugin;

impl Plugin for TooltipPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (grid, keep_within)
                .chain()
                .run_if(in_state(AppState::InGame)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
