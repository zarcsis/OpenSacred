//! The interface over the world (#1087; docs/re/interface.md): the original's bar along the bottom
//! (`cUI_Taskbar2`, `UI_WND_TASKBAR` at (0, 676, 1024, 92)) and the hero's portrait in the column
//! at the right (`cUI_Mercenary`, `UI_WND_MERC` at (932, 0, 92, 676)), each piece an image of the
//! interface's table (`tables/ui_images.ron`, `0x9ECEB0`) at its place on the original's
//! 1024 × 768 screen, the bar centred along the bottom of a wider window and the column at its
//! right edge.
//!
//! The bar: the disc of day and night turned by the clock (`0x6E3E00`) under its housing, the
//! rail's ends and middle pieces, the weapon sets' slots at the left and the combat arts' at the
//! right, 1 to 5 a side by the hero's level, the selected art's icon split by its regeneration,
//! the inventory's, the options', the map's, the book's and the collecting's buttons, and the
//! potions' belt. The column: the health's ring, red below and grey above as the health drains,
//! the portrait by the hero's type, the bonuses' "+" and the experience's bar, and below them up
//! to four followers' entries, each its ring and its portrait. Clicks over the bar's area and the
//! portraits do not reach the world (`0x6E1BC0`). The low health's glow and alarm are
//! `low_health`'s.
//!
//! Not followed yet: the buttons' windows and their pressed images, the slots'
//! selection and their edit mode, the weapons' models and damage icons in the left slots, the
//! glows, the sparkle of the flag `0x20000000` (the potions' are `sparkles`'), the tooltips, the
//! points' "+", the followers' rings' glow and the mount's bar.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Result, anyhow};
use bevy::image::{ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use opensacred_data::tables::{UI_IMAGES, UiImage};

use crate::ai::Mind;
use crate::arts::HeroArts;
use crate::daynight::Clock;
use crate::fight::{Experience, Health};
use crate::files;
use crate::followers::Follower;
use crate::frame::HoldsControl;
use crate::game::AppState;
use crate::hero::Body;
use crate::hud::{self, Pointer};
use crate::talk::Portraits;

/// The original's screen, the bar's and the column's places on it.
pub(crate) const SCREEN: Vec2 = Vec2::new(1024.0, 768.0);
pub(crate) const BAR: Rect = Rect {
    min: Vec2::new(0.0, 676.0),
    max: Vec2::new(1024.0, 768.0),
};
pub(crate) const COLUMN: Rect = Rect {
    min: Vec2::new(932.0, 0.0),
    max: Vec2::new(1024.0, 676.0),
};
/// The disc's texture and its ring of runes, on an ellipse of 110 × 55 about (513, 710.5),
/// turned 0.25° a minute (`0x6E3E00`).
const DISC: &str = "gui_daynightdisc";
const DISC_RING: &str = "gui_daynightalpha";
const DISC_CENTRE: Vec2 = Vec2::new(513.0, 710.5);
const DISC_SIDE: f32 = 55.0;
const DISC_TURN: f32 = 0.004_363_323;
/// The images of the bar's pieces (docs/re/interface.md, "The bar").
const HOUSING: (u32, Vec2) = (11, Vec2::new(397.0, 680.0));
/// The inventory's button (`UI_TB_INVENTORY`, `inventory`).
pub(crate) const INVENTORY_BUTTON: (u32, Vec2) = (87, Vec2::new(399.0, 705.0));
/// The collecting's button (`UI_TB_COLLECT`, `pickup`).
pub(crate) const COLLECT_BUTTON: (u32, Vec2) = (134, Vec2::new(496.0, 743.0));
/// The options' button (`UI_TB_OPTIONS`), which opens the menu over the game (`esc_menu`).
pub(crate) const OPTIONS_BUTTON: (u32, Vec2) = (89, Vec2::new(434.0, 729.0));
/// The map's button (`UI_TB_MAP`, `world_map`).
pub(crate) const MAP_BUTTON: (u32, Vec2) = (91, Vec2::new(558.0, 729.0));
/// The book's button (`UI_TB_BOOK`, `quest_book`).
pub(crate) const BOOK_BUTTON: (u32, Vec2) = (93, Vec2::new(594.0, 705.0));
const BUTTONS: [(u32, Vec2); 5] = [
    INVENTORY_BUTTON,
    OPTIONS_BUTTON,
    MAP_BUTTON,
    BOOK_BUTTON,
    COLLECT_BUTTON,
];
/// The potions' belt's buttons, 32 × 32 each, the slots 1 to 5 (`belt`).
pub(crate) const BELT: [Vec2; 5] = [
    Vec2::new(435.0, 667.0),
    Vec2::new(465.0, 653.0),
    Vec2::new(497.0, 649.0),
    Vec2::new(530.0, 653.0),
    Vec2::new(560.0, 667.0),
];
pub(crate) const BELT_SIDE: f32 = 32.0;
pub(crate) const EMPTY_FLASK: u32 = 175;
/// The rail's left end and its two middle pieces, its right end and its two.
const RAIL_LEFT: (u32, f32) = (5, 747.0);
const RAIL_LEFT_MIDDLE: [(u32, f32); 2] = [(6, 749.0), (7, 750.0)];
const RAIL_RIGHT: (u32, f32) = (8, 747.0);
const RAIL_RIGHT_MIDDLE: [(u32, f32); 2] = [(9, 748.0), (10, 748.0)];
/// The slots: a side's count by the hero's level (`0x95703C`), their row, the orbs' backgrounds
/// selected and not (a weapon set's, an empty art slot's) and their rings.
const SLOTS_FROM: [u32; 5] = [1, 2, 8, 16, 30];
const SLOT_Y: f32 = 691.0;
const SLOT_SIDE: f32 = 64.0;
const WEAPON_SELECTED: u32 = 515;
const ORB: u32 = 635;
const ART_SELECTED: u32 = 479;
/// An empty art slot while the cursor holds a spell, or a weapon art in the edit mode
/// (`0x6E3430`: the image 533, `GUI_spell04`, green).
const ART_TAKES: u32 = 533;
const RING_SELECTED: u32 = 103;
const RING: u32 = 102;
/// The rings' layer among the bar's pieces: over a weapon set's halves' glow (`sets`, at 1) and
/// under its items' models (at 3), as `0x6E29E0` draws them in turn.
pub(crate) const RING_Z: i32 = 2;
/// The column's pieces: the health's ring, red and grey, the clasp's rows always grey, the
/// portrait, the bonuses' "+", and the experience's frame and fill.
const HEALTH_RED: u32 = 41;
const HEALTH_GREY: u32 = 42;
const RING_AT: Vec2 = Vec2::new(932.0, 16.0);
const RING_SIZE: Vec2 = Vec2::new(96.0, 108.0);
const CLASP: f32 = 11.0;
const PORTRAIT_AT: Vec2 = Vec2::new(941.0, 34.0);
const PORTRAIT_SIZE: Vec2 = Vec2::new(64.0, 74.0);
const BONUSES: (u32, Vec2) = (110, Vec2::new(1000.0, 34.0));
const EXPERIENCE_FRAME: (u32, Vec2) = (66, Vec2::new(940.0, 122.0));
const EXPERIENCE_FILL: (u32, Vec2) = (67, Vec2::new(940.0, 124.0));
const EXPERIENCE_WIDTH: f32 = 73.0;
/// The followers' entries under the hero's, 132 pixels apart, four at most (`0x6D6430`).
const ENTRY_STEP: f32 = 132.0;
const MOST_FOLLOWERS: usize = 4;
/// An image read as stored, sampled without its smaller levels, as the original's interface
/// draws its textures (one level): the disc's rings keep their runes as the ellipse shrinks them.
fn crisp(assets: &AssetServer, path: String) -> Handle<Image> {
    assets
        .load_builder()
        .with_settings(|settings: &mut ImageLoaderSettings| {
            settings.is_srgb = false;
            settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                lod_max_clamp: 0.0,
                ..ImageSamplerDescriptor::linear()
            });
        })
        .load(path)
}

