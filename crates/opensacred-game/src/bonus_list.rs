//! The text of the hero's list of bonuses (#1698; docs/re/inventory.md, "The hero's list of
//! bonuses"), which the column's "+" shows (`0x55E5E0`): a line for each entry of the list
//! (`opensacred_rules::fight::listed_bonuses`), its class and its opponent before it as an item's
//! bonus's, in its effect's colour, of the format its effect takes, the value whole and signed.
//!
//! Not followed: the opponents' names but the ranged ones' and the riders' (`0x43D0E0`), and the
//! line of the effect 852 (`SCHRECKEN_NEU`, its share by the difficulty and `0x570ED0`).

use opensacred_data::tables::EffectChance;
use opensacred_rules::fight::Listed;

/// The colours: white, the kinds' (physical, fire, magic, poison), the rare effects' gold and the
/// others' blue.
const WHITE: u32 = 0xFFFF_FFFF;
const KIND_COLOURS: [u32; 4] = [0xFFFF_FFA0, 0xFFFF_A0A0, 0xFFA0_A0FF, 0xFFA0_FFA0];
const GOLD: u32 = 0xFFD8_AB30;
const BLUE: u32 = 0xFF44_9BEA;
/// The classes' colours by their index (`0x43C250`).
const CLASS_COLOURS: [u32; 10] = [
    0xFF00_0000,
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
const RESISTANCE_KEYS: [&str; 4] = [
    "INVENTAR_RES_PHYSICAL",
    "INVENTAR_RES_FIRE",
    "INVENTAR_RES_MAGIC",
    "INVENTAR_RES_POISON",
];
/// A rare effect's chance's parameters where the balance's table lacks it (`0x5F23A0`).
const CHANCE: (f32, f32, f32) = (100.0, 100.0, 90.0);

/// The game's texts by name, and the combat arts' names by their ids.
pub(crate) struct Words<'a> {
    pub text: &'a dyn Fn(&str) -> Option<String>,
    pub art: &'a dyn Fn(u32) -> Option<String>,
}

impl Words<'_> {
    fn of(&self, name: &str) -> String {
        (self.text)(name).unwrap_or_default()
    }

    fn n(&self, number: u32) -> String {
        self.of(&number.to_string())
    }
}

/// `x` truncated toward zero, as the runtime's `_ftol`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a bonus's value, well within an i64, truncated as the original's _ftol"
)]
fn ftol(x: f64) -> i64 {
    x.trunc() as i64
}

/// The list's text of the entries `list`, the rare effects' chances of `chances`: a line each,
/// coloured by the tags of the game's texts.
pub(crate) fn text(list: &[Listed], words: &Words, chances: &[EffectChance]) -> String {
    list.iter()
        .filter_map(|listed| line(listed, words, chances))
        .collect::<Vec<_>>()
        .join("<n>")
}

