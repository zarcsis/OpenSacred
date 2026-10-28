//! What a container gives as it opens (#1381; docs/re/containers.md, "The contents"): the
//! scripts' `FillChest` (78, `0x4A02A0`), which its `Take:` function runs as it opens. With a
//! value, 16 cases in 101 give nothing (a grave or a shrine raises a ghost instead); else a run of
//! random items of about the hero's level, each in 19 of 101 rolls and each taking its price off
//! the value, and then a pile of gold by the hero's level; or the books, or the type it names.

use crate::drops::{self, Catalogue, Hero, Kind, Wanted, between};
use crate::spawning::Dice;

/// `BalanceSpawnItem` (107), as the start script of every tree gives it (`0x475EBE`; the
/// interpreter's `+0x7684` to `+0x76A0`): the share in 101 that gives nothing, the share below
/// which the run makes no more items, the gold's per cent of the value, the share that raises an
/// item's level, and the shares of the weapons, the armour, the runes and the jewels.
pub const SPAWN_ITEM: [i64; 8] = [15, 82, 20, 1, 40, 40, 1, 1];

/// The categories of the run's armour and jewels (`0x4A08D7`…, `0x4A0945`…), and the weapons'
/// and the runes'.
const ARMOUR: [u8; 10] = [6, 13, 17, 18, 19, 21, 22, 23, 24, 25];
const JEWELS: [u8; 9] = [7, 8, 9, 15, 16, 20, 29, 30, 31];
const WEAPONS: u8 = 5;
const RUNES: u8 = 28;
/// The quality that the run asks of its types (`0x4A0AAC`).
const RUN_QUALITY: i64 = 14;
/// The containers that raise a creature where the value gives nothing: the shrines and the
/// graves (the table `0x4A158C`).
const RAISING: [u32; 9] = [5214, 5215, 5216, 5217, 5219, 5223, 5224, 5225, 5226];
/// The creatures they raise: the ghost, and the nobles (`0x4A0786`…).
const GHOST: u32 = 46;
const NOBLE: [u32; 2] = [597, 598];
/// The icons, the potion that stands in for one, and the instruments (`0x4A0B35`, `0x4A04A0`…).
const ICONS: [u32; 6] = [5101, 5102, 5103, 5104, 5105, 5106];
const ICON_POTION: u32 = 5171;
const INSTRUMENTS: [u32; 5] = [5108, 5109, 5110, 5111, 5112];
/// The piles of gold by the value (`0x4A0CE5`).
const PILES: [u32; 4] = [5132, 5133, 5134, 5135];
/// The books' types (`0x4A0F86`…).
const BOOKS: i64 = 4100;
const BOOK_SPAN: i64 = 202;
const SCROLLS: i64 = 4613;
const SCROLL_SPAN: i64 = 26;
const GAMEBOXES: i64 = 5150;
const GAMEBOX_SPAN: i64 = 10;

/// A `FillChest`'s operands: the value (`0x53`), a type (`0x02`), a count of books (`0x86`) and
/// whether it names its container (`0x01`). The counts of random items (`0x57`) are not
/// followed: their categories' table (`0x890B60`) is not read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fill {
    pub value: Option<i32>,
    pub item: Option<u32>,
    pub books: u32,
    pub named: bool,
}

/// What a container gives: an item of a type made at a level, gold of a pile and an amount, or a
/// creature raised.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Content {
    Item { item: u32, level: u32 },
    Gold { pile: u32, amount: u32 },
    Creature(u32),
}

/// Who opens the container and what it is: the hero, its level held for the gold (to its
/// sector's highest plus the difficulty's `LevelKap`), the drops' rules, whether the container
/// raises a creature, whether an icon lies in the world, and the count of the class's runes given
/// (`drops::Rules::rune_range`).
pub struct Opening<'a> {
    pub hero: &'a Hero,
    pub gold_level: u32,
    pub rules: &'a drops::Rules,
    pub raises: bool,
    pub icon_in_world: bool,
    pub given: &'a mut u32,
}

impl Opening<'_> {
    /// Whether the container of the type `kind` raises a creature where its value gives nothing.
    pub fn raising(kind: u32) -> bool {
        RAISING.contains(&kind)
    }
}

/// The seed of a container's rolls (`0x4A0705`): the sum of the hero's type, its id and its
/// experience, and the container's point and id, so that a container gives a hero the same until
/// its experience changes.
pub fn seed(hero: (u32, u32, u64), container: (i32, i32, u32)) -> u64 {
    let (kind, id, experience) = hero;
    let (x, y, object) = container;
    let sum = i64::from(kind)
        + i64::from(id)
        + i64::try_from(experience).unwrap_or(i64::MAX)
        + i64::from(x)
        + i64::from(y)
        + i64::from(object);
    u64::from_ne_bytes(sum.to_ne_bytes())
}

