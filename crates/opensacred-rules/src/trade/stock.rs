//! The merchants' stock (#1617; docs/re/trade.md): the three pages of the merchant's window
//! (`cUI_Merchandise`), each a grid of 16 × 9 cells and a list of item categories, filled as the
//! hero speaks to a merchant (the page's receive `0x6DAD90`, its event `0x2E` 8): cleared and
//! filled anew for another merchant than the last, refreshed in part for the same one, the potions
//! topped up; the items chosen by the drops' choice of a type (`0x5C7FA0`) and made by the item
//! maker. One stock for every merchant: it is the last merchant's.

use crate::drops::{self, Catalogue, Difficulty, Dropped, Hero, Wanted, between};
use crate::fight::Templates;
use crate::inventory::Grid;
use crate::spawning::Dice;

use super::TRADING;

/// A page's grid: 16 columns and 9 rows of 32 pixels (`0x6DB850`).
pub const COLUMNS: u8 = 16;
pub const ROWS: u8 = 9;
/// The float a little below 1/32767 (`0x890718`), by which the original scales `rand()`.
const UNIT: f32 = f32::from_bits(0x3800_0100);
/// The weapons', the potions' and the runes' categories (`items.pak +0x2E`).
const WEAPONS: u8 = 5;
const POTIONS: u8 = 9;
const RUNES: u8 = 28;
/// The rings' category, whose page also takes the sold items of [`ALSO_WITH_RINGS`].
const RINGS: u8 = 8;
const ALSO_WITH_RINGS: [u8; 3] = [RUNES, 7, 15];
/// The items that fit nowhere which end a fill (`0x6DA8AB`).
const MISSES: u32 = 5;
/// The potions below which a page of them is topped up (`0x6DB71F`).
const POTIONS_KEPT: usize = 8;
/// The weapon skill whose list the Dwarf takes only its guns of (`0x6DA7C2`), the Dwarf's type
/// and the guns' weapon kinds (`0x428000`).
const RANGED: u32 = 7;
const DWARF: u32 = 8;
const GUNS: [u8; 2] = [13, 14];

/// A page of the merchant's window (`UI_TABPAGE_WEAPONS`, `_ARMORS`, `_VARIOUS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Page {
    Weapons,
    Armour,
    Various,
}

impl Page {
    /// The pages in the tab sheet's order.
    pub const ALL: [Self; 3] = [Self::Weapons, Self::Armour, Self::Various];

    /// The page's list of categories (`cUI_Merchant`'s layout `0x6DC010`), each weighed by the
    /// table of `0x43B060`: the weapons; the armour, the shields, the helmets, the belts, the
    /// shoulders, the wings, the legs, the boots, the arms, the gloves, the bridles and the
    /// cannons; the rings, the books, the runes, the amulets, the potions three times, the rings
    /// again and the category 30.
    pub fn categories(self) -> &'static [u8] {
        match self {
            Self::Weapons => &[WEAPONS],
            Self::Armour => &[6, 13, 17, 19, 21, 25, 23, 18, 22, 24, 29, 33],
            Self::Various => &[RINGS, 16, RUNES, 20, POTIONS, POTIONS, POTIONS, RINGS, 30],
        }
    }

    /// Whether an item of `category` sold to the merchant goes onto the page (the event `0x2E`
    /// 7): a category of its list, and on the rings' page the runes and the categories 7 and 15
    /// too.
    pub fn takes(self, category: u8) -> bool {
        let list = self.categories();
        list.contains(&category) || (list.contains(&RINGS) && ALSO_WITH_RINGS.contains(&category))
    }

    /// The page an item of `category` sold goes onto, none for a category of no page's.
    pub fn of(category: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|page| page.takes(category))
    }

    /// A category of the page's list by its weights (`0x6D9710` and `0x6DA8FE`): `r = ⌊rand() ·
    /// u · W⌋`, `u` the float a little below 1/32767 and `W` the weights' sum, and the first
    /// category whose weights summed in turn pass `r`; none where `r` reaches `W`.
    fn pick(self, dice: &mut impl Dice) -> Option<u8> {
        let list = self.categories();
        let total: u32 = list.iter().map(|&c| drops::category_weight(c)).sum();
        let r = whole(roll(dice) * f64::from(total));
        let mut summed = 0;
        list.iter().copied().find(|&c| {
            summed += drops::category_weight(c);
            r < summed
        })
    }
}

