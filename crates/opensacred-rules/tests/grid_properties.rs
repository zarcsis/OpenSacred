//! Properties of the backpack's grid on random sequences of its operations (#1430): the grid stays
//! whole, its first fit is the first in its order, the sort keeps the items, and a held item's cell
//! agrees with its offset. None goes beyond the original's (docs/re/inventory.md): no stability
//! among items of one area, and no order of a sort sorted again.

use std::collections::BTreeSet;

use opensacred_rules::inventory::{
    CELL, COLUMNS, Grid, Placed, Put, ROWS, SORT_ORDER, by_area, held_at, held_offset, size,
};
use proptest::prelude::*;

/// The sizes that `size` gives an item: of its record's bytes, or of its category.
fn item_size() -> impl Strategy<Value = (u8, u8)> {
    (any::<u32>(), (0..=3_u8, 0..=4_u8), any::<u8>())
        .prop_map(|(item, record, category)| size(item, [record.0, record.1], category))
}

/// A cell where a held item may be put: in the grid, or a little off it.
fn put_at() -> impl Strategy<Value = (i32, i32)> {
    (-2..i32::from(COLUMNS) + 1, -2..i32::from(ROWS) + 1)
}

/// An operation on the grid.
#[derive(Clone, Debug)]
enum Op {
    /// The pick-up's place at the first fit.
    Take((u8, u8)),
    /// A place at a cell.
    Place((u8, u8), (u8, u8)),
    /// A held item's put, which may swap.
    Put((i32, i32), (u8, u8)),
    /// The removal of the item at an index of those in the grid.
    Remove(prop::sample::Index),
    /// The sort, by rows or by columns, of the items' categories by a seed.
    Sort(bool, u8),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => item_size().prop_map(Op::Take),
        2 => ((0..COLUMNS, 0..ROWS), item_size()).prop_map(|(at, size)| Op::Place(at, size)),
        3 => (put_at(), item_size()).prop_map(|(at, size)| Op::Put(at, size)),
        2 => any::<prop::sample::Index>().prop_map(Op::Remove),
        1 => (any::<bool>(), any::<u8>()).prop_map(|(rows, seed)| Op::Sort(rows, seed)),
    ]
}

/// The category of the item `key` by the seed `seed`: one that the sort names, or another.
fn category(key: u32, seed: u8) -> u8 {
    let pick = (key.wrapping_mul(2_654_435_761) ^ u32::from(seed)) % 40;
    usize::try_from(pick)
        .ok()
        .and_then(|i| SORT_ORDER.get(i).copied())
        .unwrap_or(40)
}

/// Whether the grid is whole: each item inside it and over no other.
fn whole(grid: &Grid) -> Result<(), String> {
    for (i, a) in grid.items.iter().enumerate() {
        let inside = u16::from(a.at.0) + u16::from(a.size.0) <= u16::from(grid.width)
            && u16::from(a.at.1) + u16::from(a.size.1) <= u16::from(grid.height);
        if !inside {
            return Err(format!("{a:?} out of the grid"));
        }
        for b in &grid.items[i + 1..] {
            let apart = a.at.0 + a.size.0 <= b.at.0
                || b.at.0 + b.size.0 <= a.at.0
                || a.at.1 + a.size.1 <= b.at.1
                || b.at.1 + b.size.1 <= a.at.1;
            if !apart {
                return Err(format!("{a:?} over {b:?}"));
            }
        }
    }
    Ok(())
}

/// The keys in the grid.
fn keys(grid: &Grid) -> BTreeSet<u32> {
    grid.items.iter().map(|item| item.key).collect()
}

/// The grid after the operations `ops`, each new item of a key of its own, and the keys it should
/// hold: those put less those taken out.
fn run(ops: &[Op]) -> Result<(Grid, BTreeSet<u32>), String> {
    let mut grid = Grid::default();
    let mut held = BTreeSet::new();
    for (key, op) in (0..).zip(ops) {
        match *op {
            Op::Take(size) => {
                if grid.take(key, size).is_some() {
                    held.insert(key);
                }
            }
            Op::Place(at, size) => {
                if grid.place(key, at, size) {
                    held.insert(key);
                }
            }
            Op::Put(at, size) => match grid.put(key, at, size) {
                Put::Placed => {
                    held.insert(key);
                }
                Put::Swapped(other) => {
                    held.insert(key);
                    held.remove(&other);
                }
                Put::Refused => {}
            },
            Op::Remove(pick) => {
                if !grid.items.is_empty() {
                    let key = grid.items[pick.index(grid.items.len())].key;
                    if grid.remove(key).is_some() {
                        held.remove(&key);
                    }
                }
            }
            Op::Sort(by_rows, seed) => {
                if let Some(sorted) = grid.sorted(|key| category(key, seed), by_rows) {
                    grid = sorted;
                }
            }
        }
        whole(&grid).map_err(|why| format!("after {op:?}: {why}"))?;
    }
    Ok((grid, held))
}

