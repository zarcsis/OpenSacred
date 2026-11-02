//! The lines of an item's tooltip (#1273; docs/re/inventory.md, §9), as the grid asks for them
//! (the flags `0x5F`): the name by the item's class (its virtual `+0x44`: a weapon's, an armour's
//! and a shield's with its difficulty's icon, its colour and its class, a weapon's lore and hands;
//! a ring's, an amulet's and a rune's, each with its sockets filled over its sockets; any other
//! plain), then `0x5C5360`'s: the refusal of the class, the price, the damage and the protection
//! as the hero would deal and take them, the hands' and the pace's slowing, the item's level, `A`
//! and `D`, each kind of damage with its bonuses, the requirements red where the hero falls short,
//! the resistances, the other bonuses (`0x5C4450`) and the hint of its use; then each socket's
//! item's own lines (#1691).
//!
//! The price's place factor is 1.0 offline, which a network game's message alone changes (#1728).
//!
//! Not followed: the quest's line (the quest items' page's), the class's variants in the refusal
//! (`0x43CA20`), the extra text of the item's `+0xFC`, and the hands of the types that the list
//! of `0x5CECD0` names.

use opensacred_rules::equipment::{CLASSES, NO_CLASS};
use opensacred_rules::fight::{Bonus, ItemValues, KINDS};

use super::{Line, Piece, WHITE};
use crate::drops::Record;

pub(crate) const RED: u32 = 0xFFFF_0000;
const GREY: u32 = 0xFFAF_AFAF;
const GREEN: u32 = 0xFF30_FF30;
const PROTECTION: u32 = 0xFF00_7700;
const GOLD: u32 = 0xFFD8_AB30;
const BLUE: u32 = 0xFF44_9BEA;
/// The kinds' colours and texts: physical, fire, magic and poison.
const KIND_COLOURS: [u32; KINDS] = [0xFFFF_FFA0, 0xFFFF_A0A0, 0xFFA0_A0FF, 0xFFA0_FFA0];
const KIND_TEXTS: [&str; KINDS] = ["1078", "1079", "1080", "1081"];
const RESISTANCE_KEYS: [&str; KINDS] = [
    "INVENTAR_RES_PHYSICAL",
    "INVENTAR_RES_FIRE",
    "INVENTAR_RES_MAGIC",
    "INVENTAR_RES_POISON",
];
/// The images of the kinds' shares in the damage's and the protection's lines, of the
/// resistances' line, and of the difficulties.
const SHARE_ICONS: u32 = 205;
const RESISTANCE_ICONS: u32 = 209;
const DIFFICULTY_ICONS: u32 = 414;
/// The categories (`items.pak +0x2E`).
const WEAPON: u8 = 5;
const RING: u8 = 8;
const POTION: u8 = 9;
const SHIELD: u8 = 13;
const BOOK: u8 = 16;
const AMULET: u8 = 20;
const NO_DAMAGE: u8 = 26;
const RUNE: u8 = 28;
const BRIDLE: u8 = 29;
const ARMOUR: [u8; 10] = [6, 17, 18, 19, 21, 22, 23, 24, 25, 33];
/// The skills of the lores' names (`9399 +` the skill) and of the armour's pace, and the
/// weapons' kinds held in both hands (`0x5CECD0`).
const SKILL_NAMES: u32 = 9399;
const ARMOUR_LORE: u8 = 11;
const TWO_HANDED: [u8; 5] = [2, 3, 4, 7, 12];
/// The classes' bits of a record's word `+0x04` and their indices (`0x43C250`), the index the
/// name's text, and each index's colour.
const CLASS_BITS: [(u32, u32); 8] = [
    (0, 1),
    (1, 2),
    (2, 3),
    (3, 4),
    (4, 5),
    (5, 6),
    (22, 8),
    (23, 9),
];
const CLASS_COLOURS: [u32; 10] = [
    0xFFEF_EFEF,
    0xFF87_CEFA,
    0xFFE3_8217,
    0xFF6C_4CE5,
    0xFFA0_20F0,
    0xFF22_8B22,
    0xFFB7_5555,
    0xFFB7_5555,
    0xFF00_FFFF,
    0xFFFF_6C00,
];
/// The plain colour of a name and of every class.
const PLAIN: u32 = 0xFFEF_EFEF;

/// What a tooltip tells of an item: its type, its name, its category, its type's record, its
/// price (its record's, a made item's worth) and whether the hero would buy it (a merchant's), its
/// quality, its values, its level and the level it asks, the share of its health that it heals,
/// of a healing potion, its sockets filled and its sockets, and its set's name where it is a set's
/// piece.
pub(crate) struct Item<'a> {
    pub item: u32,
    pub name: &'a str,
    pub category: u8,
    pub record: Record,
    pub price: i64,
    pub buying: bool,
    pub quality: u8,
    pub values: ItemValues,
    pub level: u32,
    pub asked: u32,
    pub heals: Option<u32>,
    pub sockets: (usize, usize),
    pub set: Option<String>,
}

