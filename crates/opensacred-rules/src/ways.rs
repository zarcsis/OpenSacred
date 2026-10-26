//! The ways and the cells in the way (docs/re/movement.md): the search of the fewest steps over
//! the walkable cells of a 64 × 64 window (`0x421B90`), on a layer and, through the doors, the
//! others; the straight line's test of the cells it crosses; and who stops whose step into a
//! cell that another creature holds (`0x4F7E10`, "The creatures in the way").
//! `opensacred-game` (`paths`, `crowd`) keeps the world's points, the cells that the creatures
//! stand on and the systems that walk them; this module is the rules alone.

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

use crate::Cell;

/// The side of the window the search covers, in cells, around the start's cell.
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

/// A point of the ground in cells: the cell `(x, y)` spans `x` to `x + 1` and `y` to `y + 1`.
pub type Point = (f32, f32);

/// The cell that holds `point`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the cell of a point in the world is a small whole number"
)]
pub fn cell_at(point: Point) -> Cell {
    (point.0.floor() as i32, point.1.floor() as i32)
}

/// As [`path_onto`], to a place in cell `to` on any layer.
pub fn path<L: Copy + Eq + Hash>(
    from: (Cell, L),
    to: Cell,
    neighbours: impl Fn((Cell, L)) -> Vec<(Cell, L)>,
) -> Option<Vec<(Cell, L)>> {
    path_onto(from, to, |_| true, neighbours)
}

/// The places from `from` to a place in cell `to` on a layer that `arrives` takes (the level that
/// a click chose, docs/re/movement.md, "The cursor's point"), both included, of the fewest steps
/// over the places `neighbours` gives (a cell on a layer: the walkable neighbours on its layer,
/// and the same cell on another layer through a door), within the window around `from`'s cell;
/// `None` when `to` lies outside the window or cannot be reached.
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

/// How far along an axis a creature's way's search takes its target as it is (`0x4FABE0`).
const NEARER_THAN: i32 = 25;

