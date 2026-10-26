//! What a killed creature drops (docs/re/items.md, "Drops"): the calls of the drop that its death
//! makes and their bonus, each call's chance and passes, a pass's quality key and kind, the type
//! of its item by the category, the level and the templates, the item's level and key, gold's
//! amount and pile, and the item's values by the item maker ([`Templates::make`]); the result is a
//! list of drops. `opensacred-game` makes them in the world; this module is the rules alone, its
//! rolls taken from a [`Dice`].
//!
//! Left out (docs/decisions/2026-10-21-drops.md): the runes, the uniques and the set pieces, whose
//! keys fall back as the original's do where it finds none; the bosses' drops by their type; the
//! scripts' drops (`SetDrop`) and the potions of a hero who wears a set; the party's factor of a
//! network game; and the effects 856 and 868.

use std::collections::{HashMap, HashSet};

use opensacred_data::tables::{Balance, Item, ItemSet, Weapon};

use crate::fight::{self, ItemValues, Rank, Templates};
use crate::spawning::Dice;

/// The dagger 1712, which a pass drops where it finds no type of its kind.
pub const DAGGER: u32 = 1712;
/// The gold piles (`0x949920`), by their models `GoldCoins_01`, `_02`, `_05` and `_10.grn`.
pub const GOLD: [u32; 4] = [5132, 5133, 5134, 5135];
/// The potions by row and size, small, medium and large (`0x9499B4`): the rows 0 and 1 alike,
/// the row 5 the healing potions.
const POTIONS: [[u32; 3]; 6] = [
    [5169, 5143, 5138],
    [5169, 5143, 5138],
    [5168, 5142, 5137],
    [5170, 5144, 5139],
    [5172, 5146, 5141],
    [5171, 5145, 5140],
];
/// The weapons' category (`items.pak +0x2E`, docs/02 §6.1).
const WEAPONS: u8 = 5;
/// The armour's categories, of which a pass draws one.
const ARMOUR: [u8; 11] = [6, 13, 17, 18, 19, 21, 22, 23, 24, 25, 29];
/// The bridles' category, which a pass draws once again.
const BRIDLES: u8 = 29;
/// The jewels' categories: a ring in two cases of three, an amulet in one.
const JEWELS: [u8; 3] = [8, 20, 8];
/// The categories whose items the drop does not make further: the potions, the blacksmith's
/// upgrades, the combat arts' runes, 30 and 31.
const UNMADE: [u8; 5] = [9, 26, 28, 30, 31];
/// The potions' category.
const POTIONS_CATEGORY: u8 = 9;
/// The combat arts' runes' category, whose types are listed only of an items table's `+0x6A`.
const RUNES: u8 = 28;
/// The categories whose records' levels the type choice does not test.
const UNLEVELLED: [u8; 2] = [30, 31];
/// The `Creature.pak` class of the heroes, who drop nothing here.
const HEROES: u8 = 1;
/// The animals' class, which drops gold where it drops no potion.
const ANIMALS: u8 = 6;
/// The ghosts' class, which drops no weapon or armour.
const GHOSTS: u8 = 11;
/// The class bits of an item's record (`Weapon.pak +0x84` bits 0–5, 22 and 23).
const CLASS_BITS: u32 = 0x00C0_003F;
/// The record's bit that keeps its type from the drops.
const EXCLUDED: u32 = 0x40;
/// The record's quality bits (16–19), all set for a unique.
const UNIQUE: u32 = 0x000F_0000;
/// Another record's bit that keeps its type from the type choice.
const EXCLUDED_TOO: u32 = 0x0010_0000;
/// The items table's flags (`+0x00`) that keep a type out of the drops' lists.
const UNLISTED: u32 = 0x0A00_0000;
/// The types the items table holds room for (`0x425EA0`).
const TABLE: u32 = 0x7E60;
/// The type whose category the loader of `Weapon.pak` sets, whatever `items.pak` says
/// (`0x434100`): the gladiator's armour `gladarmor01.grn`, of the category 21 on file, 6.
const RECATEGORISED: (u32, u8) = (4053, 6);
/// The highest level the type choice holds a level at.
const HELD: u32 = 28;
/// The tries of a pass.
const TRIES: u32 = 6;
/// The redraws of a call for a type without its record, at most.
const MISSES: u32 = 5;

/// What a pass drew (docs/re/items.md, "A pass: what it makes").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Potion,
    Gold,
    Weapon,
    Armour,
    Jewel,
    /// An item of no drop's kind, a quest's or another, which only the scripts' drops make.
    Other,
}

/// An item that a death drops.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dropped {
    /// What the pass drew; its item is the dagger 1712 where no type of the kind was found.
    pub kind: Kind,
    /// The item's type, an `items.pak` slot.
    pub item: u32,
    /// The level the item maker made it at; 0 for an item it does not make (gold, potions).
    pub level: u32,
    /// The pass's quality key, 0 to 17: at least 16 for potions and gold, and 2 for the key 1,
    /// whose set piece is left out.
    pub key: u8,
    /// The key the item maker took: `16 − key` for the keys 2 to 15, 14 for the key 0 (the
    /// original's where it finds no unique), else 0.
    pub quality: u8,
    /// Gold's amount, 1 for another item.
    pub amount: u32,
    /// The values that the item maker gave the item, of a type with a template.
    pub values: Option<ItemValues>,
}

/// The difficulties, bronze to niobium (the int `0x182EE44`, 0 to 4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Difficulty {
    #[default]
    Bronze,
    Silver,
    Gold,
    Platinum,
    Niobium,
}

impl Difficulty {
    /// The gold that a hero of `gold` pays as it rises after its death (`0x53E4A8` to
    /// `0x53E53C`, by the difficulty `0x182EE44`): a twentieth on bronze, a tenth on silver, a
    /// seventh on gold, a fifth on platinum and a half on niobium, truncated (docs/re/fight.md;
    /// seen on bronze, #976: 18 kept 18, 500 became 475 and then 452).
    pub fn death_toll(self, gold: u32) -> u32 {
        let share = match self {
            Self::Bronze => 20,
            Self::Silver => 10,
            Self::Gold => 7,
            Self::Platinum => 5,
            Self::Niobium => 2,
        };
        gold / share
    }

    /// The difficulties, bronze first.
    pub const ALL: [Self; 5] = [
        Self::Bronze,
        Self::Silver,
        Self::Gold,
        Self::Platinum,
        Self::Niobium,
    ];

