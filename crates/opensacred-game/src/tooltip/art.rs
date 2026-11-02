//! The lines of a combat art's tooltip (#1694; docs/re/arts.md, "The art's tooltip"), as the
//! bar's art slot asks for them (`0x5893B0` with the flags 0): its name in the hero's class's
//! colour (grey where the hero does not hold it, which colours the rest grey too), a red line
//! where it may not be made on horseback or wants a bow or a two-handed weapon, its short
//! description, its level, its regeneration time over the hero's speed, then its own lines by its
//! id, of its records' values at its level.
//!
//! The spells page asks for them with the flags 1 (#1696): a held art's lines end with the next
//! level's line and its block, its time the entry's scaled by the record's growth.
//!
//! Not followed: the first block's time of the flags 1 by the last entry's level and bonus (the
//! entry's own taken, as the bar's), the language's other
//! level line (`0x17E7D34` 3), and of some arts a part of their values: the weapon multipliers'
//! second array (`+0xA6`, the stats' `+0x96` alone taken) of 56, 1018 and 1085 to 1087 and 1101
//! to 1103, the traps' skill 26 (its curve taken as 0) of 1049, 1050 and 1052, the difficulty's
//! scaling of the levels of 66, 1003 and 1091 (bronze's), the hero's weapon of 1013 and 1054 (no
//! line), the stats' flag `0x400000` of 1022 and 1023 (1), and the lowest bit of 1011's value,
//! which the original prints as the bits of a double.

use opensacred_data::tables::{Arts as Tables, Spell, WeaponArt};
use opensacred_rules::arts::{self, Arts, Speeds};
use opensacred_rules::fight::KINDS;

use super::{Line, Piece, WHITE};

/// The colours: the restriction's red, the level line's, the grey of an art not held, and the
/// damage's kinds' (fire, magic, poison; the physical in the line's own).
const RED: u32 = 0xFFFF_0000;
const LEVEL: u32 = 0xFFFF_FF80;
const GREY: u32 = 0xFFAF_AFAF;
const DAMAGE_COLOURS: [u32; 3] = [0xFFFF_5A09, 0xFF09_77FF, 0xFF09_FF15];
/// The classes' colours by the creature type (`0x43C250`), any other black.
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
/// The class `Rundum` and `Hardhit` arts that a bow in hand makes 1061 and 1062.
const RUNDUM: [u16; 6] = [1057, 1065, 1069, 1073, 1078, 1090];
const HARDHIT: [u16; 6] = [1058, 1066, 1070, 1074, 1079, 1091];
/// The weapon arts that cast a spell, and its id (`0x4D1E10`).
const CASTS: [(u16, u16); 8] = [
    (1018, 8),
    (1019, 51),
    (1044, 47),
    (1049, 45),
    (1050, 6),
    (1051, 48),
    (1052, 12),
    (1053, 46),
];

/// The hero that the tooltip weighs the art for: its creature type, its level, its arts, their
/// speeds, its damages' multipliers (the stats' `+0x96`) and its spells' (`+0x96` and `+0xB6`),
/// its attributes and skills, whether it holds a ranged weapon, whether the day shows, and the
/// fight's rules.
pub(crate) struct Hero<'a> {
    pub kind: u32,
    pub level: u32,
    pub arts: &'a Arts,
    pub speeds: Speeds,
    pub multipliers: [f32; KINDS],
    pub spells: [f32; KINDS],
    pub attributes: [f32; 6],
    pub skills: &'a [(u8, u32)],
    pub ranged: bool,
    pub day: bool,
    pub rules: &'a opensacred_rules::fight::Rules,
    /// Whether its weapon arts' strikes are halved, the hero holding two weapons (its arts'
    /// clips' `halved`, `0x4EA3F0`).
    pub halved: bool,
}

impl Hero<'_> {
    /// The level of its skill `skill`, 0 without it.
    fn skill(&self, skill: u8) -> u32 {
        self.skills
            .iter()
            .find(|&&(id, _)| id == skill)
            .map_or(0, |&(_, level)| level)
    }

    /// The curve of its skill `skill` at its level, 0 without it.
    fn curve(&self, skill: u8) -> f64 {
        match self.skill(skill) {
            0 => 0.0,
            level => f64::from(self.rules.lore_curve(skill, level).unwrap_or(0.0)),
        }
    }
}

/// The game's texts by name, and by their ids.
pub(crate) struct Words<'a> {
    pub text: &'a dyn Fn(&str) -> Option<String>,
    pub id: &'a dyn Fn(u32) -> Option<String>,
}

impl Words<'_> {
    fn of(&self, name: &str) -> String {
        (self.text)(name).unwrap_or_default()
    }

    /// The text of the number `number`.
    fn n(&self, number: u32) -> String {
        self.of(&number.to_string())
    }
}

/// `x` truncated toward zero, as the runtime's `_ftol`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a shown value, well within an i64, truncated as the original's _ftol"
)]
fn ftol(x: f64) -> i64 {
    x.trunc() as i64
}

/// `x` as a float, a shown value well within its precision.
#[expect(clippy::cast_precision_loss, reason = "a shown value, far below 2^52")]
fn float(x: i64) -> f64 {
    x as f64
}

/// A record's word `base + step·level` as its 16 bits hold it, signed.
fn word([base, step]: [u16; 2], level: u32) -> i64 {
    let raw = u32::from(step)
        .wrapping_mul(level)
        .wrapping_add(u32::from(base));
    i64::from(i16::from_le_bytes(
        u16::try_from(raw & 0xFFFF).unwrap_or(0).to_le_bytes(),
    ))
}

/// What the lines take of an art at its level.
struct Values<'a> {
    level: u32,
    hero: &'a Hero<'a>,
    spell: Option<&'a Spell>,
    weapon: Option<&'a WeaponArt>,
}