/// The name's colour of a set's piece, and its set's name's (`0x5C13C0`, `0x5C5360`).
const SET_PIECE: u32 = 0xFF15_FF25;
const SET_NAME: u32 = 0xFFFF_D070;

/// The hero that the tooltip weighs the item for: its creature type, its level, its attributes
/// with its items' bonuses (the stats' words `+0x10`…), its skills and their levels, its
/// damage's multipliers (the stats' `+0x96` and `+0xA6`) and its resistances' factors (the
/// stats' `+0xD6`).
pub(crate) struct Hero<'a> {
    pub creature: u32,
    pub level: u32,
    pub attributes: [f32; 6],
    pub skills: &'a [(u8, u32)],
    pub multipliers: [f32; KINDS],
    pub resistance_factors: [f32; KINDS],
}

impl Hero<'_> {
    /// The level of its skill `skill`, 0 without it.
    fn skill(&self, skill: u8) -> u32 {
        self.skills
            .iter()
            .find(|&&(id, _)| id == skill)
            .map_or(0, |&(_, level)| level)
    }
}

/// The game's texts by name, and the combat arts' names by their ids.
pub(crate) struct Words<'a> {
    pub text: &'a dyn Fn(&str) -> Option<String>,
    pub art: &'a dyn Fn(u32) -> Option<String>,
}

impl Words<'_> {
    /// The text named `name`, empty without it.
    fn of(&self, name: &str) -> String {
        (self.text)(name).unwrap_or_default()
    }
}

/// The tooltip's lines of `item` for `hero` (none: the name alone, as the original without a
/// hero).
pub(crate) fn lines(item: &Item, hero: Option<&Hero>, words: &Words) -> Vec<Line> {
    let mut lines = name_lines(item, words);
    let Some(hero) = hero else {
        return lines;
    };
    let word = item.record.word;
    let values = &item.values;
    if let Some(set) = &item.set {
        lines.push(Line::of(set.clone(), SET_NAME));
    }
    if refused(word, hero.creature) {
        lines.push(Line::of(words.of("UI_ITEM_CANTUSE"), RED));
    }
    // The price, as the hero would sell it, or buy it from a merchant (`0x59FE80`).
    if item.price > 0 {
        let trading = hero.skill(opensacred_rules::trade::TRADING);
        let price = opensacred_rules::trade::shown(
            item.price,
            hero.level,
            trading,
            f64::from(hero.attributes[5]),
            item.buying,
            1.0,
        );
        let text = if trading > 0 {
            format!("{}: {price} ({})", words.of("1055"), item.price)
        } else {
            format!("{}: {price}", words.of("1055"))
        };
        lines.push(Line::of(text, WHITE));
    }
    if item.category != NO_DAMAGE {
        let most = values.damage.map(|[_, most]| most);
        if most.iter().any(|&most| most != 0.0) {
            let (least, most) = (0..KINDS).fold((0.0_f32, 0.0_f32), |(l, m), k| {
                let factor = hero.multipliers[k];
                let [least, most] = values.damage[k];
                (l + (least * factor).trunc(), m + (most * factor).trunc())
            });
            let mut pieces = shares(&values.damage.map(|[_, most]| most));
            pieces.push(Piece::Text(
                format!("{}: {least}-{most}", words.of("1176")),
                GREEN,
            ));
            lines.push(Line(pieces));
        }
        // Each kind's resistance, a word, times the hero's factor of it, truncated as it is
        // summed (`0x5C5DA7`).
        if values.resistance.iter().any(|&r| r != 0.0) {
            let sum: f32 = values
                .resistance
                .iter()
                .zip(hero.resistance_factors)
                .map(|(r, factor)| (r.trunc() * factor).trunc())
                .sum();
            let mut pieces = shares(&values.resistance);
            pieces.push(Piece::Text(format!("{}: {sum}", words.of("1177")), GREEN));
            lines.push(Line(pieces));
        }
    }
    lines.extend(slowing(item, hero, words));
    if item.level != 0 {
        lines.push(Line::of(
            format!("{}: {}", words.of("1070"), item.level),
            WHITE,
        ));
    }
    lines.extend(details(item, hero, words));
    lines.extend(hint(item, words));
    lines
}

/// The lines of an item in a socket of the item whose tooltip shows it, after that item's (the
/// flags `0x30`): its refusal of the hero's class, then its details, of what its socket's
/// merging left of it (`0x5C4220`), its bonuses alone.
pub(crate) fn socket_lines(item: &Item, hero: &Hero, words: &Words) -> Vec<Line> {
    let mut lines = Vec::new();
    if refused(item.record.word, hero.creature) {
        lines.push(Line::of(words.of("UI_ITEM_CANTUSE"), RED));
    }
    lines.extend(details(item, hero, words));
    lines
}

