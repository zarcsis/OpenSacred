//! The equipment window's slots and who may use an item (#1288; docs/re/inventory.md, §4): the
//! category each slot takes (`0x43EFC0`), each class's layout of the slots' frames (`0x6B7700`
//! and the class's builder), and the use's test of an item's classes, its weapon's kind and the
//! level it asks (`0x54ABE0`, `0x5BF810`, `0x5BEB30`).

/// The slots of a creature's equipment (docs/re/equipment.md).
pub const SLOTS: u8 = 19;

/// Whether the slot `slot` of the equipment window takes an item of the category `category`
/// (`0x43EFC0`): the helmets 17 in 0, the armour 6 in 1, the belts 19 in 2, the boots 18 in 3, the
/// gloves 24 in 4, the arms 22 in 5, the amulets 20 in 6 and 7, the rings 8 in 8 to 11, the cannons
/// 33 in 14, the shoulders 21 in 15, the greaves 23 in 16 and the wings 25 in 17; the hands and
/// the horse have no frame in the window.
pub fn takes(slot: u8, category: u8) -> bool {
    let wanted = match slot {
        0 => 17,
        1 => 6,
        2 => 19,
        3 => 18,
        4 => 24,
        5 => 22,
        6 | 7 => 20,
        8..=11 => 8,
        14 => 33,
        15 => 21,
        16 => 23,
        17 => 25,
        _ => return false,
    };
    category == wanted
}

/// A slot's frame in the equipment window: the slot, its image of the interface's atlases and its
/// top left in the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub slot: u8,
    pub image: u32,
    pub at: (u16, u16),
}

const fn frame(slot: u8, image: u32, at: (u16, u16)) -> Frame {
    Frame { slot, image, at }
}

/// The frames of the hero of the type `hero` (1 to 9), as its class's builder lays them out
/// (docs/re/inventory.md, §4's table): the Seraphim's, the Gladiator's, the Mage's and the Wood
/// Elf's, the Dark Elf's, the Vampiress's and the Daemon's, and the Dwarf's.
pub fn frames(hero: u32) -> Vec<Frame> {
    const HELMET: u32 = 146;
    const ARMOUR: u32 = 147;
    const TALL_BELT: u32 = 148;
    const BELT: u32 = 149;
    const BOOTS: u32 = 150;
    const GLOVES: u32 = 151;
    const ARMS: u32 = 152;
    const AMULETS: [u32; 2] = [153, 154];
    const RINGS: [u32; 4] = [155, 156, 157, 158];
    const SHOULDER: u32 = 162;
    const GREAVES: u32 = 163;
    const WINGS: u32 = 164;
    const CANNON: u32 = 1611;
    let mut frames = vec![frame(0, HELMET, (96, 6))];
    match hero {
        1 => frames.extend([
            frame(1, ARMOUR, (16, 80)),
            frame(2, TALL_BELT, (176, 186)),
            frame(3, BOOTS, (96, 186)),
            frame(5, ARMS, (16, 186)),
            frame(6, AMULETS[0], (208, 74)),
            frame(8, RINGS[0], (208, 112)),
            frame(9, RINGS[1], (208, 148)),
            frame(15, SHOULDER, (176, 6)),
            frame(17, WINGS, (96, 80)),
        ]),
        2 => frames.extend([
            frame(1, ARMOUR, (96, 80)),
            frame(2, TALL_BELT, (176, 186)),
            frame(3, BOOTS, (16, 186)),
            frame(4, GLOVES, (16, 146)),
            frame(5, ARMS, (16, 76)),
            frame(6, AMULETS[0], (192, 82)),
            frame(8, RINGS[0], (176, 120)),
            frame(9, RINGS[1], (208, 120)),
            frame(15, SHOULDER, (176, 6)),
            frame(16, GREAVES, (96, 186)),
        ]),
        8 => frames.extend([
            frame(1, ARMOUR, (96, 80)),
            frame(2, BELT, (96, 186)),
            frame(3, BOOTS, (16, 186)),
            frame(4, GLOVES, (16, 146)),
            frame(6, AMULETS[0], (176, 138)),
            frame(7, AMULETS[1], (208, 138)),
            frame(8, RINGS[0], (176, 172)),
            frame(9, RINGS[1], (208, 172)),
            frame(10, RINGS[2], (176, 206)),
            frame(11, RINGS[3], (208, 206)),
            frame(14, CANNON, (176, 6)),
            frame(15, SHOULDER, (16, 76)),
        ]),
        // The Mage and the Wood Elf, and the Dark Elf, the Vampiress and the Daemon.
        _ => {
            let mage = matches!(hero, 3 | 5);
            frames.extend([
                frame(1, ARMOUR, (96, 80)),
                frame(2, BELT, (176, 218)),
                frame(3, BOOTS, (16, 186)),
                frame(4, GLOVES, (16, 146)),
                frame(5, ARMS, (16, 76)),
                frame(8, RINGS[0], (176, 120)),
                frame(9, RINGS[1], (208, 120)),
                frame(10, RINGS[2], (176, 158)),
                frame(11, RINGS[3], (208, 158)),
                frame(16, GREAVES, (96, 186)),
            ]);
            if mage {
                frames.extend([
                    frame(6, AMULETS[0], (176, 82)),
                    frame(7, AMULETS[1], (208, 82)),
                ]);
            } else {
                frames.extend([
                    frame(6, AMULETS[0], (192, 82)),
                    frame(15, SHOULDER, (176, 6)),
                ]);
            }
        }
    }
    frames
}