impl Values<'_> {
    fn l(&self) -> f64 {
        f64::from(self.level)
    }

    /// The effect `k` (`e_k`).
    fn e(&self, k: usize) -> i64 {
        self.spell.map_or(0, |s| word(s.effects[k], self.level))
    }

    /// The value `j` held at its cap (`v_j`, `0x4D26B0`).
    fn v(&self, j: usize) -> i64 {
        self.spell.map_or(0, |s| {
            let value = word(s.values[j], self.level);
            match s.caps[j] {
                0 => value,
                cap => value.min(i64::from(cap)),
            }
        })
    }

    /// The value `a` over the value `b`, held at `a`'s cap, `a` alone where `b` is 0 (`0x4D26E0`).
    fn r(&self, a: usize, b: usize) -> i64 {
        match self.v(b) {
            0 => self.v(a),
            over => {
                let ratio = self.v(a) / over;
                match self.spell.map_or(0, |s| s.caps[a]) {
                    0 => ratio,
                    cap => ratio.min(i64::from(cap)),
                }
            }
        }
    }

    /// The four damages, each the effect times its multiplier truncated, `weapon` the stats'
    /// damage multipliers rather than the spells'.
    fn damages(&self, weapon: bool) -> [i64; KINDS] {
        let multipliers = if weapon {
            self.hero.multipliers
        } else {
            self.hero.spells
        };
        std::array::from_fn(|k| ftol(float(self.e(k)) * f64::from(multipliers[k])))
    }

    /// The weapon art's values by the level: `W50`, `W58` (floats), `W60`, `W68` (whole).
    fn w50(&self) -> f64 {
        self.weapon.map_or(0.0, |w| {
            self.l() * f64::from(w.factor[1]) + f64::from(w.factor[0])
        })
    }

    /// The factor that the art's strike carries (`0x4EA3F0`, `opensacred_rules::arts::held_factor`):
    /// its `W50`, halved where the hero holds two weapons, where the original's lines take `W50`
    /// whole (#1742).
    fn strike(&self) -> f64 {
        self.weapon.map_or(0.0, |w| {
            f64::from(arts::held_factor(
                arts::factor(w, self.level),
                self.hero.halved,
            ))
        })
    }

    fn w58(&self) -> f64 {
        self.weapon.map_or(0.0, |w| {
            self.l() * f64::from(w.value_58[1]) + f64::from(w.value_58[0])
        })
    }

    fn w60(&self) -> i64 {
        self.weapon.map_or(0, |w| {
            i64::from(w.value_60[1]) * i64::from(self.level) + i64::from(w.value_60[0])
        })
    }

    fn w68(&self) -> i64 {
        self.weapon.map_or(0, |w| {
            i64::from(w.value_68[1]) * i64::from(self.level) + i64::from(w.value_68[0])
        })
    }

    /// The attack's bonus in per cent, of the factor that the strike takes
    /// (`opensacred_rules::fight::art_attack_factor`), where the original's
    /// `⌊(0.125·L + 1.3f)·100 − 100⌋` takes the float 1.3 a little short and truncates, one
    /// short of the strike's at the level 0 (#1741).
    fn attack(&self) -> i64 {
        per_cent(opensacred_rules::fight::art_attack_factor(self.level))
    }

    /// The chance to block of the level `⌊100·min(1, n(5L + 9)/(H + 9 + n(5L + 9)))⌋`.
    fn block(&self, n: f64) -> i64 {
        let own = n * (5.0 * self.l() + 9.0);
        let chance = (own / (f64::from(self.hero.level) + 9.0 + own)).min(1.0);
        ftol(100.0 * chance)
    }

    /// The kicks' reach's factor `min(1, 5L/H)`, 1 at the level 0.
    fn kick(&self) -> f64 {
        match self.hero.level {
            0 => 1.0,
            level => (5.0 * self.l() / f64::from(level)).min(1.0),
        }
    }

    /// The trap's factor `1 + 0.01f·(c₂ + c₂₆) + (e2 + a2)/BalanceGeschick`, the skill 26's curve
    /// and the extra points `e2` taken as 0.
    fn trap(&self) -> f64 {
        self.hero.curve(2) * f64::from(0.01_f32)
            + f64::from(self.hero.attributes[2]) / f64::from(self.hero.rules.geschick())
            + 1.0
    }

    /// The Dwarf's guns' factor `1 + a2/BalanceGeschick + 0.01·c₂` (`0x4D21C0`), as his guns
    /// take it (`opensacred_rules::fight::Rules::glance_factor`), 1 of another.
    fn gun(&self) -> f64 {
        if self.hero.kind == 8 {
            self.hero
                .rules
                .glance_factor(self.hero.skills, self.hero.attributes[2])
        } else {
            1.0
        }
    }

    /// The Daemon's multiplier of the kind `kind` that his arts of the second table take, 1.0
    /// for another caster (`0x4D2250`…).
    fn daemon(&self, kind: usize) -> f32 {
        if self.hero.kind == 9 {
            self.hero.multipliers[kind]
        } else {
            1.0
        }
    }
}

/// The characters of a line of the tooltip's text after which its first space breaks it (the
/// style's bit `0x80`, `0x6E69F0`).
const WRAP: usize = 46;
/// The characters of a colour's code before a line's text (`\c` or `\d` and eight digits), which
/// the wrap counts.
const CODE: usize = 10;

/// `text` broken as the tooltip's window breaks a line of its style's bit `0x80`: at the first
/// space from its 46th character on, the space left out, `code` characters of a colour's code
/// counted before it.
fn wrap(text: &str, code: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut count = code;
    for c in text.chars() {
        if c == ' ' && count >= WRAP {
            lines.push(std::mem::take(&mut line));
            count = 0;
            continue;
        }
        line.push(c);
        count += 1;
    }
    lines.push(line);
    lines
}

/// The lines of `text` in `colour` as the window breaks them, after a code of `code` characters.
fn wrapped(text: &str, code: usize, colour: u32) -> impl Iterator<Item = Line> {
    wrap(text, code)
        .into_iter()
        .map(move |line| Line::of(line, colour))
}

/// The per cent that the factor `x` adds, `100·x − 100` rounded to the nearest, where the
/// original's `P` adds 0.5 and truncates toward zero, a negative factor's one point short: −19
/// for the 0.80000001 of 1071 (#1741; docs/decisions/arts.md, "Arts' tooltips' numbers from
/// their code").
fn per_cent(x: f64) -> i64 {
    ftol((100.0 * x - 100.0).round())
}

/// The damage's line (`DMG`): `label: total = physical+fire+magic+poison`, the fire, magic and
/// poison parts in their colours and each `+` after them white.
fn damage_line(label: String, total: i64, parts: [i64; KINDS], colour: u32) -> Line {
    let mut pieces = vec![Piece::Text(
        format!("{label}: {total} = {}+", parts[0]),
        colour,
    )];
    for (k, (&part, &tint)) in parts[1..].iter().zip(&DAMAGE_COLOURS).enumerate() {
        pieces.push(Piece::Text(part.to_string(), tint));
        if k < 2 {
            pieces.push(Piece::Text("+".into(), WHITE));
        }
    }
    Line(pieces)
}

