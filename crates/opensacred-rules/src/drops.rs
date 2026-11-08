//! What a killed creature drops (docs/re/items.md, "Drops"): the calls of the drop that its death
//! makes and their bonus, each call's chance and passes, a pass's quality key and kind, the type
//! of its item by the category, the level and the templates, the item's level and key, gold's
//! amount and pile, and the item's values by the item maker ([`Templates::make`]) with the sockets
//! its quality pass rolls ([`Sockets`]); the result is a list of drops. `opensacred-game` makes them in the world; this module is the rules alone, its
//! rolls taken from a [`Dice`].
//!
//! The runes come by their own pass (#1183), and the scripts' drops (`SetDrop`) are the game's
//! (#930). Left out (#1367, #1262): the uniques and the set pieces, whose keys fall back as the
//! original's do where it finds none; the bosses' drops by their type; the potions of a hero who
//! wears a set; the party's factor of a network game; and the effects 856 and 868.

use std::collections::{HashMap, HashSet};

use opensacred_data::tables::{Balance, BossBonus, BossDrop, BossSpecial, Item, ItemSet, Weapon};

use crate::fight::{self, ItemValues, Rank, Templates};
use crate::spawning::Dice;

mod names;
pub mod quality;
mod sockets;
pub use names::name;
pub(crate) use names::{norm, norms};
pub use sockets::Sockets;

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
/// The creature types that drop a potion where they would drop a rune (`0x559BF0`).
const NO_RUNES: [u32; 3] = [516, 592, 593];
/// The rune's roll's scale, both of whose sides `0x559BF0` multiplies by it.
const RUNE_SCALE: f32 = 30.0;
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Kind {
    Potion,
    Gold,
    Weapon,
    Armour,
    Jewel,
    /// A combat art's rune (the category 28, #1183).
    Rune,
    /// An item of no drop's kind, a quest's or another, which only the scripts' drops make.
    Other,
}

/// An item that a death drops.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
    /// Whether the item maker named it ([`name`], `0x5C3130`), and its name's roll of one in two:
    /// the digit 1 taken where it counts no bonus. `None` for an item that it did not make, which
    /// goes by its type's name.
    pub named: Option<bool>,
    /// The items in its sockets, in the order the smith put them there (its object's list of
    /// records, `+0x100`; [`crate::smith::fill`]), none as it drops.
    pub socketed: Vec<crate::smith::Socketed>,
}

/// The difficulties, bronze to niobium (the int `0x182EE44`, 0 to 4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

    /// The factor of the life leech's value (the effect 841) on this difficulty (the strike's
    /// resolution, `0x5708D3`; docs/re/items.md, "The bonus effects that the code reads"): 1 on
    /// bronze, 0.8, 0.7, 0.6 and 0.5 from silver to niobium; the item's line tells the leech so
    /// (#1759).
    pub fn leech(self) -> f32 {
        match self {
            Self::Bronze => 1.0,
            Self::Silver => 0.8,
            Self::Gold => 0.7,
            Self::Platinum => 0.6,
            Self::Niobium => 0.5,
        }
    }

    /// The difficulties, bronze first.
    pub const ALL: [Self; 5] = [
        Self::Bronze,
        Self::Silver,
        Self::Gold,
        Self::Platinum,
        Self::Niobium,
    ];

    /// The item maker's term of its damage (`0x5D0280`, docs/re/items.md, "The damage"), which
    /// moves a made item's share toward magic and poison: 0 on bronze, then 0.34, 0.67, 1.0 and
    /// 1.34.
    pub fn term(self) -> f32 {
        match self {
            Self::Bronze => 0.0,
            Self::Silver => 0.34,
            Self::Gold => 0.67,
            Self::Platinum => 1.0,
            Self::Niobium => 1.34,
        }
    }

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
    /// The runes it carries (the items of the category 28 in its backpack and its storage,
    /// `0x5598B0`) and the sum of its combat arts' levels without their bonuses, which make a
    /// rune's chance (#1183).
    pub runes: u32,
    pub arts: u32,
    /// The types it wears in its 19 slots, of which a set's pieces weigh the choice of a set's
    /// piece (`0x55A8E7`, #2268).
    pub worn: Vec<u32>,
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
    /// Whether the local hero wears five pieces of set 22, whose calls of no type then make a
    /// potion in one case of ten (`0x559CA9`, `set_bonuses::Stats::potions`, #1727).
    pub potions: bool,
    /// The killer's alchemy, its items' effect 868, and the balance's parameters of it: after
    /// the calls a roll below its chance makes a healing potion ([`drops`], #1802); none where
    /// no roll is drawn.
    pub alchemy: Option<(f32, [f32; 3])>,
}

impl Default for Kill {
    /// The hero's kill, at no time played and the luck of no effect.
    fn default() -> Self {
        Self {
            by_player: true,
            minutes: 0.0,
            luck: 100.0,
            treasure: 0,
            potions: false,
            alchemy: None,
        }
    }
}

/// The one case in so many in which a call of no type makes a potion where the local hero wears
/// five pieces of set 22 ([`set_potion`]).
pub const SET_POTION_ONE_IN: u8 = 10;

/// The per cent of the calls of no type that make a potion where the local hero wears five pieces
/// of set 22 ([`set_potion`]), which the list of the sets tells.
pub fn set_potion_chance() -> f32 {
    100.0 / f32::from(SET_POTION_ONE_IN)
}

/// The potion that a call of no type makes where the local hero wears five pieces of set 22: in
/// one case of ten (`rand() % 10 + 1` at 10, [`SET_POTION_ONE_IN`]), one of 5137, 5138, 5144 and
/// 5146 by a second `rand() % 10` of 0 to 3, else 5140 (`0x559CA9`, docs/re/items.md, "One call
/// of the drop").
fn set_potion(dice: &mut impl Dice) -> Option<u32> {
    let one_in = u32::from(SET_POTION_ONE_IN);
    if dice.below(one_in) + 1 != one_in {
        return None;
    }
    Some(match dice.below(10) {
        0 => 5137,
        1 => 5138,
        2 => 5144,
        3 => 5146,
        _ => 5140,
    })
}

/// A death, as its drops take it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Death {
    pub dead: Dead,
    pub hero: Hero,
    pub kill: Kill,
    pub difficulty: Difficulty,
    /// Whether a doll 3140 is in the world, which keeps the Medusa and the troll from making
    /// another (#2269).
    pub doll: bool,
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
    runes: RuneRules,
    /// The effects' records of six floats from the image's `+0x9DC` ([`names::NORMS`]), which the
    /// namer weighs a bonus by; none without a balance.
    norms: Vec<[f32; 6]>,
    /// The rules of the sockets that the item maker rolls.
    sockets: Sockets,
    /// The bosses' drops by their type ([`Boss`], #2269).
    bosses: Vec<Boss>,
}

/// What a boss of some types adds to its death's drops (`0x533E2E`, #2269): its types, its
/// bonus's value, its calls more `R(a, b)`, its treasure (`0x5516B0`: its chance per mille, its
/// least and most key and its level's jitter) and its drop of its own.
#[derive(Clone, Debug, PartialEq)]
struct Boss {
    types: Vec<u32>,
    bonus: u32,
    more: Option<[u32; 2]>,
    treasure: Option<[u32; 4]>,
    special: Option<BossSpecial>,
}

/// The balance's values of a rune's chance (`0x5598B0`): its chance per mille (`UpgradeDrop`),
/// the runes per level of each hero type 1 to 9 (`RunenProLevel_S`, `_G`, `_M`, `_D`, `_E`, `_V`
/// for 6 and 7, `_Z` and `_T`), the runes carried that weigh as an art's level
/// (`RuneInventoryFactor`), the factor's most (`RuneChanceFactorMax`) and a boss's or a
/// champion's factor (`RunesPerBoss`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct RuneRules {
    chance: f32,
    per_level: [f32; 9],
    inventory: f32,
    most: f32,
    boss: f32,
}

/// The count of a class's runes given above which the quests' count starts again from the runes
/// carried and the arts (`0x5595A0`), and the most of the range of its roll.
const QUEST_RUNES_RESET: u32 = 10_000;
const QUEST_RUNES_MOST: f64 = 32_767.0;

impl Default for RuneRules {
    /// The original's values.
    fn default() -> Self {
        Self {
            chance: 50.0,
            per_level: [1.85, 1.2, 2.2, 1.75, 1.75, 2.2, 2.2, 1.75, 1.9],
            inventory: 3.0,
            most: 2.0,
            boss: 1.5,
        }
    }
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
            runes: RuneRules::default(),
            norms: Vec::new(),
            sockets: Sockets::default(),
            bosses: Vec::new(),
        }
    }
}