/// What an item asks of the hero that would use it: its type, its category (`items.pak +0x2E`),
/// its weapon's kind (`Weapon.pak +0x1E`, 0 for none), the classes' bits of its record's `+0x04`,
/// and the level it asks (`+0x14`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Asks {
    pub item: u32,
    pub category: u8,
    pub kind: u8,
    pub classes: u32,
    pub level: u32,
}

/// The classes' bits of the record's `+0x04`: the types 1 to 6 in the bits 0 to 5 (the
/// Vampiress's night form 7 as her day's), the Dwarf in 22, the Daemon in 23; and the bit 6, no
/// class.
pub const CLASSES: u32 = 0x00C0_003F;
pub const NO_CLASS: u32 = 0x40;

/// The bit of the hero of the type `hero` among [`CLASSES`].
pub fn class_bit(hero: u32) -> u32 {
    match hero {
        1..=6 => 1 << (hero - 1),
        7 => 1 << 5,
        8 => 1 << 22,
        9 => 1 << 23,
        _ => 0,
    }
}

/// The level an item of the item's level `level` asks (`0x5D3AB0`, `0x5D4973`):
/// `⌊max((level + 9)·0.8 − 8.5, 1)⌋` (docs/re/items.md, "The level an item asks", #1303).
pub fn asked_level(level: u32) -> u32 {
    let asked = ((f64::from(level) + 9.0) * 0.8 - 8.5).max(1.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a level of an item, from 1 to about 210"
    )]
    let asked = asked.floor() as u32;
    asked
}

/// The bit among [`CLASSES`] of the class whose items a creature of the type `creature` takes
/// (`0x5BF810`): a hero's own ([`class_bit`]), the Seraphim's for the type 132, the Dark Elf's
/// for 133, 319 to 321, 326, 327 and 693, the Wood Elf's for 105 and 134; none for another.
fn taker_bit(creature: u32) -> u32 {
    match creature {
        132 => class_bit(1),
        133 | 319..=321 | 326 | 327 | 693 => class_bit(4),
        105 | 134 => class_bit(5),
        _ => class_bit(creature),
    }
}

/// The types below which the bit 6 of an item's classes refuses its taker, the heroes' among
/// them (`0x5BF810`): a hireling's type is above.
const CLASSED_BELOW: u32 = 0x20;