/// What the stock asks of the items' tables.
pub trait Supply {
    /// The category of the type `item`, none for a type that the items table does not hold.
    fn category(&self, item: u32) -> Option<u8>;
    /// The weapon kind of the record of `item`, 0 for none.
    fn weapon_kind(&self, item: u32) -> u8;
    /// The size of `item` in cells.
    fn size(&self, item: u32) -> (u8, u8);
    /// The drops' choice of a type (`0x5C7FA0`).
    fn choose(&self, wanted: Wanted, hero: &Hero, dice: &mut impl Dice) -> Option<u32>;
    /// The item of `item` that the item maker makes at `level` by the key `key`.
    fn make(&self, item: u32, level: u32, key: u8, dice: &mut impl Dice) -> Option<Dropped>;
}

/// The drops' tables as the stock's supply: the catalogue, the templates, and the items' sizes.
pub struct Tables<'a, S: Fn(u32) -> (u8, u8)> {
    pub catalogue: &'a Catalogue,
    pub templates: &'a Templates,
    pub size: S,
}

impl<S: Fn(u32) -> (u8, u8)> Supply for Tables<'_, S> {
    fn category(&self, item: u32) -> Option<u8> {
        self.catalogue.category(item)
    }

    fn weapon_kind(&self, item: u32) -> u8 {
        self.catalogue.weapon_kind(item)
    }

    fn size(&self, item: u32) -> (u8, u8) {
        (self.size)(item)
    }

    fn choose(&self, wanted: Wanted, hero: &Hero, dice: &mut impl Dice) -> Option<u32> {
        self.catalogue.choose(wanted, hero, dice)
    }

    fn make(&self, item: u32, level: u32, key: u8, dice: &mut impl Dice) -> Option<Dropped> {
        drops::keyed(self.catalogue, self.templates, (item, level, key), dice)
    }
}

/// The buyer that the stock is made for: the local hero and the difficulty.
pub struct Buyer<'a> {
    pub hero: &'a Hero,
    pub difficulty: Difficulty,
    /// The balance's lists of the weapon skills' types (its tail's sixth table, `0xAD57EC`), each
    /// by its skill, in the order of the skills.
    pub weapons: &'a [(u32, Vec<u32>)],
}

impl Buyer<'_> {
    /// The level of the hero's skill `skill`, 0 where it has none.
    fn skill(&self, skill: u32) -> u32 {
        self.hero
            .skills
            .iter()
            .find(|&&(id, _)| u32::from(id) == skill)
            .map_or(0, |&(_, level)| level)
    }

    /// The quality keys that the hero's trading gives (`0x6DA46C`): `x = 10.5·s/(L + 1)` of the
    /// trading skill `s` and the hero's level `L`, the keys from `⌊(x + 2)/4⌋` to `⌊min(x, 10) +
    /// 2⌋`, the least one below the most where it would reach it.
    pub fn keys(&self) -> (i64, i64) {
        let x =
            f64::from(self.skill(u32::from(TRADING))) / (f64::from(self.hero.level) + 1.0) * 10.5;
        let most = whole_i64(x.min(10.0) + 2.0);
        let least = whole_i64((x + 2.0) * 0.25);
        (if least >= most { most - 1 } else { least }, most)
    }
}

/// A page's items, each by its key, and its grid (the page's inventory entry, `0x5DEC90` of the
/// entries 20 to 22).
#[derive(Clone, Debug, PartialEq)]
pub struct Shelf {
    pub page: Page,
    pub items: Vec<(u32, Dropped)>,
    pub grid: Grid,
    next: u32,
}

/// What an item's add to a page did (`0x6D9DE0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Added {
    /// Made by the item maker and placed.
    Made,
    /// Placed as it is: gold, a potion, a rune or one of the categories 26, 30 and 31.
    Placed,
    /// No type, or no room for it.
    Refused,
}

