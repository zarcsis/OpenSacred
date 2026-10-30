//! The statistics window beside the inventory (#1521; docs/re/character.md): the original's
//! `UI_WND_STATS_CHAR` (`cUI_StatisticsChar`, `0x6A5D70`) at (656, 0), 256 × 388, shown with the
//! inventory (`0x6C5A50`), read here: its two backgrounds and its frame (`0x6AC470`), the hero's
//! name and level, the header bar that turns its upper panel from the skills to the attributes
//! and back, the skills with their levels and the points to spend (`0x6A9790`) or the attributes,
//! the experience and the next level's (`0x6A8DF0`), the health, the experience still to gain,
//! `A` and `D` with the hit chance against the last foe and its portrait, the weapon's damage
//! raised by the survival bonus with its kinds' icons, the resistances with theirs, and the gold
//! (`0x6A9F20`); the "+" buttons while points wait, which spend them (`0x6A6110`, #1522): an
//! attribute point on an attribute, a skill point on a skill below the hero's level, or the
//! learning mode's list of the skills to learn, one of which a "+" learns.
//!
//! Not followed yet: the combat arts' damage, the panels and the tooltips that the hovered lines
//! show, the button that closes the inventory, the bonus of a set's pieces in the survival bonus,
//! the items' bonuses of the skills, the player's own name, the window's following a follower's
//! portrait, and the gold dropped by a click on it.

use bevy::prelude::*;

use crate::arts::HeroArts;
use crate::bar::{self, UiImages};
use crate::effects::Effects;
use crate::fight::{Base, Combat, Experience, Health, Holding, Regenerates, Rules};
use crate::game::{AppState, FONT};
use crate::hero::{Body, Hero};
use crate::hud::Pointer;
use crate::inventory::{Shown, screen_point};
use crate::lines::Texts;
use crate::pickup::Pack;
use crate::tooltip::argb;

/// The window on the original's screen (`0x6A5D70`).
pub(crate) const WINDOW: Rect = Rect {
    min: Vec2::new(656.0, 0.0),
    max: Vec2::new(912.0, 388.0),
};
/// The backgrounds: each image, its place and its size in the window, at the alpha `0x9F`.
const BACKGROUNDS: [(u32, Vec2, Vec2); 2] = [
    (376, Vec2::new(0.0, 20.0), Vec2::new(256.0, 286.0)),
    (380, Vec2::new(0.0, 306.0), Vec2::new(256.0, 256.0)),
];
const BACKGROUND_TINT: u32 = 0x9FFF_FFFF;
/// The frame: the top edge's pieces, 32 apart from −4, the first's and the last's images and the
/// others', at their heights; the sides' image, its rows from 28 and its places; the last row's
/// images at 348.
const TOP: (u32, u32, u32) = (54, 21, 55);
const TOP_Y: (f32, f32, f32) = (11.0, 17.0, 10.0);
const SIDE: u32 = 34;
const SIDE_ROWS: u8 = 10;
const SIDE_X: (f32, f32) = (-4.0, 255.0);
const LAST_ROW: (u32, u32, f32, f32) = (52, 53, 348.0, 251.0);
/// The button that closes the inventory (`UI_STAT_BTN_BASE`) and the header bar, the skills' and
/// the attributes' images.
const CLOSE: (u32, Vec2) = (132, Vec2::new(242.0, 10.0));
const HEADER: Rect = Rect {
    min: Vec2::new(9.0, 49.0),
    max: Vec2::new(212.0, 70.0),
};
const HEADER_SKILLS: u32 = 1605;
const HEADER_ATTRIBUTES: u32 = 1604;
/// The "+" buttons' image and their places, 19 apart.
const PLUS: u32 = 110;
const PLUS_AT: Vec2 = Vec2::new(232.0, 72.0);
/// The separators' heights and colour.
const SEPARATORS: [f32; 5] = [48.0, 222.0, 263.0, 305.0, 365.0];
const SEPARATOR: u32 = 0xFFAC_8A67;
/// The lists' rows' step, the font's size (**estimate**, as the tooltip's).
const ROW: f32 = 19.0;
const FONT_SIZE: f32 = 20.0;
/// The colours: the window's, the name's, the headers' and their points', a skill above its
/// base, at the hero's level, the skills to learn.
const WHITE: u32 = 0xFFFF_FFFF;
const NAME: u32 = 0xFF00_EF00;
const HEADER_TEXT: u32 = 0xFFFF_FF80;
const RAISED: u32 = 0xFF41_A4FF;
const AT_LEVEL: u32 = 0xFFFF_6347;
const TO_LEARN: u32 = 0xFFFF_FF00;
/// The kinds' icons of the damage and of the resistances, their places, and the portrait's box.
const DAMAGE_ICONS: (u32, Vec2) = (205, Vec2::new(154.0, 308.0));
const RESISTANCE_ICONS: (u32, Vec2) = (209, Vec2::new(154.0, 347.0));
const PORTRAIT: Rect = Rect {
    min: Vec2::new(148.0, 264.0),
    max: Vec2::new(188.0, 304.0),
};

/// The last foe the hero struck (the player's statistics' record, `0x424700`; **hypothesis**:
/// the foe the hero struck last): its defence gives the window's hit chance, its type the
/// portrait, and the whole the foe's panel.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct LastFoe(pub Option<Foe>);

/// A foe as the player's statistics keep it (`0x424250`, a record of `0x24` bytes of the 16 at
/// `+0x30A4`): its type, level and health, its attack and defence, its damage (`0x564800`: each
/// kind's most, or its least where the most is 0) and its resistances' sum (`0x553A00`), and each
/// kind's share of those times 255 (`0x564470`, `0x564740`), the icons' alphas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Foe {
    pub creature: u32,
    pub level: u32,
    pub health: u32,
    pub attack: f32,
    pub defence: f32,
    pub damage: f32,
    pub resistance: f32,
    pub damage_shares: [u32; 4],
    pub resistance_shares: [u32; 4],
}

impl Foe {
    /// The record of a foe of the type `creature`, of `health` and `combat`, as the hero kills it:
    /// its most health (**seen**).
    pub(crate) fn of(
        creature: u32,
        health: &Health,
        combat: &opensacred_rules::fight::Combat,
    ) -> Self {
        let damages = combat
            .damage
            .map(|[least, most]| if most <= 0.001 { least } else { most });
        Self {
            creature,
            level: health.level,
            health: health.most,
            attack: combat.attack,
            defence: combat.defence,
            damage: damages.iter().sum(),
            resistance: combat.resistance.iter().sum(),
            damage_shares: of_total(damages),
            resistance_shares: of_total(combat.resistance),
        }
    }
}

