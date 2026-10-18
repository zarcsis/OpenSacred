//! The hero's way around what blocks it (docs/re/movement.md). The client finds a way from the
//! hero's cell to the target's by a search of the fewest steps over the walkable cells of a
//! 64 × 64 window (`0x421B90`); the game does the same, and heads for the farthest cell of the
//! way it can reach in a straight line, so that the hero does not zigzag from cell centre to
//! cell centre.

use std::collections::VecDeque;

use bevy::math::{Vec2, Vec3, Vec3Swizzles};

use crate::iso;

/// The side of the window the search covers, in cells, around the hero's cell.
const WINDOW: i32 = 64;
/// The steps to a neighbour: the four sides, then the four corners.
const STEPS: [(i32, i32); 8] = [
    (1, 0),
    (0, 1),
    (-1, 0),
    (0, -1),
    (1, 1),
    (-1, 1),
    (-1, -1),
    (1, -1),
];

/// A global cell.
pub type Cell = (i32, i32);

/// The global cell of world `(x, z)`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the cell of a point in the world is a small whole number"
)]
pub fn cell_of(x: f32, z: f32) -> Cell {
    (
        (x / iso::CELL).floor() as i32,
        (z / iso::CELL).floor() as i32,
    )
}

/// The world point at the centre of `cell`, on the ground plane.
fn centre(cell: Cell) -> Vec2 {
    let point = iso::ground(cell.0 as f32 + 0.5, cell.1 as f32 + 0.5);
    Vec2::new(point.x, point.z)
}

/// The cells from `from` to `to`, both included, of the fewest steps to one of the eight
/// neighbours over walkable cells within the window around `from`; `None` when `to` lies outside
/// the window or cannot be reached. `from` itself need not be walkable.
pub fn path(from: Cell, to: Cell, walkable: impl Fn(Cell) -> bool) -> Option<Vec<Cell>> {
    let origin = (from.0 - WINDOW / 2, from.1 - WINDOW / 2);
    let index = |cell: Cell| {
        let (x, y) = (cell.0 - origin.0, cell.1 - origin.1);
        ((0..WINDOW).contains(&x) && (0..WINDOW).contains(&y))
            .then(|| usize::try_from(y * WINDOW + x).ok())
            .flatten()
    };
    index(to)?;
    let side = usize::try_from(WINDOW).ok()?;
    // Where each reached cell was reached from.
    let mut came: Vec<Option<Cell>> = vec![None; side * side];
    let start = index(from)?;
    came[start] = Some(from);
    let mut queue = VecDeque::from([from]);
    while let Some(cell) = queue.pop_front() {
        if cell == to {
            let mut way = vec![to];
            let mut at = to;
            while at != from {
                at = came[index(at)?]?;
                way.push(at);
            }
            way.reverse();
            return Some(way);
        }
        for (dx, dy) in STEPS {
            let next = (cell.0 + dx, cell.1 + dy);
            let Some(i) = index(next) else {
                continue;
            };
            // Round a corner, not between two cells that touch at it.
            let corner = dx != 0 && dy != 0;
            if corner && !(walkable((cell.0 + dx, cell.1)) && walkable((cell.0, cell.1 + dy))) {
                continue;
            }
            if came[i].is_none() && walkable(next) {
                came[i] = Some(cell);
                queue.push_back(next);
            }
        }
    }
    None
}

/// Whether the straight line from `a` to `b` on the ground crosses walkable cells only, but
/// for the cell of `a`.
///
/// Every cell the line passes through counts, cell by cell along it, and where it passes a
/// corner, the two cells beside the corner too: a hero cannot slip between two cells that touch
/// at a corner.
pub fn clear(a: Vec2, b: Vec2, walkable: impl Fn(Cell) -> bool) -> bool {
    let (from, to) = (a / iso::CELL, b / iso::CELL);
    let (mut cell, end) = (cell_of(a.x, a.y), cell_of(b.x, b.y));
    let d = to - from;
    let step = (sign(d.x), sign(d.y));
    // The line's parameter at its next crossing of a column's and of a row's edge, and between
    // two crossings.
    let next = |p: f32, d: f32| {
        if d > 0.0 {
            (p.floor() + 1.0 - p) / d
        } else if d < 0.0 {
            (p - p.floor()) / -d
        } else {
            f32::INFINITY
        }
    };
    let (mut tx, mut ty) = (next(from.x, d.x), next(from.y, d.y));
    let (dx, dy) = (1.0 / d.x.abs(), 1.0 / d.y.abs());
    let cells = (end.0 - cell.0).abs() + (end.1 - cell.1).abs();
    for _ in 0..cells {
        if (tx - ty).abs() < 1e-6 {
            // Through a corner.
            if !walkable((cell.0 + step.0, cell.1)) || !walkable((cell.0, cell.1 + step.1)) {
                return false;
            }
            cell = (cell.0 + step.0, cell.1 + step.1);
            tx += dx;
            ty += dy;
        } else if tx < ty {
            cell.0 += step.0;
            tx += dx;
        } else {
            cell.1 += step.1;
            ty += dy;
        }
        if !walkable(cell) {
            return false;
        }
        if cell == end {
            break;
        }
    }
    true
}

