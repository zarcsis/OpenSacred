//! The potions and the belt (docs/re/inventory.md, §7; #1311): the belt's kind of each potion's
//! type (`0x43F760`), what a potion does to its drinker (`cPotion3D`'s `0x5CAA60` and the
//! creature's events 6 that it sends, `0x52E590`; #1353), and the potion that a slot of the belt
//! drinks (`cInventoryEntry::getPotionRef`, `0x5DA8C0`).

use opensacred_data::tables::Balance;

use crate::drops::Difficulty;
use crate::fight::{Effect, HUNDREDTH, effects, stored};

/// The belt's slots, one a kind: 1 red (healing), 2 purple, 3 blue, 4 green and 5 yellow.
pub const KINDS: u8 = 5;

/// The belt's kind of a potion of the type `item` (`0x43F760`, the category 9): 1 red, 2 purple,
/// 3 blue, 4 green, 5 yellow; none for another type.
pub fn kind(item: u32) -> Option<u8> {
    Some(match item {
        3999 | 5056..=5058 | 5140 | 5145 | 5171 => 1,
        5137 | 5142 | 5168 => 2,
        5059..=5061 | 5138 | 5143 | 5169 => 3,
        5139 | 5144 | 5170 => 4,
        5062..=5064 | 5141 | 5146 | 5172 => 5,
        _ => return None,
    })
}

/// The type that the scripts' object maker makes of the type `item` (`0x43FC40`, which
/// `sdk_OBJ_CREATE_NPOS` at `0x5FB530` calls before it makes a `CreateObj`'s or a `CreateNPC`'s
/// object; #1630): an old potion, a bottle of `BOTTLE01`–`03.GRN` or the bottle 3999, becomes the
/// new one of its kind and size, the red 5171, 5145 and 5140, the blue 5169, 5143 and 5138 and
/// the yellow 5172, 5146 and 5141; any other type stays.
pub fn made(item: u32) -> u32 {
    match item {
        3999 | 5056 => 5171,
        5057 => 5145,
        5058 => 5140,
        5059 => 5169,
        5060 => 5143,
        5061 => 5138,
        5062 => 5172,
        5063 => 5146,
        5064 => 5141,
        _ => item,
    }
}

/// The sizes of the potions, by which they give more or last longer: the rows of
/// `HealingPotion`, `BlackPotion…Dur` and `BluePotion…Dur`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Size {
    Small,
    Medium,
    Large,
}

impl Size {
    /// Its index in a row of the three sizes.
    fn index(self) -> usize {
        match self {
            Self::Small => 0,
            Self::Medium => 1,
            Self::Large => 2,
        }
    }

    /// The size of a purple, blue, green or yellow potion of the type `item` (`0x5CAA60`): large
    /// 5137 to 5139 and 5141, medium 5142 to 5144 and 5146, small 5168 to 5170 and 5172.
    fn of_others(item: u32) -> Option<Self> {
        Some(match item {
            5137..=5139 | 5141 => Self::Large,
            5142..=5144 | 5146 => Self::Medium,
            5168..=5170 | 5172 => Self::Small,
            _ => return None,
        })
    }

    /// The per cent by which a green potion cures and a yellow one refills: 100, 50 and 25.
    fn per_cent(self) -> u32 {
        match self {
            Self::Small => 25,
            Self::Medium => 50,
            Self::Large => 100,
        }
    }

    /// The size by which a potion of the type `item` heals (`0x5CA9E0`): small 5056 and 5171,
    /// medium 5057 and 5145, large 5058, 5140 and the bottle 3999; none for another type.
    fn heals(item: u32) -> Option<Self> {
        Some(match item {
            5056 | 5171 => Self::Small,
            5057 | 5145 => Self::Medium,
            5058 | 5140 | 3999 => Self::Large,
            _ => return None,
        })
    }

    /// The size whose per cent the belt's choice of a healing potion weighs a potion of the type
    /// `item` by (`0x5DA8C0`): small 5171, large 5145, medium 5140 and 3999, the medium and the
    /// large crossed against [`Self::heals`]; none for the others, which the choice takes only
    /// as the first of the kind.
    fn weighed(item: u32) -> Option<Self> {
        Some(match item {
            5171 => Self::Small,
            5145 => Self::Large,
            5140 | 3999 => Self::Medium,
            _ => return None,
        })
    }
}

