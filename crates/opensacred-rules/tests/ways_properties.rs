//! Properties of the ways on random maps (#1429): the straight line's test against a line sampled
//! densely, a way's validity and its length as the cells open and close, and the cell that a far
//! target is brought nearer to. Each holds of any map, for the original's search and line as for
//! ours: no symmetry of the line's test, which leaves out its start's cell, and no tie between
//! ways of one length. The maps lie at the origin and far out in the world, where a point in `f32`
//! carries about 6·10⁻⁴ of a cell.

use std::collections::HashSet;

use opensacred_rules::Cell;
use opensacred_rules::ways::{Point, around, cell_at, clear, nearer, path_onto};
use proptest::prelude::*;

/// The side of a map, in cells.
const SIDE: i32 = 20;
/// Where the maps lie: at the origin, and about the world's cell 5,600, some 300,000 units out.
const ORIGINS: [i32; 2] = [0, 5_600];
/// How deep inside a cell a sampled point lies for the line to cross the cell for certain, and
/// how near the line a cell lies that the line's test may look at: both well above the test's
/// corner of 0.002 and the floats' rounding.
const INSIDE: f64 = 0.005;
const NEAR: f64 = 0.01;
/// The points sampled along a line.
const SAMPLES: u32 = 4_000;
/// How far along an axis a far target is brought nearer (`0x4FABE0`).
const NEARER_THAN: i32 = 25;

/// A map: its origin and its closed cells, row by row; the cells about it are open.
#[derive(Clone, Debug)]
struct Map {
    origin: i32,
    closed: Vec<bool>,
}

impl Map {
    /// The index of `cell` among the map's cells, where it lies on the map.
    fn index(&self, cell: Cell) -> Option<usize> {
        let (x, y) = (cell.0 - self.origin, cell.1 - self.origin);
        ((0..SIDE).contains(&x) && (0..SIDE).contains(&y))
            .then(|| usize::try_from(y * SIDE + x).ok())
            .flatten()
    }

    /// Whether `cell` is open: off the map, or not closed on it.
    fn walkable(&self, cell: Cell) -> bool {
        self.index(cell).is_none_or(|index| !self.closed[index])
    }

    /// The map with `cell` closed or opened.
    fn with(&self, cell: Cell, closed: bool) -> Self {
        let mut map = self.clone();
        if let Some(index) = self.index(cell) {
            map.closed[index] = closed;
        }
        map
    }

    /// Its cells.
    fn cells(&self) -> impl Iterator<Item = Cell> + '_ {
        (0..SIDE).flat_map(move |y| (0..SIDE).map(move |x| (self.origin + x, self.origin + y)))
    }
}

/// A map at one of the origins, of up to half its cells closed.
fn map() -> impl Strategy<Value = Map> {
    (prop::sample::select(ORIGINS.to_vec()), 0.0..0.5_f64).prop_flat_map(|(origin, density)| {
        let cells = usize::try_from(SIDE * SIDE).unwrap_or_else(|_| panic!("a map of no cells"));
        prop::collection::vec(prop::bool::weighted(density), cells)
            .prop_map(move |closed| Map { origin, closed })
    })
}

/// A point on a map at `origin`: anywhere, or a cell's centre the more often.
fn point(origin: i32) -> impl Strategy<Value = Point> {
    let (low, high) = (origin as f32, (origin + SIDE) as f32);
    let centre = (0..SIDE, 0..SIDE)
        .prop_map(move |(x, y)| ((origin + x) as f32 + 0.5, (origin + y) as f32 + 0.5));
    prop_oneof![2 => (low..high, low..high), 1 => centre]
}

/// A map and a line on it.
fn line() -> impl Strategy<Value = (Map, Point, Point)> {
    map().prop_flat_map(|map| {
        let origin = map.origin;
        (Just(map), point(origin), point(origin))
    })
}

/// The cells that the line from `a` to `b` crosses for certain: those whose inside, [`INSIDE`]
/// from their edges, a point of the line sampled densely falls in.
fn crossed(a: Point, b: Point) -> HashSet<Cell> {
    let (a, b) = (
        (f64::from(a.0), f64::from(a.1)),
        (f64::from(b.0), f64::from(b.1)),
    );
    (0..=SAMPLES)
        .filter_map(|i| {
            let t = f64::from(i) / f64::from(SAMPLES);
            let (x, y) = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
            let (fx, fy) = (x - x.floor(), y - y.floor());
            let inside = |f: f64| (INSIDE..1.0 - INSIDE).contains(&f);
            (inside(fx) && inside(fy)).then(|| (whole(x.floor()), whole(y.floor())))
        })
        .collect()
}

