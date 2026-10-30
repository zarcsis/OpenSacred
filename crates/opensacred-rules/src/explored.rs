//! The land explored (docs/re/maps.md, "The lens and the fog of war"; #1568): each sector's
//! bitmap of 32 × 32 bits, a bit for 2 × 2 of its cells (the sector's `+0x180`), which the lens
//! of the world map and the overview map show as their fog of war, and the stamps of cells about
//! the hero that set its bits as it steps into a cell (`0x5997C0`): a disc 29 cells across by
//! day, 15 at night, and a smaller patch in a dungeon.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A sector's cells along each side, and its bitmap's bytes: 32 rows of 4 (`0x622B10`).
pub const SECTOR_CELLS: i32 = 64;
const ROW_BYTES: usize = 4;
pub const BYTES: usize = 32 * ROW_BYTES;
/// The world's cells along each side, below which a cell is explored (`0x638E00`).
const WORLD_CELLS: i32 = 8192;

/// A stamp of cells about the hero (`0x5997C0`): by day (`0x8915CC`, 616 cells), at night, 22:20
/// to 03:20 (`0x41BAB0`, `0x891328`, 145 cells), or in a dungeon (`0x891409`, 67 cells).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stamp {
    Day,
    Night,
    Dungeon,
}

/// Each stamp's rows from its first row's offset down, each its first and last cells' offsets
/// across, as the original's masks hold them (rasterised discs with their own unevenness).
const DAY: (i32, [(i8, i8); 29]) = (
    -14,
    [
        (-4, 4),
        (-6, 6),
        (-7, 7),
        (-8, 8),
        (-9, 9),
        (-10, 10),
        (-10, 10),
        (-11, 11),
        (-11, 11),
        (-12, 12),
        (-12, 12),
        (-13, 13),
        (-13, 13),
        (-14, 14),
        (-14, 14),
        (-14, 14),
        (-13, 13),
        (-13, 13),
        (-12, 12),
        (-12, 12),
        (-11, 11),
        (-11, 10),
        (-11, 10),
        (-10, 10),
        (-9, 9),
        (-8, 8),
        (-7, 6),
        (-6, 6),
        (-4, 4),
    ],
);
const NIGHT: (i32, [(i8, i8); 15]) = (
    -7,
    [
        (0, 0),
        (-3, 3),
        (-4, 4),
        (-5, 5),
        (-5, 5),
        (-6, 6),
        (-6, 6),
        (-7, 7),
        (-6, 6),
        (-6, 6),
        (-5, 5),
        (-5, 5),
        (-4, 4),
        (-3, 3),
        (0, 0),
    ],
);
const DUNGEON: (i32, [(i8, i8); 10]) = (
    -5,
    [
        (0, 1),
        (-2, 3),
        (-3, 3),
        (-3, 4),
        (-4, 5),
        (-4, 5),
        (-3, 4),
        (-3, 4),
        (-2, 3),
        (0, 1),
    ],
);

impl Stamp {
    /// The stamp at the minute of the day `minute` (`0x41BAB0`: night from 22:20 to 03:20), or in
    /// a dungeon (the renderer's `+0x9E0C`).
    pub fn at(minute: u32, dungeon: bool) -> Self {
        if dungeon {
            Self::Dungeon
        } else if !(200..1340).contains(&(minute % 1440)) {
            Self::Night
        } else {
            Self::Day
        }
    }

    /// Its rows: the first row's offset and each row's first and last cells' offsets.
    fn rows(self) -> (i32, &'static [(i8, i8)]) {
        match self {
            Self::Day => (DAY.0, &DAY.1),
            Self::Night => (NIGHT.0, &NIGHT.1),
            Self::Dungeon => (DUNGEON.0, &DUNGEON.1),
        }
    }

    /// The cells' offsets it covers.
    pub fn cells(self) -> impl Iterator<Item = (i32, i32)> {
        let (first, rows) = self.rows();
        rows.iter()
            .zip(first..)
            .flat_map(|(&(from, to), dy)| (i32::from(from)..=i32::from(to)).map(move |dx| (dx, dy)))
    }
}