/// The whole values of the effect 33 that the purple and the blue potions give (`0x52E590`'s
/// cases 6 and 7): the purple's, under which the undead that its drinker kills never rise again
/// (`+0xB0` bit `0x80000`, `0x532C48`), and the blue's, the mentor's, under which a kill earns
/// its drinker a share of its base experience more (`+0xB0` bit `0x1000000`, `0x57DC62`).
pub const BANISHES: i32 = 4;
pub const MENTORS: i32 = 5;

/// The cure from which a green potion ends an effect 30 at once (`0x583F84`).
const CURES_WHOLLY: u32 = 90;

/// What a potion does to its drinker (`0x5CAA60`, the creature's events 6, `0x52E590`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Drink {
    /// A healing potion: this per cent of the most health back, only where the health is below
    /// its most; else it is not used up.
    Heals(u32),
    /// A purple or a blue potion: the effect for these seconds, one alone of its whole value
    /// (`0x582D50` at `0x584328`).
    Lasts(Effect, f32),
    /// A green potion: each effect 30 cured by this per cent ([`cured`]).
    Cures(u32),
    /// A yellow potion: every combo's regeneration set to this factor of its time, its size's
    /// cocktail over the difficulty's `PotionDiv` (`0x562470` with the kind 3 and the time −40
    /// times its per cent, `crate::combos::refilled`).
    Refills(f32),
}

/// The balance's cocktails of the yellow potions by size, small to large (`DriversComboCocktail`,
/// `ComboCocktail`, `JumboComboCocktail`, at `0xAD5788`, `0xAD5784` and `0xAD5780`).
const COCKTAILS: [f32; 3] = [120.0, 200.0, 300.0];

/// A whole count as a float, exact below 2²⁴.
fn to_f32(value: u32) -> f32 {
    f32::from(u16::try_from(value).unwrap_or(u16::MAX))
}

/// The balance's seconds of the purple and the blue potions by size, small to large, and by the
/// difficulty, bronze to niobium (`BlackPotion…Dur` and `BluePotion…Dur`, the same for both).
const LASTS: [[f32; 5]; 3] = [
    [90.0, 80.0, 70.0, 55.0, 40.0],
    [90.0, 90.0, 80.0, 70.0, 60.0],
    [90.0, 90.0, 90.0, 90.0, 75.0],
];

/// What the potions do on a difficulty: the healing ones give back a share of the most health
/// in per cent by their sizes, `HealingPotion`'s row (120, 200 and 300) over the difficulty's
/// value of `PotionDiv` (1 on bronze to 5 on niobium), at most 100; the purple and the blue last
/// the seconds of their sizes' rows by the difficulty, `BlackPotion…Dur` and `BluePotion…Dur`,
/// taken whole (`0x5CAA60`), with the values `PotionOfUndeadDeathPercent` (0.8) and
/// `PotionOfMentorPercent` (0.5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Potions {
    per_cent: [u32; 3],
    /// The yellow potions' factors by size, small to large: `DriversComboCocktail`,
    /// `ComboCocktail` and `JumboComboCocktail` (120, 200 and 300) over `PotionDiv`.
    cocktails: [f32; 3],
    /// The purple's value and seconds by size, and the blue's.
    banishes: (f32, [f32; 3]),
    mentors: (f32, [f32; 3]),
}

impl Default for Potions {
    /// The balance's values on bronze.
    fn default() -> Self {
        Self::at(None, Difficulty::Bronze)
    }
}

impl Potions {
    /// The shares of `healing`, the small, medium and large potions' values, over `divisor`; the
    /// purple and the blue potions as on bronze.
    pub fn new(healing: [u32; 3], divisor: u32) -> Self {
        let divisor = divisor.max(1);
        let bronze = LASTS.map(|row| row[0]);
        Self {
            per_cent: healing.map(|value| (value / divisor).min(100)),
            cocktails: COCKTAILS.map(|cocktail| cocktail / to_f32(divisor)),
            banishes: (0.8, bronze),
            mentors: (0.5, bronze),
        }
    }

