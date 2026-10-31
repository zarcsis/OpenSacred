//! The names that the item maker gives the items it makes (`0x5C3130`, which `0x5D5080` calls
//! last; docs/re/items.md, "The made item's name"): a key of the game's texts that the item's
//! object keeps (`+0x3C`, set by `0x44A1B0`) in place of its type's, its family's generic name,
//! then one of the words of its quality, its element, its class and the count of its bonuses
//! where the texts hold it.

use super::{Catalogue, Dropped, Rules, WEAPONS};
use crate::fight::{ItemValues, Templates};
use crate::spawning::Dice;

/// The words of the families and of the classes by their number: a family's generic name is
/// `EWT_` and its word (`0x43B030`, the table `0x949930`); a class's word goes into the composed
/// name (the jump table `0x5C4194`), the class 32's `Kanone` where the family 32's is `Rucksack`.
const WORDS: [&str; 33] = [
    "Schwert",
    "Dolch",
    "Degen",
    "Saebel",
    "2HSchwert",
    "Axt",
    "2HAxt",
    "Schild",
    "Bogen",
    "Armbrust",
    "Klingenwaffe",
    "Kettenwaffe",
    "Peitsche",
    "Ruestung",
    "Ring",
    "Amulett",
    "Helm",
    "Armschiene",
    "Beinschiene",
    "Guertel",
    "Schulter",
    "Speer",
    "Keule",
    "Stab",
    "Magierstab",
    "Zaumzeug",
    "Schuhe",
    "Handschuhe",
    "Fluegel",
    "Item",
    "Pistole",
    "Muskete",
    "Rucksack",
];
/// The class of the two-handed sword, whose word is the sword's where its type's weapon kind is 1.
const TWO_HANDED_SWORD: usize = 4;
/// The class of the mage's staffs, which keep their type's own name where it has a text.
const MAGE_STAFF: usize = 24;
/// The class of the gloves, which keep their type's own name where it has a text and they are
/// of the weapons' category.
const GLOVES: usize = 27;
/// The class 32's word.
const CANNON: &str = "Kanone";
/// The quality of a made item's record (`+0x04` bits 16–19), which the quality pass `0x5D2E40`
/// writes after the maker's bonuses and sockets; neither is followed (#1367), so every made item
/// is of the quality 0.
const MADE_QUALITY: u32 = 0;
/// The effects' records of the balance's image from `+0x9DC`, of six floats each (`0x5E22B0`):
/// the effects 800 to 869, then those below 200, those above 999, 600 to 799, and the others.
pub(super) const NORMS: usize = 74;
/// The image's word of the first record.
const NORMS_AT: usize = 0x9DC / 4;
/// The share in per cent above which an element names an item: fire, magic, poison.
const SHARES: [u32; 3] = [51, 41, 31];

/// The effects' records of the balance's image `image` ([`NORMS`]); none where it is too short.
pub(crate) fn norms(image: &[u32]) -> Vec<[f32; 6]> {
    image
        .get(NORMS_AT..NORMS_AT + 6 * NORMS)
        .map(|words| {
            let (records, _) = words.as_chunks::<6>();
            records
                .iter()
                .map(|record| record.map(f32::from_bits))
                .collect()
        })
        .unwrap_or_default()
}

/// The name's roll of one in two of an item that the maker made, of the values `values`.
pub(super) fn roll(values: Option<&ItemValues>, dice: &mut impl Dice) -> Option<bool> {
    values.map(|_| dice.below(2) == 1)
}

