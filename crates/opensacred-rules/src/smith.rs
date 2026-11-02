//! The blacksmith (#1658; docs/re/blacksmith.md): his tier, the upgrades his three buttons give,
//! what may go into a socket, his price, and what a socket's item adds to an item and takes off
//! it again as it comes out; an item that carries its sockets' items (#1661), their values added
//! to its own and their bonus slots counted where it is worn. The window that does it is
//! `opensacred-game`'s.

use crate::drops::Dropped;
use crate::fight::{BONUSES, Bonus, HUNDREDTH, ItemValues, KINDS, SOCKETS, stored};

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

    /// The factor of the price (`0x6A2B20`): 1 for a creature, [`dwarf_price_factor`] for the
    /// Dwarf.
    fn price_factor(self) -> f64 {
        match self {
            Self::Creature { .. } => 1.0,
            Self::Dwarf { skill } => dwarf_price_factor(u32::from(skill)),
        }
    }
}

/// The factor of the price of the Dwarf as the smith of skill value `skill` (`0x6A2B20`):
/// `100/(s + 79)` stored as a float, 1.27 at 0, 1 at 21, 0.5 at 121; the skill's panel tells it
/// (`fight::skill_effects`).
pub fn dwarf_price_factor(skill: u32) -> f64 {
    f64::from(stored(100.0 / (f64::from(skill) + 79.0)))
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

impl Values {
    /// The values of `values` as an item's record holds them, each a 16-bit word.
    fn of(values: &ItemValues) -> Self {
        Self {
            least: values.damage.map(|[least, _]| word(least)),
            most: values.damage.map(|[_, most]| word(most)),
            attack: word(values.attack),
            defence: word(values.defence),
            word_30: 0,
            resistance: values.resistance.map(word),
        }
    }

    /// Adds them to `values`.
    fn onto(self, values: &mut ItemValues) {
        for (ends, (least, most)) in values
            .damage
            .iter_mut()
            .zip(self.least.into_iter().zip(self.most))
        {
            ends[0] += f32::from(least);
            ends[1] += f32::from(most);
        }
        for (value, added) in values.resistance.iter_mut().zip(self.resistance) {
            *value += f32::from(added);
        }
        values.attack += f32::from(self.attack);
        values.defence += f32::from(self.defence);
    }
}

/// An item in a socket of another (`0x5BFF60`): the item as it went in, its values then (its
/// maker's, else its type's record's), and the tier of the smith who put it there, which its
/// record keeps (`+0x81`) and by which its values come off again (`0x5C0430`).
#[derive(Clone, Debug, PartialEq)]
pub struct Socketed {
    pub item: Dropped,
    pub values: ItemValues,
    pub tier: u8,
}

/// Puts `socket`, an item of category `category` with the values `values`, into the next empty
/// socket of `item`, whose sockets' kinds are `kinds`, by a smith of tier `tier` (`0x5BFF60`):
/// `socket` given back where it is refused, every socket of `item` (a kind other than 0) filled
/// or `socket` going into none ([`socketable`]); none where it went in.
pub fn fill(
    item: &mut Dropped,
    kinds: &[u8; SOCKETS],
    (socket, values, category): (Dropped, ItemValues, u8),
    tier: u8,
) -> Option<Dropped> {
    let sockets = kinds.iter().filter(|&&kind| kind != 0).count();
    if item.socketed.len() >= sockets || !socketable(socket.item, category) {
        return Some(socket);
    }
    item.socketed.push(Socketed {
        item: socket,
        values,
        tier,
    });
    None
}

/// Takes the item in the socket `index` of `item` out, as the smith's window's press on a filled
/// socket does (`0x6A27B0`): every socket of `item` emptied, each item's values off it again
/// (`0x5C0430`, which clears the list), and the item of `index` given back, the others lost.
pub fn take_out(item: &mut Dropped, index: usize) -> Option<Dropped> {
    std::mem::take(&mut item.socketed)
        .into_iter()
        .nth(index)
        .map(|socketed| socketed.item)
}

/// The values of an item whose own are `own` with what its sockets' items `socketed` add, each
/// by its socket's kind (`own.sockets`, in order) and its smith's tier (`0x5BFB60` as
/// `0x5BFF60` calls it).
pub fn with_sockets(own: &ItemValues, socketed: &[Socketed]) -> ItemValues {
    let mut values = *own;
    for (socket, kind) in socketed.iter().zip(own.sockets) {
        let mut added = Values::default();
        added.add(&Values::of(&socket.values), kind, socket.tier);
        added.onto(&mut values);
    }
    values
}

/// What the items in an item's sockets `socketed` count in its wearer's values besides what they
/// add to the item: their bonus slots alone, which the stats (`0x5796A0`) and the attack record
/// (`0x568D70`) take after the item's own, of each socket of a kind other than 0.
pub fn bonuses(socketed: &[Socketed]) -> impl Iterator<Item = ItemValues> + '_ {
    socketed.iter().map(|socket| ItemValues {
        bonuses: socket.values.bonuses,
        ..ItemValues::default()
    })
}

