//! The Dwarf's smithing (#1820; docs/re/blacksmith.md, "The Dwarf's smithing"): the button of the
//! equipment window that a Dwarf hero holding the skill 33 has (`cUI_Equipment`, `0x6BAC80`,
//! drawn by `0x6B7700`), its tooltip, and its press (`0x6B6890`), which shows the smith's window
//! with the hero as the smith (`blacksmith::open_own`), or hides it where it shows as his.
//!
//! The button is drawn while the hero is of the Dwarf's class and his skill 33's value is not 0:
//! its pictures 1607 and 1608 where the smith's window shows as his or where none of the windows
//! of the traders and the stashes shows (`0x75ADB0`), else the greyed 1609 and 1610; the second
//! of each while a smith's window shows (its flag `0x10` clear, `0x7325C0`), the first else
//! (`0x732550`; **seen**: the button darker as the window shows).

use bevy::prelude::*;
use opensacred_rules::fight::EffectLine;
use opensacred_rules::smith::Smith;

use crate::arts::HeroArts;
use crate::bar::{self, UiImages};
use crate::blacksmith::Blacksmith;
use crate::fight::{Base, Experience, Rules};
use crate::game::AppState;
use crate::hero::{Body, Hero};
use crate::hud::Pointer;
use crate::inventory::{EQUIPMENT, EquipmentWindow, Shown, screen_point};
use crate::lines::Texts;
use crate::tooltip::{Rest, TipRoot, spawn_tip};

/// The smithing's skill.
const FORGE_LORE: u8 = 33;
/// The button's corner in the equipment window.
const AT: Vec2 = Vec2::new(190.0, 105.0);
/// Its pictures: usable and greyed, each without and with a smith's window shown.
const PICTURES: [[u32; 2]; 2] = [[1607, 1608], [1609, 1610]];
/// Its tooltip's centre from its corner (the style `0x209`).
const TIP_FROM: Vec2 = Vec2::new(48.0, -16.0);

/// The button, by the picture it shows.
#[derive(Component)]
struct Forge(u32);

/// The windows of the traders and the stashes but the smith's (`0x75ADB0`: the interface
/// manager's `+0x90` the merchant's, `+0xB8` the combat master's, `+0xC0` the town stash's and
/// `+0xD8` the players' trade, which the game has not; `+0x8C` the smith's).
type Others<'w> = (
    Option<Res<'w, crate::town_stash::Opened>>,
    Option<Res<'w, crate::master::Master>>,
    Option<Res<'w, crate::merchant::Merchant>>,
);

/// Whether one of the `others` shows.
fn others_show((stash, master, merchant): &Others) -> bool {
    stash.as_ref().is_some_and(|stash| stash.0.is_some()) || master.is_some() || merchant.is_some()
}

/// Whether the button may be used (`0x6B6890`, `0x6B7FBE`): the smith's window shows as the
/// Dwarf's own (`0x75AA50`, `0x75AA70`), or none of the traders' and the stashes' windows shows.
fn usable(others: bool, smith: Option<Smith>) -> bool {
    matches!(smith, Some(Smith::Dwarf { .. })) || (!others && smith.is_none())
}

/// The button's picture where `others` show and the smith's window as `smith`'s: greyed where it
/// may not be used, the second while a smith's window shows.
fn picture(others: bool, smith: Option<Smith>) -> u32 {
    PICTURES[usize::from(!usable(others, smith))][usize::from(smith.is_some())]
}

