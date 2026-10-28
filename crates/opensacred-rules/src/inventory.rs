//! The hero's backpack as the original's grid (#1268; docs/re/inventory.md, §3): every hero's of
//! 20 columns by 8 rows, an item's size in cells (`0x428D60`), the fit of an item at a cell
//! (`0x5DACB0`), the search for the first fit column by column (`0x5DABA0`'s mode 0), the place
//! and the removal (`0x5D9F00`, `0x5DA630`), and the swap's test (`0x5DB0D0`); the cursor's item
//! (#1269; §3 "The clicks on the items' page" and §6): its offset from the cursor, the cell it is
//! held over, the cells' highlight under it and its put.

/// Every hero's grid: 20 columns by 8 rows (`0x5DF570`, `0x5D7A50`).
pub const COLUMNS: u8 = 20;
pub const ROWS: u8 = 8;
/// A cell's side, in pixels of the original's screen.
pub const CELL: f32 = 32.0;

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

/// The offset of a held item's top left from the cursor, in pixels, along each side of `size`
/// (`0x655810`, `0x655870`): 8 · n for an even number of cells, 16 · n − 16 for an odd one.
pub fn held_offset(size: (u8, u8)) -> (f32, f32) {
    let along = |cells: u8| {
        if cells.is_multiple_of(2) {
            8.0 * f32::from(cells)
        } else {
            16.0 * f32::from(cells) - 16.0
        }
    };
    (along(size.0), along(size.1))
}

/// The cell, column and row, of the top left of an item of `size` held with the cursor at `point`,
/// in pixels from the grid's top left: the cell under the cursor less the item's offset, which
/// may lie outside the grid.
pub fn held_at(point: (f32, f32), size: (u8, u8)) -> (i32, i32) {
    let (dx, dy) = held_offset(size);
    let cell = |at: f32| {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a whole number of cells held within a thousand"
        )]
        let cell = (at / CELL).floor().clamp(-1000.0, 1000.0) as i32;
        cell
    };
    (cell(point.0 - dx), cell(point.1 - dy))
}

/// How the cells under a held item are tinted (`0x5DC430`): green where it fits, purple where it
/// would swap with the one item under it, red where it would not go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Highlight {
    Fits,
    Swaps,
    Refused,
}

/// What a held item's put did (`0x6C0150`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Put {
    /// It went into the grid; the cursor is empty.
    Placed,
    /// It went into the grid in place of the item `key`, which goes onto the cursor.
    Swapped(u32),
    /// Nothing moved.
    Refused,
}

/// The eight cells about a cell, by the direction toward them (`0x95DCB0`): from −x, turning
/// toward −y.
const AROUND: [(i32, i32); 8] = [
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
];

/// The cell where an item dropped off the cursor by a click toward the cell `toward` lands, the
/// hero standing on the cell `hero` (`cEngine::dropItem`, `0x611620`): of the eight about the
/// hero's, the one in the direction of the click, `(atan2(dy, dx) + π + π/8) · 4/π` cut to a
/// whole eighth, else the next to each side in turn (one further, one back, two further, …), the
/// first of eight tries that `walkable` takes; else the hero's own.
pub fn drop_cell(
    hero: (i32, i32),
    toward: (i32, i32),
    walkable: impl Fn((i32, i32)) -> bool,
) -> (i32, i32) {
    let (dx, dy) = (
        f64::from(toward.0.saturating_sub(hero.0)),
        f64::from(toward.1.saturating_sub(hero.1)),
    );
    let eighth = (dy.atan2(dx) + std::f64::consts::PI + std::f64::consts::FRAC_PI_8) * 4.0
        / std::f64::consts::PI;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "an angle's eighth from 0 to 8, cut as the original's `ftol` cuts it"
    )]
    let mut direction = eighth.clamp(0.0, 8.0) as usize;
    for turn in 1..=8usize {
        let (ax, ay) = AROUND[direction % 8];
        let cell = (hero.0 + ax, hero.1 + ay);
        if walkable(cell) {
            return cell;
        }
        direction = if turn % 2 == 1 {
            direction % 8 + turn
        } else {
            (direction % 8 + 8 - turn % 8) % 8
        };
    }
    hero
}