/// The shares of `values` of their sum, `⌊v/s·255⌋` (`0x564470`, the float 255 at `0x890908`).
fn of_total(values: [f32; 4]) -> [u32; 4] {
    let sum: f32 = values.iter().sum();
    values.map(|v| {
        if sum <= 0.0 {
            return 0;
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a share of 0 to 255"
        )]
        let share = (v / sum * 255.0).clamp(0.0, 255.0) as u32;
        share
    })
}

/// A line of the lower panel under the mouse (`0x6AA169`, its zones over x 2 to 248, both edges
/// in): C the health's and the next level's, D the attack's and the defence's, E the weapon's
/// damage, F the art's, G the resistances'.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Block {
    C,
    D,
    E,
    F,
    G,
}

/// What the mouse is over in the window: a line of the lower panel, or a row of the upper panel's
/// list, a skill's name or its level, an attribute, or a skill to learn (`0x6A72F0`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Hover {
    #[default]
    None,
    Block(Block),
    Attribute(u8),
    SkillName(u8),
    SkillLevel(u8),
    Learn(u8),
}

/// The hovered fills' colour (`0x6A72F0`, an untextured quad).
const FILL: u32 = 0x60FF_FF80;
/// The upper panel's fill while a line of the lower panel is under the mouse.
const UPPER_FILL: Rect = Rect {
    min: Vec2::new(3.0, 50.0),
    max: Vec2::new(254.0, 221.0),
};

/// What the mouse at `at`, in the window's pixels, is over of `sheet`'s window.
fn hover_at(at: Vec2, sheet: &Sheet) -> Hover {
    let within =
        |x0: f32, x1: f32, y0: f32, y1: f32| (x0..=x1).contains(&at.x) && (y0..=y1).contains(&at.y);
    for (block, (y0, y1)) in [
        (Block::C, (222.0, 263.0)),
        (Block::D, (264.0, 305.0)),
        (Block::E, (306.0, 325.0)),
        (Block::F, (326.0, 346.0)),
        (Block::G, (347.0, 366.0)),
    ] {
        if within(2.0, 248.0, y0, y1) {
            return Hover::Block(block);
        }
    }
    let rows = |count: usize| (0..count).filter_map(|i| u8::try_from(i).ok());
    if sheet.learning && !sheet.attributes {
        let shown = if sheet.to_learn > 0 {
            sheet.candidates.len()
        } else {
            0
        };
        return rows(shown)
            .find(|&i| {
                let y = 67.0 + ROW * f32::from(i);
                within(8.0, 252.0, y, y + 18.0)
            })
            .map_or(Hover::None, Hover::Learn);
    }
    if sheet.attributes {
        let width = if sheet.attribute_points > 0 {
            240.0
        } else {
            220.0
        };
        return rows(sheet.attribute_rows.len())
            .find(|&k| {
                let y = 67.0 + ROW * f32::from(k);
                within(8.0, 8.0 + width, y, y + 16.0)
            })
            .map_or(Hover::None, Hover::Attribute);
    }
    for i in rows(sheet.skills.len()) {
        let y = 69.0 + ROW * f32::from(i);
        if within(8.0, 188.0, y, y + 18.0) {
            return Hover::SkillName(i);
        }
        if within(189.0, 248.0, y, y + 18.0) {
            return Hover::SkillLevel(i);
        }
    }
    Hover::None
}

/// The fills that the hover `hover` of `sheet` draws, in the window's pixels.
fn fills(hover: Hover, sheet: &Sheet) -> Vec<Rect> {
    let rect = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::new(x0, y0, x1, y1);
    match hover {
        Hover::None => Vec::new(),
        Hover::Block(block) => {
            let lower = match block {
                Block::C => rect(3.0, 224.0, 254.0, 262.0),
                Block::D => rect(3.0, 265.0, 254.0, 304.0),
                Block::E => rect(3.0, 307.0, 254.0, 326.0),
                Block::F => rect(3.0, 326.0, 254.0, 346.0),
                Block::G => rect(3.0, 346.0, 254.0, 365.0),
            };
            vec![UPPER_FILL, lower]
        }
        Hover::Attribute(k) => {
            let y = 67.0 + ROW * f32::from(k);
            let width = if sheet.attribute_points > 0 {
                240.0
            } else {
                220.0
            };
            vec![rect(6.0, y + 4.0, 12.0 + width, y + 21.0)]
        }
        Hover::SkillName(i) => {
            let y = 69.0 + ROW * f32::from(i);
            vec![rect(6.0, y + 2.0, 188.0, y + 19.0)]
        }
        Hover::SkillLevel(i) => {
            let y = 69.0 + ROW * f32::from(i);
            vec![rect(189.0, y + 2.0, 230.0, y + 19.0)]
        }
        Hover::Learn(i) => {
            let y = 67.0 + ROW * f32::from(i);
            vec![rect(6.0, y, 228.0, y + 24.0)]
        }
    }
}

/// The last foe's portrait in its panel, stretched (`0x6A7390`).
const FOE_PORTRAIT: Rect = Rect {
    min: Vec2::new(8.0, 56.0),
    max: Vec2::new(72.0, 120.0),
};
/// The panels' first line's colour, the last foe's header's.
const PANEL_HEADER: u32 = 0xFFFF_FF80;

/// A row of kinds' icons that the panel draws: the first image, its place (the image 1 lower),
/// and each kind's alpha, each the next to the left of the last by its width less 4 (`0x6A7140`).
type Icons = (u32, Vec2, [u32; 4]);