    /// Its number, the int `0x182EE44`: 0 for bronze to 4 for niobium.
    pub fn index(self) -> usize {
        match self {
            Self::Bronze => 0,
            Self::Silver => 1,
            Self::Gold => 2,
            Self::Platinum => 3,
            Self::Niobium => 4,
        }
    }

    /// Its column of the balance's rows of six (`0x94E7C8`): bronze the sixth, silver the first,
    /// gold the second and so on.
    pub fn column(self) -> usize {
        match self {
            Self::Bronze => 5,
            Self::Silver => 0,
            Self::Gold => 1,
            Self::Platinum => 2,
            Self::Niobium => 3,
        }
    }
}

/// The dead creature, as its drops take it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dead {
    /// The creature itself, as the object's id (`+0x0C`) tells it from the others ([`LastJewel`]).
    pub id: u64,
    /// Its type, its `Creature.pak` record's id.
    pub creature: u32,
    /// Its `Creature.pak` class (`0x43ADC0`).
    pub class: u8,
    /// Its level `L` (`+0x3FE`).
    pub level: u32,
    /// Its `Creature.pak` experience by level, `b` (`+0x0A`).
    pub worth: u16,
    pub rank: Rank,
}

/// The local hero, as the drops take it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hero {
    /// Its type (`+0x10`, 1 to 9), whose bit an item's record holds for its class.
    pub creature: u32,
    pub level: u32,
    /// Its skills and their levels, which an item may require.
    pub skills: Vec<(u8, u32)>,
}

/// The kill, as it raises the bonus: whether a player made it, and the killer's values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kill {
    pub by_player: bool,
    /// The time played, in minutes (**hypothesis**: the statistics' counter `+0x570C` over 1,000
    /// and over 60).
    pub minutes: f32,
    /// The killer's luck `ℓ`, 100 plus the value of its effect 840 (`0x564A60`).
    pub luck: f32,
    /// The value `s` of the killer's effect 42 (the combat art 55's), 0 without it.
    pub treasure: u32,
}

impl Default for Kill {
    /// The hero's kill, at no time played and the luck of no effect.
    fn default() -> Self {
        Self {
            by_player: true,
            minutes: 0.0,
            luck: 100.0,
            treasure: 0,
        }
    }
}

/// A death, as its drops take it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Death {
    pub dead: Dead,
    pub hero: Hero,
    pub kill: Kill,
    pub difficulty: Difficulty,
}

/// A death's calls of the drop (`0x533BA8`): their count `N` and their bonus `B`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Calls {
    pub count: u32,
    pub bonus: u32,
}

/// The balance's values of the drops: the calls' count (`WurfAnzahl`); a champion's and a boss's
/// bonus and calls more (`ITBonusChamp`, `ITThrowCountBonusChamp`, `ITBonusBoss`,
/// `ITThrowCountBonusBoss`); the time played's factor (`LongivityFactor`); the chance per mille
/// (`WurfWarsch`) and its factor for a boss or a champion (`WurfBoss`); the factor of the bonus by
/// the creature's worth (`DropQuality`); the share of the weighing in the keys' weights
/// (`ItemThrowFactor`); the levels' values of the weighing (`UnterExpAnteil`, `UnterExpVoll`,
/// `ObereEXPVoll`); the kinds' chances per mille (`DropSaft`, `DropGold`, `DropWaffe`,
/// `DropRuestung`); gold's two values (`GoldProLevel`, `GoldProLevel[1]`); the quality keys'
/// weights (the tail's second table); and each difficulty's offset and per cent of the levels
/// (`BaseOffLevel`, `OffLevelProz`).
#[derive(Clone, Debug, PartialEq)]
pub struct Rules {
    throws: u32,
    champion: [u32; 2],
    boss: [u32; 2],
    longevity: f32,
    chance: f32,
    boss_chance: f32,
    quality: f32,
    throw_factor: f32,
    levels: (f32, f32, f32),
    kinds: [u32; 4],
    gold: [i64; 2],
    weights: Vec<(u32, u32)>,
    offsets: [[i64; 2]; 6],
}

impl Default for Rules {
    /// The original's values.
    fn default() -> Self {
        Self {
            throws: 1,
            champion: [35, 3],
            boss: [50, 4],
            longevity: 0.5,
            chance: 350.0,
            boss_chance: 2.0,
            quality: 40.0,
            throw_factor: 0.15,
            levels: (0.85, 1.0, 1.1),
            kinds: [180, 300, 330, 880],
            gold: [20, 2000],
            weights: [
                7, 9, 0, 3, 5, 6, 8, 10, 12, 13, 14, 15, 15, 15, 15, 15, 300, 544,
            ]
            .into_iter()
            .zip(0..)
            .map(|(weight, key)| (key, weight))
            .collect(),
            offsets: [[1, 9], [3, 14], [5, 19], [8, 25], [0, 0], [0, 0]],
        }
    }
}

impl Rules {
    /// The rules of the balance `balance`, the original's values where it lacks them.
    pub fn of(balance: Option<&Balance>) -> Self {
        let default = Self::default();
        let Some(b) = balance else {
            return default;
        };
        let u = |name: &str, or: u32| {
            b.i32(name)
                .and_then(|v| u32::try_from(v).ok())
                .unwrap_or(or)
        };
        let f = |name: &str, or: f32| b.f32(name).unwrap_or(or);
        let (anteil, voll, upper) = default.levels;
        let mut offsets = default.offsets;
        for (column, pair) in offsets.iter_mut().enumerate() {
            for (value, name) in pair.iter_mut().zip(["BaseOffLevel", "OffLevelProz"]) {
                if let Some(v) = b.row_i32(name, column) {
                    *value = i64::from(v);
                }
            }
        }
        Self {
            throws: u("WurfAnzahl", default.throws),
            champion: [
                u("ITBonusChamp", default.champion[0]),
                u("ITThrowCountBonusChamp", default.champion[1]),
            ],
            boss: [
                u("ITBonusBoss", default.boss[0]),
                u("ITThrowCountBonusBoss", default.boss[1]),
            ],
            longevity: f("LongivityFactor", default.longevity),
            chance: f("WurfWarsch", default.chance),
            boss_chance: f("WurfBoss", default.boss_chance),
            quality: f("DropQuality", default.quality),
            throw_factor: f("ItemThrowFactor", default.throw_factor),
            levels: (
                f("UnterExpAnteil", anteil),
                f("UnterExpVoll", voll),
                f("ObereEXPVoll", upper),
            ),
            kinds: [
                u("DropSaft", default.kinds[0]),
                u("DropGold", default.kinds[1]),
                u("DropWaffe", default.kinds[2]),
                u("DropRuestung", default.kinds[3]),
            ],
            gold: [
                b.i32("GoldProLevel").map_or(default.gold[0], i64::from),
                b.i32("GoldProLevel[1]").map_or(default.gold[1], i64::from),
            ],
            weights: if b.quality_weights.is_empty() {
                default.weights
            } else {
                b.quality_weights
                    .iter()
                    .map(|w| (w.key, w.weight))
                    .collect()
            },
            offsets,
        }
    }

