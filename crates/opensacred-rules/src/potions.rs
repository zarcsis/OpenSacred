//! The potions and the belt (docs/re/inventory.md, §7; #1311): the belt's kind of each potion's
//! type (`0x43F760`), the per cent of its most health that a healing potion gives back
//! (`0x5CA9E0`), and the potion that a slot of the belt drinks (`cInventoryEntry::getPotionRef`,
//! `0x5DA8C0`).

use opensacred_data::tables::Balance;

use crate::drops::Difficulty;

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

/// The sizes of the healing potions, the rows of `HealingPotion`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Size {
    Small,
    Medium,
    Large,
}

impl Size {
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

/// What the healing potions give back on a difficulty: a share of the most health in per cent
/// by their sizes, `HealingPotion`'s row (120, 200 and 300) over the difficulty's value of
/// `PotionDiv` (1 on bronze to 5 on niobium), at most 100.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Healing {
    per_cent: [u32; 3],
}

impl Default for Healing {
    /// The balance's values on bronze.
    fn default() -> Self {
        Self::new([120, 200, 300], 1)
    }
}

impl Healing {
    /// The shares of `healing`, the small, medium and large potions' values, over `divisor`.
    pub fn new(healing: [u32; 3], divisor: u32) -> Self {
        let divisor = divisor.max(1);
        Self {
            per_cent: healing.map(|value| (value / divisor).min(100)),
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
        let divisor = value(
            "PotionDiv",
            difficulty.index(),
            [1, 2, 3, 4, 5][difficulty.index()],
        );
        Self::new(healing, divisor)
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
    healing: &Healing,
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
    fn a_healing_potion_heals_its_share_over_the_difficulty() {
        let bronze = Healing::at(None, Difficulty::Bronze);
        // Seen on bronze: the small potion at 40 of 119 brought the health to 119.
        assert_eq!(bronze.heals(5171), Some(100));
        assert_eq!(healed(40, 119, 100), 119);
        let silver = Healing::at(None, Difficulty::Silver);
        assert_eq!(
            [5171, 5145, 5140, 3999].map(|item| silver.heals(item)),
            [Some(60), Some(100), Some(100), Some(100)]
        );
        let niobium = Healing::at(None, Difficulty::Niobium);
        assert_eq!(
            [5056, 5057, 5058].map(|item| niobium.heals(item)),
            [Some(24), Some(40), Some(60)]
        );
        assert_eq!(niobium.heals(5172), None);
        assert_eq!(healed(10, 119, 24), 38);
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
        let niobium = Healing::at(None, Difficulty::Niobium);
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
        let bronze = Healing::default();
        assert_eq!(choose(grid, 1, Some(10), &bronze), Some(2));
    }
}
