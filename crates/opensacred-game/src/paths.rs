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
/// How finely a straight line is checked, in cells.
const PROBE: f32 = 0.125;

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
pub fn clear(a: Vec2, b: Vec2, walkable: impl Fn(Cell) -> bool) -> bool {
    let start = cell_of(a.x, a.y);
    let length = a.distance(b) / iso::CELL;
    let steps = (length / PROBE).ceil().max(1.0);
    let mut i = 1.0;
    while i <= steps {
        let p = a.lerp(b, i / steps);
        let cell = cell_of(p.x, p.y);
        if cell != start && !walkable(cell) {
            return false;
        }
        i += 1.0;
    }
    true
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
