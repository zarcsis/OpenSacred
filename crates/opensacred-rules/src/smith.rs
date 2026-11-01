//! The blacksmith (#1658; docs/re/blacksmith.md): his tier, the upgrades his three buttons give,
//! what may go into a socket, his price, and what a socket's item adds to an item and takes off
//! it again as it comes out. The window that does it is `opensacred-game`'s.

use crate::fight::{HUNDREDTH, KINDS, stored};

/// The highest tier of a smith.
pub const TOP_TIER: u8 = 5;
/// The type of the smith whose tier is one above the difficulty's (`TYPE_NPC_NEOPHYSTIS`).
const NEOPHYSTIS: u32 = 367;
/// The Dwarf's class, the only one that smiths by a skill of its own.
const DWARF: u32 = 8;
/// What a rune counts in the price as it waits for a socket.
const RUNE_WORTH: i64 = 2000;
/// The categories that may go into a socket (`0x428780`): the rings, the amulets, the
/// blacksmith's upgrades and the runes.
const RINGS: u8 = 8;
const AMULETS: u8 = 20;
pub const UPGRADES: u8 = 26;
const RUNES: u8 = 28;
/// The types of other categories that go into a socket too: the skeleton warrior's head, the
/// dragon orbs, the dragon scale and the sacred symbols.
const SOCKETED_QUEST_ITEMS: [u32; 6] = [5100, 6075, 6076, 6089, 6112, 6113];
/// The categories of the items that the smith works (`0x6A4380`): the weapons, the shields and
/// the armour's pieces.
const WORKED: [u8; 12] = [5, 6, 13, 17, 18, 19, 21, 22, 23, 24, 25, 33];

/// Who smiths: a creature, by its type, or the Dwarf, by his skill 33's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Smith {
    Creature { kind: u32 },
    Dwarf { skill: u8 },
}

impl Smith {
    /// The smith of a creature of class `class` and type `kind` with the skill 33's value
    /// `skill`: the Dwarf by his skill where the class is his, else the creature.
    pub fn of(class: u32, kind: u32, skill: u8) -> Self {
        if class == DWARF {
            Self::Dwarf { skill }
        } else {
            Self::Creature { kind }
        }
    }

    /// The smith's tier (`0x6A2230`), 0 to [`TOP_TIER`]: a creature's the difficulty (0 bronze
    /// to 4 niobium), one more for [`NEOPHYSTIS`]; the Dwarf's `⌊min(0.025·s + 0.5, 5)⌋` of his
    /// skill's value `s`, the product of the float 0.025 at the x87 unit's 53 bits.
    pub fn tier(self, difficulty: u8) -> u8 {
        match self {
            Self::Creature { kind } => {
                let tier = difficulty.min(4);
                if kind == NEOPHYSTIS { tier + 1 } else { tier }
            }
            Self::Dwarf { skill } => {
                let tier = f64::from(skill) * f64::from(0.025_f32) + 0.5;
                truncate(tier.min(f64::from(TOP_TIER)))
                    .try_into()
                    .unwrap_or(TOP_TIER)
            }
        }
    }

    /// The factor of the price (`0x6A2B20`): 1 for a creature, `100/(s + 79)` stored as a float
    /// for the Dwarf of skill value `s`.
    fn price_factor(self) -> f64 {
        match self {
            Self::Creature { .. } => 1.0,
            Self::Dwarf { skill } => f64::from(stored(100.0 / (f64::from(skill) + 79.0))),
        }
    }
}

/// The smith's three buttons, each giving an upgrade of his tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    /// `SCHLEIFEN`, the types 8000 to 8005.
    Grinding,
    /// `HAERTEN`, 8020 to 8025.
    Hardening,
    /// `VEREDELN`, 8030 to 8035; the Dwarf's `VEREDELN_R`, 8042 to 8047.
    Refining,
}

/// The type of the upgrade that `button` gives at `tier` (the tables at `0x9E00D0` and, for the
/// Dwarf's third button, `0x9E0118`).
pub fn upgrade(button: Button, tier: u8, smith: Smith) -> u32 {
    let first = match (button, smith) {
        (Button::Grinding, _) => 8000,
        (Button::Hardening, _) => 8020,
        (Button::Refining, Smith::Creature { .. }) => 8030,
        (Button::Refining, Smith::Dwarf { .. }) => 8042,
    };
    first + u32::from(tier.min(TOP_TIER))
}

