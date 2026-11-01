//! The sockets that the item maker's quality pass rolls for an item it makes (`0x5D2E40`, which
//! `0x5D5080` calls before the name; docs/re/items.md, "The sockets"): none for a book, a roll
//! per mille against the first socket's chance, none for a ring or an amulet, then one socket and
//! each next while its family's most allows and a roll falls below the next's chance; each
//! socket's kind by the difficulty and a roll.

use opensacred_data::tables::Balance;

use super::Difficulty;
use crate::fight::SOCKETS;
use crate::spawning::Dice;

/// The books' category, which the pass leaves alone (`0x428490`).
const BOOKS: u8 = 16;
/// The rings' and the amulets' categories, which take no socket (`0x428540`).
const JEWELS: [u8; 2] = [8, 20];
/// The most sockets of the families 0 to 32, the original's: the first record of each family's
/// list in the first table of the tail of `Balance.bin` (`0xAD57B0`).
const MOST: [u32; 33] = [
    3, 2, 3, 3, 4, 3, 4, 3, 3, 4, 3, 3, 3, 4, 0, 0, 3, 2, 2, 2, 2, 4, 3, 4, 4, 0, 1, 1, 4, 0, 2, 3,
    3,
];

/// The balance's rules of the sockets: the chances per mille of the first socket and of each
/// next (`SlotFeasibility`, `SlotFeasibility2`), what a merchant's item adds to the first's
/// (`SlotProbBonusMerchant`, docs/re/trade.md), and the most sockets of each family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sockets {
    chances: [u32; 2],
    merchant: u32,
    /// The third word of the first record of each family's list, by the family (the template's
    /// word 38): the item maker's table of the sockets and the effects (`0x440470`), which the
    /// families' own lists of `scripts\waffenmod.txt` would come first in, a file not shipped.
    most: Vec<(u8, u32)>,
}

impl Default for Sockets {
    /// The original's values.
    fn default() -> Self {
        Self {
            chances: [250, 333],
            merchant: 0,
            most: (0..).zip(MOST).collect(),
        }
    }
}

impl Sockets {
    /// The rules of the balance `balance`, the original's values where it lacks them; a family
    /// whose list is empty has none.
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
        Self {
            chances: [
                u("SlotFeasibility", default.chances[0]),
                u("SlotFeasibility2", default.chances[1]),
            ],
            merchant: u("SlotProbBonusMerchant", default.merchant),
            most: if b.tail_1.is_empty() {
                default.most
            } else {
                b.tail_1
                    .iter()
                    .filter_map(|list| {
                        let first = list.records.first()?;
                        Some((u8::try_from(list.key).ok()?, first[2]))
                    })
                    .collect()
            },
        }
    }

    /// The sockets' kinds that the pass rolls for an item of category `category` and of the
    /// family `family` (its template's word 38's low byte) made on `difficulty`, for a merchant's
    /// page where `merchant`: none for a book; else, where `rand() % 1000` falls below the first
    /// chance and the item is no ring or amulet, one socket and each next while the family's most
    /// `n` allows, `n − 1` tries at most, and `rand() % 1000` falls below the next's chance (none
    /// for a family of no list or of 0); each socket's kind `t + ⌊(rand() % 251)/100⌋` held to 1
    /// to 3, `t` 0 on bronze, 1 on silver, 2 from gold on. At most the record's eight.
    pub fn roll(
        &self,
        (family, category): (u8, u8),
        difficulty: Difficulty,
        merchant: bool,
        dice: &mut impl Dice,
    ) -> [u8; SOCKETS] {
        let mut sockets = [0; SOCKETS];
        if category == BOOKS {
            return sockets;
        }
        let chance = self.chances[0] + if merchant { self.merchant } else { 0 };
        if dice.below(1000) >= chance || JEWELS.contains(&category) {
            return sockets;
        }
        let most = self
            .most
            .iter()
            .find(|&&(f, _)| f == family)
            .map_or(0, |&(_, most)| most);
        let count = if most == 0 {
            0
        } else {
            let more = (1..most)
                .take_while(|_| dice.below(1000) < self.chances[1])
                .count();
            (1 + more).min(SOCKETS)
        };
        let tier = match difficulty {
            Difficulty::Bronze => 0,
            Difficulty::Silver => 1,
            _ => 2,
        };
        for socket in &mut sockets[..count] {
            let kind = tier + dice.below(251) / 100;
            *socket = u8::try_from(kind.clamp(1, 3)).unwrap_or(3);
        }
        sockets
    }
}

#[cfg(test)]
mod tests {
    use opensacred_data::tables::{BalanceList, BalanceValue, BalanceVariable};

    use super::*;
    use crate::spawning::Script;

    fn rolls(values: &[u32]) -> Script {
        Script::new(values.to_vec())
    }

