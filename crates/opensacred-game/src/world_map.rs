//! The world map (#1561; docs/re/maps.md): `M` and the bar's map button show and hide it
//! (`cUI_Megamap`, `UI_WND_MEGAMAP`), its close button and Esc hide it; it pauses the game while it shows (a
//! single player's, `0x6C7BB0`), and its show hides the inventory and the quest book. It is laid
//! out on the original's 1024 × 768 screen centred along the window's width, as the bar is
//! (`bar`).
//!
//! The map of 2,048 × 2,816 pixels, the Underworld's three rows of 256 × 256 pictures above
//! Ancaria's eight (`ui_worldMapUW<row><column>`, `ui_worldMapX<row><column>`), shown from its
//! scroll, which its show puts so that the hero stands at the screen's middle; the pointer at the
//! screen's edge, off the buttons' bar, scrolls it 4 pixels a frame, and the right button held
//! drags it, within the map. A world point's pixel is its iso point's, scaled and moved
//! (`0x6CB3B0`). Over the map: each quest log entry's icon at its compass's cell (`0x6C9A60`), a
//! main quest's and a side quest's, coloured by how it stands, the titles of those under the
//! pointer in a tooltip; the hero's white cross (`0x6C9520`); the black edge 70 pixels deep; the
//! window's frame (`0x725870`, the style 1); the buttons' ornament and the buttons.
//!
//! The left button held off the bar shows the lens (#1568, `0x6CCBD0`): over 0.2 s a view of
//! 700 × 525 at the screen's middle of the minimap's ground about the point under the pointer
//! (`minimap`), its fog of war over the land explored (`explored`), and the quests' icons with
//! their titles beside them; the map's tooltip hides under it.
//!
//! The pin's button starts the pin's mode, its cursor the pin (#1569, `0x6C8140`): a left
//! release off the bar places the pin there, its mark the pin's picture 64 × 64, its world
//! point the compass's outer pin's target (`compass`); a right press on the mark takes it
//! away, elsewhere it ends the mode; the edges do not scroll and the lens does not show in it.
//!
//! Not followed yet: the places' names, the people's icons, the portals, the frontier of the land
//! walked and the pacified areas (#1567); the lens's creatures and party (#1571), its places and
//! pin, and its titles' layout apart; the buttons' pressed looks and tooltips, and the edges'
//! cursors.

use bevy::input::InputSystems;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::text::FontSize;
use opensacred_rules::quests::{Entry, Standing};

use crate::bar::{self, UiImages};
use crate::frame::Control;
use crate::game::AppState;
use crate::hud::Pointer;
use crate::inventory::screen_point;
use crate::lines::Texts;
use crate::menu::MENU_FONT;
use crate::pause::{Play, Still};
use crate::tooltip::argb;

/// The map's pictures, eight across and eleven down, the first three rows the Underworld's, and
/// its size (`0x6C7540`, `0x6CB530`).
const COLUMNS: u8 = 8;
const ROWS: u8 = 11;
const UNDERWORLD_ROWS: u8 = 3;
const TILE: f32 = 256.0;
const SIZE: Vec2 = Vec2::new(2048.0, 2816.0);
/// A world point's pixel of the map from its iso point (`0x6CB3B0`).
const SCALE: Vec2 = Vec2::new(0.007_446, 0.014_914);
const OFFSET: Vec2 = Vec2::new(1_018.749_94, -186.947_39);
/// The scroll at the screen's edges, 4 pixels a frame, within 1 pixel of them (`0x6C9F60`).
const EDGE_STEP: f32 = 4.0;
const EDGE: f32 = 1.0;
/// The buttons' bar, where the edges do not scroll: the ornament's two halves 80 wide, 32 more on
/// either side, from y 685 (`0x6CB530`).
const BAR: Rect = Rect {
    min: Vec2::new(400.0, 685.0),
    max: Vec2::new(624.0, 750.0),
};
/// The ornament's halves (`GUI_main_07`, images 228 and 229) under the buttons: the pin's (230)
/// and the close button (232), 34 × 34.
const ORNAMENT: [(u32, Vec2); 2] = [
    (228, Vec2::new(432.0, 685.0)),
    (229, Vec2::new(512.0, 685.0)),
];
const PIN_BUTTON: (u32, Vec2) = (230, Vec2::new(432.0, 685.0));
const CLOSE_BUTTON: (u32, Vec2) = (232, Vec2::new(558.0, 685.0));
const BUTTON: f32 = 34.0;
/// The window's frame (`0x725870`, the style 1) about (48, 48) 928 × 672: its corners, its edges
/// across and down (`GUI_main_12`, images 310 to 321), the bottom edge 12 below the rectangle and
/// the right one 15 right of it.
const FRAMED: Rect = Rect {
    min: Vec2::new(48.0, 48.0),
    max: Vec2::new(976.0, 720.0),
};
const LEFT_EDGE: u32 = 310;
const TOP_EDGE: u32 = 311;
const RIGHT_EDGE: u32 = 312;
const BOTTOM_EDGE: u32 = 313;
const CORNERS: [u32; 4] = [318, 319, 320, 321];
const BOTTOM_BELOW: f32 = 12.0;
const RIGHT_BESIDE: f32 = 15.0;
/// The black edge, from black at the screen's edge to clear 70 pixels in.
const FADE: f32 = 70.0;
/// The hero's cross: the diagonals of a 22-pixel square, white.
const CROSS: f32 = 22.0;
/// The quests' icons (`0x4974B0`, `0x4972E0`): a main quest's (138) and another's (139), each a
/// square of a third of its width twice over, coloured by how the quest stands; the box about an
/// icon whose title the tooltip writes.
const MAIN_ICON: u32 = 138;
const SIDE_ICON: u32 = 139;
const MAIN_RUNS: u32 = 0xFFFF_FF80;
const SIDE_RUNS: u32 = 0xFF00_6CFF;
const WON: u32 = 0xFF00_FF00;
const LOST: u32 = 0xFFFF_0000;
const TIP_BOX: f32 = 16.0;
/// The tooltip: the titles in the font 2, Carolingia 12, at the pointer's right, over grey at half
/// its alpha (**hypothesis** for its offsets).
const TIP_RIGHT: f32 = 34.0;
const TIP_SIZE: f32 = 18.0;
const TIP_GROUND: u32 = 0x7F7F_7F7F;
const TIP_INSET: f32 = 3.0;