/// A grid of `width` × `height` cells and the items in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    pub width: u8,
    pub height: u8,
    pub items: Vec<Placed>,
}

/// The categories in the order that the sort puts their groups back (`0x5DB6B0`): the runes, 30
/// and 31, the rings, the amulets, the potions, the quest items, the keys, the books, the weapons,
/// the helmets, the boots, the belts, the armour, the shields, the shoulders, the arms, the legs,
/// the gloves, the wings, the bridles, the cannons, 0, then the rest; the table names 0 twice more
/// at its end.
pub const SORT_ORDER: [u8; 35] = [
    28, 30, 31, 8, 20, 9, 15, 14, 16, 5, 17, 18, 19, 6, 13, 21, 22, 23, 24, 25, 29, 33, 0, 10, 11,
    12, 27, 1, 26, 7, 2, 3, 4, 0, 0,
];

/// The sort's order of a group of items (`0x5DB6B0`, `0x5DB280`): each item against every other,
/// the two swapped where the first's area (w · h) is the smaller; the areas so come out from the
/// smallest up.
pub fn by_area(items: &mut [Placed]) {
    let area = |item: &Placed| u16::from(item.size.0) * u16::from(item.size.1);
    for i in 0..items.len() {
        for j in 0..items.len() {
            if i != j && area(&items[i]) < area(&items[j]) {
                items.swap(i, j);
            }
        }
    }
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

    /// The first cell where an item of `size` fits, by rows where `by_rows`, each from the left
    /// (`0x5DABA0`'s mode 1), else as [`Self::first_fit`].
    pub fn first_fit_by(&self, size: (u8, u8), by_rows: bool) -> Option<(u8, u8)> {
        if !by_rows {
            return self.first_fit(size);
        }
        (0..self.height)
            .flat_map(|row| (0..self.width).map(move |column| (column, row)))
            .find(|&at| self.fits(at, size))
    }

    /// The grid sorted (`cInventoryEntry::autoSortItems`, `0x5DB6B0`): every item taken out in the
    /// order of its first cell row by row, grouped by its category (`category`), each group in the
    /// sort's order of the areas ([`by_area`]), and the groups put back in the order of
    /// [`SORT_ORDER`], each item at its first fit by columns, or by rows where `by_rows`; where an
    /// item finds no place, every item in the sort's order of the areas, ungrouped (`0x5DB280`);
    /// none where that leaves one out too, the grid then as it was. A category the order names
    /// again puts nothing again.
    pub fn sorted(&self, category: impl Fn(u32) -> u8, by_rows: bool) -> Option<Grid> {
        let mut items = self.items.clone();
        items.sort_by_key(|item| (item.at.1, item.at.0));
        let place = |order: &[Placed]| {
            let mut grid = Grid::new(self.width, self.height);
            for item in order {
                let at = grid.first_fit_by(item.size, by_rows)?;
                grid.place(item.key, at, item.size);
            }
            Some(grid)
        };
        let mut grouped = Vec::with_capacity(items.len());
        let mut done = Vec::new();
        for &wanted in &SORT_ORDER {
            if done.contains(&wanted) {
                continue;
            }
            done.push(wanted);
            let mut group: Vec<Placed> = items
                .iter()
                .filter(|item| category(item.key) == wanted)
                .copied()
                .collect();
            by_area(&mut group);
            grouped.extend(group);
        }
        // The items of a category the order does not name, last.
        grouped.extend(
            items
                .iter()
                .filter(|item| !SORT_ORDER.contains(&category(item.key)))
                .copied(),
        );
        place(&grouped).or_else(|| {
            by_area(&mut items);
            place(&items)
        })
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

    /// The cell `at` of a held item, where it lies within the grid's range of cells.
    fn cell(at: (i32, i32)) -> Option<(u8, u8)> {
        Some((u8::try_from(at.0).ok()?, u8::try_from(at.1).ok()?))
    }

    /// The highlight under an item of `size` held over the cell `at`, which may lie outside the
    /// grid.
    pub fn highlight(&self, at: (i32, i32), size: (u8, u8)) -> Highlight {
        match Self::cell(at).map(|at| self.under(at, size)) {
            Some(Under::Free) => Highlight::Fits,
            Some(Under::One(_)) => Highlight::Swaps,
            Some(Under::Refused) | None => Highlight::Refused,
        }
    }

    /// Whether the cell `cell` lies under an item of `size` held over `at`.
    pub fn held_over(cell: (u8, u8), at: (i32, i32), size: (u8, u8)) -> bool {
        let (column, row) = (i32::from(cell.0), i32::from(cell.1));
        column >= at.0
            && column < at.0 + i32::from(size.0)
            && row >= at.1
            && row < at.1 + i32::from(size.1)
    }

    /// Puts the held item `key` of `size` at the cell `at` (`0x6C0150`): where it fits, there;
    /// over exactly one item, there in its place, the other taken out of the grid onto the cursor;
    /// else nothing.
    pub fn put(&mut self, key: u32, at: (i32, i32), size: (u8, u8)) -> Put {
        let Some(cell) = Self::cell(at) else {
            return Put::Refused;
        };
        match self.under(cell, size) {
            Under::Free => {
                self.items.push(Placed {
                    key,
                    at: cell,
                    size,
                });
                Put::Placed
            }
            Under::One(other) => {
                self.remove(other);
                self.items.push(Placed {
                    key,
                    at: cell,
                    size,
                });
                Put::Swapped(other)
            }
            Under::Refused => Put::Refused,
        }
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
    fn a_dropped_item_lands_beside_the_hero_toward_the_click() {
        let anywhere = |_| true;
        assert_eq!(drop_cell((10, 10), (20, 10), anywhere), (11, 10));
        assert_eq!(drop_cell((10, 10), (0, 10), anywhere), (9, 10));
        assert_eq!(drop_cell((10, 10), (10, 20), anywhere), (10, 11));
        assert_eq!(drop_cell((10, 10), (20, 21), anywhere), (11, 11));
        // A click on the hero's own cell: the angle 0, toward +x.
        assert_eq!(drop_cell((10, 10), (10, 10), anywhere), (11, 10));
        // Toward +x blocked: one further (+x +y), then one back (+x −y).
        assert_eq!(drop_cell((10, 10), (20, 10), |c| c != (11, 10)), (11, 11));
        assert_eq!(
            drop_cell((10, 10), (20, 10), |c| c != (11, 10) && c != (11, 11)),
            (11, 9)
        );
        // Nowhere to go: the hero's own cell.
        assert_eq!(drop_cell((10, 10), (20, 10), |_| false), (10, 10));
    }

    #[test]
    fn a_held_item_lies_off_the_cursor_by_its_size() {
        assert_eq!(held_offset((1, 1)), (0.0, 0.0));
        assert_eq!(held_offset((2, 3)), (16.0, 32.0));
        assert_eq!(held_offset((1, 4)), (0.0, 32.0));
        // A potion under the cursor's cell; a 2 × 2 item's top left 16 pixels up and left.
        assert_eq!(held_at((40.0, 70.0), (1, 1)), (1, 2));
        assert_eq!(held_at((40.0, 70.0), (2, 2)), (0, 1));
        assert_eq!(held_at((10.0, 10.0), (2, 2)), (-1, -1));
    }

    #[test]
    fn a_put_places_swaps_or_refuses_and_the_cells_show_which() {
        let mut grid = Grid::default();
        grid.place(1, (0, 0), (1, 1));
        grid.place(2, (0, 1), (1, 1));
        assert_eq!(grid.highlight((5, 5), (2, 2)), Highlight::Fits);
        assert_eq!(grid.highlight((0, 1), (1, 2)), Highlight::Swaps);
        assert_eq!(grid.highlight((0, 0), (1, 2)), Highlight::Refused);
        assert_eq!(grid.highlight((-1, 0), (1, 1)), Highlight::Refused);
        assert_eq!(grid.highlight((19, 7), (2, 1)), Highlight::Refused);
        assert!(Grid::held_over((1, 2), (0, 1), (2, 2)));
        assert!(!Grid::held_over((2, 2), (0, 1), (2, 2)));
        assert_eq!(grid.put(3, (0, 0), (1, 2)), Put::Refused);
        assert_eq!(grid.put(3, (0, 1), (1, 2)), Put::Swapped(2));
        assert_eq!(grid.at((0, 2)).map(|item| item.key), Some(3));
        assert_eq!(grid.put(2, (4, 4), (1, 1)), Put::Placed);
        assert_eq!(grid.put(4, (-1, 4), (1, 1)), Put::Refused);
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

    #[test]
    fn the_sort_groups_by_category_and_orders_the_areas_from_the_smallest() {
        let placed = |key, at, size| Placed { key, at, size };
        // A ring 1, a sword 2, a potion 3, an armour 4, a potion 5 and a bigger sword 6.
        let mut grid = Grid::new(4, 6);
        for item in [
            placed(1, (3, 5), (1, 1)),
            placed(2, (0, 0), (1, 3)),
            placed(3, (1, 0), (1, 1)),
            placed(4, (2, 0), (2, 3)),
            placed(5, (1, 1), (1, 1)),
            placed(6, (0, 3), (1, 3)),
        ] {
            assert!(grid.place(item.key, item.at, item.size));
        }
        let category = |key| match key {
            1 => 8,
            2 | 6 => 5,
            3 | 5 => 9,
            _ => 6,
        };
        // The ring, the potions, the swords, the armour, each at its first fit by columns.
        let sorted = grid.sorted(category, false).unwrap();
        let at = |key| {
            sorted
                .items
                .iter()
                .find(|item| item.key == key)
                .map(|item| item.at)
        };
        assert_eq!(
            [1, 3, 5, 2, 6, 4].map(at),
            [
                Some((0, 0)),
                Some((0, 1)),
                Some((0, 2)),
                Some((0, 3)),
                Some((1, 0)),
                Some((1, 3))
            ]
        );
        // By rows.
        let sorted = grid.sorted(category, true).unwrap();
        let at = |key| {
            sorted
                .items
                .iter()
                .find(|item| item.key == key)
                .map(|item| item.at)
        };
        assert_eq!(
            [1, 3, 5].map(at),
            [Some((0, 0)), Some((1, 0)), Some((2, 0))]
        );
    }

    #[test]
    fn the_sorts_order_compares_every_item_with_every_other() {
        let placed = |key, h| Placed {
            key,
            at: (0, 0),
            size: (1, h),
        };
        let mut items = [placed(1, 3), placed(2, 1), placed(3, 2), placed(4, 1)];
        by_area(&mut items);
        assert_eq!(items.map(|item| item.key), [2, 4, 3, 1]);
    }

    #[test]
    fn a_sort_that_leaves_an_item_out_tries_ungrouped_and_else_keeps_the_grid() {
        let placed = |key, at, size| Placed { key, at, size };
        // A 2 × 2 grid of a 1 × 2 sword, a ring and a potion.
        let mut grid = Grid::new(2, 2);
        for item in [
            placed(1, (0, 0), (1, 2)),
            placed(2, (1, 0), (1, 1)),
            placed(3, (1, 1), (1, 1)),
        ] {
            assert!(grid.place(item.key, item.at, item.size));
        }
        let category = |key| match key {
            1 => 5,
            2 => 8,
            _ => 9,
        };
        // By columns the ring and the potion fill the first column and the sword the second.
        let sorted = grid.sorted(category, false).unwrap();
        let at = |key| {
            sorted
                .items
                .iter()
                .find(|item| item.key == key)
                .map(|item| item.at)
        };
        assert_eq!(
            [2, 3, 1].map(at),
            [Some((0, 0)), Some((0, 1)), Some((1, 0))]
        );
        // By rows the small ones take the first row, grouped or not, and the sword no place: none,
        // the grid kept by its caller.
        assert_eq!(grid.sorted(category, true), None);
    }
}
