//! The worth of an item (#1621; docs/re/items.md, "The worth of an item"): what `0x5D3AB0`
//! answers, by the item's values and the balance's `Price…` keys, which the price takes where
//! the item has no price of its own (`0x43C350`, [`super::shown`]).

use opensacred_data::tables::Balance;

use crate::drops;
use crate::fight::{Bonus, ItemValues, KINDS};

/// The potions', the books', the rings', the amulets' and the runes' categories.
const POTIONS: u8 = 9;
const BOOKS: u8 = 16;
const RINGS: u8 = 8;
const AMULETS: u8 = 20;
const RUNES: u8 = 28;
/// A book's worth without a price of its own or of its record (`0x5D3D36`).
const BOOK: i64 = 600;
/// The share of a rune's worth (`0x891130`, the float a little below 0.02).
const RUNE_SHARE: f32 = 0.02;
/// The smallest float the bonuses' test takes as more than 0 (`0x890C9C`, 2⁻²³).
const EPSILON: f32 = 1.192_092_9e-7;
/// The effects of fixed worths or worths by their values (`0x5D43BE`'s table, from 840 to 867):
/// each its balance's key and whether the value multiplies it.
const SPECIAL: [Option<(&str, bool)>; 28] = [
    Some(("840_Glckskind", true)),
    Some(("841_Blutdurst", false)),
    Some(("842_Schmerz", false)),
    Some(("843_erkenntniss", false)),
    Some(("844_Weisheit", true)),
    Some(("845_Konzentration", true)),
    Some(("846_Zerteilen", false)),
    Some(("847_Schutzgeist", true)),
    Some(("848_Agilitat", true)),
    Some(("849_Erprobung", true)),
    Some(("850_Nachtschicht", false)),
    Some(("851_Tag", false)),
    Some(("852_schreken", true)),
    Some(("853_Weitsicht", false)),
    Some(("854_Knockback", false)),
    Some(("855_Fluggegner", false)),
    Some(("856_Diebesgut", false)),
    Some(("857_OffeneWunden", false)),
    None,
    Some(("859_Kritischer", false)),
    None,
    None,
    None,
    None,
    Some(("864_Metzger", false)),
    Some(("865_licht", false)),
    Some(("866_Gold_statt_Leben", true)),
    Some(("867_Tollwut", false)),
];
/// The first of the special effects.
const FIRST_SPECIAL: u32 = 840;

/// The balance's prices of the items' values.
#[derive(Clone, Debug, PartialEq)]
pub struct Prices {
    /// The rows `PriceDmg:` and `PriceRes:`, and the two of `PriceAwVw:`, of `A` and of `D`:
    /// each `c0`, `c1` and `c2` of `c2·x² + c1·x + c0`.
    damage: [f32; 3],
    resistance: [f32; 3],
    attack: [f32; 3],
    defence: [f32; 3],
    /// The kinds' weights of the damage (`PriceDPh`, `PriceDFe`, `PriceDMa`, `PriceDGi`) and of
    /// the resistances (`PriceR…`), physical, fire, magic and poison.
    damage_weights: [f32; KINDS],
    resistance_weights: [f32; KINDS],
    /// The row `PriceBoni:`: a bonus's worth per its value of an effect below 200, above 999, of
    /// 600 to 798, (a fourth not read), of 815 to 819, and of the others.
    bonuses: [f32; 6],
    /// The special effects' worths ([`SPECIAL`]).
    special: [Option<(i32, bool)>; 28],
    /// The sockets' rarities' raises (`RareSlotBronze`, `RareSlotSilber`, `RareSlotGold`).
    rare_slots: [f32; 3],
    /// `PricePromille`.
    promille: f32,
    /// The effects' records (`0x5E22B0`, `drops::names`).
    norms: Vec<[f32; 6]>,
}