/// The cells of the grid in the order of the first fit: by columns from the left, each from the
/// top, or by rows where `by_rows`, each from the left.
fn cells(by_rows: bool) -> Vec<(u8, u8)> {
    if by_rows {
        (0..ROWS)
            .flat_map(|row| (0..COLUMNS).map(move |column| (column, row)))
            .collect()
    } else {
        (0..COLUMNS)
            .flat_map(|column| (0..ROWS).map(move |row| (column, row)))
            .collect()
    }
}

proptest! {
    #[test]
    fn the_grid_stays_whole_and_holds_the_items_put_less_those_taken(
        ops in prop::collection::vec(op(), 0..80),
    ) {
        let (grid, held) = run(&ops).map_err(TestCaseError::fail)?;
        prop_assert_eq!(keys(&grid), held);
        prop_assert_eq!(grid.items.len(), keys(&grid).len());
    }

    #[test]
    fn the_first_fit_is_the_first_cell_in_its_order_that_fits(
        ops in prop::collection::vec(op(), 0..60),
        size in item_size(),
        by_rows in any::<bool>(),
    ) {
        let (grid, _) = run(&ops).map_err(TestCaseError::fail)?;
        let first = cells(by_rows).into_iter().find(|&at| grid.fits(at, size));
        prop_assert_eq!(grid.first_fit_by(size, by_rows), first);
        if !by_rows {
            prop_assert_eq!(grid.first_fit(size), first);
        }
    }

    #[test]
    fn an_item_fits_where_its_cells_lie_in_the_grid_and_no_item_covers_them(
        ops in prop::collection::vec(op(), 0..60),
        size in item_size(),
    ) {
        let (grid, _) = run(&ops).map_err(TestCaseError::fail)?;
        // Each cell's item, of the items' rectangles.
        let span = |from: u8, cells: u8| u16::from(from)..u16::from(from) + u16::from(cells);
        let owner = |(column, row): (u16, u16)| {
            grid.items.iter().find(|item| {
                span(item.at.0, item.size.0).contains(&column)
                    && span(item.at.1, item.size.1).contains(&row)
            })
        };
        for at in cells(false) {
            let cell = (u16::from(at.0), u16::from(at.1));
            prop_assert_eq!(grid.at(at).map(|item| item.key), owner(cell).map(|item| item.key));
            let fits = span(at.0, size.0).all(|column| {
                span(at.1, size.1).all(|row| {
                    column < u16::from(COLUMNS)
                        && row < u16::from(ROWS)
                        && owner((column, row)).is_none()
                })
            });
            prop_assert_eq!(grid.fits(at, size), fits, "{:?} at {:?}", size, at);
        }
    }

    #[test]
    fn the_sort_by_area_orders_its_items_from_the_smallest_up(
        sizes in prop::collection::vec(item_size(), 0..30),
    ) {
        let mut items: Vec<Placed> = (0..)
            .zip(sizes)
            .map(|(key, size)| Placed { key, at: (0, 0), size })
            .collect();
        let before: BTreeSet<u32> = items.iter().map(|item| item.key).collect();
        by_area(&mut items);
        let after: BTreeSet<u32> = items.iter().map(|item| item.key).collect();
        prop_assert_eq!(items.len(), before.len());
        prop_assert_eq!(after, before);
        let area = |item: &Placed| u16::from(item.size.0) * u16::from(item.size.1);
        prop_assert!(items.windows(2).all(|pair| area(&pair[0]) <= area(&pair[1])));
    }

    #[test]
    fn the_grid_sorted_keeps_its_items_whole(
        ops in prop::collection::vec(op(), 0..60),
        by_rows in any::<bool>(),
        seed in any::<u8>(),
    ) {
        let (grid, _) = run(&ops).map_err(TestCaseError::fail)?;
        if let Some(sorted) = grid.sorted(|key| category(key, seed), by_rows) {
            whole(&sorted).map_err(TestCaseError::fail)?;
            let sizes = |grid: &Grid| -> BTreeSet<(u32, (u8, u8))> {
                grid.items.iter().map(|item| (item.key, item.size)).collect()
            };
            prop_assert_eq!(sizes(&sorted), sizes(&grid));
            prop_assert_eq!((sorted.width, sorted.height), (grid.width, grid.height));
        }
    }

    #[test]
    fn a_held_item_lies_at_the_cell_under_the_cursor_less_its_offset(
        point in (-64.0..i32::from(COLUMNS) as f32 * CELL + 64.0, -64.0..i32::from(ROWS) as f32 * CELL + 64.0),
        size in (1..=4_u8, 1..=4_u8),
    ) {
        let (dx, dy) = held_offset(size);
        let corner = (point.0 - dx, point.1 - dy);
        let (column, row) = held_at(point, size);
        // The item's top left corner lies in the cell it is held at.
        prop_assert!((column as f32) * CELL <= corner.0 && corner.0 < (column + 1) as f32 * CELL);
        prop_assert!((row as f32) * CELL <= corner.1 && corner.1 < (row + 1) as f32 * CELL);
    }
}
