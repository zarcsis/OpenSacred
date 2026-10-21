//! The hero's way around what blocks it (docs/re/movement.md). The client finds a way from the
//! hero's cell to the target's by a search of the fewest steps over the walkable cells of a
//! 64 × 64 window (`0x421B90`), on the hero's layer and, through the doors, the others; the game
//! does the same (`opensacred_rules::ways`), and heads for the farthest cell of the way on its
//! layer that it reaches in a straight line, so that the hero does not zigzag from cell centre to
//! cell centre.

use std::hash::Hash;

use bevy::math::{Vec2, Vec3, Vec3Swizzles};
use opensacred_rules::ways;
#[cfg(test)]
pub use opensacred_rules::ways::path;
pub use opensacred_rules::ways::{around, path_onto};

use crate::iso;

/// A global cell, the rules' (`opensacred_rules::Cell`).
pub use opensacred_rules::Cell;

/// The global cell of world `(x, z)`.
pub fn cell_of(x: f32, z: f32) -> Cell {
    ways::cell_at((x / iso::CELL, z / iso::CELL))
}

/// The world point at the centre of `cell`, on the ground plane.
fn centre(cell: Cell) -> Vec2 {
    let point = iso::ground(cell.0 as f32 + 0.5, cell.1 as f32 + 0.5);
    Vec2::new(point.x, point.z)
}

/// Where a creature at `position` heads its way's search for `target` (`0x4FABE0`): the target
/// itself within 25 cells along each axis, else the centre of the cell that
/// `opensacred_rules::ways::nearer` brings it to along the line, at `position`'s height, so that
/// a far target is reached a leg at a time; the target itself when no cell of the line within
/// 25 may be stood on (`walkable`).
pub fn leg(position: Vec3, target: Vec3, walkable: impl Fn(Cell) -> bool) -> Vec3 {
    let (from, to) = (cell_of(position.x, position.z), cell_of(target.x, target.z));
    match ways::nearer(from, to, walkable) {
        Some(cell) if cell != to => {
            let point = centre(cell);
            Vec3::new(point.x, position.y, point.y)
        }
        _ => target,
    }
}

/// Whether the straight line from world `a` to `b` on the ground crosses walkable cells only,
/// but for the cell of `a` (`opensacred_rules::ways::clear`).
pub fn clear(a: Vec2, b: Vec2, walkable: impl Fn(Cell) -> bool) -> bool {
    let in_cells = |p: Vec2| {
        let p = p / iso::CELL;
        (p.x, p.y)
    };
    ways::clear(in_cells(a), in_cells(b), walkable)
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
    fn a_far_target_is_headed_for_a_leg_at_a_time() {
        let open = |_: Cell| true;
        // Within 25 cells, the target itself.
        let near = at(20.5, 3.5);
        assert_eq!(leg(at(0.5, 0.5), near, open), near);
        // 40 cells along x: the cell 25 along the line, at its centre.
        let leg_point = leg(at(0.5, 0.5), at(40.5, 0.5), open);
        assert!((leg_point - at(25.5, 0.5)).length() < 1e-3, "{leg_point}");
        // The line's far cells blocked: the farthest it may stand on.
        let short = leg(at(0.5, 0.5), at(40.5, 0.5), |cell: Cell| cell.0 < 20);
        assert!((short - at(19.5, 0.5)).length() < 1e-3, "{short}");
        // No cell of the line within 25 to stand on: the target itself.
        let far = at(40.5, 0.5);
        assert_eq!(leg(at(0.5, 0.5), far, |cell: Cell| cell.0 == 0), far);
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
    fn the_hero_goes_through_a_door_onto_the_inside_layer() {
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