/// The panel that takes the upper panel's place while the line `block` of the lower panel is under
/// the mouse (`0x6AA7D2`): the last foe's (D, `0x6A7390`), the weapon's kinds (E, `0x6A7C60`) or
/// the resistances' (G, `0x6A8A00`), and below the weapon's and the resistances' the four more
/// lines (`0x6AAA6E`); its texts by `write` and its icons into `icons`.
fn upper_panel(
    sheet: &Sheet,
    block: Option<Block>,
    write: &mut impl FnMut(&str, Vec2, f32, (u32, JustifyContent)),
    icons: &mut Vec<Icons>,
) {
    let (left, right) = (JustifyContent::FlexStart, JustifyContent::FlexEnd);
    let row = |i: u8| 48.0 + ROW * f32::from(i);
    let kind_texts = ["1078", "1079", "1080", "1081"];
    match block {
        Some(Block::D) => {
            let foe = &sheet.foe_panel;
            write(
                said(sheet, "1120"),
                Vec2::new(78.0, row(0)),
                240.0,
                (PANEL_HEADER, left),
            );
            write(&foe.name, Vec2::new(78.0, row(1)), 170.0, (WHITE, left));
            write(
                said(sheet, "1070"),
                Vec2::new(78.0, row(2)),
                170.0,
                (WHITE, left),
            );
            write(
                &foe.level.to_string(),
                Vec2::new(78.0, row(2)),
                170.0,
                (WHITE, right),
            );
            write(
                said(sheet, "1073"),
                Vec2::new(78.0, row(3)),
                170.0,
                (WHITE, left),
            );
            write(
                &foe.health.to_string(),
                Vec2::new(78.0, row(3)),
                170.0,
                (WHITE, right),
            );
            for (i, name, chance) in [(4, "1121", foe.hit), (5, "1122", foe.its_hit)] {
                write(
                    said(sheet, name),
                    Vec2::new(8.0, row(i)),
                    240.0,
                    (WHITE, left),
                );
                write(
                    &format!("{chance}%"),
                    Vec2::new(8.0, row(i)),
                    240.0,
                    (WHITE, right),
                );
            }
            write(
                said(sheet, "1076"),
                Vec2::new(8.0, row(7)),
                240.0,
                (WHITE, left),
            );
            write(
                &foe.damage.to_string(),
                Vec2::new(8.0, row(7)),
                240.0,
                (WHITE, right),
            );
            icons.push((
                DAMAGE_ICONS.0,
                Vec2::new(154.0, row(7) + 1.0),
                foe.damage_shares,
            ));
            write(
                said(sheet, "1077"),
                Vec2::new(8.0, row(8)),
                240.0,
                (WHITE, left),
            );
            write(
                &foe.resistance.to_string(),
                Vec2::new(8.0, row(8)),
                240.0,
                (WHITE, right),
            );
            icons.push((
                RESISTANCE_ICONS.0,
                Vec2::new(154.0, row(8) + 1.0),
                foe.resistance_shares,
            ));
        }
        Some(Block::E) => {
            write(
                said(sheet, "1087"),
                Vec2::new(8.0, row(0)),
                240.0,
                (WHITE, left),
            );
            let total = format!("{}-{}", sheet.weapon.0, sheet.weapon.1);
            write(&total, Vec2::new(8.0, row(0)), 240.0, (WHITE, right));
            for (k, (name, ends)) in (0_u8..).zip(kind_texts.iter().zip(sheet.kinds)) {
                let y = row(k + 1);
                write(said(sheet, name), Vec2::new(24.0, y), 216.0, (WHITE, left));
                let value = format!("{}-{}", ends.0, ends.1);
                write(&value, Vec2::new(8.0, y), 240.0, (WHITE, right));
                icons.push((
                    DAMAGE_ICONS.0 + u32::from(k),
                    Vec2::new(4.0, y + 2.0),
                    [255, 0, 0, 0],
                ));
            }
            more_lines(sheet, sheet.bonuses.0, "1152", write);
        }
        Some(Block::G) => {
            write(
                said(sheet, "1077"),
                Vec2::new(8.0, row(0)),
                240.0,
                (WHITE, left),
            );
            let total: i64 = sheet.resistances.iter().sum();
            write(
                &total.to_string(),
                Vec2::new(8.0, row(0)),
                240.0,
                (WHITE, right),
            );
            for (k, (name, value)) in (0_u8..).zip(kind_texts.iter().zip(sheet.resistances)) {
                let y = row(k + 1);
                write(said(sheet, name), Vec2::new(24.0, y), 216.0, (WHITE, left));
                write(&value.to_string(), Vec2::new(8.0, y), 240.0, (WHITE, right));
                icons.push((
                    RESISTANCE_ICONS.0 + u32::from(k),
                    Vec2::new(4.0, y + 2.0),
                    [255, 0, 0, 0],
                ));
            }
            more_lines(sheet, sheet.bonuses.1, "1153", write);
        }
        _ => {}
    }
}

/// The four more lines under the weapon's and the resistances' panels (`0x6AAA6E`): the bonus
/// `bonus` of the text `name`, the pace, the attack speed and the survival bonus.
fn more_lines(
    sheet: &Sheet,
    bonus: i64,
    name: &str,
    write: &mut impl FnMut(&str, Vec2, f32, (u32, JustifyContent)),
) {
    let (left, right) = (JustifyContent::FlexStart, JustifyContent::FlexEnd);
    let lines = [
        (name, format!("{bonus}%"), 143.0),
        ("1101", sheet.pace.to_string(), 162.0),
        ("1100", sheet.speed.to_string(), 181.0),
        ("1159", format!("{}%", sheet.survival), 200.0),
    ];
    for (label, value, y) in lines {
        write(said(sheet, label), Vec2::new(8.0, y), 240.0, (WHITE, left));
        write(&value, Vec2::new(8.0, y), 240.0, (WHITE, right));
    }
}

/// What the last foe's panel shows (`0x6A7390`), its labels and zeros before a foe is kept.
#[derive(Clone, Debug, Default, PartialEq)]
struct FoePanel {
    name: String,
    level: u32,
    health: u32,
    /// The hero's chance to hit it and its chance to hit the hero, in per cent.
    hit: u32,
    its_hit: u32,
    damage: u32,
    resistance: u32,
    damage_shares: [u32; 4],
    resistance_shares: [u32; 4],
}

/// The damage's bonus in per cent (`0x5642A0`): each kind's most before its multiplier, or its
/// least where the most is 0, against itself times the multiplier.
fn damage_bonus(combat: &Combat) -> i64 {
    let (plain, raised) = combat.damage.iter().zip(combat.multipliers).fold(
        (0.0_f64, 0.0_f64),
        |(plain, raised), (&[least, most], m)| {
            let end = if most <= 0.001 { least } else { most };
            let own = if m > 0.001 {
                f64::from(end) / f64::from(m)
            } else {
                0.0
            };
            (plain + own, raised + own * f64::from(m))
        },
    );
    bonus(plain, raised)
}

/// The resistances' bonus in per cent (`0x564250`): each resistance before its factor against
/// itself times the factor.
fn resistance_bonus(combat: &Combat) -> i64 {
    let (plain, raised) = combat
        .resistance
        .iter()
        .zip(combat.resistance_factors)
        .fold((0.0_f64, 0.0_f64), |(plain, raised), (&r, m)| {
            let own = if m > 0.001 {
                f64::from(r) / f64::from(m)
            } else {
                0.0
            };
            (plain + own, raised + own * f64::from(m))
        });
    bonus(plain, raised)
}

/// The survival bonus's whole per cent, truncated (`%d`).
fn per_cent(sb: f64) -> u32 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a per cent of 0 to 100"
    )]
    let whole = sb.clamp(0.0, 100.0) as u32;
    whole
}

/// `(raised − plain)·100/plain` truncated, 0 where `plain` is 0.
fn bonus(plain: f64, raised: f64) -> i64 {
    if plain <= 0.0 {
        return 0;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a per cent, far below the integers' range"
    )]
    let bonus = ((raised - plain) * 100.0 / plain).trunc() as i64;
    bonus
}

