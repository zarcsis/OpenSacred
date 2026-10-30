//! The hero's way around what blocks it (docs/re/movement.md). The client finds a way from the
//! hero's cell to the target's by a search of the fewest steps over the walkable cells of a
//! 64 × 64 window (`0x421B90`), on the hero's layer and, through the doors, the others; the game
//! does the same (`opensacred_rules::ways`), once at each order ([`Course`]), and heads for the
//! farthest cell of the way on its layer that it reaches in a straight line over the way's own
//! cells, so that the hero does not zigzag from cell centre to cell centre.

use std::collections::HashSet;
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
pub fn centre(cell: Cell) -> Vec2 {
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
    ways::clear(in_cells(a), in_cells(b), walkable)
}

/// Whether a straight way leads from the cell of world `a` to the cell of `b` by the original's
/// test (`0x4FCC80`, `opensacred_rules::ways::straight`): the cells of the line by whole cells,
/// `b`'s let through, with a way round one closed cell.
pub fn straight(a: Vec2, b: Vec2, walkable: impl Fn(Cell) -> bool) -> bool {
    ways::straight(cell_of(a.x, a.y), cell_of(b.x, b.y), walkable)
}

/// Whether the straight line from world `a` to `b` crosses only cells that `crossed` takes and
/// passes a corner only between cells that `beside` takes (`opensacred_rules::ways::clear_by`).
fn clear_by(
    a: Vec2,
    b: Vec2,
    crossed: impl Fn(Cell) -> bool,
    beside: impl Fn(Cell) -> bool,
) -> bool {
    ways::clear_by(in_cells(a), in_cells(b), crossed, beside)
}

/// A world point on the ground in cells.
fn in_cells(p: Vec2) -> (f32, f32) {
    let p = p / iso::CELL;
    (p.x, p.y)
}

/// What the hero does next on its way to its target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Step<L> {
    /// Heads for the second point on its layer, along the line to it from the first ([`advance`]).
    Toward(Vec2, Vec3),
    /// Goes onto another layer, through the door it stands on.
    Through(L),
    /// Stops: the next step of its way, from the centre of its cell, is no longer clear (a
    /// creature has come beside the corner it would pass), as the original's walker stops at
    /// the test of its line (`0x4F7DD0`).
    Blocked,
}

/// Of the places `places` that a way's search may step onto, those where `stops` does not stop
/// the walker, and the way's last cell `end` whoever stands in it: the hero that a creature
/// comes on at stands in the cell its way heads for, and the walk itself stops short of it
/// (#883).
pub fn passable<L: Copy>(
    mut places: Vec<(Cell, L)>,
    end: Cell,
    stops: impl Fn(Cell, L) -> bool,
) -> Vec<(Cell, L)> {
    places.retain(|&(cell, layer)| cell == end || !stops(cell, layer));
    places
}

/// A way as the original keeps it from an order to go somewhere (`0x4F7710`, docs/re/movement.md,
/// "The way"): straight to the target when the line to it is clear as the order is given, else
/// the places that the search found then (`0x4FABE0`). Neither is searched again until the next
/// order, so that a creature that comes into the way afterwards stops the walker at its step
/// (`0x4F7E10`) rather than turning it round, and the walker keeps to its way's cells, as the
/// original's hero kept off a barrel's cell that its step's test let it onto (#981, #989).
#[derive(Debug, Clone, PartialEq)]
pub enum Course<L> {
    /// Straight to the target, from where the walker stood.
    Straight(Vec2),
    /// Along these places, the walker's own first, and what it heads for on them since it took it.
    Way(Vec<(Cell, L)>, Option<Aim>),
}

/// What a walker heads for on its way ([`follow`]): the place of index `to`, the places' count for
/// the target itself, which it took at the place of index `from`, standing at `at`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aim {
    to: usize,
    from: usize,
    at: Vec2,
}