/// The cell that a creature's way's search from `from` heads for, its target `to` (`0x4FABE0`):
/// a target farther than 25 cells along an axis it first brings nearer along the line, to the
/// farthest cell of the line within 25 along each axis that it may stand on (`walkable`, the
/// test of `0x4EBCA0`); `None` when none of them is, the start's own cell aside.
pub fn nearer(from: Cell, to: Cell, walkable: impl Fn(Cell) -> bool) -> Option<Cell> {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let far = dx.abs().max(dy.abs());
    if far <= NEARER_THAN {
        return Some(to);
    }
    (1..=NEARER_THAN)
        .rev()
        .map(|k| (from.0 + dx * k / far, from.1 + dy * k / far))
        .find(|&cell| walkable(cell))
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

/// How near each other along a line, in cells, its crossings of a column's edge and of a row's
/// make it pass a corner: a point of the world in `f32` carries about 6·10⁻⁴ of a cell (a
/// coordinate of 300,000 units to 0.03), so that a line between two cells' centres along their
/// diagonal passes a hair beside the corners it runs through.
const CORNER: f32 = 2e-3;

/// Whether the straight line from `a` to `b` on the ground crosses walkable cells only, but
/// for the cell of `a`.
///
/// Every cell the line passes through counts, cell by cell along it, and where it passes a
/// corner, the two cells beside the corner too: a hero cannot slip between two cells that touch
/// at a corner.
pub fn clear(a: Point, b: Point, walkable: impl Fn(Cell) -> bool) -> bool {
    clear_by(a, b, &walkable, &walkable)
}

/// Whether the straight line from `a` to `b` crosses only cells that `crossed` takes, but for
/// the cell of `a`, and passes a corner only between two cells that `beside` takes: as
/// [`clear`], for a line that keeps to some cells, a way's, while a corner it passes needs only
/// cells that may be walked on beside it, as the way's own diagonal steps do ([`around`]).
pub fn clear_by(
    a: Point,
    b: Point,
    crossed: impl Fn(Cell) -> bool,
    beside: impl Fn(Cell) -> bool,
) -> bool {
    let (mut cell, end) = (cell_at(a), cell_at(b));
    let d = (b.0 - a.0, b.1 - a.1);
    let step = (sign(d.0), sign(d.1));
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
    let (mut tx, mut ty) = (next(a.0, d.0), next(a.1, d.1));
    let (dx, dy) = (1.0 / d.0.abs(), 1.0 / d.1.abs());
    let length = d.0.hypot(d.1);
    let cells = (end.0 - cell.0).abs() + (end.1 - cell.1).abs();
    for _ in 0..cells {
        if (tx - ty).abs() * length < CORNER {
            // Through a corner.
            if !beside((cell.0 + step.0, cell.1)) || !beside((cell.0, cell.1 + step.1)) {
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
        if !crossed(cell) {
            return false;
        }
        if cell == end {
            break;
        }
    }
    true
}

/// A door's outer and inner points as cells (docs/re/ai.md, "Houses and doors"; #273, #431),
/// measured from the doors' models, placements and the floors' grids rather than taken from the
/// client's table of its types (`0x43B290`): of the four cells that share a side with the door's
/// own, the one that lies the most along the door's normal on each side of its leaf.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DoorPoints {
    /// The door's cell, its trigger's, and its normal on the grid toward the floor's side:
    /// `(u_y, −u_x)` of its leaf's direction `u`, away from the viewer.
    door: Cell,
    normal: Point,
    /// The cell on the floor's side and the one on the other, each with whether it is walkable
    /// on the building's floor (`true`) or on the ground; `None` where no cell of the side is.
    pub inner: Option<(Cell, bool)>,
    pub outer: Option<(Cell, bool)>,
}

/// The four cells that share a side with a cell.
const SIDES: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];

impl DoorPoints {
    /// The points of the door on cell `door` whose leaf runs along `along` on the grid (its
    /// type's heading, `items.pak +0x57`), by `walkable(cell, floor)`, a cell's walkability on the
    /// building's floor or on the ground: the inner point the cell of the floor's side walkable
    /// on the floor, else on the ground, and the outer the cell of the other side walkable on the
    /// ground, else on the floor; so that a house's door has its inner point on the floor and its
    /// outer on the ground, and a door with the same on both sides a point on each.
    pub fn of(door: Cell, along: Point, walkable: impl Fn(Cell, bool) -> bool) -> Self {
        let normal = (along.1, -along.0);
        #[expect(
            clippy::cast_precision_loss,
            reason = "a step of one cell along an axis"
        )]
        let best = |toward: f32, floor: bool| {
            SIDES
                .iter()
                .map(|&(dx, dy)| {
                    let lies = toward * (dx as f32 * normal.0 + dy as f32 * normal.1);
                    ((door.0 + dx, door.1 + dy), lies)
                })
                .filter(|&(cell, lies)| lies > 0.0 && walkable(cell, floor))
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(cell, _)| (cell, floor))
        };
        Self {
            door,
            normal,
            inner: best(1.0, true).or_else(|| best(1.0, false)),
            outer: best(-1.0, false).or_else(|| best(-1.0, true)),
        }
    }

    /// The point that a creature at `here` (in cells), on a building's floor or not (`on_floor`),
    /// walks to for the door (`0x53A170`): the inner point on the floor and the outer on the
    /// ground where one lies on the floor and the other on the ground, as the client's types of a
    /// pair give them; otherwise the one on its own side of the leaf, as its types of two points
    /// give the nearer; the other where its side has none.
    pub fn near(&self, here: Point, on_floor: bool) -> Option<Cell> {
        let cell = |point: Option<(Cell, bool)>| point.map(|(cell, _)| cell);
        if let (Some((inner, true)), Some((outer, false))) = (self.inner, self.outer) {
            return Some(if on_floor { inner } else { outer });
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "a global cell, below 6,400, is exact in an f32"
        )]
        let centre = (self.door.0 as f32 + 0.5, self.door.1 as f32 + 0.5);
        let side = (here.0 - centre.0) * self.normal.0 + (here.1 - centre.1) * self.normal.1;
        if side > 0.0 {
            cell(self.inner).or(cell(self.outer))
        } else {
            cell(self.outer).or(cell(self.inner))
        }
    }
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

/// The bits of a creature's flags word (`+0x1F4`) that stop the hero: a hostile creature, awake
/// (bit 0) or of a dormant group (bit 1).
const STOPS_THE_HERO: u32 = 0x3;
/// The bit of the flags word that the creature maker sets on a hostile creature (`0x480D40`).
pub const HOSTILE: u32 = 0x1;
/// The bit of a creature's flags word that keeps it from stopping others: the critters', which
/// their constructor sets ([`constructed`]).
pub const CRITTER: u32 = 0x100_0000;
/// The bit of a `Creature.pak` record's flags of the big creatures, which stop no step by their
/// cell (`0x426CB0`).
pub const BIG: u8 = 0x02;
/// The bit of a `Creature.pak` record's flags whose creatures the constructor makes critters
/// (`0x52DE40`).
const CRITTER_RECORD: u8 = 0x20;

