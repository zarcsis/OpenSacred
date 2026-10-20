//! The cell under a point of the screen, as the original finds it for the cursor and for an
//! order of the player (`0x616AD0`, docs/re/movement.md, "The cursor's point"): from the cell
//! under the point at height 0 it walks down the screen, a cell along both axes a step, and
//! takes, of the walkable cells of the ground and of the buildings' floors that count, the one
//! whose point at its height lies near where the point's line of sight crosses that height, the
//! nearest the eye. A click sends the hero there, onto that cell's level (`0x617030`).

use bevy::prelude::*;
use bevy::shape::Ray3d;

use crate::buildings::{self, Buildings};
use crate::iso;
use crate::paths::{self, Cell};
use crate::terrain::{Heights, Level};

/// The cells tested at each step of the walk, from the step's own (`0x95F208`).
const AROUND: [Cell; 3] = [(0, 0), (1, 0), (0, 1)];
/// The steps of the walk besides those of the hero's height.
const STEPS: f32 = 8.0;
/// Units of the models' height per step that the hero's height adds.
const HEIGHT_PER_STEP: f32 = 24.0;
/// How near the line of sight passes a cell's point, squared, in units of the models' frame
/// (`0x616AD0`); [`near`] in world units.
const NEAR_IN_MODEL_UNITS: f32 = 1200.0;

/// How near, squared, in world units, the line of sight must pass a cell's point at its height:
/// about a cell's side (**hypothesis**: the client measures it in the models' frame, whose unit is
/// [`iso::MODEL_SCALE`] world units across the screen and a little less along it).
fn near() -> f32 {
    NEAR_IN_MODEL_UNITS * iso::MODEL_SCALE * iso::MODEL_SCALE
}

/// What a point of the screen picks: a point of the world and its level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Picked {
    pub point: Vec3,
    pub level: Level,
}

/// The cell under `ray`, the line of sight of a point of the screen, for a hero standing at
/// height `y` on `level`: of the cells the walk passes, on the ground and on each floor of a
/// building there that counts, the walkable one whose point at its height lies nearest the
/// line's crossing of that height, the nearest the eye; `None` when none does.
///
/// A floor `n` counts when its building is open to the sky, when `n` is 1 and the hero is on the
/// ground, when the building's state shows it, or when the hero stands on a floor `n`. The point
/// is the line's crossing of the ground in the cell under the screen's point, else the centre of
/// the cell found.
pub fn pick(
    ray: Ray3d,
    heights: &Heights,
    buildings: &Buildings,
    (y, level): (f32, Level),
) -> Option<Picked> {
    let direction = ray.direction.as_vec3();
    if direction.y >= 0.0 {
        return None;
    }
    // Where the line of sight crosses a height, and how far along it.
    let along = |height: f32| (height - ray.origin.y) / direction.y;
    let crossing = |height: f32| ray.origin + direction * along(height);
    let ground = crossing(0.0);
    let start = paths::cell_of(ground.x, ground.z);
    let units = y / iso::height(iso::PIXELS_PER_MODEL_HEIGHT);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a few dozen steps, truncated as the client's `__ftol` does"
    )]
    let steps = (STEPS + (units / HEIGHT_PER_STEP).abs().trunc()) as i32;
    let own_floor = heights
        .floor(level)
        .map_or(0, |(_, floor)| usize::from(floor));
    let counts = |building: buildings::Anchor, floor: usize| {
        heights.open(building)
            || (floor == 1 && level.is_none())
            || usize::try_from(buildings::layer(buildings.state(building)))
                .is_ok_and(|shown| shown == floor)
            || floor == own_floor
    };
    let mut best: Option<(f32, Picked)> = None;
    for step in 0..steps {
        for (dx, dy) in AROUND {
            let cell = (start.0 + step + dx, start.1 + step + dy);
            #[expect(
                clippy::cast_precision_loss,
                reason = "a global cell, below 6,400, is exact in an f32"
            )]
            let centre = iso::ground(cell.0 as f32 + 0.5, cell.1 as f32 + 0.5);
            let building = heights
                .building(centre.x, centre.z)
                .map(|(building, _)| building);
            let floors = building.map_or(0, |building| heights.floor_count(building));
            for floor in 0..=floors {
                let at = match (floor, building) {
                    (0, _) => None,
                    (_, Some(building)) if counts(building, floor) => {
                        match heights.floor_level(building, floor) {
                            Some(key) => Some(key),
                            None => continue,
                        }
                    }
                    _ => continue,
                };
                if !heights.walkable_at(cell, at) {
                    continue;
                }
                let Some(height) = heights.standing(centre.x, centre.z, at) else {
                    continue;
                };
                let seen = crossing(height);
                if (seen.xz() - centre.xz()).length_squared() > near() {
                    continue;
                }
                let t = along(height);
                if best.is_some_and(|(nearest, _)| nearest <= t) {
                    continue;
                }
                let point = if cell == start && at.is_none() {
                    ground.with_y(height)
                } else {
                    centre.with_y(height)
                };
                best = Some((t, Picked { point, level: at }));
            }
        }
    }
    best.map(|(_, picked)| picked)
}