/// The course of a walker at `position` on `layer` ordered to `target`, on level `onto` when
/// given: straight when the line to the target is clear on its layer and the target is on it,
/// else the way that the search finds; `None` when the target cannot be reached. A place's layer
/// is `level` of its `L`, which may carry more than the layer.
///
/// The original's line runs from cell to cell (`0x4FABE0`'s first loop, each cell and, where it
/// steps diagonally, the cells beside the corner through `0x4FAA60`), so that a walker standing
/// off its cell's centre does not slip between two closed cells that touch at a corner of its
/// line; the line between the two cells' centres has to be clear as well as the walker's own.
pub fn course<L: Copy + Eq + Hash, K: PartialEq>(
    position: Vec3,
    layer: L,
    target: Vec3,
    onto: Option<K>,
    walkable: impl Fn(Cell, L) -> bool,
    neighbours: impl Fn((Cell, L)) -> Vec<(Cell, L)>,
    level: impl Fn(L) -> K,
) -> Option<Course<L>> {
    let (from, to) = (position.xz(), target.xz());
    let arrives = |at: L| onto.as_ref().is_none_or(|onto| level(at) == *onto);
    let on = |cell| walkable(cell, layer);
    let centres = (centre(cell_of(from.x, from.y)), centre(cell_of(to.x, to.y)));
    if arrives(layer) && clear(from, to, on) && clear(centres.0, centres.1, on) {
        return Some(Course::Straight(from));
    }
    path_onto(
        (cell_of(from.x, from.y), layer),
        cell_of(to.x, to.y),
        arrives,
        neighbours,
    )
    .map(|way| Course::Way(way, None))
}

/// What a walker at `position` on `layer` does next on its `course` to `target`: straight on;
/// or along its way, through the door it stands on onto the way's next layer, else for the
/// farthest place of the way on its layer that it reaches in a straight line over the way's own
/// cells, past corners between cells it may step on (`walkable`), and for the target itself once
/// the line to it is clear; with none, for the centre of its own cell, from which the next is a
/// step of the way, or, there, it stops ([`Step::Blocked`]). `None` when it stands off its way,
/// on no place of it on its layer.
///
/// What it heads for it keeps while the line to it from where it took it stays clear, for a
/// creature may step in, and takes a farther place once the line to that clears from where it
/// stands. Tested from where it stands at every frame, the line to what it headed for moved with
/// the rounding of its position, and a line that cleared at a corner's tolerance as it took it
/// failed a few frames on: it turned back to its cell's centre, and from there took it again, or
/// it headed for the target, whose line it had not tested, back and forth every frame. It walks
/// along the line it took ([`advance`]); one put off it (a leap) takes its next anew, or, going
/// straight, has its course found anew (`None`).
pub fn follow<L: Copy, K: PartialEq>(
    course: &mut Course<L>,
    position: Vec3,
    layer: L,
    target: Vec3,
    walkable: impl Fn(Cell, L) -> bool,
    level: impl Fn(L) -> K,
) -> Option<Step<L>> {
    let from = position.xz();
    let (way, aim) = match course {
        &mut Course::Straight(start) => {
            return (off(start, target.xz(), from) <= ON_LINE)
                .then_some(Step::Toward(start, target));
        }
        Course::Way(way, aim) => (way, aim),
    };
    let here = cell_of(from.x, from.y);
    let at = way
        .iter()
        .rposition(|&(cell, place)| cell == here && level(place) == level(layer))?;
    // The way on this layer, up to its first door.
    let door = way[at..]
        .windows(2)
        .position(|w| level(w[0].1) != level(w[1].1));
    if door == Some(0) {
        return way.get(at + 1).map(|&(_, next)| Step::Through(next));
    }
    let end = door.map_or(way.len(), |door| at + door + 1);
    // What it may head for, by its index on the way: its places, and past the last the target
    // itself where the stretch ends the way.
    let last = if door.is_none() { way.len() } else { end - 1 };
    let point = |i: usize| match way.get(i) {
        Some(&(cell, _)) => {
            let point = centre(cell);
            Vec3::new(point.x, position.y, point.y)
        }
        None => target,
    };
    let beside = |cell| walkable(cell, layer);
    // The way's cells on this layer from the place of index `first`.
    let cells =
        |first: usize| -> HashSet<Cell> { way[first..end].iter().map(|&(cell, _)| cell).collect() };
    // Kept while ahead of it on this layer, the line from where it took it clear and the walker
    // on it.
    let kept = aim.filter(|aim| {
        if aim.from > at || !(at + 1..=last).contains(&aim.to) {
            return false;
        }
        let (along, to) = (cells(aim.from), point(aim.to).xz());
        off(aim.at, to, from) <= ON_LINE && clear_by(aim.at, to, |c| along.contains(&c), beside)
    });
    // At its cell's centre, from the centre itself, which the rounding of where it stands does
    // not move: the step of the way to the next place, along a diagonal, passes the corner there.
    let on_centre = (centre(here) - from).length() <= ON_CENTRE;
    let start = if on_centre { centre(here) } else { from };
    let along = cells(at);
    let farther = (kept.map_or(at + 1, |aim| aim.to + 1)..=last)
        .rev()
        .find(|&to| clear_by(start, point(to).xz(), |c| along.contains(&c), beside))
        .map(|to| Aim {
            to,
            from: at,
            at: start,
        });
    *aim = farther.or(kept);
    Some(match *aim {
        Some(aim) => Step::Toward(aim.at, point(aim.to)),
        // Off its cell's centre, to the centre first, from which the next place is a step; there,
        // that step no longer clear, it stops.
        None if !on_centre => Step::Toward(from, point(at)),
        None => Step::Blocked,
    })
}