/// An item's details (the flag `0x10`): its `A` and `D`, each kind of damage with its bonuses,
/// its requirements, its resistances with their bonuses, then its other bonuses in their slots'
/// order.
fn details(item: &Item, hero: &Hero, words: &Words) -> Vec<Line> {
    let values = &item.values;
    let mut lines = Vec::new();
    for (value, text) in [(values.attack, "1074"), (values.defence, "1075")] {
        if value != 0.0 {
            lines.push(Line::of(format!("{}: {value}", words.of(text)), WHITE));
        }
    }
    // The bonuses listed, each taken where a line before claims it.
    let mut listed: Vec<Bonus> = values
        .bonuses
        .iter()
        .copied()
        .filter(|bonus| bonus.effect & 0xFFFF != 0)
        .collect();
    let mut claim = |test: &dyn Fn(&Bonus) -> bool, lines: &mut Vec<Line>| {
        let (taken, kept): (Vec<Bonus>, Vec<Bonus>) =
            listed.iter().copied().partition(|bonus| test(bonus));
        listed = kept;
        lines.extend(taken.iter().filter_map(|bonus| bonus_line(bonus, words)));
    };
    for (k, &[least, most]) in values.damage.iter().enumerate() {
        if most == 0.0 {
            continue;
        }
        let text = if most > least {
            format!("{}: {least}-{most}", words.of(KIND_TEXTS[k]))
        } else {
            format!("{}: {most}", words.of(KIND_TEXTS[k]))
        };
        lines.push(Line::of(text, KIND_COLOURS[k]));
        let effects = kind_effects(k);
        claim(
            &|bonus: &Bonus| {
                effects.contains(&(bonus.effect & 0xFFFF))
                    && (k == 0 || bonus.condition == 0 || bonus.condition & 0x4000 != 0)
            },
            &mut lines,
        );
    }
    lines.extend(requirements(item, hero, words));
    if values.resistance.iter().any(|&r| r != 0.0) {
        let mut pieces = vec![
            Piece::Text(format!("{}:", words.of("1195")), PROTECTION),
            Piece::Text(" ".into(), WHITE),
        ];
        let mut sum = 0.0;
        for ((k, &resistance), icon) in values.resistance.iter().enumerate().zip(RESISTANCE_ICONS..)
        {
            if resistance != 0.0 {
                pieces.push(Piece::Image(icon, WHITE));
                pieces.push(Piece::Text(format!("{resistance}"), KIND_COLOURS[k]));
                sum += resistance;
            }
        }
        pieces.push(Piece::Text(format!(" [{sum}]"), WHITE));
        lines.push(Line(pieces));
        for (k, &resistance) in values.resistance.iter().enumerate() {
            if resistance == 0.0 {
                continue;
            }
            let effects = kind_effects(k);
            claim(
                &|bonus: &Bonus| {
                    bonus.condition & 0x8000 != 0 && effects.contains(&(bonus.effect & 0xFFFF))
                },
                &mut lines,
            );
        }
    }
    claim(&|_| true, &mut lines);
    lines
}

/// The name's lines by the item's class (its virtual `+0x44`): a weapon's, an armour's and a
/// shield's (`0x5CED90`, `0x5BA5F0`, `0x5CC3C0`) its difficulty's icon and its name in its
/// colour, a ring's (`0x5CBC20`) the same with no space, an amulet's and a rune's
/// (**hypothesis**: `0x5CC780`) its name in its colour, each then its class where one alone may
/// use it, a weapon's then its lore and its hands; any other's its name plain (`0x5CBA30`).
fn name_lines(item: &Item, words: &Words) -> Vec<Line> {
    let word = item.record.word;
    let colour = name_colour(item);
    let icon = Piece::Image(DIFFICULTY_ICONS + difficulty_step(word), WHITE);
    let armoury =
        item.category == WEAPON || item.category == SHIELD || ARMOUR.contains(&item.category);
    // The sockets filled over the sockets after the name, where it has some (`0x5BE990`,
    // `0x5BE9C0`).
    let name = match item.sockets {
        (_, 0) => item.name.to_owned(),
        (filled, sockets) => format!("{} ({filled}/{sockets})", item.name),
    };
    let mut lines = match item.category {
        _ if armoury => vec![Line(vec![icon, Piece::Text(format!(" {name}"), colour)])],
        RING => vec![Line(vec![icon, Piece::Text(name, colour)])],
        AMULET | RUNE => vec![Line::of(name, colour)],
        _ => return vec![Line::of(item.name, WHITE)],
    };
    if let Some(index) = single_class(word) {
        lines.push(Line::of(
            words.of(&index.to_string()),
            CLASS_COLOURS[usize::try_from(index).unwrap_or(0)],
        ));
    }
    if item.category == WEAPON {
        let family = item.values.family;
        let skill =
            opensacred_rules::fight::weapon_skill(1, Some(family), &[(family, item.record.kind)]);
        let lore = words.of(&(SKILL_NAMES + u32::from(skill)).to_string());
        if !lore.is_empty() {
            lines.push(Line::of(lore, WHITE));
        }
        if TWO_HANDED.contains(&item.record.kind) {
            lines.push(Line::of(words.of("EWX_TWOHANDED"), WHITE));
        }
    }
    lines
}