/// The interface's images (`tables/ui_images.ron`) and their textures' files by their names.
#[derive(Resource, Default)]
pub struct UiImages {
    images: Vec<UiImage>,
    files: HashMap<String, String>,
}

impl UiImages {
    /// Reads `tables/ui_images.ron` of `assets` and finds its textures; none without the table
    /// (assets converted before it).
    pub fn read(assets: &Path) -> Result<Self> {
        let path = assets.join("tables").join(UI_IMAGES);
        if !files::exists(&path) {
            return Ok(Self::default());
        }
        let text =
            files::read_to_string(&path).map_err(|e| anyhow!("reading {}: {e}", path.display()))?;
        let images: Vec<UiImage> =
            ron::from_str(&text).map_err(|e| anyhow!("parsing {}: {e}", path.display()))?;
        let tiles = crate::world_map::tiles();
        let mut names: Vec<&str> = images
            .iter()
            .map(|image| image.texture.as_str())
            .chain([
                DISC,
                DISC_RING,
                crate::low_health::SONAR,
                crate::art_burn::BURN,
                crate::sparkles::SPARK,
                crate::sparkles::SPARK_HALF,
                crate::spells_page::FLASH,
            ])
            .chain(crate::quest_book::BASES)
            .chain([crate::quest_book::CIRCLE])
            .chain(tiles.iter().map(String::as_str))
            .chain([crate::world_map::PUSHPIN])
            .chain(crate::world_map::PEOPLE)
            .chain([crate::lens_creatures::DOT])
            .collect();
        names.sort_unstable();
        names.dedup();
        let found = hud::find_textures(assets, &names);
        let files = names
            .into_iter()
            .zip(found)
            .filter_map(|(name, file)| Some((name.to_owned(), file?)))
            .collect();
        Ok(Self { images, files })
    }

    /// The file of the texture `name`, one of those found beside the images'.
    pub(crate) fn file(&self, name: &str) -> Option<&str> {
        self.files.get(name).map(String::as_str)
    }

    /// The image `index`: its texture's file and its part in it.
    fn part(&self, index: u32) -> Option<(&str, Rect)> {
        self.turned_part(index).map(|(file, rect, _)| (file, rect))
    }

