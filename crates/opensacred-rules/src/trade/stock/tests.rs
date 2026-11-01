use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};

use super::*;
use crate::drops::Kind;
use crate::spawning::Script;

/// A supply of fixed categories, weapon kinds and sizes, whose choice answers the types it is
/// given in turn and logs what it was asked, and whose maker rolls nothing.
#[derive(Default)]
struct Fake {
    categories: HashMap<u32, u8>,
    kinds: HashMap<u32, u8>,
    answers: RefCell<VecDeque<Option<u32>>>,
    asked: RefCell<Vec<Wanted>>,
}

impl Fake {
    fn of(types: &[(u32, u8)]) -> Self {
        Self {
            categories: types.iter().copied().collect(),
            ..Self::default()
        }
    }

    fn answering(self, answers: &[Option<u32>]) -> Self {
        Self {
            answers: RefCell::new(answers.iter().copied().collect()),
            ..self
        }
    }
}

impl Supply for Fake {
    fn category(&self, item: u32) -> Option<u8> {
        self.categories.get(&item).copied()
    }

    fn weapon_kind(&self, item: u32) -> u8 {
        self.kinds.get(&item).copied().unwrap_or(0)
    }

    fn size(&self, item: u32) -> (u8, u8) {
        if self.category(item) == Some(POTIONS) {
            (1, 1)
        } else {
            (1, 3)
        }
    }

    fn choose(&self, wanted: Wanted, _hero: &Hero, _dice: &mut impl Dice) -> Option<u32> {
        self.asked.borrow_mut().push(wanted);
        self.answers.borrow_mut().pop_front().flatten()
    }

    fn make(&self, item: u32, level: u32, key: u8, _dice: &mut impl Dice) -> Option<Dropped> {
        self.categories.contains_key(&item).then_some(Dropped {
            kind: Kind::Other,
            item,
            level,
            key: 16 - key,
            quality: key,
            amount: 1,
            values: None,
            named: None,
            socketed: Vec::new(),
        })
    }
}

fn hero(creature: u32, level: u32, skills: &[(u8, u32)]) -> Hero {
    Hero {
        creature,
        level,
        skills: skills.to_vec(),
        ..Hero::default()
    }
}

/// The `rand()` values that the dice answer, and a check that every one was taken.
fn rolls(values: &[u32]) -> Script {
    Script::new(values.to_vec())
}

/// The `rand()` value whose roll `rand() · u` times `scale` is `n` and a little more.
fn value_of(n: u32, scale: u32) -> u32 {
    (n * 32_767).div_ceil(scale)
}

#[test]
fn a_fills_tries_grow_with_the_merchants_level() {
    // ⌊10 + r·(⌊10 + 10·r'⌋ + 10·t)⌋: at most 28 below level 20, 78 from level 100.
    let tries = |level, values: &[u32]| rolled_tries(level, &mut rolls(values));
    assert_eq!(tries(2, &[0, 0]), 10);
    assert_eq!(tries(2, &[32_767, 32_767]), 28);
    assert_eq!(tries(45, &[16_384, 32_767]), 44);
    assert_eq!(tries(250, &[32_767, 32_767]), 78);
}

#[test]
fn the_trading_skill_raises_the_quality_keys() {
    let keys = |level, skill| {
        let hero = hero(1, level, &[(TRADING, skill)]);
        Buyer {
            hero: &hero,
            difficulty: Difficulty::Bronze,
            weapons: &[],
        }
        .keys()
    };
    // Untrained 0 to 2; x = 10.5·s/(L + 1).
    assert_eq!(keys(1, 0), (0, 2));
    assert_eq!(keys(1, 1), (1, 7));
    assert_eq!(keys(1, 5), (7, 12));
    assert_eq!(keys(20, 1), (0, 2));
    assert_eq!(keys(10, 12), (3, 12));
}

