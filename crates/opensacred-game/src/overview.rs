//! The overview map (#1562; docs/re/maps.md, "The overview map"): while Tab or the middle button
//! is held (`cUI_OverviewMap`, `UI_WND_OVERVIEWMAP`, `0x6C6C50`), unless the world map shows or
//! the game is paused, the lens over the world in a view of 700 × 525 at (162, 120) about the
//! middle of the hero's cell: the minimap's ground at the alpha 128 (255 less `MINIMAP_ALPHA`'s
//! 127), its fog of war over the land explored, opaque (`EXPLOREMAP` 1), the quests' icons and
//! titles, and the compass's markers' arrows on the view's edges; over it the hero's cross, the
//! difficulty's name, and the dialogue's frame about the view. The game goes on, and the clicks
//! reach the world.
//!
//! The region's line under the difficulty's name: the region of the hero's area, the kills that
//! remain in it and its total (#1574, `regions`).
//!
//! The creatures' marks as the lens draws them (#1571, `lens_creatures`).
//!
//! Not followed yet: the
//! settings' `MINIMAP_ALPHA` and `EXPLOREMAP`, and the scripts' boxes' tooltip, which no box of
//! the release has.

use bevy::prelude::*;
use bevy::text::FontSize;
use opensacred_rules::quests::Marker;

use crate::bar::{self, UiImages};
use crate::game::{AppState, FONT};
use crate::lines::Texts;
use crate::pause::Play;
use crate::tooltip::argb;

/// The view (`0x759AF0`), its middle, the lens's point's place, and the alpha of the ground.
const VIEW: Rect = Rect {
    min: Vec2::new(162.0, 120.0),
    max: Vec2::new(862.0, 645.0),
};
const ALPHA: f32 = 128.0 / 255.0;
/// The hero's cross, two white lines (**measured**).
const CROSS: [(Vec2, Vec2); 2] = [
    (Vec2::new(501.0, 379.0), Vec2::new(522.0, 389.0)),
    (Vec2::new(522.0, 379.0), Vec2::new(501.0, 389.0)),
];
/// The texts: the font 3, AntiquaSSK 16, in `0xFFFFFF80` from (170, 124), 25 apart
/// (**measured**), and the difficulties' names (`0x182EE44`).
const TEXT_AT: Vec2 = Vec2::new(170.0, 124.0);
const TEXT_SIZE: f32 = 24.0;
const TEXT_DROP: f32 = 2.0;
const TEXT_COLOUR: u32 = 0xFFFF_FF80;
const TEXT_STEP: f32 = 25.0;
const DIFFICULTIES: [&str; 5] = [
    "UI_DIFFICULTY_BRONZE",
    "UI_DIFFICULTY_SILVER",
    "UI_DIFFICULTY_GOLD",
    "UI_DIFFICULTY_PLATINUM",
    "UI_DIFFICULTY_NIOB",
];
/// The markers' arrows (`GUI_main_07`): the top's, the bottom's, the left's and the right's
/// images, the markers' colours (`0x9E124C`: the main quest's, the side quest's and the world
/// map's pin's), and their place within the view's edge.
const ARROWS: [u32; 4] = [234, 235, 236, 237];
const ARROW_COLOURS: [u32; 3] = [0xFFFF_FF00, 0xFF00_00FF, 0xFFFF_0000];
const ARROW_INSET: f32 = 3.0;

/// Whether the overview shows, and the iso point and sector its lens was laid out for.
#[derive(Resource, Debug, Default)]
pub struct Overview {
    pub shown: bool,
    laid: Option<(Vec2, (i32, i32))>,
}

/// The overview's container, its view's plane and the markers' arrows.
#[derive(Component)]
struct OverviewRoot;
#[derive(Component)]
struct Plane;
#[derive(Component)]
struct Arrow(usize);
/// The region's line, under the difficulty's name.
#[derive(Component)]
struct RegionLine;

/// The edge that a ray from the view's middle along `direction` leaves it by, 0 the top, 1 the
/// bottom, 2 the left and 3 the right, and where on the view's edge.
fn edge_of(direction: Vec2) -> Option<(usize, Vec2)> {
    if direction == Vec2::ZERO {
        return None;
    }
    let middle = VIEW.center();
    let half = VIEW.half_size();
    let across = if direction.x == 0.0 {
        f32::INFINITY
    } else {
        half.x / direction.x.abs()
    };
    let down = if direction.y == 0.0 {
        f32::INFINITY
    } else {
        half.y / direction.y.abs()
    };
    let at = middle + direction * across.min(down);
    let edge = if down <= across {
        usize::from(direction.y > 0.0)
    } else if direction.x < 0.0 {
        2
    } else {
        3
    };
    Some((edge, at))
}

