//! The hero's backpack as the original's grid (#1268; docs/re/inventory.md, §3): every hero's of
//! 20 columns by 8 rows, an item's size in cells (`0x428D60`), the fit of an item at a cell
//! (`0x5DACB0`), the search for the first fit column by column (`0x5DABA0`'s mode 0), the place
//! and the removal (`0x5D9F00`, `0x5DA630`), and the swap's test (`0x5DB0D0`).

/// Every hero's grid: 20 columns by 8 rows (`0x5DF570`, `0x5D7A50`).
pub const COLUMNS: u8 = 20;
pub const ROWS: u8 = 8;

/// The Seraphim's hair of her template, which the grid takes as 2 × 2 whatever its record.
const HAIR: u32 = 4007;
/// The weapons that are 1 × 4 rather than 1 × 3 where their record gives no size.
const LONG: [u32; 6] = [1715, 1716, 1717, 1736, 1763, 1766];

/// The size in cells, columns and rows, of an item of the type `item`, of the `Weapon.pak` record's
/// bytes `+0x1C` and `+0x1D` `record`, and of the category `category` (`items.pak +0x2E`;
/// `0x428D60`): the type 4007 2 × 2; else the record's where both are above 0; else by the category,
/// the weapons 1 × 3 (some 1 × 4), the armour and the shields 2 × 3, the books 1 × 2, the helmets,
/// the boots, the shoulders, the arms and the greaves 2 × 2, the belts and the gloves 2 × 1, any
/// other 1 × 1.
pub fn size(item: u32, record: [u8; 2], category: u8) -> (u8, u8) {
    if item == HAIR {
        return (2, 2);
    }
    if record[0] > 0 && record[1] > 0 {
        return (record[0], record[1]);
    }
    match category {
        5 if LONG.contains(&item) => (1, 4),
        5 => (1, 3),
        6 | 13 => (2, 3),
        16 => (1, 2),
        17 | 18 | 21 | 22 | 23 => (2, 2),
        19 | 24 => (2, 1),
        _ => (1, 1),
    }
}

/// An item in the grid: its key in the hero's pack, its top left cell and its size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    pub key: u32,
    pub at: (u8, u8),
    pub size: (u8, u8),
}

impl Placed {
    /// Whether it covers the cell `(column, row)`.
    fn covers(&self, (column, row): (u8, u8)) -> bool {
        column >= self.at.0
            && column < self.at.0 + self.size.0
            && row >= self.at.1
            && row < self.at.1 + self.size.1
    }

    /// Whether it overlaps the rectangle of `size` from `at`.
    fn overlaps(&self, at: (u8, u8), size: (u8, u8)) -> bool {
        at.0 < self.at.0 + self.size.0
            && self.at.0 < at.0 + size.0
            && at.1 < self.at.1 + self.size.1
            && self.at.1 < at.1 + size.1
    }
}

/// What lies under an item held at a cell (`0x5DB0D0`): no item, where it may be put; exactly one,
/// with which it swaps; or more, where it may not go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Under {
    Free,
    One(u32),
    Refused,
}

/// A grid of `width` × `height` cells and the items in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    pub width: u8,
    pub height: u8,
    pub items: Vec<Placed>,
}

impl Default for Grid {
    fn default() -> Self {
        Self::new(COLUMNS, ROWS)
    }
}

impl Grid {
    /// An empty grid of `width` × `height`.
    pub fn new(width: u8, height: u8) -> Self {
        Self {
            width,
            height,
            items: Vec::new(),
        }
    }

    /// Whether the rectangle of `size` from `at` lies in the grid.
    fn inside(&self, at: (u8, u8), size: (u8, u8)) -> bool {
        size.0 > 0
            && size.1 > 0
            && u16::from(at.0) + u16::from(size.0) <= u16::from(self.width)
            && u16::from(at.1) + u16::from(size.1) <= u16::from(self.height)
    }

    /// Whether an item of `size` fits at `at`: inside the grid, over no item (`0x5DACB0`).
    pub fn fits(&self, at: (u8, u8), size: (u8, u8)) -> bool {
        self.inside(at, size) && !self.items.iter().any(|item| item.overlaps(at, size))
    }

