//! The list of the sets worn that the equipment window's set icon shows (#1670; docs/re/sets.md,
//! §2 and §8): for each set of a piece worn, by its number (`0x5711F0`), its name, each of its
//! pieces green where worn and red where not, and the lines of its bonuses reached
//! (`0x571710`'s text), told as the game applies them (the rule of #1737,
//! `opensacred_rules::set_bonuses::told`), each number of the values that the stats take.
//!
//! The original writes the lines of its switch's formats, some of which tell another value, kind
//! or threshold than the code applies, and some a bonus that nothing applies (docs/re/sets.md §6);
//! these lines are the game's texts of the bonuses as the items' tooltips word them, of the
//! values applied. No line tells a flag whose readers the game does not follow yet, nor a bonus
//! that another reader of the counts applies (docs/re/sets.md §4), until the game follows it
//! (#1727).

use opensacred_rules::drops::Difficulty;
use opensacred_rules::fight::{KINDS, Rules, ranged_curves, resistance_curves};
use opensacred_rules::set_bonuses::{self, Array, Bonus, Special, Told, Worn, told};

use super::art::wrapped;
use super::lines::{
    KIND_COLOURS, KIND_TEXTS, RED, RESISTANCE_KEYS, SET_NAME, SET_PIECE, SKILL_NAMES, Words,
};
use super::{Line, WHITE};

/// The characters of a colour's code (`\c` and eight digits) before the name's and the pieces'
/// lines, which the window's wrap counts (the style `0x8B`'s bit `0x80`).
const CODE: usize = 10;

/// The wearer whose values some bonuses take: its level, and its attributes `a0` to `a5`.
pub(crate) struct Wearer<'a> {
    pub level: u32,
    pub attributes: [f32; 6],
    pub rules: &'a Rules,
}

/// `x` as a share in per cent, signed, to a tenth where it is not whole.
fn share(x: f32) -> String {
    let p = (f64::from(x) * 1000.0).round() / 10.0;
    if p.fract() == 0.0 {
        format!("{p:+}%")
    } else {
        format!("{p:+.1}%")
    }
}

/// A value, signed, to a tenth where it is not whole.
fn tenths(x: f32) -> String {
    let t = (f64::from(x) * 10.0).round() / 10.0;
    if t.fract() == 0.0 {
        format!("{t:+}")
    } else {
        format!("{t:+.1}")
    }
}

/// A whole value, signed.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a set's bonus of a few hundred at most, whole"
)]
fn plus(x: f32) -> String {
    format!("{:+}", x.trunc() as i64)
}

/// The lines of the sets of `worn` on `difficulty`: a set's name, its pieces of `pieces` (each
/// set's types, by its number) and its bonuses' lines, an empty line between two sets.
pub(crate) fn lines(
    worn: &Worn,
    difficulty: Difficulty,
    pieces: &dyn Fn(u8) -> Vec<u32>,
    wearer: &Wearer,
    words: &Words,
) -> Vec<Line> {
    let mut lines = Vec::new();
    for (set, _) in worn.sets() {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        lines.extend(wrapped(&words.of(&format!("SetName{set}")), CODE, SET_NAME));
        for item in pieces(set) {
            let colour = if worn.wears(item) { SET_PIECE } else { RED };
            lines.extend(wrapped(&words.of(&item.to_string()), CODE, colour));
        }
        for line in told(set, worn, difficulty) {
            lines.extend(told_lines(line, wearer, words));
        }
    }
    lines
}

