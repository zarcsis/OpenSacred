//! The minimap's ground, which the lens of the world map and of the overview map (`overview`)
//! and the quest book's pictures draw
//! (`0x6CCBD0`; docs/re/questbook.md, "The lens's ground"; #1560): each sector's picture
//! `MINIMAP<x><y>` of `texture.pak` (a sector's x and y in three digits each), 256 × 256, laid on
//! the iso plane at a twelfth of its scale, a cell 4 × 2 pixels: the sector (x, y) at
//! ((x − y) · 256 − 256, (x + y) · 128), its columns of one x − y 256 apart, every other one 128
//! lower. The point the lens shows, a point of the iso plane, stands at the screen's middle
//! (512, 384), whatever part of
//! the screen its view clips to (**measured**: the quest book's picture of the Seraphim's first
//! quest, a view of (556, 204) 256 × 256, matches these pictures so laid to the pixel). The 25
//! sectors within three steps of the point's are drawn, a sector without a picture black.
//!
//! The fog of war over them (`0x638F30`, #1568): each sector's explored bitmap
//! (`opensacred_rules::explored`) as a texture of 32 × 32, black where a bit is clear and
//! clear where it is set, laid on the sector's own diamond, (0, 128), (256, 0), (512, 128) and
//! (256, 256) of its picture's corner, its texels along the sector's x from the top corner to
//! the right one and along its y to the left one.
//!
//! The world map's lens draws the hidden sectors black (#3479). A sector's picture fades toward
//! the places of the grid that lack a sector about it (`0x6CD327`–`0x6CD3C2`, the outcode's
//! switch `0x6CD3E1`; docs/re/questbook.md, "Per vertex", #3609): each of the six vertices of
//! its strip white or black by its links and the switch, the colours blended across the strip's
//! four triangles on the texels' stored values, a copy of the picture made so as it first shows
//! ([`fade`]).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use opensacred_data::texture;
use opensacred_rules::explored::Explored;

use crate::files;
use crate::iso::CELL;

/// A sector's cells along each side, the side of its picture, and the lens's scale of the iso
/// plane (`0x891070`).
const SECTOR_CELLS: f32 = 64.0;
pub const PICTURE: f32 = 256.0;
const SCALE: f32 = 12.0;
/// The screen's point that shows the lens's point.
const MIDDLE: Vec2 = Vec2::new(512.0, 384.0);
/// How many sectors' steps from the point's the ground reaches (`|dx| + |dy|`).
const REACH: i32 = 3;
/// A sector's fog: its bits along each side, and its diamond's width and height.
const FOG_BITS: usize = 32;
const DIAMOND: Vec2 = Vec2::new(512.0, 256.0);
const SEAM: f32 = 2.0;

/// The sectors' pictures' files, by the sector, found as the game starts, and the places of the
/// grid that have a sector, by which a picture fades ([`outcode`]).
#[derive(Resource, Default)]
pub struct Minimaps {
    files: HashMap<(i32, i32), String>,
    grid: HashSet<(i32, i32)>,
}

impl Minimaps {
    /// The pictures of `textures/texture` of `assets`, and the grid of `world`'s sectors.
    pub fn read(assets: &Path, world: &opensacred_data::world::World) -> Self {
        let dir = assets.join("textures").join("texture");
        let suffix = format!(".{}", texture::EXTENSION);
        let pictures = files::read_dir(&dir)
            .into_iter()
            .flatten()
            .filter_map(|path| {
                let file = path.file_name()?.to_str()?.to_owned();
                let (_, name) = file.strip_suffix(&suffix)?.split_once('-')?;
                let digits = name.strip_prefix("minimap")?;
                let (x, y) = (
                    digits.get(..3)?.parse().ok()?,
                    digits.get(3..)?.parse().ok()?,
                );
                Some(((x, y), format!("textures/texture/{file}")))
            })
            .collect();
        let grid = world
            .sectors
            .iter()
            .filter_map(|sector| {
                Some((i32::try_from(sector.x).ok()?, i32::try_from(sector.y).ok()?))
            })
            .collect();
        Self {
            files: pictures,
            grid,
        }
    }

