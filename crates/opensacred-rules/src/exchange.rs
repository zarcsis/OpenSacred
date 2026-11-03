//! The combat master's exchange of runes (`cUI_MasterLotto`, #1895; docs/re/arts.md, "The combat
//! master's exchange"): the runes that the page puts in four slots go for one rune, by their
//! count, at the balance's price of that count ([`crate::fight::Rules::exchange_price`]).

use opensacred_data::tables::Arts as Tables;

use crate::arts::class_mask;
use crate::spawning::Dice;

/// The types that the exchange's lists run over (`0x6D4160`: `0x14FF` to `0x1696`).
pub const FIRST_RUNE: u32 = 5375;
pub const LAST_RUNE: u32 = 5782;
/// The slots the runes go into.
pub const SLOTS: usize = 4;
/// The rune that a pick from an empty list gives (`0x6D4400`).
pub const FALLBACK: u32 = 5767;

/// What a press of the button makes of the runes in the slots, by their count (`0x6D4520`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Made {
    /// Below two: nothing, the runes kept.
    Nothing,
    /// Two: a rune drawn of every class's.
    AnyClass,
    /// Three: a rune drawn of the hero's class's.
    OwnClass,
    /// Four: the rune of the hero's class's that the arrows chose.
    Chosen,
}

/// What `count` runes make.
pub fn made(count: usize) -> Made {
    match count {
        2 => Made::AnyClass,
        3 => Made::OwnClass,
        4.. => Made::Chosen,
        _ => Made::Nothing,
    }
}

/// The runes of the list of the class of the creature type `kind`, or of every class for `None`
/// (`0x6D4160`): each type of [`FIRST_RUNE`] to [`LAST_RUNE`] in its order whose art (`art_of`,
/// `0x43AF10`) is not 0, and for a class one of its spells (`0x4D1B20`) or else of its weapon
/// arts (`0x4D24C0`).
pub fn list(kind: Option<u32>, tables: &Tables, art_of: impl Fn(u32) -> Option<u16>) -> Vec<u32> {
    let mask = kind.map(class_mask);
    (FIRST_RUNE..=LAST_RUNE)
        .filter(|&rune| {
            let Some(art) = art_of(rune).filter(|&art| art != 0) else {
                return false;
            };
            let Some(mask) = mask else {
                return true;
            };
            tables
                .spells
                .iter()
                .any(|spell| spell.id == art && spell.classes & mask != 0)
                || tables
                    .weapon_arts
                    .iter()
                    .any(|weapon| weapon.id == art && weapon.classes & mask != 0)
        })
        .collect()
}

/// A rune drawn of `list` (`0x6D4400`: `rand()` over the list's length), [`FALLBACK`] of an empty
/// one.
pub fn draw(list: &[u32], dice: &mut impl Dice) -> u32 {
    let count = u32::try_from(list.len()).unwrap_or(u32::MAX);
    if count == 0 {
        return FALLBACK;
    }
    usize::try_from(dice.below(count))
        .ok()
        .and_then(|k| list.get(k))
        .copied()
        .unwrap_or(FALLBACK)
}

/// The choice of the arrows (`0x6D4250`, the global `0x17EA250`): its index into the hero's
/// class's list, kept through the game, which the right arrow raises and the left lowers, each
/// round the list's ends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Choice(pub usize);

impl Choice {
    /// The rune chosen of `list`, the index held within it.
    pub fn of(self, list: &[u32]) -> Option<u32> {
        let count = list.len();
        (count > 0).then(|| list[self.0 % count])
    }

    /// The next of `count` (the right arrow).
    pub fn next(self, count: usize) -> Self {
        if count == 0 {
            return self;
        }
        let k = self.0 % count;
        Self(if k + 1 < count { k + 1 } else { 0 })
    }

    /// The one before of `count` (the left arrow).
    pub fn previous(self, count: usize) -> Self {
        if count == 0 {
            return self;
        }
        let k = self.0 % count;
        Self(if k == 0 { count - 1 } else { k - 1 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opensacred_data::tables::{Spell, WeaponArt};

    #[test]
    fn the_count_of_runes_names_what_the_button_makes() {
        assert_eq!(
            [0, 1, 2, 3, 4].map(made),
            [
                Made::Nothing,
                Made::Nothing,
                Made::AnyClass,
                Made::OwnClass,
                Made::Chosen
            ]
        );
    }

    #[test]
    fn a_class_list_takes_the_runes_of_its_arts() {
        let tables = Tables {
            spells: vec![Spell {
                id: 7,
                name: 0,
                description: 0,
                short_description: 0,
                icon: String::new(),
                sprites: [0; 3],
                launch: 1,
                regeneration: [0; 2],
                effects: [[0; 2]; 13],
                values: [[0; 2]; 3],
                caps: [0; 3],
                element: 0,
                classes: 0x1,
                flags: 0,
            }],
            weapon_arts: vec![WeaponArt {
                id: 1020,
                name: 0,
                description: 0,
                short_description: 0,
                icon: String::new(),
                sprites: [0; 3],
                value_3c: 0,
                regeneration: [0.0; 2],
                distance: 130,
                launch: 1,
                factor: [0.75, 0.05],
                value_58: [0.0; 2],
                value_60: [0; 2],
                value_68: [0; 2],
                classes: 0x2,
                flags: 1,
            }],
        };
        let art_of = |rune: u32| match rune {
            5375 => Some(7),
            5553 => Some(1020),
            5554 => Some(1020),
            5600 => Some(0),
            _ => None,
        };
        // The Gladiator's (2) by his weapon art, the Seraphim's (1) by her spell, every class's.
        assert_eq!(list(Some(2), &tables, art_of), [5553, 5554]);
        assert_eq!(list(Some(1), &tables, art_of), [5375]);
        assert_eq!(list(None, &tables, art_of), [5375, 5553, 5554]);
    }

    #[test]
    fn the_arrows_go_round_the_list() {
        // Seen: from the first, the right arrow twice and the left once (5553, 5554, 5555, 5554).
        let list = [5553, 5554, 5555, 5556];
        let choice = Choice::default();
        assert_eq!(choice.of(&list), Some(5553));
        let choice = choice.next(4).next(4);
        assert_eq!(choice.of(&list), Some(5555));
        assert_eq!(choice.previous(4).of(&list), Some(5554));
        assert_eq!(Choice(3).next(4), Choice(0));
        assert_eq!(Choice(0).previous(4), Choice(3));
        assert_eq!(Choice(9).of(&list), Some(5554));
    }
}