/// The world map: whether it shows, and its scroll, the map's pixel at the screen's corner.
#[derive(Resource, Debug, Default)]
pub struct Map {
    pub shown: bool,
    scroll: Vec2,
    /// Whether the pin's mode holds (`+0xC0154` bit `0x100`), and the pin: its place on the map
    /// (`+0xC06C0`) and its world point, the compass's outer pin's (#1569).
    pin_mode: bool,
    pin: Option<Vec2>,
    pin_world: Option<Vec2>,
}

impl Map {
    /// The scroll in whole pixels, as the original keeps it (**measured**: the map's pictures 1
    /// pixel left of the original's at the scroll's fraction).
    fn shown_scroll(&self) -> Vec2 {
        self.scroll.trunc()
    }

    /// Whether the pin's mode holds, which the cursor's pin shows.
    pub fn pin_mode(&self) -> bool {
        self.shown && self.pin_mode
    }

    /// The world point of the pin, the compass's outer pin's target.
    pub fn pin_target(&self) -> Option<Vec2> {
        self.pin_world
    }
}

/// The pin's picture, which its mode's cursor and its mark show (`MOUSE_PUSHPIN`), the mark's
/// side, the hot spot's offset of the cursor's picture, and the box about the mark that a right
/// press takes it away within (`0x6C8140`).
pub(crate) const PUSHPIN: &str = "mouse_pushpin";
const PIN_MARK: f32 = 64.0;
const PIN_SPOT: f32 = 16.0;

/// The frontier of the land walked (`0x6C8800`): a red square of 2 × 2 for each entered neighbour
/// of a sector not entered, about the sector's corner's pixel 20 lower, toward the neighbour.
const FRONTIER_DOWN: f32 = 20.0;
const FRONTIER_TOWARD: [Vec2; 8] = [
    Vec2::new(0.0, -20.0),
    Vec2::new(10.0, -10.0),
    Vec2::new(20.0, 0.0),
    Vec2::new(-10.0, -10.0),
    Vec2::new(10.0, 10.0),
    Vec2::new(-20.0, 0.0),
    Vec2::new(-10.0, 10.0),
    Vec2::new(0.0, 20.0),
];
const FRONTIER_DOT: f32 = 2.0;
const FRONTIER_RED: u32 = 0xFFFF_0000;

/// A dot of the frontier.
#[derive(Component)]
struct FrontierDot;

/// The frontier's dots' places on the map (`0x6C8800`), of the sectors and their bits.
fn frontier_dots(frontier: &[((u16, u16), u8)]) -> Vec<Vec2> {
    frontier
        .iter()
        .flat_map(|&((x, y), bits)| {
            let corner = crate::paths::centre((i32::from(x) * 64, i32::from(y) * 64));
            let base = pixel(corner) + Vec2::new(0.0, FRONTIER_DOWN);
            (0..8)
                .filter(move |k| bits & (1 << k) != 0)
                .map(move |k| base + FRONTIER_TOWARD[k])
        })
        .collect()
}