/// How a creature or an object stands in the others' way, as the client's test of a step has it
/// stop another's step into its cell (`0x4F7E10`, docs/re/movement.md, "The creatures in the
/// way").
///
/// A creature stops others only when it is alive, not big (its `Creature.pak` flags' bit
/// `0x02`) and not a critter (its flags word's bit `0x1000000`, which only a fill's critters keep
/// from their constructor, [`constructed`]); and it stops the hero, of id 1,
/// only when its flags word has bit 0 or 1 (`CreateNPC`'s tags `0x12` and `0x13`: the monsters,
/// the start's captain), every other creature, of an id above 16, whatever its flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InTheWay {
    /// Whether it stops the others' steps: not big, not a critter.
    pub stops: bool,
    /// Whether it stops the hero's too: its flags word has bit 0 or 1.
    pub hostile: bool,
    /// Whether it is a creature, which crowds another in its cell (`0x54FCB0` counts the types
    /// of the categories 2, 3 and `0x20`, `0x426610`); an object is not.
    pub creature: bool,
}

impl InTheWay {
    /// An object of category 4 (a chest, a barrel, a vase, a coffin, a cabinet, a book stand, a
    /// shrine), which `0x4F7E10` has stop a creature of id above 16 as a creature does, not the
    /// hero.
    pub fn object() -> Self {
        Self {
            stops: true,
            hostile: false,
            creature: false,
        }
    }

    /// A creature whose `Creature.pak` record has `record_flags` and whose flags word is `word`.
    pub fn of(record_flags: u8, word: u32) -> Self {
        Self {
            stops: record_flags & BIG == 0 && word & CRITTER == 0,
            hostile: word & STOPS_THE_HERO != 0,
            creature: true,
        }
    }

    /// Whether it stops a step into its cell (`0x4F7E10`): the hero's (`hero`) when it is
    /// hostile, another's whenever it stops the others' at all.
    pub fn stops_step(self, hero: bool) -> bool {
        self.stops && (self.hostile || !hero)
    }
}

/// The guards' mode of the flags word (`CreateNPC`'s tag `0x46`).
const GUARDS_MODE: u32 = 0x4000;
/// The soldiers' class (`Creature.pak +4`, `0x43ADC0`).
const SOLDIERS: u8 = 7;

/// Whether a follower in its master's list (`+0x39C`) stops its master's steps all the same
/// (`0x4F7E10`): only where it is hostile to the master (`hostile`, `0x548F60`), of the soldiers'
/// class 7 or of the guards' mode (its flags word `word`'s bit `0x4000`); else its master steps
/// through it whatever its flags, a summoned one's bit 0 among them (#1013).
pub fn stops_its_master(hostile: bool, class: u8, word: u32) -> bool {
    hostile || class == SOLDIERS || word & GUARDS_MODE != 0
}

/// The bits that the constructor (`0x52DE40`) sets on the flags word of a creature of type
/// `creature`, whose `Creature.pak` record has `record_flags`: [`CRITTER`] on the critters, some
/// types by their numbers and the types whose record has [`CRITTER_RECORD`].
/// `CreateNPC` writes the whole word from its tags (`0x4839F6`), so that only a fill's creatures
/// keep them (read in the original 2026-10-22, #468: a fill's rabbit `0x1000040`, `CreateNPC`'s
/// chickens `0x20` alone).
pub fn constructed(creature: u32, record_flags: u8) -> u32 {
    let critter = matches!(
        creature,
        0x47 | 0x4B
            | 0x201..=0x204
            | 0x24D
            | 0x24E
            | 0x250..=0x254
            | 0x230..=0x233
            | 0x235
            | 0x236
    ) || record_flags & CRITTER_RECORD != 0;
    if critter { CRITTER } else { 0 }
}