    /// The image `index`: its texture's file, its part in it, and whether it is turned about
    /// across and along (#1236).
    pub(crate) fn turned_part(&self, index: u32) -> Option<(&str, Rect, [bool; 2])> {
        let image = self.images.get(usize::try_from(index).ok()?)?;
        let file = self.files.get(&image.texture)?;
        let [x, y, w, h] = image.rect.map(f32::from);
        Some((file.as_str(), Rect::new(x, y, x + w, y + h), image.turned))
    }

    /// The image `index`'s anchors relative to its part's top left (#1267), none for an image
    /// it does not have.
    pub(crate) fn anchors(&self, index: u32) -> [f32; 4] {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.images.get(index))
            .map_or([0.0; 4], |image| image.anchors.map(f32::from))
    }

    /// The image `index`'s size in pixels.
    pub(crate) fn size(&self, index: u32) -> Vec2 {
        self.part(index).map_or(Vec2::ZERO, |(_, rect)| rect.size())
    }
}

/// The containers of the bar and of the column, which hold the pieces at their places.
#[derive(Component)]
pub(crate) struct BarRoot;
#[derive(Component)]
pub(crate) struct ColumnRoot;

/// The disc's turning picture.
#[derive(Component)]
struct DiscPicture;

/// The parts of the slots and the rail, made again as the slots' count changes.
#[derive(Component)]
struct SlotPiece;

/// The health's ring's grey part above and red part below.
#[derive(Component)]
pub(crate) struct HealthGrey;
#[derive(Component)]
pub(crate) struct HealthRed;

/// The experience's fill.
#[derive(Component)]
pub(crate) struct ExperienceFill;

/// A piece of a follower's entry in the column: the follower it shows, and for its ring's parts
/// whether the part is the red one and the ring's top in the column; none for the portrait.
#[derive(Component)]
struct Entry {
    follower: Entity,
    ring: Option<(bool, f32)>,
}

/// The followers whose entries the column shows, in their order.
#[derive(Resource, Default)]
struct Entries(Vec<Entity>);

/// An art slot's icon, by its slot: its grey top and its pale or vivid bottom; and its ring.
#[derive(Component)]
struct ArtTop(usize);
#[derive(Component)]
struct ArtBottom(usize);
#[derive(Component)]
struct ArtRing(usize);

/// The slots a side shown, 0 before the first are made.
#[derive(Resource, Default)]
pub(crate) struct Shown(pub usize);

/// The top left of the weapon set `set`'s slot of `count` shown, the first at the left
/// (`0x6E0070`).
pub(crate) fn weapon_slot(set: usize, count: usize) -> Vec2 {
    let (set, count) = (
        f32::from(u16::try_from(set).unwrap_or(0)),
        f32::from(u16::try_from(count).unwrap_or(0)),
    );
    Vec2::new(394.0 + 66.0 * (set - count), SLOT_Y)
}

/// The art slot `i`'s place at the right, from 0 (`0x6E3430`).
pub(crate) fn art_slot(i: usize) -> Vec2 {
    let i = u16::try_from(i).unwrap_or(0);
    Vec2::new(640.0 + 66.0 * f32::from(i), SLOT_Y)
}

/// A side's slots at the hero's level `level` (`0x57F890`).
pub(crate) fn slots(level: u32) -> usize {
    SLOTS_FROM
        .iter()
        .filter(|&&from| level >= from)
        .count()
        .max(1)
}

/// A piece of the interface: the image `index` at `at` of the original's screen, relative to its
/// container's corner `origin`, at its own size.
pub(crate) fn piece(
    images: &UiImages,
    assets: &AssetServer,
    index: u32,
    at: Vec2,
    origin: Vec2,
) -> (ImageNode, Node) {
    let (image, size) = match images.turned_part(index) {
        Some((file, rect, [flip_x, flip_y])) => (
            ImageNode {
                rect: Some(rect),
                flip_x,
                flip_y,
                ..ImageNode::new(hud::image(assets, file.to_owned()))
            },
            rect.size(),
        ),
        None => (ImageNode::default(), Vec2::ZERO),
    };
    (image, placed(at - origin, size))
}

/// A portrait's piece stretched to the column's 64 × 74.
fn stretched((image, mut node): (ImageNode, Node)) -> (ImageNode, Node) {
    node.width = Val::Px(PORTRAIT_SIZE.x);
    node.height = Val::Px(PORTRAIT_SIZE.y);
    (image, node)
}

/// A node at `at` of its container, of `size`.
pub(crate) fn placed(at: Vec2, size: Vec2) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(at.x),
        top: Val::Px(at.y),
        width: Val::Px(size.x),
        height: Val::Px(size.y),
        ..default()
    }
}