/// What one `FillChest` of `fill` puts in its container (`0x4A02A0`), in order, `price` each
/// type's price (`+0x08`): a value `v + R(0, ⌊v/10⌋ + 1)`, or without a value, books or type the
/// hero's level times 800; for a value above 0, nothing in [`SPAWN_ITEM`]`[0] + 1` cases of 101 (a
/// creature raised by a raising container), else the run of items and its gold; then the books;
/// then the type named, where there is no value.
pub fn fill(
    fill: &Fill,
    opening: &mut Opening<'_>,
    catalogue: &Catalogue,
    price: impl Fn(u32) -> i64,
    dice: &mut impl Dice,
) -> Vec<Content> {
    let mut made = Vec::new();
    let hero = opening.hero;
    let mut value = match fill.value {
        Some(v) => i64::from(v) + between(0, i64::from(v) / 10 + 1, dice),
        None if fill.books == 0 && fill.item.is_none() => i64::from(hero.level) * 800,
        None => 0,
    };
    if value > 0 {
        if between(0, 100, dice) <= SPAWN_ITEM[0] {
            if opening.raises {
                made.push(Content::Creature(raised(dice)));
            }
        } else {
            run(&mut value, &mut made, opening, catalogue, &price, dice);
        }
    }
    for _ in 0..fill.books {
        if let Some(book) = book(catalogue, dice) {
            made.push(Content::Item {
                item: book,
                level: hero.level,
            });
        }
    }
    if let Some(item) = fill.item.filter(|_| !fill.named && fill.value.is_none()) {
        let item = replaced_rune(item, opening, catalogue, dice);
        made.push(Content::Item {
            item,
            level: hero.level,
        });
    }
    made
}

/// The creature that a raising container raises (`0x4A0786`): the ghost in 2 cases of 3, else a
/// noble, the man in 2 of 3.
fn raised(dice: &mut impl Dice) -> u32 {
    if dice.below(3) != 0 {
        GHOST
    } else if dice.below(3) != 0 {
        NOBLE[0]
    } else {
        NOBLE[1]
    }
}

/// The run of items and its gold (`0x4A08BE`): while `R(0, 100)` is at least
/// [`SPAWN_ITEM`]`[1]`, an item of a category rolled anew until a type of it is found, its price
/// off the value, the run ending without gold at a value of 0 or less; at the run's end the gold.
fn run(
    value: &mut i64,
    made: &mut Vec<Content>,
    opening: &Opening<'_>,
    catalogue: &Catalogue,
    price: &impl Fn(u32) -> i64,
    dice: &mut impl Dice,
) {
    let hero = opening.hero;
    let mut loose = false;
    while between(0, 100, dice) >= SPAWN_ITEM[1] {
        let (item, level) = loop {
            let category = loop {
                if between(0, 100, dice) < SPAWN_ITEM[4] {
                    loose = dice.below(3) != 0;
                    break WEAPONS;
                }
                if between(0, 100, dice) < SPAWN_ITEM[5] {
                    let category = ARMOUR[pick(ARMOUR.len(), dice)];
                    loose = dice.below(3) != 0;
                    break category;
                }
                if between(0, 100, dice) < SPAWN_ITEM[6] {
                    break RUNES;
                }
                if between(0, 100, dice) < SPAWN_ITEM[7] {
                    break JEWELS[pick(JEWELS.len(), dice)];
                }
            };
            let level = if between(0, 100, dice) < SPAWN_ITEM[3] {
                hero.level + dice.below(2) + 1
            } else {
                hero.level
            };
            let wanted = Wanted {
                category,
                level,
                class: 0,
                loose,
                quality: RUN_QUALITY,
                kind: None,
            };
            if let Some(item) = catalogue.choose(wanted, hero, dice) {
                break (item, level);
            }
        };
        // In 2 cases of 1,001 an icon, where none lies in the world (`0x4A0B1D`).
        let item = if dice.below(1001) < 2 && !opening.icon_in_world {
            ICONS[pick(ICONS.len(), dice)]
        } else {
            item
        };
        made.push(Content::Item { item, level });
        *value -= price(item);
        if *value <= 0 {
            return;
        }
    }
    let pile = match *value {
        v if v >= 1_000_000 => PILES[3],
        v if v >= 100_000 => PILES[2],
        v if v >= 10_000 => PILES[1],
        _ => PILES[0],
    };
    let g = opening.rules.gold_by_level(opening.gold_level);
    let amount = between(g / 2, g + g / 2, dice);
    made.push(Content::Gold {
        pile,
        amount: u32::try_from(amount.max(1)).unwrap_or(1),
    });
}

/// A book of the container's books (`0x4A0F86`): `4100 + R(0, 202)`, in a case of 3 a scroll
/// `4613 + R(0, 26)`, in one of 91 a game box `5150 + R(0, 10)`; none of a type the catalogue
/// does not hold.
fn book(catalogue: &Catalogue, dice: &mut impl Dice) -> Option<u32> {
    let mut book = BOOKS + between(0, BOOK_SPAN, dice);
    if dice.below(3) == 0 {
        book = SCROLLS + between(0, SCROLL_SPAN, dice);
    }
    if dice.below(91) == 0 {
        book = GAMEBOXES + between(0, GAMEBOX_SPAN, dice);
    }
    u32::try_from(book)
        .ok()
        .filter(|&book| catalogue.slot(book).is_some())
}

