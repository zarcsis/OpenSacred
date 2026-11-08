//! The item maker's quality pass (`0x5D2E40`, which `0x5D5080` calls after the maker's values,
//! the triples and the magic bonus; docs/re/items.md, "The quality pass"; #2267): the quality of
//! the fresh record by its bonuses (`0x42BF40`), its sockets ([`super::Sockets::roll`]), and the
//! map value `q` of the maker's key shared out as steps among up to four more bonus slots, each
//! of a kind drawn ([`kind_of`]) and given its effect by the kind (`0x5D1750`) and its value by
//! its steps (`0x5D1370`); its quality then the value it reached, at most 14.

use opensacred_data::tables::Balance;

use super::Difficulty;
use crate::fight::magic::{LISTS, art_hero};
use crate::fight::{Bonus, ItemValues};
use crate::spawning::Dice;

/// The quality pass's values of the balance: the map of the maker's keys to their values (the
/// third table of the tail of `Balance.bin`, `0xAD57C8`), what a socket of each kind adds to the
/// quality's factor (`RareSlotBronze`, `RareSlotSilber`, `RareSlotGold`), and the factors of a
/// bonus's condition (`RareGegen`, an opponent's class; `RareHeroReiter`, the class 1;
/// `RareDurch`, a hero type's).
#[derive(Clone, Debug, PartialEq)]
pub struct Quality {
    map: Vec<(u32, u32)>,
    slots: [f32; 3],
    against: f32,
    rider: f32,
    through: f32,
}

impl Default for Quality {
    /// The original's values.
    fn default() -> Self {
        Self {
            map: (0..8)
                .map(|key| (key, key))
                .chain([
                    (8, 9),
                    (9, 11),
                    (10, 13),
                    (11, 15),
                    (12, 18),
                    (13, 21),
                    (14, 25),
                    (15, 25),
                ])
                .collect(),
            slots: [0.1; 3],
            against: 0.6,
            rider: 1.0,
            through: 1.0,
        }
    }
}

impl Quality {
    /// The values of `balance`, the original's where it lacks them.
    pub fn of(balance: Option<&Balance>) -> Self {
        let default = Self::default();
        let Some(b) = balance else {
            return default;
        };
        let mut map: Vec<(u32, u32)> = b
            .quality_passes
            .iter()
            .map(|pass| (pass.key, pass.value))
            .collect();
        map.sort_unstable();
        let f = |name: &str, or: f32| b.f32(name).unwrap_or(or);
        Self {
            map: if map.is_empty() { default.map } else { map },
            slots: [
                f("RareSlotBronze", default.slots[0]),
                f("RareSlotSilber", default.slots[1]),
                f("RareSlotGold", default.slots[2]),
            ],
            against: f("RareGegen", default.against),
            rider: f("RareHeroReiter", default.rider),
            through: f("RareDurch", default.through),
        }
    }

    /// The map's value of the key `key` (`0x63EA90`, a missing key 0).
    pub fn value(&self, key: u32) -> u32 {
        self.map
            .iter()
            .find(|&&(k, _)| k == key)
            .map_or(0, |&(_, value)| value)
    }

    /// The quality of the weight `t` (`0x42BF40`): one less than the index of the first key whose
    /// value reaches it, at least 0, 14 where none does.
    fn quality_of(&self, t: f64) -> u8 {
        match self
            .map
            .iter()
            .position(|&(_, value)| f64::from(value) >= t)
        {
            Some(index) => u8::try_from(index.saturating_sub(1)).unwrap_or(14),
            None => 14,
        }
    }
}

/// What the pass reads of a made item besides its values: its type, its category, its family
/// (its template's word 38's low byte), whether it is a ranged weapon (`Weapon.pak`'s group 7,
/// 12, 13 or 14), its level (`+0x19`), the game's difficulty and its family's list of the sockets
/// and the effects (`0x440470`: the first record `(−1, −1, n)`, then effect ranges).
pub struct Making<'a> {
    pub item: u32,
    pub category: u8,
    pub family: u8,
    pub ranged: bool,
    pub level: u32,
    pub difficulty: Difficulty,
    pub table: &'a [[u32; 3]],
    pub norms: &'a [[f32; 6]],
}