impl Default for Prices {
    /// The original's values without the balance, its effects' records none.
    fn default() -> Self {
        let values = [
            20, 80, 30, 50, 1000, 850, 50, 50, 50, 500, 30, 20, 30, 30, 50, 60, 30, 50, 0, 150, 0,
            0, 0, 0, 200, 30, 20, 80,
        ];
        Self {
            damage: [300.0, 10.0, 0.4],
            resistance: [450.0, 21.0, 0.9],
            attack: [0.0, 3.0, 0.4],
            defence: [0.0, 4.0, 0.4],
            damage_weights: [1.0, 2.0, 2.0, 1.7],
            resistance_weights: [2.0, 1.0, 2.0, 1.5],
            bonuses: [100.0, 90.0, 40.0, 30.0, 20.0, 7.0],
            special: std::array::from_fn(|i| {
                SPECIAL[i].map(|(_, per_value)| (values[i], per_value))
            }),
            rare_slots: [0.1; 3],
            promille: 1.0,
            norms: Vec::new(),
        }
    }
}

impl Prices {
    /// The prices of the balance `balance`, the original's where it lacks them.
    pub fn of(balance: Option<&Balance>) -> Self {
        let default = Self::default();
        let Some(b) = balance else {
            return default;
        };
        let row =
            |name: &str, or: [f32; 3]| std::array::from_fn(|i| b.row_f32(name, i).unwrap_or(or[i]));
        let floats = |names: [&str; KINDS], or: [f32; KINDS]| {
            std::array::from_fn(|k| b.f32(names[k]).unwrap_or(or[k]))
        };
        Self {
            damage: row("PriceDmg:", default.damage),
            resistance: row("PriceRes:", default.resistance),
            attack: row("PriceAwVw:", default.attack),
            defence: std::array::from_fn(|i| {
                b.row_f32("PriceAwVw:", 3 + i).unwrap_or(default.defence[i])
            }),
            damage_weights: floats(
                ["PriceDPh", "PriceDFe", "PriceDMa", "PriceDGi"],
                default.damage_weights,
            ),
            resistance_weights: floats(
                ["PriceRPh", "PriceRFe", "PriceRMa", "PriceRGi"],
                default.resistance_weights,
            ),
            bonuses: std::array::from_fn(|i| {
                b.row_f32("PriceBoni:", i).unwrap_or(default.bonuses[i])
            }),
            special: std::array::from_fn(|i| {
                let (name, per_value) = SPECIAL[i]?;
                let value = b.i32(name).or(default.special[i].map(|(value, _)| value))?;
                Some((value, per_value))
            }),
            rare_slots: [
                b.f32("RareSlotBronze").unwrap_or(default.rare_slots[0]),
                b.f32("RareSlotSilber").unwrap_or(default.rare_slots[1]),
                b.f32("RareSlotGold").unwrap_or(default.rare_slots[2]),
            ],
            promille: b.f32("PricePromille").unwrap_or(default.promille),
            norms: drops::norms(&b.image),
        }
    }
}

/// What the worth asks of an item: its type, its category, its values with its sockets'
/// rarities ([`ItemValues::sockets`]), its record's flags (`+0x04`: its quality in the bits 16
/// to 19, its difficulty in 24 to 27) and its record's price (`+0x08`).
#[derive(Clone, Copy, Debug)]
pub struct Item<'a> {
    pub item: u32,
    pub category: u8,
    pub values: &'a ItemValues,
    pub flags: u32,
    pub price: i64,
}