/// -1, 0 or 1 by the sign of `v`.
fn sign(v: f32) -> i32 {
    if v > 0.0 {
        1
    } else if v < 0.0 {
        -1
    } else {
        0
    }
}

/// Where the hero at `position` heads for on its way to `target`: the target itself when the
/// straight line to it is clear, else the farthest cell of the way to it that the hero reaches
/// in a straight line; `None` when the target cannot be reached.
pub fn next(position: Vec3, target: Vec3, walkable: impl Fn(Cell) -> bool) -> Option<Vec3> {
    let (from, to) = (position.xz(), target.xz());
    if clear(from, to, &walkable) {
        return Some(target);
    }
    let way = path(cell_of(from.x, from.y), cell_of(to.x, to.y), &walkable)?;
    let corner = way
        .iter()
        .skip(1)
        .rev()
        .map(|&cell| centre(cell))
        .find(|&point| clear(from, point, &walkable))
        .or_else(|| way.get(1).map(|&cell| centre(cell)))?;
    Some(Vec3::new(corner.x, position.y, corner.y))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wall along x = 5 from y = 0 to y = 8, the rest walkable.
    fn wall(cell: Cell) -> bool {
        !(cell.0 == 5 && (0..=8).contains(&cell.1))
    }

    fn at(x: f32, y: f32) -> Vec3 {
        iso::ground(x, y)
    }

    #[test]
    fn the_way_goes_round_a_wall() {
        let way = path((2, 4), (8, 4), wall).unwrap();
        assert_eq!(way.first(), Some(&(2, 4)));
        assert_eq!(way.last(), Some(&(8, 4)));
        assert!(way.iter().all(|&cell| wall(cell)));
        // Round the wall's end, steps of one cell.
        assert!(way.contains(&(5, 9)));
        assert!(
            way.windows(2)
                .all(|w| (w[0].0 - w[1].0).abs() <= 1 && (w[0].1 - w[1].1).abs() <= 1)
        );
        // Out of the window, or walled in.
        assert_eq!(path((0, 0), (40, 0), wall), None);
        assert_eq!(path((2, 4), (8, 4), |cell| cell == (2, 4)), None);
    }

    #[test]
    fn no_way_slips_between_two_cells_that_touch_at_a_corner() {
        let walkable = |cell: Cell| cell != (5, 5) && cell != (6, 6);
        assert!(!clear(centre((5, 6)), centre((6, 5)), walkable));
        let way = path((5, 6), (6, 5), walkable).unwrap();
        assert!(way.len() > 3, "{way:?}");
        // Along a row, and past a single corner.
        assert!(clear(centre((2, 2)), centre((9, 2)), walkable));
        assert!(clear(centre((4, 4)), centre((5, 3)), walkable));
    }

    #[test]
    fn the_hero_heads_straight_for_what_it_sees_and_round_what_it_does_not() {
        let open = |_: Cell| true;
        assert_eq!(next(at(2.5, 4.5), at(8.5, 4.5), open), Some(at(8.5, 4.5)));
        assert!(!clear(at(2.5, 4.5).xz(), at(8.5, 4.5).xz(), wall));
        // Behind the wall: first to the farthest cell of the way it sees, by the wall's end.
        let corner = next(at(2.5, 4.5), at(8.5, 4.5), wall).unwrap();
        let cell = cell_of(corner.x, corner.z);
        assert!(cell.1 >= 8 && cell.0 <= 5, "{cell:?}");
        assert!(clear(at(2.5, 4.5).xz(), corner.xz(), wall));
    }
}