/// Which list the upper panel shows: the attributes where the header bar is pressed.
#[derive(Resource, Default)]
struct Attributes(bool);

/// The learning mode (`+0xF84`): the skills' list gives way to the skills to learn, left as the
/// window shows (`0x6A5FA0`) and as a skill is learnt.
#[derive(Resource, Default)]
struct Learning(bool);

/// The window's container.
#[derive(Component)]
struct StatsWindow;

/// What the window shows, made again as it changes.
#[derive(Clone, Debug, Default, PartialEq)]
struct Sheet {
    name: String,
    level: u32,
    attributes: bool,
    skills: Vec<(String, u32, u32)>,
    skill_points: u32,
    to_learn: u32,
    learning: bool,
    candidates: Vec<String>,
    pluses: Vec<u8>,
    attribute_rows: Vec<(String, u32, bool)>,
    attribute_points: u32,
    experience: u64,
    next: u64,
    health: (u32, u32),
    attack: u32,
    defence: u32,
    hit: u32,
    foe: Option<u32>,
    weapon: (i64, i64),
    damage_shares: [u32; 4],
    resistance: i64,
    resistance_shares: [u32; 4],
    gold: u32,
    texts: Vec<String>,
    /// What the mouse is over, the last foe's panel, each kind's raised damage and resistance,
    /// and the four more lines: the damage's and the
    /// resistances' bonuses, the pace, the attack speed and the survival bonus.
    hover: Hover,
    foe_panel: FoePanel,
    kinds: [(i64, i64); 4],
    resistances: [i64; 4],
    bonuses: (i64, i64),
    pace: u32,
    speed: u32,
    survival: u32,
}

/// The survival bonus of `minutes` played (`0x6AA062`, `0x565D70(0, m, 120, 50)`):
/// `100·(1 − 1/(1 + (m − 1)/120))`, 0 below a minute.
fn survival(minutes: u32) -> f64 {
    if minutes < 1 {
        return 0.0;
    }
    100.0 * (1.0 - 1.0 / (1.0 + (f64::from(minutes) - 1.0) / 120.0))
}

/// A damage `d` raised by the survival bonus `sb` (the balance's `LongivityCombatFactor` 0.5):
/// `⌊d·sb·0.5·0.01 + d + 0.5⌋` stored as a float.
fn raised(d: f32, sb: f64) -> i64 {
    let d = f64::from(d);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a damage, far below the integers' range"
    )]
    let raised = (d * sb * 0.5 * 0.01 + d + 0.5) as f32;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a damage, far below the integers' range"
    )]
    let whole = raised.trunc() as i64;
    whole
}

/// The shares of `values` of the greatest, `⌊v/m·255⌋` at most 255.
fn shares(values: [f32; 4]) -> [u32; 4] {
    let most = values.iter().copied().fold(0.0_f32, f32::max);
    values.map(|v| {
        if most <= 0.0 {
            return 0;
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a share of 0 to 255"
        )]
        let share = (v / most * 255.0).clamp(0.0, 255.0) as u32;
        share
    })
}

/// A whole number of a value, truncated.
fn whole(value: f32) -> u32 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a value of the fight's, far below the integers' range"
    )]
    let whole = value.max(0.0) as u32;
    whole
}

/// What the window shows of the hero now.
fn sheet(
    texts: &Texts,
    name: &str,
    experience: &opensacred_rules::fight::Experience,
    (base, combat, health): (&Base, &Combat, &Health),
    pack: &Pack,
    (foe, minutes): (Option<Foe>, u32),
    (attributes, learning): (bool, bool),
) -> Sheet {
    let text = |name: &str| texts.named(name).unwrap_or_default().to_owned();
    let skill_name = |id: u8| text(&(9399 + u32::from(id)).to_string());
    let level = experience.level;
    // The skills by their slots at their base levels (the items' bonuses are not kept yet).
    let skills: Vec<(String, u32, u32)> = opensacred_rules::fight::skills(base, level, true)
        .into_iter()
        .map(|(id, at)| (skill_name(id), at, at))
        .collect();
    let candidates: Vec<String> = base.learnable_now().into_iter().map(skill_name).collect();
    let pluses = pluses(
        experience,
        (skills.len(), candidates.len()),
        (attributes, learning),
    );
    // An attribute that the items raise above its own (`+0x22`): above the base grown by the level
    // with the points spent on it.
    let attribute_rows = (0..6)
        .map(|k| {
            let own = u32::from(base.attributes[k]);
            let grown = own + own * level.saturating_sub(1) / 10 + u32::from(base.extras[k]);
            let value = whole(combat.attributes[k]);
            (text(&(1090 + k).to_string()), value, value > grown)
        })
        .collect();
    let sb = survival(minutes);
    let (least, most) = combat.damage.iter().fold((0, 0), |(l, m), &[least, most]| {
        (l + raised(least, sb), m + raised(most, sb))
    });
    let resistance = combat
        .resistance
        .iter()
        .map(|&r| i64::from(whole(r + 0.5)))
        .sum();
    let chance = |attack: f32, defence: f32| {
        whole((opensacred_rules::fight::hit_chance(attack, defence) * 100.0).min(100.0))
    };
    let hit = foe.map_or(0, |foe| chance(combat.attack, foe.defence));
    // The last foe's panel: its name its type's text (`0x44A1C0`).
    let foe_panel = foe.map_or_else(FoePanel::default, |foe| FoePanel {
        name: text(&foe.creature.to_string()),
        level: foe.level,
        health: foe.health,
        hit,
        its_hit: chance(foe.attack, combat.defence),
        damage: whole(foe.damage),
        resistance: whole(foe.resistance),
        damage_shares: foe.damage_shares,
        resistance_shares: foe.resistance_shares,
    });
    // Each kind's damage raised by the survival bonus; its ends raised by a half are never 0.0,
    // so that a kind of none shows `0-0` (**seen**), not the `-` of the code.
    let kinds = combat
        .damage
        .map(|[least, most]| (raised(least, sb), raised(most, sb)));
    Sheet {
        name: name.to_owned(),
        level,
        attributes,
        skills,
        skill_points: experience.skill_points,
        to_learn: experience.to_learn(),
        learning,
        candidates,
        pluses,
        attribute_rows,
        attribute_points: experience.attribute_points,
        experience: experience.points,
        next: opensacred_rules::fight::level_end(level),
        health: (health.current, health.most),
        attack: whole(combat.attack),
        defence: whole(combat.defence),
        hit,
        foe: foe.map(|foe| foe.creature),
        weapon: (least, most),
        damage_shares: shares(combat.damage.map(|[_, most]| most)),
        resistance,
        resistance_shares: shares(combat.resistance),
        gold: pack.gold,
        texts: TEXTS
            .iter()
            .chain(&MORE_TEXTS)
            .map(|name| text(name))
            .collect(),
        hover: Hover::None,
        foe_panel,
        kinds,
        resistances: combat.resistance.map(|r| i64::from(whole(r + 0.5))),
        bonuses: (damage_bonus(combat), resistance_bonus(combat)),
        pace: whole(combat.pace),
        speed: whole(combat.speed.min(220.0)),
        survival: per_cent(sb),
    }
}

