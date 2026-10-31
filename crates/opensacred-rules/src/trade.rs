//! The prices (#1273; docs/re/inventory.md, §9 "The price"): what an item fetches as the hero sells
//! it and costs as the hero buys it (`0x43C350`), by its price, the hero's level, its trading
//! (skill 21), its charisma and the factor of the place; and the merchants' stock (`stock`, #1617).

pub mod stock;

/// The balance's gold of the hero's level and of its square (`GoldXLev`, `GoldX2Lev`), which cap
/// what a sale fetches.
const GOLD_BY_LEVEL: f64 = 300.0;
const GOLD_BY_SQUARE: f64 = 0.6;

/// The trading skill's id (text 9420).
pub const TRADING: u8 = 21;

/// What an item of the price `price` fetches as the hero sells it (`buying` false) or costs as it
/// buys it, the hero of the level `level`, the trading skill's level `skill` and the charisma
/// `charisma`, at a place of the factor `factor` (1 but where the scripts change it): the price
/// over the factor `B`; the trade's cut `k = 1 − (1 + t)^−e`, `t` 1/skill (10 without the skill)
/// and `e = −ln 0.9 / ln 1.5`, so 0.46 untrained, 0.16 at the first level; a purchase `⌊(1 + k)·B⌋`;
/// a sale `S = ⌊(1 − k)·B⌋` held under the level's gold
/// `M = 0.6·(10·charisma/(level + 9))·level²/15 + 15·level·(skill + 20)` as `⌊S·M/(S + M)⌋`.
pub fn shown(price: i64, level: u32, skill: u32, charisma: f64, buying: bool, factor: f64) -> i64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a price of a few millions at most, which an f64 holds whole"
    )]
    let price = price as f64;
    let b = (price / factor).trunc();
    let t = if skill == 0 {
        10.0
    } else {
        1.0 / f64::from(skill)
    };
    // The single-precision 0.9 of the original's constant.
    let e = -f64::from(0.9_f32).ln() / 1.5_f64.ln();
    let k = 1.0 - (1.0 + t).powf(-e);
    let whole = |x: f64| {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a price truncated as the original's `_ftol` does"
        )]
        let x = x.trunc() as i64;
        x
    };
    if buying {
        return whole((1.0 + k) * b);
    }
    let sold = ((1.0 - k) * b).trunc();
    let (level, skill) = (f64::from(level), f64::from(skill));
    let most = GOLD_BY_SQUARE * (10.0 * charisma / (level + 9.0)) * level * level / 15.0
        + (skill + 20.0) * level * GOLD_BY_LEVEL * 0.05;
    let r = if sold.abs() <= 0.001 {
        10_000.0
    } else {
        most / sold
    };
    whole(sold * (1.0 - 1.0 / (r + 1.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_hero_sells_as_the_original_shows() {
        // Seen 2026-10-30 at the Seraphim's start (level 1, no trading): the small healing potion
        // of 400 at 125, the sword 7901 of 1120 at 200, the scroll 4097 of 600 at 155 and the rune
        // 5375 of 6 at 2.
        for (price, seen) in [(400, 125), (1120, 200), (600, 155), (6, 2)] {
            assert_eq!(shown(price, 1, 0, 15.0, false, 1.0), seen, "{price}");
        }
        // The potion stands on an edge: a charisma of 10 fetches one less.
        assert_eq!(shown(400, 1, 0, 10.0, false, 1.0), 124);
    }

    #[test]
    fn buying_costs_more_and_trading_narrows_the_cut() {
        assert_eq!(shown(400, 1, 0, 15.0, true, 1.0), 585);
        assert!(shown(400, 10, 5, 20.0, true, 1.0) < 585);
        assert!(shown(400, 10, 5, 20.0, false, 1.0) > shown(400, 10, 0, 20.0, false, 1.0));
    }
}
