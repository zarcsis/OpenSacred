//! The minimap's ground, which the lens of the world map and the quest book's pictures draw
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
//! Not followed: the fog of war over the explored bitmaps, the quests', the creatures', the
//! party's and the places' icons and the pin, which the quest book leaves out; and the corners
//! faded where a sector lacks a neighbour.

use std::collections::HashMap;
use std::path::Path;

use bevy::prelude::*;
use opensacred_data::texture;

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

/// The sectors' pictures' files, by the sector, found as the game starts.
#[derive(Resource, Default)]
pub struct Minimaps(HashMap<(i32, i32), String>);

impl Minimaps {
    /// The pictures of `textures/texture` of `assets`.
    pub fn read(assets: &Path) -> Self {
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
        Self(pictures)
    }

    /// The file of the sector `sector`'s picture.
    pub fn file(&self, sector: (i32, i32)) -> Option<&str> {
        self.0.get(&sector).map(String::as_str)
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

/// Spawns the ground about the iso plane's point `point` as the children of `parent`, a node
/// whose top left corner stands at `origin` of the screen and which clips them.
pub fn spawn(
    commands: &mut Commands,
    parent: Entity,
    (minimaps, assets): (&Minimaps, &AssetServer),
    point: Vec2,
    origin: Vec2,
) {
    for (sector, corner) in ground(point) {
        let node = crate::bar::placed(corner - origin, Vec2::splat(PICTURE));
        let child = match minimaps.file(sector) {
            Some(file) => commands
                .spawn((
                    node,
                    ImageNode::new(crate::hud::image(assets, file.to_owned())),
                    Pickable::IGNORE,
                ))
                .id(),
            None => commands
                .spawn((node, BackgroundColor(Color::BLACK), Pickable::IGNORE))
                .id(),
        };
        commands.entity(parent).add_child(child);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Seraphim's first quest's picture (measured): its point's iso coordinates (32312,
    /// 138587) in the original's units, its view of (556, 204) 256 × 256 the part at (1200, 488)
    /// of the sectors' pictures laid side by side from x − y = 7 and x + y = 85, to the pixel.
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