/// The bar and the column of a game played.
pub(crate) fn spawn(
    mut commands: Commands,
    images: Res<UiImages>,
    assets: Res<AssetServer>,
    portraits: Res<Portraits>,
    arts: Option<Res<HeroArts>>,
) {
    if images.images.is_empty() {
        return;
    }
    let image = |index: u32, at: Vec2, origin: Vec2| piece(&images, &assets, index, at, origin);
    // The bar, centred along the bottom.
    let bar = commands
        .spawn((
            BarRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                bottom: Val::Px(0.0),
                margin: UiRect::left(Val::Px(-BAR.width() / 2.0)),
                width: Val::Px(BAR.width()),
                height: Val::Px(BAR.height()),
                ..default()
            },
            Pickable::IGNORE,
            DespawnOnExit(AppState::InGame),
        ))
        .id();
    // The disc first: its picture turned in the square, the square stretched across.
    let file = |name: &str| images.files.get(name).cloned();
    if let (Some(disc), Some(ring)) = (file(DISC), file(DISC_RING)) {
        let at = DISC_CENTRE - Vec2::splat(DISC_SIDE / 2.0) - BAR.min;
        let square = || Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        };
        commands.entity(bar).with_children(|bar| {
            bar.spawn((
                placed(at, Vec2::splat(DISC_SIDE)),
                UiTransform::from_scale(Vec2::new(2.0, 1.0)),
                ZIndex(-1),
                Pickable::IGNORE,
            ))
            .with_children(|ellipse| {
                ellipse.spawn((
                    DiscPicture,
                    square(),
                    UiTransform::IDENTITY,
                    ImageNode::new(crisp(&assets, disc)),
                    Pickable::IGNORE,
                ));
                ellipse.spawn((
                    square(),
                    ImageNode::new(crisp(&assets, ring)),
                    Pickable::IGNORE,
                ));
            });
        });
    }
    // The slots and the rail come with the level; the housing and the buttons over them, then
    // the belt.
    let mut top = vec![image(HOUSING.0, HOUSING.1, BAR.min)];
    top.extend(BUTTONS.map(|(index, at)| image(index, at, BAR.min)));
    for bundle in top {
        let child = commands.spawn((bundle, ZIndex(1))).id();
        commands.entity(bar).add_child(child);
    }
    for (kind, at) in (1..).zip(BELT) {
        let child = commands
            .spawn((
                crate::belt::Flask::new(kind),
                image(EMPTY_FLASK, at, BAR.min),
                ZIndex(2),
            ))
            .id();
        commands.entity(bar).add_child(child);
    }
    // The column at the right edge.
    let column = commands
        .spawn((
            ColumnRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(COLUMN.width()),
                height: Val::Px(COLUMN.height()),
                ..default()
            },
            Pickable::IGNORE,
            DespawnOnExit(AppState::InGame),
        ))
        .id();
    // The ring first, the grey part, then the red with its glow (`low_health`), the glass middle
    // under the portrait by the hero's type (`0x435D30`), which is stretched to 64 × 74.
    let kind = arts.as_deref().map_or(1, |arts| arts.kind);
    let mut pieces = vec![
        commands
            .spawn((HealthGrey, image(HEALTH_GREY, RING_AT, COLUMN.min)))
            .id(),
        commands
            .spawn((HealthRed, image(HEALTH_RED, RING_AT, COLUMN.min)))
            .id(),
    ];
    if let Some(portrait) = portraits.column(kind) {
        let portrait = stretched(image(portrait, PORTRAIT_AT, COLUMN.min));
        pieces.push(commands.spawn(portrait).id());
    }
    pieces.extend([
        commands.spawn(image(BONUSES.0, BONUSES.1, COLUMN.min)).id(),
        commands
            .spawn(image(EXPERIENCE_FRAME.0, EXPERIENCE_FRAME.1, COLUMN.min))
            .id(),
        commands
            .spawn((
                ExperienceFill,
                image(EXPERIENCE_FILL.0, EXPERIENCE_FILL.1, COLUMN.min),
            ))
            .id(),
    ]);
    commands.entity(column).add_children(&pieces);
    commands.insert_resource(Shown(0));
    commands.insert_resource(Entries::default());
}