/// Lays the frontier's dots on the map as it shows.
fn show_frontier(
    mut commands: Commands,
    sectors: Res<crate::explored::MapSectors>,
    scrolled: Query<Entity, Added<Scrolled>>,
) {
    let Ok(container) = scrolled.single() else {
        return;
    };
    for at in frontier_dots(&sectors.frontier()) {
        let dot = commands
            .spawn((
                FrontierDot,
                bar::placed(at, Vec2::splat(FRONTIER_DOT)),
                BackgroundColor(argb(FRONTIER_RED)),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(container).add_child(dot);
    }
}

/// The pin's mark on the map.
#[derive(Component)]
struct PinMark;

/// The pin's button starts the pin's mode; in it a left release off the bar places the pin, its
/// world point kept as the compass's outer pin's (`0x4A63B0`, the marker 2); a right press within
/// the pin's mark takes it away, elsewhere in the mode it ends the mode (`0x6C8140`).
fn place_pin(
    mut map: ResMut<Map>,
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
) {
    if !map.shown {
        return;
    }
    let Some(at) = screen_point(&pointer, &ui_scale, &windows) else {
        return;
    };
    let on_button =
        Rect::from_corners(PIN_BUTTON.1, PIN_BUTTON.1 + Vec2::splat(BUTTON)).contains(at);
    if buttons.just_pressed(MouseButton::Left) && on_button && !map.pin_mode {
        map.pin_mode = true;
        return;
    }
    let scroll = map.shown_scroll();
    if map.pin_mode && buttons.just_released(MouseButton::Left) && !BAR.contains(at) {
        // The mark at the cursor's picture's middle, the world point at the pointer.
        map.pin = Some(scroll + at + Vec2::splat(PIN_SPOT));
        map.pin_world = Some(crate::minimap::world(iso_of(scroll + at)));
        map.pin_mode = false;
        info!("world map: the pin placed at the map's {}", scroll + at);
    }
    if buttons.just_pressed(MouseButton::Right) {
        let on_pin = map
            .pin
            .is_some_and(|pin| (pin - scroll - at).abs().max_element() <= PIN_MARK / 2.0);
        if on_pin {
            map.pin = None;
            map.pin_world = None;
            info!("world map: the pin taken away");
        } else if map.pin_mode {
            map.pin_mode = false;
        }
    }
}

/// The pin's mark, its picture 64 × 64 about its place on the map.
fn show_pin(
    mut commands: Commands,
    map: Res<Map>,
    (images, assets): (Res<UiImages>, Res<AssetServer>),
    scrolled: Query<Entity, With<Scrolled>>,
    mut marks: Query<(Entity, &mut Node), With<PinMark>>,
) {
    let place = map.pin.filter(|_| map.shown);
    match (place, marks.single_mut()) {
        (Some(pin), Ok((_, mut node))) => {
            node.left = Val::Px(pin.x - PIN_MARK / 2.0);
            node.top = Val::Px(pin.y - PIN_MARK / 2.0);
        }
        (Some(pin), Err(_)) => {
            let (Ok(container), Some(file)) = (scrolled.single(), images.file(PUSHPIN)) else {
                return;
            };
            let mark = commands
                .spawn((
                    PinMark,
                    ImageNode::new(crate::hud::image(&assets, file.to_owned())),
                    bar::placed(pin - Vec2::splat(PIN_MARK / 2.0), Vec2::splat(PIN_MARK)),
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(container).add_child(mark);
        }
        (None, _) => {
            for (mark, _) in &marks {
                commands.entity(mark).despawn();
            }
        }
    }
}

/// The lens (`0x6CCBD0`): its view of 700 × 525 about the screen's middle, grown and shrunk at 5
/// a second, and the title's colours by how a quest stands (`0x4972E0(i, 1)`).
const LENS: Vec2 = Vec2::new(700.0, 525.0);
const LENS_RATE: f32 = 5.0;
const TITLE_RUNS: u32 = 0xFFFF_FFFF;
const TITLE_ENDED: u32 = 0xFFAF_AFAF;
/// The lens's titles beside their icons (**hypothesis**: the label layout `0x6620F0`, which
/// keeps them apart, is not followed), 2 pixels off.
const TITLE_BESIDE: f32 = 2.0;

/// The lens: whether the left button holds it, its alpha, the screen's point it shows, and the
/// iso plane's point and sector its content was laid out for.
#[derive(Resource, Debug, Default)]
struct Lens {
    on: bool,
    alpha: f32,
    at: Vec2,
    laid: Option<(Vec2, (i32, i32))>,
}

/// The lens's view, and its plane, which holds its content at the screen's places of the point it
/// was laid out for.
#[derive(Component)]
struct LensView;
#[derive(Component)]
struct LensPlane;

/// The nodes of a kind of the lens's, apart from another's.
type Placed<'w, 's, T, U> = Query<'w, 's, (Entity, &'static mut Node), (With<T>, Without<U>)>;

/// The iso plane's point of the map's pixel `pixel` (`0x6CCBD0`: `0x6CB3B0` undone).
fn iso_of(pixel: Vec2) -> Vec2 {
    (pixel - OFFSET) / SCALE
}

/// The lens's view at its alpha `alpha`: (512 − 350a, 384 − 262.5a) 700a × 525a.
fn lens_view(alpha: f32) -> Rect {
    Rect::from_center_size(bar::SCREEN / 2.0, LENS * alpha)
}

/// The left button held on the map, off the bar, holds the lens on, the pointer kept; its alpha
/// follows at 5 a second, of the real time.
fn hold_lens(
    map: Res<Map>,
    mut lens: ResMut<Lens>,
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    time: Res<Time<Real>>,
) {
    if !map.shown {
        *lens = Lens::default();
        return;
    }
    let at = screen_point(&pointer, &ui_scale, &windows);
    let off_bar = at.is_some_and(|at| !BAR.contains(at));
    if buttons.just_pressed(MouseButton::Left) && off_bar && !map.pin_mode {
        lens.on = true;
    }
    if buttons.just_released(MouseButton::Left) {
        lens.on = false;
    }
    if lens.on
        && let Some(at) = at.filter(|_| off_bar)
    {
        lens.at = at;
    }
    let step = LENS_RATE * time.delta_secs();
    lens.alpha = if lens.on {
        (lens.alpha + step).min(1.0)
    } else {
        (lens.alpha - step).max(0.0)
    };
}

/// Lays the lens's content out about its point: the ground, the fog and the quests' icons with
/// their titles; moves it as the point moves within its sector, and lays it out again as it
/// leaves it.
#[expect(
    clippy::too_many_arguments,
    reason = "the lens, its pieces, the land explored and the quests"
)]
fn show_lens(
    mut commands: Commands,
    (map, mut lens): (Res<Map>, ResMut<Lens>),
    (images, assets, mut pictures): (Res<UiImages>, Res<AssetServer>, ResMut<Assets<Image>>),
    (minimaps, land, texts): (
        Res<crate::minimap::Minimaps>,
        Res<crate::explored::Land>,
        Option<Res<Texts>>,
    ),
    roots: Query<Entity, With<MapRoot>>,
    mut views: Placed<LensView, LensPlane>,
    mut planes: Placed<LensPlane, LensView>,
    (live, named): (
        Option<NonSend<crate::scripts::Live>>,
        Query<(&crate::npcs::ScriptName, &GlobalTransform)>,
    ),
) {
    if lens.alpha <= 0.0 {
        for (view, _) in &views {
            commands.entity(view).despawn();
        }
        lens.laid = None;
        return;
    }
    let Some(root) = roots.iter().next() else {
        return;
    };
    let point = iso_of(lens.at + map.shown_scroll());
    let world = crate::minimap::world(point);
    let sector = (
        (world.x / (64.0 * crate::iso::CELL)).floor(),
        (world.y / (64.0 * crate::iso::CELL)).floor(),
    );
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a sector of the world, within its 128"
    )]
    let sector = (sector.0 as i32, sector.1 as i32);
    let rect = lens_view(lens.alpha);
    let (view, plane) = match (views.single_mut(), lens.laid) {
        (Ok((view, mut node)), Some((_, laid))) if laid == sector => {
            node.left = Val::Px(rect.min.x);
            node.top = Val::Px(rect.min.y);
            node.width = Val::Px(rect.width());
            node.height = Val::Px(rect.height());
            (view, None)
        }
        _ => {
            for (view, _) in &views {
                commands.entity(view).despawn();
            }
            let mut node = bar::placed(rect.min, rect.size());
            node.overflow = Overflow::clip();
            let view = commands
                .spawn((LensView, node, ZIndex(1), Pickable::IGNORE))
                .id();
            commands.entity(root).add_child(view);
            let plane = commands
                .spawn((
                    LensPlane,
                    bar::placed(Vec2::ZERO, bar::SCREEN),
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(view).add_child(plane);
            crate::minimap::spawn(
                &mut commands,
                plane,
                (&minimaps, &assets),
                (point, Vec2::ZERO),
                1.0,
            );
            crate::minimap::spawn_fog(
                &mut commands,
                plane,
                (&land, &mut pictures),
                point,
                Vec2::ZERO,
            );
            if let Some(live) = live.as_deref() {
                quests_in_lens(
                    &mut commands,
                    plane,
                    (&images, &assets, texts.as_deref()),
                    (live, &named),
                    point,
                );
            }
            lens.laid = Some((point, sector));
            (view, Some(plane))
        }
    };
    let _ = view;
    // The plane at the point it was laid out for, less the view's corner.
    let laid = lens.laid.map_or(point, |(laid, _)| laid);
    let offset = (laid - point) / 12.0 - rect.min;
    match plane {
        Some(plane) => {
            commands
                .entity(plane)
                .insert(bar::placed(offset, bar::SCREEN));
        }
        None => {
            if let Ok((_, mut node)) = planes.single_mut() {
                node.left = Val::Px(offset.x);
                node.top = Val::Px(offset.y);
            }
        }
    }
}

/// The quests' icons and titles in the lens about the iso plane's point `point`, at their
/// compasses' cells (`0x497620`): each icon at its own size in its colour, its title beside it in
/// the font 2 over grey, white for a quest that runs and grey for one ended.
pub(crate) fn quests_in_lens(
    commands: &mut Commands,
    plane: Entity,
    (images, assets, texts): (&UiImages, &AssetServer, Option<&Texts>),
    (live, named): (
        &crate::scripts::Live,
        &Query<(&crate::npcs::ScriptName, &GlobalTransform)>,
    ),
    point: Vec2,
) {
    let quests = live.quests();
    for entry in quests.book() {
        let Some(at) = quests
            .compass(entry.quest)
            .and_then(|compass| crate::compass::target_of(compass, named))
        else {
            continue;
        };
        let middle = crate::paths::centre(crate::paths::cell_of(at.x, at.y));
        let screen = (crate::minimap::iso(middle) - point) / 12.0 + bar::SCREEN / 2.0;
        let (image, colour) = icon(entry);
        let size = images.size(image);
        let (mut picture, node) =
            bar::piece(images, assets, image, screen - size / 2.0, Vec2::ZERO);
        picture.color = argb(colour);
        let child = commands.spawn((picture, node, Pickable::IGNORE)).id();
        commands.entity(plane).add_child(child);
        let Some(title) = entry.title else {
            continue;
        };
        let title = texts
            .and_then(|texts| texts.id(title))
            .map_or_else(|| title.to_string(), crate::talk::shown_text);
        let colour = if entry.standing == Standing::Entered {
            TITLE_RUNS
        } else {
            TITLE_ENDED
        };
        let label = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(screen.x + size.x / 2.0 + TITLE_BESIDE),
                    top: Val::Px(screen.y - TIP_SIZE / 2.0 - TIP_INSET),
                    padding: UiRect::all(Val::Px(TIP_INSET)),
                    ..default()
                },
                BackgroundColor(argb(TIP_GROUND)),
                Pickable::IGNORE,
            ))
            .with_child((
                Text::new(title),
                TextFont {
                    font: assets.load(MENU_FONT).into(),
                    font_size: FontSize::Px(TIP_SIZE),
                    ..default()
                },
                TextColor(argb(colour)),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(plane).add_child(label);
    }
}

/// The map's container, its pictures' and its markers' that its scroll moves, the hero's cross,
/// and the tooltip.
#[derive(Component)]
struct MapRoot;
#[derive(Component)]
struct Scrolled;
#[derive(Component)]
struct Cross;
#[derive(Component)]
struct MapTip;
/// A quest's icon, by its quest.
#[derive(Component)]
struct QuestIcon(u32);

/// The picture of the map's row `row` and column `column`.
fn tile(row: u8, column: u8) -> String {
    if row < UNDERWORLD_ROWS {
        format!("ui_worldmapuw{row}{column}")
    } else {
        format!("ui_worldmapx{}{column}", row - UNDERWORLD_ROWS)
    }
}

/// The pictures of the map, for `UiImages` to find.
pub(crate) fn tiles() -> Vec<String> {
    (0..ROWS)
        .flat_map(|row| (0..COLUMNS).map(move |column| tile(row, column)))
        .collect()
}

/// The map's pixel of the world point `at` (`0x6CB3B0`).
pub fn pixel(at: Vec2) -> Vec2 {
    crate::minimap::iso(at) * SCALE + OFFSET
}

/// The scroll held within the map.
fn clamped(scroll: Vec2) -> Vec2 {
    scroll.clamp(Vec2::ZERO, SIZE - bar::SCREEN)
}

/// The scroll of the pointer at `at`, off the bar, at the screen's edges: 4 pixels a frame toward
/// each edge it is at.
fn edge_scroll(at: Vec2) -> Vec2 {
    if BAR.contains(at) {
        return Vec2::ZERO;
    }
    let along = |value: f32, size: f32| {
        if value < EDGE {
            -EDGE_STEP
        } else if value > size - 2.0 * EDGE {
            EDGE_STEP
        } else {
            0.0
        }
    };
    Vec2::new(along(at.x, bar::SCREEN.x), along(at.y, bar::SCREEN.y))
}

/// A quest's icon and colour by its entry (`0x4974B0`, `0x4972E0`).
fn icon(entry: &Entry) -> (u32, u32) {
    let image = if entry.main() { MAIN_ICON } else { SIDE_ICON };
    let colour = match (entry.standing, entry.main()) {
        (Standing::Entered, true) => MAIN_RUNS,
        (Standing::Entered, false) => SIDE_RUNS,
        (Standing::Won, _) => WON,
        (Standing::Lost, _) => LOST,
    };
    (image, colour)
}

/// The side of an icon's square, twice a third of its image's width (`0x6C9A60`).
fn icon_side(width: f32) -> f32 {
    2.0 * (width / 3.0).floor()
}

/// The pieces of the window's frame about [`FRAMED`] (`0x725870`), by `size` of an image.
fn frame(size: impl Fn(u32) -> Vec2) -> Vec<(u32, Vec2)> {
    let rect = FRAMED;
    let corner = size(CORNERS[0]);
    let mut pieces = vec![
        (CORNERS[0], rect.min - corner),
        (CORNERS[1], Vec2::new(rect.max.x, rect.min.y - corner.y)),
        (
            CORNERS[2],
            Vec2::new(rect.min.x - size(CORNERS[2]).x, rect.max.y),
        ),
        (CORNERS[3], rect.max),
    ];
    let (top, bottom) = (size(TOP_EDGE), size(BOTTOM_EDGE));
    for i in 0..times(rect.width(), top.x) {
        let i = f32::from(i);
        pieces.push((
            TOP_EDGE,
            Vec2::new(rect.min.x + top.x * i, rect.min.y - corner.y),
        ));
        pieces.push((
            BOTTOM_EDGE,
            Vec2::new(rect.min.x + bottom.x * i, rect.max.y + BOTTOM_BELOW),
        ));
    }
    let (left, right) = (size(LEFT_EDGE), size(RIGHT_EDGE));
    for i in 0..times(rect.height() + corner.y, left.y).saturating_sub(1) {
        let i = f32::from(i);
        pieces.push((
            LEFT_EDGE,
            Vec2::new(rect.min.x - corner.x, rect.min.y + left.y * i),
        ));
        pieces.push((
            RIGHT_EDGE,
            Vec2::new(rect.max.x + RIGHT_BESIDE, rect.min.y + right.y * i),
        ));
    }
    pieces
}

/// The whole times `step` goes into `length`.
fn times(length: f32, step: f32) -> u16 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a frame's pieces along a side, a few dozen"
    )]
    let times = (length / step).floor().max(0.0) as u16;
    times
}