/// The name's colour (`0x5C13C0`): a set's piece's green; else a rune's its class's, any other's
/// its quality's (the setting `UNIQUE_COLOR` taken as on): 0 to 4 plain, 5 to 11 blue, 12 to 14
/// gold and 15 bronze.
fn name_colour(item: &Item) -> u32 {
    if item.set.is_some() {
        return SET_PIECE;
    }
    if item.category == RUNE {
        return class_colour(item.record.word);
    }
    match item.quality {
        5..=11 => 0xFFA0_A0FF,
        12..=14 => 0xFFF0_E040,
        15 => 0xFFC1_9A33,
        _ => PLAIN,
    }
}

/// The colour of a record's classes (`0x5C12F0`): plain for every class, else the last class's.
fn class_colour(word: u32) -> u32 {
    if word & CLASSES == CLASSES {
        return PLAIN;
    }
    CLASS_BITS
        .iter()
        .rev()
        .find(|&&(bit, _)| word & (1 << bit) != 0)
        .map_or(PLAIN, |&(_, index)| {
            CLASS_COLOURS[usize::try_from(index).unwrap_or(0)]
        })
}

/// The index of a record's one class, none for a record of several or none (`0x5C14C0`).
fn single_class(word: u32) -> Option<u32> {
    let mut classes = CLASS_BITS
        .iter()
        .filter(|&&(bit, _)| word & (1 << bit) != 0);
    let (_, index) = *classes.next()?;
    classes.next().is_none().then_some(index)
}

/// The step of the difficulty's icon from the record's bits 24 to 27 (`0x5C8C20`): 2 to 5 the
/// icons 415 to 418, any other 414.
fn difficulty_step(word: u32) -> u32 {
    match (word >> 24) & 0xF {
        step @ 2..=5 => step - 1,
        _ => 0,
    }
}

/// Whether the hero of the creature type `creature` is told that its class may not use an item
/// of the record's word `word`: one of no class or of the bit 6, or of several classes without
/// the hero's.
fn refused(word: u32, creature: u32) -> bool {
    let classes = word & CLASSES;
    if word & NO_CLASS != 0 || classes == 0 {
        return true;
    }
    single_class(word).is_none()
        && classes != CLASSES
        && classes & opensacred_rules::equipment::class_bit(creature) == 0
}

/// The icons of the kinds' shares of `values` (each its value over the greatest, `⌊255·v/m⌋`,
/// above 5) at that alpha.
fn shares(values: &[f32; KINDS]) -> Vec<Piece> {
    let most = values.iter().copied().fold(0.0_f32, f32::max);
    if most <= 0.0 {
        return Vec::new();
    }
    values
        .iter()
        .zip(SHARE_ICONS..)
        .filter_map(|(&value, icon)| {
            let share = (255.0 * value / most).trunc();
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a share of 0 to 255"
            )]
            let alpha = share.clamp(0.0, 255.0) as u32;
            (alpha > 5).then_some(Piece::Image(icon, (alpha << 24) | 0x00FF_FFFF))
        })
        .collect()
}

/// The slowing of a weapon's hands or an armour's pace that a level above the hero's and its
/// lore makes (`0x5C8B00`, `0x579140`, `0x579470`), each `− x %` of 100 where it is not about 0
/// or 100 per cent.
fn slowing(item: &Item, hero: &Hero, words: &Words) -> Vec<Line> {
    let (v, r) = (f64::from(hero.level), f64::from(item.level));
    let shortfall = |skill: u8| {
        let s = f64::from(hero.skill(skill));
        (r != 0.0 && r > v + s).then(|| ((s + v + 9.0) / (r + 9.0)).powi(2))
    };
    let factor = if item.category == WEAPON {
        let family = item.values.family;
        let skill =
            opensacred_rules::fight::weapon_skill(1, Some(family), &[(family, item.record.kind)]);
        Some(("1169", shortfall(skill).unwrap_or(1.0)))
    } else if item.category == SHIELD || ARMOUR.contains(&item.category) {
        let scale = f64::from(item.values.scale);
        let x = shortfall(ARMOUR_LORE)
            .filter(|_| (1.0..=999.0).contains(&scale))
            .map_or(1.0, |y| 1.0 - (1.0 - y) * scale * 0.01);
        Some(("1168", x))
    } else {
        None
    };
    factor
        .filter(|&(_, f)| f.abs() > 0.01 && (f - 1.0).abs() > 0.01)
        .map(|(text, f)| {
            Line::of(
                format!("{}: - {:5.2}%", words.of(text), 100.0 - 100.0 * f),
                WHITE,
            )
        })
        .into_iter()
        .collect()
}

