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
/// The cell flag that refuses a creature (`+0x1E` bit 1, docs/02 §5.2).
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

/// A type by the entries' weights; `None` when they weigh nothing.
pub fn choose(entries: &[Entry], rng: &mut Rng) -> Option<Entry> {
    let total: u32 = entries.iter().map(|e| e.weight).sum();
    if total == 0 {
        return None;
    }
    let mut pick = rng.below(total);
    entries.iter().copied().find(|e| {
        if pick < e.weight {
            true
        } else {
            pick -= e.weight;
            false
        }
    })
}

/// A cell of the sector of origin `(x0, y0)` for a creature, of up to 50 tried at random: one
/// that `walkable` takes with three more in a row after it along either axis (**hypothesis**
/// for the direction), whose `flags` lack bit 1.
pub fn spot(
    (x0, y0): Cell,
    rng: &mut Rng,
    walkable: impl Fn(Cell) -> bool,
    flags: impl Fn(Cell) -> Option<u8>,
) -> Option<Cell> {
    (0..TRIES).find_map(|_| {
        let cell = (
            x0 + i32::try_from(rng.below(64)).ok()?,
            y0 + i32::try_from(rng.below(64)).ok()?,
        );
        let row =
            |dx: i32, dy: i32| (0..=ROW).all(|k| walkable((cell.0 + k * dx, cell.1 + k * dy)));
        (flags(cell).is_some_and(|f| f & REFUSED == 0) && (row(1, 0) || row(0, 1))).then_some(cell)
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
        // Only the cells of row 10 are walkable, x 20 to 23: the one spot is (20, 10).
        let walkable = |(x, y): Cell| y == 10 && (20..=23).contains(&x);
        let flags = |_| Some(0);
        let mut found = std::collections::HashSet::new();
        for record in 0..400 {
            let mut rng = Rng::of(0, 0, record);
            if let Some(cell) = spot((0, 0), &mut rng, walkable, flags) {
                found.insert(cell);
            }
        }
        assert_eq!(found.into_iter().collect::<Vec<_>>(), [(20, 10)]);
        // A cell of flag bit 1 refuses it; the same seed finds the same cell.
        let mut rng = Rng::of(0, 0, 1);
        assert_eq!(spot((0, 0), &mut rng, |_| true, |_| Some(REFUSED)), None);
        let (mut a, mut b) = (Rng::of(5, 6, 7), Rng::of(5, 6, 7));
        assert_eq!(a.below(1000), b.below(1000));
    }
}