/// The slots and the rail for the hero's level, made again as a side's count changes
/// (`0x6E0070`, `0x6E01F0`): the left's weapon sets, the first selected, and the right's art
/// slots, the first holding the selected art; the rail's ends at their places and the middle
/// pieces between, each chosen at random once.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn rebuild(
    mut commands: Commands,
    mut shown: ResMut<Shown>,
    experience: Res<Experience>,
    images: Res<UiImages>,
    assets: Res<AssetServer>,
    bars: Query<Entity, With<BarRoot>>,
    pieces: Query<Entity, With<SlotPiece>>,
    (sets, mut chosen): (Option<Res<crate::sets::WeaponSets>>, Local<Option<usize>>),
    mut dice: Local<Option<opensacred_rules::spawning::Rng>>,
) {
    let Ok(bar) = bars.single() else {
        return;
    };
    let n = slots(experience.0.level);
    // The weapon set selected (#1348).
    let selected = sets.map_or(0, |sets| sets.selected);
    if shown.0 == n && *chosen == Some(selected) {
        return;
    }
    shown.0 = n;
    *chosen = Some(selected);
    for entity in &pieces {
        commands.entity(entity).despawn();
    }
    let dice = dice.get_or_insert_with(|| opensacred_rules::spawning::Rng::of(0, 0, 1087));
    let mut add = |bundle: (ImageNode, Node), marker: Option<(usize, u8)>, z: i32| {
        let mut entity = commands.spawn((SlotPiece, bundle));
        match marker {
            Some((slot, 0)) => {
                entity.insert(ArtTop(slot));
            }
            Some((slot, 1)) => {
                entity.insert(ArtBottom(slot));
            }
            Some((slot, _)) => {
                entity.insert(ArtRing(slot));
            }
            None => {}
        }
        if z != 0 {
            entity.insert(ZIndex(z));
        }
        let child = entity.id();
        commands.entity(bar).add_child(child);
    };
    let image = |index: u32, at: Vec2| piece(&images, &assets, index, at, BAR.min);
    let count = i16::try_from(n).unwrap_or(1);
    let n = f32::from(count);
    // The rail: the left end, its middle pieces to x 397; the right end, its middle pieces from
    // x 570 + 64n − 64 leftwards while x passes 532.
    let left_end = Vec2::new(362.0 - 66.0 * n, RAIL_LEFT.1);
    add(image(RAIL_LEFT.0, left_end), None, 0);
    let mut x = left_end.x + images.size(RAIL_LEFT.0).x;
    while x < 397.0 {
        let (index, y) = RAIL_LEFT_MIDDLE[usize::from(dice.below(2) == 1)];
        add(image(index, Vec2::new(x, y)), None, 0);
        x += images.size(index).x.max(1.0);
    }
    let mut x = 570.0 + 64.0 * n - 64.0;
    while x > 532.0 {
        let (index, y) = RAIL_RIGHT_MIDDLE[usize::from(dice.below(2) == 1)];
        add(image(index, Vec2::new(x, y)), None, 0);
        x -= images.size(index).x.max(1.0);
    }
    add(
        image(RAIL_RIGHT.0, Vec2::new(570.0 + 64.0 * n, RAIL_RIGHT.1)),
        None,
        0,
    );
    // The art slots at the right, each its art's icon and its ring set at each frame
    // (`art_icon`).
    for i in 0..count {
        let slot = usize::try_from(i).unwrap_or(0);
        let at = art_slot(slot);
        add(image(ART_SELECTED, at), Some((slot, 0)), 0);
        add(image(ART_SELECTED, at), Some((slot, 1)), 0);
        add(image(RING, at), Some((slot, 2)), RING_Z);
    }
    // The weapon sets at the left, the selected one's orb and ring (`0x6E1690`).
    for i in 0..count {
        let set = usize::try_from(i).unwrap_or(0);
        let at = weapon_slot(set, usize::from(count.unsigned_abs()));
        let chosen = set == selected;
        add(
            image(if chosen { WEAPON_SELECTED } else { ORB }, at),
            None,
            0,
        );
        add(
            image(if chosen { RING_SELECTED } else { RING }, at),
            None,
            RING_Z,
        );
    }
}

/// The disc turned by the clock: clockwise a quarter of a degree a minute, the sun at the top at
/// noon.
fn turn(clock: Option<Res<Clock>>, mut discs: Query<&mut UiTransform, With<DiscPicture>>) {
    let Some(clock) = clock else {
        return;
    };
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a minute of the day, 0 to 1,440"
    )]
    let minutes = clock.minutes() as f32;
    for mut transform in &mut discs {
        transform.rotation = Rot2::radians(minutes * DISC_TURN);
    }
}

/// The health's ring: of its 108 rows, the top `⌊97·(1 − f) + 11⌋` grey and the rest red, `f` the
/// hero's health over its most; the experience's fill, ⌊73·e⌋ pixels of `e` the experience into
/// the level over the level's span.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
pub(crate) fn fill(
    images: Res<UiImages>,
    experience: Res<Experience>,
    heroes: Query<&Health, With<Body>>,
    mut greys: Query<(&mut ImageNode, &mut Node), (With<HealthGrey>, Without<HealthRed>)>,
    mut reds: Query<
        (&mut ImageNode, &mut Node),
        (
            With<HealthRed>,
            Without<HealthGrey>,
            Without<ExperienceFill>,
        ),
    >,
    mut fills: Query<
        (&mut ImageNode, &mut Node),
        (
            With<ExperienceFill>,
            Without<HealthGrey>,
            Without<HealthRed>,
        ),
    >,
) {
    let grey = grey_rows(heroes.iter().next().map_or(1.0, health_share));
    let top = (RING_AT - COLUMN.min).y;
    if let Ok((mut image, mut node)) = greys.single_mut() {
        cut_ring(&images, (false, top), grey, &mut image, &mut node);
    }
    if let Ok((mut image, mut node)) = reds.single_mut() {
        cut_ring(&images, (true, top), grey, &mut image, &mut node);
    }
    let level = experience.0.level;
    let (start, end) = (
        opensacred_rules::fight::level_start(level),
        opensacred_rules::fight::level_end(level),
    );
    let into = ratio(
        experience.0.points.saturating_sub(start),
        end.saturating_sub(start).max(1),
    );
    let width = (EXPERIENCE_WIDTH * into).floor();
    if let (Some((_, f)), Ok((mut image, mut node))) =
        (images.part(EXPERIENCE_FILL.0), fills.single_mut())
    {
        image.rect = Some(Rect::new(f.min.x, f.min.y, f.min.x + width, f.max.y));
        node.width = Val::Px(width);
    }
}

/// The share of its most that a creature's health is, 0 for no most.
fn health_share(health: &Health) -> f32 {
    if health.most == 0 {
        0.0
    } else {
        ratio(u64::from(health.current), u64::from(health.most))
    }
}