/// The corner of an arrow of `size` on the edge `edge` at `at`, within the view's edge.
fn arrow_corner(edge: usize, at: Vec2, size: Vec2) -> Vec2 {
    let corner = match edge {
        0 => Vec2::new(at.x - size.x / 2.0, VIEW.min.y + ARROW_INSET),
        1 => Vec2::new(at.x - size.x / 2.0, VIEW.max.y - ARROW_INSET - size.y),
        2 => Vec2::new(VIEW.min.x + ARROW_INSET, at.y - size.y / 2.0),
        _ => Vec2::new(VIEW.max.x - ARROW_INSET - size.x, at.y - size.y / 2.0),
    };
    corner.clamp(
        VIEW.min + Vec2::splat(ARROW_INSET),
        VIEW.max - Vec2::splat(ARROW_INSET) - size,
    )
}

/// Tab or the middle button held shows the overview, unless the world map shows or the game is
/// paused (`cEngine_receiveEvent`).
fn hold(
    keys: crate::controls::Keys,
    buttons: Res<ButtonInput<MouseButton>>,
    (map, play): (Res<crate::world_map::Map>, Res<State<Play>>),
    console: Option<Res<crate::console::Console>>,
    mut overview: ResMut<Overview>,
) {
    let held =
        keys.pressed(crate::controls::Action::Overview) || buttons.pressed(MouseButton::Middle);
    let free = !map.shown
        && *play.get() == Play::Running
        && !console.is_some_and(|console| console.is_open());
    let shown = held && free;
    if overview.shown != shown {
        overview.shown = shown;
    }
}

/// The hero's cell's middle on the iso plane, the camera's point that the lens shows.
fn hero_point(heroes: &Query<&GlobalTransform, With<crate::hero::Body>>) -> Option<Vec2> {
    let at = heroes.iter().next()?.translation();
    let middle = crate::paths::centre(crate::paths::cell_of(at.x, at.z));
    Some(crate::minimap::iso(middle))
}

