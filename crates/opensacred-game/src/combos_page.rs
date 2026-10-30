//! The inventory's combos page (#1530; docs/re/arts.md, "The combos page"): the original's
//! `cUI_ComboTab` (`0x6BEFB0`), the third of the inventory's tabs, at the window's corner
//! (0, 388): its three backgrounds `GUI_hero_combo1` to `3` of 256 × 256, tinted `0xAF`; the
//! hero's four combos' controls (`UI_IV_COMBO1` to `4`) at (32, 32), (320, 32), (32, 140) and
//! (320, 140), each the frame 174 under its combo's symbol `UI_COMBO1` to `4`, and its steps'
//! lines (`0x581550`'s mode 2) at its right, 72 along and 8 above (`0x6BFCF0`, `0x6BF6F0`); and over
//! a symbol rested on a third of a second its tooltip, `UI_TOOLTIP_COMBOSYMBOL`, centred 48 along
//! from the control and 80 below it (`0x7327B0`).
//!
//! A left press on a symbol takes its combo onto the cursor (the kind 4), the cursor empty, an
//! empty combo too; a press with one on the cursor lets it go; a right press puts the combo into
//! the bar's selected slot (`0x6BF420`, the taskbar's message 6; `art_slots`).
//!
//! Not followed: the master's window's view of the page (#1540), and `FX_COMBOALPHA180`, `120`
//! and `90`, which the page loads and never draws.

use bevy::prelude::*;

use crate::art_slots::{ArtSlots, HeldArt};
use crate::arts::HeroArts;
use crate::bar;
use crate::combos::HeroCombos;
use crate::game::{AppState, Data, FONT};
use crate::hud::Pointer;
use crate::inventory::{Page, Shown};
use crate::lines::Texts;

/// The page's corner on the original's screen (the inventory's window), and its size.
const CORNER: Vec2 = Vec2::new(0.0, 388.0);
const SIZE: Vec2 = Vec2::new(640.0, 256.0);
/// The page's number among the inventory's tabs.
pub(crate) const PAGE: u8 = 2;
/// The backgrounds, their side and tint (`0xAFFFFFFF`).
const BACKGROUNDS: [&str; 3] = ["gui_hero_combo1", "gui_hero_combo2", "gui_hero_combo3"];
const BACKGROUND: f32 = 256.0;
const TINT: f32 = 175.0 / 255.0;
/// The combos' symbols, the controls' places on the page, their side and the frame under them.
const SYMBOLS: [&str; 4] = ["ui_combo1", "ui_combo2", "ui_combo3", "ui_combo4"];
const CONTROLS: [Vec2; 4] = [
    Vec2::new(32.0, 32.0),
    Vec2::new(320.0, 32.0),
    Vec2::new(32.0, 140.0),
    Vec2::new(320.0, 140.0),
];
const SYMBOL: f32 = 64.0;
/// The control's own frame under the symbol (**measured** in the original's frames 2026-10-30):
/// a line of a pixel of `0xBB` grey, a darker bevel of two within it, and the square black.
const FRAME_LINE: Color = Color::srgb(187.0 / 255.0, 187.0 / 255.0, 187.0 / 255.0);
const FRAME_BEVEL: Color = Color::srgb(34.0 / 255.0, 34.0 / 255.0, 34.0 / 255.0);
/// The steps' lines from a control's corner, their height and colour.
const LINES_FROM: Vec2 = Vec2::new(72.0, -8.0);
const LINE: f32 = 20.0;
const WHITE: u32 = 0xFFFF_FFFF;
/// The tooltip: its text, the rest before it shows, and its centre's place from the control.
const TOOLTIP: &str = "UI_TOOLTIP_COMBOSYMBOL";
const TOOLTIP_AFTER: f32 = 1.0 / 3.0;
const TOOLTIP_FROM: Vec2 = Vec2::new(48.0, 80.0);

/// The files of the backgrounds and the symbols, found once.
#[derive(Resource, Default)]
struct Textures {
    backgrounds: [Option<String>; 3],
    symbols: [Option<String>; 4],
}

/// The page's container.
#[derive(Component)]
struct PageRoot;

/// The tooltip's window, and the centre of its top on the original's screen.
#[derive(Component)]
struct Tip(Vec2);

/// The tooltip's container.
#[derive(Component)]
struct TipRoot;

/// The control under the point `at` of the original's screen.
fn control_at(at: Vec2) -> Option<usize> {
    CONTROLS.iter().position(|&control| {
        let corner = CORNER + control;
        Rect::from_corners(corner, corner + Vec2::splat(SYMBOL)).contains(at)
    })
}