    /// The file of the sector `sector`'s picture.
    pub fn file(&self, sector: (i32, i32)) -> Option<&str> {
        self.files.get(&sector).map(String::as_str)
    }

    /// The vertices of the sector `sector`'s picture that the lens draws black ([`blacks`]).
    fn blacks(&self, sector: (i32, i32)) -> [bool; 6] {
        blacks(outcode(|place| self.grid.contains(&place), sector))
    }
}

/// The links that the lens reads of a sector (`0x6CD327`–`0x6CD3C2`; docs/re/questbook.md, "The
/// links"): the neighbour's place by its offset and the outcode's bit that its empty link sets,
/// (x − 1, y − 1) `0x01`, (x − 1, y) `0x02`, (x − 1, y + 1) `0x04`, (x, y + 1) `0x08`,
/// (x + 1, y + 1) `0x10`, (x + 1, y) `0x20` and (x, y − 1) `0x40`; the link to (x + 1, y − 1)
/// is not read.
const LINKS: [((i32, i32), u8); 7] = [
    ((-1, -1), 0x01),
    ((-1, 0), 0x02),
    ((-1, 1), 0x04),
    ((0, 1), 0x08),
    ((1, 1), 0x10),
    ((1, 0), 0x20),
    ((0, -1), 0x40),
];

/// The outcode of the sector at `(x, y)`: the bits of its links ([`LINKS`]) toward a place of the
/// grid that has no sector by `has`, a link being empty exactly there (docs/02 §5.1).
fn outcode(has: impl Fn((i32, i32)) -> bool, (x, y): (i32, i32)) -> u8 {
    LINKS
        .iter()
        .filter(|&&((dx, dy), _)| !has((x + dx, y + dy)))
        .fold(0, |code, &(_, bit)| code | bit)
}

/// The outcodes for which the switch (`0x6CD3E1`) blacks the right middle, the sector's own
/// middle: its 18 values but `0x01`, `0x07` and `0x10`.
const RIGHT_MIDDLE: [u8; 15] = [
    0x20, 0x21, 0x30, 0x38, 0x3C, 0x40, 0x41, 0x43, 0x47, 0x60, 0x61, 0x63, 0x70, 0x71, 0x73,
];

/// Which of the six vertices of a picture's strip of the outcode `code` the lens draws black, in
/// the strip's order: the top left, the top right, the left middle, the right middle, the bottom
/// left and the bottom right (docs/re/questbook.md, "Per vertex"); each by the links of the
/// sectors it lies on, and the switch's own (`0x6CD3E1`).
fn blacks(code: u8) -> [bool; 6] {
    let any = |bits: u8| code & bits != 0;
    [
        any(0x02) || matches!(code, 0x01 | 0x21 | 0x41 | 0x61 | 0x71),
        any(0x01 | 0x40 | 0x02),
        any(0x08 | 0x04 | 0x02),
        RIGHT_MIDDLE.contains(&code),
        any(0x08) || matches!(code, 0x10 | 0x30 | 0x70 | 0x71 | 0x73),
        any(0x20 | 0x10 | 0x08),
    ]
}

/// The colour's share at the point `(u, v)` of a picture's square, 0 to 1 across and down, of the
/// vertices `black`: the white of 1 and the black of 0 blended linearly across the strip's
/// triangle that holds the point, (top left, top right, left middle), (top right, left middle,
/// right middle), (left middle, right middle, bottom left) or (right middle, bottom left, bottom
/// right), as Direct3D's Gouraud shading blends a strip's colours (**seen**, #3609).
fn shade(black: [bool; 6], (u, v): (f32, f32)) -> f32 {
    let white = black.map(|black| if black { 0.0 } else { 1.0 });
    let [tl, tr, lm, rm, bl, br] = white;
    // Each half in its own coordinates, its upper corners at 0 down and its lower at 1.
    let (upper, w) = if v <= 0.5 {
        ((tl, tr, lm, rm), 2.0 * v)
    } else {
        ((lm, rm, bl, br), 2.0 * v - 1.0)
    };
    let (top_left, top_right, bottom_left, bottom_right) = upper;
    if u + w <= 1.0 {
        top_left * (1.0 - u - w) + top_right * u + bottom_left * w
    } else {
        top_right * (1.0 - w) + bottom_left * (1.0 - u) + bottom_right * (u + w - 1.0)
    }
}