#[cfg(test)]
mod tests {
    use opensacred_data::world::{self, Building, Layer, LayerCell, SIDE, Sector};

    use super::*;

    /// A cell of the ground: walkable unless `ground`'s low nibble makes it a mass or a wall.
    fn cell(ground: u8, flags: u8, offsets: [i8; 2], raised: i8) -> world::Cell {
        world::Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [raised; 4],
            offsets,
            flags,
            ground,
        }
    }

    /// Sector (0, 0), flat but for `changes`, and a building over cells 30..=32 anchored at
    /// (30, 30), a mass on the ground, of two floors, the second 112 units of the models' height
    /// up, open to the sky or not.
    fn world(changes: &[((usize, usize), world::Cell)], open: bool) -> Heights {
        let mut cells = vec![cell(0x40, 0, [0, 0], 0); SIDE * SIDE];
        let back = |v: usize| i8::try_from(30 - i64::try_from(v).unwrap()).unwrap();
        for y in 30..=32_usize {
            for x in 30..=32_usize {
                cells[y * SIDE + x] = cell(0x41, 0x1, [back(x), back(y)], 0);
            }
        }
        for ((x, y), c) in changes {
            cells[y * SIDE + x] = c.clone();
        }
        let free = LayerCell {
            heights2: [0; 4],
            offsets: [0; 2],
            flags: 0,
            ground: 0x40,
        };
        let floor = Layer {
            x: 30,
            y: 30,
            width: 3,
            height: 3,
            cells: vec![free; 9],
        };
        let sector = Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells,
            statics: Vec::new(),
            layers: vec![floor.clone(), floor],
            buildings: vec![Building {
                cell: u16::try_from(30 * SIDE + 30).unwrap(),
                floors: vec![1, 2],
                heights: vec![0, 112],
                open,
            }],
        };
        let mut heights = Heights::default();
        heights.add(&sector);
        heights
    }

    /// The line of sight through the point of cell fractions `(x, y)` at `height`.
    fn sight(x: f32, y: f32, height: f32) -> Ray3d {
        let point = iso::ground(x, y).with_y(height);
        let forward = iso::forward();
        Ray3d::new(point - forward * 5_000.0, Dir3::new(forward).unwrap())
    }

    /// The cell and the level of what a click picks.
    fn picked_cell(picked: Option<Picked>) -> Option<(Cell, Level)> {
        picked.map(|p| (paths::cell_of(p.point.x, p.point.z), p.level))
    }

    #[test]
    fn a_click_on_flat_ground_picks_the_point_under_the_cursor() {
        let heights = world(&[], false);
        let ray = sight(20.7, 20.5, 0.0);
        let picked = pick(ray, &heights, &Buildings::default(), (0.0, None)).unwrap();
        assert_eq!(picked.level, None);
        // The line's crossing itself, in the cell under the cursor.
        assert!(
            (picked.point - iso::ground(20.7, 20.5)).length() < 0.01,
            "{picked:?}"
        );
    }

    #[test]
    fn a_raised_cell_whose_top_shows_under_the_cursor_wins_unless_it_is_a_wall() {
        // Cell (24, 24) raised by 40 steps: its top is drawn over the flat cells up the screen.
        let top = |ground| ((24, 24), cell(ground, 0, [0, 0], 40));
        let heights = world(&[top(0x40)], false);
        let centre = iso::ground(24.5, 24.5);
        let height = heights.standing(centre.x, centre.z, None).unwrap();
        assert!(height > iso::CELL, "{height}");
        let ray = sight(24.5, 24.5, height);
        let buildings = Buildings::default();
        assert_eq!(
            picked_cell(pick(ray, &heights, &buildings, (0.0, None))),
            Some(((24, 24), None))
        );
        // A wall there: the flat ground the line reaches instead, up the screen.
        let heights = world(&[top(0x42)], false);
        let (at, level) = picked_cell(pick(ray, &heights, &buildings, (0.0, None))).unwrap();
        assert_eq!(level, None);
        assert!(at.0 < 24 && at.1 < 24, "{at:?}");
    }

    #[test]
    fn an_upper_floor_counts_when_open_or_the_heros_own() {
        let second = Some((0, 0, 2));
        let up = iso::height(112.0 * iso::PIXELS_PER_MODEL_HEIGHT);
        let ray = sight(31.5, 31.5, up);
        let buildings = Buildings::default();
        // A closed building, the hero outside: not its floor 2; the ground before the house.
        let closed = world(&[], false);
        let (at, level) = picked_cell(pick(ray, &closed, &buildings, (0.0, None))).unwrap();
        assert_eq!(level, None);
        assert!(at.0 < 30 && at.1 < 30, "{at:?}");
        // Open to the sky, or the hero on a floor 2: floor 2 of the cell under the cursor.
        let open = world(&[], true);
        assert_eq!(
            picked_cell(pick(ray, &open, &buildings, (0.0, None))),
            Some(((31, 31), second))
        );
        assert_eq!(
            picked_cell(pick(ray, &closed, &buildings, (up, second))),
            Some(((31, 31), second))
        );
    }
}