    /// The balance's on `difficulty`, its values of the original where it lacks them.
    pub fn at(balance: Option<&Balance>, difficulty: Difficulty) -> Self {
        let value = |name: &str, index: usize, or: u32| {
            balance
                .and_then(|b| b.row_i32(name, index))
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or(or)
        };
        let healing = [0, 1, 2].map(|index| value("HealingPotion", index, [120, 200, 300][index]));
        let column = difficulty.index();
        let divisor = value("PotionDiv", column, [1, 2, 3, 4, 5][column]);
        let float = |name: &str, or: f32| balance.and_then(|b| b.f32(name)).unwrap_or(or);
        let lasts = |kind: &str| {
            [("Small", 0), ("Medium", 1), ("Large", 2)].map(|(size, index)| {
                balance
                    .and_then(|b| b.row_f32(&format!("{kind}Potion{size}Dur"), column))
                    .unwrap_or(LASTS[index][column])
                    // Taken whole (`_ftol`), and back to a float as the event carries it.
                    .trunc()
            })
        };
        let cocktails = [
            "DriversComboCocktail",
            "ComboCocktail",
            "JumboComboCocktail",
        ]
        .iter()
        .zip(COCKTAILS)
        .map(|(name, or)| float(name, or) / to_f32(divisor.max(1)));
        let cocktails: Vec<f32> = cocktails.collect();
        Self {
            cocktails: [cocktails[0], cocktails[1], cocktails[2]],
            banishes: (float("PotionOfUndeadDeathPercent", 0.8), lasts("Black")),
            mentors: (float("PotionOfMentorPercent", 0.5), lasts("Blue")),
            ..Self::new(healing, divisor)
        }
    }

    /// What a potion of the type `item` does to its drinker (`0x5CAA60`): a healing potion
    /// heals; the purple ones (5137, 5142, 5168) give the effect 33 of the whole value 4 and the
    /// blue ones (5138, 5143, 5169) of the whole value 5; the green ones (5139, 5144, 5170) cure
    /// and the yellow ones (5141, 5146, 5172) refill by 100, 50 or 25 per cent; none for another
    /// type, which is drunk to no effect and kept (5059 to 5064 among them).
    pub fn drink(&self, item: u32) -> Option<Drink> {
        if let Some(share) = self.heals(item) {
            return Some(Drink::Heals(share));
        }
        let size = Size::of_others(item)?;
        let lasts = |whole: i32, (value, seconds): (f32, [f32; 3])| {
            let seconds = seconds[size.index()];
            let effect = Effect {
                whole,
                // The adder's art, the seconds whole (`0x52ED58`).
                art: whole_seconds(seconds),
                ..Effect::new(effects::DAMAGE_OF, value)
            };
            Drink::Lasts(effect, seconds)
        };
        Some(match item {
            5137 | 5142 | 5168 => lasts(BANISHES, self.banishes),
            5138 | 5143 | 5169 => lasts(MENTORS, self.mentors),
            5139 | 5144 | 5170 => Drink::Cures(size.per_cent()),
            _ => Drink::Refills(self.cocktails[size.index()]),
        })
    }

    fn of(&self, size: Size) -> u32 {
        let [small, medium, large] = self.per_cent;
        match size {
            Size::Small => small,
            Size::Medium => medium,
            Size::Large => large,
        }
    }

    /// The per cent of its most health that a potion of the type `item` gives its drinker back
    /// (`0x5CA9E0`); none for a potion that does not heal.
    pub fn heals(&self, item: u32) -> Option<u32> {
        Size::heals(item).map(|size| self.of(size))
    }
}

/// Seconds of a potion's balance, whole.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a potion's seconds, whole and far below the integers' range"
)]
fn whole_seconds(seconds: f32) -> u32 {
    seconds.max(0.0) as u32
}

/// The value that an effect 30 of `value` keeps after a green potion's cure of `per_cent`
/// (`0x582D50` at `0x583F1B`): `value · per_cent` hundredths of it taken off, stored as a float;
/// none from 90 per cent, when it ends at once (its value 0 and its seconds 0.001).
pub fn cured(value: f32, per_cent: u32) -> Option<f32> {
    if per_cent >= CURES_WHOLLY {
        return None;
    }
    let value = f64::from(value);
    Some(stored(value - f64::from(per_cent) * value * HUNDREDTH))
}

/// Whether `effects` hold a purple potion's, under which the undead that their bearer kills never
/// rise again (`+0xB0` bit `0x80000`, `0x532C48`).
pub fn banishes(effects: &[Effect]) -> bool {
    effects
        .iter()
        .any(|effect| effect.id == effects::DAMAGE_OF && effect.whole == BANISHES)
}