/// The texts that the window shows, by their names.
const TEXTS: [&str; 26] = [
    "1070", "1096", "1145", "1099", "1071", "1072", "1073", "1119", "1074", "1117", "1075", "1087",
    "1089", "1077", "1082", "1120", "1121", "1122", "1076", "1078", "1079", "1080", "1081", "1152",
    "1153", "1101",
];
/// The texts of the four more lines' last two (the attack speed and the survival bonus).
const MORE_TEXTS: [&str; 2] = ["1100", "1159"];

/// The rows whose "+" the window draws (`0x6A8DF0`, `0x6A9790`), while points wait or the learning
/// mode is on (`0x6AC310`): the attributes' six with attribute points, the learning mode's skills
/// to learn, else the skills held and the first empty slot while skills wait to be learnt.
fn pluses(
    experience: &opensacred_rules::fight::Experience,
    (held, candidates): (usize, usize),
    (attributes, learning): (bool, bool),
) -> Vec<u8> {
    let shown = experience.attribute_points > 0 || experience.skill_points > 0 || learning;
    let rows = if !shown {
        0
    } else if attributes {
        if experience.attribute_points > 0 {
            6
        } else {
            0
        }
    } else if learning {
        if experience.to_learn() > 0 {
            candidates
        } else {
            0
        }
    } else {
        held + usize::from(experience.to_learn() > 0 && held < 8)
    };
    (0..8).take(rows).collect()
}

/// The text of `sheet` of the name `name` among those it keeps.
fn said<'a>(sheet: &'a Sheet, name: &str) -> &'a str {
    TEXTS
        .iter()
        .chain(&MORE_TEXTS)
        .position(|&n| n == name)
        .and_then(|at| sheet.texts.get(at))
        .map_or("", String::as_str)
}