/// How far, in world units, `p` stands off the line through `from` and `to`.
fn off(from: Vec2, to: Vec2, p: Vec2) -> f32 {
    (to - from)
        .try_normalize()
        .map_or((p - from).length(), |along| along.perp_dot(p - from).abs())
}

/// How far, in world units, a walker may stand off the line of its step before it takes another:
/// one that something put elsewhere (a leap), not one that its walk's rounding moved.
const ON_LINE: f32 = 1.0;

/// Where a walker at `here` stands `length` further along its line from `from` to `to`, at its
/// own height, at `to` once it gets there: on the line itself, however the rounding of its
/// position moved it off it. At the world's coordinates of some 300,000 units a position rounds
/// to 0.03 along each axis; a walker that stepped from where it stood toward where it heads, along
/// a line nearly along an axis, gained a rounding of the same sign at every frame, and wandered
/// tenths of a unit off its line, into the closed cell beside a corner that the line passed clear.
pub fn advance(from: Vec2, to: Vec3, here: Vec3, length: f32) -> Vec3 {
    let line = to.xz() - from;
    let left = line.length();
    let point = match line.try_normalize() {
        Some(along) => {
            let on = (here.xz() - from).dot(along) + length;
            if on >= left {
                to.xz()
            } else {
                from + along * on
            }
        }
        None => to.xz(),
    };
    Vec3::new(point.x, here.y, point.y)
}