/// How many of the bonus slots of an item and of its sockets' items its tooltip merges.
const MERGED: usize = 6;

/// The bonus slots of an item `own` and of its sockets' items `socketed` as its tooltip shows
/// them (`0x5C4220`): each of a socket's first six added to a slot of the item's first six of the
/// same condition and effect, then each socket's first six taking the slots of the same condition
/// and effect that follow it, its own and the later sockets', every slot taken cleared. The
/// item's slots, then what is left of each socket's, which its own lines show.
pub fn shown_bonuses(
    own: [Bonus; BONUSES],
    socketed: &[Socketed],
) -> ([Bonus; BONUSES], Vec<[Bonus; BONUSES]>) {
    let same = |a: Bonus, b: Bonus| a.condition == b.condition && a.effect == b.effect;
    let mut item = own;
    let mut sockets: Vec<[Bonus; BONUSES]> = socketed.iter().map(|s| s.values.bonuses).collect();
    for socket in &mut sockets {
        for taken in socket.iter_mut().take(MERGED) {
            for slot in item.iter_mut().take(MERGED) {
                if same(*slot, *taken) {
                    slot.value = slot.value.wrapping_add(taken.value);
                    *taken = Bonus::default();
                }
            }
        }
    }
    for a in 0..sockets.len() {
        for j in 0..MERGED {
            for b in a..sockets.len() {
                let from = if b == a { j + 1 } else { 0 };
                for k in from..MERGED {
                    let taken = sockets[b][k];
                    if same(sockets[a][j], taken) {
                        sockets[a][j].value = sockets[a][j].value.wrapping_add(taken.value);
                        sockets[b][k] = Bonus::default();
                    }
                }
            }
        }
    }
    (item, sockets)
}