/// The health's ring's grey rows at the share `share`: ⌊97·(1 − f) + 11⌋.
fn grey_rows(share: f32) -> f32 {
    ((RING_SIZE.y - CLASP) * (1.0 - share) + CLASP).floor()
}

/// Cuts a health's ring's part, the red one or the grey by `(red, top)`, `top` the ring's in the
/// column, at its `grey` rows: the grey's top rows or the red's rest below them.
fn cut_ring(
    images: &UiImages,
    (red, top): (bool, f32),
    grey: f32,
    image: &mut ImageNode,
    node: &mut Node,
) {
    let Some((_, rect)) = images.part(if red { HEALTH_RED } else { HEALTH_GREY }) else {
        return;
    };
    if red {
        image.rect = Some(Rect::new(
            rect.min.x,
            rect.min.y + grey,
            rect.max.x,
            rect.max.y,
        ));
        node.top = Val::Px(top + grey);
        node.height = Val::Px(RING_SIZE.y - grey);
    } else {
        image.rect = Some(Rect::new(
            rect.min.x,
            rect.min.y,
            rect.max.x,
            rect.min.y + grey,
        ));
        node.height = Val::Px(grey);
    }
}

/// The followers' entries under the hero's (`0x6D6430`, `0x6D8250`): the hero's followers, four
/// at most, those shown keeping their places and the newcomers after them, each its health's ring
/// and its portrait by its type (`0x435D30`), none for a type without one; made again as they
/// come, die and go.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn entries(
    mut commands: Commands,
    mut shown: ResMut<Entries>,
    images: Res<UiImages>,
    assets: Res<AssetServer>,
    portraits: Res<Portraits>,
    columns: Query<Entity, With<ColumnRoot>>,
    heroes: Query<Entity, With<Body>>,
    followers: Query<(Entity, &Follower, &Mind)>,
    pieces: Query<Entity, With<Entry>>,
) {
    let (Ok(column), Ok(hero)) = (columns.single(), heroes.single()) else {
        return;
    };
    let theirs = followers
        .iter()
        .filter(|(_, follower, _)| follower.master == hero)
        .map(|(entity, ..)| entity)
        .collect();
    let order = entry_order(&shown.0, theirs);
    if order == shown.0 {
        return;
    }
    for piece in &pieces {
        commands.entity(piece).despawn();
    }
    for (k, &follower) in (1_u8..).zip(&order) {
        let step = Vec2::new(0.0, ENTRY_STEP * f32::from(k));
        let image = |index: u32, at: Vec2| piece(&images, &assets, index, at + step, COLUMN.min);
        let top = (RING_AT + step - COLUMN.min).y;
        let mut parts = vec![
            (image(HEALTH_GREY, RING_AT), Some((false, top))),
            (image(HEALTH_RED, RING_AT), Some((true, top))),
        ];
        let kind = followers.get(follower).map(|(_, _, mind)| mind.creature);
        if let Some(portrait) = kind.ok().and_then(|kind| portraits.column(kind)) {
            parts.push((stretched(image(portrait, PORTRAIT_AT)), None));
        }
        for (bundle, ring) in parts {
            let child = commands.spawn((Entry { follower, ring }, bundle)).id();
            commands.entity(column).add_child(child);
        }
    }
    shown.0 = order;
}

/// The followers that the entries show of the hero's followers `theirs`, those shown in `shown`:
/// those still followers in their places, then the newcomers by their entities' indices, four at
/// most.
fn entry_order(shown: &[Entity], mut theirs: Vec<Entity>) -> Vec<Entity> {
    theirs.sort_unstable_by_key(|entity| entity.index_u32());
    let mut order: Vec<Entity> = shown
        .iter()
        .copied()
        .filter(|entity| theirs.contains(entity))
        .collect();
    theirs.retain(|entity| !order.contains(entity));
    order.extend(theirs);
    order.truncate(MOST_FOLLOWERS);
    order
}

/// The followers' rings split by their health as the hero's.
fn entry_rings(
    images: Res<UiImages>,
    healths: Query<&Health>,
    mut parts: Query<(&Entry, &mut ImageNode, &mut Node)>,
) {
    for (entry, mut image, mut node) in &mut parts {
        let Some(ring) = entry.ring else {
            continue;
        };
        let share = healths.get(entry.follower).map_or(0.0, health_share);
        cut_ring(&images, ring, grey_rows(share), &mut image, &mut node);
    }
}