impl Rules {
    /// The rules of the balance `balance`, the original's values where it lacks them.
    /// The rules with the bosses' drops of `drops` (`tables/boss_drops.ron`), their bonuses read
    /// off `balance`: a key's value, or the image's word at an offset; 0 without it (#2269).
    #[must_use]
    pub fn with_bosses(mut self, drops: &[BossDrop], balance: Option<&Balance>) -> Self {
        let value = |bonus: &BossBonus| -> u32 {
            let Some(b) = balance else {
                return 0;
            };
            match bonus {
                BossBonus::Key(key) => b.i32(key).and_then(|v| u32::try_from(v).ok()).unwrap_or(0),
                BossBonus::Offset(offset) => usize::try_from(offset / 4)
                    .ok()
                    .and_then(|i| b.image.get(i))
                    .copied()
                    .unwrap_or(0),
            }
        };
        self.bosses = drops
            .iter()
            .map(|drop| Boss {
                types: drop.types.iter().copied().map(u32::from).collect(),
                bonus: drop.bonus.as_ref().map_or(0, value),
                more: drop.more.map(|ends| ends.map(u32::from)),
                treasure: drop.treasure.map(|words| words.map(u32::from)),
                special: drop.special,
            })
            .collect();
        self
    }

    /// The boss case of the type `creature`, none for a type the switch does not name.
    fn boss_of(&self, creature: u32) -> Option<&Boss> {
        self.bosses
            .iter()
            .find(|boss| boss.types.contains(&creature))
    }

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
            runes: RuneRules {
                chance: f("UpgradeDrop", default.runes.chance),
                per_level: std::array::from_fn(|kind| {
                    let key = ["S", "G", "M", "D", "E", "V", "V", "Z", "T"][kind];
                    f(
                        &format!("RunenProLevel_{key}"),
                        default.runes.per_level[kind],
                    )
                }),
                inventory: f("RuneInventoryFactor", default.runes.inventory),
                most: f("RuneChanceFactorMax", default.runes.most),
                boss: f("RunesPerBoss", default.runes.boss),
            },
            norms: names::norms(&b.image),
            sockets: Sockets::of(balance),
            bosses: Vec::new(),
        }
    }

    /// The rules of the sockets that the item maker rolls.
    pub fn sockets(&self) -> &Sockets {
        &self.sockets
    }

    /// A rune's chance per mille in a pass of a creature of level `level`, a boss or a
    /// champion where `ranked`, for `hero` (`0x5598B0`): `x = p·H/(n/RuneInventoryFactor + a)`,
    /// `p` the runes per level of the hero's type, `H` its level at least 1, `n` the runes it
    /// carries and `a` its arts' levels; `x⁵` where `x` is above 1 or the creature ranked, else
    /// `x⁴` times the weighing of the levels; at least 0.01 and at most `RuneChanceFactorMax`,
    /// times `RunesPerBoss` where ranked; times `UpgradeDrop`.
    pub fn rune_chance(&self, level: u32, ranked: bool, hero: &Hero) -> f32 {
        let runes = self.runes;
        let per_level = usize::try_from(hero.creature)
            .ok()
            .and_then(|kind| runes.per_level.get(kind.checked_sub(1)?))
            .copied()
            .unwrap_or(1.0);
        let h = f64::from(u16::try_from(hero.level).unwrap_or(u16::MAX).max(1));
        let held =
            f64::from(u16::try_from(hero.runes).unwrap_or(u16::MAX)) / f64::from(runes.inventory);
        let a = f64::from(u16::try_from(hero.arts).unwrap_or(u16::MAX));
        let x = f64::from(per_level) * h / (held + a);
        let r = if x > 1.0 || ranked {
            x.powi(5)
        } else {
            f64::from(fight::weighing(level, hero.level, self.levels)) * x.powi(4)
        };
        let mut r = r.max(0.01).min(f64::from(runes.most));
        if ranked {
            r *= f64::from(runes.boss);
        }
        single(r * f64::from(runes.chance))
    }

    /// The range of the roll of a class rune that a side quest's end or a reward gives the hero
    /// `hero` (`0x5595A0(1000, 1)`, from `0x45FC60`; #1386), with `given` the runes of its class
    /// the quests have given (the player's `+0x5710`): `given` first set to its runes carried and
    /// its arts' levels where it is 0 or above 10,000, and where those fall below it they count
    /// as its two thirds and its third; `x = (runes/2 + arts)/H/p`, `p` the runes per level of the
    /// hero's type 1 to 7 and 1 for the others, `H` its level; `x⁵·1000`, at most 32,767, whole.
    /// The rune comes where a roll of 0 to the range falls below `UpgradeDrop` ([`Self::rune_given`]).
    pub fn quest_rune_range(&self, hero: &Hero, given: &mut u32) -> i64 {
        self.rune_range(hero, given, true)
    }

    /// [`Self::quest_rune_range`] as `0x5595A0(1000, fifth)` takes it: with `fifth` false (a
    /// container's rune, `FillChest`), `x⁴` where `x` is below `p`'s 1, else `x⁵`.
    pub fn rune_range(&self, hero: &Hero, given: &mut u32, fifth: bool) -> i64 {
        let (mut runes, mut arts) = (hero.runes, hero.arts);
        let carried = runes.saturating_add(arts);
        if *given == 0 || *given > QUEST_RUNES_RESET {
            *given = carried;
        }
        if carried < *given {
            arts = *given * 2 / 3;
            runes = *given / 3;
        }
        let per_level = match hero.creature {
            1..=7 => usize::try_from(hero.creature - 1)
                .ok()
                .and_then(|kind| self.runes.per_level.get(kind))
                .copied()
                .unwrap_or(1.0),
            _ => 1.0,
        };
        let h = f64::from(u16::try_from(hero.level).unwrap_or(u16::MAX).max(1));
        let v = single(f64::from(runes / 2 + arts) / h);
        let x = single(f64::from(v) / f64::from(per_level));
        let power = if fifth || per_level <= v { 5 } else { 4 };
        let r = (f64::from(x).powi(power) * 1000.0).min(QUEST_RUNES_MOST);
        truncated(r)
    }

    /// The gold of a container's pile for the hero's level `level` held to its sector's
    /// (`0x4A0CE5`): `a + ⌊(b − a)·L/100⌋` of `GoldProLevel`'s two values.
    pub fn gold_by_level(&self, level: u32) -> i64 {
        let [a, b] = self.gold;
        a + (b - a) * i64::from(level) / 100
    }

    /// Whether a roll of 0 to `range` gives the rune (`0x45FC60`): one below `UpgradeDrop`, the
    /// roll 0 for a range of 0, and a range below 0 taken as 1.
    pub fn rune_given(&self, range: i64, dice: &mut impl Dice) -> bool {
        let roll = match range {
            0 => 0,
            range => between(0, range.max(1), dice),
        };
        to_f64(roll) < f64::from(self.runes.chance)
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
                // The switch by the type (`0x533E2E`, #2269): its key, then its calls more.
                if let Some(case) = self.boss_of(death.dead.creature) {
                    bonus += case.bonus;
                    if let Some([a, b]) = case.more {
                        count +=
                            u32::try_from(between(i64::from(a), i64::from(b), dice)).unwrap_or(0);
                    }
                    match case.special {
                        Some(BossSpecial::Undo) => {
                            count = self.throws;
                            bonus = bonus.saturating_sub(self.boss[0]);
                        }
                        Some(BossSpecial::Anducar | BossSpecial::AnducarDemon) => count = 0,
                        _ => {}
                    }
                }
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
        self.spread(f)
    }

    /// The factor `g` of the keys' weights of the share `f`: `10·((1 − ItemThrowFactor)·f +
    /// ItemThrowFactor)`, stored as a float (`0x559BF0`; a quest's reward's, `0x4AD4BD`).
    pub fn spread(&self, f: f64) -> f32 {
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
    potion_from(0, difficulty, dice)
}

/// The potion of `0x43B1A0` whose second argument makes `least` the least of its rows 0 to 4: 0
/// for a drop's, 2 for a merchant's (docs/re/trade.md).
pub fn potion_from(least: i64, difficulty: Difficulty, dice: &mut impl Dice) -> u32 {
    potion_of(least, None, difficulty, dice)
}

/// The row of the healing potions (5171, 5145, 5140) in `0x43B1A0`'s table.
const HEALING_ROW: usize = 5;

/// The potion of `0x43B1A0` of the least row `least`, its row `row` where one is given (its
/// third argument, 0 to 5), which the rolls of the row are drawn before, as the original draws
/// them; then its column by the difficulty.
fn potion_of(least: i64, row: Option<usize>, difficulty: Difficulty, dice: &mut impl Dice) -> u32 {
    let rolled = if dice.below(1000) > 600 {
        usize::try_from(between(least, 4, dice)).unwrap_or(5)
    } else {
        HEALING_ROW
    };
    let row = row.unwrap_or(rolled);
    let column = match difficulty {
        Difficulty::Bronze => 0,
        Difficulty::Silver => dice.below(2),
        Difficulty::Gold => 1 + dice.below(2),
        Difficulty::Platinum | Difficulty::Niobium => 2,
    };
    POTIONS[row][usize::try_from(column).unwrap_or(2)]
}