/// `value` as a 16-bit word, truncated toward zero as the record stores it.
fn word(value: f32) -> i16 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "an item's record holds each value as a 16-bit word"
    )]
    let word = value as i16;
    word
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

    /// An item of type `item` as a drop makes it, of `values` where the item maker gave some.
    fn item(item: u32, values: Option<ItemValues>) -> Dropped {
        Dropped {
            kind: crate::drops::Kind::Other,
            item,
            level: 0,
            key: 16,
            quality: 0,
            amount: 1,
            values,
            named: None,
            socketed: Vec::new(),
        }
    }

    #[test]
    fn an_item_takes_rings_into_its_sockets_until_they_are_filled_and_gives_one_back() {
        let kinds = [1, 3, 0, 0, 0, 0, 0, 0];
        let mut sword = item(7901, None);
        let ring = item(4876, None);
        let ring_values = ItemValues {
            attack: 100.0,
            ..ItemValues::default()
        };
        // A ring into the bronze socket by a smith of tier 2, a rune into the gold one by tier 0.
        assert_eq!(
            fill(&mut sword, &kinds, (ring.clone(), ring_values, 8), 2),
            None
        );
        let rune = item(2000, None);
        assert_eq!(
            fill(
                &mut sword,
                &kinds,
                (rune.clone(), ItemValues::default(), 28),
                0
            ),
            None
        );
        assert_eq!(sword.socketed.len(), 2);
        assert_eq!((sword.socketed[0].tier, sword.socketed[1].tier), (2, 0));
        // Every socket filled: refused, the item given back.
        assert_eq!(
            fill(&mut sword, &kinds, (ring.clone(), ring_values, 8), 0),
            Some(ring.clone())
        );
        // A weapon goes into no socket.
        let mut empty = item(7901, None);
        let dagger = item(1712, None);
        assert_eq!(
            fill(
                &mut empty,
                &kinds,
                (dagger.clone(), ItemValues::default(), 5),
                0
            ),
            Some(dagger)
        );
        // An item of no sockets takes none.
        assert_eq!(
            fill(&mut empty, &[0; SOCKETS], (ring.clone(), ring_values, 8), 0),
            Some(ring.clone())
        );
        // Taking the rune out empties both sockets and gives the rune alone back.
        assert_eq!(take_out(&mut sword, 1), Some(rune));
        assert!(sword.socketed.is_empty());
        assert_eq!(take_out(&mut sword, 0), None);
    }

    #[test]
    fn an_items_values_take_its_sockets_items_by_their_kinds_and_tiers() {
        let own = ItemValues {
            damage: [[21.0, 23.0], [0.0; 2], [0.0; 2], [0.0; 2]],
            attack: 5.0,
            defence: 2.0,
            sockets: [1, 3, 0, 0, 0, 0, 0, 0],
            ..ItemValues::default()
        };
        let ring = ItemValues {
            damage: [[0.0; 2], [10.0, 20.0], [0.0; 2], [0.0; 2]],
            attack: 100.0,
            resistance: [0.0, 0.0, 30.0, 0.0],
            bonuses: [crate::fight::Bonus {
                condition: 0,
                effect: 815,
                value: 4,
            }; crate::fight::BONUSES],
            ..ItemValues::default()
        };
        let socketed = |tier| Socketed {
            item: item(4876, None),
            values: ring,
            tier,
        };
        // In the bronze socket by tier 0, times 1.01 a little below: 20 stays 20, 100 stays 100.
        let one = with_sockets(&own, &[socketed(0)]);
        assert_eq!(one.damage[1], [10.0, 20.0]);
        assert_eq!((one.attack, one.defence), (105.0, 2.0));
        assert_eq!(one.resistance[2], 30.0);
        // A second ring in the gold socket by tier 2: times 1.11, so 100 gives 110 and 30 33.
        let two = with_sockets(&own, &[socketed(0), socketed(2)]);
        assert_eq!(two.damage[1], [10.0 + 11.0, 20.0 + 22.0]);
        assert_eq!(two.attack, 5.0 + 100.0 + 110.0);
        assert_eq!(two.resistance[2], 30.0 + 33.0);
        // The item's own damage, its bonuses and its sockets' kinds stay as they were.
        assert_eq!(two.damage[0], own.damage[0]);
        assert_eq!((two.bonuses, two.sockets), (own.bonuses, own.sockets));
        assert_eq!(with_sockets(&own, &[]), own);
        // Their bonus slots count where the item is worn, their other values not again.
        let counted: Vec<ItemValues> = bonuses(&[socketed(0), socketed(2)]).collect();
        assert_eq!(counted.len(), 2);
        assert!(counted.iter().all(|values| values.bonuses == ring.bonuses
            && values.attack == 0.0
            && values.damage == [[0.0; 2]; KINDS]));
    }

    #[test]
    fn a_tooltip_merges_the_sockets_bonuses_of_the_same_condition_and_effect() {
        let bonus = |condition, effect, value| Bonus {
            condition,
            effect,
            value,
        };
        let mut own = [Bonus::default(); BONUSES];
        own[0] = bonus(0, 815, 5);
        own[1] = bonus(0x80, 841, 10);
        // The item's seventh slot is not among those merged into.
        own[6] = bonus(0, 816, 1);
        let socket = |slots: &[(usize, Bonus)]| {
            let mut values = ItemValues::default();
            for &(at, slot) in slots {
                values.bonuses[at] = slot;
            }
            Socketed {
                item: item(4876, None),
                values,
                tier: 0,
            }
        };
        let first = socket(&[
            (0, bonus(0, 815, 3)),
            (1, bonus(0, 816, 2)),
            (2, bonus(0, 817, 4)),
            (3, bonus(0, 817, 1)),
            (6, bonus(0, 815, 9)),
        ]);
        let second = socket(&[(0, bonus(0x80, 841, 7)), (4, bonus(0, 817, 6))]);
        let (item, sockets) = shown_bonuses(own, &[first, second]);
        // The item's 815 and 841 take the sockets' of the same condition, its 816 of the seventh
        // slot nothing.
        assert_eq!(item[0], bonus(0, 815, 8));
        assert_eq!(item[1], bonus(0x80, 841, 17));
        assert_eq!(item[6], bonus(0, 816, 1));
        // The first socket's 817 takes its own later one and the second socket's; its 816 and
        // its seventh slot's 815 stay its own.
        assert_eq!(sockets[0][0], Bonus::default());
        assert_eq!(sockets[0][1], bonus(0, 816, 2));
        assert_eq!(sockets[0][2], bonus(0, 817, 11));
        assert_eq!(sockets[0][3], Bonus::default());
        assert_eq!(sockets[0][6], bonus(0, 815, 9));
        assert!(sockets[1].iter().all(|slot| *slot == Bonus::default()));
        assert_eq!(shown_bonuses(own, &[]), (own, Vec::new()));
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