/// The tooltip's lines of the art `art` for `hero`, and with `next` (the spells page's flags 1)
/// a held art's next level; none for no art or a combo's.
pub(crate) fn lines(
    art: u16,
    hero: &Hero,
    tables: &Tables,
    words: &Words,
    next: bool,
) -> Vec<Line> {
    if art == 0 || (0x200..=0x204).contains(&art) {
        return Vec::new();
    }
    // The level in play, 0 where the hero holds none (`0x580A10`).
    let held = hero
        .arts
        .entries
        .iter()
        .find(|entry| entry.art == art)
        .map_or(0, |entry| entry.in_play());
    let id = if hero.ranged && RUNDUM.contains(&art) {
        1061
    } else if hero.ranged && HARDHIT.contains(&art) {
        1062
    } else {
        art
    };
    let spell = tables.spells.iter().find(|s| s.id == id);
    let weapon = tables.weapon_arts.iter().find(|w| w.id == id);
    let (name, description) = spell
        .map(|s| (s.name, s.short_description))
        .or_else(|| weapon.map(|w| (w.name, w.short_description)))
        .unwrap_or((0, 0));
    let cast = CASTS
        .iter()
        .find(|&&(of, _)| weapon.is_some() && of == id)
        .and_then(|&(_, spell)| tables.spells.iter().find(|s| s.id == spell));
    let spell = cast.or(spell);
    let base = if held == 0 { GREY } else { WHITE };
    let name_colour = if held == 0 {
        GREY
    } else {
        usize::try_from(hero.kind)
            .ok()
            .and_then(|kind| CLASS_COLOURS.get(kind))
            .copied()
            .unwrap_or(CLASS_COLOURS[0])
    };
    // Each line as the window breaks it, those after a colour's code counting it.
    let mut lines: Vec<Line> =
        wrapped(&(words.id)(name).unwrap_or_default(), CODE, name_colour).collect();
    if let Some(key) = restriction(spell.map(|s| s.flags), weapon.map(|w| w.flags)) {
        lines.extend(wrapped(&words.of(key), CODE, RED));
    }
    lines.push(Line::default());
    let description = (words.id)(description).unwrap_or_default();
    for line in description.split(['\n']).flat_map(|line| line.split("<n>")) {
        lines.extend(wrapped(line, 0, base));
    }
    lines.push(Line::default());
    if held == 0 {
        lines.extend(wrapped(&format!("{}: 1", words.n(1072)), 0, base));
    } else {
        let level = format!("{} {}: {held}", words.n(1109), words.n(1070));
        lines.extend(wrapped(&level, CODE, LEVEL));
    }
    // The regeneration time over its kind's speed: the entry's, else its record's at 0, or with
    // `next` at 1, the level its block shows (`0x582A20`).
    let speed = if id < 999 || id == 1019 {
        hero.speeds.spells
    } else {
        hero.speeds.weapon_arts
    };
    let time = hero
        .arts
        .entries
        .iter()
        .find(|entry| entry.art == art)
        .map(|entry| f64::from(entry.time))
        .or_else(|| opensacred_rules::arts::time(tables, id, u32::from(next)))
        .unwrap_or(0.0)
        / f64::from(speed).max(f64::EPSILON);
    // The block of the level `level`, its time `time`.
    let block = |level: u32, time: f64| {
        let values = Values {
            level,
            hero,
            spell,
            weapon,
        };
        // An art of no case of its own shows its regeneration alone, which it has, where the
        // original's shows nothing (#1742).
        let own = own_lines(id, &values, words, base).unwrap_or_default();
        let mut block = vec![Line::of(format!("{}: {time:.1}s", words.n(1107)), base)];
        block.extend(own);
        // A line of one colour broken as the window breaks it; the damage's of several colours
        // has no space after its 46th character.
        block
            .into_iter()
            .flat_map(|line| match line.0.as_slice() {
                [Piece::Text(text, colour)] => wrapped(text, 0, *colour).collect(),
                _ => vec![line],
            })
            .collect::<Vec<_>>()
    };
    lines.extend(block(held.max(1), time));
    // The spells page's next level of a held art (the flags 1): its line, then its block, its time
    // the entry's scaled by the record's growth.
    if next && held != 0 {
        let growth = match (
            opensacred_rules::arts::time(tables, id, held),
            opensacred_rules::arts::time(tables, id, held + 1),
        ) {
            (Some(now), Some(then)) if now > f64::from(f32::EPSILON) => then / now,
            _ => 1.0,
        };
        lines.push(Line::default());
        lines.push(Line::of(
            format!("{}:", words.of("UI_TT_NEXTLEARNEDLEVEL")),
            LEVEL,
        ));
        lines.extend(block(held + 1, time * growth));
    }
    lines
}

/// The statistics window's damage of the art `art` (`0x6A6550`, docs/re/character.md, "The
/// combat art's damage"): each kind's least and most at the hero's level of it, a spell's of its
/// record's effects times its multipliers (the traps' times their factor), a weapon art's of the
/// weapon's damages `weapon` (or of its own values) times its factor by the art, each end
/// `f·d + 0.5`; none where the hero holds no such art or neither record is found.
pub(crate) fn window_damage(
    art: u16,
    hero: &Hero,
    tables: &Tables,
    weapon: [[f32; 2]; KINDS],
) -> Option<[[f64; 2]; KINDS]> {
    let held = hero
        .arts
        .entries
        .iter()
        .find(|entry| entry.art == art)
        .map_or(0, |entry| entry.in_play());
    if held == 0 {
        return None;
    }
    let id = if hero.ranged && RUNDUM.contains(&art) {
        1061
    } else if hero.ranged && HARDHIT.contains(&art) {
        1062
    } else {
        art
    };
    let weapon_art = tables.weapon_arts.iter().find(|w| w.id == id);
    let cast = CASTS
        .iter()
        .find(|&&(of, _)| weapon_art.is_some() && of == id)
        .and_then(|&(_, spell)| tables.spells.iter().find(|s| s.id == spell));
    let spell = cast.or_else(|| tables.spells.iter().find(|s| s.id == id));
    let values = Values {
        level: held,
        hero,
        spell,
        weapon: weapon_art,
    };
    // A spell's: its damages, the mines' and 1018's by the weapon's multipliers, the traps' times
    // their factor, both ends alike, with no half added.
    if spell.is_some() {
        let mut damages = values.damages(id == 56 || id == 1018);
        if matches!(id, 1049 | 1050 | 1052) {
            let factor = values.trap();
            damages = damages.map(|d| ftol(float(d) * factor));
        }
        return Some(damages.map(|d| [float(d), float(d)]));
    }
    let weapon_art = weapon_art?;
    let (w50, w58) = (values.w50(), values.w58());
    let l = f64::from(held);
    let m = |k: usize| f64::from(hero.multipliers[k]);
    // The Daemon's arts' kinds by her multipliers (`0x4D2250` to `0x4D22B0`), 1 of another.
    let mu = |k: usize| if hero.kind == 9 { m(k) } else { 1.0 };
    let gamma = values.gun();
    let own = |d: f64| [d, d];
    let weapons = weapon.map(|ends| ends.map(f64::from));
    let mut d = [[0.0; 2]; KINDS];
    let f = match id {
        1004 | 1009 | 1060 | 1064 | 1068 | 1072 | 1076 | 1013 | 1054 => {
            d = weapons;
            w50
        }
        1008 => {
            d = weapons;
            w58
        }
        1007 => {
            d = weapons;
            1.0
        }
        1041 => {
            d = weapons;
            ((0.1 * l + 0.5 + 1.0) * 100.0 - 100.0 + 0.5) * 0.01
        }
        1014 | 1015 | 1031 => 1.0,
        1030 => {
            d[0] = own(w50);
            1.0
        }
        1033 => {
            d[0] = own(10.0 * w50);
            1.0
        }
        1016 => {
            let lore = 0.01 * hero.curve(2)
                + f64::from(hero.attributes[0]) / f64::from(hero.rules.geschick());
            d[0] = own((1.0 + lore) * w50);
            1.0
        }
        1032 => {
            d[1] = own(w58);
            1.0
        }
        1085 => {
            d[1] = own(w58 * m(1));
            gamma
        }
        1086 | 1087 => {
            d[0] = own(w50 * m(0));
            d[1] = own(w58 * m(1));
            gamma
        }
        1101 => {
            d[3] = own(mu(3) * w50);
            1.0
        }
        1102 => {
            d[1] = own(mu(1) * w50);
            1.0
        }
        1103 => {
            d[0] = own(w50);
            d[2] = own(mu(2) * w58);
            1.0
        }
        _ => {
            d = weapons;
            level_factor(id, l, (w50, w58), weapon_art)
        }
    };
    Some(d.map(|ends| ends.map(|end| f * end + 0.5)))
}