/// The hero, his skills and his values, whose skill 33 the button takes.
type Dwarf<'w, 's> = (
    Option<Res<'w, Hero>>,
    Option<Res<'w, HeroArts>>,
    Query<'w, 's, (Entity, &'static Base), With<Body>>,
);

/// The Dwarf hero, his type and his skill 33's value where he holds it.
fn dwarf((hero, arts, heroes): &Dwarf) -> Option<(Entity, u32, u8)> {
    let (entity, base) = heroes.iter().next()?;
    let skill = forge_lore(hero.as_deref(), Some(base), arts.as_deref())?;
    Some((entity, base.id, skill))
}

/// The hero's skill 33's value where he is a Dwarf and holds it.
fn forge_lore(hero: Option<&Hero>, base: Option<&Base>, arts: Option<&HeroArts>) -> Option<u8> {
    let (hero, base, arts) = (hero?, base?, arts?);
    let value = arts
        .skills
        .iter()
        .find(|&&(skill, _)| skill == FORGE_LORE)
        .map_or(0, |&(_, value)| value);
    let skill = u8::try_from(value.min(255)).ok()?;
    match Smith::of(hero.creature, base.id, skill) {
        Smith::Dwarf { skill } if skill > 0 => Some(skill),
        _ => None,
    }
}

/// The button's rectangle on the original's screen.
fn rect(images: &UiImages) -> Rect {
    let corner = EQUIPMENT.min + AT;
    Rect::from_corners(corner, corner + images.size(PICTURES[0][0]))
}

/// Draws the button in the equipment window while the Dwarf holds the skill, its picture by the
/// windows that show.
fn draw(
    mut commands: Commands,
    windows: Query<Entity, With<EquipmentWindow>>,
    buttons: Query<(Entity, &Forge)>,
    hero: Dwarf,
    (others, smith): (Others, Option<Res<Blacksmith>>),
    (images, assets): (Res<UiImages>, Res<AssetServer>),
) {
    let skill = dwarf(&hero);
    let window = windows.iter().next();
    let smith = smith.as_deref().map(Blacksmith::smith);
    let picture = window
        .and(skill)
        .map(|_| picture(others_show(&others), smith));
    let mut kept = false;
    for (button, forge) in &buttons {
        if Some(forge.0) == picture {
            kept = true;
        } else {
            commands.entity(button).despawn();
        }
    }
    let (Some(window), Some(picture), false) = (window, picture, kept) else {
        return;
    };
    let (image, node) = bar::piece(&images, &assets, picture, AT, Vec2::ZERO);
    let button = commands
        .spawn((image, node, Forge(picture), Pickable::IGNORE))
        .id();
    commands.entity(window).add_child(button);
}

/// Its tooltip (`0x6B7700` at `0x6B8054`) after the rest (`0x7327B0`): the skill's lines
/// (`UI_TOOLTIP_SCHMIEDEN`, its `_ITEM`, `_NEXT` and `_COST`, as the skill's panel tells them,
/// `opensacred_rules::fight::skill_effects`), of the style `0x209` centred 48 along and 16 above
/// its corner.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn tooltip(
    mut commands: Commands,
    time: Res<Time<Real>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    buttons: Query<(), With<Forge>>,
    (images, assets, texts): (Res<UiImages>, Res<AssetServer>, Option<Res<Texts>>),
    (experience, rules): (Option<Res<Experience>>, Option<Res<Rules>>),
    hero: Dwarf,
    tips: Query<Entity, With<TipRoot>>,
    mut rest: Local<Rest>,
) {
    let rect = rect(&images);
    let over = !buttons.is_empty()
        && screen_point(&pointer, &ui_scale, &windows).is_some_and(|at| rect.contains(at));
    let (gone, due) = rest.step(over.then_some(0), time.elapsed_secs());
    if gone {
        for tip in &tips {
            commands.entity(tip).despawn();
        }
    }
    let (Some(_), Some(texts), Some(experience), Some(rules)) = (due, texts, experience, rules)
    else {
        return;
    };
    let Some((_, _, skill)) = dwarf(&hero) else {
        return;
    };
    let text = |name: &str| texts.named(name).unwrap_or_default().to_owned();
    let lines: Vec<String> = opensacred_rules::fight::skill_effects(
        FORGE_LORE,
        u32::from(skill),
        experience.level,
        false,
        &rules,
    )
    .into_iter()
    .map(|effect| match effect {
        EffectLine::Text(name) => text(&name),
        EffectLine::Value(names, value) => {
            let mut parts: Vec<String> = names.iter().map(|name| text(name)).collect();
            parts.push(value);
            parts.join(" ")
        }
        EffectLine::Colon(name, value) => format!("{}: {value}", text(&name)),
    })
    .collect();
    spawn_tip(
        &mut commands,
        &assets,
        &lines.join("\n"),
        rect.min + TIP_FROM,
    );
}

/// Its press (`0x6B6890`): a left press in it, the inventory shown and the Dwarf holding the
/// skill, shows the smith's window with him as the smith where none of the traders' and the
/// stashes' windows shows (the UI events `0x11` and `0x30` 8 with his id), else hides the smith's
/// window where it shows as his (`0x6B6E71`: the event `0x12`).
fn press(
    mut commands: Commands,
    (mouse, shown): (Res<ButtonInput<MouseButton>>, Res<Shown>),
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    images: Res<UiImages>,
    (hero, experience): (Dwarf, Option<Res<Experience>>),
    (others, mut smith): (Others, Option<ResMut<Blacksmith>>),
) {
    if !shown.0 || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(at) = screen_point(&pointer, &ui_scale, &windows) else {
        return;
    };
    if !rect(&images).contains(at) {
        return;
    }
    let Some((entity, kind, skill)) = dwarf(&hero) else {
        return;
    };
    match smith.as_deref_mut() {
        None if !others_show(&others) => {
            let level = experience.map_or(1, |experience| experience.level);
            commands.queue(move |world: &mut World| {
                crate::blacksmith::open_own(world, entity, kind, (skill, level));
            });
        }
        Some(smith) if matches!(smith.smith(), Smith::Dwarf { .. }) => smith.closing = true,
        _ => {}
    }
}

pub struct ForgePlugin;

impl Plugin for ForgePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (press, draw, tooltip)
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
    fn the_button_greys_under_another_window_and_darkens_as_the_smith_shows() {
        let dwarf = Some(Smith::Dwarf { skill: 5 });
        let creature = Some(Smith::Creature { kind: 688 });
        // Nothing shows, the Dwarf's own window shows.
        assert_eq!(picture(false, None), 1607);
        assert_eq!(picture(false, dwarf), 1608);
        // A merchant's or a stash's window, a creature's smith's window.
        assert_eq!(picture(true, None), 1609);
        assert_eq!(picture(false, creature), 1610);
        assert!(usable(false, None) && usable(true, dwarf));
        assert!(!usable(true, None) && !usable(false, creature));
    }
}
