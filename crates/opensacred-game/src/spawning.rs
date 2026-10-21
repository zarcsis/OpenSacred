//! The creatures that the sectors' `FillSector` records make at random as their sectors come
//! near (`0x47C610`, docs/re/spawning.md, "`FillSector`"): how many a record makes, of which
//! types by their weights, and on which cells. `npcs` reads the records and places what they
//! make; this module is the rule alone.
//!
//! The original rolls with the C library's `rand()` at every `Enter` and keeps the living
//! creatures while the region is the same; the game rolls with a generator seeded by the sector
//! and the record, so that a sector that comes back has the same creatures where they were.

use crate::paths::Cell;

/// The tries at a cell for each creature (`0x47C610`).
const TRIES: u32 = 50;
/// The walkable cells in a row along an axis that a creature's cell needs after its own
/// (`0x45EF70`).
const ROW: i32 = 3;
/// The cell flag that refuses a creature, a settlement's ground (`+0x1E` bit 1, docs/02 §5.2).
const REFUSED: u8 = 0x2;
/// The `Creature.pak` classes whose records' density takes `SpawnAnimalsSP` rather than
/// `SpawnEnemysSP`.
const ANIMALS: [u8; 3] = [3, 4, 6];

/// A `FillSector` record of a sector's `Enter` function.
#[derive(Clone, Debug, PartialEq)]
pub struct Fill {
    /// `35:(a, b, area)`: `a` to `b` creatures per `area` cells.
    pub per_area: (u32, u32, u32),
    /// The `19:(type, weight, hostile)` entries, a bare `02:type` weighing 100, peaceful;
    /// `08` and `0E` already applied.
    pub entries: Vec<Entry>,
}

/// A type that a fill may make, its weight, and whether it is made hostile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub creature: u32,
    pub weight: u32,
    pub hostile: bool,
}

/// A sector's `SpawnValues`: its density in percent and its creatures' lowest and highest levels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnValues {
    pub density: u32,
    pub lowest: u32,
    pub highest: u32,
}

impl Default for SpawnValues {
    /// A sector without its record: no density added, levels from 1 up.
    fn default() -> Self {
        Self {
            density: 0,
            lowest: 1,
            highest: u32::MAX,
        }
    }
}

/// The balance's factors of the density: `SpawnAnimalsSP` for the animals' records, else
/// `SpawnEnemysSP`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Factors {
    pub animals: f32,
    pub enemies: f32,
}

/// Whether a record's density takes the animals' factor: its first type's class (the
/// `Creature.pak` record's `+4`) is 3, 4 or 6.
pub fn animals(first_class: Option<u8>) -> bool {
    first_class.is_some_and(|class| ANIMALS.contains(&class))
}

/// How many creatures a fill makes over a sector of 4,096 cells for a hero of `level`: its
/// `a` to `b` raised by the sector's density (`b` by all of it, `a` by all of it or by half when
/// `a = b`), the lower for a hero at or below the sector's lowest level, the higher at or above
/// its highest, one between at random otherwise; times the balance's factor (truncated), over
/// the record's area; at least 1.
pub fn count(fill: &Fill, values: SpawnValues, level: u32, factor: f32, rng: &mut Rng) -> u32 {
    let (a, b, area) = fill.per_area;
    let (a, b) = (a.min(b), a.max(b));
    let raised_a = if a == b {
        a + a * values.density / 200
    } else {
        a + a * values.density / 100
    };
    let raised_b = b + b * values.density / 100;
    let density = if level <= values.lowest {
        raised_a
    } else if level >= values.highest {
        raised_b
    } else {
        raised_a + rng.below(raised_b - raised_a + 1)
    };
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "a few dozen creatures, truncated as the client's integer density is"
    )]
    let density = (density as f32 * factor) as u32;
    let cells = 4096;
    (cells * density / area.max(1)).max(1)
}

/// A type by the entries' weights, as the client picks it (`0x47C610`): `r` at random below the
/// weights' sum, then the first entry whose sum of weights so far reaches `r`, which gives the
/// first one more chance than its weight; `None` when they weigh nothing.
pub fn choose(entries: &[Entry], rng: &mut Rng) -> Option<Entry> {
    let total: u32 = entries.iter().map(|e| e.weight).sum();
    if total == 0 {
        return None;
    }
    let pick = rng.below(total);
    let mut sum = 0;
    entries.iter().copied().find(|e| {
        sum += e.weight;
        sum >= pick
    })
}