/// A weapon art's factor of its level (`0x4D1EB0`): `W50` of most, `W58` of 1003 and 1083,
/// `√L/W5C + W58` of 1024 and 1025, `0.1·L + 1` of 1010, 0 of the rest.
fn level_factor(id: u16, l: f64, (w50, w58): (f64, f64), weapon: &WeaponArt) -> f64 {
    match id {
        1000..=1002
        | 1012
        | 1013
        | 1015..=1017
        | 1020
        | 1031
        | 1033
        | 1038..=1040
        | 1054
        | 1057..=1059
        | 1061..=1063
        | 1065..=1067
        | 1069..=1071
        | 1073..=1075
        | 1078..=1080
        | 1084
        | 1090..=1092 => w50,
        1003 | 1083 => w58,
        1024 | 1025 => {
            let step = f64::from(weapon.value_58[1]);
            let root = if step == 0.0 { 0.0 } else { l.sqrt() / step };
            root + f64::from(weapon.value_58[0])
        }
        1010 => 0.1 * l + 1.0,
        _ => 0.0,
    }
}

/// The key of the line of where the art may not be made, from its records' flags (the spell's
/// `+0x90`, the weapon art's `+0x74`): not on horseback, on horseback only with a bow, or with a
/// two-handed weapon only.
fn restriction(spell: Option<u8>, weapon: Option<u8>) -> Option<&'static str> {
    // Whether it may be made on horseback, with a bow (`b`) or not (`0x4D25A0`, `0x4D1BE0`).
    let may = |flags: u8, bow: bool| {
        if flags & 2 == 0 || bow {
            flags & 1 == 0
        } else {
            false
        }
    };
    let mut kind = 0;
    for flags in [spell, weapon].into_iter().flatten() {
        if !may(flags, false) {
            kind = if may(flags, true) { 2 } else { 1 };
        }
    }
    if [spell, weapon].into_iter().flatten().any(|f| f & 4 != 0) {
        kind = 4;
    }
    match kind {
        1 => Some("UI_HORSE_CANTCAST"),
        2 => Some("UI_HORSE_ONLYWITHBOW"),
        4 => Some("UI_STAT_2H_ONLY"),
        _ => None,
    }
}