/// The requirements that are not 0, white where the hero meets them and red where it does not:
/// the level it asks, the strength, the dexterity, the charisma and the endurance, then the
/// skill at its level (`0x5C5360`).
fn requirements(item: &Item, hero: &Hero, words: &Words) -> Vec<Line> {
    let [strength, dexterity, charisma, endurance, skill, skill_level] = item.record.requirements;
    let a = hero.attributes;
    let mut lines: Vec<Line> = [
        (item.asked, f64::from(hero.level), "INVENTAR_MIN_LEVEL"),
        (u32::from(strength), f64::from(a[0]), "INVENTAR_MIN_FORCE"),
        (u32::from(dexterity), f64::from(a[2]), "INVENTAR_MIN_DEX"),
        (
            u32::from(charisma),
            f64::from(a[5]),
            "INVENTAR_MIN_CHARISMA",
        ),
        (u32::from(endurance), f64::from(a[1]), "INVENTAR_MIN_END"),
    ]
    .into_iter()
    .filter(|&(asked, ..)| asked != 0)
    .map(|(asked, has, key)| {
        let colour = if has < f64::from(asked) { RED } else { WHITE };
        Line::of(format!("{}: {asked}", words.of(key)), colour)
    })
    .collect();
    if skill != 0 && skill_level != 0 {
        let colour = if hero.skill(skill) < u32::from(skill_level) {
            RED
        } else {
            WHITE
        };
        lines.push(Line::of(
            format!(
                "{} {}: {skill_level}",
                words.of("1108"),
                words.of(&(SKILL_NAMES + u32::from(skill)).to_string())
            ),
            colour,
        ));
    }
    lines
}

/// The bonuses' effects of the kind `k`: its damage's and its resistance's (801 + k, 805 + k).
fn kind_effects(k: usize) -> [u32; 2] {
    let k = u32::try_from(k).unwrap_or(0);
    [801 + k, 805 + k]
}

/// The attributes' texts by a bonus's byte 2 (`a0` 1090, `a2` 1092, `a1` 1091, `a3` 1093, `a4`
/// 1094, `a5` 1095).
fn attribute_text(a: u32) -> Option<&'static str> {
    match a {
        1 => Some("1090"),
        2 => Some("1092"),
        3 => Some("1091"),
        4 => Some("1093"),
        5 => Some("1094"),
        6 => Some("1095"),
        _ => None,
    }
}

/// The bonuses' texts of the form `name: +v` and of the form `name: +v%` by their effects.
const PLUS: [(u32, &str); 20] = [
    (811, "1100"),
    (812, "1098"),
    (815, "1090"),
    (816, "1092"),
    (817, "1091"),
    (818, "1093"),
    (819, "1094"),
    (820, "1095"),
    (840, "1124"),
    (844, "1138"),
    (845, "1137"),
    (849, "1136"),
    (853, "1130"),
    (858, "UI_RARE_MAGIEABWEHR"),
    (859, "1132"),
    (861, "UI_RARE_STONEWALL"),
    (862, "UI_RARE_SCHOCK"),
    (863, "UI_RARE_EXORZIST"),
    (865, "1133"),
    (868, "UI_RARE_ALCHEMIE"),
];
const PER_CENT: [(u32, &str); 17] = [
    (813, "1148"),
    (814, "1149"),
    (841, "1125"),
    (842, "1127"),
    (843, "1128"),
    (846, "1238"),
    (847, "1139"),
    (848, "1140"),
    (850, "1141"),
    (851, "1142"),
    (852, "1129"),
    (855, "1150"),
    (856, "1143"),
    (857, "1151"),
    (860, "UI_RARE_SCHADEN"),
    (866, "1134"),
    (867, "1135"),
];

/// The texts of the creatures' classes 1 to 12 that a bonus holds against (`0x8EB79C`, the
/// records `{index, name, text}` of `CL_HERO` to `CL_ELF`).
const OPPONENTS: [&str; 12] = [
    "1184", "1187", "1178", "1196", "1180", "1179", "1185", "1186", "1182", "1181", "1188", "1189",
];