    /// The level `E` of a creature of level `level` on bronze's scale (`0x5522D0`, docs/re/fight.md,
    /// "Who fights"): 1 up to the difficulty's `BaseOffLevel`, else
    /// `(L − BaseOffLevel)/(1 + OffLevelProz/100)`, rounded, at least 1; `L` on bronze.
    pub fn bronze(&self, level: u32, difficulty: Difficulty) -> u32 {
        let [offset, per_cent] = self.offsets[difficulty.column()];
        let level = i64::from(level);
        if level <= offset {
            return 1;
        }
        let e = to_f64(level - offset) / (to_f64(per_cent) / 100.0 + 1.0);
        u32::try_from(truncated(e + 0.5).max(1)).unwrap_or(1)
    }

    /// The levels `lo` and `hi` that an item of a creature of level `level` is made between
    /// (`0x559BF0`): `lo = ⌊(L − BaseOffLevel)/(1 + OffLevelProz/100)⌋` by the difficulty's
    /// column, truncated, `L` itself on bronze; `hi = ⌊1.05·lo + 0.5⌋`.
    pub fn item_levels(&self, level: u32, difficulty: Difficulty) -> (i64, i64) {
        let [offset, per_cent] = self.offsets[difficulty.column()];
        let below = to_f64(i64::from(level) - offset);
        let lo = truncated(below / (to_f64(per_cent) * f64::from(0.01_f32) + 1.0));
        let hi = truncated(to_f64(lo) * f64::from(1.05_f32) + 0.5);
        (lo, hi)
    }

    /// Whether a creature of level `e` on bronze's scale is too weak for a hero of level `hero` to
    /// make more calls: `(E + 9)/(H + 9)` below `ObereEXPVoll` and its weighing at the floor 0.04.
    fn too_weak(&self, e: u32, hero: u32) -> bool {
        ratio(e, hero) < self.levels.2 && fight::weighing(e, hero, self.levels) <= 0.04
    }

    /// The calls of the drop that a death makes, `N`, and their bonus `B` (`0x533BA8`): `N` is
    /// `WurfAnzahl` and `B` 0; a champion has `B = ITBonusChamp` and, unless too weak,
    /// `R(1, ITThrowCountBonusChamp)` calls more; a boss adds `ITBonusBoss` and, unless too weak,
    /// `R(1, ITThrowCountBonusBoss)`; for a player's kill `B` takes `LongivityFactor·v(t)` of the
    /// time played `t`, `v(t) = 100·(1 − 1/(1 + (t − 1)/120))` from a minute on, and then
    /// `√(100·(ℓ − 100))` of the killer's luck, whole after each.
    pub fn calls(&self, death: &Death, dice: &mut impl Dice) -> Calls {
        let e = self.bronze(death.dead.level, death.difficulty);
        let weak = self.too_weak(e, death.hero.level);
        let (mut count, mut bonus) = (self.throws, 0);
        let more = |most: u32, dice: &mut _| {
            if weak {
                0
            } else {
                u32::try_from(between(1, i64::from(most), dice)).unwrap_or(0)
            }
        };
        match death.dead.rank {
            Rank::Common => {}
            // A champion's levelling (`+0xB0` bit `0x400`), a champion's or alone (#621).
            Rank::Champion | Rank::Levelled => {
                bonus = self.champion[0];
                count += more(self.champion[1], dice);
            }
            Rank::Boss => {
                bonus += self.boss[0];
                count += more(self.boss[1], dice);
            }
        }
        let kill = death.kill;
        if kill.by_player {
            let t = kill.minutes;
            let time = if t >= 1.0 {
                100.0 * (1.0 - 1.0 / (1.0 + (t - 1.0) / 120.0))
            } else {
                0.0
            };
            bonus = whole(to_f32(bonus) + self.longevity * time);
            bonus = whole(to_f32(bonus) + (100.0 * (kill.luck - 100.0)).max(0.0).sqrt());
        }
        Calls { count, bonus }
    }

    /// A call's chance per mille, `P` (`0x559BF0`): `WurfWarsch`, times `WurfBoss` for a boss or a
    /// champion, times `(100 + v(s))/100` of the killer's treasure `s`,
    /// `v(s) = 10 + 180·(1 − 1/(1 + (s − 1)/100))` from 1 on (`0x565D70`), truncated at each step.
    pub fn chance(&self, rank: Rank, treasure: u32) -> u32 {
        let mut p = whole(self.chance);
        // Of the champion's mark (`+0x2B8` bit `0x40`), not a champion's levelling alone (#621).
        if matches!(rank, Rank::Boss | Rank::Champion) {
            p = whole(to_f32(p) * self.boss_chance);
        }
        let v = if treasure >= 1 {
            10.0 + 180.0 * (1.0 - 1.0 / (1.0 + to_f32(treasure - 1) / 100.0))
        } else {
            0.0
        };
        whole(to_f32(p) * (100.0 + v) / 100.0)
    }

    /// The bonus that a pass of a creature of type `creature` and worth `b` takes (`0x559BF0`):
    /// for a creature not a player's (above 16) of a worth, `⌊B·√(DropQuality·b)/DropQuality⌋`,
    /// at each pass, so that it compounds over a call's passes (a quirk).
    pub fn compound(&self, bonus: u32, creature: u32, worth: u16) -> u32 {
        if creature <= 16 || self.quality <= f32::EPSILON || worth == 0 {
            return bonus;
        }
        whole(to_f32(bonus) * (self.quality * f32::from(worth)).sqrt() / self.quality)
    }