/// `rgba`'s texels, `width` across, faded toward the black vertices `black`: each texel's stored
/// values times the colour's share at its centre ([`shade`]), as the device modulates the texture
/// by the diffuse colour with no gamma; the alpha as it is.
fn fade_texels(rgba: &mut [u8], width: usize, black: [bool; 6]) {
    let height = rgba.len() / 4 / width.max(1);
    let size = |n: usize| f32::from(u16::try_from(n).unwrap_or(u16::MAX));
    for (index, texel) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let (x, y) = (index % width, index / width);
        let share = shade(
            black,
            (
                (size(x) + 0.5) / size(width),
                (size(y) + 0.5) / size(height),
            ),
        );
        for value in &mut texel[..3] {
            *value = byte(f32::from(*value) * share);
        }
    }
}

/// A value of 0 to 255 rounded to its byte.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a value held within a byte's range"
)]
fn byte(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

/// A copy of the picture `source` faded toward its black vertices `black`, of its first level's
/// texels in 8 bits: its BC7 blocks decoded, or its texels as they are; none of another format.
fn faded(source: &Image, black: [bool; 6]) -> Option<Image> {
    let (width, height) = (
        usize::try_from(source.width()).ok()?,
        usize::try_from(source.height()).ok()?,
    );
    let data = source.data.as_ref()?;
    let (mut rgba, format) = match source.texture_descriptor.format {
        TextureFormat::Bc7RgbaUnorm | TextureFormat::Bc7RgbaUnormSrgb => {
            let mut rgba = vec![0; 4 * width * height];
            let across = width.div_ceil(4);
            let level = data.get(..16 * across * height.div_ceil(4))?;
            for (index, block) in level.as_chunks::<16>().0.iter().enumerate() {
                let (bx, by) = (index % across, index / across);
                let mut texels = [0; 64];
                bcdec_rs::bc7(block, &mut texels, 16);
                for row in 0..4 {
                    let y = 4 * by + row;
                    let take = 4.min(width.saturating_sub(4 * bx));
                    if y < height {
                        let at = 4 * (y * width + 4 * bx);
                        rgba[at..at + 4 * take]
                            .copy_from_slice(&texels[16 * row..16 * row + 4 * take]);
                    }
                }
            }
            let format = if source.texture_descriptor.format == TextureFormat::Bc7RgbaUnormSrgb {
                TextureFormat::Rgba8UnormSrgb
            } else {
                TextureFormat::Rgba8Unorm
            };
            (rgba, format)
        }
        format @ (TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb) => {
            (data.get(..4 * width * height)?.to_vec(), format)
        }
        _ => return None,
    };
    fade_texels(&mut rgba, width, black);
    let mut image = Image::new(
        Extent3d {
            width: source.width(),
            height: source.height(),
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba,
        format,
        // Kept in the main world too, where the interface's layers read its size (`ui_layers`).
        RenderAssetUsages::default(),
    );
    image.sampler = source.sampler.clone();
    Some(image)
}

/// A picture of the lens that fades toward its sector's missing neighbours, shown as it is until
/// its faded copy is made ([`fade`]): its sector, its black vertices and its own image.
#[derive(Component)]
struct Fading {
    sector: (i32, i32),
    black: [bool; 6],
    source: Handle<Image>,
}

/// The faded copies of the sectors' pictures made, kept for the lens's next draws.
#[derive(Resource, Default)]
struct Faded(HashMap<(i32, i32), Handle<Image>>);

/// Shows each fading picture as its faded copy, made once its own image has loaded and kept for
/// its sector ([`faded`]); a picture of a format it cannot fade shows as it is.
fn fade(
    mut commands: Commands,
    mut fading: Query<(Entity, &Fading, &mut ImageNode)>,
    mut images: ResMut<Assets<Image>>,
    mut made: ResMut<Faded>,
) {
    for (entity, picture, mut node) in &mut fading {
        let handle = if let Some(handle) = made.0.get(&picture.sector) {
            handle.clone()
        } else {
            let Some(source) = images.get(&picture.source) else {
                continue;
            };
            match faded(source, picture.black) {
                Some(image) => {
                    let handle = images.add(image);
                    made.0.insert(picture.sector, handle.clone());
                    handle
                }
                None => {
                    warn!(
                        "minimap: the picture of {:?} is of a format the lens cannot fade",
                        picture.sector
                    );
                    picture.source.clone()
                }
            }
        };
        node.image = handle;
        commands.entity(entity).remove::<Fading>();
    }
}

/// The lens's faded pictures.
pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Faded>().add_systems(Update, fade);
    }
}