/// The art's own lines by its id after its time's (`0x5893B0`'s switch), none where it has no
/// case or lacks the record its case reads.
fn own_lines(id: u16, v: &Values, words: &Words, base: u32) -> Option<Vec<Line>> {
    let n = |number: u32| words.n(number);
    let k = |key: &str| words.of(key);
    let plain = |text: String| Line::of(text, base);
    let seconds = |label: String, value: i64| plain(format!("{label}: {value}s"));
    let whole = |label: String, value: i64| plain(format!("{label}: {value}"));
    let signed = |label: String, value: i64| plain(format!("{label}: {value:+}"));
    let signed_pc = |label: String, value: i64| plain(format!("{label}: {value:+}%"));
    let pc = |label: String, value: i64| plain(format!("{label}: {value}%"));
    let metres = |label: String, value: f64| plain(format!("{label}: {value:2.2}m"));
    let dmg = |damages: [i64; KINDS]| damage_line(n(1085), damages.iter().sum(), damages, base);
    let duration = || seconds(n(1097), v.v(0));
    let spell = v.spell.is_some();
    let weapon = v.weapon.is_some();
    let magic_lore = || ftol(float(v.v(0)) * (1.0 + f64::from(0.01_f32) * v.hero.curve(14)));
    // The lines of the effects that a spell's objects give by their action 6, of the values that
    // give them (`opensacred_rules::spells::effects`), each by the game's own text: the attack's
    // and the defence's per cent, the four resistances, the speed's per cent, the regeneration of
    // the spells and of the weapon arts, and the regeneration (#1742).
    let effect_lines = || -> Vec<Line> {
        let per_cent = |value: f32| ftol((f64::from(value) * 100.0).round());
        let resistance = |key: &str, value: f32| signed(k(key), ftol(f64::from(value).round()));
        v.spell
            .map(|spell| opensacred_rules::spells::effects(spell, v.level))
            .unwrap_or_default()
            .into_iter()
            .flat_map(|(effect, _)| match effect.id {
                1 => vec![signed_pc(n(1166), per_cent(effect.value))],
                2 => vec![signed_pc(n(1167), per_cent(effect.value))],
                7 => vec![resistance("INVENTAR_RES_PHYSICAL", effect.value)],
                8 => vec![resistance("INVENTAR_RES_FIRE", effect.value)],
                9 => vec![resistance("INVENTAR_RES_MAGIC", effect.value)],
                10 => vec![resistance("INVENTAR_RES_POISON", effect.value)],
                11 => vec![
                    signed_pc(n(1148), per_cent(effect.value)),
                    signed_pc(n(1149), per_cent(effect.value)),
                ],
                12 => vec![signed_pc(n(1093), per_cent(effect.value))],
                28 => vec![signed_pc(n(1098), per_cent(effect.value - 1.0))],
                _ => Vec::new(),
            })
            .collect()
    };
    let lines = match id {
        // The farthest the teleport takes the caster, `v0 − 1` steps of 10 (`spells::teleport`,
        // `TELEPORT_STEP`) in metres as the other ranges, where the original's line prints the
        // count of its steps `v0` as the range (#1741).
        1 => vec![metres(
            n(1083),
            f64::from(0.02_f32)
                * f64::from(opensacred_rules::spells::TELEPORT_STEP)
                * float((v.v(0) - 1).max(0)),
        )],
        2 => vec![
            seconds(n(1097), v.v(0)),
            signed_pc(n(1167), v.e(5)),
            signed(k("INVENTAR_RES_PHYSICAL"), v.e(6)),
            signed(k("INVENTAR_RES_POISON"), v.e(9)),
            signed(k("INVENTAR_RES_FIRE"), v.e(7)),
        ],
        3 => vec![whole(n(1070), v.v(0))],
        4 | 6 | 9 | 10 | 16 | 18 | 19 | 25 | 36 | 40 => vec![duration()],
        5 | 13 | 38 => vec![dmg(v.damages(false))],
        7 => vec![plain(format!("{}: +{}", n(1073), magic_lore()))],
        8 => vec![duration(), whole(n(1098), v.v(1)), whole(n(1083), v.v(2))],
        11 | 15 | 17 | 23 | 60 | 64 => vec![dmg(v.damages(false)), duration()],
        12 => vec![whole(n(1097), v.v(0)), whole(n(1104), v.v(1))],
        14 => {
            let d = v.damages(false);
            vec![
                plain(format!(
                    "{}: {}({}+{}+{}+{})",
                    n(1085),
                    d.iter().sum::<i64>(),
                    d[0],
                    d[1],
                    d[2],
                    d[3]
                )),
                whole(n(1105), v.v(0)),
            ]
        }
        20 => vec![dmg(v.damages(false)), duration(), whole(n(1104), v.v(1))],
        21 => vec![duration(), signed_pc(n(1166), v.e(4))],
        22 => vec![pc(k("CHANCE4BLOCK"), v.block(2.0)), duration()],
        // The fire skin, the quickening, the speed, the shield wall and the mana's spring: every
        // effect that their objects' action 6 gives, from what gives it, where the original's
        // lines leave out the fire skin's defence, the quickening's and the speed's attack and
        // the shield wall's physical resistance, and name the spring's speed of both kinds of
        // arts the mental regeneration (#1742).
        24 => [
            vec![
                pc(k("CHANCE4BLOCK"), v.block(1.0)),
                dmg(v.damages(false)),
                duration(),
            ],
            effect_lines(),
        ]
        .concat(),
        26 => vec![dmg(v.damages(false)), whole(n(1105), v.r(0, 1))],
        33 => vec![dmg(v.damages(false)), whole(n(1105), v.v(0))],
        27 | 31 | 37 => [vec![duration()], effect_lines()].concat(),
        28 => vec![
            pc(k("CHANCE4BLOCK"), v.block(1.0)),
            dmg(v.damages(false)),
            duration(),
        ],
        29 => vec![
            dmg(v.damages(false)),
            whole(n(1106), v.v(0)),
            whole(k("TOOLTIP_RUECKWURF_MAGE"), v.v(1)),
        ],
        30 => [
            vec![pc(k("CHANCE4BLOCK"), v.block(1.0)), duration()],
            effect_lines(),
        ]
        .concat(),
        32 => vec![duration(), signed_pc(n(1093), v.e(12))],
        34 => vec![whole(n(1106), v.r(0, 1)), dmg(v.damages(false))],
        35 => vec![duration(), dmg(v.damages(false))],
        39 | 41 => vec![
            dmg(v.damages(false)),
            whole(n(1105), v.r(0, 2)),
            seconds(n(1097), v.v(1)),
        ],
        42 => vec![whole(n(1073), magic_lore())],
        55 if spell => vec![
            signed(n(1124), v.v(1)),
            pc(k("UI_STAT_REDUCE_VW"), ten_thousandth(v.e(5))),
            duration(),
        ],
        56 if spell => vec![whole(n(1105), v.r(0, 1)), dmg(v.damages(true))],
        57 if spell => {
            let r = [v.e(6), v.e(7), v.e(8), v.e(9)];
            vec![
                signed_pc(n(1167), v.e(5)),
                damage_line(n(1086), r.iter().sum(), r, base),
                duration(),
            ]
        }
        58 if spell => {
            let r = [0, v.e(7), v.e(8), v.e(9)];
            vec![damage_line(n(1086), r.iter().sum(), r, base), duration()]
        }
        59 if spell => vec![
            signed_pc(n(1166), v.e(4)),
            pc(k("UI_STAT_REDUCE_ENEMY_PHY_RES"), ten_thousandth(v.e(6))),
            duration(),
        ],
        // The most that the kills raise the strikes' damage to, of its word `v1`
        // (`spells::doom_most`), where the original's line takes a tenth of the duration `v0`,
        // +3 % for +90 % at the level 1 (#1741).
        62 if spell => vec![
            plain(format!(
                "{}: +{}%",
                k("UI_MAX_DMGBONUS"),
                per_cent(f64::from(opensacred_rules::spells::doom_most(
                    u16::try_from(v.v(1)).unwrap_or(0)
                )))
            )),
            duration(),
        ],
        63 if spell => vec![
            signed_pc(n(1074), v.e(4)),
            dmg(v.damages(false)),
            signed_pc(k("UT_FIREDMG_BONUS"), v.v(1)),
            duration(),
        ],
        65 if spell => vec![
            dmg(v.damages(false)),
            seconds(k("UI_DAUER_TENTAKEL"), v.v(1)),
            seconds(k("UI_DAUER_AURA"), v.v(0)),
        ],
        // The time's factor that the choir's objects give a creature of the hero's level
        // (`spells::choir_slows`), where the original's line weighs `0.75·a/(5·L)` (#1741).
        66 if spell => {
            let q = f64::from(opensacred_rules::spells::choir_slows(v.hero.level, v.level));
            vec![
                dmg(v.damages(false)),
                plain(format!(
                    "{}: {}% ({})",
                    k("UT_SLOWDOWN_VS_LEVEL"),
                    ftol(100.0 * q),
                    v.hero.level
                )),
                duration(),
            ]
        }
        67 if spell => vec![
            pc(k("UI_REDUCED_ATTACK"), ten_thousandth(v.e(4))),
            pc(k("UI_REDUCED_DEFENSE"), ten_thousandth(v.e(5))),
            duration(),
        ],
        1000 | 1057 | 1061 | 1065 | 1069 | 1073 | 1078 | 1090 | 1038 => vec![
            signed_pc(n(1116), per_cent(v.strike())),
            plain(format!("{}: +{}%", n(1104), v.w60() - 100)),
            signed_pc(n(1074), v.attack()),
        ],
        1001 | 1058 | 1062 | 1066 | 1070 | 1074 | 1079 | 1002 | 1059 | 1063 | 1067 | 1071
        | 1075 | 1080 | 1092 | 1039 | 1040 => vec![
            signed_pc(n(1116), per_cent(v.strike())),
            signed_pc(n(1074), v.attack()),
        ],
        1003 | 1091 => {
            let level = v.hero.level;
            let chance = if level == 0 {
                100
            } else {
                ftol(100.0 * 5.0 * v.l() / f64::from(level)).min(100)
            };
            let mut lines = vec![
                signed_pc(
                    n(1116),
                    if id == 1003 {
                        ftol(100.0 * v.w58() - 99.5)
                    } else {
                        per_cent(v.strike())
                    },
                ),
                plain(format!(
                    "{} ({}) {chance}% ({level})",
                    k("UI_STAT_STUNNCHANCE"),
                    k("UI_STAT_AGAINST_LEVEL")
                )),
            ];
            if id == 1091 {
                lines.push(signed_pc(n(1074), v.attack()));
            }
            lines
        }
        1004 | 1012 | 1017 => vec![signed_pc(n(1116), per_cent(v.strike()))],
        1005 => vec![
            seconds(n(1097), v.w60()),
            whole(n(1105), ftol(v.w50() / v.w58())),
            signed_pc(n(1074), v.attack()),
        ],
        1006 => vec![
            whole(n(1105), if v.w68() == 0 { 0 } else { v.w60() / v.w68() }),
            whole(
                n(1070),
                (5 * i64::from(v.level)).min(i64::from(v.hero.level)),
            ),
        ],
        1007 => vec![whole(k("TOOLTIP_RUECKWURF_HELF"), v.w60())],
        1008 => vec![
            pc(n(1116), per_cent(v.w58())),
            seconds(n(1097), ftol(v.w50())),
        ],
        1009 | 1060 | 1064 | 1068 | 1072 | 1076 | 1093 | 1041 => {
            let base_60 = v.weapon.map_or(0.0, |w| f64::from(w.value_60[0]));
            let reach = metres(n(1083), f64::from(0.02_f32) * base_60 * v.kick());
            let strike = if id == 1041 {
                pc(
                    n(1116),
                    ftol((f64::from(0.1_f32) * v.l() + 0.5 + 1.0) * 100.0 - 100.0 + 0.5),
                )
            } else {
                signed_pc(n(1116), per_cent(v.strike()))
            };
            vec![reach, strike, signed_pc(n(1074), v.attack())]
        }
        1010 => vec![signed_pc(n(1116), ftol(10.0 * v.l() + 0.5))],
        1011 => {
            let w = v.weapon;
            let value = w.map_or(0.0, |w| {
                (v.l() * f64::from(w.factor[1]) + f64::from(w.value_58[0]) + f64::from(w.factor[0])
                    - f64::from(w.value_58[1]))
                .max(f64::from(w.factor[0]))
            });
            vec![whole(n(1104), ftol(value))]
        }
        1013 | 1054 | 1021 | 1034 => vec![],
        1014 => {
            let mage = v.weapon.map_or(0.0, |w| {
                f64::from(0.02_f32)
                    * ((f64::from(w.value_60[1]) + 1.0) * v.l() - f64::from(w.value_68[1])
                        + f64::from(w.value_68[0])
                        + f64::from(w.value_60[0]))
            });
            vec![
                whole(n(1104), ftol(v.w50())),
                metres(k("TOOLTIP_RUECKWURF_MAGE"), mage),
                seconds(n(1097), ftol(v.w58())),
            ]
        }
        1015 => vec![
            signed_pc(n(1166), ftol(100.0 * v.w50())),
            signed_pc(n(1167), ftol(100.0 * v.w50())),
            seconds(n(1097), ftol(v.w58())),
        ],
        // The strike's whole value as the art gives it, taken whole, where the original's line
        // rounds it, one above it at times (13 shown for 12 dealt; #1741).
        1016 => {
            let factor = v
                .hero
                .rules
                .glance_factor(v.hero.skills, v.hero.attributes[0]);
            let strikes = v
                .weapon
                .and_then(|w| opensacred_rules::arts::glance(w, v.level, factor))
                .and_then(|boon| boon.effects.first().map(|(effect, _)| effect.whole))
                .unwrap_or(0);
            vec![
                plain(format!("{}: {strikes}/s", n(1116))),
                seconds(n(1097), ftol(v.w58())),
            ]
        }
        1018 | 1019 => vec![dmg(v.damages(id == 1018)), seconds(n(1097), ftol(v.w58()))],
        1020 => vec![
            signed_pc(n(1116), per_cent(v.strike())),
            plain(format!(
                "{}: {:2.2}",
                k("TOOLTIP_RUECKWURF_HELF"),
                f64::from(0.02_f32) * v.w58()
            )),
            plain(format!("{}: +{}%", n(1104), v.w60() - 100)),
        ],
        // The changeling's night form as long as it lasts at this time of the day, doubled but
        // by day (`opensacred_rules::arts::changeling_seconds`), where the original's line shows
        // the day's alone (#1742).
        1022 | 1023 => {
            let w = v.weapon?;
            vec![seconds(
                n(1097),
                ftol(f64::from(arts::changeling_seconds(w, v.level, v.hero.day))),
            )]
        }
        1024 | 1025 | 1031 => vec![seconds(
            n(1097),
            ftol(if id == 1031 { v.w58() } else { v.w50() }),
        )],
        1026 | 1027 => {
            let level = float(ftol(f64::from(0.4_f32) * f64::from(v.hero.level)));
            let per = if v.hero.day { 2.5 } else { 3.0 };
            vec![whole(n(1070), ftol(per * v.l() + level))]
        }
        1028 | 1029 => vec![metres(n(1083), f64::from(0.02_f32) * float(v.w60()))],
        1030 => vec![whole(k("TOOLTIP_LEBEN_ABSAUGUNG2"), ftol(v.w50()))],
        1032 => vec![
            metres(n(1104), f64::from(0.02_f32) * v.w50()),
            whole(n(1085), ftol(v.w58())),
        ],
        // The death's kiss: its bleeding a second, the curse of its target's attack and defence,
        // and how long both hold, as the skull gives them (`opensacred_rules::arts::bleed_value`,
        // `curse`, `CURSE_SECONDS`), where the original's line shows the bleeding once, leaves the
        // curse out and gives the skull's flight as the duration (#1742).
        1033 => {
            let w = v.weapon?;
            let bleeds = ftol(f64::from(arts::bleed_value(w, v.level)));
            let curse = ftol((f64::from(arts::curse(w, v.level)) * 100.0).round());
            vec![
                plain(format!("{}: {bleeds}/s", n(1085))),
                signed_pc(n(1166), curse),
                signed_pc(n(1167), curse),
                seconds(n(1097), ftol(f64::from(arts::CURSE_SECONDS))),
            ]
        }
        // The bats that the art keeps, its record's factor (5·L, 2, 2), where the original's
        // line takes constants compiled in, about 1 to 4 (#1741).
        1035..=1037 => vec![whole(
            n(1105),
            v.weapon.map_or(0, |w| {
                i64::from(opensacred_rules::arts::bats_most(w, v.level))
            }),
        )],
        1042 => vec![
            metres(n(1104), f64::from(0.02_f32) * v.w50()),
            seconds(n(1097), ftol(v.w58())),
        ],
        1043 => vec![metres(n(1083), f64::from(0.02_f32) * float(ftol(v.w50())))],
        1044 => vec![plain(format!("{}: +{}", n(1073), v.v(1))), duration()],
        1045 | 1046 => vec![
            pc(k("CHANCE4BLOCK"), v.block(1.0)),
            seconds(n(1097), ftol(v.w50())),
            signed_pc(n(1085), ftol(100.0 * v.w58() - 100.0)),
        ],
        1047 | 1048 => vec![
            seconds(n(1097), v.w60()),
            signed_pc(
                n(if id == 1047 { 1166 } else { 1167 }),
                ftol(100.0 * v.w50()),
            ),
            signed_pc(
                n(if id == 1047 { 1165 } else { 1093 }),
                ftol(100.0 * v.w58()),
            ),
        ],
        1049 | 1050 | 1052 => {
            let g = v.trap();
            let d = v.damages(false);
            let total = ftol(float(d.iter().sum::<i64>()) * g);
            let line = damage_line(n(1085), total, d.map(|d| ftol(float(d) * g)), base);
            match id {
                1049 => vec![line],
                1050 => vec![duration(), whole(n(1070), v.v(1)), line],
                _ => vec![duration(), line],
            }
        }
        1051 | 1053 => vec![duration()],
        1082 if weapon => {
            let berserk = v.weapon.map_or(0, |w| {
                i64::from(w.value_68[0]) * i64::from(v.level) + i64::from(w.value_60[1])
            });
            vec![
                signed(n(1100), ftol(v.w58())),
                signed_pc(k("UI_STAT_BERSERK"), berserk),
                seconds(n(1097), ftol(v.w50())),
            ]
        }
        1083 if weapon => vec![
            signed_pc(n(1116), ftol(100.0 * (v.w58() - 1.0) + 0.5)),
            signed_pc(n(1074), v.attack()),
        ],
        1084 if weapon => vec![
            signed_pc(n(1116), ftol(100.0 * (f64::from(stored(v.w50())) - 1.0))),
            signed_pc(n(1074), v.attack()),
        ],
        // The guns' damages and the flames' seconds as the Dwarf's guns deal them
        // (`opensacred_rules::arts::gun_damage`, `flame_seconds`, `cannon_damages`), where the
        // original's lines leave out the flames' least of 2 s and pair the cannon's damages with
        // their own kinds' multipliers, which its shot crosses (#1741).
        1085..=1087 if weapon => {
            let w = v.weapon?;
            let m = v.hero.multipliers;
            let gun = v.gun();
            let whole = |d: f32| ftol(f64::from(d));
            let [p, f] = match id {
                1085 => [0.0, arts::gun_damage(w.value_58, v.level, m[1], gun)],
                1086 => [
                    arts::gun_damage(w.factor, v.level, m[0], gun),
                    arts::gun_damage(w.value_58, v.level, m[1], gun),
                ],
                _ => {
                    let [p, f, ..] = arts::cannon_damages(w, v.level, m, gun, true);
                    [p, f]
                }
            };
            let (p, f) = (whole(p), whole(f));
            let mut lines = vec![damage_line(n(1085), p + f, [p, f, 0, 0], base)];
            if id == 1085 {
                lines.push(seconds(
                    k("UI_STAT_BURN_TIME"),
                    i64::from(arts::flame_seconds(w, v.level)),
                ));
            }
            lines
        }
        // The allies take half of each effect, the attack's of `W50`, and the time is
        // `W60/100` seconds, where the original's line takes the attack's half of `W58` and the
        // float 0.01 a little short, a whole time one short (#1741).
        1089 if weapon => vec![
            plain(format!(
                "{}: {:+}% ({}: {:+}%)",
                n(1166),
                ftol(100.0 * v.w50()),
                k("UI_STAT_ON_ALLIES"),
                ftol(50.0 * v.w50())
            )),
            plain(format!(
                "{}: {:+}% ({}: {:+}%)",
                n(1167),
                ftol(100.0 * v.w58()),
                k("UI_STAT_ON_ALLIES"),
                ftol(50.0 * v.w58())
            )),
            seconds(n(1097), ftol(float(v.w60()) / 100.0)),
        ],
        1094 if weapon => vec![
            signed_pc(n(1167), ftol(100.0 * v.w58())),
            signed_pc(n(1098), v.w60() - 100),
            seconds(n(1097), ftol(v.w50().min(180.0))),
        ],
        // The effects 1 and 3 of `W68/100` and `W60/100`, in per cent their whole `W68` and
        // `W60`, where the original's line takes the float 0.01 a little short, 19 for 20 (#1741).
        1097 if weapon => vec![
            signed_pc(n(1166), v.w68()),
            plain(format!("{} {:+}%", k("UI_PHY_DMG"), v.w60())),
            plain(format!(
                "{} {:+}",
                k("INVENTAR_RES_PHYSICAL"),
                ftol(v.w58())
            )),
            whole(n(1097), ftol(v.w50().min(180.0))),
        ],
        1098..=1100 if weapon => {
            let (redirect, resistance) = match id {
                1098 => ("UI_REDIRECT_PHY2FIRE", "INVENTAR_RES_FIRE"),
                1099 => ("UI_REDIRECT_PHY2POISON", "INVENTAR_RES_POISON"),
                _ => ("UI_REDIRECT_PHY2MAGIC", "INVENTAR_RES_MAGIC"),
            };
            let kept = v.weapon.map_or(0.0, |w| {
                (f64::from(0.001_f32) * f64::from(w.value_68[1]))
                    .powi(i32::try_from(v.level).unwrap_or(i32::MAX))
            });
            vec![
                pc(k(redirect), ftol(100.0 * (1.0 - kept))),
                plain(format!("{} {:+}", k(resistance), ftol(v.w58()))),
                seconds(n(1097), ftol(v.w50().min(180.0))),
            ]
        }
        // The Daemon's blows as his objects take them, whole (`opensacred_rules::arts::
        // ring_poison`, `fan_magic`, `daemon_ratio`), where the original's lines round them, one
        // above them at times (#1741).
        1101 | 1102 if weapon => {
            let w = v.weapon?;
            let kind = if id == 1101 { 3 } else { 1 };
            let (damage, time) = if id == 1101 {
                ("UI_POI_DMG", n(1097))
            } else {
                ("UI_STAT_FEUERHAUCH_DMG", k("UI_STAT_FEUERHAUCH_TIME"))
            };
            vec![
                whole(
                    k(damage),
                    ftol(f64::from(arts::ring_poison(w, v.level, v.daemon(kind)))),
                ),
                seconds(time, i64::from(arts::daemon_ratio(w, v.level))),
            ]
        }
        1103 if weapon => {
            let w = v.weapon?;
            let physical = ftol(f64::from(arts::ring_poison(w, v.level, v.daemon(0))));
            let magic = ftol(f64::from(arts::fan_magic(w, v.level, v.daemon(2))));
            vec![
                damage_line(n(1085), physical + magic, [physical, 0, magic, 0], base),
                whole(
                    k("UI_NUMBER_FLASHS"),
                    2 * i64::from(arts::daemon_ratio(w, v.level)) + 1,
                ),
            ]
        }
        _ => return None,
    };
    Some(lines)
}