/// The flags word (`+0x1F4`) that `CreateNPC` (`0x482510`) makes from a record's operand-less
/// tags, in order (docs/02 §7.3): one mode in the low 16 bits, the last of the mode tags; a few
/// tags add bits above it.
pub fn flags_word(tags: impl IntoIterator<Item = u8>) -> u32 {
    let (mut mode, mut more) = (0u32, 0u32);
    for tag in tags {
        match tag {
            0x12 => mode = 0x1,
            0x13 => mode = 0x2,
            0x14 => mode = 0,
            0x2B => mode = 0x8,
            0x2E => mode = 0x10,
            0x30 => mode = 0x20,
            0x2F | 0x31 => mode = 0x40,
            0x32 => mode = 0x80,
            0x42 => mode = 0x400,
            0x43 => mode = 0x800,
            0x44 => mode = 0x1000,
            0x46 => mode = 0x4000,
            0x4C => mode = 0x8000,
            0x45 => more |= 0x2000,
            0x50 => more |= 0x1_0000,
            0x74 => more |= 0x800_0000,
            0x85 => {
                mode = 0;
                more |= 0x200_0000;
            }
            0x8A => {
                mode = 0;
                more |= 0x1000_0000;
            }
            _ => {}
        }
    }
    mode | more
}

/// The flags word that a `SetNPCState` record's operand-less tags make, in order, 0 for none
/// (`0x48BB40`, docs/re/netscript.md, "`SetNPCState`"): the modes of [`flags_word`], each of which
/// replaces the low 16 bits, so that a mode after `0x45` takes its `0x2000` away, unlike
/// `CreateNPC`'s; `0x45`, `0x50` and `0x74` add their bits, `0x85` and `0x8A` clear the mode and
/// set theirs.
pub fn state_word(tags: impl IntoIterator<Item = u8>) -> u32 {
    let mut word = 0u32;
    for tag in tags {
        let mode = |value: u32| (word & 0xFFFF_0000) | value;
        word = match tag {
            0x12 => mode(0x1),
            0x13 => mode(0x2),
            0x14 => mode(0),
            0x2B => mode(0x8),
            0x2E => mode(0x10),
            0x30 => mode(0x20),
            0x2F | 0x31 => mode(0x40),
            0x32 => mode(0x80),
            0x42 => mode(0x400),
            0x43 => mode(0x800),
            0x44 => mode(0x1000),
            0x46 => mode(0x4000),
            0x4C => mode(0x8000),
            0x45 => word | 0x2000,
            0x50 => word | 0x1_0000,
            0x74 => word | 0x800_0000,
            0x85 => mode(0x200_0000),
            0x8A => mode(0x1000_0000),
            _ => word,
        };
    }
    word
}

/// The bits of a creature's flags word that a `SetNPCState`'s word leaves as they were
/// (`0x461540`): `0x2000` and the boss's `0x80000` among them.
const KEPT_BY_STATE: u32 = 0x668_2304;