/// Finds the files of the backgrounds and the symbols once the assets are known.
fn begin(mut commands: Commands, data: Option<Res<Data>>) {
    let Some(data) = data else {
        return;
    };
    let names: Vec<&str> = BACKGROUNDS.iter().chain(&SYMBOLS).copied().collect();
    let mut found = crate::hud::find_textures(&data.0.assets, &names).into_iter();
    let backgrounds = std::array::from_fn(|_| found.next().flatten());
    let symbols = std::array::from_fn(|_| found.next().flatten());
    commands.insert_resource(Textures {
        backgrounds,
        symbols,
    });
}

/// A press on the page (`0x6BF420`): with a combo or an art on the cursor, the left lets it go;
/// with the cursor empty, the combo under it onto the cursor; the right puts the combo under it
/// into the bar's selected slot.
fn pick(
    buttons: Res<ButtonInput<MouseButton>>,
    (shown, page): (Res<Shown>, Res<Page>),
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    (combos, pack): (Res<HeroCombos>, Res<crate::pickup::Pack>),
    (mut held, mut slots): (ResMut<HeldArt>, ResMut<ArtSlots>),
) {
    let (left, right) = (
        buttons.just_pressed(MouseButton::Left),
        buttons.just_pressed(MouseButton::Right),
    );
    if !shown.0 || page.0 != PAGE || !(left || right) {
        return;
    }
    let Some(at) = crate::inventory::screen_point(&pointer, &ui_scale, &windows) else {
        return;
    };
    if !Rect::from_corners(CORNER, CORNER + SIZE).contains(at) {
        return;
    }
    if left && held.0.is_some() {
        held.0 = None;
        return;
    }
    let Some(combo) = control_at(at).and_then(|index| combos.0.get(index)) else {
        return;
    };
    if left && pack.held.is_none() {
        held.0 = Some(combo.id);
    } else if right {
        let slot = slots.selected;
        if slots.arts[slot] != Some(combo.id) {
            slots.arts[slot] = Some(combo.id);
        }
    }
}