/// `10000/x` truncated, of a whole `x` (`0x890C3C`), 0 of 0.
fn ten_thousandth(x: i64) -> i64 {
    if x == 0 { 0 } else { ftol(10_000.0 / float(x)) }
}

/// `x` as the float the original stores it, then widened.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the original stores the product as a float"
)]
fn stored(x: f64) -> f32 {
    x as f32
}

#[cfg(test)]
mod tests {
    use opensacred_rules::arts::{Entry, Table};

    use super::*;

    fn texts(line: &Line) -> String {
        line.0
            .iter()
            .map(|piece| match piece {
                Piece::Text(text, _) => text.clone(),
                Piece::Image(index, _) => format!("[{index}]"),
            })
            .collect()
    }

    fn spell(id: u16) -> Spell {
        Spell {
            id,
            name: 7,
            description: 8,
            short_description: 9,
            icon: String::new(),
            sprites: [0; 3],
            launch: 1,
            regeneration: [10, 2],
            effects: [[0, 0]; 13],
            values: [[0, 0]; 3],
            caps: [0; 3],
            element: 0,
            classes: 1,
            flags: 0,
        }
    }

    fn weapon_art(id: u16) -> WeaponArt {
        WeaponArt {
            id,
            name: 7,
            description: 8,
            short_description: 9,
            icon: String::new(),
            sprites: [0; 3],
            value_3c: 0,
            regeneration: [4.0, 0.5],
            distance: 0,
            launch: 1,
            factor: [1.2, 0.1],
            value_58: [0.0, 0.0],
            value_60: [110, 10],
            value_68: [0, 0],
            classes: 1,
            flags: 0,
        }
    }