impl Shelf {
    /// An empty page.
    pub fn new(page: Page) -> Self {
        Self {
            page,
            items: Vec::new(),
            grid: Grid::new(COLUMNS, ROWS),
            next: 1,
        }
    }

    /// Whether the page holds an item of type `item` (`0x5DA780`).
    pub fn holds(&self, item: u32) -> bool {
        self.items.iter().any(|(_, dropped)| dropped.item == item)
    }

    /// The potions on the page (`0x5DA810`).
    pub fn potions(&self, supply: &impl Supply) -> usize {
        self.items
            .iter()
            .filter(|(_, dropped)| supply.category(dropped.item) == Some(POTIONS))
            .count()
    }

    /// Takes the item `key` off the page, as the hero buys it.
    pub fn take(&mut self, key: u32) -> Option<Dropped> {
        self.grid.remove(key)?;
        let index = self.items.iter().position(|(k, _)| *k == key)?;
        Some(self.items.remove(index).1)
    }

    /// Puts `dropped` at its first fit, column by column (`0x5DABA0`'s mode 0), as a sold item
    /// goes onto the page: its key, none where it fits nowhere (the original then destroys it).
    pub fn put(&mut self, dropped: Dropped, size: (u8, u8)) -> Option<u32> {
        let key = self.next;
        self.grid.take(key, size)?;
        self.next += 1;
        self.items.push((key, dropped));
        Some(key)
    }

    /// Empties the page (`0x6D9B90(1)`).
    fn clear(&mut self) {
        self.items.clear();
        self.grid = Grid::new(COLUMNS, ROWS);
    }

    /// Empties up to `⌊5·r⌋` cells at random (`0x6D9B90(0)`), each of a column `⌊r·16⌋` and a row
    /// `⌊r·9⌋`: how many items left.
    fn thin(&mut self, dice: &mut impl Dice) -> u32 {
        let tries = whole(roll(dice) * 5.0);
        let mut removed = 0;
        for _ in 0..tries {
            let column = whole(roll(dice) * f64::from(COLUMNS));
            let row = whole(roll(dice) * f64::from(ROWS));
            let cell = (
                u8::try_from(column).unwrap_or(0),
                u8::try_from(row).unwrap_or(0),
            );
            if let Some(key) = self.grid.at(cell).map(|placed| placed.key) {
                self.take(key);
                removed += 1;
            }
        }
        removed
    }

    /// Adds an item of type `item` (`0x6D9DE0`): its first fit by its size, refused where it has
    /// none; gold, potions, runes and the categories 26, 30 and 31 placed as they are; another
    /// made, after a roll of the maker's fourth argument that it does not read (0 in three cases
    /// of four, else 1, 3 or 2), at `R(⌊0.98·L⌋, ⌊1.1·L⌋)` of the hero's level `L` in one case
    /// of two, else at `L`, rolled again where the first roll is below 1 (1 then), with the key
    /// `key`.
    fn add(
        &mut self,
        item: u32,
        key: u8,
        hero: &Hero,
        supply: &impl Supply,
        dice: &mut impl Dice,
    ) -> Added {
        if item == 0 {
            return Added::Refused;
        }
        let size = supply.size(item);
        if self.grid.first_fit(size).is_none() {
            return Added::Refused;
        }
        let category = supply.category(item);
        let made = category.is_some_and(|c| !matches!(c, POTIONS | RUNES | 26 | 30 | 31))
            && !drops::GOLD.contains(&item);
        let dropped = if made {
            // The fourth argument's rolls (`0x6D9F6E`).
            if dice.below(4) == 0 && dice.below(4) == 0 {
                dice.below(4);
            }
            let level = if level_roll(hero.level, dice) < 1 {
                1
            } else {
                level_roll(hero.level, dice)
            };
            supply.make(item, u32::try_from(level).unwrap_or(0), key, dice)
        } else {
            supply.make(item, 0, 0, dice)
        };
        let Some(dropped) = dropped else {
            return Added::Refused;
        };
        if self.put(dropped, size).is_none() {
            return Added::Refused;
        }
        if made { Added::Made } else { Added::Placed }
    }