/// A creature's flags word `old` after a `SetNPCState` whose tags make `word` ([`state_word`])
/// (`0x461540`): the record's word with the old one's [`KEPT_BY_STATE`] bits, or the old word
/// where the record makes none.
pub fn stated_word(old: u32, word: u32) -> u32 {
    if word == 0 {
        old
    } else {
        word | (old & KEPT_BY_STATE)
    }
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
    fn a_far_target_is_brought_nearer_along_the_line() {
        let open = |_: Cell| true;
        // Within 25 along each axis, as it is; farther, 25 along the longer axis.
        assert_eq!(nearer((10, 10), (35, -15), open), Some((35, -15)));
        assert_eq!(nearer((10, 10), (10, 50), open), Some((10, 35)));
        assert_eq!(nearer((0, 0), (-60, 30), open), Some((-25, 12)));
        // The farthest of the line that may be stood on; none, none.
        let wall = |cell: Cell| cell.1 < 30;
        assert_eq!(nearer((10, 10), (10, 50), wall), Some((10, 29)));
        assert_eq!(nearer((10, 10), (10, 50), |_| false), None);
    }

    #[test]
    fn no_way_slips_between_two_cells_that_touch_at_a_corner() {
        let walkable = |cell: Cell| cell != (5, 5) && cell != (6, 6);
        // From the centre of (5, 6) to that of (6, 5).
        assert!(!clear((5.5, 6.5), (6.5, 5.5), walkable));
        let way = path(((5, 6), ()), (6, 5), |(cell, layer)| {
            around(cell, walkable)
                .map(|next| (next, layer))
                .collect::<Vec<_>>()
        })
        .unwrap();
        assert!(way.len() > 3, "{way:?}");
        // Along a row, and past a single corner.
        assert!(clear((2.5, 2.5), (9.5, 2.5), walkable));
        assert!(clear((4.5, 4.5), (5.5, 3.5), walkable));
        // Not through the wall; a point's cell is the one it lies in.
        assert!(!clear((2.5, 4.5), (8.5, 4.5), wall));
        assert_eq!(cell_at((5.99, -0.01)), (5, -1));
    }

    #[test]
    fn a_line_a_hair_beside_a_corner_passes_the_corner() {
        // Two cells' centres far out in the world, as they come back from its `f32` coordinates,
        // a few ten-thousandths of a cell off: the line between them on their diagonal crosses
        // the two cells alone and asks only the cells beside the corner (#981).
        let (a, b) = ((5940.5004, 403.4996), (5939.4997, 404.5003));
        let ends = |cell: Cell| cell == (5940, 403) || cell == (5939, 404);
        assert!(clear_by(a, b, ends, |_| true));
        assert!(!clear_by(a, b, ends, |cell| cell != (5940, 404)));
        // A line that passes a tenth of a cell beside it crosses a third cell.
        assert!(!clear_by((5940.6, 403.5), (5939.5, 404.5), ends, |_| true));
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
        // The door's cell lies on both layers: the way to it ends on the level asked for, and on
        // the nearer when none is (#208).
        let to_door = |arrives: fn(bool) -> bool| {
            path_onto(((5, 9), false), (5, 6), arrives, neighbours)
                .and_then(|way| way.last().copied())
        };
        assert_eq!(to_door(|_| true), Some(((5, 6), false)));
        assert_eq!(to_door(|inside| inside), Some(((5, 6), true)));
    }

    #[test]
    fn set_npc_state_tags_make_a_word_that_replaces_the_creatures_but_its_kept_bits() {
        assert_eq!(state_word([]), 0);
        assert_eq!(state_word([0x12]), 0x1);
        assert_eq!(state_word([0x46]), 0x4000);
        assert_eq!(state_word([0x74, 0x2B]), 0x800_0008);
        // A mode after `0x45` takes its bit away, unlike `CreateNPC`'s word.
        assert_eq!(state_word([0x45, 0x12]), 0x1);
        assert_eq!(flags_word([0x45, 0x12]), 0x2001);
        assert_eq!(state_word([0x12, 0x45]), 0x2001);
        assert_eq!(state_word([0x50, 0x85]), 0x201_0000);
        assert_eq!(state_word([0x13, 0x14]), 0);
        // The captain of 52,39 (`0E 46 13`) made a guard: its mode is replaced, a boss's bit
        // and the bit `0x2000` stay.
        let captain = flags_word([0x0E, 0x46, 0x13]) | 0x8_0000 | 0x2000;
        assert_eq!(stated_word(captain, state_word([0x46])), 0x8_6000);
        assert_eq!(stated_word(captain, 0), captain);
    }

    #[test]
    fn create_npc_tags_make_the_flags_word() {
        // The guards (`0E 46`), the captain (`0E 46 13`), the monsters (`08 12`).
        assert_eq!(flags_word([0x0E, 0x46]), 0x4000);
        assert_eq!(flags_word([0x0E, 0x46, 0x13]), 0x2);
        assert_eq!(flags_word([0x08, 0x12]), 0x1);
        assert_eq!(flags_word([0x12, 0x45, 0x85]), 0x200_2000);
    }

    #[test]
    fn the_hero_is_stopped_by_the_hostile_alone_and_the_others_by_all() {
        let guard = InTheWay::of(0, flags_word([0x0E, 0x46]));
        let captain = InTheWay::of(0, flags_word([0x0E, 0x46, 0x13]));
        let hero = InTheWay::of(0, 0);
        let chest = InTheWay::object();
        // The hero walks through the guard and past the chest, not through the captain.
        assert!(!guard.stops_step(true) && !chest.stops_step(true));
        assert!(captain.stops_step(true));
        // An NPC is stopped by the guard, the captain, the hero and the chest.
        assert!(
            [guard, captain, hero, chest]
                .iter()
                .all(|way| way.stops_step(false))
        );
        // A fill's critter, by its type or its record, keeps its constructor's bit, and it and a
        // big creature stop no one; they crowd another all the same, a chest does not.
        let critters = [
            InTheWay::of(0, flags_word([0x12]) | constructed(0x4B, 0)),
            InTheWay::of(CRITTER_RECORD, HOSTILE | constructed(295, CRITTER_RECORD)),
        ];
        let troll = InTheWay::of(BIG, flags_word([0x12]));
        for way in critters.iter().chain([&troll]) {
            assert!(!way.stops_step(false) && !way.stops_step(true), "{way:?}");
            assert!(way.creature);
        }
        assert!(!chest.creature);
        // `CreateNPC`'s chicken, its word written from its tags, stops the others, not the hero.
        let chicken = InTheWay::of(0, flags_word([0x30]));
        assert!(chicken.stops_step(false) && !chicken.stops_step(true));
    }

    #[test]
    fn a_follower_stops_its_master_only_hostile_a_soldier_or_a_guard() {
        // The Vampiress's wolf, of class 6 and the flags `0x101`, would stop her by its bit 0,
        // but not as her follower.
        assert!(InTheWay::of(0, 0x101).stops_step(true));
        assert!(!stops_its_master(false, 6, 0x101));
        assert!(stops_its_master(true, 6, 0x101));
        assert!(stops_its_master(false, SOLDIERS, 0x100));
        assert!(stops_its_master(false, 3, flags_word([0x46])));
    }

    #[test]
    fn the_constructor_marks_the_critters_by_their_types_and_records() {
        // The rabbits, their variants among them, the chickens, the small spiders: critters.
        for creature in [516, 592, 593, 560, 0x47] {
            assert_eq!(constructed(creature, 0), CRITTER, "{creature}");
        }
        // A pig and a soldier are not, unless the soldier's record says so.
        assert_eq!(constructed(520, 0) | constructed(295, 0), 0);
        assert_eq!(constructed(295, CRITTER_RECORD), CRITTER);
    }

    #[test]
    fn a_houses_door_has_its_inner_point_on_the_floor_and_its_outer_on_the_ground() {
        let door = (10, 10);
        // A door of 28°, its leaf along the grid's +x: the floor up the grid's −y, the ground
        // down it.
        let floor = |cell: Cell| cell.1 < 10;
        let walkable = |cell: Cell, on_floor: bool| on_floor == floor(cell) && cell.1 != 10;
        let points = DoorPoints::of(door, (1.0, 0.0), walkable);
        assert_eq!(points.inner, Some(((10, 9), true)));
        assert_eq!(points.outer, Some(((10, 11), false)));
        assert_eq!(points.near((10.5, 14.0), false), Some((10, 11)));
        assert_eq!(points.near((10.5, 7.0), true), Some((10, 9)));
        // A door of 333°, its leaf along −y: the floor along −x.
        let floor = |cell: Cell| cell.0 < 10;
        let walkable = |cell: Cell, on_floor: bool| on_floor == floor(cell) && cell.0 != 10;
        let points = DoorPoints::of(door, (0.0, -1.0), walkable);
        assert_eq!(points.inner, Some(((9, 10), true)));
        assert_eq!(points.outer, Some(((11, 10), false)));
    }

    #[test]
    fn a_door_of_17_degrees_whose_cell_across_is_a_walls_has_its_outer_point_beside() {
        // Its leaf 13.5° off +x toward −y; the cell across on the ground's side a wall's.
        let along = (13.5_f32.to_radians().cos(), -13.5_f32.to_radians().sin());
        let walkable = |cell: Cell, on_floor: bool| match cell {
            (10, 9) => on_floor,
            (11, 10) | (9, 10) => !on_floor,
            _ => false,
        };
        let points = DoorPoints::of((10, 10), along, walkable);
        assert_eq!(points.inner, Some(((10, 9), true)));
        // Of the ground's cells beside, the one on the heading's side, the ground's side of the
        // leaf by a little.
        assert_eq!(points.outer, Some(((11, 10), false)));
    }

    #[test]
    fn at_a_door_with_ground_on_both_sides_a_creature_takes_the_point_on_its_own_side() {
        let walkable = |cell: Cell, on_floor: bool| !on_floor && cell != (10, 10);
        let points = DoorPoints::of((10, 10), (1.0, 0.0), walkable);
        assert_eq!(points.inner, Some(((10, 9), false)));
        assert_eq!(points.outer, Some(((10, 11), false)));
        assert_eq!(points.near((10.5, 6.0), false), Some((10, 9)));
        assert_eq!(points.near((12.0, 13.0), false), Some((10, 11)));
        // A side without a cell to stand on gives the other's.
        let one_side = |cell: Cell, on_floor: bool| !on_floor && cell == (10, 11);
        let points = DoorPoints::of((10, 10), (1.0, 0.0), one_side);
        assert_eq!(points.near((10.5, 6.0), false), Some((10, 11)));
    }
}