/// The type `item` named, a rune of which a hero above level 2 may find replaced (`0x4A0340`):
/// where `R(0, r)` passes `UpgradeDrop`, `r` the range of its runes over its level
/// (`drops::Rules::rune_range`), in a case of 26 an icon, or the potion 5171 where one lies in the
/// world, else an instrument.
fn replaced_rune(
    item: u32,
    opening: &mut Opening<'_>,
    catalogue: &Catalogue,
    dice: &mut impl Dice,
) -> u32 {
    if catalogue.kind(item) != Some(Kind::Rune) || opening.hero.level <= 2 {
        return item;
    }
    let range = opening.rules.rune_range(opening.hero, opening.given, false);
    if opening.rules.rune_given(range, dice) {
        return item;
    }
    if dice.below(26) == 0 {
        if opening.icon_in_world {
            ICON_POTION
        } else {
            ICONS[pick(ICONS.len(), dice)]
        }
    } else {
        INSTRUMENTS[pick(INSTRUMENTS.len(), dice)]
    }
}

/// An index below `len` at random.
fn pick(len: usize, dice: &mut impl Dice) -> usize {
    let len = u32::try_from(len).unwrap_or(1);
    usize::try_from(dice.below(len)).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dice that give the numbers of `rolls` in turn, 0 after them.
    struct Rolls(Vec<u32>);

    impl Dice for Rolls {
        fn below(&mut self, n: u32) -> u32 {
            if self.0.is_empty() {
                0
            } else {
                self.0.remove(0).min(n.saturating_sub(1))
            }
        }
    }

    fn hero(level: u32) -> Hero {
        Hero {
            creature: 1,
            level,
            arts: 1,
            ..Hero::default()
        }
    }

    #[test]
    fn a_value_gives_gold_where_the_run_makes_no_item() {
        let rules = drops::Rules::default();
        let hero = hero(1);
        let mut given = 0;
        let mut opening = Opening {
            hero: &hero,
            gold_level: 1,
            rules: &rules,
            raises: false,
            icon_in_world: false,
            given: &mut given,
        };
        // 200 plus 0; the roll 50 above 15 gives; the run's roll 10 below 82 makes no item;
        // the gold of level 1, `20 + 19.8` whole 39... at its least.
        let made = fill(
            &Fill {
                value: Some(200),
                ..Fill::default()
            },
            &mut opening,
            &Catalogue::default(),
            |_| 600,
            &mut Rolls(vec![0, 50, 10, 0]),
        );
        assert_eq!(made.len(), 1);
        let Content::Gold { pile, amount } = made[0] else {
            panic!("{made:?}");
        };
        assert_eq!(pile, 5132);
        assert!(amount >= 1);
    }

    #[test]
    fn a_value_gives_nothing_or_a_ghost_in_16_cases_of_101() {
        let rules = drops::Rules::default();
        let hero = hero(1);
        let mut given = 0;
        let fill_of = |raises: bool, given: &mut u32| {
            let mut opening = Opening {
                hero: &hero,
                gold_level: 1,
                rules: &rules,
                raises,
                icon_in_world: false,
                given,
            };
            fill(
                &Fill {
                    value: Some(200),
                    ..Fill::default()
                },
                &mut opening,
                &Catalogue::default(),
                |_| 600,
                &mut Rolls(vec![0, 15, 1]),
            )
        };
        assert!(fill_of(false, &mut given).is_empty());
        assert_eq!(fill_of(true, &mut given), [Content::Creature(GHOST)]);
    }

    #[test]
    fn without_a_value_the_hero_s_level_times_800_fills_it_and_a_type_named_comes_as_it_is() {
        let rules = drops::Rules::default();
        let hero = hero(1);
        let mut given = 0;
        let mut opening = Opening {
            hero: &hero,
            gold_level: 1,
            rules: &rules,
            raises: false,
            icon_in_world: false,
            given: &mut given,
        };
        // A type named and no value: the type alone.
        let made = fill(
            &Fill {
                item: Some(2149),
                ..Fill::default()
            },
            &mut opening,
            &Catalogue::default(),
            |_| 600,
            &mut Rolls(vec![]),
        );
        assert_eq!(
            made,
            [Content::Item {
                item: 2149,
                level: 1
            }]
        );
    }

    #[test]
    fn a_containers_seed_follows_the_heros_experience() {
        assert_eq!(seed((1, 1, 0), (10, 20, 5)), 37);
        assert_ne!(seed((1, 1, 0), (10, 20, 5)), seed((1, 1, 30), (10, 20, 5)));
    }

    #[test]
    fn the_shrines_and_the_graves_raise_where_a_value_gives_nothing() {
        assert!(Opening::raising(5216));
        assert!(!Opening::raising(5201));
    }
}