    /// One weapon of the hero's best weapon skill (`0x6DA54F`): of the balance's lists in their
    /// skills' order, the skill of the highest level, one of the same level taking its place in
    /// one case of two; one of its list at random, of the guns alone for the Dwarf's ranged
    /// combat, the others drawn out in turn; none where no list is taken.
    fn best_weapon(
        &mut self,
        buyer: &Buyer,
        keys: (i64, i64),
        supply: &impl Supply,
        dice: &mut impl Dice,
    ) {
        let mut best: Option<(u32, &[u32])> = None;
        let mut highest = 0;
        for (skill, list) in buyer.weapons {
            let level = buyer.skill(*skill);
            if level > highest || (level == highest && dice.below(2) != 0) {
                highest = level;
                best = Some((*skill, list));
            }
        }
        let Some((skill, list)) = best else {
            return;
        };
        let mut list = list.to_vec();
        while !list.is_empty() {
            let index = usize::try_from(between(0, to_i64(list.len()) - 1, dice)).unwrap_or(0);
            let item = list[index];
            if skill != RANGED
                || buyer.hero.creature != DWARF
                || GUNS.contains(&supply.weapon_kind(item))
            {
                let key = between(keys.0, keys.1, dice);
                self.add(item, key_of(key), buyer.hero, supply, dice);
                return;
            }
            list.remove(index);
        }
    }

    /// Fills the page (`0x6DA320`) for a merchant of level `level`: `tries` tries, else `⌊10 +
    /// r·(⌊10 + 10·r'⌋ + 10·t)⌋`, `t = min(⌊level/20⌋, 5)`. A category by the weights first: for
    /// the weapons, one of the hero's best weapon skill. Then each try a category by the weights
    /// (the runes none), a key by the trading ([`Buyer::keys`]), the hero's class in one case of
    /// two, and the type of the drops' choice at `⌊L + r·(⌊L/3⌋ + 6)⌋`, its records' levels
    /// tested, of that quality; gold and a type the page holds passed over; the item added, and
    /// five that were not made end the fill.
    fn fill(
        &mut self,
        level: u32,
        tries: Option<u32>,
        buyer: &Buyer,
        supply: &impl Supply,
        dice: &mut impl Dice,
    ) {
        let rolled = rolled_tries(level, dice);
        let tries = tries.unwrap_or(rolled);
        let hero = buyer.hero;
        let top = hero.level + hero.level / 3 + 6;
        let keys = buyer.keys();
        if self.page.pick(dice) == Some(WEAPONS) {
            self.best_weapon(buyer, keys, supply, dice);
        }
        let mut misses = 0;
        for _ in 0..tries {
            if misses >= MISSES {
                return;
            }
            let category = self.page.pick(dice);
            if category == Some(RUNES) {
                continue;
            }
            let key = between(keys.0, keys.1, dice);
            let class = if dice.below(2) == 0 { hero.creature } else { 0 };
            let level = whole(roll(dice) * f64::from(top - hero.level) + f64::from(hero.level));
            let wanted = Wanted {
                category: category.unwrap_or(0),
                level,
                class,
                loose: false,
                quality: key,
                kind: None,
            };
            let item = supply.choose(wanted, hero, dice).unwrap_or(0);
            if drops::GOLD.contains(&item) || (item != 0 && self.holds(item)) {
                continue;
            }
            if self.add(item, key_of(key), hero, supply, dice) != Added::Made {
                misses += 1;
            }
        }
    }

    /// Tops the potions up (`0x6DB712`): on a page of the potions holding fewer than 8, `⌊4 +
    /// 12·r⌋` more, each the drops' potion of the difficulty of the rows 2 to 4 where it is no
    /// healing one.
    fn top_up(&mut self, buyer: &Buyer, supply: &impl Supply, dice: &mut impl Dice) {
        if !self.page.categories().contains(&POTIONS) || self.potions(supply) >= POTIONS_KEPT {
            return;
        }
        let count = whole(roll(dice) * 12.0 + 4.0);
        for _ in 0..count {
            let potion = drops::potion_from(2, buyer.difficulty, dice);
            self.add(potion, 0, buyer.hero, supply, dice);
        }
    }
}