/// The directions of the row of walkable cells after a creature's cell, in the order tried
/// (`0x45EF70`).
const ROWS: [Cell; 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// Whether a creature of `class` may stand on `cell` by its flags: a cell of bit 1 refuses
/// every class whose low nibble is not 0 (`0x47C610`).
fn allowed(cell: Cell, class: u8, flags: &impl Fn(Cell) -> Option<u8>) -> bool {
    flags(cell).is_some_and(|f| f & REFUSED == 0 || class & 0xF == 0)
}

/// A cell at random of the sector of origin `(x0, y0)`.
fn anywhere((x0, y0): Cell, rng: &mut Rng) -> Option<Cell> {
    Some((
        x0 + i32::try_from(rng.below(64)).ok()?,
        y0 + i32::try_from(rng.below(64)).ok()?,
    ))
}

/// A cell of the sector of origin `origin` for a creature of `class`, of up to 50 tried at
/// random: one that `walkable` takes with three more in a row after it, along `+x`, `-x`, `+y` or
/// `-y`, that its flags allow, and that `free` takes.
fn spot(
    origin: Cell,
    class: u8,
    rng: &mut Rng,
    walkable: &impl Fn(Cell) -> bool,
    flags: &impl Fn(Cell) -> Option<u8>,
    free: impl Fn(Cell) -> bool,
) -> Option<Cell> {
    (0..TRIES).find_map(|_| {
        let cell = anywhere(origin, rng)?;
        let row = |(dx, dy): Cell| (0..=ROW).all(|k| walkable((cell.0 + k * dx, cell.1 + k * dy)));
        (ROWS.into_iter().any(row) && allowed(cell, class, flags) && free(cell)).then_some(cell)
    })
}

/// A creature that a fill makes: its type, its cell, and whether it is hostile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Made {
    pub creature: u32,
    pub cell: Cell,
    pub hostile: bool,
    /// Its pack, by the place of its leader among the run's creatures, if it is in one.
    pub pack: Option<usize>,
}

/// What a creature of a fill is in its pack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Alone,
    /// A leader, with the slots left for its followers.
    Leader(u32),
    /// A follower, by its leader's place among the run's creatures.
    Follower(usize),
}

/// The types that lead every few of their kind under the run's second counter (`0x47C610`):
/// the dark elves and the ice elves, then the goblin warriors.
const ELVES: [u32; 6] = [319, 320, 321, 327, 140, 141];
const GOBLINS: [u32; 2] = [76, 118];
/// Under the third: the wolves.
const WOLVES: [u32; 4] = [512, 568, 569, 570];
/// The types that always lead: the champions of the elves, the wolf, and some others.
const CHAMPIONS: [u32; 2] = [326, 146];
const WOLF: u32 = 567;
const ALWAYS: [u32; 9] = [37, 43, 48, 67, 258, 342, 343, 349, 212];
/// How near, in cells, an earlier leader of the run keeps a leader off a cell (`0x47DEE5`): more
/// than a sector is wide, so a run places its first leader alone.
const LEADERS_APART: f32 = 2000.0;

/// The run's counters of the creatures that lead every few of their kind.
#[derive(Default)]
struct Counters([i32; 3]);

impl Counters {
    /// Counts a creature of counter `k` down, and when the counter was spent makes it a leader of
    /// `slots` followers and sets the counter to them.
    fn count(&mut self, k: usize, slots: u32) -> Option<u32> {
        let old = self.0[k];
        self.0[k] -= 1;
        (old <= 0).then(|| {
            self.0[k] = i32::try_from(slots).unwrap_or(0);
            slots
        })
    }
}