/// Whether a creature of the type `creature` and the level `level` may use an item that `asks`
/// so (`0x54ABE0`): its classes (no bit or every bit any class; the bit 6, which the types 1800
/// to 1804 take, none below the type 32 and the classes' bits cleared above it; else the
/// creature's own, a hero's or that of a type of [`taker_bit`], none for another; `0x5BF810`),
/// the weapons' kinds (the Dwarf not the kinds 4, 7 and 12 nor the bridles 29, the Daemon not 7,
/// the kind 3 the Gladiator's and the Dwarf's alone), and the level it asks (`0x5BEB30`). A
/// class's variant (`0x43CA20`) and the attributes' and the skill's requirements are not
/// followed.
pub fn may_use(creature: u32, level: u32, asks: &Asks) -> bool {
    let dwarf = creature == 8;
    let refused_kind = match asks.kind {
        4 | 12 => dwarf,
        7 => dwarf || creature == 9,
        3 => !(creature == 2 || dwarf),
        _ => false,
    };
    if refused_kind || (dwarf && asks.category == 29) {
        return false;
    }
    let no_class = asks.classes & NO_CLASS != 0 || (1800..=1804).contains(&asks.item);
    if no_class && creature < CLASSED_BELOW {
        return false;
    }
    let classes = if (1800..=1804).contains(&asks.item) {
        0
    } else {
        asks.classes & CLASSES
    };
    if classes != 0 && classes != CLASSES && classes & taker_bit(creature) == 0 {
        return false;
    }
    asks.level <= level
}

/// Why the hero may not use an item: its classes (the hero's line `NOTMYITEM`, the event `(9, 6)`),
/// or anything else, the level among it (`NOTMYITEMLVL`, `(9, 8)`; `0x6B6690`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Class,
    Level,
}

/// Why the hero of the type `hero` and the level `level` may not use an item that `asks` so, none
/// where it may.
pub fn refusal(hero: u32, level: u32, asks: &Asks) -> Option<Refusal> {
    if may_use(hero, level, asks) {
        return None;
    }
    let classes_only = Asks { level: 0, ..*asks };
    Some(if may_use(hero, u32::MAX, &classes_only) {
        Refusal::Level
    } else {
        Refusal::Class
    })
}

/// The slot that an item of the category `category` sent from the grid goes into (the UI event
/// `0x2C` of the sub-kind 1, `0x6B6690`): the first of the window's slots 0 to 18 that takes it and
/// is empty, else the first that takes it; none where none takes it.
pub fn slot_for(category: u8, empty: impl Fn(u8) -> bool) -> Option<u8> {
    let taking: Vec<u8> = (0..SLOTS).filter(|&slot| takes(slot, category)).collect();
    taking
        .iter()
        .copied()
        .find(|&slot| empty(slot))
        .or_else(|| taking.first().copied())
}

/// The slot that an item the hero of the type `hero` and the level `level` picks up goes into by
/// itself (`cCreature::pickupItem`, `0x558080`, docs/re/items.md, "What becomes of the item",
/// #1601): where the hero may use it (`0x54ABE0`), the first of its class's slots (`0x43F3B0`, the
/// window's frames) that takes it (`0x43EFC0`) and is `empty`, holding nothing or a class's
/// default; none where none is, the item going on into the grid. The hands (the slots 12 and 13)
/// are not among them.
pub fn slot_on_pickup(
    hero: u32,
    level: u32,
    asks: &Asks,
    empty: impl Fn(u8) -> bool,
) -> Option<u8> {
    if !may_use(hero, level, asks) {
        return None;
    }
    let mut slots: Vec<u8> = frames(hero).iter().map(|frame| frame.slot).collect();
    slots.sort_unstable();
    slots
        .into_iter()
        .find(|&slot| takes(slot, asks.category) && empty(slot))
}

/// The hands' slots, the left's and the right's (`0x556F50`'s 12 and 13).
pub const LEFT_HAND: u8 = 12;
pub const RIGHT_HAND: u8 = 13;