/// The merchant's window's pages and the merchant they were filled for (each page's `+0x3E8`).
#[derive(Clone, Debug, PartialEq)]
pub struct Stock {
    pub shelves: [Shelf; 3],
    pub merchant: Option<u32>,
}

impl Default for Stock {
    fn default() -> Self {
        Self {
            shelves: Page::ALL.map(Shelf::new),
            merchant: None,
        }
    }
}

impl Stock {
    /// The page `page`'s shelf.
    pub fn shelf(&mut self, page: Page) -> &mut Shelf {
        &mut self.shelves[Page::ALL.iter().position(|&p| p == page).unwrap_or(0)]
    }

    /// The hero speaking to the merchant `merchant` of level `level` (the event `0x2E` 8 to each
    /// page in turn): for another merchant than the last, each page emptied and filled; for the
    /// same, in one case of two up to five cells emptied and `R(1, 2k)` tries made of the `k`
    /// items that left; the potions topped up either way. A merchant's own items that the scripts
    /// give it (`0x6DA1A0`) are not followed.
    pub fn visit(
        &mut self,
        merchant: u32,
        level: u32,
        buyer: &Buyer,
        supply: &impl Supply,
        dice: &mut impl Dice,
    ) {
        let again = self.merchant == Some(merchant);
        self.merchant = Some(merchant);
        for shelf in &mut self.shelves {
            if again {
                if dice.below(2) != 0 {
                    let removed = shelf.thin(dice);
                    if removed != 0 {
                        let tries = between(1, 2 * i64::from(removed), dice);
                        let tries = u32::try_from(tries).unwrap_or(1);
                        shelf.fill(level, Some(tries), buyer, supply, dice);
                    }
                }
            } else {
                shelf.clear();
                shelf.fill(level, None, buyer, supply, dice);
            }
            shelf.top_up(buyer, supply, dice);
        }
    }
}

/// A fill's tries for a merchant of level `level` (`0x6DA36F`): `⌊10 + r·(⌊10 + 10·r'⌋ + 10·t)⌋`,
/// `t = min(⌊level/20⌋, 5)`.
fn rolled_tries(level: u32, dice: &mut impl Dice) -> u32 {
    let t = (level / 20).min(5);
    let spread = whole(10.0 + roll(dice) * 10.0) + 10 * t;
    whole(roll(dice) * f64::from(spread) + 10.0)
}

/// The original's roll `rand() · u`, `u` the float a little below 1/32767: 0 to a little below 1.
fn roll(dice: &mut impl Dice) -> f64 {
    f64::from(dice.below(32_768)) * f64::from(UNIT)
}

/// A level's roll of an item made for a page (`0x6D9FE6`): `R(⌊0.98·L⌋, ⌊1.1·L⌋)` in one case of
/// two, by the floats a little above 0.98 and 1.1, else `L`.
fn level_roll(level: u32, dice: &mut impl Dice) -> i64 {
    if dice.below(2) == 0 {
        return i64::from(level);
    }
    let level = f64::from(level);
    let most = whole_i64(level * f64::from(1.1_f32));
    let least = whole_i64(level * f64::from(0.98_f32));
    between(least, most, dice)
}

/// A quality key as a byte, at most 15.
fn key_of(key: i64) -> u8 {
    u8::try_from(key.clamp(0, 15)).unwrap_or(0)
}

/// A length as a signed number.
fn to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// A double truncated toward zero, as `__ftol` gives it, held at 0 below.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a count or a level of a few hundreds, whole and positive"
)]
fn whole(value: f64) -> u32 {
    value.max(0.0).trunc() as u32
}

/// A double truncated toward zero, as `__ftol` gives it.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a quality key or a level of a few hundreds"
)]
fn whole_i64(value: f64) -> i64 {
    value.trunc() as i64
}

#[cfg(test)]
mod tests;