/// Makes the page as it shows and as the combos' steps change, and takes it away as it hides.
fn show(
    mut commands: Commands,
    (shown, page): (Res<Shown>, Res<Page>),
    (assets, textures): (Res<AssetServer>, Option<Res<Textures>>),
    (combos, arts, texts): (Res<HeroCombos>, Option<Res<HeroArts>>, Option<Res<Texts>>),
    roots: Query<Entity, With<PageRoot>>,
    mut was: Local<Option<Vec<Vec<String>>>>,
) {
    let on = shown.0 && page.0 == PAGE;
    let now = on.then(|| {
        combos
            .0
            .iter()
            .map(|combo| match (arts.as_deref(), texts.as_deref()) {
                (Some(arts), Some(texts)) => crate::combos::step_lines(combo, arts, texts),
                _ => Vec::new(),
            })
            .collect::<Vec<_>>()
    });
    if *was == now {
        return;
    }
    *was = now.clone();
    for root in &roots {
        commands.entity(root).despawn();
    }
    let Some(lines) = now else {
        return;
    };
    let textures = textures.as_deref();
    let root = commands
        .spawn((PageRoot, crate::backpack::screen(), GlobalZIndex(-1)))
        .id();
    let picture = |file: &Option<String>, at: Vec2, side: f32| {
        file.as_ref().map(|file| {
            (
                ImageNode::new(crate::hud::image(&assets, file.clone())),
                bar::placed(at, Vec2::splat(side)),
                Pickable::IGNORE,
            )
        })
    };
    for (index, file) in textures
        .map(|textures| &textures.backgrounds)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let x = BACKGROUND * f32::from(u8::try_from(index).unwrap_or(0));
        if let Some((mut image, node, pickable)) =
            picture(file, CORNER + Vec2::new(x, 0.0), BACKGROUND)
        {
            image.color = Color::srgba(1.0, 1.0, 1.0, TINT);
            let child = commands.spawn((image, node, pickable)).id();
            commands.entity(root).add_child(child);
        }
    }
    let font = assets.load(FONT);
    for (index, control) in CONTROLS.iter().enumerate() {
        let at = CORNER + *control;
        let mut outer = bar::placed(at, Vec2::splat(SYMBOL));
        outer.border = UiRect::all(Val::Px(1.0));
        let mut inner = Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        };
        inner.position_type = PositionType::Relative;
        let child = commands
            .spawn((
                outer,
                BorderColor::all(FRAME_LINE),
                BackgroundColor(Color::BLACK),
                Pickable::IGNORE,
            ))
            .with_child((
                inner,
                BorderColor::all(FRAME_BEVEL),
                BackgroundColor(Color::BLACK),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(root).add_child(child);
        let symbol = textures.and_then(|textures| textures.symbols[index].as_ref());
        if let Some(symbol) = picture(&symbol.cloned(), at, SYMBOL) {
            let child = commands.spawn(symbol).id();
            commands.entity(root).add_child(child);
        }
        for (row, line) in lines[index].iter().enumerate() {
            let y = LINE * f32::from(u8::try_from(row).unwrap_or(0));
            let child = commands
                .spawn((
                    bar::placed(at + LINES_FROM + Vec2::new(0.0, y), Vec2::new(240.0, LINE)),
                    Pickable::IGNORE,
                ))
                .with_child((
                    Text::new(line.clone()),
                    TextFont {
                        font: font.clone().into(),
                        font_size: FontSize::Px(crate::tooltip::FONT_SIZE),
                        ..default()
                    },
                    TextLayout {
                        linebreak: LineBreak::NoWrap,
                        ..default()
                    },
                    TextColor(crate::tooltip::argb(WHITE)),
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(root).add_child(child);
        }
    }
}

/// The symbol's tooltip, shown once the pointer rests on a control a third of a second and taken
/// away as it leaves or the page hides.
fn tooltip(
    mut commands: Commands,
    time: Res<Time<Real>>,
    (shown, page): (Res<Shown>, Res<Page>),
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    (assets, texts): (Res<AssetServer>, Option<Res<Texts>>),
    tips: Query<Entity, With<TipRoot>>,
    mut rested: Local<Option<(usize, f32, bool)>>,
) {
    let over = (shown.0 && page.0 == PAGE)
        .then(|| crate::inventory::screen_point(&pointer, &ui_scale, &windows))
        .flatten()
        .and_then(control_at);
    let now = time.elapsed_secs();
    match (over, *rested) {
        (Some(control), Some((was, _, _))) if control == was => {}
        (Some(control), _) => {
            for tip in &tips {
                commands.entity(tip).despawn();
            }
            *rested = Some((control, now, false));
        }
        (None, _) => {
            for tip in &tips {
                commands.entity(tip).despawn();
            }
            *rested = None;
        }
    }
    let Some((control, since, false)) = *rested else {
        return;
    };
    if now - since < TOOLTIP_AFTER {
        return;
    }
    *rested = Some((control, since, true));
    let Some(text) = texts.as_deref().and_then(|texts| texts.named(TOOLTIP)) else {
        return;
    };
    let centre = CORNER + CONTROLS[control] + TOOLTIP_FROM;
    let panel = commands
        .spawn((
            Tip(centre),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(centre.x),
                top: Val::Px(centre.y),
                padding: UiRect::axes(
                    Val::Px(crate::tooltip::INSET),
                    Val::Px(crate::tooltip::INSET / 2.0),
                ),
                ..default()
            },
            BackgroundColor(crate::tooltip::PANEL),
            Pickable::IGNORE,
        ))
        .with_child((
            Text::new(text),
            TextFont {
                font: assets.load(FONT).into(),
                font_size: FontSize::Px(crate::tooltip::FONT_SIZE),
                ..default()
            },
            TextLayout {
                linebreak: LineBreak::NoWrap,
                ..default()
            },
            TextColor(crate::tooltip::argb(WHITE)),
            Pickable::IGNORE,
        ))
        .id();
    let root = commands
        .spawn((TipRoot, crate::backpack::screen(), GlobalZIndex(2)))
        .id();
    commands.entity(root).add_child(panel);
}

/// The tooltip centred on its place by its width as laid out, kept 16 pixels within the screen.
fn centre_tip(mut tips: Query<(&Tip, &ComputedNode, &mut Node)>) {
    for (tip, computed, mut node) in &mut tips {
        let width = computed.size().x * computed.inverse_scale_factor();
        let left = tip_left(tip.0.x, width);
        if node.left != Val::Px(left) {
            node.left = Val::Px(left);
        }
    }
}

/// The left of a tooltip of `width` centred on `centre`, kept 16 pixels within the screen.
fn tip_left(centre: f32, width: f32) -> f32 {
    let margin = crate::tooltip::MARGIN;
    (centre - width / 2.0)
        .min(bar::SCREEN.x - margin - width)
        .max(margin)
}

/// The inventory's combos page.
pub struct CombosPagePlugin;

impl Plugin for CombosPagePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            begin
                .run_if(resource_exists::<Data>)
                .run_if(not(resource_exists::<Textures>)),
        )
        .add_systems(
            Update,
            (pick, show, tooltip, centre_tip)
                .chain()
                .after(crate::inventory::Opened)
                .run_if(in_state(AppState::InGame)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_press_finds_its_combos_control() {
        assert_eq!(control_at(Vec2::new(32.0, 420.0)), Some(0));
        assert_eq!(control_at(Vec2::new(383.0, 483.0)), Some(1));
        assert_eq!(control_at(Vec2::new(40.0, 530.0)), Some(2));
        assert_eq!(control_at(Vec2::new(100.0, 420.0)), None);
    }

    #[test]
    fn the_tooltip_is_centred_and_kept_on_the_screen() {
        assert_eq!(tip_left(400.0, 200.0), 300.0);
        // The first control's, as wide as the original's line, moved to the margin.
        assert_eq!(tip_left(80.0, 340.0), 16.0);
        assert_eq!(tip_left(1000.0, 100.0), 1024.0 - 16.0 - 100.0);
    }
}