/// The types held as two-handed weapons of the right hand whatever their kind, by their record's
/// parent, else their own type (`0x5CEC00`, `0x5CECD0`).
const BOTH_HANDS: [u32; 6] = [4067, 4068, 4069, 4070, 4071, 4072];

/// How a creature holds an item: whether it goes into the right hand (`0x5CEC00`'s 2 and 3),
/// else the left, and whether it takes both hands (`0x5CECD0`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Grip {
    pub right: bool,
    pub both: bool,
}

/// How a creature holds a weapon of the kind `kind` (`Weapon.pak +0x1E`) whose record's parent,
/// else its own type, is `root` (`0x5CEC00`, `0x5CECD0`): the kinds 3, 7 and 12 in the left hand,
/// the others in the right; the kinds 2, 3, 4, 7 and 12 and the types of [`BOTH_HANDS`] in both.
/// A shield goes into the left hand alone (its grip 1, `0x6D6760`): [`Grip::default`].
pub fn grip(kind: u8, root: u32) -> Grip {
    if BOTH_HANDS.contains(&root) {
        return Grip {
            right: true,
            both: true,
        };
    }
    Grip {
        right: !matches!(kind, 3 | 7 | 12),
        both: matches!(kind, 2 | 3 | 4 | 7 | 12),
    }
}

/// What a hireling's hands hold: for each hand, whether it holds an item and whether that item
/// takes both hands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hands {
    pub left: Option<bool>,
    pub right: Option<bool>,
}

/// Where an item given to a hireling goes: the hand that takes it, and the hand whose item comes
/// off onto the cursor, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handing {
    pub into: u8,
    pub off: Option<u8>,
}

/// Where a hireling of the hands `hands` puts an item of the grip `grip` given through its
/// portrait (`cUI_Mercenary`, `0x6D6760`), with the skill of two weapons (6, `0x582450`) or not.
///
/// An item of one hand goes into the hand of its grip, without the skill; with it, into the
/// right hand where the left holds an item or the right none, else into the left. The item it
/// replaces comes off, else an item of both hands in the left, else in the right.
///
/// An item of both hands goes into the hand of its grip where one hand is empty, the item of the
/// other coming off; with both hands full it goes nowhere, and the cursor keeps it.
pub fn handed(grip: Grip, hands: Hands, two_weapons: bool) -> Option<Handing> {
    let of_grip = if grip.right { RIGHT_HAND } else { LEFT_HAND };
    if grip.both {
        let off = match (hands.left, hands.right) {
            (None, right) => right.map(|_| RIGHT_HAND),
            (Some(_), None) => Some(LEFT_HAND),
            (Some(_), Some(_)) => return None,
        };
        return Some(Handing { into: of_grip, off });
    }
    let into = match two_weapons {
        false => of_grip,
        true if hands.left.is_some() || hands.right.is_none() => RIGHT_HAND,
        true => LEFT_HAND,
    };
    let held = if into == LEFT_HAND {
        hands.left
    } else {
        hands.right
    };
    let off = match held {
        Some(_) => Some(into),
        None if hands.left == Some(true) => Some(LEFT_HAND),
        None if hands.right == Some(true) => Some(RIGHT_HAND),
        None => None,
    };
    Some(Handing { into, off })
}

/// An item's worth to a hireling that compares it with its hands' (its object's query 4,
/// `cWeapon3D`'s `0x5CED50`): the sum of its four kinds' most damages, its record's whole numbers
/// (`+0x24` to `+0x2A`).
#[expect(
    clippy::cast_possible_truncation,
    reason = "a record's damage, a whole number of 16 bits stored as a float"
)]
pub fn hand_worth(values: &crate::fight::ItemValues) -> i32 {
    values.damage.iter().map(|&[_, most]| most as i32).sum()
}