/// The window made again as it shows and as what it shows changes; taken away as the inventory
/// hides.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn show(
    mut commands: Commands,
    shown: Option<Res<Shown>>,
    (images, assets, texts): (Res<UiImages>, Res<AssetServer>, Option<Res<Texts>>),
    (experience, arts, pack, foe, played): (
        Res<Experience>,
        Option<Res<HeroArts>>,
        Res<Pack>,
        Res<LastFoe>,
        Res<crate::drops::Played>,
    ),
    (attributes, learning, portraits): (
        Res<Attributes>,
        Res<Learning>,
        Res<crate::talk::Portraits>,
    ),
    heroes: Query<(&Base, &Combat, &Health), With<Body>>,
    (pointer, ui_scale, screens): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    windows: Query<Entity, With<StatsWindow>>,
    mut last: Local<Option<Sheet>>,
) {
    let on = shown.is_some_and(|shown| shown.0);
    let current = texts
        .as_deref()
        .zip(heroes.iter().next())
        .filter(|_| on)
        .map(|(texts, hero)| {
            // The hero's name, its class's by default (the player's own is not kept yet).
            let class = arts.as_deref().map_or(1, |arts| arts.kind);
            let name = texts.named(&class.to_string()).unwrap_or_default();
            sheet(
                texts,
                name,
                &experience.0,
                hero,
                &pack,
                (foe.0, played.minutes()),
                (attributes.0, learning.0),
            )
        })
        .map(|mut sheet| {
            if let Some(at) = screen_point(&pointer, &ui_scale, &screens) {
                sheet.hover = hover_at(at - WINDOW.min, &sheet);
            }
            sheet
        });
    if *last == current {
        return;
    }
    for window in &windows {
        commands.entity(window).despawn();
    }
    *last = current.clone();
    let Some(sheet) = current else {
        return;
    };
    let font: Handle<Font> = assets.load(FONT);
    let window = commands
        .spawn((
            StatsWindow,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Px(WINDOW.min.y),
                margin: UiRect::left(Val::Px(WINDOW.min.x - bar::SCREEN.x / 2.0)),
                width: Val::Px(WINDOW.width()),
                height: Val::Px(WINDOW.height()),
                ..default()
            },
            Pickable::IGNORE,
            DespawnOnExit(AppState::InGame),
        ))
        .id();
    let add = |bundle: (ImageNode, Node), tint: Option<u32>, commands: &mut Commands| {
        let (mut image, node) = bundle;
        if let Some(tint) = tint {
            image.color = argb(tint);
        }
        let child = commands.spawn((image, node, Pickable::IGNORE)).id();
        commands.entity(window).add_child(child);
    };
    let piece = |index: u32, at: Vec2| bar::piece(&images, &assets, index, at, Vec2::ZERO);
    let sized = |index: u32, at: Vec2, size: Vec2| {
        let (image, mut node) = piece(index, at);
        node.width = Val::Px(size.x);
        node.height = Val::Px(size.y);
        (image, node)
    };
    for (index, at, size) in BACKGROUNDS {
        add(sized(index, at, size), Some(BACKGROUND_TINT), &mut commands);
    }
    // The frame: the top's eight pieces, the sides' ten rows and their last.
    for i in 0..8_u8 {
        let x = -4.0 + 32.0 * f32::from(i);
        let (index, y) = match i {
            0 => (TOP.0, TOP_Y.0),
            7 => (TOP.2, TOP_Y.2),
            _ => (TOP.1, TOP_Y.1),
        };
        add(piece(index, Vec2::new(x, y)), None, &mut commands);
    }
    let side_height = images.size(SIDE).y;
    for row in 0..SIDE_ROWS {
        let y = 28.0 + side_height * f32::from(row);
        for x in [SIDE_X.0, SIDE_X.1] {
            add(piece(SIDE, Vec2::new(x, y)), None, &mut commands);
        }
    }
    add(
        piece(LAST_ROW.0, Vec2::new(SIDE_X.0, LAST_ROW.2)),
        None,
        &mut commands,
    );
    add(
        piece(LAST_ROW.1, Vec2::new(LAST_ROW.3, LAST_ROW.2)),
        None,
        &mut commands,
    );
    // The separators, a pixel across the window.
    for y in SEPARATORS {
        let line = commands
            .spawn((
                bar::placed(Vec2::new(0.0, y), Vec2::new(WINDOW.width(), 1.0)),
                BackgroundColor(argb(SEPARATOR)),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(window).add_child(line);
    }
    // The hovered fills, under what they light (`0x6A72F0`).
    for fill in fills(sheet.hover, &sheet) {
        let quad = commands
            .spawn((
                bar::placed(fill.min, fill.size()),
                BackgroundColor(argb(FILL)),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(window).add_child(quad);
    }
    let block = match sheet.hover {
        Hover::Block(block) => Some(block),
        _ => None,
    };
    // The header bar, under its text, but in the learning mode and under a hovered line of the
    // lower panel (`0x6AA993`).
    if !sheet.learning && block.is_none() {
        add(
            piece(
                if sheet.attributes {
                    HEADER_ATTRIBUTES
                } else {
                    HEADER_SKILLS
                },
                HEADER.min,
            ),
            None,
            &mut commands,
        );
    }
    // A text in its box, at its left, its right or its middle (the font's flags 0, 4 and 1).
    let mut write = |text: &str, at: Vec2, width: f32, (colour, justify): (u32, JustifyContent)| {
        let mut boxed = bar::placed(at, Vec2::new(width, 18.0));
        boxed.flex_direction = FlexDirection::Row;
        boxed.justify_content = justify;
        let child = commands
            .spawn((boxed, Pickable::IGNORE))
            .with_child((
                Text::new(text),
                TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(FONT_SIZE),
                    ..default()
                },
                TextLayout {
                    linebreak: LineBreak::NoWrap,
                    ..default()
                },
                TextColor(argb(colour)),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(window).add_child(child);
    };
    let (left, right, centre) = (
        JustifyContent::FlexStart,
        JustifyContent::FlexEnd,
        JustifyContent::Center,
    );
    write(&sheet.name, Vec2::new(8.0, 24.0), 180.0, (NAME, left));
    write(
        said(&sheet, "1070"),
        Vec2::new(8.0, 24.0),
        188.0,
        (WHITE, right),
    );
    write(
        &sheet.level.to_string(),
        Vec2::new(8.0, 24.0),
        220.0,
        (WHITE, right),
    );
    // The upper panel: the header and its points, then the list; the attributes' under the
    // health's and the next level's lines (**seen**).
    let attributes = sheet.attributes || block == Some(Block::C);
    let (header, points) = if attributes {
        (said(&sheet, "1145"), sheet.attribute_points)
    } else {
        (said(&sheet, "1096"), sheet.skill_points)
    };
    let learning = sheet.learning && !attributes;
    let panel = matches!(block, Some(Block::D | Block::E | Block::G));
    let mut panel_icons: Vec<Icons> = Vec::new();
    if panel {
        upper_panel(&sheet, block, &mut write, &mut panel_icons);
    } else if !learning {
        write(header, Vec2::new(28.0, 49.0), 180.0, (0xA000_0000, centre));
        write(header, Vec2::new(26.0, 47.0), 180.0, (HEADER_TEXT, centre));
        write(
            &points.to_string(),
            Vec2::new(8.0, 48.0),
            236.0,
            (HEADER_TEXT, right),
        );
    }
    if panel {
        // The panel stands in the list's place.
    } else if learning {
        // The learning mode: its header in yellow, the skills to learn below.
        if sheet.to_learn > 0 {
            write(
                said(&sheet, "1099"),
                Vec2::new(8.0, 48.0),
                220.0,
                (TO_LEARN, left),
            );
            for (row, name) in (1_u8..).zip(&sheet.candidates) {
                let y = 48.0 + ROW * f32::from(row);
                write(name, Vec2::new(8.0, y), 220.0, (WHITE, left));
            }
        }
    } else if attributes {
        for (k, (label, value, raised)) in (0_u8..).zip(&sheet.attribute_rows) {
            let y = 67.0 + ROW * f32::from(k);
            write(label, Vec2::new(8.0, y), 220.0, (WHITE, left));
            let colour = if *raised { RAISED } else { WHITE };
            write(
                &value.to_string(),
                Vec2::new(8.0, y),
                220.0,
                (colour, right),
            );
        }
        let rows = [
            (said(&sheet, "1071"), sheet.experience),
            (said(&sheet, "1072"), sheet.next),
        ];
        for ((label, value), y) in rows.into_iter().zip([181.0, 200.0]) {
            write(label, Vec2::new(8.0, y), 220.0, (WHITE, left));
            write(&value.to_string(), Vec2::new(8.0, y), 220.0, (WHITE, right));
        }
    } else {
        let mut row = 0_u8;
        for (name, total, base) in &sheet.skills {
            let y = 69.0 + ROW * f32::from(row);
            write(name, Vec2::new(8.0, y), 180.0, (WHITE, left));
            let colour = if *base >= sheet.level {
                AT_LEVEL
            } else if total > base {
                RAISED
            } else {
                WHITE
            };
            write(
                &total.to_string(),
                Vec2::new(189.0, y),
                39.0,
                (colour, right),
            );
            row += 1;
        }
        if sheet.to_learn > 0 {
            let y = 48.0 + ROW * f32::from(row + 1);
            write(
                said(&sheet, "1099"),
                Vec2::new(8.0, y),
                220.0,
                (TO_LEARN, left),
            );
            write(
                &sheet.to_learn.to_string(),
                Vec2::new(8.0, y),
                220.0,
                (TO_LEARN, right),
            );
        }
    }
    // The lower panel.
    let lines = [
        (
            222.0,
            said(&sheet, "1073"),
            format!("{}/{}", sheet.health.0, sheet.health.1),
        ),
        (
            241.0,
            said(&sheet, "1119"),
            sheet.next.saturating_sub(sheet.experience).to_string(),
        ),
        (
            306.0,
            said(&sheet, "1087"),
            format!("{}-{}", sheet.weapon.0, sheet.weapon.1),
        ),
        (325.0, said(&sheet, "1089"), String::new()),
        (344.0, said(&sheet, "1077"), sheet.resistance.to_string()),
    ];
    for (y, label, value) in lines {
        write(label, Vec2::new(8.0, y), 240.0, (WHITE, left));
        write(&value, Vec2::new(8.0, y), 240.0, (WHITE, right));
    }
    write(
        said(&sheet, "1074"),
        Vec2::new(8.0, 264.0),
        135.0,
        (WHITE, left),
    );
    write(
        &sheet.attack.to_string(),
        Vec2::new(8.0, 264.0),
        135.0,
        (WHITE, right),
    );
    write(
        said(&sheet, "1117"),
        Vec2::new(8.0, 264.0),
        240.0,
        (WHITE, right),
    );
    write(
        said(&sheet, "1075"),
        Vec2::new(8.0, 283.0),
        135.0,
        (WHITE, left),
    );
    write(
        &sheet.defence.to_string(),
        Vec2::new(8.0, 283.0),
        135.0,
        (WHITE, right),
    );
    write(
        &format!("{}%", sheet.hit),
        Vec2::new(8.0, 283.0),
        240.0,
        (WHITE, right),
    );
    write(
        &format!("{} {:8}", said(&sheet, "1082"), sheet.gold),
        Vec2::new(8.0, 365.0),
        240.0,
        (WHITE, centre),
    );
    // The kinds' icons, each the next to the left of the last, at its share's alpha; the panel's
    // its image 1 lower.
    let rows = [
        (DAMAGE_ICONS, sheet.damage_shares),
        (RESISTANCE_ICONS, sheet.resistance_shares),
    ]
    .into_iter()
    .chain(
        panel_icons
            .into_iter()
            .map(|(first, at, alphas)| ((first, at + Vec2::Y), alphas)),
    );
    for ((first, at), alphas) in rows {
        let mut x = at.x;
        for (index, alpha) in (first..).zip(alphas) {
            if alpha == 0 {
                continue;
            }
            add(
                piece(index, Vec2::new(x, at.y)),
                Some((alpha << 24) | 0x00FF_FFFF),
                &mut commands,
            );
            x -= images.size(index).x - 4.0;
        }
    }
    // The last foe's portrait.
    if let Some((file, [x, y, w, h])) = sheet.foe.and_then(|foe| portraits.of(foe)) {
        let rect = Rect::new(
            f32::from(x),
            f32::from(y),
            f32::from(x) + f32::from(w),
            f32::from(y) + f32::from(h),
        );
        let child = commands
            .spawn((
                ImageNode {
                    rect: Some(rect),
                    ..ImageNode::new(crate::hud::image(&assets, file.to_owned()))
                },
                bar::placed(PORTRAIT.min, PORTRAIT.size()),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(window).add_child(child);
    }
    if block == Some(Block::D)
        && let Some((file, [x, y, w, h])) = sheet.foe.and_then(|foe| portraits.of(foe))
    {
        let rect = Rect::new(
            f32::from(x),
            f32::from(y),
            f32::from(x) + f32::from(w),
            f32::from(y) + f32::from(h),
        );
        let child = commands
            .spawn((
                ImageNode {
                    rect: Some(rect),
                    image_mode: NodeImageMode::Stretch,
                    ..ImageNode::new(crate::hud::image(&assets, file.to_owned()))
                },
                bar::placed(FOE_PORTRAIT.min, FOE_PORTRAIT.size()),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(window).add_child(child);
    }
    add(piece(CLOSE.0, CLOSE.1), None, &mut commands);
    for &i in &sheet.pluses {
        add(piece(PLUS, plus_at(i)), None, &mut commands);
    }
}

/// The "+" `i`'s corner in the window.
fn plus_at(i: u8) -> Vec2 {
    PLUS_AT + Vec2::new(0.0, ROW * f32::from(i))
}

/// The hero's values made anew after a point spent or a skill learnt (`0x5796A0(0, 1)`), as its
/// effects' change makes them (`effects::refit_hero`): its attack, defence, damage and
/// resistances, its regeneration, its pace, its most health with its health held below it, and
/// its arts' skills.
fn refit(
    level: u32,
    rules: &Rules,
    (base, holding, effects): (&Base, Option<&Holding>, Option<&Effects>),
    (combat, regenerates, health): (&mut Combat, &mut Regenerates, &mut Health),
    (arts, hero): (Option<&mut HeroArts>, Option<&mut Hero>),
) {
    let skills = opensacred_rules::fight::skills(base, level, true);
    let holds = holding.map(|holding| holding.0.clone()).unwrap_or_default();
    let list = effects.map(Effects::list).unwrap_or_default();
    *combat = Combat(opensacred_rules::fight::combat_with(
        base, level, holds, &skills, &list, rules,
    ));
    regenerates.value =
        opensacred_rules::fight::regeneration_with(base, level, &skills, &list, rules);
    let most = opensacred_rules::fight::hero_health_with(
        base.attributes,
        base.extras,
        level,
        &skills,
        rules,
    );
    health.most = most;
    health.current = health.current.min(most);
    if let Some(hero) = hero {
        hero.pace = combat.pace_factor();
    }
    if let Some(arts) = arts {
        arts.skills = skills;
    }
}

/// A left click on a shown "+" spends a point (`0x6A6110`): in the attributes' list with attribute
/// points it raises the attribute (`0x580E90`); in the skills' list, on a slot that holds a skill
/// with skill points left, it raises the skill where its base is below the hero's level
/// (`0x5824A0`), else, with skills to learn, it turns the learning mode on; in the learning mode
/// it learns the skill of its row into the first empty slot (`0x581070`, `0x581120`) and leaves
/// the mode. Any stops the taskbar's "+"'s glow (its message 7 with 2). The sounds it asks for,
/// (9, `0x13`) and (9, `0x14`), play none (docs/re/sound.md).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn spend(
    buttons: Res<ButtonInput<MouseButton>>,
    shown: Option<Res<Shown>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    (mut experience, mut learning, attributes, rules): (
        ResMut<Experience>,
        ResMut<Learning>,
        Res<Attributes>,
        Res<Rules>,
    ),
    (mut plus, mut arts, mut hero): (
        ResMut<crate::points::Plus>,
        Option<ResMut<HeroArts>>,
        Option<ResMut<Hero>>,
    ),
    mut heroes: Query<
        (
            &mut Base,
            Option<&Holding>,
            Option<&Effects>,
            &mut Combat,
            &mut Regenerates,
            &mut Health,
        ),
        With<Body>,
    >,
) {
    if !shown.is_some_and(|shown| shown.0) || !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let points = experience.attribute_points > 0 || experience.skill_points > 0;
    if !(points || learning.0) {
        return;
    }
    let Some(at) = screen_point(&pointer, &ui_scale, &windows) else {
        return;
    };
    let Some(i) = (0_u8..8).find(|&i| {
        let corner = WINDOW.min + plus_at(i);
        Rect::from_corners(corner, corner + Vec2::splat(16.0)).contains(at)
    }) else {
        return;
    };
    plus.glowing = false;
    let Some((mut base, holding, effects, mut combat, mut regenerates, mut health)) =
        heroes.iter_mut().next()
    else {
        return;
    };
    let slot = usize::from(i);
    let level = experience.level;
    let spent = if experience.attribute_points > 0 && i <= 5 && attributes.0 {
        base.0.raise_attribute(slot, &mut experience.0)
    } else if !learning.0 {
        if attributes.0 {
            false
        } else if base.skills[slot] != 0 && experience.skill_points > 0 {
            base.0.raise_skill(slot, level, &mut experience.0)
        } else {
            if experience.to_learn() > 0 {
                learning.0 = true;
            }
            false
        }
    } else if !attributes.0 {
        learning.0 = false;
        base.learnable_now()
            .get(slot)
            .is_some_and(|&skill| base.0.learn(skill, &mut experience.0))
    } else {
        false
    };
    if !spent {
        return;
    }
    info!(
        "statistics: a point spent: attributes {:?}, skills {:?} at {:?}, points left {} and {}, \
         {} to learn",
        base.extras,
        base.skills,
        base.levels,
        experience.attribute_points,
        experience.skill_points,
        experience.to_learn()
    );
    refit(
        level,
        &rules,
        (&base, holding, effects),
        (&mut combat, &mut regenerates, &mut health),
        (arts.as_deref_mut(), hero.as_deref_mut()),
    );
}

/// The learning mode left as the window shows (`0x6A5FA0`).
fn leave_learning(shown: Option<Res<Shown>>, mut learning: ResMut<Learning>) {
    if shown.is_some_and(|shown| shown.is_changed() && shown.0) {
        learning.0 = false;
    }
}

/// A left click on the header bar turns the upper panel between the skills and the attributes
/// (its flag `0x10`, `0x72B560`).
fn toggle(
    buttons: Res<ButtonInput<MouseButton>>,
    shown: Option<Res<Shown>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    mut attributes: ResMut<Attributes>,
) {
    if !shown.is_some_and(|shown| shown.0) || !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(at) = screen_point(&pointer, &ui_scale, &windows) else {
        return;
    };
    let header = Rect::from_corners(WINDOW.min + HEADER.min, WINDOW.min + HEADER.max);
    if header.contains(at) {
        attributes.0 = !attributes.0;
    }
}

/// A left click on the close button (`UI_STAT_BTN_BASE`, `cCommand_onInvClose`, `0x756870`)
/// closes the inventory's windows, with their sound.
fn close(
    buttons: Res<ButtonInput<MouseButton>>,
    shown: Option<ResMut<Shown>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    images: Res<UiImages>,
    mut sounds: MessageWriter<crate::sfx::Play>,
) {
    let Some(mut shown) = shown.filter(|shown| shown.0) else {
        return;
    };
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let corner = WINDOW.min + CLOSE.1;
    let button = Rect::from_corners(corner, corner + images.size(CLOSE.0));
    if screen_point(&pointer, &ui_scale, &windows).is_some_and(|at| button.contains(at)) {
        shown.0 = false;
        sounds.write(crate::sfx::Play {
            sound: crate::sfx::Of::Id(crate::inventory::CLOSES),
            at: None,
            rate: (0.5, crate::sfx::Roll::None),
        });
    }
}

/// The statistics window.
pub struct StatisticsPlugin;

impl Plugin for StatisticsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Attributes>()
            .init_resource::<Learning>()
            .init_resource::<LastFoe>()
            .add_systems(
                Update,
                (leave_learning, close, toggle, spend, show)
                    .chain()
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mouse_finds_its_line_or_row() {
        let sheet = Sheet {
            skills: vec![("a".into(), 3, 3), ("b".into(), 1, 1)],
            ..Sheet::default()
        };
        let at = |x: f32, y: f32| hover_at(Vec2::new(x, y), &sheet);
        // The lower panel's lines, their zones' edges in.
        assert_eq!(at(2.0, 222.0), Hover::Block(Block::C));
        assert_eq!(at(248.0, 305.0), Hover::Block(Block::D));
        assert_eq!(at(100.0, 306.0), Hover::Block(Block::E));
        assert_eq!(at(100.0, 346.0), Hover::Block(Block::F));
        assert_eq!(at(100.0, 366.0), Hover::Block(Block::G));
        assert_eq!(at(1.0, 300.0), Hover::None);
        // The skills' names and levels, the second slot's at 88.
        assert_eq!(at(20.0, 70.0), Hover::SkillName(0));
        assert_eq!(at(200.0, 90.0), Hover::SkillLevel(1));
        assert_eq!(at(200.0, 120.0), Hover::None);
        // A hovered line lights the upper panel and itself.
        assert_eq!(
            fills(Hover::Block(Block::E), &sheet),
            [UPPER_FILL, Rect::new(3.0, 307.0, 254.0, 326.0)]
        );
        assert_eq!(
            fills(Hover::SkillLevel(1), &sheet),
            [Rect::new(189.0, 90.0, 230.0, 107.0)]
        );
        let attributes = Sheet {
            attributes: true,
            attribute_rows: vec![("s".into(), 10, false); 6],
            ..Sheet::default()
        };
        assert_eq!(
            hover_at(Vec2::new(200.0, 86.0), &attributes),
            Hover::Attribute(1)
        );
        assert_eq!(hover_at(Vec2::new(240.0, 86.0), &attributes), Hover::None);
    }

    #[test]
    fn a_foes_kinds_take_their_shares_of_the_sum() {
        assert_eq!(of_total([30.0, 10.0, 0.0, 0.0]), [191, 63, 0, 0]);
        assert_eq!(of_total([0.0; 4]), [0; 4]);
        // The bonus of a multiplier of 1.5 on one kind and 1 on another of the same own value.
        assert_eq!(bonus(20.0, 25.0), 25);
        assert_eq!(bonus(0.0, 5.0), 0);
        assert_eq!(bonus(20.0, 19.9), 0);
    }

    #[test]
    fn the_survival_bonus_grows_with_the_minutes_played() {
        assert!(survival(0).abs() < 1e-9);
        assert!(survival(1).abs() < 1e-9);
        assert!((survival(121) - 50.0).abs() < 1e-9);
        // A damage of 30 after 121 minutes: 30·50·0.5·0.01 + 30 + 0.5.
        assert_eq!(raised(30.0, survival(121)), 38);
        assert_eq!(raised(30.0, 0.0), 30);
    }

    #[test]
    fn the_pluses_follow_the_points_and_the_list() {
        let rules = opensacred_rules::fight::Rules::default();
        let mut experience = opensacred_rules::fight::Experience::default();
        assert!(pluses(&experience, (2, 6), (false, false)).is_empty());
        // Level 3: attribute points, skill points and a skill to learn.
        experience.add(1200, &rules);
        assert_eq!(
            pluses(&experience, (2, 6), (true, false)),
            [0, 1, 2, 3, 4, 5]
        );
        assert_eq!(pluses(&experience, (2, 6), (false, false)), [0, 1, 2]);
        assert_eq!(
            pluses(&experience, (2, 6), (false, true)),
            [0, 1, 2, 3, 4, 5]
        );
        experience.attribute_points = 0;
        assert!(pluses(&experience, (2, 6), (true, false)).is_empty());
    }

    #[test]
    fn the_shares_are_of_the_greatest() {
        assert_eq!(shares([3.0, 2.0, 0.0, 0.0]), [255, 170, 0, 0]);
        assert_eq!(shares([0.0; 4]), [0; 4]);
    }
}
