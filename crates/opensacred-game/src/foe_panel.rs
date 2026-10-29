//! The foe's panel (#1459; docs/re/cursor.md, "The foe's panel"): while the cursor is over a
//! living foe (`cursors::Foe`), the inventory hidden and the settings' `interface.enemy_info` on,
//! the original shows at the bottom's middle the foe's level on bronze's scale, its name and its
//! class's name over a red bar of its health, and an icon of each kind it resists, the stronger
//! the brighter (`cEngine_updateWorldCursor` hands them to the interface, `0x75C760`, which
//! draws them, `0x75C310`), until 0.1 s past the cursor's leaving it.
//!
//! Its text is laid out first, unseen, and the panel about it at the next frame, once its width
//! is known.

use bevy::prelude::*;
use bevy::text::{FontSize, LineHeight};
use bevy::ui::{BackgroundGradient, ColorStop, InterpolationColorSpace, LinearGradient};

use crate::ai::Mind;
use crate::bar::{self, UiImages};
use crate::cursors::Foe;
use crate::fight::{Combat, Health, Ranked, Rules};
use crate::frame::Playing;
use crate::game::{AppState, FONT};
use crate::lines::Texts;
use crate::npcs::ScriptName;
use crate::settings::Settings;

/// The panel's top on the original's screen, the line of its font, and the room each icon takes
/// in the panel's width (`0x75C310`).
const TOP: f32 = 620.0;
const LINE: f32 = 20.0;
const ICON_ROOM: f32 = 16.0;
/// How far its icons step along, their width less 2, and how far below the panel's top they
/// stand, their row's middle less 7 (`0x75C200`).
const ICON_OVERLAP: f32 = 2.0;
const ICON_DOWN: f32 = (LINE + 2.0) / 2.0 - 7.0;
/// The size of its font, AntiquaSSK, at which its words are the original's width (its GDI's
/// font 1, whose size is not read), and how far below the panel's top Bevy sets its letters
/// where the original's are.
const FONT_SIZE: f32 = 19.5;
const TEXT_DOWN: f32 = 3.0;
/// A text's room narrower than this is widened by half, and always that of the class 10, the
/// dragons (`0x75C45E`).
const NARROW: f32 = 100.0;
const WIDENED: u8 = 10;
/// How long the panel stays past the cursor's leaving the foe, in seconds (`0x612535`).
const LINGERS: f32 = 0.1;
/// The ground black at 159/255, the lines grey at 111/255, the health's bar from its left's dark
/// red to its right's, which the health lightens, the text white, a champion's light blue and a
/// boss's yellow (`0x75C310`), each in the frame's sRGB values (`hud`).
const GROUND: Color = Color::linear_rgba(0.0, 0.0, 0.0, 159.0 / 255.0);
const LINES: Color = Color::linear_rgba(159.0 / 255.0, 159.0 / 255.0, 159.0 / 255.0, 111.0 / 255.0);
const BAR_LEFT: [f32; 3] = [64.0, 16.0, 16.0];
const BAR_RIGHT_BY_HEALTH: [f32; 3] = [119.0, 69.0, 69.0];
const WORDS: Color = Color::WHITE;
const CHAMPION: Color = Color::linear_rgb(135.0 / 255.0, 206.0 / 255.0, 250.0 / 255.0);
const BOSS: Color = Color::linear_rgb(242.0 / 255.0, 250.0 / 255.0, 108.0 / 255.0);
/// The interface's images of the kinds' icons, physical, fire, magic and poison (`0x9EC5CC`).
const ICONS: [u32; 4] = [209, 210, 211, 212];
/// The creatures' classes' names (`0x8EB79C`, by the class of `Creature.pak +0x04`): each the
/// game's text of its number's string, none for the classes 0, 14 and 15.
const CLASS_NAMES: [u32; 16] = [
    0, 1184, 1187, 1178, 1196, 1180, 1179, 1185, 1186, 1182, 1181, 1188, 1189, 1183, 0, 0,
];