/// The categories of a random item and their weights (`0x43B090`, the table at `0x890B60`): the
/// weapons, the armour, the cannons, the rings, the potions, the shields, the books, the helmets,
/// the boots, the belts, the amulets, the shoulders, the arms, the legs, the gloves, the wings,
/// the runes, the bridles and the category 30.
const RANDOM_CATEGORIES: [(u8, u32); 19] = [
    (5, 100),
    (6, 100),
    (33, 75),
    (8, 50),
    (9, 80),
    (13, 60),
    (16, 30),
    (17, 60),
    (18, 50),
    (19, 40),
    (20, 70),
    (21, 40),
    (22, 40),
    (23, 40),
    (24, 30),
    (25, 40),
    (28, 50),
    (29, 20),
    (30, 20),
];

/// The weight of `category` in the table of `0x43B060` (`0x890B60`), 0 for a category that it
/// does not name.
pub fn category_weight(category: u8) -> u32 {
    RANDOM_CATEGORIES
        .iter()
        .find(|&&(c, _)| c == category)
        .map_or(0, |&(_, weight)| weight)
}

/// A random category of any class (`0x43B090(0)`): `r = ⌊rand()/32767 · W⌋`, `W` the weights'
/// sum, and the first category whose weights summed in turn pass `r`; 0, no category, where `r`
/// reaches `W`.
pub fn random_category(dice: &mut impl Dice) -> u8 {
    let total: u32 = RANDOM_CATEGORIES.iter().map(|&(_, weight)| weight).sum();
    let r = u64::from(dice.below(32_768)) * u64::from(total) / 32_767;
    let mut summed = 0;
    for &(category, weight) in &RANDOM_CATEGORIES {
        summed += u64::from(weight);
        if r < summed {
            return category;
        }
    }
    0
}

/// What a `CreateObj` of `0x54` and no type makes for the local hero (`0x4863D4`; docs/re/
/// containers.md, "The hidden stashes"): a potion, or a type to choose as `Wanted` says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptItem {
    Potion(u32),
    Wanted(Wanted),
}

/// The random item of a `CreateObj` of `0x54` and no type for the hero of the level `level` and
/// the creature type `class` on `difficulty` (`0x4863D4`): in 1 case of 4 a potion (`0x43B1A0`);
/// else in 1 of 3 a weapon, else in 1 of 2 an armour, else a [`random_category`]; each at the
/// level `level + rand() % (⌊level/5⌋ + 3) + d`, `d` 0, 1, 3, 5 and 8 bronze to niobium, of the
/// hero's class in 1 case of 2, loose, of the quality 14.
pub fn script_item(
    level: u32,
    class: u32,
    difficulty: Difficulty,
    dice: &mut impl Dice,
) -> ScriptItem {
    let bonus = script_bonus(difficulty);
    if dice.below(4) == 1 {
        return ScriptItem::Potion(potion(difficulty, dice));
    }
    let category = if dice.below(3) == 1 {
        Some(5)
    } else if dice.below(2) == 1 {
        Some(6)
    } else {
        None
    };
    let mut wanted = script_wanted(category.unwrap_or(0), level, class, bonus, dice);
    // The random category is rolled after the class and the level (`0x48661C`).
    if category.is_none() {
        wanted.category = random_category(dice);
    }
    ScriptItem::Wanted(wanted)
}

/// A weapon's roll once more, where a [`script_item`] found no type (`0x48662D`).
pub fn script_weapon(
    level: u32,
    class: u32,
    difficulty: Difficulty,
    dice: &mut impl Dice,
) -> Wanted {
    script_wanted(5, level, class, script_bonus(difficulty), dice)
}

/// The levels that the difficulty adds to a `CreateObj`'s random item: 0, 1, 3, 5 and 8, bronze
/// to niobium (`0x48640B`).
fn script_bonus(difficulty: Difficulty) -> u32 {
    match difficulty {
        Difficulty::Bronze => 0,
        Difficulty::Silver => 1,
        Difficulty::Gold => 3,
        Difficulty::Platinum => 5,
        Difficulty::Niobium => 8,
    }
}