/// Whether a creature of `creature`, of `class`, leads a pack, and its slots (`0x47D755` to
/// `0x47DD6B`): under the run's three counters, each counting down the creatures of its kinds and
/// making a leader of the one that finds it spent (the classes 9 and 14 every three or four, the
/// elves every four to eight, the goblin warriors every four or five, the wolves every two to
/// four), or always (the champions); in a dungeon, the slots and the counter halved.
fn lead(
    creature: u32,
    class: Option<u8>,
    counters: &mut Counters,
    dungeon: bool,
    rng: &mut Rng,
) -> Option<u32> {
    let halve = |slots: u32| if dungeon { slots / 2 } else { slots };
    let mut leader = None;
    if matches!(class, Some(9 | 14)) {
        let slots = halve(rng.below(2) + 3);
        leader = counters.count(0, slots);
    }
    if ELVES.contains(&creature) {
        let slots = halve(rng.below(5) + 4);
        leader = counters.count(1, slots);
    } else if GOBLINS.contains(&creature) {
        let slots = halve(rng.below(2) + 4);
        leader = counters.count(1, slots);
    }
    if WOLVES.contains(&creature) {
        let slots = halve(rng.below(3) + 2);
        leader = counters.count(2, slots);
    }
    if CHAMPIONS.contains(&creature) {
        leader = Some(halve(rng.below(5) + 4));
    } else if creature == WOLF {
        leader = Some(halve(rng.below(2) + 3));
    } else if ALWAYS.contains(&creature) {
        leader = Some(halve(rng.below(4) + 3));
    }
    leader
}

/// What a fill's run takes of the world: the sector's origin, whether it is a dungeon, each
/// type's `Creature.pak` class, and the cells' walkability and flags.
pub struct Ground<C, W, F> {
    pub origin: Cell,
    pub dungeon: bool,
    pub class: C,
    pub walkable: W,
    pub flags: F,
}

/// The creatures a fill makes, `count` of them (`0x47C610`): first each one's type by the
/// weights, whether it leads a pack ([`lead`]), and a cell ([`spot`]; a leader's also no nearer
/// an earlier leader than [`LEADERS_APART`]), a creature without one dropped; then each one that
/// does not lead put by the first leader of its class with slots left, within two cells of it,
/// and joined to its pack, or else on any walkable cell of the sector, or else left where it was;
/// last, some of them made of their type's variants ([`variant`]).
pub fn make<C, W, F>(fill: &Fill, count: u32, ground: &Ground<C, W, F>, rng: &mut Rng) -> Vec<Made>
where
    C: Fn(u32) -> Option<u8>,
    W: Fn(Cell) -> bool,
    F: Fn(Cell) -> Option<u8>,
{
    let mut placed: Vec<(Entry, Cell, Role)> = Vec::new();
    let mut counters = Counters::default();
    for _ in 0..count {
        let Some(entry) = choose(&fill.entries, rng) else {
            break;
        };
        let class = (ground.class)(entry.creature);
        let leads = lead(entry.creature, class, &mut counters, ground.dungeon, rng);
        let apart = |cell: Cell| {
            leads.is_none()
                || placed.iter().all(|&(_, at, role)| {
                    !matches!(role, Role::Leader(_)) || distance(at, cell) >= LEADERS_APART
                })
        };
        let cell = spot(
            ground.origin,
            class.unwrap_or(0),
            rng,
            &ground.walkable,
            &ground.flags,
            apart,
        );
        if let Some(cell) = cell {
            placed.push((entry, cell, leads.map_or(Role::Alone, Role::Leader)));
        }
    }
    for index in 0..placed.len() {
        let (entry, _, role) = placed[index];
        if role != Role::Alone {
            continue;
        }
        let class = (ground.class)(entry.creature);
        let leader = placed.iter().position(|&(other, _, role)| {
            matches!(role, Role::Leader(slots) if slots > 0)
                && (ground.class)(other.creature) == class
        });
        let near = leader.and_then(|leader| {
            let (_, at, _) = placed[leader];
            (0..TRIES).find_map(|_| {
                let dx = i32::try_from(rng.below(5)).ok()? - 2;
                let dy = i32::try_from(rng.below(5)).ok()? - 2;
                let cell = (at.0 + dx, at.1 + dy);
                ((ground.walkable)(cell) && allowed(cell, class.unwrap_or(0), &ground.flags))
                    .then_some((leader, cell))
            })
        });
        if let Some((leader, cell)) = near {
            if let (_, _, Role::Leader(slots)) = &mut placed[leader] {
                *slots -= 1;
            }
            placed[index] = (entry, cell, Role::Follower(leader));
        } else if let Some(cell) = (0..TRIES)
            .find_map(|_| anywhere(ground.origin, rng).filter(|&cell| (ground.walkable)(cell)))
        {
            placed[index].1 = cell;
        }
    }
    placed
        .into_iter()
        .enumerate()
        .map(|(index, (entry, cell, role))| Made {
            creature: variant(entry.creature, rng),
            cell,
            hostile: entry.hostile,
            pack: match role {
                Role::Leader(_) => Some(index),
                Role::Follower(leader) => Some(leader),
                Role::Alone => None,
            },
        })
        .collect()
}