/// The worth of `item` (`0x5D3AB0`), its sockets' items left out: a potion's and a book's its
/// record's price (a book of none 600); another's of its values:
///
/// 1. `d = (Σ wₖ·leastₖ + Σ wₖ·mostₖ)/2` by the damage's weights; where above 0, the worth
///    `⌊c2·d² + c1·d + c0⌋` by the row `PriceDmg:`, a single float before it is cut;
/// 2. `r = Σ wₖ·resistanceₖ`; where above 0, `⌊worth + (c2·r² + c1·r + c0)⌋` by `PriceRes:`;
/// 3. `A` and `D` where not 0, each `⌊c2·x² + c1·x + worth + c0⌋` by its row of `PriceAwVw:`;
/// 4. each bonus by its effect: its value times `PriceBoni:`'s by its range, or the special
///    effects' worths, added where its effect has no attribute and its record's fourth float is
///    0; else its record's share `⌊min(v·f/r₄, r₃)⌋` (`f` its record's sixth float for a ring or
///    an amulet, times the difficulty's 1/1.33, 1/1.66, 1/2 or 1/2.33) adds a tenth to a factor
///    from 1, which a special effect sets back to 1;
/// 5. the worth, at least the last row's value, times that factor where above 1, then times
///    `1 + q/8` of its quality `q` (`1 + q/2` for a ring or an amulet), then times one plus its
///    sockets' raises by their rarities, each step cut;
/// 6. a rune's (not a ring's or an amulet's) a fiftieth; a worth of 0 its record's price; times
///    `PricePromille`.
pub fn worth(prices: &Prices, item: &Item) -> i64 {
    match item.category {
        POTIONS => return item.price,
        BOOKS if item.price != 0 => return item.price,
        BOOKS => return BOOK,
        _ => {}
    }
    let values = item.values;
    let jewel = matches!(item.category, RINGS | AMULETS);
    // The damage's and the resistances' weighed sums.
    let weighed = |values: &[f32; KINDS], weights: &[f32; KINDS]| -> f64 {
        values
            .iter()
            .zip(weights)
            .map(|(&v, &w)| f64::from(v) * f64::from(w))
            .sum()
    };
    let poly =
        |row: [f32; 3], x: f64| (f64::from(row[2]) * x + f64::from(row[1])) * x + f64::from(row[0]);
    let least = values.damage.map(|[least, _]| least);
    let most = values.damage.map(|[_, most]| most);
    let d =
        (weighed(&least, &prices.damage_weights) + weighed(&most, &prices.damage_weights)) * 0.5;
    let mut last = 0.0_f32;
    let mut worth: i64 = 0;
    if d > 0.0 {
        last = to_f32(poly(prices.damage, d));
        worth = whole(f64::from(last));
    }
    let r = weighed(&values.resistance, &prices.resistance_weights);
    if r > 0.0 {
        last = to_f32(poly(prices.resistance, r));
        worth = whole(to_f64(worth) + f64::from(last));
    }
    for (value, row) in [
        (values.attack, prices.attack),
        (values.defence, prices.defence),
    ] {
        if value != 0.0 {
            let x = f64::from(value);
            worth = whole(
                (f64::from(row[2]) * x + f64::from(row[1])) * x + to_f64(worth) + f64::from(row[0]),
            );
        }
    }
    let mut factor = 1.0_f32;
    for bonus in &values.bonuses {
        bonus_worth(prices, item, jewel, *bonus, &mut worth, &mut factor);
    }
    let mut worth = whole(to_f64(worth).max(f64::from(last)));
    worth = whole(to_f64(worth) * f64::from(factor.max(1.0)));
    let quality = f64::from((item.flags >> 16) & 0xF);
    let per_quality = if jewel { 0.5 } else { 0.125 };
    worth = whole((quality * per_quality + 1.0) * to_f64(worth));
    let rarity: f64 = item
        .values
        .sockets
        .iter()
        .filter_map(|&socket| {
            let index = usize::from(socket).checked_sub(1)?;
            prices.rare_slots.get(index).copied().map(f64::from)
        })
        .fold(1.0, |sum, raise| sum + raise);
    worth = whole(to_f64(worth) * rarity);
    if !jewel && item.category == RUNES {
        worth = whole(to_f64(worth) * f64::from(RUNE_SHARE));
    }
    if worth == 0 && item.price != 0 {
        worth = item.price;
    }
    whole(to_f64(worth) * f64::from(prices.promille))
}