/// The sector of a cell, and the cell's bit in its bitmap: its byte and its bit (`0x622B10`: the
/// bit `(dx >> 1) & 7` of the byte `(dy >> 1) · 4 + (dx >> 4)`).
fn place((x, y): (i32, i32)) -> Option<((u16, u16), usize, u8)> {
    if !(0..WORLD_CELLS).contains(&x) || !(0..WORLD_CELLS).contains(&y) {
        return None;
    }
    let sector = (
        u16::try_from(x / SECTOR_CELLS).ok()?,
        u16::try_from(y / SECTOR_CELLS).ok()?,
    );
    let (dx, dy) = (
        usize::try_from(x % SECTOR_CELLS).ok()?,
        usize::try_from(y % SECTOR_CELLS).ok()?,
    );
    let byte = (dy >> 1) * ROW_BYTES + (dx >> 4);
    let bit = u8::try_from((dx >> 1) & 7).ok()?;
    Some((sector, byte, bit))
}

/// The sectors' bitmaps of the land explored, by sector; a sector never stamped holds none.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Explored(BTreeMap<(u16, u16), Vec<u8>>);

impl Explored {
    /// The cells of `stamp` about the cell `cell` explored (`0x5997C0`).
    pub fn stamp(&mut self, cell: (i32, i32), stamp: Stamp) {
        for (dx, dy) in stamp.cells() {
            let Some((sector, byte, bit)) = place((cell.0 + dx, cell.1 + dy)) else {
                continue;
            };
            let bits = self.0.entry(sector).or_insert_with(|| vec![0; BYTES]);
            if let Some(byte) = bits.get_mut(byte) {
                *byte |= 1 << bit;
            }
        }
    }

    /// Whether the cell `cell` is explored (`0x638E60`).
    pub fn explored(&self, cell: (i32, i32)) -> bool {
        place(cell).is_some_and(|(sector, byte, bit)| {
            self.0
                .get(&sector)
                .and_then(|bits| bits.get(byte))
                .is_some_and(|byte| byte & (1 << bit) != 0)
        })
    }

    /// Whether the bit (`column`, `row`), 2 × 2 cells, of the sector `sector`'s bitmap is set:
    /// the texel of its fog (`0x638F30`).
    pub fn bit(&self, sector: (u16, u16), column: usize, row: usize) -> bool {
        self.0
            .get(&sector)
            .and_then(|bits| bits.get(row * ROW_BYTES + column / 8))
            .is_some_and(|byte| byte & (1 << (column % 8)) != 0)
    }

    /// Whether the sector `sector` holds a bitmap.
    pub fn holds(&self, sector: (u16, u16)) -> bool {
        self.0.contains_key(&sector)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stamps_cover_their_cells() {
        assert_eq!(Stamp::Day.cells().count(), 616);
        assert_eq!(Stamp::Night.cells().count(), 145);
        assert_eq!(Stamp::Dungeon.cells().count(), 67);
        assert!(Stamp::Day.cells().any(|cell| cell == (14, 0)));
        assert!(!Stamp::Day.cells().any(|cell| cell == (14, -14)));
    }

    #[test]
    fn night_runs_from_22_20_to_3_20() {
        assert_eq!(Stamp::at(22 * 60 + 19, false), Stamp::Day);
        assert_eq!(Stamp::at(22 * 60 + 20, false), Stamp::Night);
        assert_eq!(Stamp::at(3 * 60 + 19, false), Stamp::Night);
        assert_eq!(Stamp::at(3 * 60 + 20, false), Stamp::Day);
        assert_eq!(Stamp::at(12 * 60, true), Stamp::Dungeon);
    }

    #[test]
    fn a_stamp_sets_its_cells_bits_by_two_by_two() {
        let mut explored = Explored::default();
        explored.stamp((3236, 2511), Stamp::Night);
        // Within the disc, and its 2 × 2 neighbour sharing the bit.
        assert!(explored.explored((3236, 2511)));
        assert!(explored.explored((3243, 2511)));
        assert!(explored.explored((3237, 2510)));
        assert!(!explored.explored((3250, 2511)));
        // The cell (3236, 2511) is the sector (50, 39)'s (36, 15): the bit 18 of the row 7.
        assert!(explored.bit((50, 39), 18, 7));
        assert!(!explored.bit((50, 39), 0, 0));
        assert!(explored.holds((50, 39)) && !explored.holds((0, 0)));
        // Off the world, nothing.
        explored.stamp((-100, 5), Stamp::Day);
        assert!(!explored.explored((-100, 5)));
    }
}