/// The categories that the pass tests (`0x428490`, `0x428540`, `0x426740`, `0x427A40`,
/// `0x427CD0`): the books, the rings, the amulets, the bridles, the weapons, the shields.
const BOOKS: u8 = 16;
const RINGS: u8 = 8;
const AMULETS: u8 = 20;
const BRIDLES: u8 = 29;
const WEAPONS: u8 = 5;
const SHIELDS: u8 = 13;

/// The class bits that make an item class-restricted where not every class is set, and every
/// class's.
const RESTRICTING: u32 = 0x2A;
const DWARF: u32 = 0x40_0000;
const EVERY: u32 = 0x00C0_003F;

impl Making<'_> {
    fn ring(&self) -> bool {
        matches!(self.category, RINGS | AMULETS)
    }

    /// The horse's gear: the bridles and the bits 3197 to 3204 and 3245 to 3259 (`0x43AFC0`).
    fn horse(&self) -> bool {
        self.category == BRIDLES || matches!(self.item, 3197..=3204 | 3245..=3259)
    }

    fn record(&self, effect: u32) -> [f32; 6] {
        super::norm(self.norms, effect & 0xFFFF)
            .copied()
            .unwrap_or_default()
    }
}

/// The kind of an effect (`0x5D2D70`): 0 the damages 801–804 (and anything not named below), 1 the
/// multipliers 805–808, 2 `A` and `D` 809–810, 3 the speeds 811–812, 4 813–814, 5 the attributes
/// 815–820, 6 the spells and the arts (up to 200, the empty slot among them, and 1000 to 1200),
/// 7 the skills 600–798, 8 the special effects 840–869.
pub fn kind_of(effect: u32) -> u8 {
    match effect & 0xFFFF {
        801..=804 => 0,
        805..=808 => 1,
        809..=810 => 2,
        811..=812 => 3,
        813..=814 => 4,
        815..=820 => 5,
        0..=200 | 1000..=1200 => 6,
        600..=798 => 7,
        840..=869 => 8,
        _ => 0,
    }
}

/// The factor of a bonus of the condition `condition` (`0x42BF40`, `0x5D1370`): `RareDurch` for a
/// hero type's (bits 8 to 11), times `RareHeroReiter` for the class 1 or `RareGegen` for another
/// opponent's class, times the record's sixth float for a ring or an amulet, times the
/// difficulty's term `term`.
fn factor(quality: &Quality, condition: u16, (ring, r5): (bool, f32), term: f64) -> f64 {
    let mut phi = if condition & 0x0F00 != 0 {
        f64::from(quality.through)
    } else {
        1.0
    };
    match condition & 0xFF {
        0 => {}
        1 => phi *= f64::from(quality.rider),
        _ => phi *= f64::from(quality.against),
    }
    if ring {
        phi *= f64::from(r5);
    }
    phi * term
}

/// `x` truncated toward zero (`_ftol`), as a whole number of the bonus's short.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a bonus's value, truncated as the original's _ftol"
)]
fn trunc(x: f64) -> i64 {
    x.trunc() as i64
}

/// A whole number as a float, as the x87 unit loads it.
#[expect(
    clippy::cast_precision_loss,
    reason = "a count of steps or a bonus's value, well within a float's integers"
)]
fn float(x: i64) -> f64 {
    x as f64
}

/// `x` stored as a single float, as the original stores the term.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the original stores the product as a float"
)]
fn single(x: f64) -> f64 {
    f64::from(x as f32)
}

/// A whole number as a bonus's short, held within it.
fn short(x: i64) -> i16 {
    i16::try_from(x.clamp(i64::from(i16::MIN), i64::from(i16::MAX))).unwrap_or(0)
}