/// Makes the overview as it shows, lays its lens out again as the hero leaves the sector it was
/// laid out for and moves it as the hero walks within it, and takes it away as it hides.
#[expect(
    clippy::too_many_arguments,
    reason = "the overview, its pieces, the land explored, the quests and the hero"
)]
fn show(
    mut commands: Commands,
    mut overview: ResMut<Overview>,
    (images, assets, mut pictures): (Res<UiImages>, Res<AssetServer>, ResMut<Assets<Image>>),
    (minimaps, land, texts): (
        Res<crate::minimap::Minimaps>,
        Res<crate::explored::Land>,
        Option<Res<Texts>>,
    ),
    difficulty: Option<Res<crate::game::Difficulty>>,
    roots: Query<Entity, With<OverviewRoot>>,
    mut planes: Query<&mut Node, With<Plane>>,
    heroes: Query<&GlobalTransform, With<crate::hero::Body>>,
    (live, named): (
        Option<NonSend<crate::scripts::Live>>,
        Query<(&crate::npcs::ScriptName, &GlobalTransform)>,
    ),
) {
    let point = hero_point(&heroes).filter(|_| overview.shown);
    let Some(point) = point else {
        for root in &roots {
            commands.entity(root).despawn();
        }
        overview.laid = None;
        return;
    };
    let world = crate::minimap::world(point);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a sector of the world, within its 128"
    )]
    let sector = (
        (world.x / (64.0 * crate::iso::CELL)).floor() as i32,
        (world.y / (64.0 * crate::iso::CELL)).floor() as i32,
    );
    if let (Some((laid, at)), Ok(mut plane)) = (overview.laid, planes.single_mut())
        && at == sector
        && !roots.is_empty()
    {
        let offset = (laid - point) / 12.0 - VIEW.min;
        plane.left = Val::Px(offset.x);
        plane.top = Val::Px(offset.y);
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    let root = commands
        .spawn((OverviewRoot, crate::backpack::screen(), GlobalZIndex(1)))
        .id();
    let mut view = bar::placed(VIEW.min, VIEW.size());
    view.overflow = Overflow::clip();
    let view = commands.spawn((view, Pickable::IGNORE)).id();
    commands.entity(root).add_child(view);
    let plane = commands
        .spawn((Plane, bar::placed(-VIEW.min, bar::SCREEN), Pickable::IGNORE))
        .id();
    commands.entity(view).add_child(plane);
    crate::minimap::spawn(
        &mut commands,
        plane,
        (&minimaps, &assets),
        (point, Vec2::ZERO),
        ALPHA,
    );
    crate::minimap::spawn_fog(
        &mut commands,
        plane,
        (&land, &mut pictures),
        point,
        Vec2::ZERO,
    );
    if let Some(live) = live.as_deref() {
        crate::world_map::quests_in_lens(
            &mut commands,
            plane,
            (&images, &assets, texts.as_deref()),
            (live, &named),
            point,
        );
    }
    overview.laid = Some((point, sector));
    // The hero's cross over the lens.
    for (from, to) in CROSS {
        let length = from.distance(to);
        let middle = (from + to) / 2.0;
        let turn = (to - from).to_angle();
        let line = commands
            .spawn((
                bar::placed(
                    middle - Vec2::new(length / 2.0, 0.5),
                    Vec2::new(length, 1.0),
                ),
                UiTransform::from_rotation(Rot2::radians(turn)),
                BackgroundColor(Color::WHITE),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(root).add_child(line);
    }
    // The difficulty's name.
    let index = difficulty.map_or(0, |difficulty| difficulty.0.index());
    let key = DIFFICULTIES[index.min(DIFFICULTIES.len() - 1)];
    let name = texts
        .as_deref()
        .and_then(|texts| texts.named(key))
        .unwrap_or(key);
    let lettering = || {
        (
            TextFont {
                font: assets.load(FONT).into(),
                font_size: FontSize::Px(TEXT_SIZE),
                ..default()
            },
            TextLayout {
                linebreak: bevy::text::LineBreak::NoWrap,
                ..default()
            },
            TextColor(argb(TEXT_COLOUR)),
            Pickable::IGNORE,
        )
    };
    let line = |n: f32| {
        bar::placed(
            TEXT_AT + Vec2::new(0.0, TEXT_DROP + TEXT_STEP * n),
            Vec2::new(VIEW.width(), 32.0),
        )
    };
    let text = commands
        .spawn((line(0.0), Text::new(name), lettering()))
        .id();
    commands.entity(root).add_child(text);
    // The region's line under it, written as the counts change (#1574).
    let region = commands
        .spawn((RegionLine, line(1.0), Text::default(), lettering()))
        .id();
    commands.entity(root).add_child(region);
    // The markers' arrows, placed as they show.
    for marker in 0..ARROW_COLOURS.len() {
        let arrow = commands
            .spawn((
                Arrow(marker),
                ImageNode::default(),
                bar::placed(Vec2::ZERO, Vec2::ZERO),
                Visibility::Hidden,
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(root).add_child(arrow);
    }
    // The frame about the view, over the lens.
    let frame = commands
        .spawn((bar::placed(VIEW.min, VIEW.size()), Pickable::IGNORE))
        .id();
    commands.entity(root).add_child(frame);
    for (image, node) in crate::talk::frame(&images, &assets, VIEW.width()) {
        let piece = commands.spawn((image, node, Pickable::IGNORE)).id();
        commands.entity(frame).add_child(piece);
    }
}

/// Puts each marker's arrow on the view's edge where the ray from its middle toward the marker
/// leaves it (`0x6CCBD0`'s end): the main quest's, the side quest's and the world map's pin's.
fn arrows(
    overview: Res<Overview>,
    (images, assets): (Res<UiImages>, Res<AssetServer>),
    heroes: Query<&GlobalTransform, With<crate::hero::Body>>,
    map: Res<crate::world_map::Map>,
    (live, named): (
        Option<NonSend<crate::scripts::Live>>,
        Query<(&crate::npcs::ScriptName, &GlobalTransform)>,
    ),
    mut arrows: Query<(&Arrow, &mut ImageNode, &mut Node, &mut Visibility)>,
) {
    let Some(hero) = hero_point(&heroes).filter(|_| overview.shown) else {
        return;
    };
    for (arrow, mut picture, mut node, mut visibility) in &mut arrows {
        let target = match arrow.0 {
            0 | 1 => live.as_deref().and_then(|live| {
                let marker = if arrow.0 == 0 {
                    Marker::Main
                } else {
                    Marker::Side
                };
                crate::compass::target_of(live.marker(marker)?, &named)
            }),
            _ => map.pin_target(),
        };
        let place = target.and_then(|target| edge_of(crate::minimap::iso(target) - hero));
        let Some((edge, at)) = place else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        let index = ARROWS[edge];
        let (image, piece) = bar::piece(&images, &assets, index, Vec2::ZERO, Vec2::ZERO);
        let size = images.size(index);
        let corner = arrow_corner(edge, at, size);
        *picture = image.with_color(argb(ARROW_COLOURS[arrow.0]));
        *node = piece;
        node.left = Val::Px(corner.x);
        node.top = Val::Px(corner.y);
        visibility.set_if_neq(Visibility::Inherited);
    }
}

/// Writes the region's line of the hero's area (#1574): none where the area is of no region.
fn region_line(
    overview: Res<Overview>,
    (region_kills, streamer, texts): (
        Res<crate::regions::RegionKills>,
        Option<Res<crate::sectors::Streamer>>,
        Option<Res<Texts>>,
    ),
    heroes: Query<&GlobalTransform, With<crate::hero::Body>>,
    mut lines: Query<&mut Text, With<RegionLine>>,
) {
    if !overview.shown {
        return;
    }
    let line = heroes
        .iter()
        .next()
        .zip(streamer.as_deref())
        .and_then(|(hero, streamer)| {
            let at = hero.translation();
            streamer.area(crate::music::sector_of(at.x, at.z))
        })
        .and_then(|area| region_kills.line(area, texts.as_deref()))
        .unwrap_or_default();
    for mut text in &mut lines {
        if text.0 != line {
            text.0.clone_from(&line);
        }
    }
}

/// The creatures' marks of the overview's lens, laid again each frame.
#[derive(Component)]
struct CreatureLayer;

/// Lays the creatures' marks in the overview's plane each frame (#1571, `lens_creatures`).
fn marks(
    mut commands: Commands,
    overview: Res<Overview>,
    (images, assets): (Res<UiImages>, Res<AssetServer>),
    creatures: crate::lens_creatures::Creatures,
    planes: Query<Entity, With<Plane>>,
    layers: Query<Entity, With<CreatureLayer>>,
    mut classes: Local<std::collections::HashMap<u32, u8>>,
) {
    for layer in &layers {
        commands.entity(layer).despawn();
    }
    let (Some((laid, _)), Ok(plane)) = (overview.laid.filter(|_| overview.shown), planes.single())
    else {
        return;
    };
    let reach = 4.0 * 64.0 * crate::iso::CELL;
    let marks = creatures.marks(crate::minimap::world(laid), reach, &mut classes);
    let layer = commands
        .spawn((
            CreatureLayer,
            bar::placed(Vec2::ZERO, bar::SCREEN),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(plane).add_child(layer);
    crate::lens_creatures::spawn(&mut commands, layer, (&images, &assets), &marks, laid);
}

/// A new game's overview, hidden.
fn reset(mut overview: ResMut<Overview>) {
    *overview = Overview::default();
}

/// The overview map.
pub struct OverviewPlugin;

impl Plugin for OverviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Overview>()
            .add_systems(OnEnter(AppState::InGame), reset)
            .add_systems(
                Update,
                (hold, show, marks, arrows, region_line)
                    .chain()
                    .after(crate::frame::Step::Follow)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_markers_arrow_stands_where_its_ray_leaves_the_view() {
        // Straight left: the left edge, at the view's middle's height.
        let (edge, at) = edge_of(Vec2::new(-10.0, 0.0)).unwrap_or_default();
        assert_eq!(edge, 2);
        assert_eq!(at, Vec2::new(162.0, VIEW.center().y));
        // Down and a little right: the bottom.
        let (edge, at) = edge_of(Vec2::new(1.0, 5.0)).unwrap_or_default();
        assert_eq!(edge, 1);
        assert_eq!(at.y, VIEW.max.y);
        assert!(edge_of(Vec2::ZERO).is_none());
        // Its picture within the edge, inset.
        let corner = arrow_corner(2, Vec2::new(162.0, 518.0), Vec2::new(11.0, 11.0));
        assert_eq!(corner, Vec2::new(165.0, 512.5));
    }
}