    #[test]
    fn a_roll_below_the_first_chance_gives_one_socket_and_more_while_the_next_rolls_pass() {
        let sockets = Sockets::default();
        // The family 4's most is 4: the first roll 249 passes 250, then 332 and 100 pass 333 and
        // 333 fails it; three kinds on bronze, 99, 100 and 250 of 251.
        let mut dice = rolls(&[249, 332, 100, 333, 99, 100, 250]);
        assert_eq!(
            sockets.roll((4, 5), Difficulty::Bronze, false, &mut dice),
            [1, 1, 2, 0, 0, 0, 0, 0]
        );
        assert_eq!(dice.taken(), 7);
        // 250 fails the first chance: no socket, one roll taken.
        let mut dice = rolls(&[250]);
        assert_eq!(
            sockets.roll((4, 5), Difficulty::Bronze, false, &mut dice),
            [0; SOCKETS]
        );
        assert_eq!(dice.taken(), 1);
    }

    #[test]
    fn a_family_holds_the_sockets_to_its_most_and_one_of_none_gets_none() {
        let sockets = Sockets::default();
        // The family 1's most is 2: one more try at most, then the kind.
        let mut dice = rolls(&[0, 0, 0, 0]);
        assert_eq!(
            sockets.roll((1, 5), Difficulty::Bronze, false, &mut dice),
            [1, 1, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(dice.taken(), 4);
        // The family 26's most is 1: no try for a second.
        let mut dice = rolls(&[0, 0]);
        assert_eq!(
            sockets.roll((26, 5), Difficulty::Bronze, false, &mut dice),
            [1, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(dice.taken(), 2);
        // The family 14's most is 0, and 99 is of no list: the first roll alone.
        for family in [14, 99] {
            let mut dice = rolls(&[0]);
            assert_eq!(
                sockets.roll((family, 5), Difficulty::Bronze, false, &mut dice),
                [0; SOCKETS]
            );
            assert_eq!(dice.taken(), 1);
        }
    }

    #[test]
    fn a_book_takes_no_roll_and_a_jewel_takes_the_first_alone() {
        let sockets = Sockets::default();
        let mut dice = rolls(&[0]);
        assert_eq!(
            sockets.roll((0, 16), Difficulty::Gold, false, &mut dice),
            [0; SOCKETS]
        );
        assert_eq!(dice.taken(), 0);
        for jewel in [8, 20] {
            let mut dice = rolls(&[0, 0, 0]);
            assert_eq!(
                sockets.roll((0, jewel), Difficulty::Gold, false, &mut dice),
                [0; SOCKETS]
            );
            assert_eq!(dice.taken(), 1);
        }
    }

    #[test]
    fn the_difficulty_raises_the_kinds_held_to_gold() {
        let sockets = Sockets::default();
        let kinds = |difficulty, roll| {
            let mut dice = rolls(&[0, 999, roll]);
            sockets.roll((0, 5), difficulty, false, &mut dice)[0]
        };
        for (difficulty, kinds_by_roll) in [
            (Difficulty::Bronze, [1, 1, 1, 2]),
            (Difficulty::Silver, [1, 1, 2, 3]),
            (Difficulty::Gold, [2, 2, 3, 3]),
            (Difficulty::Platinum, [2, 2, 3, 3]),
            (Difficulty::Niobium, [2, 2, 3, 3]),
        ] {
            for (roll, kind) in [0, 99, 100, 200].into_iter().zip(kinds_by_roll) {
                assert_eq!(kinds(difficulty, roll), kind, "{difficulty:?} {roll}");
            }
        }
    }

    #[test]
    fn a_merchant_s_item_takes_the_merchant_s_bonus_on_the_first_chance() {
        let sockets = Sockets {
            merchant: 100,
            ..Sockets::default()
        };
        let roll = |merchant| {
            let mut dice = rolls(&[300, 999, 0]);
            sockets.roll((0, 5), Difficulty::Bronze, merchant, &mut dice)[0]
        };
        assert_eq!(roll(false), 0);
        assert_eq!(roll(true), 1);
    }

    #[test]
    fn the_balance_gives_the_chances_and_each_family_s_most_from_its_first_record() {
        let variable = |name: &str, value| BalanceVariable {
            name: name.to_owned(),
            offset: 0,
            value: BalanceValue::I32(value),
        };
        let balance = Balance {
            variables: vec![
                variable("SlotFeasibility", 500),
                variable("SlotFeasibility2", 900),
                variable("SlotProbBonusMerchant", 7),
            ],
            tail_1: vec![
                BalanceList {
                    key: 0,
                    records: vec![[u32::MAX, u32::MAX, 2], [801, 810, 0]],
                },
                BalanceList {
                    key: 1,
                    records: Vec::new(),
                },
            ],
            ..Balance::default()
        };
        let sockets = Sockets::of(Some(&balance));
        assert_eq!(sockets.chances, [500, 900]);
        assert_eq!(sockets.merchant, 7);
        assert_eq!(sockets.most, vec![(0, 2)]);
        assert_eq!(Sockets::of(None), Sockets::default());
    }
}