/// Whether an item of type `kind` and category `category` may go into a socket (`0x428780`).
pub fn socketable(kind: u32, category: u8) -> bool {
    matches!(category, RINGS | AMULETS | UPGRADES | RUNES) || SOCKETED_QUEST_ITEMS.contains(&kind)
}

/// Whether the smith works an item of category `category` (`0x6A4380`; the item's own check
/// `0xE` not followed).
pub fn worked(category: u8) -> bool {
    WORKED.contains(&category)
}

/// Something that waits for an empty socket of the item: a rune, or another item or upgrade of
/// its record's worth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Waiting {
    Rune,
    Worth(i64),
}

/// The smith's price of putting `waiting` into the empty sockets of an item worth `worth` with
/// `sockets` sockets (`0x6A2B20`): `⌊(0.05·P + 0.1·S + 500) · n · f⌋`, `S` the sum of what
/// waits (a rune 2,000), `n` the sockets, at least 1, and `f` the smith's factor; the hundredths
/// of the whole numbers `5·P` and `10·S` by the float 0.01, at the x87 unit's 53 bits.
pub fn price(smith: Smith, worth: i64, sockets: usize, waiting: &[Waiting]) -> i64 {
    let waiting: i64 = waiting
        .iter()
        .map(|waiting| match *waiting {
            Waiting::Rune => RUNE_WORTH,
            Waiting::Worth(worth) => worth,
        })
        .sum();
    let sockets = i32::try_from(sockets.max(1)).unwrap_or(i32::MAX);
    let sum = whole_f64(5 * worth) * HUNDREDTH + whole_f64(10 * waiting) * HUNDREDTH + 500.0;
    truncate(sum * f64::from(sockets) * smith.price_factor())
}

/// The values of an item's record that its sockets' items add to (`+0x1C…+0x38`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Values {
    /// Each kind's least and most damage (`+0x1C…+0x2A`).
    pub least: [i16; KINDS],
    pub most: [i16; KINDS],
    /// `A`, `D` and the word between them, not read (`+0x2C`, `+0x2E`, `+0x30`).
    pub attack: i16,
    pub defence: i16,
    pub word_30: i16,
    /// The four resistances (`+0x32…+0x38`).
    pub resistance: [i16; KINDS],
}

/// The factor of a socket's item's values (`0x5BFB60`): `1 + (k + t)/100`, `k` 1, 3 or 9 by the
/// socket's kind (bronze, silver, gold; 4 as gold, any other 0) and `t` the smith's tier, the
/// hundredth the float 0.01 at the x87 unit's 53 bits.
fn factor(kind: u8, tier: u8) -> f64 {
    let k = match kind {
        1 => 1.0,
        2 => 3.0,
        3 | 4 => 9.0,
        _ => 0.0,
    };
    (k + f64::from(tier)) * HUNDREDTH + 1.0
}

impl Values {
    /// Every value, in the order the original adds them.
    fn each(&mut self) -> [&mut i16; 15] {
        let [l0, l1, l2, l3] = &mut self.least;
        let [m0, m1, m2, m3] = &mut self.most;
        let [r0, r1, r2, r3] = &mut self.resistance;
        [
            m0,
            m1,
            m2,
            m3,
            l0,
            l1,
            l2,
            l3,
            r0,
            r1,
            r2,
            r3,
            &mut self.attack,
            &mut self.word_30,
            &mut self.defence,
        ]
    }

    /// Adds the values of `socket`'s item, put into a socket of kind `kind` by a smith of tier
    /// `tier`, each times [`factor`] and truncated (`0x5BFB60` as `0x5BFF60` calls it).
    pub fn add(&mut self, socket: &Self, kind: u8, tier: u8) {
        let factor = factor(kind, tier);
        let mut socket = *socket;
        for (value, added) in self.each().into_iter().zip(socket.each()) {
            *value = value.wrapping_add(scaled(*added, factor));
        }
    }