/// The distance between two cells, in cells.
#[expect(
    clippy::cast_precision_loss,
    reason = "cells of the world, below 6,400, are exact in an f32"
)]
fn distance(a: Cell, b: Cell) -> f32 {
    let (dx, dy) = ((a.0 - b.0) as f32, (a.1 - b.1) as f32);
    (dx * dx + dy * dy).sqrt()
}

/// The type a creature of `creature` is made of (`0x47E551`): a quarter of the rabbits (516) of
/// one of their variants 592 and 593, a sixth of those of another of the three, a ninth of the
/// crows (513) of their variants 589 and 590.
pub fn variant(creature: u32, rng: &mut Rng) -> u32 {
    let mut pick = |chance: u32, even: u32, odd: u32| {
        (rng.below(chance) == 1).then(|| if rng.below(2) == 0 { even } else { odd })
    };
    let made = match creature {
        516 => pick(4, 593, 592),
        592 => pick(6, 593, 516),
        593 => pick(6, 516, 592),
        513 => pick(9, 590, 589),
        _ => None,
    };
    made.unwrap_or(creature)
}

/// The types a fill may make, its entries' and their variants.
pub fn types(fill: &Fill) -> impl Iterator<Item = u32> + '_ {
    fill.entries.iter().flat_map(|entry| match entry.creature {
        516 | 592 | 593 => vec![516, 592, 593],
        513 => vec![513, 589, 590],
        other => vec![other],
    })
}