/// Whether the cell `cell` lies within `margin` of the line from `a` to `b`.
fn near(cell: Cell, a: Point, b: Point, margin: f64) -> bool {
    let (a, b) = (
        (f64::from(a.0), f64::from(a.1)),
        (f64::from(b.0), f64::from(b.1)),
    );
    let (x0, y0) = (f64::from(cell.0), f64::from(cell.1));
    let inside = |(x, y): (f64, f64)| {
        (x0 - margin..=x0 + 1.0 + margin).contains(&x)
            && (y0 - margin..=y0 + 1.0 + margin).contains(&y)
    };
    // The line enters the cell grown by the margin where a point of it, sampled more densely
    // than the margin, lies inside it.
    let steps = (((b.0 - a.0).abs() + (b.1 - a.1).abs()) / (margin / 2.0))
        .ceil()
        .max(1.0);
    let steps = whole(steps).unsigned_abs();
    (0..=steps).any(|i| {
        let t = f64::from(i) / f64::from(steps);
        inside((a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1)))
    })
}

/// A whole number of cells.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a cell's coordinate, whole and within the world"
)]
fn whole(value: f64) -> i32 {
    value as i32
}

/// The way's search over the walkable neighbours of one layer (`opensacred_rules::ways::path`).
fn way(map: &Map, from: Cell, to: Cell) -> Option<Vec<(Cell, ())>> {
    path_onto(
        (from, ()),
        to,
        |()| true,
        |(cell, ())| around(cell, |c| map.walkable(c)).map(|c| (c, ())).collect(),
    )
}

/// The steps from `a` to `b` on an open field.
fn chebyshev(a: Cell, b: Cell) -> usize {
    let steps = (a.0 - b.0).abs().max((a.1 - b.1).abs());
    usize::try_from(steps).unwrap_or_else(|_| panic!("{steps} steps"))
}

/// A map and two of its cells.
fn ends() -> impl Strategy<Value = (Map, Cell, Cell)> {
    map().prop_flat_map(|map| {
        let origin = map.origin;
        let cell = move || (0..SIDE, 0..SIDE).prop_map(move |(x, y)| (origin + x, origin + y));
        (Just(map), cell(), cell())
    })
}