/// A bonus's line (`0x5C4450`): the class it holds for, in the class's colour, then in the
/// effect's colour (the kinds' for 801 to 808, gold for 840 to 868 but 854, else blue) the
/// opponent it holds against and its text by the effect; none for an effect of no text.
fn bonus_line(bonus: &Bonus, words: &Words) -> Option<Line> {
    let (c, e, a) = (
        u32::from(bonus.condition),
        bonus.effect & 0xFFFF,
        (bonus.effect >> 16) & 0xFF,
    );
    let v = i32::from(bonus.value);
    let attribute = attribute_text(a).map(|text| words.of(text));
    let kind = |offset: u32| usize::try_from(e - offset).unwrap_or(0).min(KINDS - 1);
    let body = match e {
        0..200 | 1000.. => format!("{}: {v:+}", (words.art)(e)?),
        600..=798 => format!("{}: {v:+}", words.of(&(e + 8800).to_string())),
        801..=804 if c & 0x8000 == 0 => {
            let (sign, name) = attribute.as_deref().map_or(("", ""), |name| ("%", name));
            format!(
                "{} {}: {v:+}{sign} {name}",
                words.of("1154"),
                words.of(KIND_TEXTS[kind(801)])
            )
        }
        801..=804 => format!(
            "{}: {v:+}% {}",
            words.of(RESISTANCE_KEYS[kind(801)]),
            attribute.unwrap_or_default()
        ),
        805..=808 if c & 0x8000 != 0 => {
            format!("{}: {v:+}%", words.of(RESISTANCE_KEYS[kind(805)]))
        }
        805..=808 => {
            let of = if c & 0x4000 != 0 {
                "1087"
            } else if c & 0x2000 != 0 {
                "1088"
            } else {
                "1076"
            };
            format!(
                "{} {}: {v:+}%",
                words.of(KIND_TEXTS[kind(805)]),
                words.of(of)
            )
        }
        809 | 810 => {
            let (space, name) = attribute.as_deref().map_or(("", ""), |name| (" ", name));
            format!(
                "+{v}% {space}{name} {} {}",
                words.of("1110"),
                words.of(if e == 809 { "1074" } else { "1075" })
            )
        }
        864 => format!("{} ", words.of("1144")),
        _ => {
            if let Some((_, text)) = PLUS.iter().find(|&&(effect, _)| effect == e) {
                format!("{}: {v:+}", words.of(text))
            } else if let Some((_, text)) = PER_CENT.iter().find(|&&(effect, _)| effect == e) {
                format!("{}: {v:+}%", words.of(text))
            } else {
                return None;
            }
        }
    };
    let colour = match e {
        801..=808 => KIND_COLOURS[usize::try_from((e - 801) % 4).unwrap_or(0)],
        840..=868 if e != 854 => GOLD,
        _ => BLUE,
    };
    let mut pieces = Vec::new();
    match (c >> 8) & 0xF {
        0 => {}
        15 => pieces.push(Piece::Text(format!("{} ", words.of("1183")), WHITE)),
        class => pieces.push(Piece::Text(
            format!("{} ", words.of(&class.to_string())),
            CLASS_COLOURS[usize::try_from(class).unwrap_or(0).min(9)],
        )),
    }
    // The opponent by the condition's low byte: a class of creatures of the table at `0x8EB79C`
    // (`0x43D0E0`), but 0, `CL_ALLENEMY` and the classes of no text; else the ranged ones and
    // the riders.
    let opponent = match c & 0xFF {
        0x80 => Some("RES_GEG_FERNK"),
        0x81 => Some("RES_GEG_REITER"),
        low => usize::try_from(low)
            .ok()
            .and_then(|low| low.checked_sub(1))
            .and_then(|k| OPPONENTS.get(k))
            .copied(),
    };
    if let Some(key) = opponent {
        pieces.push(Piece::Text(
            format!("{} {} ", words.of("1161"), words.of(key)),
            colour,
        ));
    }
    pieces.push(Piece::Text(body, colour));
    Some(Line(pieces))
}

/// The hint of the item's use, grey (the flag 8): a potion's effect and the right button's line
/// (1012), a rune's (1010) but of the bit 6, a book's (1011), a bridle's.
fn hint(item: &Item, words: &Words) -> Vec<Line> {
    let grey = |text: String| Line::of(text, GREY);
    match item.category {
        POTION => {
            let effect = match (item.heals, potion_key(item.item)) {
                (Some(share), _) => {
                    Some(format!("{}: {share}%", words.of("POTION_GENERIC_RED_USE")))
                }
                (None, Some(key)) => Some(words.of(&key)),
                (None, None) => None,
            };
            effect
                .into_iter()
                .chain([words.of("1012")])
                .map(grey)
                .collect()
        }
        RUNE if item.record.word & NO_CLASS == 0 => vec![grey(words.of("1010"))],
        BOOK => vec![grey(words.of("1011"))],
        BRIDLE => vec![grey(words.of("UI_HORSE_BRIDDLE_USE"))],
        _ => Vec::new(),
    }
}