/// The type wanted of `category` for the hero of `level` and `class` with the difficulty's bonus.
fn script_wanted(category: u8, level: u32, class: u32, bonus: u32, dice: &mut impl Dice) -> Wanted {
    let class = if dice.below(2) == 1 { class } else { 0 };
    let level = level + dice.below(level / 5 + 3) + bonus;
    Wanted {
        category,
        level,
        class,
        loose: true,
        quality: 14,
        kind: None,
    }
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

/// The gold pile of an amount laid by the hero (`0x4AEBD0` with the type `0x7E60`): 5132 below
/// 50, 5133 below 350, 5134 below 1,000, else 5135.
pub fn pile_of(amount: u32) -> u32 {
    match amount {
        0..50 => GOLD[0],
        50..350 => GOLD[1],
        350..1000 => GOLD[2],
        _ => GOLD[3],
    }
}

/// `amount` pieces of gold laid by the hero in the pile of their amount ([`pile_of`]): of no
/// level and the key 16, as a death's gold.
pub fn gold_pile(amount: u32) -> Dropped {
    Dropped {
        kind: Kind::Gold,
        item: pile_of(amount),
        level: 0,
        key: 16,
        quality: 0,
        amount,
        values: None,
        named: None,
        socketed: Vec::new(),
    }
}

/// What a pass asks of the type choice ([`Catalogue::choose`]): the category, the dead
/// creature's level, the class (the hero's type, 0 for any), whether the records' levels go
/// untested, the quality, `16 − key` for the keys 2 to 15, else 0, and the weapon kind of
/// `Weapon.pak +0x1E`, none for any (`0xF`; a drop's), as a creature's crossbow that its model
/// cannot shoot asks for a bow or a weapon of kind 1 (#1217).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wanted {
    pub category: u8,
    pub level: u32,
    pub class: u32,
    pub loose: bool,
    pub quality: i64,
    pub kind: Option<u8>,
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
    /// `+0x1E`: a weapon's kind, 0 for a type that is no weapon.
    kind: u8,
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
    /// The types whose entries have a rune (`items.pak +0x6A`, `0x43AF10`), and the runes that
    /// the drops draw from (`0x9565A0`, `tables/runes.ron`; #1183).
    readable: HashSet<u32>,
    runes: Vec<u32>,
    /// Each slot's `items.pak` name (`+0x37`, `0x43CED0`), by which the namer tells a sword from
    /// an axe.
    models: HashMap<u32, String>,
    /// The sets' pieces in the order of `sets.bin` (`+0x563244`, #2268), each its count's.
    sets: Vec<Vec<u32>>,
    /// The types of the quality 15 by their template's word 38, their family (the map
    /// `0xAD4428`), in `Weapon.pak`'s order.
    uniques: HashMap<i64, Vec<u32>>,
    /// The records' own values of the uniques and the set pieces, which a drop raises.
    owned: HashMap<u32, crate::fight::ItemValues>,
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
                    kind: weapon.bytes_1c[2],
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
        let readable = slots
            .keys()
            .copied()
            .filter(|&item| entry(item).is_some_and(|entry| entry.words_58[4] >> 16 != 0))
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
        let set_pieces = sets
            .iter()
            .map(|set| {
                let count = usize::from(set.bytes[0]).min(set.items.len());
                set.items[..count].to_vec()
            })
            .collect();
        // The family map (`0x5D4AC0`): every `Weapon.pak` type of a template by the lookup
        // `0x42CA70`, by its word 38, in the file's order; the drop keeps those of the quality 15.
        let mut uniques: HashMap<i64, Vec<u32>> = HashMap::new();
        let mut owned = HashMap::new();
        for weapon in weapons {
            let Some(record) = records.get(&weapon.id) else {
                continue;
            };
            let unique = record.flags & UNIQUE == UNIQUE;
            if unique && let Some([family, ..]) = templates.looked_up(weapon.id) {
                uniques.entry(family).or_default().push(weapon.id);
            }
            if unique || record.set {
                owned.insert(
                    weapon.id,
                    crate::fight::ItemValues::of_record(&weapon.words_84),
                );
            }
        }
        Self {
            categories,
            records,
            lists,
            slots,
            readable,
            runes: Vec::new(),
            models: items
                .iter()
                .map(|item| (item.slot, item.name.clone()))
                .collect(),
            sets: set_pieces,
            uniques,
            owned,
        }
    }

    /// The catalogue with the runes that the drops draw from (`0x9565A0`, #1183).
    pub fn with_runes(self, runes: Vec<u32>) -> Self {
        Self { runes, ..self }
    }

    /// The `items.pak` slot whose record the items table holds for type `item`: the type's own,
    /// or a derived type's root's, whose model and texture it shows (#596).
    pub fn slot(&self, item: u32) -> Option<u32> {
        self.slots.get(&item).copied()
    }

    /// Whether the type `item` is a piece of a set (`sets.bin`), which the item maker leaves out
    /// and which keeps its record as it stands (#1701).
    pub fn set_piece(&self, item: u32) -> bool {
        self.records.get(&item).is_some_and(|record| record.set)
    }

    /// Whether the type `item` is a unique, its record's quality bits all set, which a forced
    /// drop gives as its record holds it (the key 0, #1709).
    pub fn unique(&self, item: u32) -> bool {
        self.records
            .get(&item)
            .is_some_and(|record| record.flags & UNIQUE == UNIQUE)
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
            RUNES => Some(Kind::Rune),
            WEAPONS => Some(Kind::Weapon),
            category if ARMOUR.contains(&category) => Some(Kind::Armour),
            category if JEWELS.contains(&category) => Some(Kind::Jewel),
            _ => None,
        }
    }

    /// The category of the type `item` (`items.pak +0x2E` of its entry), none for a type that the
    /// items table does not hold.
    pub fn category(&self, item: u32) -> Option<u8> {
        self.categories.get(&item).copied()
    }

    /// The class bits of an item of type `item` that the item maker made, its triples having
    /// given it `triples` (`0x5D5080`, #2266): a weapon's, a shield's or a ring's (the categories
    /// 5, 13 and 8) the triples' alone, another's its record's (`+0x04`'s class bits) and the
    /// triples'; none of them, those of its model's name ([`crate::equipment::named_classes`]),
    /// else every class's. The bit `0x40` of the record never stays.
    pub fn made_classes(&self, item: u32, triples: u32) -> u32 {
        let classes = self.kept_classes(item) | triples;
        if classes != 0 {
            return classes;
        }
        let root = self.slot(item).unwrap_or(item);
        let model = self.models.get(&root).map_or("", String::as_str);
        match crate::equipment::named_classes(item, root, model) {
            0 => crate::equipment::CLASSES,
            named => named,
        }
    }

    /// The class bits of the record of `item` that the item maker keeps (`0x5D5080`): none for a
    /// weapon, a shield or a ring (the categories 5, 13 and 8), else its record's.
    pub fn kept_classes(&self, item: u32) -> u32 {
        match self.category(item) {
            Some(5 | 8 | 13) => 0,
            _ => self
                .records
                .get(&item)
                .map_or(0, |record| record.flags & crate::equipment::CLASSES),
        }
    }

    /// The weapon kind of the record of `item` (`Weapon.pak +0x1E`), 0 for a type without one.
    pub fn weapon_kind(&self, item: u32) -> u8 {
        self.records.get(&item).map_or(0, |record| record.kind)
    }

    /// The types listed for `category`, in the order of their slots.
    pub fn list(&self, category: u8) -> &[u32] {
        self.lists.get(&category).map_or(&[], Vec::as_slice)
    }

    /// The type of a drop (`0x5C7FA0`, the client's `TypeManager::getRandomItem`) of the
    /// category, level, class, quality and weapon kind `wanted`, and no set piece, for the hero
    /// `hero`; none where no type fits. A listed type
    /// is a candidate when it has a template whose word 39 is at most the level held at 28 plus
    /// a jitter (1, above level 5 the product of two rolls of 1 to a third of the held level, at
    /// most 3) and at most the level, whose word 40 is at most the quality and whose record's
    /// levels lie below the held level plus the jitter, of the kind asked if any; of the types
    /// with class bits, those of the class asked; no unique of a word 38 of 29. Then one at
    /// random: with a class asked, in one case of two among those of its bit or of none; else, in
    /// one case of two, the first of up to three draws that the hero can wear.
    pub fn choose(&self, wanted: Wanted, hero: &Hero, dice: &mut impl Dice) -> Option<u32> {
        let Wanted {
            category,
            level,
            class,
            loose,
            quality,
            kind,
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
            // The kind tested after the jitter's rolls, as the original rolls them first.
            if kind.is_some_and(|kind| record.kind != kind) {
                continue;
            }
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

/// The creatures that made the last jewel (`0xAD4000`) and the last rune (`0xAD3FFC`), the
/// objects' ids: a creature makes no jewel while it is the last that made one, over its deaths
/// as over a death's passes, so that one creature's drops make one jewel at most until another
/// makes one (#610), and no rune likewise (#1183).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LastJewel {
    jewel: Option<u64>,
    rune: Option<u64>,
}

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
    // A boss's drops of its type, before the ordinary calls (`0x533E2E`, #2269).
    if death.dead.rank == Rank::Boss
        && let Some(case) = rules.boss_of(death.dead.creature)
    {
        if let Some(treasure) = case.treasure {
            out.extend(dropping.treasure(treasure, dice));
        }
        match case.special {
            Some(BossSpecial::Doll) if between(0, 25, dice) == 0 && !death.doll => {
                out.extend(dropping.forced_call(DOLL, dice));
            }
            Some(BossSpecial::Anducar) => out.extend(dropping.anducar(dice)),
            Some(BossSpecial::AnducarDemon) => out.extend(dropping.anducar_demon(dice)),
            _ => {}
        }
    }
    for _ in 0..calls.count {
        // The scripts' drops of the dead's type (`0x4B06D0`): the client's one entry makes its
        // item when `rand() % 100 + 1` is at most its chance, ours each entry so (#930); the
        // items made so are the call's type at each of its passes, in place of its random ones.
        let mut forced: Vec<u32> = scripted
            .iter()
            .filter(|&&(_, chance)| dice.below(100) < chance)
            .map(|&(item, _)| item)
            .collect();
        // Still of no type, set 22's potion (#1727).
        if forced.is_empty()
            && death.kill.potions
            && let Some(potion) = set_potion(dice)
        {
            forced.push(potion);
        }
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
    // The killer's alchemy (868, `0x5346BC`–`0x53475E`, #1802): `R(0, 99)`, drawn whatever its
    // value, below its chance makes a healing potion (`0x43B1A0` of the row 5), dropped by a call
    // of its type, of no bonus and no greed (`0x559BF0`).
    if let Some((value, params)) = death.kill.alchemy {
        let roll = dice.below(crate::fight::ALCHEMY_FACES);
        if f64::from(roll) < crate::fight::rare_chance(value, params) {
            let potion = potion_of(0, Some(HEALING_ROW), death.difficulty, dice);
            let (mut bonus, mut p) = (0, i64::from(rules.chance(death.dead.rank, 0)));
            while p > 0 {
                out.push(dropping.forced(potion, &mut bonus, dice));
                p -= 1000;
            }
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
            let key = if matches!(kind, Kind::Potion | Kind::Gold | Kind::Rune) {
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
        // A forced set's piece takes the key 1 and a unique the key 0, both only raised (#2268).
        if self.catalogue.set_piece(item) || self.catalogue.unique(item) {
            let key = u8::from(self.catalogue.set_piece(item));
            let (lo, _) = self.rules.item_levels(dead.level, self.death.difficulty);
            let mut drop = Dropped {
                kind,
                item,
                level: 0,
                key,
                quality: 0,
                amount: 1,
                values: None,
                named: None,
                socketed: Vec::new(),
            };
            raised(
                &mut drop,
                (self.catalogue, self.templates),
                (u32::try_from(lo).unwrap_or(0), self.death.difficulty),
            );
            return drop;
        }
        let key = if matches!(kind, Kind::Potion | Kind::Gold | Kind::Rune | Kind::Other) {
            key.max(16)
        } else {
            key
        };
        self.make(kind, item, key, dice)
    }

    /// What a pass makes, in order (`0x559BF0`): a potion in `DropSaft` ‰; gold in `DropGold` ‰,
    /// a rune ([`Self::rune`]); gold in `DropGold` ‰, always for the animals, and for the ghosts in
    /// 600 ‰ of the rest; but for the ghosts a weapon in `DropWaffe` ‰ and armour in
    /// `DropRuestung` ‰ of the rest; else a jewel, unless the creature made the last one
    /// ([`LastJewel`]); the type none where none is found.
    fn kind(&mut self, quality: i64, dice: &mut impl Dice) -> (Kind, Option<u32>) {
        let dead = self.death.dead;
        let [potion_chance, gold, weapon, armour] = self.rules.kinds;
        if dice.below(1000) < potion_chance {
            return (Kind::Potion, Some(potion(self.death.difficulty, dice)));
        }
        if let Some(rune) = self.rune(dice) {
            return rune;
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
        if self.last.jewel == Some(dead.id) {
            return (Kind::Jewel, None);
        }
        self.last.jewel = Some(dead.id);
        let category = JEWELS[index(dice.below(3))];
        let class = self.asked(dice);
        (
            Kind::Jewel,
            self.choose(category, class, false, quality, dice),
        )
    }

    /// A rune, unless the creature is an animal or made the last rune (`0x559BF0`): where a roll
    /// below 1,000 is below its chance ([`Rules::rune_chance`]), the creature makes the last
    /// rune, and the rune is one of the drops' runes at random, but a potion for the types 516,
    /// 592 and 593 or a rune whose entry has none (`0x43AF10`).
    fn rune(&mut self, dice: &mut impl Dice) -> Option<(Kind, Option<u32>)> {
        let dead = self.death.dead;
        if dead.class == ANIMALS || self.last.rune == Some(dead.id) {
            return None;
        }
        let roll = f32::from(u16::try_from(dice.below(1000)).unwrap_or(999)) * RUNE_SCALE;
        // The champions' mark gates it, as it gates the drop's chance (#621).
        let ranked = matches!(dead.rank, Rank::Boss | Rank::Champion);
        let chance = self.rules.rune_chance(dead.level, ranked, &self.death.hero) * RUNE_SCALE;
        if chance <= roll {
            return None;
        }
        self.last.rune = Some(dead.id);
        let runes = &self.catalogue.runes;
        let count = u32::try_from(runes.len()).unwrap_or(0);
        let rune = (count > 0)
            .then(|| runes.get(index(dice.below(count))).copied())
            .flatten();
        Some(match rune {
            Some(rune)
                if !NO_RUNES.contains(&dead.creature)
                    && self.catalogue.readable.contains(&rune) =>
            {
                (Kind::Rune, Some(rune))
            }
            _ => (Kind::Potion, Some(potion(self.death.difficulty, dice))),
        })
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
            kind: None,
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

    /// The drop of `item` by the key `key` (`0x559BF0`): the key 1 a set's piece in its place
    /// where one is taken ([`Self::set_piece`]), else the key 2; gold with its amount; potions,
    /// runes and the categories 26, 30 and 31 as they are; another item: a set's piece raised to
    /// `lo` ([`raised`]); the key 0 a unique of its family raised to `lo` ([`Self::unique`]),
    /// else with the key 14 at `R(lo, hi)`, or its record as it stands where its type has no
    /// template of its own; 2 to 15 with `16 − key` at `R(lo, hi)`, 16 with 0 at `lo`, 17 with 0
    /// at `R(⌊lo/2⌋, lo)`, the rolls at least 1.
    fn make(&self, kind: Kind, item: u32, key: u32, dice: &mut impl Dice) -> Dropped {
        let (kind, item, key) = match key {
            1 => match self.set_piece(dice) {
                Some(piece) => (self.catalogue.kind(piece).unwrap_or(kind), piece, 1),
                None => (kind, item, 2),
            },
            key => (kind, item, key),
        };
        let mut drop = Dropped {
            kind,
            item,
            level: 0,
            key: u8::try_from(key).unwrap_or(17),
            quality: 0,
            amount: 1,
            values: None,
            named: None,
            socketed: Vec::new(),
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
        let lo_level = u32::try_from(lo).unwrap_or(0);
        if key == 1 {
            raised(
                &mut drop,
                (self.catalogue, self.templates),
                (lo_level, self.death.difficulty),
            );
            return drop;
        }
        if key == 0 {
            match self.unique(item, dice) {
                Unique::Kept => return drop,
                Unique::Taken(unique) => {
                    drop.item = unique;
                    drop.kind = self.catalogue.kind(unique).unwrap_or(drop.kind);
                    raised(
                        &mut drop,
                        (self.catalogue, self.templates),
                        (lo_level, self.death.difficulty),
                    );
                    return drop;
                }
                Unique::Made => {}
            }
        }
        let (quality, level) = match key {
            0 => (14, at_least_one(lo, hi, dice)),
            16 => (0, lo),
            17 => (0, at_least_one(lo / 2, lo, dice)),
            key => (16 - key, at_least_one(lo, hi, dice)),
        };
        drop.quality = u8::try_from(quality).unwrap_or(0);
        drop.level = u32::try_from(level).unwrap_or(0);
        drop.values = made(
            (self.catalogue, self.templates),
            (item, drop.level, self.death.difficulty),
            dice,
        );
        socket(
            &mut drop,
            (self.catalogue, self.templates),
            (&self.rules.sockets, self.death.difficulty, false),
            dice,
        );
        drop.named = names::roll(drop.values.as_ref(), dice);
        drop
    }
}

/// The doll that the Medusa and the troll make (`V_Doll.grn`).
const DOLL: u32 = 3140;
/// The gold pile of a treasure's eight (`GoldCoins_02.grn`).
const TREASURE_GOLD: u32 = 5133;
/// The categories of a treasure's items (`0x5516B0`), by `rand() % 19`.
const TREASURE_CATEGORIES: [u8; 19] = [
    5, 5, 5, 5, 5, 6, 13, 17, 18, 19, 21, 22, 23, 24, 25, 20, 20, 8, 8,
];
/// The categories of the unique that Anducar's demon makes (`0x534248`), by `R(0, 21)`.
const UNIQUE_CATEGORIES: [u8; 22] = [
    5, 5, 5, 5, 5, 5, 5, 6, 13, 17, 18, 19, 21, 22, 23, 24, 25, 8, 8, 8, 20, 20,
];
/// The sets of Anducar's class items (`0x5340D1`), and each hero type's item.
const CLASS_SETS: std::ops::RangeInclusive<usize> = 33..=38;
const CLASS_ITEMS: [u32; 9] = [7921, 7920, 7924, 7923, 7922, 7925, 7925, 7926, 7927];

impl Dropping<'_> {
    /// A call of the drop whose type is forced, of no bonus and no greed (`0x559BF0(T, 0, 0, 0)`):
    /// its passes while its chance per mille lasts.
    fn forced_call(&mut self, item: u32, dice: &mut impl Dice) -> Vec<Dropped> {
        let (mut bonus, mut p) = (0, i64::from(self.rules.chance(self.death.dead.rank, 0)));
        let mut out = Vec::new();
        while p > 0 {
            out.push(self.forced(item, &mut bonus, dice));
            p -= 1000;
        }
        out
    }

    /// A boss's treasure (`0x5516B0`, #2269) of the chance `p` per mille, the keys `lo` to `hi`
    /// and the level's jitter: eight forced calls of a gold pile, then two rounds, each of which,
    /// by a roll below 1,000 below `p`, takes a type (`0x5C7FA0`: the level test left out in two
    /// cases of three, the hero's class in one case of two, a category of [`TREASURE_CATEGORIES`],
    /// the creature's own level, the quality at most `hi`) and makes it with the maker's key
    /// `R(lo, hi)` at the creature's level plus `R(0, jitter)`, `p` halved after, doubled on a
    /// miss; `lo` and `hi` halved after each round, at least 1.
    fn treasure(&mut self, [p, lo, hi, jitter]: [u32; 4], dice: &mut impl Dice) -> Vec<Dropped> {
        let mut out = Vec::new();
        for _ in 0..8 {
            out.extend(self.forced_call(TREASURE_GOLD, dice));
        }
        let (mut p, mut lo, mut hi) = (p, lo, hi);
        let dead = self.death.dead;
        for _ in 0..2 {
            if dice.below(1000) < p {
                let loose = dice.below(3) != 0;
                let class = if dice.below(2) == 0 {
                    self.death.hero.creature
                } else {
                    0
                };
                let category = TREASURE_CATEGORIES
                    [usize::try_from(dice.below(19)).unwrap_or(0) % TREASURE_CATEGORIES.len()];
                let wanted = Wanted {
                    category,
                    level: dead.level,
                    class,
                    loose,
                    quality: i64::from(hi),
                    kind: None,
                };
                if let Some(item) = self.catalogue.choose(wanted, &self.death.hero, dice) {
                    let key =
                        u32::try_from(between(i64::from(lo), i64::from(hi), dice)).unwrap_or(0);
                    let level = dead.level
                        + if jitter == 0 {
                            0
                        } else {
                            dice.below(jitter + 1)
                        };
                    out.extend(self.keyed(item, (level, key), dice));
                }
                p /= 2;
            } else {
                p = p.saturating_mul(2);
            }
            lo = if lo <= 1 { 1 } else { lo >> 1 };
            hi = if hi <= 1 { 1 } else { hi >> 1 };
        }
        out
    }

    /// An item of type `item` that the maker makes at `level` by its own key `key`, as a
    /// treasure makes it (`0x5D5080`), its sockets and its name rolled.
    fn keyed(&self, item: u32, (level, key): (u32, u32), dice: &mut impl Dice) -> Option<Dropped> {
        let kind = self.catalogue.kind(item)?;
        let mut drop = Dropped {
            kind,
            item,
            level,
            key: u8::try_from(16_u32.saturating_sub(key)).unwrap_or(0),
            quality: u8::try_from(key).unwrap_or(0),
            amount: 1,
            values: None,
            named: None,
            socketed: Vec::new(),
        };
        drop.values = made(
            (self.catalogue, self.templates),
            (item, level, self.death.difficulty),
            dice,
        );
        socket(
            &mut drop,
            (self.catalogue, self.templates),
            (&self.rules.sockets, self.death.difficulty, false),
            dice,
        );
        drop.named = names::roll(drop.values.as_ref(), dice);
        Some(drop)
    }

    /// Anducar's drop (`0x5340D1`, #2269): a set of `R(1, 65)`; of the sets 33 to 38 the class's
    /// item of the killer's type (a hero's, else `R(1, 9)`), else one of the set's pieces at
    /// `R(0, count − 1)`, each by a forced call.
    fn anducar(&mut self, dice: &mut impl Dice) -> Vec<Dropped> {
        let sets = self.catalogue.sets.len();
        let r = usize::try_from(between(1, i64::try_from(sets).unwrap_or(1), dice)).unwrap_or(1);
        let item = if CLASS_SETS.contains(&r) {
            let hero = match self.death.hero.creature {
                hero @ 1..=9 => hero,
                _ => u32::try_from(between(1, 9, dice)).unwrap_or(1),
            };
            CLASS_ITEMS
                .get(usize::try_from(hero - 1).unwrap_or(0))
                .copied()
        } else {
            self.catalogue.sets.get(r - 1).and_then(|pieces| {
                let last = i64::try_from(pieces.len()).unwrap_or(1) - 1;
                let piece = usize::try_from(between(0, last, dice)).unwrap_or(0);
                pieces.get(piece).copied()
            })
        };
        item.map_or_else(Vec::new, |item| self.forced_call(item, dice))
    }

    /// Anducar's demon's drop (`0x534248`, #2269): six treasures of the chance 1,000, the keys
    /// `q` (6, 9, 11, 12, 13, 14: `0x565D70(6, i, 4, 13)` rounded) to `min(q + R(1, 2), 14)` and
    /// the jitter 25; then, in 33 cases of 100, a unique: the killer's class by `R(0, 1)`, a
    /// category of [`UNIQUE_CATEGORIES`] by `R(0, 21)`, a type of any quality with the level test,
    /// and of its family's types of the quality 15, no set's piece, not 7911 to 7930 and of a
    /// least level up to the hero's, one at `R(0, n − 1)` by a forced call.
    fn anducar_demon(&mut self, dice: &mut impl Dice) -> Vec<Dropped> {
        let mut out = Vec::new();
        for i in 1..=6_u32 {
            let x = f64::from(i);
            let curve = 6.0 + 2.0 * (13.0 - 6.0) * (1.0 - 1.0 / (1.0 + (x - 1.0) / 4.0));
            let q = whole_level(curve + 0.5);
            let hi = (q + u32::try_from(between(1, 2, dice)).unwrap_or(1)).min(14);
            out.extend(self.treasure([1000, q, hi, 25], dice));
        }
        if between(0, 99, dice) >= 33 {
            return out;
        }
        let class = if between(0, 1, dice) == 0 {
            self.death.hero.creature
        } else {
            0
        };
        let category = UNIQUE_CATEGORIES
            [usize::try_from(between(0, 21, dice)).unwrap_or(0) % UNIQUE_CATEGORIES.len()];
        let wanted = Wanted {
            category,
            level: self.death.dead.level,
            class,
            loose: false,
            quality: 15,
            kind: None,
        };
        let Some(item) = self.catalogue.choose(wanted, &self.death.hero, dice) else {
            return out;
        };
        let Some([family, ..]) = self.templates.looked_up(item) else {
            return out;
        };
        let hero = self.death.hero.level;
        let uniques: Vec<u32> =
            self.catalogue
                .uniques
                .get(&family)
                .map_or(&[][..], Vec::as_slice)
                .iter()
                .copied()
                .filter(|&unique| {
                    !(7911..=7930).contains(&unique)
                        && self.catalogue.records.get(&unique).is_some_and(|record| {
                            !record.set && u32::from(record.levels[0]) <= hero
                        })
                })
                .collect();
        if uniques.is_empty() {
            return out;
        }
        let last = i64::try_from(uniques.len()).unwrap_or(1) - 1;
        let chosen = usize::try_from(between(0, last, dice)).unwrap_or(0);
        if let Some(&unique) = uniques.get(chosen) {
            out.extend(self.forced_call(unique, dice));
        }
        out
    }
}

/// What the key 0 makes of a drop's type ([`Dropping::unique`]).
enum Unique {
    /// Its type has no template of its own: its record as it stands.
    Kept,
    /// The maker makes it with the key 14.
    Made,
    /// A unique of its family in its place.
    Taken(u32),
}

impl Dropping<'_> {
    /// The set's piece that the key 1 takes in place of a drop's type (`0x55A8E7`–`0x55B2CB`,
    /// #2268), none for the key 2: in 60 cases of 100 of the sets whose first piece is of the
    /// local hero's class (its bit, every class's for another type), no icon (5101 to 5106), has
    /// a template ([`Templates::limits`], its word 38 not −1) and asks a least level up to the
    /// hero's, else of the sets with a piece that has a template; one at `rand() % n` (no roll
    /// for one); of its pieces with a template, each the hero wears kept by a roll below 50 of
    /// 100, in their order, one at `rand() % m`; taken where its own level times 0.8 lies below
    /// the hero's.
    fn set_piece(&self, dice: &mut impl Dice) -> Option<u32> {
        let hero = &self.death.hero;
        let catalogue = self.catalogue;
        let has_template = |item: u32| {
            self.templates
                .limits(item)
                .is_some_and(|[flags, ..]| flags != -1)
        };
        let mask = match crate::equipment::class_bit(hero.creature) {
            0 => CLASS_BITS,
            bit => bit,
        };
        let classed = dice.below(100) < 60;
        let sets: Vec<&Vec<u32>> = catalogue
            .sets
            .iter()
            .filter(|pieces| {
                if classed {
                    pieces.first().is_some_and(|&first| {
                        catalogue.records.get(&first).is_some_and(|record| {
                            record.flags & mask != 0
                                && !(5101..=5106).contains(&first)
                                && has_template(first)
                                && u32::from(record.levels[0]) <= hero.level
                        })
                    })
                } else {
                    pieces.iter().any(|&piece| has_template(piece))
                }
            })
            .collect();
        let set = sets.get(pick(sets.len(), dice)?)?;
        let mut pieces = Vec::new();
        for &piece in set.iter() {
            if !has_template(piece) {
                continue;
            }
            if !hero.worn.contains(&piece) || dice.below(100) < 50 {
                pieces.push(piece);
            }
        }
        let piece = *pieces.get(pick(pieces.len(), dice)?)?;
        let own = catalogue.records.get(&piece).map_or(0, |r| r.levels[1]);
        (f32::from(own) * 0.8 < to_f32(hero.level)).then_some(piece)
    }

    /// What the key 0 makes of a drop of type `item` (`0x55B598`–`0x55B952`, #2268): its record as
    /// it stands where the type has no template of its own; else of the uniques of its family
    /// (its template's word 38) not of 7911 to 7930, no set's piece and of a least level up to
    /// the hero's, by a roll below 30 of 100 one at `rand() % n` (no roll for one, none for no
    /// unique); else the maker's.
    fn unique(&self, item: u32, dice: &mut impl Dice) -> Unique {
        let Some(family) = self.templates.word_38(item) else {
            return Unique::Kept;
        };
        let hero = self.death.hero.level;
        let candidates: Vec<u32> =
            self.catalogue
                .uniques
                .get(&family)
                .map_or(&[][..], Vec::as_slice)
                .iter()
                .copied()
                .filter(|&unique| {
                    !(7911..=7930).contains(&unique)
                        && self.catalogue.records.get(&unique).is_some_and(|record| {
                            !record.set && u32::from(record.levels[0]) <= hero
                        })
                })
                .collect();
        if candidates.is_empty() || dice.below(100) >= 30 {
            return Unique::Made;
        }
        match pick(candidates.len(), dice).and_then(|i| candidates.get(i)) {
            Some(&unique) => Unique::Taken(unique),
            None => Unique::Made,
        }
    }
}

/// One of `n` at `rand() % n`, no roll for one, none for none.
fn pick(n: usize, dice: &mut impl Dice) -> Option<usize> {
    match n {
        0 => None,
        1 => Some(0),
        n => usize::try_from(dice.below(u32::try_from(n).unwrap_or(u32::MAX))).ok(),
    }
}

/// The level that a unique or a set's piece of its own level `own` is raised to at the drop's
/// `lo` (`0x55B7B9`, #2268): `own + n·d`, `d` half its level, at least 15, `n` the whole number
/// of steps nearest to `(max(lo, own) − own)/d`, a half up.
pub fn raise_level(own: u32, lo: u32) -> u32 {
    let half = f64::from(own) * 0.5;
    let d = if half >= 15.0 { half } else { 15.0 };
    let difference = f64::from(lo.max(own) - own);
    let steps = (difference / d + 0.5).trunc();
    whole_level(f64::from(own) + steps * d)
}

/// `x` truncated, held within a `u32`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a level, whole and within a u32"
)]
fn whole_level(x: f64) -> u32 {
    x.clamp(0.0, f64::from(u32::MAX)) as u32
}

/// A unique's or a set's piece's drop raised from its record to the level `lo` on `difficulty`
/// (`0x5C29A0`, [`Templates::raise`]): its values and its own level; its record as it stands
/// where its type has no template of its own. Its name is its type's.
fn raised(
    drop: &mut Dropped,
    (catalogue, templates): (&Catalogue, &Templates),
    (lo, difficulty): (u32, Difficulty),
) {
    let Some(values) = catalogue.owned.get(&drop.item) else {
        return;
    };
    let own = catalogue
        .records
        .get(&drop.item)
        .map_or(0, |record| u32::from(record.levels[1]));
    if let Some((values, level)) =
        templates.raise(drop.item, values, (own, raise_level(own, lo), difficulty))
    {
        drop.values = Some(values);
        drop.level = level;
    }
}

/// An item of type `item` as a test throws it (the game's command `item`, #525): of its kind by
/// its category, `amount` pieces of gold, a potion as it is, any other made by the item maker at
/// `level` with the pass's quality key `key` as a drop's would be (the key 0 at the maker's key
/// 14, 2 to 15 at `16 − key`, 16 and 17 at 0); a type of no drop's kind as it is, of the kind
/// [`Kind::Other`], as the original's driver makes any type (a book, a quest's item, #1358);
/// its sockets rolled by `sockets` on `difficulty`; a set's piece (#1701) and a unique (#1709) as
/// its record holds it, not raised to a least level (`0x5C29A0`, `0x5C2210`).
/// `None` for a type that the items' table does not hold.
pub fn thrown(
    (catalogue, templates): (&Catalogue, &Templates),
    (sockets, difficulty): (&Sockets, Difficulty),
    (item, level, key, amount): (u32, u32, u8, u32),
    dice: &mut impl Dice,
) -> Option<Dropped> {
    let kind = catalogue.kind(item).or_else(|| {
        catalogue
            .categories
            .contains_key(&item)
            .then_some(Kind::Other)
    })?;
    let mut drop = Dropped {
        kind,
        item,
        level: 0,
        key: key.min(17),
        quality: 0,
        amount: 1,
        values: None,
        named: None,
        socketed: Vec::new(),
    };
    match kind {
        Kind::Gold => drop.amount = amount.max(1),
        Kind::Potion | Kind::Rune | Kind::Other => {}
        // A set's piece and a unique raised from their records to the level, which the item maker
        // leaves out (seen in the original's forced drop: their own names, bonuses and sockets;
        // #2268).
        Kind::Weapon | Kind::Armour | Kind::Jewel
            if catalogue.set_piece(item) || catalogue.unique(item) =>
        {
            raised(&mut drop, (catalogue, templates), (level, difficulty));
        }
        Kind::Weapon | Kind::Armour | Kind::Jewel => {
            drop.quality = match drop.key {
                0 => 14,
                key @ 1..=15 => 16 - key.max(2),
                _ => 0,
            };
            drop.level = level.max(1);
            drop.values = made((catalogue, templates), (item, drop.level, difficulty), dice);
            socket(
                &mut drop,
                (catalogue, templates),
                (sockets, difficulty, false),
                dice,
            );
            drop.named = names::roll(drop.values.as_ref(), dice);
        }
    }
    Some(drop)
}

/// What a script's `CreateObj` makes of the item of type `item` that it lays, at the level
/// `level` ([`crate::fight::Rules::script_level`]) on `difficulty` (`0x487AB0`…`0x487EB8`,
/// docs/re/netscript.md, "The items' levels"), `None` where it leaves the item as its type's
/// record holds it: gold, a potion and the categories 26, 28, 30 and 31, and a type that the items
/// table does not hold. One of the tag `0x9E` (`raised_only`), or a unique (its own template's
/// word 40 15, else its record's quality bits all set, where it has no template), is raised from
/// its record to the level as a drop's unique is ([`raise_level`], `0x5C29A0`). Another is made
/// by the item maker at the level, of the key `min(16 − k, 10)` for a pass's key `k` of 2 to 15,
/// else 0, `k` drawn with no bonus and the factor of the share 1 ([`Rules::key`],
/// [`Rules::spread`]; the original's sum `(1 − ItemThrowFactor) + ItemThrowFactor`), its sockets
/// and its name rolled as a drop's; an item that the maker makes nothing of stays as it is.
pub fn scripted(
    (catalogue, templates): (&Catalogue, &Templates),
    (rules, difficulty): (&Rules, Difficulty),
    (item, level, raised_only): (u32, u32, bool),
    dice: &mut impl Dice,
) -> Option<Dropped> {
    let category = catalogue.category(item)?;
    if GOLD.contains(&item) || UNMADE.contains(&category) {
        return None;
    }
    let mut drop = Dropped {
        kind: catalogue.kind(item).unwrap_or(Kind::Other),
        item,
        level: 0,
        key: 16,
        quality: 0,
        amount: 1,
        values: None,
        named: None,
        socketed: Vec::new(),
    };
    let unique = templates
        .word_40(item)
        .map_or_else(|| catalogue.unique(item), |quality| quality == 15);
    if raised_only || unique {
        raised(&mut drop, (catalogue, templates), (level, difficulty));
        return Some(drop);
    }
    let k = rules.key(0, rules.spread(1.0), dice);
    // The item maker's fourth argument, which it never reads (`0x487E89`).
    dice.below(4);
    drop.quality = match k {
        2..=15 => u8::try_from(16 - k).unwrap_or(0).min(SCRIPTED_KEY),
        _ => 0,
    };
    drop.key = 16 - drop.quality;
    drop.level = level;
    drop.values = Some(made(
        (catalogue, templates),
        (item, level, difficulty),
        dice,
    )?);
    socket(
        &mut drop,
        (catalogue, templates),
        (rules.sockets(), difficulty, false),
        dice,
    );
    drop.named = names::roll(drop.values.as_ref(), dice);
    Some(drop)
}

/// The highest key of the item maker that a script's `CreateObj` makes an item of (`0x487E6C`).
const SCRIPTED_KEY: u8 = 10;

/// An item of type `item` that the item maker makes at `level` by its own key `key` (`0x5D5080`),
/// as a merchant's page makes it (`0x6D9DE0`, docs/re/trade.md): gold, potions, runes and the
/// categories 26, 30 and 31 as they are; another, a type of no drop's kind of [`Kind::Other`], by
/// the maker at `level`, held at no least, its sockets rolled by `sockets` on `difficulty` with
/// the merchant's bonus, and its name's roll; its pass's key `16 − key`. `None` for a type that
/// the items table does not hold.
pub fn keyed(
    (catalogue, templates): (&Catalogue, &Templates),
    (sockets, difficulty): (&Sockets, Difficulty),
    (item, level, key): (u32, u32, u8),
    dice: &mut impl Dice,
) -> Option<Dropped> {
    let category = catalogue.category(item)?;
    let kind = catalogue.kind(item).unwrap_or(Kind::Other);
    let mut drop = Dropped {
        kind,
        item,
        level: 0,
        key: 16_u8.saturating_sub(key),
        quality: key,
        amount: 1,
        values: None,
        named: None,
        socketed: Vec::new(),
    };
    if GOLD.contains(&item) || UNMADE.contains(&category) {
        drop.quality = 0;
        return Some(drop);
    }
    drop.level = level;
    drop.values = made((catalogue, templates), (item, level, difficulty), dice);
    socket(
        &mut drop,
        (catalogue, templates),
        (sockets, difficulty, true),
        dice,
    );
    drop.named = names::roll(drop.values.as_ref(), dice);
    Some(drop)
}

/// The values of an item of type `item` that the item maker makes at `level` on `difficulty`
/// ([`Templates::make`]), who may use it worked out from its triples
/// ([`Catalogue::made_classes`], #2266).
fn made(
    (catalogue, templates): (&Catalogue, &Templates),
    (item, level, difficulty): (u32, u32, Difficulty),
    dice: &mut impl Dice,
) -> Option<crate::fight::ItemValues> {
    let kept = catalogue.kept_classes(item);
    let mut values = templates.make(item, level, (difficulty, kept), dice)?;
    values.classes = catalogue.made_classes(item, values.classes);
    Some(values)
}

/// The sockets that the item maker's quality pass rolls for `drop`, made on `difficulty` for a
/// merchant's page where `merchant` (`0x5D2E40`, [`Sockets::roll`]), where the maker gave it
/// values: the pass runs after them and before the name, of its template's family.
fn socket(
    drop: &mut Dropped,
    (catalogue, templates): (&Catalogue, &Templates),
    (sockets, difficulty, merchant): (&Sockets, Difficulty, bool),
    dice: &mut impl Dice,
) {
    let Some(values) = drop.values.as_mut() else {
        return;
    };
    let family = templates.made_family(drop.item).unwrap_or(u8::MAX);
    let category = catalogue.category(drop.item).unwrap_or(0);
    values.sockets = sockets.roll((family, category), difficulty, merchant, dice);
    // The rest of the pass: the quality of the maker's key's value shared out (#2267).
    let making = quality::Making {
        item: drop.item,
        category,
        family,
        ranged: matches!(catalogue.weapon_kind(drop.item), 7 | 12 | 13 | 14),
        level: drop.level,
        difficulty,
        table: sockets.table(family),
        norms: templates.norms(),
    };
    let q = sockets.quality.value(u32::from(drop.quality));
    quality::pass(values, q, &making, &sockets.quality, dice);
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

/// The peak and the time of a container's throw to its opener (`0x5C9BA0`).
const ARC_PEAK: f32 = 80.0;
const ARC_SECONDS: f32 = 0.9;

/// How high a drop leaves, in units of the models' height, wherever the ground lies there
/// (`0x44D7B0`, `0x44D520`), and how fast it rises at first (`0x559BF0`).
pub const THROWN_AT: f32 = 60.0;
const RISES: f32 = 190.0;
/// How fast an item dropped off the cursor rises at first (`0x611620`).
const DROPPED_RISES: f32 = 50.0;
/// How fast a part thrown off rises at first (`0x550580`, the float `0x43160000`).
const FLUNG_RISES: f32 = 150.0;
/// The height's fall in a second of a drop's flight, in units of the models' height a second,
/// the share of its velocity along the ground that it loses in a second, and the share of that
/// velocity that it moves by (`0x44DFE0` and its caller).
const GRAVITY: f32 = 290.81;
const DRAG: f32 = 0.1;
const ALONG: f32 = 0.1;

/// A drop's flight (`0x44DED0`, `0x44DFE0`): where it is, a point of the ground in world units
/// and a height in units of the models' height; its velocity; its angles about the axes and
/// their spin, in radians and radians a second; and whether it rests on the ground.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Flight {
    pub at: [f32; 3],
    pub velocity: [f32; 3],
    pub angles: [f32; 3],
    pub spin: [f32; 3],
    pub resting: bool,
    /// A flight to a point ([`Flight::to_point`]) in place of the velocity's.
    pub arc: Option<Arc>,
    /// Whether its velocity along the ground turns back at every frame ([`Flight::step`]): a
    /// drop's, not a part's thrown off ([`Flight::flung`]).
    pub turns: bool,
}

/// A flight to a point (`0x44DED0` with −1000, `0x44DFE0`): from where it left to where it lands,
/// along the ground in `seconds`, its height over the ground `peak·(1 − 4(s − ½)²)` at the share
/// `s` of the time, and the time flown.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Arc {
    pub from: [f32; 2],
    pub to: [f32; 2],
    pub peak: f32,
    pub seconds: f32,
    pub flown: f32,
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
            arc: None,
            turns: true,
        }
    }

    /// A part that a creature throws off from the point `(x, y)`, as a skeleton warrior its head
    /// (`0x550580`): at the height 60, with the velocity `(10·R(−100, 100), 10·R(−100, 100),
    /// 150)` and the spin `R(−15, 15)` about each axis, drawn in that order.
    pub fn flung(x: f32, y: f32, dice: &mut impl Dice) -> Self {
        let mut roll = |half: u32| to_f32(dice.below(2 * half + 1)) - to_f32(half);
        let velocity = [10.0 * roll(100), 10.0 * roll(100), FLUNG_RISES];
        let spin = [roll(15), roll(15), roll(15)];
        Self {
            at: [x, y, THROWN_AT],
            velocity,
            angles: [0.0; 3],
            spin,
            resting: false,
            arc: None,
            turns: false,
        }
    }

    /// The weapon that a creature is disarmed of (`0x4E9900`, #2298), from the point `at` of its
    /// model, its height in the units of the models' height: with the velocity
    /// `(R(−100, 100), R(−100, 100), 150)` and the spin `R(−15, 15)` about each axis, drawn in
    /// that order.
    pub fn disarmed(at: [f32; 3], dice: &mut impl Dice) -> Self {
        let mut roll = |half: u32| to_f32(dice.below(2 * half + 1)) - to_f32(half);
        let velocity = [roll(100), roll(100), FLUNG_RISES];
        let spin = [roll(15), roll(15), roll(15)];
        Self {
            at,
            velocity,
            angles: [0.0; 3],
            spin,
            resting: false,
            arc: None,
            turns: false,
        }
    }

    /// An item that a container throws to its opener (`0x5C9BA0`, `0x44DED0(x, y, 0, −1000, 80,
    /// 0.9)`): from the container's point `from` on the ground to `to`, in 0.9 s along an arc
    /// that peaks at 80.
    pub fn to_point(from: [f32; 2], to: [f32; 2]) -> Self {
        Self {
            at: [from[0], from[1], 0.0],
            velocity: [0.0; 3],
            angles: [0.0; 3],
            spin: [0.0; 3],
            resting: false,
            arc: Some(Arc {
                from,
                to,
                peak: ARC_PEAK,
                seconds: ARC_SECONDS,
                flown: 0.0,
            }),
            turns: true,
        }
    }

    /// An item dropped off the cursor at the point `(x, y)` (`cEngine::dropItem`, `0x611620`): at
    /// the height 60, with the velocity `(R(−10, 10), R(−10, 10), 50)` and the spin `R(−6, 6)`
    /// about each axis.
    pub fn dropped(x: f32, y: f32, dice: &mut impl Dice) -> Self {
        let mut roll = |half: u32| to_f32(dice.below(2 * half + 1)) - to_f32(half);
        let velocity = [roll(10), roll(10), DROPPED_RISES];
        let spin = [roll(6), roll(6), roll(6)];
        Self {
            at: [x, y, THROWN_AT],
            velocity,
            angles: [0.0; 3],
            spin,
            resting: false,
            arc: None,
            turns: true,
        }
    }

    /// The flight's frame of `dt` seconds (`0x44DFE0`): the angles grow by the spin, the point by
    /// a tenth of the velocity along the ground and the height by the vertical velocity, each
    /// times `dt`; the velocity loses a tenth of itself along the ground and 290.81 along the
    /// height, times `dt`. Its velocity along the ground then turns back where the client's test
    /// of the cell it moves to (`0x636890`) fails, which it does at every frame of a drop's flight
    /// (seen in the original, #478; **seen**, #1932: the test answered 0 at every frame of a
    /// death's drops and 1 at those of a warrior's heads thrown off, why not read), so that the
    /// drop rises and falls within a unit of where it was thrown while a part thrown off flies on
    /// ([`Flight::turns`]); below the height its point
    /// stands at (`ground`, `0x449950`) it lies there without a velocity; it rests once each
    /// velocity is below 1 and it lies within 1 of the ground.
    pub fn step(&mut self, dt: f32, ground: impl Fn(f32, f32) -> f32) {
        if self.resting {
            return;
        }
        if let Some(arc) = &mut self.arc {
            arc.flown += dt;
            let s = (arc.flown / arc.seconds).min(1.0);
            let x = arc.from[0] + (arc.to[0] - arc.from[0]) * s;
            let y = arc.from[1] + (arc.to[1] - arc.from[1]) * s;
            let height = arc.peak * (1.0 - 4.0 * (s - 0.5) * (s - 0.5));
            self.at = [x, y, ground(x, y) + height];
            self.resting = s >= 1.0;
            return;
        }
        let [x, y, z] = self.at;
        let [vx, vy, vz] = self.velocity;
        self.at = [x + ALONG * vx * dt, y + ALONG * vy * dt, z + vz * dt];
        for (angle, spin) in self.angles.iter_mut().zip(self.spin) {
            *angle += spin * dt;
        }
        let turn = if self.turns { -1.0 } else { 1.0 };
        self.velocity = [
            turn * (vx - DRAG * vx * dt),
            turn * (vy - DRAG * vy * dt),
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
pub fn between(least: i64, most: i64, dice: &mut impl Dice) -> i64 {
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

/// The element stones (`Elementstein01.grn`, `0x428680`), which every mode collects.
const ELEMENT_STONES: std::ops::RangeInclusive<u32> = 6008..=6017;
/// The potions' and the runes' categories (`items.pak +0x01`, `0x426220`).
const POTION_CATEGORY: u8 = 9;
const RUNE_CATEGORY: u8 = 28;

/// Whether the collecting takes the item of type `item` and category `category`, its quality and
/// its sockets' kinds `(quality, sockets)` (`ItemValues`), for a hero of level `level`, in the
/// options' collect mode `mode`, `PICKUPAUTO`'s number (`0x4EC330`; docs/re/items.md): the
/// element stones always; with 1 gold, the runes, the bottles 3999 and 5058 and the large potions
/// 5137 to 5141 but no other potion, and another item by a bar of its quality or its sockets that
/// rises with the hero's level at 10, 25 and 35; with 2 gold alone; else everything.
pub fn collects(
    mode: u8,
    (item, category): (u32, u8),
    (quality, sockets): (u8, [u8; fight::SOCKETS]),
    level: u32,
) -> bool {
    if ELEMENT_STONES.contains(&item) {
        return true;
    }
    let gold = GOLD.contains(&item);
    match mode {
        1 => {
            if gold || category == RUNE_CATEGORY || matches!(item, 3999 | 5058 | 5137..=5141) {
                return true;
            }
            category != POTION_CATEGORY && above_the_bar(quality, sockets, level)
        }
        2 => gold,
        _ => true,
    }
}

/// Whether an item of `quality` and the first three sockets' kinds of `sockets` passes the
/// valuables' bar at the hero's `level` (`0x4EC330`): below 10 a quality above 6 or a socket;
/// below 25 above 9 or two; below 35 above 11 or two of which one above bronze; else above 12 or
/// three of which the first or the second above silver.
fn above_the_bar(quality: u8, sockets: [u8; fight::SOCKETS], level: u32) -> bool {
    let [a, b, c] = [sockets[0], sockets[1], sockets[2]];
    match level {
        0..=9 => quality > 6 || a != 0,
        10..=24 => quality > 9 || (a != 0 && b != 0),
        25..=34 => quality > 11 || (a != 0 && b != 0 && (a > 1 || b > 1)),
        _ => quality > 12 || (a != 0 && b != 0 && c != 0 && (a > 2 || b > 2)),
    }
}

#[cfg(test)]
mod tests;