proptest! {
    #[test]
    fn the_line_is_not_clear_through_a_closed_cell_it_crosses((map, a, b) in line()) {
        let start = cell_at(a);
        let closed = crossed(a, b)
            .into_iter()
            .find(|&cell| cell != start && !map.walkable(cell));
        if let Some(cell) = closed {
            prop_assert!(!clear(a, b, |c| map.walkable(c)), "{a:?} to {b:?} through {cell:?}");
        }
    }

    #[test]
    fn closing_a_cell_the_line_crosses_makes_it_unclear(
        ((map, a, b), pick) in (line(), any::<prop::sample::Index>()),
    ) {
        let start = cell_at(a);
        let mut crossed: Vec<Cell> = crossed(a, b).into_iter().filter(|&c| c != start).collect();
        crossed.sort_unstable();
        if !crossed.is_empty() {
            let cell = crossed[pick.index(crossed.len())];
            let map = map.with(cell, true);
            prop_assert!(!clear(a, b, |c| map.walkable(c)), "{a:?} to {b:?} through {cell:?}");
        }
    }

    #[test]
    fn the_line_is_clear_where_every_cell_near_it_is_open((map, a, b) in line()) {
        let start = cell_at(a);
        let opened = map
            .cells()
            .filter(|&cell| cell != start && near(cell, a, b, NEAR))
            .fold(map.clone(), |map, cell| map.with(cell, false));
        prop_assert!(clear(a, b, |c| opened.walkable(c)), "{a:?} to {b:?}");
    }

    #[test]
    fn a_way_steps_from_its_start_to_its_end_by_the_neighbours((map, from, to) in ends()) {
        let map = map.with(from, false);
        if let Some(way) = way(&map, from, to) {
            prop_assert_eq!(way.first().map(|p| p.0), Some(from));
            prop_assert_eq!(way.last().map(|p| p.0), Some(to));
            for step in way.windows(2) {
                let next: Vec<Cell> = around(step[0].0, |c| map.walkable(c)).collect();
                prop_assert!(next.contains(&step[1].0), "{:?} to {:?}", step[0].0, step[1].0);
            }
            for &(cell, ()) in &way {
                let off = (cell.0 - from.0, cell.1 - from.1);
                prop_assert!((-32..32).contains(&off.0) && (-32..32).contains(&off.1));
            }
        }
    }

    #[test]
    fn on_an_open_field_a_way_takes_the_fewest_steps(
        origin in prop::sample::select(ORIGINS.to_vec()),
        (dx, dy) in (-32..32_i32, -32..32_i32),
    ) {
        let (from, to) = ((origin, origin), (origin + dx, origin + dy));
        let way = path_onto((from, ()), to, |()| true, |(cell, ())| {
            around(cell, |_| true).map(|c| (c, ())).collect()
        });
        prop_assert_eq!(way.map(|way| way.len() - 1), Some(chebyshev(from, to)));
    }

    #[test]
    fn opening_cells_never_lengthens_a_way(
        ((map, from, to), opened) in (ends(), prop::collection::vec(any::<prop::sample::Index>(), 0..40)),
    ) {
        let map = map.with(from, false);
        let cells: Vec<Cell> = map.cells().collect();
        let more = opened
            .iter()
            .fold(map.clone(), |map, pick| map.with(cells[pick.index(cells.len())], false));
        if let Some(before) = way(&map, from, to) {
            let after = way(&more, from, to).map(|way| way.len());
            prop_assert!(after.is_some_and(|after| after <= before.len()), "{after:?} for {}", before.len());
        }
    }

    #[test]
    fn closing_a_cell_away_from_a_way_keeps_its_length(
        ((map, from, to), pick) in (ends(), any::<prop::sample::Index>()),
    ) {
        let map = map.with(from, false);
        if let Some(before) = way(&map, from, to) {
            // A cell neither on the way nor beside one of its diagonal steps, which it would bar.
            let on: HashSet<Cell> = before.iter().map(|&(cell, ())| cell).collect();
            let beside: HashSet<Cell> = before
                .windows(2)
                .flat_map(|step| {
                    let (a, b) = (step[0].0, step[1].0);
                    [(b.0, a.1), (a.0, b.1)]
                })
                .collect();
            let away: Vec<Cell> = map.cells().filter(|c| !on.contains(c) && !beside.contains(c)).collect();
            if !away.is_empty() {
                let cell = away[pick.index(away.len())];
                let after = way(&map.with(cell, true), from, to).map(|way| way.len());
                prop_assert_eq!(after, Some(before.len()), "{:?} closed", cell);
            }
        }
    }

    #[test]
    fn a_far_target_is_brought_nearer_onto_the_farthest_open_cell_of_its_line(
        (seed, density) in (any::<u64>(), 0..100_u64),
        from in (0..200_i32, 0..200_i32),
        (dx, dy) in (-90..90_i32, -90..90_i32),
    ) {
        let to = (from.0 + dx, from.1 + dy);
        // An open or closed cell anywhere, by a hash of the cell and the seed.
        let walkable = |cell: Cell| {
            let mut h = seed ^ (u64::from(cell.0.unsigned_abs()) << 32) ^ u64::from(cell.1.unsigned_abs());
            h ^= h >> 33;
            h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
            h ^= h >> 33;
            h % 100 >= density
        };
        let far = dx.abs().max(dy.abs());
        let on_the_line = |cell: Cell| {
            let (cx, cy) = (f64::from(cell.0 - from.0), f64::from(cell.1 - from.1));
            let (lx, ly) = (f64::from(dx), f64::from(dy));
            (cx * ly - cy * lx).abs() / lx.hypot(ly) <= 1.0
        };
        match nearer(from, to, walkable) {
            _ if far <= NEARER_THAN => prop_assert_eq!(nearer(from, to, walkable), Some(to)),
            Some(cell) => {
                let off = (cell.0 - from.0, cell.1 - from.1);
                prop_assert!(off.0.abs() <= NEARER_THAN && off.1.abs() <= NEARER_THAN, "{cell:?}");
                prop_assert!(off != (0, 0) && walkable(cell) && on_the_line(cell), "{cell:?}");
            }
            None => {
                // No cell of the line within 25 along each axis that may be stood on.
                for k in 1..=NEARER_THAN {
                    let cell = (from.0 + dx * k / far, from.1 + dy * k / far);
                    prop_assert!(!walkable(cell), "{cell:?}");
                }
            }
        }
    }
}