/// The text's name of the effect of a potion of the type `item` that does not heal
/// (`POTION_<size>_<colour>_USE`, `0x43F860`), none for one of none.
fn potion_key(item: u32) -> Option<String> {
    let (size, colour) = match item {
        5137 => ("L", "BLACK"),
        5142 => ("M", "BLACK"),
        5168 => ("S", "BLACK"),
        5138 => ("L", "BLUE"),
        5143 => ("M", "BLUE"),
        5169 => ("S", "BLUE"),
        5139 => ("L", "GREEN"),
        5144 => ("M", "GREEN"),
        5170 => ("S", "GREEN"),
        5141 => ("L", "YELLOW"),
        5146 => ("M", "YELLOW"),
        5172 => ("S", "YELLOW"),
        _ => return None,
    };
    Some(format!("POTION_{size}_{colour}_USE"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The texts by their names, as themselves in brackets.
    fn words() -> Words<'static> {
        Words {
            text: &|name| Some(format!("<{name}>")),
            art: &|id| Some(format!("<art {id}>")),
        }
    }

    fn texts(line: &Line) -> String {
        line.0
            .iter()
            .map(|piece| match piece {
                Piece::Text(text, _) => text.clone(),
                Piece::Image(index, _) => format!("[{index}]"),
            })
            .collect()
    }

    #[test]
    fn a_bonus_names_the_class_of_its_opponents() {
        let line = |condition: u16| {
            let bonus = Bonus {
                condition,
                effect: 805,
                value: 80,
            };
            bonus_line(&bonus, &words()).map(|line| texts(&line))
        };
        // The sword 1712's against the monsters (seen 2026-11-01, #1708).
        assert_eq!(
            line(2).as_deref(),
            Some("<1161> <1187> <1078> <1076>: +80%")
        );
        assert_eq!(
            line(12).as_deref(),
            Some("<1161> <1189> <1078> <1076>: +80%")
        );
        // No class, `CL_ALLENEMY` and the classes of no text name none.
        for condition in [0, 13, 14, 15] {
            assert_eq!(line(condition).as_deref(), Some("<1078> <1076>: +80%"));
        }
        assert_eq!(
            line(0x80).as_deref(),
            Some("<1161> <RES_GEG_FERNK> <1078> <1076>: +80%")
        );
    }

    fn hero(skills: &[(u8, u32)]) -> Hero<'_> {
        Hero {
            creature: 1,
            level: 1,
            attributes: [10.0, 10.0, 10.0, 10.0, 10.0, 15.0],
            skills,
            multipliers: [1.0; KINDS],
            resistance_factors: [1.0; KINDS],
        }
    }

    #[test]
    fn the_expected_protection_takes_the_heros_resistance_factors() {
        let item = Item {
            item: 4000,
            name: "Helmet",
            category: 17,
            record: Record::default(),
            price: 0,
            buying: false,
            quality: 0,
            values: ItemValues {
                resistance: [27.0, 0.0, 18.0, 0.0],
                ..ItemValues::default()
            },
            level: 20,
            asked: 14,
            heals: None,
            sockets: (0, 0),
            set: None,
        };
        // The Seraphim's factors at her start, as the original's `stats` shows them: 32 and 21
        // where the original's tooltip shows 53 (seen 2026-11-01, #1706).
        let hero = Hero {
            resistance_factors: [1.202_701_4, 1.202_701_4, 1.177_363_8, 1.218_293_9],
            ..hero(&[])
        };
        let shown: Vec<String> = lines(&item, Some(&hero), &words())
            .iter()
            .map(texts)
            .collect();
        assert!(
            shown.contains(&"[205][207]<1177>: 53".to_owned()),
            "{shown:?}"
        );
    }

    #[test]
    fn a_potion_shows_its_name_price_effect_and_use() {
        let item = Item {
            item: 5171,
            name: "Potion",
            category: POTION,
            record: Record {
                word: CLASSES,
                price: 400,
                ..Record::default()
            },
            price: 400,
            buying: false,
            quality: 0,
            values: ItemValues::default(),
            level: 0,
            asked: 0,
            heals: Some(100),
            sockets: (0, 0),
            set: None,
        };
        let lines: Vec<String> = lines(&item, Some(&hero(&[])), &words())
            .iter()
            .map(texts)
            .collect();
        assert_eq!(
            lines,
            [
                "Potion",
                "<1055>: 125",
                "<POTION_GENERIC_RED_USE>: 100%",
                "<1012>"
            ]
        );
    }

    #[test]
    fn gloves_of_one_class_show_their_class_protection_slowing_and_bonus() {
        let mut values = ItemValues {
            resistance: [3.0, 2.0, 0.0, 0.0],
            scale: 15,
            ..ItemValues::default()
        };
        values.bonuses[0] = Bonus {
            condition: 0,
            effect: 816,
            value: 3,
        };
        let item = Item {
            item: 1221,
            name: "Gauntlets",
            category: 24,
            record: Record {
                word: 1 << 23,
                ..Record::default()
            },
            price: 0,
            buying: false,
            quality: 0,
            values,
            level: 3,
            asked: 1,
            heals: None,
            sockets: (0, 0),
            set: None,
        };
        let lines = lines(&item, Some(&hero(&[])), &words());
        let shown: Vec<String> = lines.iter().map(texts).collect();
        assert_eq!(
            shown,
            [
                "[414] Gauntlets",
                "<9>",
                "[205][206]<1177>: 5",
                "<1168>: -  4.58%",
                "<1070>: 3",
                "<INVENTAR_MIN_LEVEL>: 1",
                "<1195>: [209]3[210]2 [5]",
                "<1092>: +3",
            ]
        );
        // The Daemon's orange, the bonus's blue.
        assert_eq!(lines[1].0[0], Piece::Text("<9>".into(), 0xFFFF_6C00));
        assert!(matches!(lines[7].0[0], Piece::Text(_, BLUE)));
    }

    #[test]
    fn a_weapons_kind_lines_take_their_bonuses_and_the_level_turns_red() {
        let mut values = ItemValues {
            damage: [[21.0, 23.0], [0.0; 2], [0.0; 2], [0.0; 2]],
            attack: 5.0,
            defence: 2.0,
            family: 1,
            ..ItemValues::default()
        };
        values.bonuses[0] = Bonus {
            condition: 0,
            effect: 811,
            value: 14,
        };
        values.bonuses[1] = Bonus {
            condition: 0,
            effect: 801,
            value: 4,
        };
        let item = Item {
            item: 7901,
            name: "Blade",
            category: WEAPON,
            record: Record {
                word: CLASSES,
                kind: 1,
                price: 1120,
                ..Record::default()
            },
            price: 1120,
            buying: false,
            quality: 0,
            values,
            level: 1,
            asked: 2,
            heals: None,
            sockets: (0, 0),
            set: None,
        };
        let shown: Vec<String> = lines(&item, Some(&hero(&[])), &words())
            .iter()
            .map(texts)
            .collect();
        assert_eq!(
            shown,
            [
                "[414] Blade",
                "<9403>",
                "<1055>: 200",
                "[205]<1176>: 21-23",
                "<1070>: 1",
                "<1074>: 5",
                "<1075>: 2",
                "<1078>: 21-23",
                "<1154> <1078>: +4 ",
                "<INVENTAR_MIN_LEVEL>: 2",
                "<1100>: +14",
            ]
        );
    }

    #[test]
    fn a_socketed_items_name_counts_its_sockets_and_a_sockets_item_shows_its_bonuses() {
        let item = |name, category, word, sockets| Item {
            item: 1712,
            name,
            category,
            record: Record {
                word,
                ..Record::default()
            },
            price: 0,
            buying: false,
            quality: 0,
            values: ItemValues::default(),
            level: 0,
            asked: 0,
            heals: None,
            sockets,
            set: None,
        };
        let name = |item: &Item| texts(&name_lines(item, &words())[0]);
        assert_eq!(
            name(&item("Dagger", WEAPON, CLASSES, (1, 2))),
            "[414] Dagger (1/2)"
        );
        assert_eq!(
            name(&item("Ring", RING, CLASSES, (0, 1))),
            "[414]Ring (0/1)"
        );
        assert_eq!(
            name(&item("Dagger", WEAPON, CLASSES, (0, 0))),
            "[414] Dagger"
        );
        // A socket's item of no class: its refusal, then what is left of its bonuses.
        let mut socket = item("Upgrade", 26, 0, (0, 0));
        socket.values.bonuses[2] = Bonus {
            condition: 0,
            effect: 811,
            value: 7,
        };
        let shown: Vec<String> = socket_lines(&socket, &hero(&[]), &words())
            .iter()
            .map(texts)
            .collect();
        assert_eq!(shown, ["<UI_ITEM_CANTUSE>", "<1100>: +7"]);
    }

    #[test]
    fn a_sets_piece_shows_its_name_in_green_and_its_sets_name_after_it() {
        let item = Item {
            item: 1221,
            name: "Gauntlets",
            category: 24,
            record: Record {
                word: CLASSES,
                ..Record::default()
            },
            price: 0,
            buying: false,
            quality: 15,
            values: ItemValues::default(),
            level: 0,
            asked: 0,
            heals: None,
            sockets: (0, 0),
            set: Some("The Set".into()),
        };
        let lines = lines(&item, Some(&hero(&[])), &words());
        // The set's green before the quality's bronze, the set's name after the name.
        assert_eq!(lines[0].0[1], Piece::Text(" Gauntlets".into(), SET_PIECE));
        assert_eq!(lines[1], Line::of("The Set", SET_NAME));
    }

    #[test]
    fn a_rune_takes_its_class_colour_and_a_multi_class_item_refuses_others() {
        assert_eq!(class_colour(1 << 2), 0xFF6C_4CE5);
        assert_eq!(single_class(1 << 2), Some(3));
        assert_eq!(single_class(0b11), None);
        assert!(!refused(1 << 23, 1));
        // One class alone is named rather than refused, as the Daemon's gloves to the Seraphim.
        assert!(!refused(0b10, 9) && refused(0b110, 1) && !refused(0b11, 1));
        assert!(refused(NO_CLASS | CLASSES, 1));
        assert_eq!(difficulty_step(3 << 24), 2);
    }
}