/// What the panel shows of a foe: its text, the text's colour, whether its room is widened
/// however wide it is, and each kind's icon's alpha.
#[derive(Clone, Debug, PartialEq)]
struct Shows {
    text: String,
    colour: Color,
    widened: bool,
    alphas: [u8; 4],
}

/// The panel while it shows: what it shows, its root and its text, the health's bar once laid
/// out, and how long it lingers.
#[derive(Resource, Default)]
struct Panel {
    shown: Option<(Shows, Entity, Entity)>,
    bar: Option<(Entity, f32)>,
    lingers: f32,
}

/// The panel's text, laid out unseen until the panel is laid out about it.
#[derive(Component)]
struct PanelText;

/// The icons' alphas of a foe's resistances `r` (`0x564740`): each the share of the strongest,
/// times 255 and truncated, the strongest found as `0x563F20` finds it, from the first on, a
/// stronger one taken half a point stronger; none where it resists nothing.
fn shields(r: [f32; 4]) -> [u8; 4] {
    let mut most = r[0];
    if r[0] < r[1] {
        most = r[1] + 0.5;
    }
    for &next in &r[2..] {
        if most < next {
            most = next + 0.5;
        }
    }
    if most <= 0.0 {
        return [0; 4];
    }
    r.map(|r| truncated(255.0 * r.max(0.0) / most))
}

/// The whole part of `x`, held to a byte.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is held to a byte's range first"
)]
fn truncated(x: f32) -> u8 {
    x.clamp(0.0, 255.0) as u8
}

/// The text's room: its width and 2, widened by half where narrower than 100 or where
/// `widened` asks it (`0x75C452`).
fn room(width: f32, widened: bool) -> f32 {
    let room = width.round() + 2.0;
    if widened || room < NARROW {
        (room * 1.5).trunc()
    } else {
        room
    }
}

/// The health's bar's right colour at the health's share `f`.
fn bar_right(f: f32) -> Color {
    let [r, g, b] = [0, 1, 2].map(|i| (BAR_LEFT[i] + BAR_RIGHT_BY_HEALTH[i] * f) / 255.0);
    Color::linear_rgb(r, g, b)
}

/// The health's bar's gradient at the health's share `f`, its values blended as they are, as the
/// original's device shades them.
fn bar_gradient(f: f32) -> BackgroundGradient {
    let [r, g, b] = BAR_LEFT.map(|c| c / 255.0);
    BackgroundGradient::from(LinearGradient {
        color_space: InterpolationColorSpace::LinearRgba,
        ..LinearGradient::to_right(vec![
            ColorStop::auto(Color::linear_rgb(r, g, b)),
            ColorStop::auto(bar_right(f)),
        ])
    })
}

/// The foe the panel shows at this frame: none without one, while the inventory shows, or where
/// the settings turn it off.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn shows(
    foe: &Foe,
    (settings, inventory): (Option<&Settings>, bool),
    (texts, rules): (Option<&Texts>, Option<&Rules>),
    creatures: &Query<(
        &Mind,
        &Health,
        Option<&Combat>,
        Option<&Ranked>,
        Option<&ScriptName>,
    )>,
) -> Option<(Shows, f32)> {
    if inventory || settings.is_some_and(|settings| !settings.interface.enemy_info) {
        return None;
    }
    let (mind, health, combat, ranked, key) = creatures.get(foe.0?).ok()?;
    let texts = texts?;
    // Its key's text, else its type's (`sdk_REFKEY`, `0x44A1C0`).
    let name = key
        .and_then(|key| texts.named(crate::talk::resource(&key.0)))
        .or_else(|| texts.named(&mind.creature.to_string()))?;
    let level = rules.map_or(health.level, |rules| rules.0.on_bronze(health.level));
    let class = CLASS_NAMES
        .get(usize::from(mind.class))
        .filter(|&&id| id != 0)
        .and_then(|id| texts.named(&id.to_string()));
    let text = match class {
        Some(class) => format!("[{level}] {name} - {class}"),
        None => format!("[{level}] {name}"),
    };
    let colour = match ranked.map(|ranked| ranked.0) {
        Some(opensacred_rules::fight::Rank::Champion) => CHAMPION,
        Some(opensacred_rules::fight::Rank::Boss) => BOSS,
        _ => WORDS,
    };
    let alphas = combat.map_or([0; 4], |combat| shields(combat.0.resistance));
    let share = f32::from(u16::try_from(health.current).unwrap_or(u16::MAX))
        / (f32::from(u16::try_from(health.most).unwrap_or(u16::MAX)) + 1.0);
    let shows = Shows {
        text,
        colour,
        widened: mind.class == WIDENED,
        alphas,
    };
    Some((shows, share))
}

