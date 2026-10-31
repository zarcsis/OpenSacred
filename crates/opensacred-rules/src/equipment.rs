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

/// Whether the hero of the type `hero` and the level `level` may use an item that `asks` so
/// (`0x54ABE0`): its classes (no bit or every bit any class, the bit 6 none, else the hero's own;
/// `0x5BF810`), the weapons' kinds (the Dwarf not the kinds 4, 7 and 12 nor the bridles 29, the
/// Daemon not 7, the kind 3 the Gladiator's and the Dwarf's alone, the types 1800 to 1804 no
/// hero's), and the level it asks (`0x5BEB30`). A class's variant (`0x43CA20`) and the
/// attributes' and the skill's requirements are not followed.
pub fn may_use(hero: u32, level: u32, asks: &Asks) -> bool {
    let classes = asks.classes & (CLASSES | NO_CLASS);
    if classes & NO_CLASS != 0 {
        return false;
    }
    let classes = classes & CLASSES;
    if classes != 0 && classes != CLASSES && classes & class_bit(hero) == 0 {
        return false;
    }
    let dwarf = hero == 8;
    let refused_kind = match asks.kind {
        4 | 12 => dwarf,
        7 => dwarf || hero == 9,
        3 => !(hero == 2 || dwarf),
        _ => false,
    };
    if refused_kind || (dwarf && asks.category == 29) || (1800..=1804).contains(&asks.item) {
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