#[test]
fn a_page_weighs_its_categories_and_takes_the_sold_items_of_its_own() {
    let pick = |page: Page, value| page.pick(&mut rolls(&[value]));
    // The rings 50, the books 30, the runes 50, the amulets 70, the potions 80 three times, the
    // rings 50 and the category 30 20: 510 in all.
    assert_eq!(pick(Page::Various, 0), Some(RINGS));
    assert_eq!(pick(Page::Various, value_of(150, 510)), Some(20));
    assert_eq!(pick(Page::Various, 32_767), Some(30));
    assert_eq!(pick(Page::Weapons, 32_767), Some(WEAPONS));
    assert_eq!(Page::of(5), Some(Page::Weapons));
    assert_eq!(Page::of(33), Some(Page::Armour));
    assert_eq!(Page::of(7), Some(Page::Various));
    assert_eq!(Page::of(15), Some(Page::Various));
    assert_eq!(Page::of(26), None);
}

#[test]
fn a_fill_makes_the_types_of_the_drops_choice_at_the_heros_levels() {
    let supply = Fake::of(&[(1713, 5), (1758, 5)]).answering(&[Some(1713), Some(1758)]);
    let hero = hero(2, 9, &[(4, 2)]);
    let lists = [(3, vec![1758]), (4, vec![1713])];
    let buyer = Buyer {
        hero: &hero,
        difficulty: Difficulty::Bronze,
        weapons: &lists,
    };
    let mut shelf = Shelf::new(Page::Weapons);
    let mut dice = rolls(&[
        // The tries' rolls, taken though the tries are given.
        0, 0,
        // The weapons: the skill 3 at 0 taken in one case of two, the skill 4 at 2 after it; its
        // one weapon of the key 2, its fourth argument's roll, its level 9, rolled again.
        7, 1, 2, 1, 0, 1, 1,
        // A try: the weapons, the key 0, the class, the level 17; the type held already.
        5, 3, 0, 32_767,
        // A try: the weapons, the key 1, any class, the level 9; three rolls of the fourth
        // argument, the level 8, rolled again to 9.
        5, 4, 1, 0, 0, 0, 3, 1, 0, 0,
    ]);
    shelf.fill(2, Some(2), &buyer, &supply, &mut dice);
    assert_eq!(dice.taken(), 23);
    let made: Vec<_> = shelf
        .items
        .iter()
        .map(|(_, d)| (d.item, d.level, d.quality))
        .collect();
    assert_eq!(made, [(1713, 9, 2), (1758, 9, 1)]);
    let asked: Vec<_> = supply
        .asked
        .borrow()
        .iter()
        .map(|w| (w.category, w.level, w.class, w.quality, w.loose))
        .collect();
    assert_eq!(asked, [(5, 17, 2, 0, false), (5, 9, 0, 1, false)]);
    // Each item at its first fit, column by column.
    assert_eq!(shelf.grid.items[0].at, (0, 0));
    assert_eq!(shelf.grid.items[1].at, (0, 3));
}

#[test]
fn five_items_not_made_end_a_fill() {
    // No type found at all: each try a miss, the sixth not tried.
    let supply = Fake::of(&[]);
    let hero = hero(1, 1, &[]);
    let buyer = Buyer {
        hero: &hero,
        difficulty: Difficulty::Bronze,
        weapons: &[],
    };
    let mut shelf = Shelf::new(Page::Armour);
    let mut dice = rolls(&[0; 23]);
    shelf.fill(2, Some(10), &buyer, &supply, &mut dice);
    // The tries' two rolls, the first category's, then four rolls in each of five tries.
    assert_eq!(dice.taken(), 23);
    assert_eq!(supply.asked.borrow().len(), 5);
    assert!(shelf.items.is_empty());
}

#[test]
fn the_dwarfs_ranged_combat_offers_its_guns_alone() {
    let mut supply = Fake::of(&[(1786, 5), (1732, 5)]);
    supply.kinds = [(1786, 12), (1732, 14)].into_iter().collect();
    let hero = hero(DWARF, 5, &[(7, 3)]);
    let lists = [(7, vec![1786, 1732])];
    let buyer = Buyer {
        hero: &hero,
        difficulty: Difficulty::Bronze,
        weapons: &lists,
    };
    let mut shelf = Shelf::new(Page::Weapons);
    // The crossbow drawn first and drawn out; the gun, the last, of the key 0, its fourth
    // argument's roll and its level 5, rolled twice.
    let mut dice = rolls(&[0, 0, 3, 0, 0]);
    shelf.best_weapon(&buyer, buyer.keys(), &supply, &mut dice);
    assert_eq!(dice.taken(), 5);
    assert_eq!(shelf.items.len(), 1);
    assert_eq!(shelf.items[0].1.item, 1732);
}