    fn entry(table: Table, art: u16, level: u8, time: f32) -> Entry {
        Entry {
            table,
            art,
            level,
            bonus: 0,
            flags: 0,
            time,
            factor: 1.0,
            remaining: 0.0,
        }
    }

    fn shown(art: u16, arts: &Arts, tables: &Tables) -> Vec<String> {
        shown_next(art, arts, tables, false)
    }

    fn shown_next(art: u16, arts: &Arts, tables: &Tables, next: bool) -> Vec<String> {
        shown_halved(art, arts, tables, next, false)
    }

    fn shown_halved(
        art: u16,
        arts: &Arts,
        tables: &Tables,
        next: bool,
        halved: bool,
    ) -> Vec<String> {
        let rules = opensacred_rules::fight::Rules::default();
        let hero = Hero {
            kind: 1,
            level: 4,
            arts,
            speeds: Speeds {
                spells: 2.0,
                weapon_arts: 1.0,
            },
            multipliers: [1.0; KINDS],
            spells: [1.5; KINDS],
            attributes: [10.0; 6],
            skills: &[],
            ranged: false,
            day: true,
            rules: &rules,
            halved,
        };
        let words = Words {
            text: &|name| Some(format!("<{name}>")),
            id: &|id| Some(format!("#{id}")),
        };
        lines(art, &hero, tables, &words, next)
            .iter()
            .map(texts)
            .collect()
    }