/// Shows, keeps or takes away the panel by the foe under the cursor: a new foe's text laid out
/// unseen, the health's bar following the health.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn follow(
    mut commands: Commands,
    mut panel: ResMut<Panel>,
    time: Res<Time>,
    (foe, settings, inventory): (
        Res<Foe>,
        Option<Res<Settings>>,
        Res<crate::inventory::Shown>,
    ),
    (texts, rules, assets): (Option<Res<Texts>>, Option<Res<Rules>>, Res<AssetServer>),
    creatures: Query<(
        &Mind,
        &Health,
        Option<&Combat>,
        Option<&Ranked>,
        Option<&ScriptName>,
    )>,
    mut bars: Query<(&mut Node, &mut BackgroundGradient)>,
) {
    let now = shows(
        &foe,
        (settings.as_deref(), inventory.0),
        (texts.as_deref(), rules.as_deref()),
        &creatures,
    );
    let Some((shows, share)) = now else {
        panel.lingers -= time.delta_secs();
        if panel.lingers <= 0.0
            && let Some((_, root, _)) = panel.shown.take()
        {
            commands.entity(root).despawn();
            panel.bar = None;
        }
        return;
    };
    panel.lingers = LINGERS;
    if panel
        .shown
        .as_ref()
        .is_some_and(|(shown, ..)| *shown == shows)
    {
        if let Some((bar, room)) = panel.bar
            && let Ok((mut node, mut gradient)) = bars.get_mut(bar)
        {
            node.width = Val::Px((room * share).trunc());
            *gradient = bar_gradient(share);
        }
        return;
    }
    if let Some((_, root, _)) = panel.shown.take() {
        commands.entity(root).despawn();
    }
    panel.bar = None;
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(50.0),
                margin: UiRect {
                    left: Val::Px(-512.0),
                    top: Val::Px(-384.0),
                    ..default()
                },
                width: Val::Px(1024.0),
                height: Val::Px(768.0),
                ..default()
            },
            Pickable::IGNORE,
            DespawnOnExit(AppState::InGame),
        ))
        .id();
    let text = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(TOP + TEXT_DOWN),
                ..default()
            },
            Text::new(shows.text.clone()),
            TextFont {
                font: assets.load::<Font>(FONT).into(),
                font_size: FontSize::Px(FONT_SIZE),
                ..default()
            },
            LineHeight::Px(LINE),
            TextColor(shows.colour),
            TextLayout::no_wrap(),
            Visibility::Hidden,
            Pickable::IGNORE,
            PanelText,
            ChildOf(root),
        ))
        .id();
    panel.shown = Some((shows, root, text));
    commands.insert_resource(Share(share));
}

/// The health's share at the frame the panel's text was laid out, for its bar once laid out.
#[derive(Resource, Default)]
struct Share(f32);