/// The lines of `line`, white but the kinds' of the sets 33 to 38.
fn told_lines(line: Told, wearer: &Wearer, words: &Words) -> Vec<Line> {
    let n = |number: u32| words.of(&number.to_string());
    let white = |text: String| vec![Line::of(text, WHITE)];
    let a = wearer.attributes;
    match line {
        Told::WholeDamage(x) => white(format!("{}: {}", n(1076), share(x))),
        Told::Survival(per_cent) => white(format!("{}: {}", n(1159), share(per_cent / 100.0))),
        Told::IntoGold => white(words.of("UI_SET_VERSILBERN")),
        Told::Luck(x) => white(format!("{}: {}", n(1124), share(x))),
        // The text holds a space at its end.
        Told::Exorcism(x) => white(format!(
            "{}: {}",
            words.of("UI_RARE_EXORZIST").trim_end(),
            share(x)
        )),
        Told::Experience(x) => white(format!("{}: {}", n(1071), share(x))),
        Told::Sight(units) => white(format!("{}: {}", n(1130), plus(units))),
        // The critical hit's value, as an item's effect 859 adds to it, to a tenth.
        Told::Critical(value) => white(format!("{}: {}", n(1132), tenths(value))),
        // A per cent of the wounds' value: a hundredth's points (#1779).
        Told::Wounds(points) => white(format!("{}: {points:+.2}%", n(1135))),
        // The per cent chances of the effects 866 and 856, as an item's are worded.
        Told::GoldForHealth(chance) => white(format!("{}: {}%", n(1134), tenths(chance))),
        Told::GoldPile(chance) => white(format!("{}: {}%", n(1143), tenths(chance))),
        // The exorcism's value more, as an item's effect 863 adds to it, to a tenth; the text
        // holds a space at its end.
        Told::Exorcist(value) => white(format!(
            "{}: {}",
            words.of("UI_RARE_EXORZIST").trim_end(),
            tenths(value)
        )),
        // The stun's value more, as an item's effect 862 adds to it, to a hundredth: the sets'
        // terms are a twentieth to near one.
        Told::Stun(value) => white(format!(
            "{}: {value:+.2}",
            words.of("UI_RARE_SCHOCK").trim_end()
        )),
        // Set 22's potions at 5, which no text of the game tells: the project's own words that
        // the converter adds (the maintainer's decision of 2026-11-02, #1727; #1823).
        Told::Potions(per_cent) => white(format!(
            "{}: {per_cent}%",
            words.of(opensacred_data::text::SET_POTIONS)
        )),
        // The original's own line of set 26 at 7 (`0x571710`), the Dark Elf's stealth.
        Told::Unseen => white(n(783)),
        // Set 41's line at 4 (`0x571710`'s `{T1150}: %d%%`), the share of the animals' flights
        // in which they stand, of the roll's 101 faces.
        Told::Terror(per_cent) => white(format!("{}: {}", n(1150), share(per_cent / 100.0))),
        // The open wounds' chance of a strike, as the items' effect 857 is worded.
        Told::OpenWounds(chance) => white(format!("{}: {}", n(1151), share(chance))),
        // Against the ranged fighters, as an item's bonus against them is worded.
        Told::RangedDefence(x) => white(format!(
            "{} {} {}: {}",
            n(1161),
            words.of("RES_GEG_FERNK"),
            n(1075),
            share(x)
        )),
        // Against a class of creatures, as an item's bonus against its opponents is worded.
        Told::Against(class, kinds, x) => {
            let opponents = match class {
                set_bonuses::UNDEAD => n(1180),
                set_bonuses::DRAGONS => n(1181),
                _ => n(1182),
            };
            let named = if kinds.len() == KINDS {
                n(1076)
            } else {
                kinds
                    .iter()
                    .map(|&kind| format!("{} {}", words.of(KIND_TEXTS[kind]), n(1076)))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            white(format!("{} {opponents} {named}: {}", n(1161), share(x)))
        }
        Told::Bonus(bonus) => match bonus {
            Bonus::Attribute(i, v) => white(format!(
                "{}: {}",
                n(1090 + u32::try_from(i).unwrap_or(0)),
                plus(v)
            )),
            Bonus::Skill(skill, v) => {
                white(format!("{}: {v:+}", n(SKILL_NAMES + u32::from(skill))))
            }
            Bonus::Arts(arts, v) => arts
                .iter()
                .filter_map(|&art| (words.art)(u32::from(art)))
                .map(|name| Line::of(format!("{name}: {v:+}"), WHITE))
                .collect(),
            Bonus::AllArts(v) => white(format!("{}: {v:+}", n(1137))),
            Bonus::Multiplier(array, kinds, x) => {
                let of = match array {
                    Array::Damage => "1076",
                    Array::Weapon => "1087",
                    Array::Spells => "1088",
                };
                if kinds.len() == KINDS {
                    let named = match array {
                        Array::Damage => n(1076),
                        _ => format!("{} {}", words.of(of), n(1076)),
                    };
                    white(format!("{named}: {}", share(x)))
                } else {
                    kinds
                        .iter()
                        .map(|&kind| {
                            let named = format!("{} {}", words.of(KIND_TEXTS[kind]), words.of(of));
                            Line::of(format!("{named}: {}", share(x)), WHITE)
                        })
                        .collect()
                }
            }
            Bonus::ResistanceFactor(kind, x) => {
                white(format!("{}: {}", words.of(RESISTANCE_KEYS[kind]), share(x)))
            }
            Bonus::Resistance(kind, v) => {
                white(format!("{}: {}", words.of(RESISTANCE_KEYS[kind]), plus(v)))
            }
            Bonus::AttackSpeed(v) => white(format!("{}: {}", n(1100), plus(v))),
            Bonus::AttackTimes(x) => white(format!("{}: {}", n(1074), share(x))),
            Bonus::DefenceTimes(x) => white(format!("{}: {}", n(1075), share(x))),
            Bonus::ArtsSpeed(x) => white(format!("{}: {}", n(1148), share(x))),
            // The pace times the factor, truncated: its share more.
            Bonus::PaceTimes(times) => white(format!("{}: {}", n(1098), share(times - 1.0))),
            // `told` leaves the flags out.
            Bonus::Flag(_) | Bonus::CreatureFlag(_) => Vec::new(),
            Bonus::Special(Special::Festive) => {
                // Each kind's damage `⌊a0/2⌋` to `a0` and its resistance `a1`, in place of the
                // items' (`0x575CA3`).
                let (least, most) = ((a[0] / 2.0).trunc(), a[0].trunc());
                let damages = (0..KINDS).map(|k| {
                    Line::of(
                        format!("{}: {least}-{most}", words.of(KIND_TEXTS[k])),
                        KIND_COLOURS[k],
                    )
                });
                let resistances = (0..KINDS).map(|k| {
                    Line::of(
                        format!("{}: {}", words.of(RESISTANCE_KEYS[k]), a[1].trunc()),
                        KIND_COLOURS[k],
                    )
                });
                damages.chain(resistances).collect()
            }
            // The share of `a0` added to the physical damage's ends, as an item's effect 801 of
            // an attribute is worded.
            Bonus::Special(Special::StrengthPart(per_cent)) => white(format!(
                "{} {}: {}% {}",
                n(1154),
                words.of(KIND_TEXTS[0]),
                plus(per_cent),
                n(1090)
            )),
            Bonus::Special(Special::ResistanceCurves) => {
                let (factors, pace) = resistance_curves(wearer.rules, wearer.level);
                (0..KINDS)
                    .map(|k| {
                        Line::of(
                            format!("{}: {}", words.of(RESISTANCE_KEYS[k]), share(factors[k])),
                            WHITE,
                        )
                    })
                    .chain(std::iter::once(Line::of(
                        format!("{}: {}", n(1098), plus(pace)),
                        WHITE,
                    )))
                    .collect()
            }
            // With a bow or a crossbow in hand alone.
            Bonus::Special(Special::RangedCurves) => {
                let (x, speed) = ranged_curves(wearer.rules, wearer.level);
                let ranged = words.of("EWT_FERNKAMPF");
                vec![
                    Line::of(format!("{ranged} {}: {}", n(1074), share(x)), WHITE),
                    Line::of(format!("{ranged} {}: {}", n(1100), plus(speed)), WHITE),
                ]
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(name: &str) -> Option<String> {
        Some(format!("<{name}>"))
    }

    fn art(id: u32) -> Option<String> {
        Some(format!("<art {id}>"))
    }

    fn words() -> Words<'static> {
        Words {
            text: &text,
            art: &art,
        }
    }

    fn texts(lines: &[Line]) -> Vec<String> {
        lines
            .iter()
            .map(|line| {
                line.0
                    .iter()
                    .map(|piece| match piece {
                        super::super::Piece::Text(text, _) => text.clone(),
                        super::super::Piece::Image(..) => String::new(),
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn a_set_lists_its_name_its_pieces_and_its_bonuses_as_applied() {
        let rules = Rules::default();
        let wearer = Wearer {
            level: 1,
            attributes: [40.0, 30.0, 0.0, 0.0, 0.0, 0.0],
            rules: &rules,
        };
        // Three of set 2's pieces of five and one of set 3's, on gold.
        let worn = Worn::of([(4059, 2), (4055, 2), (4057, 2), (4066, 3)]);
        let pieces = |set: u8| match set {
            2 => vec![4059, 4055, 4057, 4056, 4051],
            3 => vec![4066, 4062],
            _ => Vec::new(),
        };
        let lines = lines(&worn, Difficulty::Gold, &pieces, &wearer, &words());
        assert_eq!(
            texts(&lines),
            [
                "<SetName2>",
                "<4059>",
                "<4055>",
                "<4057>",
                "<4056>",
                "<4051>",
                "<1091>: +16",
                "<1093>: +16",
                "",
                "<SetName3>",
                "<4066>",
                "<4062>",
            ]
        );
        assert_eq!(lines[0], Line::of("<SetName2>", SET_NAME));
        assert_eq!(lines[3], Line::of("<4057>", SET_PIECE));
        assert_eq!(lines[4], Line::of("<4056>", RED));
    }

    #[test]
    fn the_bonuses_tell_their_shares_and_the_wearers_values() {
        let rules = Rules::default();
        let wearer = Wearer {
            level: 1,
            attributes: [41.0, 30.0, 0.0, 0.0, 0.0, 0.0],
            rules: &rules,
        };
        let told = |line| texts(&told_lines(line, &wearer, &words()));
        assert_eq!(told(Told::WholeDamage(0.34)), ["<1076>: +34%"]);
        assert_eq!(told(Told::Survival(16.7)), ["<1159>: +16.7%"]);
        assert_eq!(told(Told::GoldForHealth(50.1)), ["<1134>: +50.1%"]);
        assert_eq!(told(Told::GoldPile(20.0)), ["<1143>: +20%"]);
        assert_eq!(told(Told::OpenWounds(11.0 / 101.0)), ["<1151>: +10.9%"]);
        assert_eq!(told(Told::Exorcist(0.501)), ["<UI_RARE_EXORZIST>: +0.5"]);
        assert_eq!(told(Told::Stun(0.0835)), ["<UI_RARE_SCHOCK>: +0.08"]);
        assert_eq!(told(Told::Unseen), ["<783>"]);
        assert_eq!(told(Told::Terror(2000.0 / 101.0)), ["<1150>: +19.8%"]);
        assert_eq!(told(Told::Potions(10.0)), ["<OPENSACRED_SET_POTIONS>: 10%"]);
        assert_eq!(
            told(Told::Bonus(Bonus::Multiplier(
                Array::Spells,
                &[0, 1, 2, 3],
                0.402
            ))),
            ["<1088> <1076>: +40.2%"]
        );
        assert_eq!(
            told(Told::Bonus(Bonus::Multiplier(Array::Damage, &[2], 0.5))),
            ["<1080> <1076>: +50%"]
        );
        assert_eq!(told(Told::Bonus(Bonus::PaceTimes(1.15))), ["<1098>: +15%"]);
        assert_eq!(
            told(Told::Bonus(Bonus::Arts(&[1014, 1015], 2))),
            ["<art 1014>: +2", "<art 1015>: +2"]
        );
        assert_eq!(
            told(Told::Bonus(Bonus::Special(Special::Festive)))[..2],
            ["<1078>: 20-41", "<1079>: 20-41"]
        );
        assert_eq!(
            told(Told::Bonus(Bonus::Special(Special::Festive)))[7],
            "<INVENTAR_RES_POISON>: 30"
        );
        assert_eq!(
            told(Told::Bonus(Bonus::Special(Special::StrengthPart(53.0)))),
            ["<1154> <1078>: +53% <1090>"]
        );
    }
}