/// `M` and the bar's map button (`UI_TB_MAP`), the close button and Esc show and hide the map; its show pauses the game and hides the
/// inventory and the book, its hide goes on.
fn toggle(
    mut keys: crate::controls::KeysMut,
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    mut map: ResMut<Map>,
    (mut inventory, mut book): (
        ResMut<crate::inventory::Shown>,
        ResMut<crate::quest_book::Book>,
    ),
    heroes: Query<&GlobalTransform, With<crate::hero::Body>>,
    (mut still, mut next): (ResMut<Still>, ResMut<NextState<Play>>),
) {
    let pressed = keys.take(crate::controls::Action::WorldMap)
        || (!map.shown
            && buttons.just_pressed(MouseButton::Left)
            && screen_point(&pointer, &ui_scale, &windows).is_some_and(|at| {
                let (_, corner) = bar::MAP_BUTTON;
                Rect::from_corners(corner, corner + Vec2::splat(BUTTON)).contains(at)
            }));
    let escaped = map.shown && keys.take(crate::controls::Action::Back);
    let closed = map.shown
        && buttons.just_pressed(MouseButton::Left)
        && screen_point(&pointer, &ui_scale, &windows).is_some_and(|at| {
            let corner = CLOSE_BUTTON.1;
            Rect::from_corners(corner, corner + Vec2::splat(BUTTON)).contains(at)
        });
    if !(pressed || escaped || closed) {
        return;
    }
    map.shown = !map.shown;
    map.pin_mode = false;
    if map.shown {
        // The hero at the screen's middle (**hypothesis**: the show centres it).
        if let Some(hero) = heroes.iter().next() {
            let at = pixel(hero.translation().xz());
            map.scroll = clamped(at - bar::SCREEN / 2.0);
        }
        inventory.0 = false;
        book.shown = false;
        still.0 = true;
        next.set(Play::Paused);
    } else {
        next.set(Play::Running);
    }
    info!("world map: {}", if map.shown { "shown" } else { "hidden" });
}