    /// The running sums of the quality keys' weights for the bonus `bonus` and the factor `g`
    /// (`0x559BF0`), in the order of the keys: each adds `⌊w·m·g + 0.5⌋`, the key 16's without
    /// `m = 1 + min(B, 700)/100`.
    pub fn sums(&self, bonus: u32, g: f32) -> Vec<(u32, u32)> {
        let m = single(f64::from(bonus.min(700)) * f64::from(0.01_f32) + 1.0);
        let mut sum = 0_u32;
        self.weights
            .iter()
            .map(|&(key, weight)| {
                let factor = if key == 16 { 1.0 } else { f64::from(m) };
                let added = f64::from(weight) * factor * f64::from(g) + 0.5 + f64::from(sum);
                sum = u32::try_from(truncated(added)).unwrap_or(sum);
                (key, sum)
            })
            .collect()
    }

    /// The factor `g` of the keys' weights for a creature of level `e` on bronze's scale and a hero
    /// of level `hero`: `10·((1 − ItemThrowFactor)·f + ItemThrowFactor)`, `f` the weighing, or 1
    /// where `(E + 9)/(H + 9)` is at least `ObereEXPVoll`.
    pub fn throw_factor(&self, e: u32, hero: u32) -> f32 {
        let f = if ratio(e, hero) >= self.levels.2 {
            1.0
        } else {
            f64::from(fight::weighing(e, hero, self.levels))
        };
        let share = f64::from(self.throw_factor);
        single(((1.0 - share) * f + share) * 10.0)
    }

    /// A pass's quality key (`0x559BF0`): `u = R(1, 10000)`, and the first key whose running sum
    /// ([`Rules::sums`]) passes it, 17 when none does.
    pub fn key(&self, bonus: u32, g: f32, dice: &mut impl Dice) -> u32 {
        let u = 1 + dice.below(10_000);
        self.sums(bonus, g)
            .into_iter()
            .find(|&(_, sum)| sum > u)
            .map_or(17, |(key, _)| key)
    }

    /// Gold's amount of a creature of level `level` (`0x559BF0`): `g = a + ⌊(L − 1)·(b − a)/100⌋`
    /// of `GoldProLevel`'s two values, then `g − ⌊g/6⌋ + R(0, ⌊g/3⌋)`, at least 1.
    pub fn gold(&self, level: u32, dice: &mut impl Dice) -> u32 {
        let [a, b] = self.gold;
        let g = a + (i64::from(level) - 1) * (b - a) / 100;
        let third = g / 3;
        let roll = if third < 0 {
            i64::from(dice.below(2))
        } else {
            between(0, third, dice)
        };
        u32::try_from((g + roll - g / 6).max(1)).unwrap_or(1)
    }
}

/// The potion of a drop (`0x43B1A0`): the row 5 in 601 cases of 1,000, else one of the rows 0
/// to 4; small on bronze, small or medium on silver, medium or large on gold, large above.
pub fn potion(difficulty: Difficulty, dice: &mut impl Dice) -> u32 {
    let row = if dice.below(1000) > 600 {
        usize::try_from(between(0, 4, dice)).unwrap_or(5)
    } else {
        5
    };
    let column = match difficulty {
        Difficulty::Bronze => 0,
        Difficulty::Silver => dice.below(2),
        Difficulty::Gold => 1 + dice.below(2),
        Difficulty::Platinum | Difficulty::Niobium => 2,
    };
    POTIONS[row][usize::try_from(column).unwrap_or(2)]
}

/// The gold pile of a creature of level `level` (`0x559BF0`): 5132, but in a case of 13 above
/// level 4 5133, of those in a case of 7 above level 9 5134, and of those in a case of 4 above
/// level 19 5135.
pub fn pile(level: u32, dice: &mut impl Dice) -> u32 {
    if dice.below(13) == 0 && level > 4 {
        if dice.below(7) == 0 && level > 9 {
            if dice.below(4) == 0 && level > 19 {
                GOLD[3]
            } else {
                GOLD[2]
            }
        } else {
            GOLD[1]
        }
    } else {
        GOLD[0]
    }
}

/// What a pass asks of the type choice ([`Catalogue::choose`]): the category, the dead
/// creature's level, the class (the hero's type, 0 for any), whether the records' levels go
/// untested, and the quality, `16 − key` for the keys 2 to 15, else 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wanted {
    pub category: u8,
    pub level: u32,
    pub class: u32,
    pub loose: bool,
    pub quality: i64,
}

/// What the drops know of a type of a `Weapon.pak` record.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Record {
    /// `+0x84`: the class bits, the quality and other flags.
    flags: u32,
    /// `+0x94` and `+0x99`: the least level a hero wears it at, and its level.
    levels: [u8; 2],
    /// `+0x9A` and `+0x9B`: a skill that wearing it needs, and that skill's level.
    skill: [u8; 2],
    /// A piece of a set of `sets.bin` (whose loader writes the set's number to `+0x23`).
    set: bool,
    /// Its template's words 38 to 40 ([`Templates::limits`]).
    limits: Option<[i64; 3]>,
}

/// The item types that the drops choose from: each type's category, the facts of its record that
/// a pass tests, and the lists of `0x5C7FA0` by category, of the types below 8191 that the items
/// table lists (not of its flags `0x2000000` or `0x8000000`, nor 5120–5131, 5632 or 5633, a rune
/// only of a `+0x6A`), with a record not of the flag `0x40` and no template of the word 38 −1.
/// The items table holds `items.pak`'s slots and the types derived from them ([`Catalogue::new`]).
#[derive(Clone, Debug, Default)]
pub struct Catalogue {
    categories: HashMap<u32, u8>,
    records: HashMap<u32, Record>,
    lists: HashMap<u8, Vec<u32>>,
    /// The `items.pak` slot whose record each type's entry of the items table holds: its own, or
    /// for a derived type its root's ([`Catalogue::slot`]).
    slots: HashMap<u32, u32>,
}