/// A bonus's part of the worth (`0x5D4314` to `0x5D4612`): its worth by its effect's range,
/// added to `worth` where the effect has no attribute and its record's fourth float is 0; else
/// its record's share added in tenths to `factor`.
fn bonus_worth(
    prices: &Prices,
    item: &Item,
    jewel: bool,
    bonus: Bonus,
    worth: &mut i64,
    factor: &mut f32,
) {
    let effect = bonus.effect & 0xFFFF;
    if effect == 0 {
        return;
    }
    let value = f64::from(bonus.value);
    let per = |i: usize| whole(value * f64::from(prices.bonuses[i]));
    let own = match effect {
        0..200 => per(0),
        1000.. => per(1),
        600..799 => per(2),
        840..870 => {
            // A special effect sets the factor back to 1 (`0x5D43A1`).
            *factor = 1.0;
            let index = usize::try_from(effect - FIRST_SPECIAL).unwrap_or(usize::MAX);
            match prices.special.get(index).copied().flatten() {
                Some((amount, true)) => i64::from(bonus.value) * i64::from(amount),
                Some((amount, false)) => i64::from(amount),
                None => 0,
            }
        }
        815..820 => per(4),
        _ => per(5),
    };
    let record = drops::norm(&prices.norms, effect)
        .copied()
        .unwrap_or([0.0; 6]);
    if bonus.effect & 0xFFFF_0000 == 0 && record[3] <= EPSILON {
        *worth += own.max(0);
        return;
    }
    let jewelled = if jewel { record[5] } else { 1.0 };
    let difficulty = match item.flags & 0x0F00_0000 {
        0x0200_0000 => 0.751_879_7_f32,
        0x0300_0000 => 0.602_409_66,
        0x0400_0000 => 0.5,
        0x0500_0000 => 0.429_184_56,
        _ => 1.0,
    };
    let g = f64::from(jewelled) * f64::from(difficulty);
    let share = (value / (f64::from(record[4]) / g)).min(f64::from(record[3]));
    *factor = to_f32(to_f64(whole(share)) * f64::from(0.1_f32) + f64::from(*factor));
}

/// A double truncated toward zero, as `__ftol` gives it.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a worth of a few millions at most"
)]
fn whole(value: f64) -> i64 {
    value.trunc() as i64
}

/// A worth as a double.
#[expect(
    clippy::cast_precision_loss,
    reason = "a worth of a few millions, which a double holds whole"
)]
fn to_f64(value: i64) -> f64 {
    value as f64
}