#[test]
fn a_thinning_empties_the_cells_it_rolls() {
    let supply = Fake::of(&[(1713, 5), (1758, 5)]);
    let made = |item| supply.make(item, 1, 0, &mut rolls(&[])).unwrap();
    let mut shelf = Shelf::new(Page::Weapons);
    shelf.put(made(1713), (1, 3));
    let key = shelf.put(made(1758), (2, 2)).unwrap();
    shelf.grid.remove(key);
    shelf.grid.place(key, (5, 5), (2, 2));
    // Four tries: the cell (0, 2) of the first, (6, 6) of the second, two empty cells.
    let mut dice = rolls(&[
        32_767,
        0,
        value_of(2, 9),
        value_of(6, 16),
        value_of(6, 9),
        value_of(10, 16),
        0,
        0,
        value_of(8, 9),
    ]);
    assert_eq!(shelf.thin(&mut dice), 2);
    assert_eq!(dice.taken(), 9);
    assert!(shelf.items.is_empty() && shelf.grid.items.is_empty());
}

#[test]
fn a_page_of_potions_is_topped_up_below_eight() {
    let supply = Fake::of(&[(5168, 9), (5170, 9), (5171, 9), (5172, 9)]);
    let hero = hero(1, 1, &[]);
    let buyer = Buyer {
        hero: &hero,
        difficulty: Difficulty::Bronze,
        weapons: &[],
    };
    let mut shelf = Shelf::new(Page::Various);
    // Four potions: of the rows 2 to 4 where 1,000 rolls above 600, else healing.
    let mut dice = rolls(&[0, 700, 0, 0, 1700, 1, 999, 2]);
    shelf.top_up(&buyer, &supply, &mut dice);
    assert_eq!(dice.taken(), 8);
    let items: Vec<u32> = shelf.items.iter().map(|(_, d)| d.item).collect();
    assert_eq!(items, [5168, 5171, 5170, 5172]);
    // Eight or more: nothing rolled; the weapons' page never.
    for _ in 0..4 {
        shelf.put(supply.make(5171, 0, 0, &mut rolls(&[])).unwrap(), (1, 1));
    }
    let mut none = rolls(&[]);
    shelf.top_up(&buyer, &supply, &mut none);
    Shelf::new(Page::Weapons).top_up(&buyer, &supply, &mut none);
    assert_eq!(none.taken(), 0);
}

#[test]
fn the_same_merchant_again_keeps_the_stock_in_one_case_of_two() {
    let supply = Fake::of(&[]);
    let hero = hero(1, 1, &[]);
    let buyer = Buyer {
        hero: &hero,
        difficulty: Difficulty::Bronze,
        weapons: &[],
    };
    let mut stock = Stock::default();
    stock.shelf(Page::Armour).put(
        Dropped {
            kind: Kind::Armour,
            item: 3000,
            level: 1,
            key: 16,
            quality: 0,
            amount: 1,
            values: None,
            named: None,
            socketed: Vec::new(),
        },
        (2, 2),
    );
    stock.merchant = Some(7);
    // Each page: an even roll keeps it; the potions' page then tops up 4 healing potions,
    // which the supply does not hold.
    let mut dice = rolls(&[0, 0, 0, 0, 0, 0, 0, 0, 0]);
    stock.visit(7, 2, &buyer, &supply, &mut dice);
    assert_eq!(dice.taken(), 8);
    assert_eq!(stock.shelf(Page::Armour).items.len(), 1);
    // Another merchant clears every page.
    let mut dice = rolls(&[0; 64]);
    stock.visit(8, 2, &buyer, &supply, &mut dice);
    assert_eq!(stock.merchant, Some(8));
    assert!(stock.shelves.iter().all(|shelf| shelf.items.is_empty()));
}