/// How near its cell's centre, in world units, a walker stands at it: where a step toward the
/// centre lands, rounded at the world's coordinates of some 300,000 units to 0.03 along each
/// axis. Half a unit stopped a walker that a step brought that near the centre short of it, the
/// line from where it stood to the next place along a diagonal passing beside the corner.
const ON_CENTRE: f32 = 0.1;

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

    /// What a walker does next on an order given where it stands: its course, then its step.
    fn next<L: Copy + Eq + Hash, K: PartialEq>(
        position: Vec3,
        layer: L,
        target: Vec3,
        onto: Option<K>,
        walkable: impl Fn(Cell, L) -> bool,
        neighbours: impl Fn((Cell, L)) -> Vec<(Cell, L)>,
        level: impl Fn(L) -> K,
    ) -> Option<Step<L>> {
        let mut course = course(position, layer, target, onto, &walkable, neighbours, &level)?;
        follow(&mut course, position, layer, target, walkable, level)
    }

    /// The cells a walker ordered from `from` to `target` on one layer stands on as it follows
    /// its course from point to point, and the legs it walks, the course kept from the order.
    fn walked(
        from: Vec3,
        target: Vec3,
        walkable: impl Fn(Cell) -> bool + Copy,
    ) -> (Vec<Cell>, Vec<(Vec3, Vec3)>) {
        let neighbours =
            |(cell, layer): (Cell, ())| around(cell, walkable).map(|c| (c, layer)).collect();
        let mut course = course(
            from,
            (),
            target,
            None::<()>,
            |c, ()| walkable(c),
            neighbours,
            |l| l,
        )
        .expect("a course");
        let (mut here, mut cells, mut legs) = (from, vec![cell_of(from.x, from.z)], Vec::new());
        for _ in 0..30 {
            let Some(Step::Toward(_, point)) =
                follow(&mut course, here, (), target, |c, ()| walkable(c), |l| l)
            else {
                panic!("off the course at {:?}", cell_of(here.x, here.z));
            };
            legs.push((here, point));
            here = point;
            cells.push(cell_of(here.x, here.z));
            if here == target {
                break;
            }
        }
        (cells, legs)
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
            Some(Step::Toward(at(2.5, 4.5).xz(), at(8.5, 4.5)))
        );
        assert!(!clear(at(2.5, 4.5).xz(), at(8.5, 4.5).xz(), wall));
        // Behind the wall: to the farthest cell of the way it reaches straight over the way's own
        // cells, toward the wall's end; and on, a stretch at a time, round it to the target.
        let Some(Step::Toward(_, corner)) = next(
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
        assert!(clear(at(2.5, 4.5).xz(), corner.xz(), wall));
        let (cells, legs) = walked(at(2.5, 4.5), at(8.5, 4.5), wall);
        assert_eq!(cells.last(), Some(&(8, 4)), "{cells:?}");
        assert!(cells.iter().any(|&(x, y)| x <= 5 && y >= 8), "{cells:?}");
        assert!(legs.iter().all(|&(a, b)| clear(a.xz(), b.xz(), wall)));
    }

    #[test]
    fn off_the_straight_line_the_hero_keeps_to_its_ways_cells() {
        // #981's barrel at (5938, 404), which the hero may step onto, between the closed cells
        // (5938, 403) to its north and (5937, 404) to its west, and (5940, 404) closed too: the
        // straight line from (5940, 402) to (5936, 406) passes their corners, and the way goes
        // round the barrel's cell, which the hero then does not cut through either.
        let closed = [(5938, 403), (5937, 403), (5937, 404), (5940, 404)];
        let open = |cell: Cell| !closed.contains(&cell);
        let (cells, legs) = walked(at(5940.5, 402.5), at(5936.5, 406.5), open);
        assert_eq!(cells.last(), Some(&(5936, 406)), "{cells:?}");
        // Every cell its legs cross, the barrel's never among them, though the straight line
        // from a cell of the way on to the target crosses it.
        for (a, b) in legs {
            let (a, b) = (a.xz(), b.xz());
            for k in 1..=100_u8 {
                let p = a + (b - a) * (f32::from(k) / 100.0);
                assert_ne!(cell_of(p.x, p.y), (5938, 404), "{cells:?}");
            }
        }
        assert!(clear(at(5939.5, 404.5).xz(), at(5936.5, 406.5).xz(), open));
        // Back from off its cell's centre, where its own line passes beside the corners: the line
        // from cell to cell does not, and it keeps to its way again.
        let (back, off) = (at(5940.5, 402.5), at(5936.45, 406.6));
        assert!(clear(off.xz(), back.xz(), open));
        let (cells, legs) = walked(off, back, open);
        assert_eq!(cells.last(), Some(&(5940, 402)), "{cells:?}");
        for (a, b) in legs {
            let (a, b) = (a.xz(), b.xz());
            for k in 1..=100_u8 {
                let p = a + (b - a) * (f32::from(k) / 100.0);
                assert_ne!(cell_of(p.x, p.y), (5938, 404), "{cells:?}");
            }
        }
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
        let Some(Step::Toward(_, point)) = next(
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
        // A creature coming on at the hero inside, who stands in the cell its way heads for: the
        // way still ends there, toward the door (#883); without it the search finds none.
        let hero = (5, 4);
        let stopped = |(cell, inside): (Cell, bool)| {
            neighbours((cell, inside))
                .into_iter()
                .filter(|&(c, _)| c != hero)
                .collect::<Vec<_>>()
        };
        let comes_on = |place| passable(neighbours(place), hero, |c, _| c == hero);
        let from = at(5.5, 9.5);
        let to = at(5.5, 4.5);
        assert_eq!(next(from, false, to, None, walkable, stopped, |l| l), None);
        let Some(Step::Toward(_, point)) = next(from, false, to, None, walkable, comes_on, |l| l)
        else {
            panic!("no way to the hero");
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

    /// How a walker on `course` to `target` on one layer walks frame by frame from `from`, as
    /// `hero::run` walks, `length` a frame, a course found anew off its way, until it arrives,
    /// stops or 2,000 frames pass.
    fn frames(
        mut course: Course<()>,
        from: Vec3,
        target: Vec3,
        walkable: impl Fn(Cell) -> bool + Copy,
        length: f32,
    ) -> Walked {
        let neighbours =
            |(cell, layer): (Cell, ())| around(cell, walkable).map(|c| (c, layer)).collect();
        let mut here = from;
        let mut walked = Walked::default();
        let mut last: Option<(Cell, Vec3)> = None;
        for _ in 0..2000 {
            let mut step = follow(&mut course, here, (), target, |c, ()| walkable(c), |l| l);
            if step.is_none() {
                course = super::course(
                    here,
                    (),
                    target,
                    None::<()>,
                    |c, ()| walkable(c),
                    neighbours,
                    |l| l,
                )
                .expect("a course");
                step = follow(&mut course, here, (), target, |c, ()| walkable(c), |l| l);
            }
            let Some(Step::Toward(line, toward)) = step else {
                walked.blocked = true;
                return walked;
            };
            if (target - here).xz().length() <= crate::hero::ARRIVED {
                walked.arrived = true;
                return walked;
            }
            let cell = cell_of(here.x, here.z);
            let own = |point: Vec3| (point.xz() - centre(cell)).length() < 1e-3;
            if last.is_some_and(|(was, before)| was == cell && !own(before)) && own(toward) {
                walked.turned += 1;
            }
            last = Some((cell, toward));
            let next = advance(line, toward, here, length);
            let into = cell_of(next.x, next.z);
            if into != cell && !walkable(into) {
                walked.walled = true;
                return walked;
            }
            here = next;
        }
        walked
    }

    /// How a walk of [`frames`] ended, and how often the walker turned back to its cell's centre
    /// from heading elsewhere.
    #[derive(Debug, Default)]
    struct Walked {
        arrived: bool,
        blocked: bool,
        walled: bool,
        turned: usize,
    }

    #[test]
    fn a_way_that_ends_in_a_diagonal_step_is_walked_to_the_target() {
        // From the centre of (0, 0) the last step of its way, the diagonal onto (1, 1), clears
        // through the corner; the line to the target, off (1, 1)'s centre toward (0, 1), does not.
        // Heading for the target from there, it had turned back to (0, 0)'s centre at the next
        // frame, and so round at every frame (#996).
        let open = |_: Cell| true;
        let course = Course::Way(vec![((0, 0), ()), ((1, 1), ())], None);
        let (from, target) = (at(0.5, 0.5), at(1.58, 1.77));
        let Some(Step::Toward(_, first)) = follow(
            &mut course.clone(),
            from,
            (),
            target,
            |c, ()| open(c),
            |l| l,
        ) else {
            panic!("no step");
        };
        assert_eq!(cell_of(first.x, first.z), (1, 1));
        assert!((first.xz() - centre((1, 1))).length() < 1e-3, "{first}");
        let walked = frames(course, from, target, open, 3.0);
        assert!(walked.arrived && walked.turned == 0, "{walked:?}");
    }

    #[test]
    fn walkers_at_the_worlds_coordinates_do_not_turn_round_on_their_ways() {
        // Random ways round random closed cells far out in the world, where a position rounds
        // to 0.03 of a unit, at a slow walk and a run: every walker that has a way gets to its
        // target, never turning back to its cell's centre and never stopping with nothing in its
        // way (#996).
        let mut seed = 0x2545_F491_4F6C_DD1D_u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let origin = 5600;
        let (mut walks, mut arrived) = (0, 0);
        for _ in 0..40 {
            let closed: HashSet<Cell> = (origin..origin + 16)
                .flat_map(|x| (origin..origin + 16).map(move |y| (x, y)))
                .filter(|_| next() % 100 < 30)
                .collect();
            let range = origin..origin + 16;
            let open = |cell: Cell| {
                range.contains(&cell.0) && range.contains(&cell.1) && !closed.contains(&cell)
            };
            let neighbours =
                |(cell, layer): (Cell, ())| around(cell, open).map(|c| (c, layer)).collect();
            for _ in 0..20 {
                let point = |r: u64| {
                    let (x, y) = (r % 1600, r / 1600 % 1600);
                    let o = origin as f32;
                    at(
                        o + f32::from(u16::try_from(x).unwrap()) / 100.0 + 0.003,
                        o + f32::from(u16::try_from(y).unwrap()) / 100.0 + 0.007,
                    )
                };
                let (from, target) = (point(next()), point(next()));
                let length = if next() % 2 == 0 { 1.5 } else { 5.5 };
                let Some(course) = course(
                    from,
                    (),
                    target,
                    None::<()>,
                    |c, ()| open(c),
                    neighbours,
                    |l| l,
                ) else {
                    continue;
                };
                if !open(cell_of(from.x, from.z)) {
                    continue;
                }
                walks += 1;
                let walked = frames(course, from, target, open, length);
                assert!(!walked.blocked && walked.turned == 0, "{walked:?}");
                arrived += usize::from(walked.arrived);
            }
        }
        assert_eq!(arrived, walks);
    }

    #[test]
    fn a_walker_keeps_to_the_line_of_its_step() {
        let (from, to) = (at(5600.5, 5600.5), at(5603.5, 5601.5));
        // Off the line by a rounding, it comes back onto it, a step further along it.
        let off = from + Vec3::new(0.0, 0.0, 0.03);
        let next = advance(from.xz(), to, off, 2.0);
        let along = (to - from).normalize();
        assert!(
            ((next - from).xz().perp_dot(along.xz())).abs() < 0.04,
            "{next}"
        );
        assert!(((next - from).dot(along) - 2.0).abs() < 0.05, "{next}");
        // At its end, there.
        assert_eq!(advance(from.xz(), to, to - along, 2.0), to);
        assert_eq!(advance(to.xz(), to, to, 2.0), to);
    }
}