/// The quality of a fresh made record (`0x42BF40`, before the pass): each bonus slot of an
/// effect counted, the first only for a ring or an amulet or an effect of 840 to 870, its value
/// held at its record's cap and weighed as its steps (`v·φ/r₁`, `v·φ/r₄`, or `v·φ/((ℓ + 9)·r₄)`
/// by its record's branch, at least 1); their sum times one and the sockets' parts gives the
/// quality ([`Quality::quality_of`]); the record is fresh, of no sockets yet, which would raise it.
fn estimate(values: &mut ItemValues, making: &Making, quality: &Quality) -> u8 {
    let ring = making.ring();
    let level = f64::from(making.level) + 9.0;
    let mut sum = 0.0;
    for (slot, bonus) in values.bonuses.iter_mut().enumerate() {
        let effect = bonus.effect;
        if effect == 0 {
            continue;
        }
        if !(slot != 0 || ring || (840..=870).contains(&(effect & 0xFFFF))) {
            continue;
        }
        let r = making.record(effect);
        let phi = factor(quality, bonus.condition, (ring, r[5]), 1.0);
        let mut v = f64::from(bonus.value);
        let by_level = effect & 0xFF_0000 == 0 && r[2] >= 1e-5;
        let (cap, step) = if effect & 0xFF_0000 != 0 {
            (f64::from(r[0]), f64::from(r[1]))
        } else if !by_level {
            (f64::from(r[3]), f64::from(r[4]))
        } else {
            (level * f64::from(r[2]), level * f64::from(r[4]))
        };
        let over = if by_level {
            v > (cap / phi).trunc()
        } else {
            cap.trunc() / phi < v
        };
        if over {
            v = (cap / phi + 0.5).trunc();
        }
        if v == 0.0 {
            v = 1.0;
        }
        bonus.value = short(trunc(v));
        if step > 0.01 {
            sum += (v * phi / step + 0.5).max(1.0);
        }
    }
    // The fresh record has no sockets yet, so their parts weigh nothing here.
    quality.quality_of(sum)
}

/// Whether `steps` buy one unit of the effect `effect` (`0x5D1660`): an attribute's scaled effect
/// by its record's second float, another by its fifth (`g/r₄`), or by the level's (`1/((ℓ + 9)·r₄·g)`)
/// where its third is not below `10⁻⁵`, `g` the sixth float for a ring or an amulet, else 1;
/// always where that float is not above 0.01.
fn buys(effect: u32, steps: u32, making: &Making) -> bool {
    let r = making.record(effect);
    let g = if making.ring() { f64::from(r[5]) } else { 1.0 };
    let threshold = if effect & 0xFFFF_0000 != 0 {
        if r[1] <= 0.01 {
            return true;
        }
        g / f64::from(r[1])
    } else {
        if r[4] <= 0.01 {
            return true;
        }
        if r[2] < 1e-5 {
            g / f64::from(r[4])
        } else {
            1.0 / ((f64::from(making.level) + 9.0) * f64::from(r[4]) * g)
        }
    };
    f64::from(steps) >= threshold
}

/// The difficulty's term of a value's worth (`0x5D1370`, the global difficulty): 1, then
/// 1/1.33, 1/1.66, 0.5 and 1/2.33 (the floats at `0x890C10`, `0x890C14`, `0x890C0C`).
fn term(difficulty: Difficulty) -> f64 {
    f64::from(match difficulty {
        Difficulty::Bronze => 1.0_f32,
        Difficulty::Silver => 0.751_88,
        Difficulty::Gold => 0.602_41,
        Difficulty::Platinum => 0.5,
        Difficulty::Niobium => 0.429_19,
    })
}