/// The edges' scroll and the right button's drag.
fn scroll(
    mut map: ResMut<Map>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
) {
    if !map.shown {
        return;
    }
    let Some(at) = screen_point(&pointer, &ui_scale, &windows) else {
        return;
    };
    // The edges scroll while no button is held, neither in the pin's mode (`0x6C9F60`).
    let step = if map.pin_mode {
        Vec2::ZERO
    } else if buttons.pressed(MouseButton::Right) {
        -motion.delta / ui_scale.0.max(0.01)
    } else if buttons.pressed(MouseButton::Left) {
        Vec2::ZERO
    } else {
        edge_scroll(at)
    };
    if step != Vec2::ZERO {
        map.scroll = clamped(map.scroll + step);
    }
}

/// Makes the map as it shows and takes it away as it hides.
fn show(
    mut commands: Commands,
    map: Res<Map>,
    (images, assets): (Res<UiImages>, Res<AssetServer>),
    roots: Query<Entity, With<MapRoot>>,
) {
    if !map.is_changed() || map.shown == !roots.is_empty() {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    if !map.shown {
        return;
    }
    let root = commands
        .spawn((
            MapRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Px(0.0),
                margin: UiRect::left(Val::Px(-bar::SCREEN.x / 2.0)),
                width: Val::Px(bar::SCREEN.x),
                height: Val::Px(bar::SCREEN.y),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::BLACK),
            GlobalZIndex(3),
            Pickable::IGNORE,
            DespawnOnExit(AppState::InGame),
        ))
        .id();
    let scrolled = commands
        .spawn((
            Scrolled,
            bar::placed(-map.shown_scroll(), SIZE),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(root).add_child(scrolled);
    for row in 0..ROWS {
        for column in 0..COLUMNS {
            let Some(file) = images.file(&tile(row, column)) else {
                continue;
            };
            let at = TILE * Vec2::new(f32::from(column), f32::from(row));
            let child = commands
                .spawn((
                    ImageNode::new(crate::hud::image(&assets, file.to_owned())),
                    bar::placed(at, Vec2::splat(TILE)),
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(scrolled).add_child(child);
        }
    }
    // The hero's cross, its two diagonals turned about its middle.
    let cross = commands
        .spawn((
            Cross,
            bar::placed(Vec2::ZERO, Vec2::splat(CROSS)),
            // The hero and the quests over the frontier's dots (`0x6C9520` after `0x6C8800`).
            ZIndex(1),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(scrolled).add_child(cross);
    let diagonal = CROSS * std::f32::consts::SQRT_2;
    for turn in [std::f32::consts::FRAC_PI_4, -std::f32::consts::FRAC_PI_4] {
        let line = commands
            .spawn((
                bar::placed(
                    Vec2::new((CROSS - diagonal) / 2.0, CROSS / 2.0 - 0.5),
                    Vec2::new(diagonal, 1.0),
                ),
                UiTransform::from_rotation(Rot2::radians(turn)),
                BackgroundColor(Color::WHITE),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(cross).add_child(line);
    }
    // The black edge, the frame, the ornament and the buttons, over the map.
    let fade = |to: fn(Vec<ColorStop>) -> LinearGradient| {
        BackgroundGradient::from(to(vec![
            ColorStop::new(Color::BLACK, Val::Px(0.0)),
            ColorStop::new(Color::NONE, Val::Px(FADE)),
        ]))
    };
    for (rect, to) in [
        (
            Rect::new(0.0, 0.0, bar::SCREEN.x, FADE),
            LinearGradient::to_bottom as fn(_) -> _,
        ),
        (
            Rect::new(0.0, bar::SCREEN.y - FADE, bar::SCREEN.x, bar::SCREEN.y),
            LinearGradient::to_top,
        ),
        (
            Rect::new(0.0, 0.0, FADE, bar::SCREEN.y),
            LinearGradient::to_right,
        ),
        (
            Rect::new(bar::SCREEN.x - FADE, 0.0, bar::SCREEN.x, bar::SCREEN.y),
            LinearGradient::to_left,
        ),
    ] {
        let band = commands
            .spawn((
                bar::placed(rect.min, rect.size()),
                fade(to),
                ZIndex(2),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(root).add_child(band);
    }
    let pieces = frame(|index| images.size(index))
        .into_iter()
        .chain(ORNAMENT)
        .chain([PIN_BUTTON, CLOSE_BUTTON]);
    for (index, at) in pieces {
        let child = commands
            .spawn((
                bar::piece(&images, &assets, index, at, Vec2::ZERO),
                ZIndex(2),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(root).add_child(child);
    }
}

/// The map's pictures' container, the hero's cross and the quests' icons, apart.
type ScrolledNodes<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static mut Node),
    (With<Scrolled>, Without<Cross>, Without<QuestIcon>),
>;
type CrossNodes<'w, 's> =
    Query<'w, 's, &'static mut Node, (With<Cross>, Without<Scrolled>, Without<QuestIcon>)>;
type IconNodes<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static QuestIcon,
        &'static mut Node,
        &'static mut ImageNode,
    ),
    (Without<Scrolled>, Without<Cross>),
>;

/// Moves the map by its scroll, the hero's cross to the hero, and the quests' icons to their
/// compasses' cells.
#[expect(
    clippy::too_many_arguments,
    reason = "the map, its pieces, the hero, the quests and their places"
)]
fn follow(
    mut commands: Commands,
    map: Res<Map>,
    (images, assets): (Res<UiImages>, Res<AssetServer>),
    mut scrolled: ScrolledNodes,
    mut crosses: CrossNodes,
    mut icons: IconNodes,
    heroes: Query<&GlobalTransform, With<crate::hero::Body>>,
    (live, named): (
        Option<NonSend<crate::scripts::Live>>,
        Query<(&crate::npcs::ScriptName, &GlobalTransform)>,
    ),
) {
    if !map.shown {
        return;
    }
    let Ok((container, mut node)) = scrolled.single_mut() else {
        return;
    };
    node.left = Val::Px(-map.shown_scroll().x);
    node.top = Val::Px(-map.shown_scroll().y);
    if let (Some(hero), Ok(mut cross)) = (heroes.iter().next(), crosses.single_mut()) {
        let at = pixel(hero.translation().xz()) - Vec2::splat(CROSS / 2.0);
        cross.left = Val::Px(at.x);
        cross.top = Val::Px(at.y);
    }
    let Some(live) = live else {
        return;
    };
    let quests = live.quests();
    let mut shown = Vec::new();
    for entry in quests.book() {
        let Some(at) = quests
            .compass(entry.quest)
            .and_then(|compass| crate::compass::target_of(compass, &named))
        else {
            continue;
        };
        // The compass's cell, its middle (`0x497540`).
        let middle = crate::paths::centre(crate::paths::cell_of(at.x, at.y));
        let (image, colour) = icon(entry);
        let side = icon_side(images.size(image).x);
        let corner = pixel(middle) - Vec2::splat(side / 2.0);
        shown.push(entry.quest);
        match icons.iter_mut().find(|(_, icon, ..)| icon.0 == entry.quest) {
            Some((_, _, mut node, mut picture)) => {
                node.left = Val::Px(corner.x);
                node.top = Val::Px(corner.y);
                picture.color = argb(colour);
            }
            None => {
                let (mut picture, mut node) =
                    bar::piece(&images, &assets, image, corner, Vec2::ZERO);
                node.width = Val::Px(side);
                node.height = Val::Px(side);
                picture.color = argb(colour);
                picture.image_mode = NodeImageMode::Stretch;
                let child = commands
                    .spawn((
                        QuestIcon(entry.quest),
                        picture,
                        node,
                        ZIndex(1),
                        Pickable::IGNORE,
                    ))
                    .id();
                commands.entity(container).add_child(child);
            }
        }
    }
    for (entity, icon, ..) in &icons {
        if !shown.contains(&icon.0) {
            commands.entity(entity).despawn();
        }
    }
}

/// The titles of the quests whose icons lie under the pointer, at its right (`0x6C9F60`); none
/// while the lens, drawn after them, shows.
fn tooltip(
    mut commands: Commands,
    (map, lens): (Res<Map>, Res<Lens>),
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    (assets, texts): (Res<AssetServer>, Option<Res<Texts>>),
    (icons, live): (
        Query<(&QuestIcon, &Node)>,
        Option<NonSend<crate::scripts::Live>>,
    ),
    (roots, tips): (Query<Entity, With<MapRoot>>, Query<Entity, With<MapTip>>),
    mut was: Local<Vec<u32>>,
) {
    let at = (map.shown && lens.alpha <= 0.0)
        .then(|| screen_point(&pointer, &ui_scale, &windows))
        .flatten();
    let under: Vec<u32> = at
        .map(|at| {
            icons
                .iter()
                .filter(|(_, node)| {
                    let (Val::Px(left), Val::Px(top), Val::Px(side)) =
                        (node.left, node.top, node.width)
                    else {
                        return false;
                    };
                    let middle =
                        Vec2::new(left, top) + Vec2::splat(side / 2.0) - map.shown_scroll();
                    (middle - at).abs().max_element() <= TIP_BOX / 2.0
                })
                .map(|(icon, _)| icon.0)
                .collect()
        })
        .unwrap_or_default();
    if *was == under && (under.is_empty() || !tips.is_empty()) {
        if let (Some(at), Some(tip)) = (at, tips.iter().next()) {
            commands.entity(tip).insert(tip_node(at));
        }
        return;
    }
    *was = under.clone();
    for tip in &tips {
        commands.entity(tip).despawn();
    }
    let (Some(at), Some(root), Some(live)) = (at, roots.iter().next(), live) else {
        return;
    };
    let titles: Vec<String> = live
        .quests()
        .book()
        .iter()
        .filter(|entry| under.contains(&entry.quest))
        .filter_map(|entry| entry.title)
        .map(|id| {
            texts
                .as_deref()
                .and_then(|texts| texts.id(id))
                .map_or_else(|| id.to_string(), crate::talk::shown_text)
        })
        .collect();
    if titles.is_empty() {
        return;
    }
    let tip = commands
        .spawn((
            MapTip,
            tip_node(at),
            ZIndex(3),
            BackgroundColor(argb(TIP_GROUND)),
            Pickable::IGNORE,
        ))
        .with_child((
            Text::new(titles.join("\n")),
            TextFont {
                font: assets.load(MENU_FONT).into(),
                font_size: FontSize::Px(TIP_SIZE),
                ..default()
            },
            TextColor(Color::WHITE),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(root).add_child(tip);
}

/// The tooltip's node at the pointer `at`.
fn tip_node(at: Vec2) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(at.x + TIP_RIGHT),
        top: Val::Px(at.y),
        padding: UiRect::all(Val::Px(TIP_INSET)),
        ..default()
    }
}

/// Whether the map hides: the world takes the pointer while it does not show.
fn hidden(map: Res<Map>) -> bool {
    !map.shown
}

/// A new game's map, hidden.
fn reset(mut map: ResMut<Map>) {
    *map = Map::default();
}

/// The world map.
pub struct WorldMapPlugin;

impl Plugin for WorldMapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Map>()
            .init_resource::<Lens>()
            .configure_sets(Update, Control.run_if(hidden))
            .add_systems(OnEnter(AppState::InGame), reset)
            .add_systems(
                PreUpdate,
                toggle
                    .after(InputSystems)
                    .after(crate::console::keys)
                    .before(crate::quest_book::toggle)
                    .before(crate::inventory::toggle)
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                Update,
                (
                    place_pin,
                    scroll,
                    show,
                    show_frontier,
                    follow,
                    show_pin,
                    tooltip,
                    hold_lens,
                    show_lens,
                )
                    .chain()
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_maps_pictures_put_the_underworld_above_ancaria() {
        let tiles = tiles();
        assert_eq!(tiles.len(), 88);
        assert_eq!(tiles[0], "ui_worldmapuw00");
        assert_eq!(tiles[23], "ui_worldmapuw27");
        assert_eq!(tiles[24], "ui_worldmapx00");
        assert_eq!(tiles[87], "ui_worldmapx77");
    }

    /// The Seraphim's start, the cell (3241, 2511), at the map's pixel that the original's frame
    /// shows at the screen's middle with the map scrolled to it.
    #[test]
    fn a_world_point_falls_on_its_maps_pixel() {
        let middle = crate::paths::centre((3241, 2511));
        let at = pixel(middle);
        assert!((at - Vec2::new(1_279.7, 1_872.2)).length() < 0.5, "{at}");
        assert_eq!(clamped(Vec2::new(-5.0, 3_000.0)), Vec2::new(0.0, 2_048.0));
    }

    #[test]
    fn the_edges_scroll_off_the_bar() {
        assert_eq!(edge_scroll(Vec2::new(0.0, 300.0)), Vec2::new(-4.0, 0.0));
        assert_eq!(edge_scroll(Vec2::new(1_023.0, 767.0)), Vec2::new(4.0, 4.0));
        assert_eq!(edge_scroll(Vec2::new(500.0, 300.0)), Vec2::ZERO);
        assert_eq!(edge_scroll(Vec2::new(500.0, 700.0)), Vec2::ZERO);
    }

    #[test]
    fn a_quests_icon_is_a_third_of_its_width_twice_over() {
        assert_eq!((icon_side(22.0), icon_side(14.0)), (14.0, 8.0));
        let mut side = Entry {
            quest: 150,
            ..Entry::default()
        };
        assert_eq!(icon(&side), (SIDE_ICON, SIDE_RUNS));
        side.standing = Standing::Won;
        assert_eq!(icon(&side).1, WON);
        let main = Entry {
            quest: 1,
            standing: Standing::Lost,
            ..Entry::default()
        };
        assert_eq!(icon(&main), (MAIN_ICON, LOST));
    }

    #[test]
    fn the_frame_stands_about_its_rectangle() {
        let size = |index: u32| match index {
            310 => Vec2::new(41.0, 32.0),
            311 => Vec2::new(32.0, 44.0),
            312 => Vec2::new(24.0, 32.0),
            313 => Vec2::new(32.0, 30.0),
            318 => Vec2::new(41.0, 44.0),
            319 => Vec2::new(39.0, 44.0),
            _ => Vec2::new(41.0, 42.0),
        };
        let pieces = frame(size);
        assert_eq!(pieces[0], (318, Vec2::new(7.0, 4.0)));
        assert_eq!(pieces[3], (321, Vec2::new(976.0, 720.0)));
        // 29 across, top and bottom, and (672 + 44) / 32 − 1 = 21 down each side.
        assert_eq!(pieces.len(), 4 + 2 * 29 + 2 * 21);
        assert!(pieces.contains(&(313, Vec2::new(48.0, 732.0))));
        assert!(pieces.contains(&(312, Vec2::new(991.0, 48.0))));
    }
}
