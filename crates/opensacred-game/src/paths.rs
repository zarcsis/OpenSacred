//! The hero's way around what blocks it (docs/re/movement.md). The client finds a way from the
//! hero's cell to the target's by a search of the fewest steps over the walkable cells of a
//! 64 × 64 window (`0x421B90`), on the hero's layer and, through the doors, the others; the game
//! does the same, and heads for the farthest cell of the way on its layer that it reaches in a
//! straight line, so that the hero does not zigzag from cell centre to cell centre.

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

use bevy::math::{Vec2, Vec3, Vec3Swizzles};

use crate::iso;

/// The side of the window the search covers, in cells, around the hero's cell.
const WINDOW: i32 = 64;
/// The steps to a neighbour: the four sides, then the four corners.
pub const STEPS: [(i32, i32); 8] = [
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

/// The places from `from` to a place in cell `to`, both included, of the fewest steps over the
/// places `neighbours` gives (a cell on a layer: the walkable neighbours on its layer, and the
/// same cell on another layer through a door), within the window around `from`'s cell; `None`
/// when `to` lies outside the window or cannot be reached.
#[cfg(test)]
pub fn path<L: Copy + Eq + Hash>(
    from: (Cell, L),
    to: Cell,
    neighbours: impl Fn((Cell, L)) -> Vec<(Cell, L)>,
) -> Option<Vec<(Cell, L)>> {
    path_onto(from, to, |_| true, neighbours)
}

/// As [`path`], to a place in cell `to` on a layer that `arrives` takes: the level that a click
/// chose (docs/re/movement.md, "The cursor's point").
pub fn path_onto<L: Copy + Eq + Hash>(
    from: (Cell, L),
    to: Cell,
    arrives: impl Fn(L) -> bool,
    neighbours: impl Fn((Cell, L)) -> Vec<(Cell, L)>,
) -> Option<Vec<(Cell, L)>> {
    let inside = |cell: Cell| {
        let (x, y) = (
            cell.0 - from.0.0 + WINDOW / 2,
            cell.1 - from.0.1 + WINDOW / 2,
        );
        (0..WINDOW).contains(&x) && (0..WINDOW).contains(&y)
    };
    if !inside(to) {
        return None;
    }
    // Where each reached place was reached from.
    let mut came = HashMap::from([(from, from)]);
    let mut queue = VecDeque::from([from]);
    while let Some(place) = queue.pop_front() {
        if place.0 == to && arrives(place.1) {
            let mut way = vec![place];
            let mut at = place;
            while at != from {
                at = *came.get(&at)?;
                way.push(at);
            }
            way.reverse();
            return Some(way);
        }
        for next in neighbours(place) {
            if inside(next.0) && !came.contains_key(&next) {
                came.insert(next, place);
                queue.push_back(next);
            }
        }
    }
    None
}

/// The walkable neighbours of `cell` on one layer, by `walkable`: round a corner, not between two
/// cells that touch at it.
pub fn around(cell: Cell, walkable: impl Fn(Cell) -> bool) -> impl Iterator<Item = Cell> {
    STEPS.iter().filter_map(move |&(dx, dy)| {
        let next = (cell.0 + dx, cell.1 + dy);
        let corner = dx != 0 && dy != 0;
        let between =
            corner && !(walkable((cell.0 + dx, cell.1)) && walkable((cell.0, cell.1 + dy)));
        (!between && walkable(next)).then_some(next)
    })
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

/// What the hero does next on its way to its target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Step<L> {
    /// Heads for this point on its layer.
    Toward(Vec3),
    /// Goes onto another layer, through the door it stands on.
    Through(L),
}

/// What the hero at `position` on `layer` does next on its way to `target`, on level `onto` when
/// given: heads for the target itself when the straight line to it is clear on its layer and
/// the target is on it, else for the farthest place of the way on its layer that it reaches in a
/// straight line, or goes through the door it stands on onto the way's next layer; `None` when
/// the target cannot be reached. A place's layer is `level` of its `L`, which may carry more
/// than the layer, the same along a stretch of the way.
pub fn next<L: Copy + Eq + Hash, K: PartialEq>(
    position: Vec3,
    layer: L,
    target: Vec3,
    onto: Option<K>,
    walkable: impl Fn(Cell, L) -> bool,
    neighbours: impl Fn((Cell, L)) -> Vec<(Cell, L)>,
    level: impl Fn(L) -> K,
) -> Option<Step<L>> {
    let (from, to) = (position.xz(), target.xz());
    let on = |cell| walkable(cell, layer);
    let arrives = |at: L| onto.as_ref().is_none_or(|onto| level(at) == *onto);
    if arrives(layer) && clear(from, to, on) {
        return Some(Step::Toward(target));
    }
    let way = path_onto(
        (cell_of(from.x, from.y), layer),
        cell_of(to.x, to.y),
        arrives,
        neighbours,
    )?;
    // The way on this layer, up to its first door.
    let door = way.windows(2).position(|w| level(w[0].1) != level(w[1].1));
    if door == Some(0) {
        return way.get(1).map(|&(_, next)| Step::Through(next));
    }
    let stretch = &way[..door.map_or(way.len(), |at| at + 1)];
    let corner = stretch
        .iter()
        .skip(1)
        .rev()
        .map(|&(cell, _)| centre(cell))
        .find(|&point| clear(from, point, on))
        .or_else(|| stretch.get(1).map(|&(cell, _)| centre(cell)))?;
    Some(Step::Toward(Vec3::new(corner.x, position.y, corner.y)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wall along x = 5 from y = 0 to y = 8, the rest walkable.
    fn wall(cell: Cell) -> bool {
        !(cell.0 == 5 && (0..=8).contains(&cell.1))
    }

    /// One layer, the wall's.
    fn flat((cell, layer): (Cell, ())) -> Vec<(Cell, ())> {
        around(cell, wall).map(|next| (next, layer)).collect()
    }

    fn at(x: f32, y: f32) -> Vec3 {
        iso::ground(x, y)
    }

    #[test]
    fn the_way_goes_round_a_wall() {
        let way: Vec<Cell> = path(((2, 4), ()), (8, 4), flat)
            .unwrap()
            .into_iter()
            .map(|(cell, _)| cell)
            .collect();
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
        assert_eq!(path(((0, 0), ()), (40, 0), flat), None);
        assert_eq!(path(((2, 4), ()), (8, 4), |_| Vec::new()), None);
    }

    #[test]
    fn no_way_slips_between_two_cells_that_touch_at_a_corner() {
        let walkable = |cell: Cell| cell != (5, 5) && cell != (6, 6);
        assert!(!clear(centre((5, 6)), centre((6, 5)), walkable));
        let way = path(((5, 6), ()), (6, 5), |(cell, layer)| {
            around(cell, walkable)
                .map(|next| (next, layer))
                .collect::<Vec<_>>()
        })
        .unwrap();
        assert!(way.len() > 3, "{way:?}");
        // Along a row, and past a single corner.
        assert!(clear(centre((2, 2)), centre((9, 2)), walkable));
        assert!(clear(centre((4, 4)), centre((5, 3)), walkable));
    }

    #[test]
    fn the_hero_heads_straight_for_what_it_sees_and_round_what_it_does_not() {
        let open = |_: Cell, _: ()| true;
        let all = |(cell, layer): (Cell, ())| {
            around(cell, |_| true)
                .map(|c| (c, layer))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            next(at(2.5, 4.5), (), at(8.5, 4.5), None, open, all, |l| l),
            Some(Step::Toward(at(8.5, 4.5)))
        );
        assert!(!clear(at(2.5, 4.5).xz(), at(8.5, 4.5).xz(), wall));
        // Behind the wall: first to the farthest cell of the way it sees, by the wall's end.
        let Some(Step::Toward(corner)) = next(
            at(2.5, 4.5),
            (),
            at(8.5, 4.5),
            None,
            |cell, ()| wall(cell),
            flat,
            |l| l,
        ) else {
            panic!("no way round the wall");
        };
        let cell = cell_of(corner.x, corner.z);
        assert!(cell.1 >= 8 && cell.0 <= 5, "{cell:?}");
        assert!(clear(at(2.5, 4.5).xz(), corner.xz(), wall));
    }

    #[test]
    fn a_door_leads_onto_the_inside_layer() {
        // The ground: a house of cells x 4..=6, y 4..=6 with its door at (5, 6); the house's
        // layer: its inside, the same cells, the door again.
        let walkable = |(x, y): Cell, inside: bool| {
            let house = (4..=6).contains(&x) && (4..=6).contains(&y);
            (x, y) == (5, 6) || if inside { house } else { !house }
        };
        let neighbours = |(cell, inside): (Cell, bool)| {
            let mut out: Vec<(Cell, bool)> = around(cell, |c| walkable(c, inside))
                .map(|c| (c, inside))
                .collect();
            if cell == (5, 6) {
                out.push((cell, !inside));
            }
            out
        };
        let way = path(((5, 9), false), (5, 4), neighbours).unwrap();
        assert_eq!(way.last(), Some(&((5, 4), true)));
        let through = way.windows(2).find(|w| w[0].1 != w[1].1).unwrap();
        assert_eq!(through, [((5, 6), false), ((5, 6), true)]);
        // On the door: through it; before it, toward it.
        assert_eq!(
            next(
                at(5.5, 6.5),
                false,
                at(5.5, 4.5),
                None,
                walkable,
                neighbours,
                |l| l
            ),
            Some(Step::Through(true))
        );
        let Some(Step::Toward(point)) = next(
            at(5.5, 9.5),
            false,
            at(5.5, 4.5),
            None,
            walkable,
            neighbours,
            |l| l,
        ) else {
            panic!("no way to the door");
        };
        assert_eq!(cell_of(point.x, point.z), (5, 6));
        // The door's cell lies on both layers: the way to it ends on the level asked for, and on
        // the nearer when none is (#208).
        let to_door = |arrives: fn(bool) -> bool| {
            path_onto(((5, 9), false), (5, 6), arrives, neighbours)
                .and_then(|way| way.last().copied())
        };
        assert_eq!(to_door(|_| true), Some(((5, 6), false)));
        assert_eq!(to_door(|inside| inside), Some(((5, 6), true)));
        // Standing at the door outside, sent to it inside: through the door, not there already.
        assert_eq!(
            next(
                at(5.5, 6.5),
                false,
                at(5.5, 6.5),
                Some(true),
                walkable,
                neighbours,
                |l| l
            ),
            Some(Step::Through(true))
        );
    }
}