/// A small generator of the game's own (SplitMix64), seeded by a sector and a record.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    /// The generator of record `record` of sector `(x, y)`'s fills.
    pub fn of(x: u32, y: u32, record: usize) -> Self {
        let record = u64::try_from(record).unwrap_or(u64::MAX);
        Self(u64::from(x) << 40 ^ u64::from(y) << 20 ^ record ^ 0x05EE_D0F5_ACE5)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number below `n`, 0 for `n` 0.
    pub fn below(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        u32::try_from(self.next() % u64::from(n)).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rabbits() -> Fill {
        Fill {
            per_area: (6, 6, 4096),
            entries: vec![Entry {
                creature: 516,
                weight: 100,
                hostile: false,
            }],
        }
    }

    #[test]
    fn the_starts_rabbits_are_five_a_record() {
        // The start's sectors: density 50 %, levels 1 to 4; a hero of level 1; the animals'
        // factor 0.8: 6 raised by a quarter is 7, times 0.8 is 5 (docs/re/spawning.md).
        let values = SpawnValues {
            density: 50,
            lowest: 1,
            highest: 4,
        };
        let mut rng = Rng::of(50, 39, 0);
        assert_eq!(count(&rabbits(), values, 1, 0.8, &mut rng), 5);
        // A hero above the highest level gets the higher, 9, times 1.2 for monsters: 10.
        let fill = Fill {
            per_area: (4, 6, 4096),
            ..rabbits()
        };
        assert_eq!(count(&fill, values, 9, 1.2, &mut rng), 10);
        // Between the levels, between the two.
        let n = count(&fill, values, 2, 1.0, &mut rng);
        assert!((6..=9).contains(&n), "{n}");
        // At least one.
        let few = Fill {
            per_area: (0, 0, 4096),
            ..rabbits()
        };
        assert_eq!(count(&few, values, 1, 0.8, &mut rng), 1);
        assert!(animals(Some(4)) && !animals(Some(14)) && !animals(None));
    }

    #[test]
    fn the_types_come_by_their_weights() {
        let entries = [
            Entry {
                creature: 1,
                weight: 75,
                hostile: true,
            },
            Entry {
                creature: 2,
                weight: 25,
                hostile: true,
            },
            Entry {
                creature: 3,
                weight: 0,
                hostile: true,
            },
        ];
        let mut rng = Rng::of(48, 39, 3);
        let mut made = [0; 4];
        for _ in 0..4000 {
            let entry = choose(&entries, &mut rng).unwrap();
            made[usize::try_from(entry.creature).unwrap()] += 1;
        }
        assert_eq!(made[3], 0);
        assert!((2800..3200).contains(&made[1]), "{made:?}");
        assert_eq!(choose(&entries[2..], &mut rng), None);
    }

    #[test]
    fn a_creature_stands_where_four_cells_in_a_row_are_free() {
        // Only the cells of row 10 are walkable, x 20 to 23: the spots are its ends, (20, 10)
        // with the row along +x and (23, 10) along -x.
        let walkable = |(x, y): Cell| y == 10 && (20..=23).contains(&x);
        let flags = |_| Some(0);
        let mut found = std::collections::BTreeSet::new();
        for record in 0..800 {
            let mut rng = Rng::of(0, 0, record);
            if let Some(cell) = spot((0, 0), 1, &mut rng, &walkable, &flags, |_| true) {
                found.insert(cell);
            }
        }
        assert_eq!(found.into_iter().collect::<Vec<_>>(), [(20, 10), (23, 10)]);
        // A cell of flag bit 1 refuses a creature of a class of a low nibble, not of class 0.
        let mut rng = Rng::of(0, 0, 1);
        let refused = |_| Some(REFUSED);
        assert_eq!(
            spot((0, 0), 3, &mut rng, &|_| true, &refused, |_| true),
            None
        );
        assert!(spot((0, 0), 0, &mut rng, &|_| true, &refused, |_| true).is_some());
        let (mut a, mut b) = (Rng::of(5, 6, 7), Rng::of(5, 6, 7));
        assert_eq!(a.below(1000), b.below(1000));
    }

    /// A sector of origin (0, 0), all walkable, of creatures all of one class.
    type Open = Ground<Box<dyn Fn(u32) -> Option<u8>>, fn(Cell) -> bool, fn(Cell) -> Option<u8>>;

    fn open_ground(class: u8) -> Open {
        Ground {
            origin: (0, 0),
            dungeon: false,
            class: Box::new(move |_| Some(class)),
            walkable: |(x, y)| (0..64).contains(&x) && (0..64).contains(&y),
            flags: |_| Some(0),
        }
    }

    #[test]
    fn a_run_places_its_first_leader_and_gathers_its_kind_about_it() {
        // Thieves (class 14): a leader every three or four; the run keeps its first leader alone
        // and drops the later ones, whose cells are all within 2,000 cells of it.
        let thieves = Fill {
            per_area: (12, 12, 4096),
            entries: vec![Entry {
                creature: 304,
                weight: 100,
                hostile: true,
            }],
        };
        let mut rng = Rng::of(48, 39, 0);
        let made = make(&thieves, 12, &open_ground(14), &mut rng);
        assert!(made.len() < 12 && made.len() >= 4, "{}", made.len());
        // The followers stand within two cells of the leader, up to its slots (three or four).
        let near = |a: Cell, b: Cell| (a.0 - b.0).abs() <= 2 && (a.1 - b.1).abs() <= 2;
        let leader = made[0].cell;
        let gathered = made[1..].iter().filter(|m| near(m.cell, leader)).count();
        assert!((3..=4).contains(&gathered), "{gathered} of {made:?}");
        assert!(made.iter().all(|m| m.hostile && m.creature == 304));
        // Rabbits lead no pack: all are made, each on a cell of its own choosing.
        let mut rng = Rng::of(50, 39, 0);
        let made = make(&rabbits(), 10, &open_ground(4), &mut rng);
        assert_eq!(made.len(), 10);
    }

    #[test]
    fn some_rabbits_and_crows_are_made_of_their_variants() {
        let mut rng = Rng::of(1, 2, 3);
        let mut made = std::collections::HashMap::new();
        for _ in 0..8000 {
            *made.entry(variant(516, &mut rng)).or_insert(0) += 1;
        }
        // Three quarters stay rabbits, an eighth each of 592 and 593.
        assert!((5700..6300).contains(&made[&516]), "{made:?}");
        assert!((800..1200).contains(&made[&592]) && (800..1200).contains(&made[&593]));
        assert_eq!(variant(76, &mut rng), 76);
        assert_eq!(types(&rabbits()).collect::<Vec<_>>(), [516, 592, 593]);
    }
}