    #[test]
    fn the_spells_page_shows_a_held_arts_next_level() {
        let mut fire = spell(5);
        fire.effects[1] = [20, 10];
        let tables = Tables {
            spells: vec![fire],
            weapon_arts: Vec::new(),
        };
        let arts = Arts {
            entries: vec![entry(Table::Spells, 5, 2, 9.0)],
        };
        let lines = shown_next(5, &arts, &tables, true);
        // The level 2's time 4.5 s scaled by the record's 14 s to 16 s, its fire 30 · 1.5.
        assert_eq!(
            lines[5..],
            [
                "<1107>: 4.5s",
                "<1085>: 60 = 0+60+0+0",
                "",
                "<UI_TT_NEXTLEARNEDLEVEL>:",
                "<1107>: 5.1s",
                "<1085>: 75 = 0+75+0+0",
            ]
        );
        // An art not held has no next level.
        // An art not held has no next level, and the page shows its record's time at the level 1
        // (12 s over the speed 2), the bar's at 0.
        let not_held = shown_next(5, &Arts::default(), &tables, true);
        assert_eq!((not_held.len(), not_held[5].as_str()), (7, "<1107>: 6.0s"));
        assert_eq!(shown(5, &Arts::default(), &tables)[5], "<1107>: 5.0s");
    }

    #[test]
    fn a_line_breaks_at_its_first_space_from_its_46th_character() {
        let text = "Aura which increases the attacking capabilities of the hero and her friends.";
        assert_eq!(
            wrap(text, 0),
            [
                "Aura which increases the attacking capabilities",
                "of the hero and her friends."
            ]
        );
        // A colour's code before it counts.
        assert_eq!(wrap("a b", 46), ["a", "b"]);
        assert_eq!(wrap("short", CODE), ["short"]);
    }

    #[test]
    fn a_held_spell_shows_its_name_level_time_and_damages() {
        let mut fire = spell(5);
        fire.effects[1] = [20, 10];
        fire.effects[3] = [4, 0];
        let tables = Tables {
            spells: vec![fire],
            weapon_arts: Vec::new(),
        };
        let arts = Arts {
            entries: vec![entry(Table::Spells, 5, 2, 9.0)],
        };
        assert_eq!(
            shown(5, &arts, &tables),
            [
                "#7",
                "",
                "#9",
                "",
                "<1109> <1070>: 2",
                "<1107>: 4.5s",
                // Fire 20 + 10·2 and poison 4, times 1.5.
                "<1085>: 66 = 0+60+0+6",
            ]
        );
    }

    #[test]
    fn an_art_not_held_shows_its_first_level_in_grey() {
        let tables = Tables {
            spells: Vec::new(),
            weapon_arts: vec![weapon_art(1000)],
        };
        let lines = shown(1000, &Arts::default(), &tables);
        assert_eq!(lines[4], "<1072>: 1");
        // Its time at the level 0 of its record, its strike's 1.3 at 1, its radius and its
        // attack's 1.425 at 1, +42.5 % rounded (#1741).
        assert_eq!(
            lines[5..],
            [
                "<1107>: 4.0s",
                "<1116>: +30%",
                "<1104>: +20%",
                "<1074>: +43%"
            ]
        );
    }

    #[test]
    fn a_spells_lines_tell_every_effect_its_objects_give() {
        // The mana's spring: both arts' regeneration speeds of (95 + 5·L)/100, and the shield
        // wall's physical resistance beside the others (#1742).
        let mut spring = spell(31);
        spring.values[0] = [70, 2];
        spring.effects[11] = [95, 5];
        let mut wall = spell(30);
        wall.values[0] = [44, 1];
        wall.effects[6] = [4, 6];
        wall.effects[7] = [25, 40];
        let tables = Tables {
            spells: vec![spring, wall],
            weapon_arts: Vec::new(),
        };
        let arts = Arts {
            entries: vec![
                entry(Table::Spells, 31, 1, 5.0),
                entry(Table::Spells, 30, 1, 5.0),
            ],
        };
        let lines = shown(31, &arts, &tables);
        assert_eq!(
            lines[lines.len() - 3..],
            ["<1097>: 72s", "<1148>: +100%", "<1149>: +100%"]
        );
        let lines = shown(30, &arts, &tables);
        assert_eq!(
            lines[lines.len() - 2..],
            ["<INVENTAR_RES_PHYSICAL>: +10", "<INVENTAR_RES_FIRE>: +65"]
        );
    }

    #[test]
    fn an_art_of_no_case_shows_its_regeneration() {
        let tables = Tables {
            spells: vec![spell(43)],
            weapon_arts: Vec::new(),
        };
        let arts = Arts {
            entries: vec![entry(Table::Spells, 43, 1, 5.0)],
        };
        let lines = shown(43, &arts, &tables);
        assert_eq!(lines.last().map(String::as_str), Some("<1107>: 2.5s"));
    }

    #[test]
    fn two_weapons_halve_the_strikes_factor_on_the_line() {
        let tables = Tables {
            spells: Vec::new(),
            weapon_arts: vec![weapon_art(1001)],
        };
        let arts = Arts {
            entries: vec![entry(Table::WeaponArts, 1001, 1, 5.0)],
        };
        // Its strike's 1.3 at the level 1: +30 % with one weapon, its half 0.65, −35 %, with two
        // (#1742), as the strike carries it.
        let one = shown_halved(1001, &arts, &tables, false, false);
        let two = shown_halved(1001, &arts, &tables, false, true);
        assert!(one.contains(&"<1116>: +30%".to_owned()), "{one:?}");
        assert!(two.contains(&"<1116>: -35%".to_owned()), "{two:?}");
    }

    #[test]
    fn the_attack_bonus_tells_the_strikes_factor() {
        let tables = Tables {
            spells: Vec::new(),
            weapon_arts: vec![weapon_art(1001)],
        };
        let arts = Arts {
            entries: vec![entry(Table::WeaponArts, 1001, 2, 5.0)],
        };
        // The strike's 1.3 + 0.125·2 (#1741), where the original's float 1.2999999523 shows 54.
        let lines = shown(1001, &arts, &tables);
        assert_eq!(lines.last().map(String::as_str), Some("<1074>: +55%"));
    }

    #[test]
    fn the_flags_name_where_an_art_may_not_be_made() {
        assert_eq!(restriction(Some(1), None), Some("UI_HORSE_CANTCAST"));
        assert_eq!(restriction(Some(2), None), Some("UI_HORSE_ONLYWITHBOW"));
        assert_eq!(restriction(None, Some(5)), Some("UI_STAT_2H_ONLY"));
        assert_eq!(restriction(Some(0), Some(0)), None);
        // A negative share rounds to the nearest (#1741), where the original's `_ftol` gives −19.
        assert_eq!(per_cent(0.8_f32.into()), -20);
        assert_eq!(per_cent(1.3), 30);
    }
}