/// Each art slot's icon (`0x6E3430`): its art ready, the vivid picture whole; else the top
/// `⌊(1 − s)·64⌋` rows of its grey picture over the rest of its pale one, `s` the share
/// regenerated, a combo's of its three (`combos::sprites`) by its own share (`0x581A20`);
/// without an art, the empty slot's orb, the selected one's 479, the others' 635, and the green
/// 533 while the cursor holds a spell, or a weapon art in the edit mode, never a combo; the
/// selected slot's ring 103, the others' 102.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn art_icon(
    (images, assets): (Res<UiImages>, Res<AssetServer>),
    mut loaded: Local<HashMap<String, Handle<Image>>>,
    (slots, held, inventory): (
        Option<Res<crate::art_slots::ArtSlots>>,
        Option<Res<crate::art_slots::HeldArt>>,
        Option<Res<crate::inventory::Shown>>,
    ),
    (arts, combos): (
        Option<Res<HeroArts>>,
        Option<Res<crate::combos::HeroCombos>>,
    ),
    mut tops: Query<(&ArtTop, &mut ImageNode, &mut Node), Without<ArtBottom>>,
    mut bottoms: Query<
        (&ArtBottom, &mut ImageNode, &mut Node),
        (Without<ArtTop>, Without<ArtRing>),
    >,
    mut rings: Query<(&ArtRing, &mut ImageNode), (Without<ArtTop>, Without<ArtBottom>)>,
) {
    let Some(slots) = slots else {
        return;
    };
    let editing = inventory.is_some_and(|inventory| inventory.0);
    let takes = held.and_then(|held| held.0).is_some_and(|art| {
        !crate::combos::is_combo(art) && (crate::art_slots::is_spell(art) || editing)
    });
    let mut set = |image: &mut ImageNode, index: u32, rows: Option<(f32, f32)>| {
        if let Some((file, rect)) = images.part(index) {
            let rect = match rows {
                Some((from, to)) => {
                    Rect::new(rect.min.x, rect.min.y + from, rect.max.x, rect.min.y + to)
                }
                None => rect,
            };
            let handle = loaded
                .entry(file.to_owned())
                .or_insert_with(|| hud::image(&assets, file.to_owned()))
                .clone();
            if image.image != handle {
                image.image = handle;
            }
            image.rect = Some(rect);
        }
    };
    for (ring, mut image) in &mut rings {
        set(
            &mut image,
            if ring.0 == slots.selected {
                RING_SELECTED
            } else {
                RING
            },
            None,
        );
    }
    for (slot, mut top, mut top_node) in &mut tops {
        let Some((_, mut bottom, mut bottom_node)) =
            bottoms.iter_mut().find(|(bottom, ..)| bottom.0 == slot.0)
        else {
            continue;
        };
        let art = slots.arts.get(slot.0).copied().flatten();
        let empty = if takes {
            ART_TAKES
        } else if slot.0 == slots.selected {
            ART_SELECTED
        } else {
            ORB
        };
        let grey_rows = slot_icon(
            art,
            (arts.as_deref(), combos.as_deref()),
            empty,
            (&mut top, &mut bottom),
            &mut set,
        );
        top_node.height = Val::Px(grey_rows);
        bottom_node.top = Val::Px(SLOT_Y - BAR.min.y + grey_rows);
        bottom_node.height = Val::Px(SLOT_SIDE - grey_rows);
    }
}

/// Sets an art slot's two pieces by its art, of `arts`, or its combo, of `combos`, or by its
/// empty orb `empty`: the grey rows of the top, by `set` of an image, its index and its rows.
fn slot_icon(
    art: Option<u16>,
    (arts, combos): (Option<&HeroArts>, Option<&crate::combos::HeroCombos>),
    empty: u32,
    (top, bottom): (&mut ImageNode, &mut ImageNode),
    set: &mut impl FnMut(&mut ImageNode, u32, Option<(f32, f32)>),
) -> f32 {
    let combo = art.zip(combos).and_then(|(art, combos)| {
        let combo = combos.get(art)?;
        Some((crate::combos::sprites(art)?, combo.share()))
    });
    let icon = combo.or_else(|| {
        art.zip(arts).and_then(|(art, arts)| {
            let sprites = arts
                .tables
                .spells
                .iter()
                .find(|spell| spell.id == art)
                .map(|spell| spell.sprites)
                .or_else(|| {
                    arts.tables
                        .weapon_arts
                        .iter()
                        .find(|weapon_art| weapon_art.id == art)
                        .map(|weapon_art| weapon_art.sprites)
                })?;
            let share = arts.arts.get(art).map_or(1.0, |entry| {
                if entry.time <= 0.0 || arts.arts.ready(art) {
                    1.0
                } else {
                    (1.0 - entry.remaining / entry.time).clamp(0.0, 1.0)
                }
            });
            Some((sprites, share))
        })
    });
    match icon {
        Some(([grey, pale, vivid], share)) => {
            let rows = if share >= 1.0 {
                0.0
            } else {
                ((1.0 - share) * SLOT_SIDE).floor()
            };
            set(top, grey, Some((0.0, rows)));
            set(
                bottom,
                if share >= 1.0 { vivid } else { pale },
                Some((rows, SLOT_SIDE)),
            );
            rows
        }
        None => {
            set(top, empty, Some((0.0, 0.0)));
            set(bottom, empty, None);
            0.0
        }
    }
}

/// `part` over `whole`, as a float.
fn ratio(part: u64, whole: u64) -> f32 {
    let narrow = |n: u64| f32::from(u16::try_from(n.min(u64::from(u16::MAX))).unwrap_or(u16::MAX));
    if whole > u64::from(u16::MAX) {
        // Large numbers in thousandths, which a bar of 73 pixels does not tell apart.
        let scale = whole / u64::from(u16::MAX) + 1;
        return (narrow(part / scale) / narrow(whole / scale)).clamp(0.0, 1.0);
    }
    (narrow(part) / narrow(whole)).clamp(0.0, 1.0)
}