/// The value that `steps` give the bonus slot `slot` and its whole worth in steps (`0x5D1370`):
/// by its record's branch, its value grown by `min(cap, r·steps)/φ + 0.5`, at least 1, the cap
/// its record's first, fourth or `(ℓ + 9)·r₂` (the level's value first held at it), the step its
/// second, fifth or `(ℓ + 9)·r₄` float; its worth `v·φ/step + 0.5`; 0 for a slot of 8 or more, and
/// for a record whose fifth float is not above 0.01 (its value as it was); a value of 0 becomes 1.
fn worth(
    values: &mut ItemValues,
    slot: usize,
    steps: i64,
    making: &Making,
    quality: &Quality,
) -> i64 {
    let Some(bonus) = values.bonuses.get_mut(slot) else {
        return 0;
    };
    let r = making.record(bonus.effect);
    let phi = factor(
        quality,
        bonus.condition,
        (making.ring(), r[5]),
        term(making.difficulty),
    );
    let mut v = f64::from(bonus.value);
    let steps = float(steps);
    let level = f64::from(making.level) + 9.0;
    let result = if r[1] > 0.01 {
        let grown = (f64::from(r[0]) / phi).min(single(f64::from(r[1]) * steps / phi)) + 0.5;
        v = (v + grown.max(1.0)).trunc();
        trunc(v * phi / f64::from(r[1]) + 0.5)
    } else if r[4] <= 0.01 {
        if bonus.value == 0 {
            bonus.value = 1;
        }
        return 0;
    } else if r[2] < 1e-5 {
        let grown = (f64::from(r[3]) / phi).min(single(f64::from(r[4]) * steps / phi)) + 0.5;
        v = (v + grown.max(1.0)).trunc();
        trunc(v * phi / f64::from(r[4]) + 0.5)
    } else {
        v = v.min((level * f64::from(r[2]) / phi).trunc());
        let grown =
            (level * f64::from(r[2]) / phi).min(level * steps * f64::from(r[4]) / phi) + 0.5;
        v = (v + grown.max(1.0)).trunc();
        trunc(v * phi / (level * f64::from(r[4])) + 0.5)
    };
    if v == 0.0 {
        v = 1.0;
    }
    bonus.value = short(trunc(v));
    result
}

/// One of `n` at `rand() % n`, none drawn for one.
fn pick(n: usize, dice: &mut impl Dice) -> usize {
    if n <= 1 {
        0
    } else {
        usize::try_from(dice.below(u32::try_from(n).unwrap_or(u32::MAX))).unwrap_or(0)
    }
}

/// Draws from `pool` as the original does (`0x5D1750`): an index at `rand() % n`, the element
/// erased and then the one at that index read, its successor, or the drawn one where it was the
/// last.
fn draw(pool: &mut Vec<u32>, dice: &mut impl Dice) -> u32 {
    let index = pick(pool.len(), dice);
    let drawn = pool.remove(index);
    pool.get(index).copied().unwrap_or(drawn)
}