/// How an item of the worth `given` compares with the items of the worths `left` and `right` that
/// a hireling's hands held, each at least 1, in hundredths (`0x6D6660`, the event (8, `0x10`)'s
/// value): `s = (given/(left + right) − 1)/2 + 1/2`, held between 0 and 1, stored as a float, then
/// times 100 and truncated (`0x6D6AA2`).
pub fn compared(given: i32, left: i32, right: i32) -> u32 {
    let held = f64::from(left.max(1)) + f64::from(right.max(1));
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the share stored as the float of the x87 unit's value"
    )]
    let s = ((f64::from(given) / held - 1.0) * 0.5 + 0.5).clamp(0.0, 1.0) as f32;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a share of 0 to 1 in hundredths"
    )]
    let value = (f64::from(s) * 100.0) as u32;
    value
}

/// How a slot of the equipment window glows while the cursor holds an item (docs/re/inventory.md,
/// §4; seen with a potion held, every slot red): green where it takes the item and the hero may
/// use it, red otherwise; white with nothing held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glow {
    White,
    Green,
    Red,
}

/// The glow of the slot `slot` while the cursor holds an item that `asks` so, none held where
/// `None`, for the hero of the type `hero` and the level `level`.
pub fn glow(slot: u8, held: Option<&Asks>, (hero, level): (u32, u32)) -> Glow {
    match held {
        None => Glow::White,
        Some(asks) if takes(slot, asks.category) && may_use(hero, level, asks) => Glow::Green,
        Some(_) => Glow::Red,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_slot_takes_its_category() {
        assert!(takes(0, 17) && takes(7, 20) && takes(11, 8) && takes(17, 25));
        assert!(!takes(0, 6) && !takes(12, 13) && !takes(13, 5) && !takes(18, 29));
    }

    #[test]
    fn each_class_lays_its_frames_out() {
        let at = |hero, slot| {
            frames(hero)
                .iter()
                .find(|frame| frame.slot == slot)
                .map(|frame| (frame.image, frame.at))
        };
        // The Seraphim's wings in the middle, her armour on the left, one amulet and two rings.
        assert_eq!(at(1, 17), Some((164, (96, 80))));
        assert_eq!(at(1, 1), Some((147, (16, 80))));
        assert_eq!(at(1, 7), None);
        assert_eq!(frames(1).len(), 10);
        // The Mage's two amulets and four rings; the Daemon's one amulet and her shoulder.
        assert_eq!(at(3, 7), Some((154, (208, 82))));
        assert_eq!(at(3, 11), Some((158, (208, 158))));
        assert_eq!(at(9, 6), Some((153, (192, 82))));
        // Her four rings, as the Mage's (seen in the original).
        assert_eq!(at(9, 11), Some((158, (208, 158))));
        assert_eq!(at(9, 15), Some((162, (176, 6))));
        // The Dwarf's cannon and his low belt.
        assert_eq!(at(8, 14), Some((1611, (176, 6))));
        assert_eq!(at(8, 2), Some((149, (96, 186))));
    }

    #[test]
    fn the_use_asks_the_class_the_kind_and_the_level() {
        let sword = Asks {
            item: 1712,
            category: 5,
            kind: 5,
            classes: 0x80CF_013F,
            level: 22,
        };
        // Every class's bit: any hero, from the level asked on (seen: 22, #1303).
        assert!(!may_use(9, 21, &sword) && may_use(9, 22, &sword) && may_use(1, 30, &sword));
        // The Gladiator's kilt, his class's alone.
        let kilt = Asks {
            item: 4054,
            category: 18,
            classes: 0x800F_1302,
            level: 1,
            ..Asks::default()
        };
        assert!(may_use(2, 1, &kilt) && !may_use(1, 1, &kilt));
        // The Dwarf's goggles, his bit 22.
        let goggles = Asks {
            classes: 0x8040_1000,
            ..kilt
        };
        assert!(may_use(8, 1, &goggles) && !may_use(9, 1, &goggles));
        // The kinds: a bow (7) not the Daemon's, the kind 3 the Gladiator's and the Dwarf's.
        let bow = Asks { kind: 7, ..sword };
        assert!(!may_use(9, 30, &bow) && may_use(1, 30, &bow));
        let three = Asks { kind: 3, ..sword };
        assert!(may_use(2, 30, &three) && may_use(8, 30, &three) && !may_use(1, 30, &three));
        // No class at all.
        assert!(!may_use(
            1,
            30,
            &Asks {
                classes: 0x40,
                ..sword
            }
        ));
    }

    #[test]
    fn a_hireling_takes_the_items_of_every_class_and_of_none() {
        let sword = Asks {
            item: 1712,
            category: 5,
            kind: 5,
            classes: 0x80CF_013F,
            level: 22,
        };
        // A hireling of the type 77: every class's item from the level asked on, no class's
        // too, a class's own not, nor the kind 3.
        assert!(may_use(77, 22, &sword) && !may_use(77, 21, &sword));
        let none = Asks {
            classes: 0x40,
            ..sword
        };
        assert!(may_use(77, 22, &none) && !may_use(10, 22, &none));
        let kilt = Asks {
            classes: 0x800F_1302,
            ..sword
        };
        assert!(!may_use(77, 22, &kilt) && may_use(2, 22, &kilt));
        assert!(!may_use(77, 22, &Asks { kind: 3, ..sword }));
        // The types 1800 to 1804: no hero's, a hireling's whatever their classes.
        let held = Asks { item: 1802, ..kilt };
        assert!(may_use(77, 22, &held) && !may_use(2, 22, &held));
        // The type 133 takes the Dark Elf's items.
        let dark = Asks {
            classes: 0x8000_0008,
            ..sword
        };
        assert!(may_use(133, 22, &dark) && !may_use(132, 22, &dark));
    }

    #[test]
    fn a_hireling_puts_an_item_given_into_the_hand_of_its_grip() {
        let sword = grip(1, 1712);
        let bow = grip(7, 1732);
        let shield = Grip::default();
        assert_eq!(
            (sword, bow, grip(2, 1700)),
            (
                Grip {
                    right: true,
                    both: false
                },
                Grip {
                    right: false,
                    both: true
                },
                Grip {
                    right: true,
                    both: true
                }
            )
        );
        assert_eq!(
            grip(5, 4069),
            Grip {
                right: true,
                both: true
            }
        );
        let one = Some(false);
        let two = Some(true);
        let at = |into, off| Some(Handing { into, off });
        // A sword for a sword, the old one onto the cursor; into empty hands, nothing off.
        let swords = Hands {
            left: None,
            right: one,
        };
        assert_eq!(
            handed(sword, swords, false),
            at(RIGHT_HAND, Some(RIGHT_HAND))
        );
        assert_eq!(handed(sword, Hands::default(), false), at(RIGHT_HAND, None));
        // A shield beside the sword; a sword beside a bow takes the bow off.
        assert_eq!(handed(shield, swords, false), at(LEFT_HAND, None));
        let archer = Hands {
            left: two,
            right: None,
        };
        assert_eq!(
            handed(sword, archer, false),
            at(RIGHT_HAND, Some(LEFT_HAND))
        );
        // With two weapons a second sword goes into the empty left hand.
        assert_eq!(handed(sword, swords, true), at(LEFT_HAND, None));
        // A bow for a sword: the sword off, the bow into the left hand; with both hands full the
        // cursor keeps it.
        assert_eq!(handed(bow, swords, false), at(LEFT_HAND, Some(RIGHT_HAND)));
        let full = Hands {
            left: one,
            right: one,
        };
        assert_eq!(handed(bow, full, false), None);
    }

    #[test]
    fn a_hireling_compares_an_item_given_with_its_hands_old_ones() {
        // Into empty hands, each counted at 1: as good as it gets (seen in the original: 100).
        assert_eq!(compared(30, 0, 0), 100);
        // A weapon of 40 for one of 39 and the empty hand's 1: the middle; for one of 40, a
        // little below it.
        assert_eq!(compared(40, 39, 0), 50);
        assert_eq!(compared(40, 40, 0), 48);
        // Half as good: a quarter; none, a shield's: 0; three times: held at 1.
        assert_eq!(compared(20, 39, 0), 25);
        assert_eq!(compared(0, 40, 0), 0);
        assert_eq!(compared(120, 40, 0), 100);
        let mut sword = crate::fight::ItemValues::default();
        sword.damage[0] = [10.0, 25.0];
        sword.damage[1] = [2.0, 6.0];
        assert_eq!(hand_worth(&sword), 31);
    }

    #[test]
    fn the_level_asked_follows_the_items_level() {
        assert_eq!(asked_level(30), 22);
        assert_eq!(asked_level(8), 5);
        assert_eq!(asked_level(1), 1);
    }

    #[test]
    fn a_refusal_names_the_class_or_the_level_and_a_slot_is_chosen() {
        let kilt = Asks {
            item: 4054,
            category: 18,
            classes: 0x800F_1302,
            level: 5,
            ..Asks::default()
        };
        assert_eq!(refusal(1, 30, &kilt), Some(Refusal::Class));
        assert_eq!(refusal(2, 1, &kilt), Some(Refusal::Level));
        assert_eq!(refusal(2, 5, &kilt), None);
        // A ring: the first empty of the four, else the first.
        assert_eq!(slot_for(8, |slot| slot != 8), Some(9));
        assert_eq!(slot_for(8, |_| false), Some(8));
        assert_eq!(slot_for(9, |_| true), None);
    }

    #[test]
    fn an_item_picked_up_goes_into_the_first_empty_slot_of_the_class_that_takes_it() {
        let ring = Asks {
            category: 8,
            ..Asks::default()
        };
        // The Seraphim's two rings, the first empty of them.
        assert_eq!(slot_on_pickup(1, 1, &ring, |_| true), Some(8));
        assert_eq!(slot_on_pickup(1, 1, &ring, |slot| slot != 8), Some(9));
        assert_eq!(slot_on_pickup(1, 1, &ring, |slot| slot < 8), None);
        // The Mage's four.
        assert_eq!(slot_on_pickup(3, 1, &ring, |slot| slot > 10), Some(11));
        // Gloves: the Seraphim has no frame of them, the Gladiator has.
        let gloves = Asks {
            category: 24,
            ..Asks::default()
        };
        assert_eq!(slot_on_pickup(1, 1, &gloves, |_| true), None);
        assert_eq!(slot_on_pickup(2, 1, &gloves, |_| true), Some(4));
        // Not where the hero may not use it, nor what no slot takes.
        let kilt = Asks {
            category: 18,
            classes: 0x800F_1302,
            level: 5,
            ..Asks::default()
        };
        assert_eq!(slot_on_pickup(1, 30, &kilt, |_| true), None);
        assert_eq!(slot_on_pickup(2, 1, &kilt, |_| true), None);
        assert_eq!(slot_on_pickup(2, 5, &kilt, |_| true), Some(3));
        let potion = Asks {
            category: 9,
            ..Asks::default()
        };
        assert_eq!(slot_on_pickup(1, 1, &potion, |_| true), None);
    }

    #[test]
    fn a_slot_glows_by_the_item_held() {
        let helmet = Asks {
            item: 5000,
            category: 17,
            classes: CLASSES,
            level: 1,
            ..Asks::default()
        };
        let potion = Asks {
            category: 9,
            ..helmet
        };
        assert_eq!(glow(0, None, (1, 1)), Glow::White);
        assert_eq!(glow(0, Some(&helmet), (1, 1)), Glow::Green);
        assert_eq!(glow(1, Some(&helmet), (1, 1)), Glow::Red);
        assert_eq!(glow(0, Some(&potion), (1, 1)), Glow::Red);
        assert_eq!(
            glow(0, Some(&Asks { level: 5, ..helmet }), (1, 1)),
            Glow::Red
        );
    }
}