impl Catalogue {
    /// The types of `items`, their records in `weapons` (the record whose id is the type,
    /// **hypothesis**, docs/02 §6.2), the pieces of `sets` and the limits of `templates`.
    ///
    /// The items table (`0x4257A0`) holds each slot's `items.pak` record; then, in `Weapon.pak`'s
    /// order, the type of each record with a parent (`+0x24`) takes its parent's entry as it
    /// stands, its own record kept (`+0x18`): a derived type, of 2000–2999 or 6500–7999 for the
    /// most part, has its root's category, flags and rune, and the drops list it as they list
    /// its root (#596). A parent without an entry leaves its type none.
    pub fn new(
        items: &[Item],
        weapons: &[Weapon],
        sets: &[ItemSet],
        templates: &Templates,
    ) -> Self {
        let pieces: HashSet<u32> = sets
            .iter()
            .flat_map(|set| set.items)
            .filter(|&item| item != 0)
            .collect();
        let records: HashMap<u32, Record> = weapons
            .iter()
            .map(|weapon| {
                let word = |i: usize| weapon.words_84.get(i).copied().unwrap_or(0);
                let [skill, needed] = word(11).to_le_bytes();
                let record = Record {
                    flags: u32::from(word(0)) | u32::from(word(1)) << 16,
                    levels: [word(8).to_le_bytes()[0], word(10).to_le_bytes()[1]],
                    skill: [skill, needed],
                    set: pieces.contains(&weapon.id),
                    limits: templates.looked_up(weapon.id),
                };
                (weapon.id, record)
            })
            .collect();
        // Each type's entry, by the slot whose record it holds.
        let by_slot: HashMap<u32, &Item> = items.iter().map(|item| (item.slot, item)).collect();
        let mut slots: HashMap<u32, u32> =
            items.iter().map(|item| (item.slot, item.slot)).collect();
        for weapon in weapons {
            let (id, parent) = (weapon.id, weapon.parent);
            if !(1..TABLE).contains(&id) || !(1..TABLE).contains(&parent) {
                continue;
            }
            match slots.get(&parent).copied() {
                Some(slot) => slots.insert(id, slot),
                None => slots.remove(&id),
            };
        }
        let entry = |item: u32| slots.get(&item).and_then(|slot| by_slot.get(slot).copied());
        let category = |item: u32| {
            let slot = *slots.get(&item)?;
            let category = by_slot.get(&slot)?.category();
            Some(if slot == RECATEGORISED.0 {
                RECATEGORISED.1
            } else {
                category
            })
        };
        let categories: HashMap<u32, u8> = slots
            .keys()
            .filter_map(|&item| Some((item, category(item)?)))
            .collect();
        let mut lists: HashMap<u8, Vec<u32>> = HashMap::new();
        for &listed_type in slots.keys() {
            let (Some(item), Some(category)) = (entry(listed_type), category(listed_type)) else {
                continue;
            };
            let rune = item.words_58[4] >> 16;
            let listed = listed_type < 0x1FFF
                && item.flags() & UNLISTED == 0
                && (category != RUNES || rune != 0)
                && !matches!(listed_type, 5120..=5131 | 5632 | 5633)
                && records.get(&listed_type).is_some_and(|record| {
                    record.flags & EXCLUDED == 0
                        && record.limits.is_none_or(|[flags, ..]| flags != -1)
                });
            if listed {
                lists.entry(category).or_default().push(listed_type);
            }
        }
        for list in lists.values_mut() {
            list.sort_unstable();
        }
        Self {
            categories,
            records,
            lists,
            slots,
        }
    }

    /// The `items.pak` slot whose record the items table holds for type `item`: the type's own,
    /// or a derived type's root's, whose model and texture it shows (#596).
    pub fn slot(&self, item: u32) -> Option<u32> {
        self.slots.get(&item).copied()
    }

    /// The kind of a drop of type `item`, by its category: gold, a potion, a weapon, armour or a
    /// jewel; `None` for a type of any other category (the runes, the blacksmith's upgrades) or
    /// none.
    pub fn kind(&self, item: u32) -> Option<Kind> {
        if GOLD.contains(&item) {
            return Some(Kind::Gold);
        }
        match *self.categories.get(&item)? {
            POTIONS_CATEGORY => Some(Kind::Potion),
            WEAPONS => Some(Kind::Weapon),
            category if ARMOUR.contains(&category) => Some(Kind::Armour),
            category if JEWELS.contains(&category) => Some(Kind::Jewel),
            _ => None,
        }
    }

    /// The types listed for `category`, in the order of their slots.
    pub fn list(&self, category: u8) -> &[u32] {
        self.lists.get(&category).map_or(&[], Vec::as_slice)
    }

    /// The type of a drop (`0x5C7FA0`, the client's `TypeManager::getRandomItem`) of the
    /// category, level, class and quality `wanted`, of any weapon kind and no set piece, for the
    /// hero `hero`; none where no type fits. A listed type
    /// is a candidate when it has a template whose word 39 is at most the level held at 28 plus
    /// a jitter (1, above level 5 the product of two rolls of 1 to a third of the held level, at
    /// most 3) and at most the level, whose word 40 is at most the quality and whose record's
    /// levels lie below the held level plus the jitter; of the types with class bits, those of
    /// the class asked; no unique of a word 38 of 29. Then one at random: with a class asked, in
    /// one case of two among those of its bit or of none; else, in one case of two, the first of
    /// up to three draws that the hero can wear.
    pub fn choose(&self, wanted: Wanted, hero: &Hero, dice: &mut impl Dice) -> Option<u32> {
        let Wanted {
            category,
            level,
            class,
            loose,
            quality,
        } = wanted;
        let held = i64::from(level.min(HELD));
        let mut found = Vec::new();
        for &item in self.list(category) {
            let Some(record) = self.records.get(&item) else {
                continue;
            };
            let jitter = if held > 5 {
                let most = if held < 9 { held / 3 } else { 3 };
                between(1, most, dice) * between(1, most, dice)
            } else {
                1
            };
            let top = held + jitter;
            let Some([flags, least, least_quality]) = record.limits else {
                continue;
            };
            let [wear, made] = record.levels.map(i64::from);
            let levelled = loose || UNLEVELLED.contains(&category) || (made < top && wear < top);
            if least > top || !levelled || !allows(record.flags, class) {
                continue;
            }
            let unique = record.flags & UNIQUE == UNIQUE && flags == 29;
            if record.flags & EXCLUDED_TOO != 0
                || flags == -1
                || least > i64::from(level)
                || least_quality > quality
                || record.set
                || unique
            {
                continue;
            }
            found.push(item);
        }
        let last = i64::try_from(found.len()).ok()? - 1;
        if last < 0 {
            return None;
        }
        let at = |index: i64| {
            usize::try_from(index)
                .ok()
                .and_then(|i| found.get(i).copied())
        };
        if dice.below(2) == 0 || class == 0 {
            if dice.below(2) == 1 {
                let mut pick = None;
                for _ in 0..3 {
                    pick = at(between(0, last, dice));
                    if pick.is_some_and(|item| self.wears(item, hero)) {
                        break;
                    }
                }
                return pick;
            }
            return at(between(0, last, dice));
        }
        let of_class: Vec<u32> = found
            .iter()
            .copied()
            .filter(|item| {
                self.records.get(item).is_some_and(|record| {
                    record.flags & 0x7F == 0
                        || (record.flags & EXCLUDED == 0
                            && class_bit(class).is_some_and(|bit| record.flags & bit != 0))
                })
            })
            .collect();
        if of_class.is_empty() {
            return at(between(0, last, dice));
        }
        let last = i64::try_from(of_class.len()).ok()? - 1;
        usize::try_from(between(0, last, dice))
            .ok()
            .and_then(|i| of_class.get(i).copied())
    }