/// The key of the game's texts that the item maker named `dropped` by (`0x5C3130`), where it
/// made it, of the names that `known` finds a text for (`sdk_QB_RESOLVE`); `None` where it goes
/// by its type's name (`0x44A250`). Of its type taken through its parents ([`Templates::root`]):
///
/// - its family's generic name, `EWT_` and the word of its template's word 38 (`0x43B030`);
/// - then, its template found, `Gen_`, its quality's band (`st_` below 5, `kl_` below 9, `mi_`
///   below 13, else `gr_`), its element's (`fe_`, `ma_` or `gi_` for an element whose share
///   passes 51, 41 or 31 per cent, else `ph_`; [`element`]) and its class's word, the word 38 or,
///   at −1, the one of its weapon kind; the gloves of the weapons' category and the mage's staffs
///   keep their type's own name where it has a text, and outside the 33 classes no word;
/// - with the count `n` of its bonuses ([`bonuses`]), the composed name and the highest digit
///   from `n` down that has a text, or for `n` 0 the digit 1 by the roll of one in two, else the
///   composed name alone where it has a text.
pub fn name(
    dropped: &Dropped,
    (rules, catalogue, templates): (&Rules, &Catalogue, &Templates),
    known: impl Fn(&str) -> bool,
) -> Option<String> {
    let coin = dropped.named?;
    let values = dropped.values.as_ref()?;
    let root = templates.root(dropped.item)?;
    let word_38 = templates.word_38(root)?;
    let family = usize::try_from(word_38 & 0xFF).unwrap_or(0);
    let generic = format!("EWT_{}", WORDS.get(family).unwrap_or(&WORDS[0]));
    let generic = known(&generic).then_some(generic);
    let record = catalogue.records.get(&root);
    // Its type's own name, which the gloves and the mage's staffs keep where it has a text, of
    // a record not of a set piece and of a quality below 4.
    let plain = record.is_some_and(|r| !r.set && (r.flags >> 16) & 0xF < 4);
    let own = || {
        let own = root.to_string();
        (plain && known(&own)).then_some(own)
    };
    let class = if word_38 == -1 {
        record.map_or(-1, |r| weapon_class(r.kind, catalogue.model(root)))
    } else {
        word_38
    };
    let word = match usize::try_from(class) {
        Ok(TWO_HANDED_SWORD) if record.is_some_and(|r| r.kind == 1) => WORDS[0],
        Ok(MAGE_STAFF) => {
            if let Some(own) = own() {
                return Some(own);
            }
            // Of another record the original names it by one of 40 numbers and a change of
            // `0x5C2E50`, not read: the generic name stays.
            if !plain {
                return generic;
            }
            WORDS[MAGE_STAFF]
        }
        Ok(GLOVES) if catalogue.categories.get(&root) == Some(&WEAPONS) => {
            if let Some(own) = own() {
                return Some(own);
            }
            WORDS[GLOVES]
        }
        Ok(32) => CANNON,
        Ok(class) => WORDS.get(class).copied().unwrap_or(""),
        Err(_) => "",
    };
    let composed = format!(
        "Gen_{}{}{word}",
        band(MADE_QUALITY),
        element_word(element(values))
    );
    let jewel = matches!(catalogue.categories.get(&root), Some(8 | 20));
    let known_key = |key: String| known(&key).then_some(key);
    let n = bonuses(values, jewel, &rules.norms);
    let digit = if n == 0 {
        coin.then(|| known_key(format!("{composed}1"))).flatten()
    } else {
        (1..=n)
            .rev()
            .find_map(|digit| known_key(format!("{composed}{digit}")))
    };
    digit.or_else(|| known_key(composed)).or(generic)
}

/// The class of a weapon of the kind `kind` (`Weapon.pak +0x1E`) whose template's word 38 is
/// −1, of the model `model` (`0x5C3130`, `0x5C3080`): a sword or an axe by its model's name, the
/// two-handed one's for the kind 2, and the others by their kinds; −1 for none.
fn weapon_class(kind: u8, model: Option<&str>) -> i64 {
    match kind {
        1 | 2 => {
            let model = model.unwrap_or_default();
            let sword = model.contains("SCHWERT") || model.contains("SWORD");
            let axe = !sword && (model.contains("AXT") || model.contains("AXE"));
            match (axe, kind) {
                (false, 2) => 4,
                (true, 2) => 6,
                (true, _) => 5,
                _ => 0,
            }
        }
        3 => 6,
        4 => 23,
        5 => 1,
        7 => 8,
        9 => 11,
        10 => 10,
        11 => 12,
        12 => 9,
        13 => 30,
        14 => 31,
        _ => -1,
    }
}

/// The band of the quality `quality` (`+0x04` bits 16–19) in a composed name.
fn band(quality: u32) -> &'static str {
    match quality {
        0..5 => "st_",
        5..9 => "kl_",
        9..13 => "mi_",
        _ => "gr_",
    }
}

/// The word of the element `element` ([`element`]) in a composed name: fire, magic or poison
/// where its share passes [`SHARES`], else the physical.
fn element_word(element: u32) -> &'static str {
    let share = element & 0x1FF;
    if element & 0x400 != 0 && share >= SHARES[0] {
        "fe_"
    } else if element & 0x800 != 0 && share >= SHARES[1] {
        "ma_"
    } else if element & 0x1000 != 0 && share >= SHARES[2] {
        "gi_"
    } else {
        "ph_"
    }
}