/// Lays the panel out about its text once the text's width is known (`0x75C310`): the panel
/// centred on the screen's middle, the text's room and 16 for each icon wide, its ground, the
/// health's bar under the text, the icons of the kinds it resists after the text, and the grey
/// lines about the text and the icons.
fn lay(
    mut commands: Commands,
    mut panel: ResMut<Panel>,
    share: Option<Res<Share>>,
    (images, assets): (Res<UiImages>, Res<AssetServer>),
    mut texts: Query<(&ComputedNode, &mut Node, &mut Visibility), With<PanelText>>,
) {
    let Some((shows, root, text)) = panel.shown.clone() else {
        return;
    };
    if panel.bar.is_some() {
        return;
    }
    let Ok((computed, mut node, mut visibility)) = texts.get_mut(text) else {
        return;
    };
    let width = computed.size().x * computed.inverse_scale_factor();
    if width <= 0.0 {
        return;
    }
    let room = room(width, shows.widened);
    let icons = shows.alphas.iter().filter(|&&alpha| alpha > 0).count();
    let wide = ICON_ROOM * f32::from(u8::try_from(icons).unwrap_or(4)) + room;
    let x0 = 512.0 - (wide / 2.0).floor();
    node.left = Val::Px(x0);
    *visibility = Visibility::Inherited;
    let share = share.map_or(1.0, |share| share.0);
    let rect = |at: Vec2, size: Vec2| bar::placed(at, size);
    let mut parts = Vec::new();
    // The ground, then the bar, then the lines.
    parts.push(
        commands
            .spawn((
                rect(Vec2::new(x0 - 2.0, TOP), Vec2::new(wide + 5.0, LINE + 7.0)),
                BackgroundColor(GROUND),
            ))
            .id(),
    );
    let bar = commands
        .spawn((
            rect(
                Vec2::new(x0, TOP),
                Vec2::new((room * share).trunc(), LINE + 5.0),
            ),
            bar_gradient(share),
        ))
        .id();
    parts.push(bar);
    let line = |at: Vec2, size: Vec2| (rect(at, size), BackgroundColor(LINES));
    for (at, size) in [
        (Vec2::new(x0 - 1.0, TOP - 1.0), Vec2::new(wide + 3.0, 1.0)),
        (
            Vec2::new(x0 - 1.0, TOP + LINE + 5.0),
            Vec2::new(wide + 3.0, 1.0),
        ),
        (Vec2::new(x0 - 1.0, TOP - 1.0), Vec2::new(1.0, LINE + 7.0)),
        (
            Vec2::new(x0 + room + 1.0, TOP - 1.0),
            Vec2::new(1.0, LINE + 7.0),
        ),
        (
            Vec2::new(x0 + wide + 1.0, TOP - 1.0),
            Vec2::new(1.0, LINE + 7.0),
        ),
    ] {
        parts.push(commands.spawn(line(at, size)).id());
    }
    let mut x = x0 + room + 2.0;
    for (&index, &alpha) in ICONS.iter().zip(&shows.alphas) {
        if alpha == 0 {
            continue;
        }
        let (mut image, node) = bar::piece(
            &images,
            &assets,
            index,
            Vec2::new(x, TOP + ICON_DOWN + 1.0),
            Vec2::ZERO,
        );
        image.color = Color::linear_rgba(1.0, 1.0, 1.0, f32::from(alpha) / 255.0);
        x += images.size(index).x - ICON_OVERLAP;
        parts.push(commands.spawn((image, node)).id());
    }
    // The text over the ground and the bar.
    for part in parts {
        commands
            .entity(part)
            .insert((Pickable::IGNORE, ChildOf(root)));
    }
    commands.entity(text).insert(ZIndex(1));
    panel.bar = Some((bar, room));
}

/// The foe's panel.
pub struct FoePanelPlugin;

impl Plugin for FoePanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Panel>()
            .add_systems(Update, (follow, lay).chain().in_set(Playing));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icons_brighten_by_the_share_of_the_strongest_resistance() {
        // The level 5 goblin warrior's resistances times their multipliers (the original's
        // `stats`, 2026-10-29): the physical strongest, fire next, no magic, a little poison.
        let goblin = [
            7.6 * 1.238_278_4,
            7.364_705_6 * 1.238_278_4,
            0.0,
            1.32 * 1.256_607_5,
        ];
        assert_eq!(shields(goblin), [255, 247, 0, 44]);
        // A stronger one after the first is taken half a point stronger.
        assert_eq!(shields([1.0, 2.0, 0.0, 0.0]), [102, 204, 0, 0]);
        assert_eq!(shields([0.0; 4]), [0; 4]);
    }

    #[test]
    fn a_narrow_text_is_widened_by_half() {
        assert!((room(220.0, false) - 222.0).abs() < f32::EPSILON);
        assert!((room(60.0, false) - 93.0).abs() < f32::EPSILON);
        assert!((room(220.0, true) - 333.0).abs() < f32::EPSILON);
    }
}