    /// Whether the hero can wear an item of type `item` (`0x54ABE0`): its record's class bits
    /// allow the hero's (**hypothesis** for the item's own test), its least level is at most the
    /// hero's, and a skill it needs the hero has at its level; the attributes that it might need
    /// are 0 in every record.
    fn wears(&self, item: u32, hero: &Hero) -> bool {
        let Some(record) = self.records.get(&item) else {
            return false;
        };
        let classes = record.flags & CLASS_BITS;
        let class = classes == 0 || class_bit(hero.creature).is_none_or(|bit| classes & bit != 0);
        let [skill, needed] = record.skill;
        let skilled = skill == 0
            || needed == 0
            || hero
                .skills
                .iter()
                .any(|&(s, at)| s == skill && at >= u32::from(needed));
        class && u32::from(record.levels[0]) <= hero.level && skilled
    }
}

/// Whether an item of the record's flags `flags` may drop for the class asked `class` (its
/// type, 0 for any): an item without class bits for any, else not of the flag `0x40` and of the
/// class's bit.
fn allows(flags: u32, class: u32) -> bool {
    if class == 0 || flags & CLASS_BITS == 0 {
        return true;
    }
    flags & EXCLUDED == 0 && class_bit(class).is_none_or(|bit| flags & bit != 0)
}

/// The bit of a hero's type in an item's record (`0x5C7FA0`): the Seraphim 1, the Gladiator 2,
/// the Battle Mage 4, the Dark Elf 8, the Wood Elf `0x10`, the Vampiress `0x20` by day and by
/// night, the Dwarf `0x400000`, the Daemon `0x800000`; none for another type.
fn class_bit(creature: u32) -> Option<u32> {
    match creature {
        1..=5 => Some(1 << (creature - 1)),
        6 | 7 => Some(0x20),
        8 => Some(0x40_0000),
        9 => Some(0x80_0000),
        _ => None,
    }
}

/// The creature that made the last jewel (`0xAD4000`, the object's id): a creature makes no jewel
/// while it is the last that made one, over its deaths as over a death's passes, so that one
/// creature's drops make one jewel at most until another makes one (#610).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LastJewel(Option<u64>);

/// What a death drops (`0x533BA8`, `0x559BF0`): [`Rules::calls`] calls, each of the chance
/// [`Rules::chance`] in passes of 1,000 per mille; nothing for a creature of the heroes' class.
/// `last` is the creature that made the last jewel, which a jewel made here becomes.
pub fn drops<D: Dice>(
    death: &Death,
    (rules, catalogue, templates): (&Rules, &Catalogue, &Templates),
    scripted: &[(u32, u32)],
    last: &mut LastJewel,
    dice: &mut D,
) -> Vec<Dropped> {
    if death.dead.class == HEROES {
        return Vec::new();
    }
    let calls = rules.calls(death, dice);
    let mut dropping = Dropping {
        death,
        rules,
        catalogue,
        templates,
        last,
    };
    let chance = i64::from(rules.chance(death.dead.rank, death.kill.treasure));
    let mut out = Vec::new();
    for _ in 0..calls.count {
        // The scripts' drops of the dead's type (`0x4B06D0`): the client's one entry makes its
        // item when `rand() % 100 + 1` is at most its chance, ours each entry so (#930); the
        // items made so are the call's type at each of its passes, in place of its random ones.
        let forced: Vec<u32> = scripted
            .iter()
            .filter(|&&(_, chance)| dice.below(100) < chance)
            .map(|&(item, _)| item)
            .collect();
        let (mut bonus, mut misses, mut p) = (calls.bonus, 0, chance);
        while p > 0 {
            if forced.is_empty() {
                out.extend(dropping.pass(p, &mut bonus, &mut misses, dice));
            } else {
                for &item in &forced {
                    out.push(dropping.forced(item, &mut bonus, dice));
                }
            }
            p -= 1000;
        }
    }
    out
}

/// A death's drops in the making: what they read, and the creature that made the last jewel.
struct Dropping<'a> {
    death: &'a Death,
    rules: &'a Rules,
    catalogue: &'a Catalogue,
    templates: &'a Templates,
    last: &'a mut LastJewel,
}

/// Whether a pass makes its type or draws again (`0x559BF0`).
enum Check {
    Make,
    Again,
}