/// The shares of a kill's base experience that the blue potions' effects of `effects` give their
/// bearer more: the sum of their values (`0x57D860` at `0x57DC62`; `fight::experience_with`).
pub fn mentored(effects: &[Effect]) -> f32 {
    effects
        .iter()
        .filter(|effect| effect.id == effects::DAMAGE_OF && effect.whole == MENTORS)
        .map(|effect| effect.value)
        .sum()
}

/// The potion of the belt's kind `kind` that its slot drinks, of `potions`, the grid's items by
/// their keys and types in its order, row by row (`0x5DA8C0`): the first of the kind; for the
/// kind 1, with `missing` the share of the drinker's health missing in per cent, the healing
/// potion whose share ([`Size::weighed`]) is the smallest above it, the first of equal shares,
/// else the first of the kind. With no share given, as the belt shows its slots, the first of
/// the kind.
pub fn choose(
    potions: impl IntoIterator<Item = (u32, u32)>,
    kind: u8,
    missing: Option<u32>,
    healing: &Potions,
) -> Option<u32> {
    let missing = missing.unwrap_or(100).min(100);
    let mut chosen = None;
    let mut best = u32::MAX;
    for (key, item) in potions {
        if self::kind(item) != Some(kind) {
            continue;
        }
        if kind != 1 {
            return Some(key);
        }
        chosen.get_or_insert(key);
        if let Some(share) = Size::weighed(item).map(|size| healing.of(size))
            && share > missing
            && share < best
        {
            chosen = Some(key);
            best = share;
        }
    }
    chosen
}

/// The share of a health of `current` out of `most` that is missing, in per cent
/// (`0x5DA8C0`): 100 less the whole per cent left, held to 0 to 100.
pub fn missing(current: u32, most: u32) -> u32 {
    if most == 0 {
        return 100;
    }
    let left = u64::from(current) * 100 / u64::from(most);
    100 - u32::try_from(left.min(100)).unwrap_or(100)
}

