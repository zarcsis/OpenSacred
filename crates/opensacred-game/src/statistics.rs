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

/// The last foe the hero struck: its defence, against which the window gives the hero's hit
/// chance, and its type, whose portrait it shows (the player's statistics' record, `0x424700`;
/// **hypothesis**: the foe the hero struck last).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct LastFoe(pub Option<(f32, u32)>);

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
    (foe, minutes): (Option<(f32, u32)>, u32),
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
    let hit = foe.map_or(0, |(defence, _)| {
        whole((opensacred_rules::fight::hit_chance(combat.attack, defence) * 100.0).min(100.0))
    });
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
        foe: foe.map(|(_, creature)| creature),
        weapon: (least, most),
        damage_shares: shares(combat.damage.map(|[_, most]| most)),
        resistance,
        resistance_shares: shares(combat.resistance),
        gold: pack.gold,
        texts: [
            "1070", "1096", "1145", "1099", "1071", "1072", "1073", "1119", "1074", "1117", "1075",
            "1087", "1089", "1077", "1082",
        ]
        .map(text)
        .to_vec(),
    }
}

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
    const NAMES: [&str; 15] = [
        "1070", "1096", "1145", "1099", "1071", "1072", "1073", "1119", "1074", "1117", "1075",
        "1087", "1089", "1077", "1082",
    ];
    NAMES
        .iter()
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
    // The header bar, under its text, but in the learning mode.
    if !sheet.learning {
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
    // The upper panel: the header and its points, then the list.
    let (header, points) = if sheet.attributes {
        (said(&sheet, "1145"), sheet.attribute_points)
    } else {
        (said(&sheet, "1096"), sheet.skill_points)
    };
    let learning = sheet.learning && !sheet.attributes;
    if !learning {
        write(header, Vec2::new(28.0, 49.0), 180.0, (0xA000_0000, centre));
        write(header, Vec2::new(26.0, 47.0), 180.0, (HEADER_TEXT, centre));
        write(
            &points.to_string(),
            Vec2::new(8.0, 48.0),
            236.0,
            (HEADER_TEXT, right),
        );
    }
    if learning {
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
    } else if sheet.attributes {
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
    // The kinds' icons, each the next to the left of the last, at its share's alpha.
    for ((first, at), alphas) in [
        (DAMAGE_ICONS, sheet.damage_shares),
        (RESISTANCE_ICONS, sheet.resistance_shares),
    ] {
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

/// The statistics window.
pub struct StatisticsPlugin;

impl Plugin for StatisticsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Attributes>()
            .init_resource::<Learning>()
            .init_resource::<LastFoe>()
            .add_systems(
                Update,
                (leave_learning, toggle, spend, show)
                    .chain()
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