impl Dropping<'_> {
    /// A pass of the chance `p` per mille: up to six tries, each of which makes nothing unless a
    /// roll below 1,000 is below `p`, takes the bonus on ([`Rules::compound`]), a quality key, a
    /// kind and its type, and draws again for a type that drops nothing.
    fn pass(
        &mut self,
        p: i64,
        bonus: &mut u32,
        misses: &mut u32,
        dice: &mut impl Dice,
    ) -> Option<Dropped> {
        let dead = self.death.dead;
        let hero = self.death.hero.level;
        let e = self.rules.bronze(dead.level, self.death.difficulty);
        let g = self.rules.throw_factor(e, hero);
        for _ in 0..TRIES {
            if i64::from(dice.below(1000)) >= p {
                return None;
            }
            *bonus = self.rules.compound(*bonus, dead.creature, dead.worth);
            let key = self.rules.key(*bonus, g, dice);
            let quality = if (2..=15).contains(&key) { 16 - key } else { 0 };
            let (kind, item) = self.kind(i64::from(quality), dice);
            let key = if matches!(kind, Kind::Potion | Kind::Gold) {
                key.max(16)
            } else {
                key
            };
            let Some(item) = item else {
                return Some(self.make(kind, DAGGER, key, dice));
            };
            match self.check(item, misses) {
                Check::Again => continue,
                Check::Make => return Some(self.make(kind, item, key, dice)),
            }
        }
        None
    }

    /// A pass of a call whose type is forced (`0x559BF0` with `T` given): no chance to miss, the
    /// bonus taken on and a key drawn as a pass draws them, the item of the type made at its key
    /// as a drop's is, a potion or another kind's at its plain keys.
    fn forced(&mut self, item: u32, bonus: &mut u32, dice: &mut impl Dice) -> Dropped {
        let dead = self.death.dead;
        let hero = self.death.hero.level;
        let e = self.rules.bronze(dead.level, self.death.difficulty);
        let g = self.rules.throw_factor(e, hero);
        *bonus = self.rules.compound(*bonus, dead.creature, dead.worth);
        let key = self.rules.key(*bonus, g, dice);
        let kind = self.catalogue.kind(item).unwrap_or(Kind::Other);
        let key = if matches!(kind, Kind::Potion | Kind::Gold | Kind::Other) {
            key.max(16)
        } else {
            key
        };
        self.make(kind, item, key, dice)
    }

    /// What a pass makes, in order (`0x559BF0`): a potion in `DropSaft` ‰; gold in `DropGold` ‰,
    /// always for the animals, and for the ghosts in 600 ‰ of the rest; but for the ghosts a weapon
    /// in `DropWaffe` ‰ and armour in `DropRuestung` ‰ of the rest; else a jewel, unless the
    /// creature made the last one ([`LastJewel`]); the type none where none is found. The runes, which come before the gold,
    /// are left out.
    fn kind(&mut self, quality: i64, dice: &mut impl Dice) -> (Kind, Option<u32>) {
        let dead = self.death.dead;
        let [potion_chance, gold, weapon, armour] = self.rules.kinds;
        if dice.below(1000) < potion_chance {
            return (Kind::Potion, Some(potion(self.death.difficulty, dice)));
        }
        if dice.below(1000) < gold
            || dead.class == ANIMALS
            || (dice.below(1000) < 600 && dead.class == GHOSTS)
        {
            return (Kind::Gold, Some(pile(dead.level, dice)));
        }
        if dice.below(1000) < weapon && dead.class != GHOSTS {
            let loose = dice.below(3) != 0;
            let class = self.asked(dice);
            return (
                Kind::Weapon,
                self.choose(WEAPONS, class, loose, quality, dice),
            );
        }
        if dice.below(1000) < armour && dead.class != GHOSTS {
            let loose = dice.below(3) != 0;
            let mut category = ARMOUR[index(dice.below(11))];
            if category == BRIDLES {
                category = ARMOUR[index(dice.below(11))];
            }
            let class = self.asked(dice);
            return (
                Kind::Armour,
                self.choose(category, class, loose, quality, dice),
            );
        }
        if self.last.0 == Some(dead.id) {
            return (Kind::Jewel, None);
        }
        self.last.0 = Some(dead.id);
        let category = JEWELS[index(dice.below(3))];
        let class = self.asked(dice);
        (
            Kind::Jewel,
            self.choose(category, class, false, quality, dice),
        )
    }

    /// The class a type is chosen for: the hero's in one case of two, else any.
    fn asked(&self, dice: &mut impl Dice) -> u32 {
        if dice.below(2) == 0 {
            self.death.hero.creature
        } else {
            0
        }
    }

    fn choose(
        &self,
        category: u8,
        class: u32,
        loose: bool,
        quality: i64,
        dice: &mut impl Dice,
    ) -> Option<u32> {
        let wanted = Wanted {
            category,
            level: self.death.dead.level,
            class,
            loose,
            quality,
        };
        self.catalogue.choose(wanted, &self.death.hero, dice)
    }

    /// Whether a pass makes `item` (`0x559BF0`): a type whose template has the word 38 −1 is
    /// drawn again, and so is one without its record or of the record's flag `0x40`, five times
    /// a call at most.
    fn check(&self, item: u32, misses: &mut u32) -> Check {
        let record = self.catalogue.records.get(&item);
        if record
            .and_then(|r| r.limits)
            .is_some_and(|[flags, ..]| flags == -1)
        {
            return Check::Again;
        }
        if record.is_none_or(|r| r.flags & EXCLUDED != 0) && *misses < MISSES {
            *misses += 1;
            return Check::Again;
        }
        Check::Make
    }

    /// The drop of `item` by the key `key` (`0x559BF0`): gold with its amount; potions, runes and
    /// the categories 26, 30 and 31 as they are; another item by the maker, the key 1 (a set
    /// piece, left out) taken as 2: the key 0 (a unique, left out) with the key 14 at `R(lo, hi)`,
    /// 2 to 15 with `16 − key` at `R(lo, hi)`, 16 with 0 at `lo`, 17 with 0 at `R(⌊lo/2⌋, lo)`,
    /// the rolls at least 1.
    fn make(&self, kind: Kind, item: u32, key: u32, dice: &mut impl Dice) -> Dropped {
        let key = if key == 1 { 2 } else { key };
        let mut drop = Dropped {
            kind,
            item,
            level: 0,
            key: u8::try_from(key).unwrap_or(17),
            quality: 0,
            amount: 1,
            values: None,
        };
        let dead = self.death.dead;
        if GOLD.contains(&item) {
            drop.amount = self.rules.gold(dead.level, dice);
            return drop;
        }
        let category = self.catalogue.categories.get(&item).copied();
        if category.is_some_and(|c| UNMADE.contains(&c))
            || !self.catalogue.records.contains_key(&item)
        {
            return drop;
        }
        let (lo, hi) = self.rules.item_levels(dead.level, self.death.difficulty);
        let (quality, level) = match key {
            0 => (14, at_least_one(lo, hi, dice)),
            16 => (0, lo),
            17 => (0, at_least_one(lo / 2, lo, dice)),
            key => (16 - key, at_least_one(lo, hi, dice)),
        };
        drop.quality = u8::try_from(quality).unwrap_or(0);
        drop.level = u32::try_from(level).unwrap_or(0);
        drop.values = self.templates.make(item, drop.level, dice);
        drop
    }
}

/// An item of type `item` as a test throws it (the game's command `item`, #525): of its kind by
/// its category, `amount` pieces of gold, a potion as it is, any other made by the item maker at
/// `level` with the pass's quality key `key` as a drop's would be (the key 0 at the maker's key
/// 14, 2 to 15 at `16 − key`, 16 and 17 at 0); `None` for a type of no drop's kind.
pub fn thrown(
    catalogue: &Catalogue,
    templates: &Templates,
    (item, level, key, amount): (u32, u32, u8, u32),
    dice: &mut impl Dice,
) -> Option<Dropped> {
    let kind = catalogue.kind(item)?;
    let mut drop = Dropped {
        kind,
        item,
        level: 0,
        key: key.min(17),
        quality: 0,
        amount: 1,
        values: None,
    };
    match kind {
        Kind::Gold => drop.amount = amount.max(1),
        Kind::Potion | Kind::Other => {}
        Kind::Weapon | Kind::Armour | Kind::Jewel => {
            drop.quality = match drop.key {
                0 => 14,
                key @ 1..=15 => 16 - key.max(2),
                _ => 0,
            };
            drop.level = level.max(1);
            drop.values = templates.make(item, drop.level, dice);
        }
    }
    Some(drop)
}