/// The iso plane's point of the world point `at` (`0x622620`): `(x − y) · 48 / 53.66563` across,
/// `(x + y) · 24 / 53.66563` down.
pub fn iso(at: Vec2) -> Vec2 {
    Vec2::new((at.x - at.y) * 48.0 / CELL, (at.x + at.y) * 24.0 / CELL)
}

/// The world point of the iso plane's point `point` (`0x622660`).
pub fn world(point: Vec2) -> Vec2 {
    Vec2::new(
        (point.x / 48.0 + point.y / 24.0) * CELL / 2.0,
        (point.y / 24.0 - point.x / 48.0) * CELL / 2.0,
    )
}

/// The sectors whose pictures the lens about the iso plane's point `point` draws, and each
/// picture's top left corner on the screen.
pub fn ground(point: Vec2) -> Vec<((i32, i32), Vec2)> {
    let at = world(point);
    let point = point / SCALE;
    let side = SECTOR_CELLS * CELL;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a world point's sector, within the world's 128"
    )]
    let (x, y) = ((at.x / side).floor() as i32, (at.y / side).floor() as i32);
    let mut sectors = Vec::new();
    for dx in -REACH..=REACH {
        for dy in -REACH..=REACH {
            if dx.abs() + dy.abs() > REACH {
                continue;
            }
            let (sx, sy) = (x + dx, y + dy);
            if sx < 0 || sy < 0 {
                continue;
            }
            let corner = corner((sx, sy));
            sectors.push(((sx, sy), corner - point + MIDDLE));
        }
    }
    sectors
}

/// The top left corner of the sector `sector`'s picture on the lens's plane.
fn corner((x, y): (i32, i32)) -> Vec2 {
    let (across, down) = (
        f32::from(i16::try_from(x - y).unwrap_or(0)),
        f32::from(i16::try_from(x + y).unwrap_or(0)),
    );
    Vec2::new(across * PICTURE - PICTURE, down * PICTURE / 2.0)
}

/// The sectors of the ground about the iso plane's point `point` in the order the lens draws them
/// (`0x6CCBD0`): a column of one x − y at a time from the left, each from the top down.
fn in_order(point: Vec2) -> Vec<((i32, i32), Vec2)> {
    let mut sectors = ground(point);
    sectors.sort_by_key(|&((x, y), _)| (x - y, x + y));
    sectors
}