/// The belt's slot, 1 to 5, under the point `at` of the original's screen.
pub(crate) fn belt_slot(at: Vec2) -> Option<u8> {
    (1..)
        .zip(BELT)
        .find(|&(_, corner)| Rect::from_corners(corner, corner + BELT_SIDE).contains(at))
        .map(|(slot, _)| slot)
}

/// The pointer on the original's screen as the column is placed, along the window's right edge.
pub(crate) fn column_point(
    pointer: &Pointer,
    ui_scale: &UiScale,
    windows: &Query<&Window>,
) -> Option<Vec2> {
    let at = pointer.0?;
    let window = windows.single().ok()?;
    let scale = ui_scale.0.max(0.01);
    let at = at / scale;
    Some(Vec2::new(at.x - window.width() / scale + SCREEN.x, at.y))
}

/// Whether the pointer is off the bar's and the portraits' areas, where the original's windows
/// take the mouse (`0x6E1BC0`: x 384 − 80n to 640 + 80n and y from 676 for `n` slots a side; the
/// hero's portrait and ring, the followers' entries below, and the points' "+" while it shows): a
/// click there does not reach the world.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn off_the_bar(
    pointer: Res<Pointer>,
    ui_scale: Res<UiScale>,
    windows: Query<&Window>,
    (shown, entries, plus): (
        Option<Res<Shown>>,
        Option<Res<Entries>>,
        Option<Res<crate::points::Plus>>,
    ),
    (bars, images): (Query<(), With<BarRoot>>, Res<UiImages>),
) -> bool {
    let (Some(at), Ok(window), Some(shown)) = (pointer.0, windows.single(), shown) else {
        return true;
    };
    if bars.is_empty() {
        return true;
    }
    let scale = ui_scale.0.max(0.01);
    let width = window.width() / scale;
    let at = at / scale;
    // The bar's places are those of the original's screen centred along the window's width.
    let bar = Vec2::new(at.x - (width - SCREEN.x) / 2.0, at.y);
    let n = f32::from(u16::try_from(shown.0.max(1)).unwrap_or(1));
    let over_bar = bar.y >= BAR.min.y && bar.x >= 384.0 - 80.0 * n && bar.x <= 640.0 + 80.0 * n
        || belt_slot(bar).is_some();
    let column = Vec2::new(at.x - (width - COLUMN.width()), at.y);
    let followers = entries.map_or(0, |entries| entries.0.len());
    let below = ENTRY_STEP * f32::from(u8::try_from(followers).unwrap_or(0));
    let over_portrait = column.x >= 0.0 && column.y <= RING_AT.y + below + RING_SIZE.y + 24.0;
    let over_plus = plus.is_some_and(|plus| plus.shown)
        && crate::points::plus_rect(&images).contains(column + COLUMN.min);
    !(over_bar || over_portrait || over_plus)
}

/// The bar and the portrait's column.
pub struct BarPlugin;

impl Plugin for BarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiImages>()
            .hold_control(off_the_bar)
            .add_systems(OnEnter(AppState::InGame), spawn)
            .add_systems(
                Update,
                (rebuild, turn, fill, art_icon, entries, entry_rings)
                    .chain()
                    .run_if(in_state(AppState::InGame))
                    .run_if(resource_exists::<Shown>),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_side_holds_more_slots_as_the_hero_rises() {
        assert_eq!(slots(1), 1);
        assert_eq!(slots(2), 2);
        assert_eq!(slots(7), 2);
        assert_eq!(slots(8), 3);
        assert_eq!(slots(16), 4);
        assert_eq!(slots(29), 4);
        assert_eq!(slots(30), 5);
        assert_eq!(slots(200), 5);
    }

    #[test]
    fn the_followers_shown_keep_their_places_and_four_at_most_are() {
        let e = |n: u32| Entity::from_raw_u32(n).unwrap();
        assert_eq!(entry_order(&[], vec![e(9), e(3)]), [e(3), e(9)]);
        // One gone: the others keep their order, the newcomer after them.
        assert_eq!(
            entry_order(&[e(9), e(3), e(5)], vec![e(5), e(1), e(9)]),
            [e(9), e(5), e(1)]
        );
        let six = (1..=6).map(e).collect();
        assert_eq!(entry_order(&[], six), [e(1), e(2), e(3), e(4)]);
    }

    #[test]
    fn the_ring_is_grey_above_its_share() {
        assert!((grey_rows(1.0) - CLASP).abs() < 1e-6);
        assert!((grey_rows(0.0) - RING_SIZE.y).abs() < 1e-6);
        // 60 of 214: ⌊97 · 154/214 + 11⌋ = 80.
        assert!((grey_rows(ratio(60, 214)) - 80.0).abs() < 1e-6);
    }

    #[test]
    fn a_share_is_a_part_over_its_whole() {
        assert!((ratio(1, 4) - 0.25).abs() < 1e-6);
        assert!((ratio(5, 4) - 1.0).abs() < 1e-6);
        assert!((ratio(1_000_000, 4_000_000) - 0.25).abs() < 1e-3);
    }
}