    /// The first cell where an item of `size` fits, column by column from the left, each from the
    /// top (`0x5DABA0`'s mode 0, the pick-up's).
    pub fn first_fit(&self, size: (u8, u8)) -> Option<(u8, u8)> {
        (0..self.width)
            .flat_map(|column| (0..self.height).map(move |row| (column, row)))
            .find(|&at| self.fits(at, size))
    }

    /// Places the item `key` of `size` at `at` where it fits (`0x5D9F00`): whether it did.
    pub fn place(&mut self, key: u32, at: (u8, u8), size: (u8, u8)) -> bool {
        if !self.fits(at, size) {
            return false;
        }
        self.items.push(Placed { key, at, size });
        true
    }

    /// Places the item `key` of `size` at its first fit: where, none without room.
    pub fn take(&mut self, key: u32, size: (u8, u8)) -> Option<(u8, u8)> {
        let at = self.first_fit(size)?;
        self.place(key, at, size);
        Some(at)
    }

    /// Takes the item `key` out (`0x5DA630`): where it was.
    pub fn remove(&mut self, key: u32) -> Option<Placed> {
        let index = self.items.iter().position(|item| item.key == key)?;
        Some(self.items.remove(index))
    }

    /// The item over the cell `cell`.
    pub fn at(&self, cell: (u8, u8)) -> Option<&Placed> {
        self.items.iter().find(|item| item.covers(cell))
    }

    /// What lies under an item of `size` held at `at` (`0x5DB0D0`): refused where it runs out of
    /// the grid.
    pub fn under(&self, at: (u8, u8), size: (u8, u8)) -> Under {
        if !self.inside(at, size) {
            return Under::Refused;
        }
        let mut over = self.items.iter().filter(|item| item.overlaps(at, size));
        match (over.next(), over.next()) {
            (None, _) => Under::Free,
            (Some(item), None) => Under::One(item.key),
            (Some(_), Some(_)) => Under::Refused,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_item_takes_its_records_size_else_its_categorys() {
        assert_eq!(size(4007, [1, 1], 15), (2, 2));
        assert_eq!(size(1712, [1, 3], 5), (1, 3));
        assert_eq!(size(1712, [0, 3], 5), (1, 3));
        assert_eq!(size(1715, [0, 0], 5), (1, 4));
        assert_eq!(size(4054, [0, 0], 19), (2, 1));
        assert_eq!(size(4600, [0, 0], 6), (2, 3));
        assert_eq!(size(5171, [0, 0], 9), (1, 1));
        assert_eq!(size(4098, [0, 0], 16), (1, 2));
    }

    #[test]
    fn the_pick_up_fills_the_grid_column_by_column() {
        let mut grid = Grid::default();
        // Three potions down the first column, then a sword beside them.
        for (key, row) in [(0, 0), (1, 1), (2, 2)] {
            assert_eq!(grid.take(key, (1, 1)), Some((0, row)));
        }
        assert_eq!(grid.take(3, (1, 3)), Some((0, 3)));
        assert_eq!(grid.take(4, (1, 3)), Some((1, 0)));
        // Below the second sword, beside the first.
        assert_eq!(grid.take(5, (2, 3)), Some((1, 3)));
        // A full grid takes nothing more.
        let mut full = Grid::new(2, 2);
        assert!(full.place(9, (0, 0), (2, 2)));
        assert_eq!(full.take(10, (1, 1)), None);
        assert!(!full.fits((1, 1), (1, 1)));
        assert!(!full.fits((1, 0), (2, 1)));
    }

    #[test]
    fn a_held_item_goes_free_swaps_with_one_or_is_refused() {
        let mut grid = Grid::default();
        grid.place(1, (0, 0), (1, 1));
        grid.place(2, (1, 0), (1, 3));
        assert_eq!(grid.under((5, 5), (2, 2)), Under::Free);
        assert_eq!(grid.under((1, 1), (2, 2)), Under::One(2));
        assert_eq!(grid.under((0, 0), (2, 1)), Under::Refused);
        assert_eq!(grid.under((19, 7), (2, 1)), Under::Refused);
        assert_eq!(grid.at((1, 2)).map(|item| item.key), Some(2));
        assert_eq!(grid.remove(2).map(|item| item.at), Some((1, 0)));
        assert_eq!(grid.under((1, 1), (2, 2)), Under::Free);
    }
}