/// Spawns the ground about the iso plane's point `point` as the children of `parent`, a node
/// whose top left corner stands at `origin` of the screen and which clips them, at the alpha
/// `alpha` (the call's, `alpha << 24 | 0xFFFFFF`), in the lens's order: each sector's picture, and
/// a sector without one, or hidden where `hides` names the dungeons whose hidden sectors it hides
/// (the world map's lens, `0x6CA81A`), black of the alpha, its square and then its own diamond,
/// whose right half the next column's pictures cover (`0x6CDADA`; docs/re/questbook.md, "The
/// lens's ground", #3479).
pub fn spawn(
    commands: &mut Commands,
    parent: Entity,
    (minimaps, assets): (&Minimaps, &AssetServer),
    (point, origin): (Vec2, Vec2),
    (alpha, hides): (f32, Option<&crate::daynight::Dungeons>),
) {
    for (sector, corner) in in_order(point) {
        let node = crate::bar::placed(corner - origin, Vec2::splat(PICTURE));
        let file = minimaps
            .file(sector)
            .filter(|_| !hides.is_some_and(|dungeons| dungeons.hides(sector)));
        if let Some(file) = file {
            let image = crate::hud::image(assets, file.to_owned());
            let black = minimaps.blacks(sector);
            let mut child = commands.spawn((
                node,
                ImageNode::new(image.clone()).with_color(Color::WHITE.with_alpha(alpha)),
                Pickable::IGNORE,
            ));
            // A picture that fades takes its faded copy once it is made (`fade`).
            if black.contains(&true) {
                child.insert(Fading {
                    sector,
                    black,
                    source: image,
                });
            }
            let child = child.id();
            commands.entity(parent).add_child(child);
            continue;
        }
        let black = BackgroundColor(Color::BLACK.with_alpha(alpha));
        let square = commands.spawn((node, black, Pickable::IGNORE)).id();
        let diamond = on_diamond(commands, corner - origin, black);
        commands.entity(parent).add_children(&[square, diamond]);
    }
}