/// The types that never drop (`0x533BA8`): 71 and 75, `small_spider.grn` and `med_spider.grn`
/// (**hypothesis**: the spiders that a giant spider brings).
pub fn never_drops(creature: u32) -> bool {
    matches!(creature, 71 | 75)
}

/// Whether a drop of type `item` is held upright in its flight (`0x44DFE0`): the types 5120 to
/// 5135, the gold piles among them, and 5375 to 5631, the combat arts' runes.
pub fn upright(item: u32) -> bool {
    matches!(item, 5120..=5135 | 5375..=5631)
}

/// How high a drop leaves, in units of the models' height, wherever the ground lies there
/// (`0x44D7B0`, `0x44D520`), and how fast it rises at first (`0x559BF0`).
pub const THROWN_AT: f32 = 60.0;
const RISES: f32 = 190.0;
/// The height's fall in a second of a drop's flight, in units of the models' height a second,
/// the share of its velocity along the ground that it loses in a second, and the share of that
/// velocity that it moves by (`0x44DFE0` and its caller).
const GRAVITY: f32 = 290.81;
const DRAG: f32 = 0.1;
const ALONG: f32 = 0.1;

/// A drop's flight (`0x44DED0`, `0x44DFE0`): where it is, a point of the ground in world units
/// and a height in units of the models' height; its velocity; its angles about the axes and
/// their spin, in radians and radians a second; and whether it rests on the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flight {
    pub at: [f32; 3],
    pub velocity: [f32; 3],
    pub angles: [f32; 3],
    pub spin: [f32; 3],
    pub resting: bool,
}

impl Flight {
    /// A drop thrown from the point `(x, y)` (`0x559BF0`): at the height 60, with the velocity
    /// `(10·R(−75, 75), 10·R(−75, 75), 190)` and the spin `R(−6, 6)` about each axis.
    pub fn thrown(x: f32, y: f32, dice: &mut impl Dice) -> Self {
        let mut roll = |half: u32| to_f32(dice.below(2 * half + 1)) - to_f32(half);
        let velocity = [10.0 * roll(75), 10.0 * roll(75), RISES];
        let spin = [roll(6), roll(6), roll(6)];
        Self {
            at: [x, y, THROWN_AT],
            velocity,
            angles: [0.0; 3],
            spin,
            resting: false,
        }
    }

    /// The flight's frame of `dt` seconds (`0x44DFE0`): the angles grow by the spin, the point by
    /// a tenth of the velocity along the ground and the height by the vertical velocity, each
    /// times `dt`; the velocity loses a tenth of itself along the ground and 290.81 along the
    /// height, times `dt`. Its velocity along the ground then turns back, as the client's test of
    /// the cell it moves to (`0x636890`) has it at every frame of a drop's flight (seen in the
    /// original, #478; **hypothesis**: the test takes the flight's offset for the place), so that
    /// the drop rises and falls within a unit of where it was thrown; below the height its point
    /// stands at (`ground`, `0x449950`) it lies there without a velocity; it rests once each
    /// velocity is below 1 and it lies within 1 of the ground.
    pub fn step(&mut self, dt: f32, ground: impl Fn(f32, f32) -> f32) {
        if self.resting {
            return;
        }
        let [x, y, z] = self.at;
        let [vx, vy, vz] = self.velocity;
        self.at = [x + ALONG * vx * dt, y + ALONG * vy * dt, z + vz * dt];
        for (angle, spin) in self.angles.iter_mut().zip(self.spin) {
            *angle += spin * dt;
        }
        self.velocity = [
            -(vx - DRAG * vx * dt),
            -(vy - DRAG * vy * dt),
            vz - GRAVITY * dt,
        ];
        let [x, y, z] = self.at;
        let floor = ground(x, y);
        if z < floor {
            self.at[2] = floor;
            self.velocity = [0.0; 3];
        }
        self.resting =
            self.velocity.iter().all(|v| v.abs() < 1.0) && (self.at[2] - floor).abs() < 1.0;
    }
}

/// The level of an item between `least` and `most` (`0x559BF0`): a roll, 1 where it is below 1,
/// else a roll again, which a least of 0 can still make 0 (a quirk).
fn at_least_one(least: i64, most: i64, dice: &mut impl Dice) -> i64 {
    if between(least, most, dice) < 1 {
        1
    } else {
        between(least, most, dice)
    }
}

/// `(E + 9)/(H + 9)`.
fn ratio(e: u32, hero: u32) -> f32 {
    (to_f32(e) + 9.0) / (to_f32(hero) + 9.0)
}

/// The game's roll of a whole number between `least` and `most` (`R`, `0x443B20`): a negative
/// least counts as 0, a negative most as 1, the two swapped when reversed; no roll when they are
/// equal.
fn between(least: i64, most: i64, dice: &mut impl Dice) -> i64 {
    let (a, b) = (least.max(0), if most < 0 { 1 } else { most });
    let (low, high) = (a.min(b), a.max(b));
    if low == high {
        return low;
    }
    let span = u32::try_from(high - low + 1).unwrap_or(u32::MAX);
    low + i64::from(dice.below(span))
}

/// An index of a roll.
fn index(roll: u32) -> usize {
    usize::try_from(roll).unwrap_or(0)
}

/// A whole number as a float.
fn to_f32(value: u32) -> f32 {
    f32::from(u16::try_from(value).unwrap_or(u16::MAX))
}

/// A whole number as a double.
fn to_f64(value: i64) -> f64 {
    f64::from(i32::try_from(value).unwrap_or(if value < 0 { i32::MIN } else { i32::MAX }))
}

/// The whole part of a float, 0 below 0 (the runtime's `_ftol` of a positive value).
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a chance or a bonus, whole and positive, far below the integers' range"
)]
fn whole(value: f32) -> u32 {
    value.max(0.0) as u32
}

/// A double truncated toward zero (the runtime's `_ftol`).
#[expect(
    clippy::cast_possible_truncation,
    reason = "a level or a sum of weights, far below the integers' range"
)]
fn truncated(value: f64) -> i64 {
    value as i64
}

/// A value stored as a single-precision float.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the original stores the value as a float"
)]
fn single(value: f64) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests;