/// Gives the bonus slot `slot` an effect of the kind `kind` that `steps` buy (`0x5D1750`), its
/// condition cleared first and its value left: false where none is found.
fn give(
    kind: u8,
    values: &mut ItemValues,
    (slot, steps): (usize, u32),
    making: &Making,
    dice: &mut impl Dice,
) -> bool {
    let Some(bonus) = values.bonuses.get_mut(slot) else {
        return false;
    };
    bonus.condition = 0;
    let flags = values.classes;
    let every = flags & EVERY == EVERY;
    let restricted = flags & (RESTRICTING | DWARF) != 0 && !every;
    let write = |values: &mut ItemValues, effect: u32, condition: u16| {
        if let Some(bonus) = values.bonuses.get_mut(slot) {
            bonus.effect = effect;
            bonus.condition = condition;
        }
    };
    let single = |values: &mut ItemValues, effect: u32| {
        let bought = buys(effect, steps, making);
        if bought {
            write(values, effect, 0);
        }
        bought
    };
    match kind {
        0 | 1 => {
            let base = if kind == 0 { 801 } else { 805 };
            let mut pool: Vec<u32> = (0..4_u32)
                .filter(|&k| {
                    let k = usize::try_from(k).unwrap_or(0);
                    values.damage[k][1] != 0.0 || values.resistance[k] != 0.0
                })
                .map(|k| base + k)
                .collect();
            let resisting = values.resistance.iter().sum::<f32>() != 0.0;
            let mut given = false;
            while !pool.is_empty() {
                let mut effect = draw(&mut pool, dice);
                if kind == 0 {
                    let mut attribute = dice.below(6) + 1;
                    if attribute == 5 && flags & RESTRICTING != 0 {
                        attribute = dice.below(5) + 1;
                        if attribute == 5 {
                            attribute = 6;
                        }
                    }
                    effect |= attribute << 16;
                }
                if buys(effect, steps, making) {
                    write(values, effect, if resisting { 0x8000 } else { 0 });
                    given = true;
                }
            }
            given
        }
        2 => single(values, 809 + dice.below(2)),
        3 => single(values, 811 + dice.below(2)),
        4 => {
            let effect = if restricted || making.family == 6 {
                814
            } else if flags & 4 != 0 && !every {
                813
            } else {
                813 + dice.below(2)
            };
            single(values, effect)
        }
        5 => {
            let mut effect = 815 + dice.below(6);
            if (effect == 819 && restricted) || making.family == 6 {
                effect = if dice.below(5) < 4 {
                    815 + dice.below(4)
                } else {
                    820
                };
            }
            single(values, effect)
        }
        6 => {
            let c = dice.below(2);
            let list = if every {
                if making.ranged && values.skill[0] == 7 {
                    let d = dice.below(2);
                    match (c, d) {
                        (1, 0) => 17,
                        (1, _) => 19,
                        (_, 0) => 18,
                        _ => 20,
                    }
                } else if c == 1 {
                    14
                } else {
                    15
                }
            } else {
                match [2, 0x10, 8, 4, 0x20, 1]
                    .iter()
                    .find(|&&bit| flags & bit != 0)
                {
                    Some(2) => 16,
                    Some(0x10) => {
                        if c == 1 {
                            19
                        } else {
                            20
                        }
                    }
                    Some(8) => {
                        if c == 1 {
                            22
                        } else {
                            21
                        }
                    }
                    Some(4) => 25,
                    Some(0x20) => {
                        if c == 1 {
                            23
                        } else {
                            24
                        }
                    }
                    Some(1) => {
                        if c == 1 {
                            17
                        } else {
                            18
                        }
                    }
                    _ => return false,
                }
            };
            let pool = LISTS.get(list).copied().unwrap_or(&[]);
            let Some(&effect) = pool.get(pick(pool.len(), dice)) else {
                return false;
            };
            if !buys(effect, steps, making) {
                return false;
            }
            let hero = art_hero(effect);
            let condition = if every && hero != 0 {
                u16::from(hero) << 8
            } else {
                0
            };
            write(values, effect, condition);
            true
        }
        7 => {
            let lists: Vec<usize> = [
                (2, 6),
                (0x10, 9),
                (8, 8),
                (4, 7),
                (0x20, 11),
                (1, 10),
                (0x40_0000, 12),
                (0x80_0000, 13),
            ]
            .iter()
            .filter(|&&(bit, _)| flags & bit != 0)
            .map(|&(_, list)| list)
            .collect();
            if lists.is_empty() {
                return false;
            }
            let list = lists[pick(lists.len(), dice)];
            let signed = |w: u32| i64::from(i32::from_ne_bytes(w.to_ne_bytes()));
            let pool: Vec<u32> = LISTS
                .get(list)
                .copied()
                .unwrap_or(&[])
                .iter()
                .copied()
                .filter(|&effect| match making.category {
                    WEAPONS => making.table.iter().any(|&[lo, hi, _]| {
                        let (lo, hi, e) = (signed(lo), signed(hi), i64::from(effect));
                        lo == e || (lo < e && e <= hi)
                    }),
                    SHIELDS => effect != 605,
                    _ => true,
                })
                .collect();
            let Some(&effect) = pool.get(pick(pool.len(), dice)) else {
                return false;
            };
            single(values, effect)
        }
        8 => {
            let mut pool = Vec::new();
            for &[lo, hi, _] in making.table {
                let signed = |w: u32| i64::from(i32::from_ne_bytes(w.to_ne_bytes()));
                let (lo, hi) = (signed(lo), signed(hi));
                if !(lo > 839 && (hi == -1 || hi < 870) && lo <= hi) {
                    continue;
                }
                for effect in lo..=hi {
                    let Ok(effect) = u32::try_from(effect) else {
                        continue;
                    };
                    let kept = match effect {
                        858 | 860..=863 => false,
                        844 => flags & RESTRICTING == 0,
                        845 => flags & 4 == 0,
                        850 => flags & 0x20 != 0,
                        851 => flags & 0x20 == 0,
                        _ => true,
                    };
                    if kept {
                        pool.push(effect);
                    }
                }
            }
            while !pool.is_empty() {
                let effect = draw(&mut pool, dice);
                if buys(effect, steps, making) {
                    write(values, effect, 0);
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}

/// The quality pass of a made item of the values `values` by the map value `q` of the maker's key
/// (`0x5D2E40`), its sockets `sockets` rolled already: its quality (`ItemValues::quality`).
/// The fresh record's quality `v₀` ([`estimate`]), times one and each socket's part, is `Q`;
/// `q` of 0 clears the bonus slots but the first (all of them for a ring or an amulet); else,
/// `Q` below `q`, the difference is shared out as steps among up to four new slots (each slot's
/// step by a roll below 50 of 100, always at four, a new slot where none took one, a step costing
/// the sockets' factor `m`; a ring, an amulet or the horse's gear one slot of them all), their
/// kinds drawn (the first's kind of the special effects by a roll below 1,000 below `20·q`, a
/// ring's by one of two, else each another kind than the first slot's and those drawn) and
/// sorted, and each slot given its effect ([`give`]) and its value, `Q` growing by its worth
/// times `m` and a half, until it reaches `q`; what is left raises the slot of the kind 1, or
/// slips a bonus of the kind 0 or 1 in at the second slot; the quality is `Q` truncated, at most
/// 14. A book is left alone.
pub fn pass(
    values: &mut ItemValues,
    q: u32,
    making: &Making,
    quality: &Quality,
    dice: &mut impl Dice,
) {
    if making.category == BOOKS {
        return;
    }
    let v0 = quality.value(u32::from(estimate(values, making, quality)));
    let m = 1.0
        + values
            .sockets
            .iter()
            .filter_map(|&kind| match kind {
                1..=3 => quality.slots.get(usize::from(kind - 1)).copied(),
                _ => None,
            })
            .map(f64::from)
            .sum::<f64>();
    let mut total = m * f64::from(v0);
    let q_value = f64::from(q);
    let ring = making.ring();
    if q == 0 {
        let first = usize::from(!ring);
        for bonus in &mut values.bonuses[first..] {
            *bonus = Bonus::default();
        }
    } else if total < q_value {
        let share = trunc(q_value - total);
        if values.bonuses[0].effect == 0 {
            values.bonuses[0].effect = 820;
            values.bonuses[0].value = 1;
        }
        // The elements: each a slot and its steps.
        let mut elements: Vec<(usize, i64)> = Vec::new();
        if ring || making.horse() {
            elements.push((1, share));
        } else {
            let mut left = float(share);
            while left > 0.0 {
                let four = elements.len() == 4;
                let mut took = false;
                for element in &mut elements {
                    if dice.below(100) < 50 || four {
                        element.1 += 1;
                        left -= m;
                        took = true;
                    }
                }
                if !took {
                    elements.push((elements.len() + 1, 1));
                    left -= m;
                }
            }
        }
        let first = kind_of(values.bonuses[0].effect);
        let mut kinds: Vec<u8> = Vec::new();
        if making.horse() {
            loop {
                let kind = u8::try_from(dice.below(4)).unwrap_or(0);
                if kind != first {
                    kinds.push(kind);
                    break;
                }
            }
        } else {
            if ring && dice.below(100) < 50 {
                kinds.push(8);
            }
            if kinds.is_empty() {
                for j in 0..elements.len() {
                    if j == 0 && !ring {
                        let roll = dice.below(1000);
                        let w14 = quality.value(14).max(1);
                        if q * 1000 / (2 * w14) > roll {
                            kinds.push(8);
                            continue;
                        }
                    }
                    loop {
                        let kind = u8::try_from(dice.below(8)).unwrap_or(0);
                        if kind != first && !kinds.contains(&kind) {
                            kinds.push(kind);
                            break;
                        }
                    }
                }
            }
        }
        kinds.sort_unstable();
        let mut fails = 0;
        let mut kind_1: Option<usize> = None;
        for (&(slot, steps), &kind) in elements.iter().zip(&kinds) {
            if total >= q_value {
                break;
            }
            let slot = slot.saturating_sub(fails).max(1);
            if kind == 1 {
                kind_1 = Some(slot);
            }
            let steps_u = u32::try_from(steps).unwrap_or(u32::MAX);
            if give(kind, values, (slot, steps_u), making, dice) {
                values.bonuses[slot].value = 0;
                total += float(worth(values, slot, steps, making, quality)) * m + 0.5;
            } else {
                fails += 1;
            }
        }
        let rest = trunc(q_value - total);
        if rest > 0 {
            let divided = trunc(float(rest) / m);
            if let Some(slot) = kind_1 {
                total += float(worth(values, slot, rest, making, quality));
            } else if first == 1 && kind_of(values.bonuses[1].effect) == 0 {
                total += float(worth(values, 0, divided, making, quality));
            } else {
                let (kind, worth_slot) = if first == 1 { (0, 0) } else { (1, 1) };
                let saved = values.bonuses[1];
                values.bonuses[1].condition = 0;
                let rest_u = u32::try_from(rest).unwrap_or(u32::MAX);
                if give(kind, values, (1, rest_u), making, dice) {
                    values.bonuses.copy_within(2..6, 3);
                    values.bonuses[2] = saved;
                    values.bonuses[1].value = 0;
                    total += float(worth(values, worth_slot, divided, making, quality));
                }
            }
        }
    }
    values.quality = u8::try_from(trunc(total.min(14.0)).max(0)).unwrap_or(0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spawning::Rng;

    /// The effects' records of the balance for the damages, the multipliers and the attributes.
    fn norms() -> Vec<[f32; 6]> {
        let mut norms = vec![[100.0, 10.0, 2.5, 0.0, 1.0, 5.0]; 74];
        norms[1..=4].fill([40.0, 3.5, 1.5, 0.0, 0.12, 5.0]);
        norms[5..=8].fill([0.0, 0.0, 0.0, 55.0, 5.0, 5.0]);
        norms[15..=20].fill([15.0, 1.0, 0.2, 0.0, 0.017, 3.0]);
        norms
    }

    fn making(norms: &[[f32; 6]]) -> Making<'_> {
        Making {
            item: 1712,
            category: WEAPONS,
            family: 0,
            ranged: false,
            level: 1,
            difficulty: Difficulty::Bronze,
            table: &[],
            norms,
        }
    }

    #[test]
    fn the_kinds_and_the_worth_of_the_steps_are_the_originals() {
        assert_eq!(
            [801, 806, 810, 811, 814, 820, 0, 1043, 612, 859, 830].map(kind_of),
            [0, 1, 2, 3, 4, 5, 6, 6, 7, 8, 0]
        );
        let norms = norms();
        let making = making(&norms);
        let quality = Quality::default();
        // Bronze, 801 of 3 steps: min(40, 10.5) + 0.5, the value 11, its worth 11/3.5 + 0.5.
        let mut values = ItemValues::default();
        values.bonuses[1].effect = 801;
        assert_eq!(worth(&mut values, 1, 3, &making, &quality), 3);
        assert_eq!(values.bonuses[1].value, 11);
        // An attribute needs 58.8/(level + 9) steps: 6 at level 1, 5 not.
        assert!(buys(815, 6, &making) && !buys(815, 5, &making));
        // The map: 9 is the quality 7, 25.5 the most.
        assert_eq!((quality.quality_of(9.0), quality.quality_of(25.5)), (7, 14));
    }

    #[test]
    fn a_made_item_takes_bonuses_and_a_quality_by_its_keys_value() {
        let norms = norms();
        let making = making(&norms);
        let quality = Quality::default();
        let mut values = ItemValues::default();
        values.damage[0] = [12.0, 15.0];
        values.classes = EVERY;
        let mut rng = Rng::of(5, 7, 11);
        pass(&mut values, 9, &making, &quality, &mut rng);
        // The first slot made 820, more slots given, the quality of what they are worth.
        assert_eq!(values.bonuses[0].effect, 820);
        assert!(values.bonuses[1..].iter().any(|bonus| bonus.effect != 0));
        assert!((1..=14).contains(&values.quality), "{values:?}");
        // The key's value 0 clears the slots but the first.
        let mut plain = values;
        pass(&mut plain, 0, &making, &quality, &mut rng);
        assert!(plain.bonuses[1..].iter().all(|bonus| bonus.effect == 0));
    }
}