/// The health that a drinker of `current` out of `most` has after a healing potion of the share
/// `per_cent`: that share of its most added, at most its most (the creature's event 6 of the
/// kind 5, `0x52E590`; #1328).
pub fn healed(current: u32, most: u32, per_cent: u32) -> u32 {
    let gain = u64::from(most) * u64::from(per_cent.min(100)) / 100;
    u32::try_from((u64::from(current) + gain).min(u64::from(most))).unwrap_or(most)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_belt_kinds_go_by_the_types() {
        assert_eq!(kind(5171), Some(1));
        assert_eq!(kind(3999), Some(1));
        assert_eq!(kind(5137), Some(2));
        assert_eq!(kind(5060), Some(3));
        assert_eq!(kind(5170), Some(4));
        assert_eq!(kind(5146), Some(5));
        assert_eq!(kind(5633), None);
    }

    #[test]
    fn the_scripts_old_potions_are_made_new_of_their_kinds_and_sizes() {
        let old = [3999, 5056, 5057, 5058, 5059, 5060, 5061, 5062, 5063, 5064];
        assert_eq!(
            old.map(made),
            [5171, 5171, 5145, 5140, 5169, 5143, 5138, 5172, 5146, 5141]
        );
        for item in old {
            assert_eq!(kind(made(item)), kind(item));
        }
        // The healing bottles keep their sizes; the bottle 3999, a large one, becomes small.
        let bottles = [5056, 5057, 5058];
        assert_eq!(
            bottles.map(|item| Size::heals(made(item))),
            bottles.map(Size::heals)
        );
        assert_eq!(Size::heals(made(3999)), Some(Size::Small));
        // Any other type stays, the new potions and the drops' items among them.
        for item in [5171, 5145, 5140, 5137, 1712, 0] {
            assert_eq!(made(item), item);
        }
    }

    #[test]
    fn a_healing_potion_heals_its_share_over_the_difficulty() {
        let bronze = Potions::at(None, Difficulty::Bronze);
        // Seen on bronze: the small potion at 40 of 119 brought the health to 119.
        assert_eq!(bronze.heals(5171), Some(100));
        assert_eq!(healed(40, 119, 100), 119);
        let silver = Potions::at(None, Difficulty::Silver);
        assert_eq!(
            [5171, 5145, 5140, 3999].map(|item| silver.heals(item)),
            [Some(60), Some(100), Some(100), Some(100)]
        );
        let niobium = Potions::at(None, Difficulty::Niobium);
        assert_eq!(
            [5056, 5057, 5058].map(|item| niobium.heals(item)),
            [Some(24), Some(40), Some(60)]
        );
        assert_eq!(niobium.heals(5172), None);
        assert_eq!(healed(10, 119, 24), 38);
    }

    #[test]
    fn the_other_potions_last_cure_or_refill_by_their_sizes() {
        let lasts = |healing: &Potions, item| match healing.drink(item) {
            Some(Drink::Lasts(effect, seconds)) => Some((effect.whole, effect.value, seconds)),
            _ => None,
        };
        let bronze = Potions::default();
        assert_eq!(lasts(&bronze, 5168), Some((BANISHES, 0.8, 90.0)));
        assert_eq!(lasts(&bronze, 5143), Some((MENTORS, 0.5, 90.0)));
        // On niobium the small ones last 40 s, the medium 60 and the large 75.
        let niobium = Potions::at(None, Difficulty::Niobium);
        assert_eq!(
            [5169, 5143, 5138].map(|item| lasts(&niobium, item).map(|lasts| lasts.2)),
            [Some(40.0), Some(60.0), Some(75.0)]
        );
        assert_eq!(bronze.drink(5139), Some(Drink::Cures(100)));
        assert_eq!(bronze.drink(5144), Some(Drink::Cures(50)));
        assert_eq!(bronze.drink(5170), Some(Drink::Cures(25)));
        // A small cocktail of 120 over bronze's `PotionDiv` of 1.
        assert_eq!(bronze.drink(5172), Some(Drink::Refills(120.0)));
        assert_eq!(bronze.drink(5171), Some(Drink::Heals(100)));
        assert_eq!(bronze.drink(5060), None);
        assert_eq!(bronze.drink(5633), None);
    }

    #[test]
    fn a_green_potion_cures_a_share_and_ends_from_ninety() {
        assert_eq!(cured(8.0, 25), Some(6.0));
        assert_eq!(cured(8.0, 50), Some(4.0));
        assert!(cured(8.0, 89).is_some_and(|value| (value - 0.88).abs() < 1e-5));
        assert_eq!(cured(8.0, 90), None);
        assert_eq!(cured(8.0, 100), None);
    }

    #[test]
    fn the_blue_and_the_purple_effects_are_read_from_the_list() {
        let bronze = Potions::default();
        let (Some(Drink::Lasts(purple, _)), Some(Drink::Lasts(blue, _))) =
            (bronze.drink(5137), bronze.drink(5138))
        else {
            panic!("the purple and the blue potions last");
        };
        let fire = Effect {
            whole: 1,
            ..Effect::new(effects::DAMAGE_OF, 0.5)
        };
        assert!(banishes(&[fire, purple]) && !banishes(&[fire, blue]));
        assert!((mentored(&[purple, blue, fire]) - 0.5).abs() < f32::EPSILON);
        assert!(mentored(&[fire]).abs() < f32::EPSILON);
    }

    #[test]
    fn the_missing_share_is_a_whole_per_cent() {
        assert_eq!(missing(119, 119), 0);
        assert_eq!(missing(40, 119), 67);
        assert_eq!(missing(0, 119), 100);
        assert_eq!(missing(5, 0), 100);
    }

    #[test]
    fn the_belt_drinks_the_least_healing_potion_that_covers_the_wound() {
        let niobium = Potions::at(None, Difficulty::Niobium);
        // The shares weighed on niobium: 5171 24, 5140 40, 5145 60.
        let grid = [(1, 5172), (2, 5145), (3, 5140), (4, 5171)];
        assert_eq!(choose(grid, 1, Some(10), &niobium), Some(4));
        assert_eq!(choose(grid, 1, Some(30), &niobium), Some(3));
        assert_eq!(choose(grid, 1, Some(50), &niobium), Some(2));
        // None covers it: the first of the kind.
        assert_eq!(choose(grid, 1, Some(70), &niobium), Some(2));
        // The belt's look: the first of the kind.
        assert_eq!(choose(grid, 1, None, &niobium), Some(2));
        assert_eq!(choose(grid, 5, None, &niobium), Some(1));
        assert_eq!(choose(grid, 3, Some(10), &niobium), None);
        // On bronze every share is 100: the first healing potion.
        let bronze = Potions::default();
        assert_eq!(choose(grid, 1, Some(10), &bronze), Some(2));
    }
}