/// A sector's diamond whose picture's top left corner stands at `corner` of its parent, filled
/// with `fill` (an image or a colour): a square turned by 45° within a box squeezed to half its
/// height, a pixel wider so that the neighbours' diamonds leave no seam between their smoothed
/// edges; the box, to add to the parent.
fn on_diamond(commands: &mut Commands, corner: Vec2, fill: impl Bundle) -> Entity {
    let side = (DIAMOND.x + SEAM) / std::f32::consts::SQRT_2;
    let squeezed = commands
        .spawn((
            crate::bar::placed(
                corner - Vec2::new(0.0, DIAMOND.x / 2.0 - DIAMOND.y / 2.0),
                Vec2::splat(DIAMOND.x),
            ),
            UiTransform::from_scale(Vec2::new(1.0, DIAMOND.y / DIAMOND.x)),
            Pickable::IGNORE,
        ))
        .id();
    let turned = commands
        .spawn((
            crate::bar::placed(Vec2::splat((DIAMOND.x - side) / 2.0), Vec2::splat(side)),
            UiTransform::from_rotation(Rot2::radians(std::f32::consts::FRAC_PI_4)),
            fill,
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(squeezed).add_child(turned);
    squeezed
}

/// A sector's fog (`0x638F30`): black where its bitmap's bit is clear, opaque, or where `opaque`
/// is false at the 4-bit alpha 8 of its texture's 15 (`0x8000` against `0xF000`), and clear
/// where it is set.
pub fn fog(explored: &Explored, sector: (i32, i32), opaque: bool) -> Image {
    let shade = if opaque { u8::MAX } else { HALF_FOG };
    let sector = (
        u16::try_from(sector.0).unwrap_or(u16::MAX),
        u16::try_from(sector.1).unwrap_or(u16::MAX),
    );
    let mut texels = Vec::with_capacity(FOG_BITS * FOG_BITS * 4);
    for row in 0..FOG_BITS {
        for column in 0..FOG_BITS {
            let alpha = if explored.bit(sector, column, row) {
                0
            } else {
                shade
            };
            texels.extend_from_slice(&[0, 0, 0, alpha]);
        }
    }
    let side = u32::try_from(FOG_BITS).unwrap_or(32);
    Image::new(
        Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        texels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// The half fog's alpha: the 4-bit 8, as a byte.
const HALF_FOG: u8 = 8 * 17;

/// Spawns the fog over the ground about the iso plane's point `point` as the children of
/// `parent`, whose top left corner stands at `origin` of the screen: each sector's fog texture on
/// its diamond ([`on_diamond`]); opaque where `opaque` (the lens's call forcing it, or
/// `EXPLOREMAP`, `0x638F30`), else at half; none over a hidden sector where `hides` names the
/// dungeons whose hidden sectors the lens strikes from its list (the world map's, #3479).
pub fn spawn_fog(
    commands: &mut Commands,
    parent: Entity,
    (explored, images): (&Explored, &mut Assets<Image>),
    (point, origin): (Vec2, Vec2),
    (opaque, hides): (bool, Option<&crate::daynight::Dungeons>),
) {
    for (sector, corner) in ground(point) {
        if hides.is_some_and(|dungeons| dungeons.hides(sector)) {
            continue;
        }
        let image = images.add(fog(explored, sector, opaque));
        let diamond = on_diamond(commands, corner - origin, ImageNode::new(image));
        commands.entity(parent).add_child(diamond);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pictures_vertices_go_black_by_its_links_and_the_outcodes_switch() {
        const TL: usize = 0;
        const TR: usize = 1;
        const LM: usize = 2;
        const RM: usize = 3;
        const BL: usize = 4;
        const BR: usize = 5;
        let black = |code: u8| -> Vec<usize> {
            blacks(code)
                .iter()
                .enumerate()
                .filter(|&(_, &black)| black)
                .map(|(vertex, _)| vertex)
                .collect()
        };
        // No empty link: white all over; the links alone, the switch adding none.
        assert_eq!(black(0x00), Vec::<usize>::new());
        assert_eq!(black(0x02), [TL, TR, LM]);
        assert_eq!(black(0x1C), [LM, BL, BR]);
        assert_eq!(black(0x1F), [TL, TR, LM, BL, BR]);
        // The switch's values, the table's last column (docs/re/questbook.md).
        let table: [(u8, &[usize]); 18] = [
            (0x01, &[TL, TR]),
            (0x07, &[TL, TR, LM]),
            (0x10, &[BL, BR]),
            (0x20, &[RM, BR]),
            (0x21, &[TL, TR, RM, BR]),
            (0x30, &[RM, BL, BR]),
            (0x38, &[LM, RM, BL, BR]),
            (0x3C, &[LM, RM, BL, BR]),
            (0x40, &[TR, RM]),
            (0x41, &[TL, TR, RM]),
            (0x43, &[TL, TR, LM, RM]),
            (0x47, &[TL, TR, LM, RM]),
            (0x60, &[TR, RM, BR]),
            (0x61, &[TL, TR, RM, BR]),
            (0x63, &[TL, TR, LM, RM, BR]),
            (0x70, &[TR, RM, BL, BR]),
            (0x71, &[TL, TR, RM, BL, BR]),
            (0x73, &[TL, TR, LM, RM, BL, BR]),
        ];
        for (code, all) in table {
            assert_eq!(black(code), all, "the outcode {code:#04x}");
        }
        // Another value with a missing neighbour on the right leaves the right middle white.
        assert!(!blacks(0x2C)[RM] && !blacks(0x7F)[RM]);
    }

    #[test]
    fn a_sectors_outcode_marks_its_links_toward_places_without_a_sector() {
        let grid: HashSet<(i32, i32)> = [(5, 5), (4, 5), (5, 6), (6, 6), (6, 5), (5, 4), (4, 4)]
            .into_iter()
            .collect();
        let has = |place| grid.contains(&place);
        // All seven read neighbours there, (6, 4) unread.
        assert_eq!(outcode(has, (5, 5)), 0x04);
        assert_eq!(outcode(has, (6, 5)) & 0x20, 0x20);
        // At the grid's edge every link is empty.
        assert_eq!(outcode(|_| false, (0, 0)), 0x7F);
    }

    #[test]
    fn the_strips_colours_blend_across_its_four_triangles() {
        let near = |a: f32, b: f32| (a - b).abs() < 1e-5;
        let white = [false; 6];
        assert!(near(shade(white, (0.3, 0.7)), 1.0));
        // The top left alone: black at it, white at the opposite edge, half way between.
        let top_left = [true, false, false, false, false, false];
        assert!(near(shade(top_left, (0.0, 0.0)), 0.0));
        assert!(near(shade(top_left, (0.5, 0.0)), 0.5));
        assert!(near(shade(top_left, (0.0, 0.25)), 0.5));
        // The upper half splits from the top right to the left middle: beyond it, white.
        assert!(near(shade(top_left, (0.6, 0.3)), 1.0));
        // The lower half splits from the right middle to the bottom left: the bottom right's
        // black reaches the right middle's corner of its triangle alone.
        let bottom_right = [false, false, false, false, false, true];
        assert!(near(shade(bottom_right, (1.0, 1.0)), 0.0));
        assert!(near(shade(bottom_right, (0.25, 0.875)), 1.0));
        assert!(near(shade(bottom_right, (1.0, 0.75)), 0.5));
        // The right middle's black ramps across both halves.
        let right_middle = [false, false, false, true, false, false];
        assert!(near(shade(right_middle, (1.0, 0.5)), 0.0));
        assert!(near(shade(right_middle, (1.0, 0.25)), 0.5));
        assert!(near(shade(right_middle, (0.5, 0.5)), 0.5));
    }

    #[test]
    fn a_faded_picture_takes_its_texels_stored_values_times_the_colour() {
        // Two texels across and two down, the right middle and the vertices below black.
        let mut rgba = vec![
            200, 100, 50, 255, 200, 100, 50, 255, 200, 100, 50, 255, 200, 100, 50, 128,
        ];
        let black = [false, false, true, true, true, true];
        fade_texels(&mut rgba, 2, black);
        // The upper texels at (0.25, 0.25) and (0.75, 0.25), the lower ones black.
        let share = shade(black, (0.25, 0.25));
        assert_eq!(
            rgba[..4],
            [
                byte(200.0 * share),
                byte(100.0 * share),
                byte(50.0 * share),
                255
            ]
        );
        assert_eq!(rgba[8..], [0, 0, 0, 255, 0, 0, 0, 128]);
        // An image of eight bits a channel, faded so whole.
        let image = Image::new(
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            vec![10, 20, 30, 40],
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::all(),
        );
        let made = faded(&image, [true; 6]).map(|image| image.data);
        assert_eq!(made, Some(Some(vec![0, 0, 0, 40])));
    }

    /// The Seraphim's first quest's picture (measured): its point's iso coordinates (32312,
    /// 138587) in the original's units, its view of (556, 204) 256 × 256 the part at (1200, 488)
    /// of the sectors' pictures laid side by side from x − y = 7 and x + y = 85, to the pixel.
    #[test]
    fn the_lens_draws_its_columns_from_the_left_each_from_the_top() {
        let point = Vec2::new(32_312.0, 138_587.0);
        let order: Vec<(i32, i32)> = in_order(point).into_iter().map(|(at, _)| at).collect();
        assert_eq!(order.len(), 25);
        let keys: Vec<(i32, i32)> = order.iter().map(|&(x, y)| (x - y, x + y)).collect();
        assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
        // Each column's pictures lie one under the other, the next column's beside them.
        let (_, first) = in_order(point)[0];
        let (_, second) = in_order(point)[1];
        assert!(second.x >= first.x);
    }

    #[test]
    fn the_lens_lays_the_pictures_about_its_point_at_the_screens_middle() {
        let point = Vec2::new(32_312.0, 138_587.0);
        assert!((iso(world(point)) - point).length() < 0.01);
        let sectors = ground(point);
        assert_eq!(sectors.len(), 25);
        let (_, corner) = sectors
            .iter()
            .find(|(sector, _)| *sector == (50, 39))
            .copied()
            .unwrap_or_default();
        // The sector (50, 39)'s picture, of x − y = 11 and x + y = 89, stands at (4 · 256,
        // 4 · 128) of the pictures laid side by side, the view's corner at (1200, 488) of them.
        let view = corner - Vec2::new(4.0 * 256.0, 4.0 * 128.0) + Vec2::new(1200.0, 488.0);
        assert!(
            (view - Vec2::new(556.0, 204.0)).length() < 1.5,
            "the view's corner at {view}"
        );
    }
}