/// The line of the entry `listed`, none of an effect of no text.
fn line(listed: &Listed, words: &Words, chances: &[EffectChance]) -> Option<String> {
    let c = u32::from(listed.condition);
    let mut e = listed.effect & 0xFFFF;
    if e == 54 {
        e = 1089;
    }
    let a = (listed.effect >> 16) & 0xFF;
    let v = ftol(f64::from(listed.value));
    let attribute = match a {
        1 => Some("1090"),
        2 => Some("1092"),
        3 => Some("1091"),
        4 => Some("1093"),
        5 => Some("1094"),
        6 => Some("1095"),
        _ => None,
    }
    .map(|text| words.of(text));
    let kind = |from: u32| usize::try_from(e - from).unwrap_or(0).min(3);
    let named = |number: u32| format!("{}: {v:+}", words.n(number));
    let per_cent = |number: u32| format!("{}: {v:+}%", words.n(number));
    let share = |name: String| {
        let (a, b, most) = chances
            .iter()
            .find(|chance| chance.effect == e)
            .map_or(CHANCE, |chance| (chance.a, chance.b, chance.most));
        let value = f64::from(stored(v));
        let share = if value <= f64::from(f32::EPSILON) {
            0.0
        } else {
            (1.0 - f64::from(a) / (f64::from(b) + value)) * 100.0 + 0.5
        };
        let share = ftol(share.min(f64::from(most)).max(0.0));
        format!("{name}: {v:+} ({share:+}%)")
    };
    let body = match e {
        0..200 | 1000.. => format!("{}: {v:+}", (words.art)(e)?),
        600..=798 => named(e + 8800),
        801..=804 => {
            let (sign, name) = attribute.as_deref().map_or(("", ""), |name| ("%", name));
            if c & 0x8000 == 0 {
                format!(
                    "{} {}: {v:+}{sign} {name}",
                    words.n(1154),
                    words.n(1078 + u32::try_from(kind(801)).unwrap_or(0))
                )
            } else {
                format!(
                    "{}: {v:+}{sign} {name}",
                    words.of(RESISTANCE_KEYS[kind(801)])
                )
            }
        }
        805..=808 if c & 0x8000 != 0 => format!("{}: {v:+}%", words.of(RESISTANCE_KEYS[kind(805)])),
        805..=808 => {
            let of = if c & 0x4000 != 0 {
                1087
            } else if c & 0x2000 != 0 {
                1088
            } else {
                1076
            };
            format!(
                "{} {}: {v:+}%",
                words.n(1078 + u32::try_from(kind(805)).unwrap_or(0)),
                words.n(of)
            )
        }
        809 | 810 => {
            let (space, name) = attribute.as_deref().map_or(("", ""), |name| (" ", name));
            format!(
                "+{v}% {space}{name} {} {}",
                words.n(1110),
                words.n(if e == 809 { 1074 } else { 1075 })
            )
        }
        811 => named(1100),
        812 => named(1098),
        813 => named(1148),
        814 => named(1149),
        815 => named(1090),
        816 => named(1092),
        817 => named(1091),
        818 => named(1093),
        819 => named(1094),
        820 => named(1095),
        840 => format!("{}: {v:+} ({:+}%)", words.n(1124), v * 100),
        841 => per_cent(1125),
        842 => per_cent(1127),
        843 => per_cent(1128),
        844 => named(1138),
        845 => named(1137),
        846 => per_cent(1238),
        847 => per_cent(1139),
        848 => per_cent(1140),
        849 => named(1136),
        850 => per_cent(1141),
        851 => per_cent(1142),
        853 => named(1130),
        855 => per_cent(1150),
        856 => per_cent(1143),
        857 => per_cent(1151),
        858 => share(words.of("UI_RARE_MAGIEABWEHR")),
        859 => share(words.n(1132)),
        860 => format!("{}: {v:+}%", words.of("UI_RARE_SCHADEN")),
        861 => share(words.of("UI_RARE_STONEWALL")),
        862 => share(words.of("UI_RARE_SCHOCK")),
        863 => share(words.of("UI_RARE_EXORZIST")),
        864 => format!("{} ", words.n(1144)),
        865 => per_cent(1133),
        866 => per_cent(1134),
        867 => per_cent(1135),
        868 => share(words.of("UI_RARE_ALCHEMIE")),
        _ => return None,
    };
    let colour = match e {
        801..=808 => KIND_COLOURS[usize::try_from((e - 801) % 4).unwrap_or(0)],
        840..=868 if e != 854 => GOLD,
        _ => BLUE,
    };
    let mut line = String::new();
    match (c >> 8) & 0xF {
        0 => {}
        15 => line.push_str(&format!("<colorl={WHITE:08X}>{} ", words.n(1183))),
        class => line.push_str(&format!(
            "<colorl={:08X}>{} ",
            CLASS_COLOURS[usize::try_from(class).unwrap_or(0).min(9)],
            words.n(class)
        )),
    }
    line.push_str(&format!("<colorl={colour:08X}>"));
    let opponent = match c & 0xFF {
        0x80 => Some("RES_GEG_FERNK"),
        0x81 => Some("RES_GEG_REITER"),
        _ => None,
    };
    if let Some(key) = opponent {
        line.push_str(&format!("{} {} ", words.n(1161), words.of(key)));
    }
    line.push_str(&body);
    Some(line)
}

/// A whole value as a float.
#[expect(
    clippy::cast_precision_loss,
    reason = "a bonus's whole value, far below 2^23"
)]
fn stored(v: i64) -> f32 {
    v as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words() -> Words<'static> {
        Words {
            text: &|name| Some(format!("[{name}]")),
            art: &|id| Some(format!("[art {id}]")),
        }
    }

    fn listed(condition: u16, effect: u32, value: f32) -> Listed {
        Listed {
            condition,
            effect,
            value,
        }
    }

    #[test]
    fn each_entry_is_a_line_in_its_effects_colour_and_format() {
        let chances = [EffectChance {
            effect: 858,
            a: 50.0,
            b: 50.0,
            most: 80.0,
        }];
        let list = [
            listed(0, 801 | 2 << 16, 35.0),
            listed(0x80, 805, 80.0),
            listed(0, 813, 25.0),
            listed(0x0300, 816, 3.0),
            listed(0, 858, 50.0),
            listed(0, 1071, 1.0),
            listed(0, 854, 5.0),
        ];
        let text = text(&list, &words(), &chances);
        let lines: Vec<&str> = text.split("<n>").collect();
        assert_eq!(
            lines,
            [
                "<colorl=FFFFFFA0>[1154] [1078]: +35% [1092]",
                "<colorl=FFFFFFA0>[1161] [RES_GEG_FERNK] [1078] [1076]: +80%",
                "<colorl=FF449BEA>[1148]: +25",
                "<colorl=FF6C4CE5>[3] <colorl=FF449BEA>[1092]: +3",
                // The share (1 − 50/(50 + 50))·100 + 0.5, 50.
                "<colorl=FFD8AB30>[UI_RARE_MAGIEABWEHR]: +50 (+50%)",
                "<colorl=FF449BEA>[art 1071]: +1",
            ]
        );
    }
}