    /// Takes the values of `socket`'s item off as it comes out (`0x5BFB60` as `0x5C0430` calls
    /// it): the same products, each value held at 0 or more.
    pub fn take_off(&mut self, socket: &Self, kind: u8, tier: u8) {
        let factor = factor(kind, tier);
        let mut socket = *socket;
        for (value, taken) in self.each().into_iter().zip(socket.each()) {
            *value = value.wrapping_sub(scaled(*taken, factor)).max(0);
        }
    }
}

/// What an item asks of its holder (its record's `+0x14…+0x1B`): the level, the three bytes
/// after it and `+0x18` (not read here), and a skill with its level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Requirements {
    pub level: u8,
    pub bytes: [u8; 3],
    pub byte_18: u8,
    pub skill: u8,
    pub skill_level: u8,
}

impl Requirements {
    /// Raises each to the socket's item's where that asks more (`0x5BFF60`); the skill goes with
    /// its level where the socket's item asks a higher one.
    pub fn raise(&mut self, socket: &Self) {
        self.level = self.level.max(socket.level);
        for (own, other) in self.bytes.iter_mut().zip(socket.bytes) {
            *own = (*own).max(other);
        }
        self.byte_18 = self.byte_18.max(socket.byte_18);
        if self.skill_level < socket.skill_level {
            self.skill = socket.skill;
            self.skill_level = socket.skill_level;
        }
    }
}

/// `value` times `factor`, truncated toward zero as the runtime's `_ftol` does, kept to 16 bits
/// as the original adds it to a word.
fn scaled(value: i16, factor: f64) -> i16 {
    let product = truncate(f64::from(value) * factor);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the original adds the product to a 16-bit word"
    )]
    let word = product as i16;
    word
}

/// A whole number as the x87 unit loads it, exact in a double for any worth.
#[expect(
    clippy::cast_precision_loss,
    reason = "the worths are far below 2⁵³, where a double holds every whole number"
)]
fn whole_f64(value: i64) -> f64 {
    value as f64
}