/// A double stored as a single float, as the original stores it.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the original stores the value as a single float"
)]
fn to_f32(value: f64) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(damage: [[f32; 2]; KINDS], resistance: [f32; KINDS]) -> ItemValues {
        ItemValues {
            damage,
            resistance,
            ..ItemValues::default()
        }
    }

    fn item(category: u8, values: &ItemValues) -> Item<'_> {
        Item {
            item: 1712,
            category,
            values,
            flags: 0,
            price: 0,
        }
    }

    #[test]
    fn the_damage_is_worth_its_row_of_its_weighed_mean() {
        let prices = Prices::default();
        // The Seraphim's staff 3809: 35 to 42 physical and 8 to 15 magic, d = 61.5:
        // 0.4·61.5² + 10·61.5 + 300 = 2427.9.
        let staff = values(
            [[35.0, 42.0], [0.0; 2], [8.0, 15.0], [0.0; 2]],
            [0.0; KINDS],
        );
        assert_eq!(worth(&prices, &item(5, &staff)), 2427);
        // A shield of 3 physical resistance and 4 fire: r = 10, 0.9·100 + 210 + 450.
        let shield = values([[0.0; 2]; KINDS], [3.0, 4.0, 0.0, 0.0]);
        assert_eq!(worth(&prices, &item(13, &shield)), 750);
        // Both, and `A` 5: 2427 + 750, then 0.4·25 + 15 + 3177.
        let mut both = staff;
        both.resistance = shield.resistance;
        both.attack = 5.0;
        assert_eq!(worth(&prices, &item(5, &both)), 3202);
    }

    #[test]
    fn the_quality_and_the_sockets_raise_the_worth() {
        let prices = Prices::default();
        let shield = values([[0.0; 2]; KINDS], [3.0, 4.0, 0.0, 0.0]);
        let socketed = ItemValues {
            sockets: [1, 3, 0, 0, 0, 0, 0, 0],
            ..shield
        };
        let mut fine = item(13, &shield);
        // The quality 4: 750 · 1.5; a ring's: · 3.
        fine.flags = 4 << 16;
        assert_eq!(worth(&prices, &fine), 1125);
        fine.category = RINGS;
        assert_eq!(worth(&prices, &fine), 2250);
        // Two sockets, a bronze and a gold: · 1.2, a little below.
        assert_eq!(worth(&prices, &item(13, &socketed)), 900);
    }

    #[test]
    fn the_bonuses_add_by_their_effects() {
        let prices = Prices::default();
        let mut ring = values([[0.0; 2]; KINDS], [0.0; KINDS]);
        let bonus = |effect, value| Bonus {
            condition: 0,
            effect,
            value,
        };
        // 3 of an effect below 200, 2 of one above 999, 5 of the luck 840 at 20 each, the
        // critical 859 at 150 whatever its value; no records, so all added.
        ring.bonuses[0] = bonus(100, 3);
        ring.bonuses[1] = bonus(1005, 2);
        ring.bonuses[2] = bonus(840, 5);
        ring.bonuses[3] = bonus(859, 9);
        assert_eq!(worth(&prices, &item(RINGS, &ring)), 300 + 180 + 100 + 150);
    }

    #[test]
    fn a_bonus_of_a_record_raises_the_factor_by_tenths_of_its_share() {
        // The sword 7901 as the file holds it: 21 to 23 physical, `A` 5, `D` 2, the effects 811
        // of 14 and 812 of 2, whose records' fourth and fifth floats are 25 and 2.5. Its values'
        // worth 747; 811's share ⌊14/2.5⌋ = 5 raises the factor to 1.5, 812's ⌊0.8⌋ nothing:
        // 1120, its record's price, which the original sells it by (`trade::shown`).
        let mut prices = Prices {
            norms: vec![[0.0; 6]; 74],
            ..Prices::default()
        };
        prices.norms[11] = [0.0, 0.0, 0.0, 25.0, 2.5, 3.0];
        prices.norms[12] = prices.norms[11];
        let mut sword = values([[21.0, 23.0], [0.0; 2], [0.0; 2], [0.0; 2]], [0.0; KINDS]);
        sword.attack = 5.0;
        sword.defence = 2.0;
        sword.bonuses[0] = Bonus {
            condition: 0,
            effect: 811,
            value: 14,
        };
        sword.bonuses[1] = Bonus {
            condition: 0,
            effect: 812,
            value: 2,
        };
        assert_eq!(worth(&prices, &item(5, &sword)), 1120);
        // A special effect sets the factor back to 1: the critical 859 after them.
        sword.bonuses[2] = Bonus {
            condition: 0,
            effect: 859,
            value: 1,
        };
        prices.norms[59] = [0.0; 6];
        assert_eq!(worth(&prices, &item(5, &sword)), 747 + 150);
    }

    #[test]
    fn a_potion_and_a_book_go_by_their_prices_and_a_rune_by_a_fiftieth() {
        let prices = Prices::default();
        let none = ItemValues::default();
        let mut potion = item(POTIONS, &none);
        potion.price = 400;
        assert_eq!(worth(&prices, &potion), 400);
        assert_eq!(worth(&prices, &item(BOOKS, &none)), BOOK);
        // No values: the record's price.
        let mut plain = item(5, &none);
        plain.price = 1120;
        assert_eq!(worth(&prices, &plain), 1120);
        let staff = values(
            [[35.0, 42.0], [0.0; 2], [8.0, 15.0], [0.0; 2]],
            [0.0; KINDS],
        );
        assert_eq!(worth(&prices, &item(RUNES, &staff)), 48);
    }
}