/// The element of an item of the values `values` (`0x5C2BF0`): of its most damages, then of its
/// resistances, the fire (`0x400`), the magic (`0x800`) or the poison (`0x1000`) where it alone
/// is the highest of the three and not 0, plus its share of the four in per cent; else of its
/// bonuses the first to the fire's, the magic's or the poison's damage or multiplier (802 to 808
/// but 805), plus twice its value; else 0 (**hypothesis**: the original returns what its x87
/// stack holds).
fn element(values: &ItemValues) -> u32 {
    let most = values.damage.map(|[_, most]| short(most));
    for [physical, fire, magic, poison] in [most, values.resistance.map(short)] {
        let total = physical + fire + magic + poison;
        let share = |part: i32| (part * 100).checked_div(total).unwrap_or(0);
        let alone = |part: i32, others: [i32; 2]| part != 0 && others.iter().all(|&o| o < part);
        for (part, others, bit) in [
            (fire, [magic, poison], 0x400),
            (magic, [fire, poison], 0x800),
            (poison, [fire, magic], 0x1000),
        ] {
            if alone(part, others) {
                return (share(part) + bit).cast_unsigned();
            }
        }
    }
    values
        .bonuses
        .iter()
        .find_map(|bonus| {
            let bit = match bonus.effect & 0xFFFF {
                0x322 | 0x326 => 0x400,
                0x323 | 0x327 => 0x800,
                0x324 | 0x328 => 0x1000,
                _ => return None,
            };
            Some((i32::from(bonus.value) * 2 + bit).cast_unsigned())
        })
        .unwrap_or(0)
}

/// How many bonuses an item of the values `values` counts in its name, at most 5 (`0x5C3130`):
/// each of an effect of the kind 0 or 1 ([`effect_kind`]) weighs `max(0.04·p + 0.5, 1)` of its
/// value `v`, `p = ⌊v·f·100/r + 0.5⌋` by its effect's record ([`norm`]), `r` the record's first
/// float where the bonus scales by an attribute (its effect's byte 2), else its fourth where its
/// third is below 0.00001, else `p` 0; `f` the record's sixth float for a ring or an amulet, else
/// 1; `p` an eighth for a bonus of an opponent's class; the count truncated after each. The
/// effects 840 to 869 count one each. The difficulty's factor is never taken: the original
/// compares the quality bits with the difficulty's.
fn bonuses(values: &ItemValues, jewel: bool, norms: &[[f32; 6]]) -> u32 {
    let mut n: u32 = 0;
    for bonus in &values.bonuses {
        let effect = bonus.effect & 0xFFFF;
        if effect == 0 {
            continue;
        }
        if effect_kind(effect) <= 1 {
            let p = norm(norms, effect).map_or(0, |record| {
                let factor = if jewel { f64::from(record[5]) } else { 1.0 };
                let weighed = |by: f32| {
                    truncated(f64::from(bonus.value) * factor * 100.0 / f64::from(by) + 0.5)
                };
                if bonus.effect & 0xFF_0000 != 0 {
                    weighed(record[0])
                } else if record[2] < SMALL {
                    weighed(record[3])
                } else {
                    0
                }
            });
            let p = if matches!(bonus.condition & 0xFF, 0 | 1) {
                p
            } else {
                p >> 3
            };
            let weight = (f64::from(p) * f64::from(WEIGHT) + 0.5).max(1.0);
            n = truncated(f64::from(n) + weight);
        } else if (0x348..0x367).contains(&effect) {
            n += 1;
        }
    }
    n.min(5)
}

/// The weight of a bonus's value in its count (`0x891B54`), a single-precision float.
const WEIGHT: f32 = 0.04;
/// The least third float of an effect's record that weighs a bonus by its first (`0x890C04`).
const SMALL: f32 = 0.000_01;

/// The kind of the effect `effect` (`0x5D2D70`): 0 for 801 to 804 and for every effect that no
/// other kind takes, 1 for 805 to 808, 2 to 5 for 809 to 820 by pairs and the attributes, 6 below
/// 201 and from 1000 to 1200, 7 for 600 to 798 and 8 for 840 to 869.
fn effect_kind(effect: u32) -> u8 {
    match effect {
        805..=808 => 1,
        809..=810 => 2,
        811..=812 => 3,
        813..=814 => 4,
        815..=820 => 5,
        0..=200 | 1000..=1200 => 6,
        600..=798 => 7,
        840..=869 => 8,
        _ => 0,
    }
}

/// The record of the effect `effect` of the effects' records `norms` (`0x5E22B0`).
pub(crate) fn norm(norms: &[[f32; 6]], effect: u32) -> Option<&[f32; 6]> {
    let index = match effect {
        0..200 => 70,
        1000.. => 71,
        600..800 => 72,
        800..870 => usize::try_from(effect - 800).ok()?,
        _ => 73,
    };
    norms.get(index)
}

/// A value of the record, whole, as a short of it.
#[expect(
    clippy::cast_possible_truncation,
    reason = "an item's damage or resistance, whole and within a short"
)]
fn short(value: f32) -> i32 {
    value as i32
}

/// A value truncated toward 0 and taken as 32 bits (`__ftol`), its low word as the original
/// reads it, unsigned.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the original truncates the value to an int and reads its 32 bits unsigned"
)]
fn truncated(value: f64) -> u32 {
    (value as i64) as u32
}

impl Catalogue {
    /// The `items.pak` name of the type `item`'s entry, its root's for a derived type.
    fn model(&self, item: u32) -> Option<&str> {
        let slot = self.slots.get(&item)?;
        self.models.get(slot).map(String::as_str)
    }
}