/// `value` truncated toward zero, as the runtime's `_ftol` does.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the original truncates the float to a whole number"
)]
fn truncate(value: f64) -> i64 {
    value as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMITH: Smith = Smith::Creature { kind: 688 };

    #[test]
    fn a_smiths_tier_is_the_difficulty_and_the_dwarfs_his_skill() {
        assert_eq!(
            (0..5).map(|d| SMITH.tier(d)).collect::<Vec<_>>(),
            [0, 1, 2, 3, 4]
        );
        let neophystis = Smith::Creature { kind: NEOPHYSTIS };
        assert_eq!(neophystis.tier(0), 1);
        assert_eq!(neophystis.tier(4), TOP_TIER);
        let dwarf = |skill| Smith::Dwarf { skill }.tier(0);
        // `0.025·s + 0.5` truncated: a tier more each 40 points from 20, at most 5.
        assert_eq!(
            [0, 19, 20, 59, 60, 100, 139, 140, 179, 180, 255].map(dwarf),
            [0, 0, 1, 1, 2, 3, 3, 4, 4, 5, 5]
        );
        assert_eq!(Smith::of(8, 0, 60), Smith::Dwarf { skill: 60 });
        assert_eq!(Smith::of(2, 688, 60), SMITH);
    }

    #[test]
    fn the_buttons_give_the_upgrades_of_the_tier() {
        assert_eq!(upgrade(Button::Grinding, 0, SMITH), 8000);
        assert_eq!(upgrade(Button::Hardening, 3, SMITH), 8023);
        assert_eq!(upgrade(Button::Refining, 5, SMITH), 8035);
        assert_eq!(
            upgrade(Button::Refining, 2, Smith::Dwarf { skill: 60 }),
            8044
        );
        assert_eq!(upgrade(Button::Grinding, 9, SMITH), 8005);
    }

    #[test]
    fn rings_amulets_upgrades_runes_and_six_quest_items_go_into_sockets() {
        for category in [8, 20, 26, 28] {
            assert!(socketable(1, category), "{category}");
        }
        assert!(!socketable(1, 5));
        assert!(!socketable(1, 9));
        assert!(socketable(6112, 15));
        assert!(!socketable(6114, 15));
        assert!(worked(5) && worked(33) && !worked(8) && !worked(9));
    }

    #[test]
    fn the_price_takes_a_twentieth_of_the_worth_a_tenth_of_what_waits_and_500_by_sockets() {
        // The float 0.01 is a little below a hundredth, so at 53 bits a sum that comes to a
        // whole number in real numbers falls just below it: 550 is 549, no socket counting as
        // one.
        assert_eq!(price(SMITH, 1000, 0, &[]), 549);
        // `(0.05·1000 + 0.1·300 + 500)·2`, a little below 1,160.
        assert_eq!(price(SMITH, 1000, 2, &[Waiting::Worth(300)]), 1159);
        // A rune counts 2,000 whatever its worth.
        assert_eq!(price(SMITH, 1000, 1, &[Waiting::Rune]), 749);
        // A sum of 561.05 keeps its 561.
        assert_eq!(price(SMITH, 999, 1, &[Waiting::Worth(111)]), 561);
        // The Dwarf of skill 21 pays as a smith does, of skill 121 half, of 0 a little more.
        let dwarf = |skill| price(Smith::Dwarf { skill }, 1000, 1, &[]);
        assert_eq!(dwarf(21), 549);
        assert_eq!(dwarf(121), 274);
        assert_eq!(dwarf(0), 696);
    }

    #[test]
    fn a_sockets_item_adds_its_values_by_the_sockets_kind_and_takes_them_off_again() {
        let socket = Values {
            least: [10, 0, 0, 0],
            most: [20, 0, 0, 0],
            attack: 100,
            defence: 50,
            word_30: 0,
            resistance: [0, 30, 0, 0],
        };
        let item = Values {
            least: [5, 0, 0, 0],
            most: [9, 0, 0, 0],
            defence: 10,
            ..Values::default()
        };
        // A gold socket by a smith of tier 2: times 1.11 by the float 0.01, a little below, and
        // truncated, so that 100 gives 110.
        let mut worked = item;
        worked.add(&socket, 3, 2);
        assert_eq!(worked.least[0], 5 + 11);
        assert_eq!(worked.most[0], 9 + 22);
        assert_eq!(worked.attack, 110);
        assert_eq!(worked.defence, 10 + 55);
        assert_eq!(worked.resistance[1], 33);
        // A bronze socket of tier 0: times 1.01, so 20 stays 20 and 100 too.
        let mut bronze = item;
        bronze.add(&socket, 1, 0);
        assert_eq!((bronze.most[0], bronze.attack), (29, 100));
        // Each kind its own share: 300 by tier 0 gives 302 in bronze, 308 in silver, 326 in
        // gold, 300 in a socket of no kind.
        let three = Values {
            attack: 300,
            ..Values::default()
        };
        let by_kind = [1, 2, 3, 4, 0].map(|kind| {
            let mut values = Values::default();
            values.add(&three, kind, 0);
            values.attack
        });
        assert_eq!(by_kind, [302, 308, 326, 326, 300]);
        // Out again, the same products off; what was below stays at 0.
        worked.take_off(&socket, 3, 2);
        assert_eq!(worked, item);
        let mut low = Values::default();
        low.take_off(&socket, 3, 2);
        assert_eq!(low, Values::default());
    }

    #[test]
    fn a_sockets_item_raises_what_the_item_asks() {
        let mut own = Requirements {
            level: 10,
            bytes: [5, 0, 7],
            byte_18: 0,
            skill: 3,
            skill_level: 2,
        };
        own.raise(&Requirements {
            level: 4,
            bytes: [1, 6, 9],
            byte_18: 2,
            skill: 7,
            skill_level: 5,
        });
        assert_eq!(
            own,
            Requirements {
                level: 10,
                bytes: [5, 6, 9],
                byte_18: 2,
                skill: 7,
                skill_level: 5,
            }
        );
        // A lower skill's level, or the same, leaves the item's skill.
        for skill_level in [1, 5] {
            own.raise(&Requirements {
                skill: 1,
                skill_level,
                ..Requirements::default()
            });
            assert_eq!((own.skill, own.skill_level), (7, 5));
        }
    }
}
