//! The bar's art slots (#1529; docs/re/interface.md, "The art slots"): each of the five slots at
//! the right holds an art (the inventory entry's records at `+0xC90`), the selected one's the art
//! that the right button makes (`0x5D9860`, `cEngine +0x5A`; `making::Selected`); a slot is
//! selected by a click, the right button's or, outside the edit mode, the left's (`0x6E1BC0`), and
//! by the keys 6 to 9 and 0, and 1 to 5 with the weapon sets (`COMBINE_SLOTS`, on by default).
//! While the inventory shows, the bar is in its edit mode (the taskbar's message 2): the left button
//! drops the art that the cursor holds into a slot, or clears an empty or the selected slot. The
//! spells page's arts go onto the cursor (`spells_page`, the cursor's kinds 2 and 3) and the right
//! button puts one into the selected slot (the messages 3 and 4); an art on the cursor shows its
//! vivid picture at the cursor, and goes as the inventory hides.
//!
//! Not followed yet: the Vampiress's second bank of the slots (`+0x1162` bit 1, `0x557040`), the
//! keys F1 to F6 (which the game's own keys hold), the combos' slots (#1530), the slots kept in
//! the saves, and a slot's tooltip.

use bevy::prelude::*;

use crate::arts::HeroArts;
use crate::bar::{self, UiImages};
use crate::game::AppState;
use crate::hud::Pointer;
use crate::inventory::Shown;
use crate::making::Selected;

/// The slots at the right.
pub(crate) const SLOTS: usize = 5;
/// The keys that select the slots 1 to 5, and those that select them with the weapon sets
/// (`COMBINE_SLOTS`, `0x81CF80`).
const KEYS: [KeyCode; SLOTS] = [
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
    KeyCode::Digit0,
];
const COMBINED: [KeyCode; SLOTS] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
];
/// An art's picture on the cursor, its side and its corner from the pointer (`0x6C1930`).
const ON_CURSOR: f32 = 64.0;

/// The slots' arts, each slot's art if it holds one, the slot selected, and whether a new game's
/// first slot took the hero's first art.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct ArtSlots {
    pub arts: [Option<u16>; SLOTS],
    pub selected: usize,
    seeded: bool,
}

impl ArtSlots {
    /// The selected slot's art.
    pub fn selected_art(&self) -> Option<u16> {
        self.arts.get(self.selected).copied().flatten()
    }
}

/// The art on the cursor (the cursor's kind 2, a spell, or 3, a weapon art; `cMouse +0x68`).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeldArt(pub Option<u16>);

/// Whether the art `art` is a spell (the first table) rather than a weapon art.
pub(crate) fn is_spell(art: u16) -> bool {
    art < 1000
}

/// What a click on the slot `slot` does (`0x6E1BC0`): the slots afterwards, and whether the art on
/// the cursor went into it. The right button selects the slot; the left, outside the edit mode,
/// too; in the edit mode with an art on the cursor, the art goes into the slot, which is selected;
/// with the cursor empty, an empty or the selected slot is cleared, then selected.
fn clicked(
    slots: &ArtSlots,
    slot: usize,
    (right, editing): (bool, bool),
    held: Option<u16>,
) -> (ArtSlots, bool) {
    let mut after = slots.clone();
    let mut dropped = false;
    if !right && editing {
        match held {
            Some(art) => {
                after.arts[slot] = Some(art);
                dropped = true;
            }
            None if after.arts[slot].is_none() || slot == slots.selected => {
                after.arts[slot] = None;
            }
            None => {}
        }
    }
    after.selected = slot;
    (after, dropped)
}

/// The art slot under the point `at` of the original's screen, of `count` shown.
fn slot_at(at: Vec2, count: usize) -> Option<usize> {
    (0..count.min(SLOTS)).find(|&slot| {
        let corner = bar::art_slot(slot);
        Rect::from_corners(corner, corner + Vec2::splat(64.0)).contains(at)
    })
}

/// Keeps the right button's art the selected slot's, and a slot the console's choice: as a game
/// starts, the first slot holds the hero's first art (`making::first_art`).
fn sync(mut slots: ResMut<ArtSlots>, mut selected: ResMut<Selected>, arts: Option<Res<HeroArts>>) {
    if !slots.seeded {
        let Some(arts) = arts else {
            return;
        };
        slots.arts[0] = crate::making::first_art(&arts);
        slots.selected = 0;
        slots.seeded = true;
    } else if selected.is_changed() && !slots.is_changed() {
        // The console's `arts select` names an art: it goes into the selected slot.
        let slot = slots.selected;
        if slots.arts[slot] != selected.0 {
            slots.arts[slot] = selected.0;
        }
    }
    let art = slots.selected_art();
    if selected.0 != art {
        selected.0 = art;
    }
}

/// The clicks on the art slots and their keys.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn choose(
    (keys, buttons): (Res<ButtonInput<KeyCode>>, Res<ButtonInput<MouseButton>>),
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    (shown, inventory, console): (
        Option<Res<bar::Shown>>,
        Option<Res<Shown>>,
        Option<Res<crate::console::Console>>,
    ),
    (mut slots, mut held): (ResMut<ArtSlots>, ResMut<HeldArt>),
) {
    let count = shown.map_or(0, |shown| shown.0);
    let editing = inventory.is_some_and(|inventory| inventory.0);
    let pressed = if buttons.just_pressed(MouseButton::Right) {
        Some(true)
    } else if buttons.just_pressed(MouseButton::Left) {
        Some(false)
    } else {
        None
    };
    if let Some(right) = pressed
        && let Some(slot) = crate::inventory::screen_point(&pointer, &ui_scale, &windows)
            .and_then(|at| slot_at(at, count))
    {
        let (after, dropped) = clicked(&slots, slot, (right, editing), held.0);
        if dropped {
            held.0 = None;
        }
        if *slots != after {
            info!(
                "art slots: the slot {} holds {:?}",
                slot + 1,
                after.arts[slot]
            );
            *slots = after;
        }
        return;
    }
    if console.is_some_and(|console| console.is_open()) {
        return;
    }
    let keyed = KEYS
        .iter()
        .position(|&key| keys.just_pressed(key))
        .or_else(|| COMBINED.iter().position(|&key| keys.just_pressed(key)));
    if let Some(slot) = keyed.filter(|&slot| slot < count)
        && slots.selected != slot
    {
        slots.selected = slot;
    }
}

/// The inventory's hide lets go of the art on the cursor (`0x6C5A50`).
fn let_go(shown: Res<Shown>, mut held: ResMut<HeldArt>) {
    if shown.is_changed() && !shown.0 && held.0.is_some() {
        held.0 = None;
    }
}

/// The picture of the art on the cursor.
#[derive(Component)]
struct OnCursor(u16);

/// The art on the cursor drawn at the cursor, its vivid picture (`0x6C1930`), made as an art comes
/// onto it and taken away as it goes.
fn draw(
    mut commands: Commands,
    held: Res<HeldArt>,
    (images, assets, arts): (Res<UiImages>, Res<AssetServer>, Option<Res<HeroArts>>),
    (pointer, ui_scale): (Res<Pointer>, Res<UiScale>),
    mut pictures: Query<(Entity, &OnCursor, &mut Node)>,
) {
    for (entity, on, _) in &pictures {
        if held.0 != Some(on.0) {
            commands.entity(entity).despawn();
        }
    }
    let Some(art) = held.0 else {
        return;
    };
    let scale = ui_scale.0.max(0.01);
    let at = pointer
        .0
        .map(|at| at / scale - Vec2::splat(ON_CURSOR / 2.0));
    if let Some((_, _, mut node)) = pictures.iter_mut().find(|(_, on, _)| on.0 == art) {
        if let Some(at) = at {
            node.left = Val::Px(at.x);
            node.top = Val::Px(at.y);
        }
        return;
    }
    let Some(vivid) = arts
        .as_deref()
        .and_then(|arts| crate::spells_page::sprites(arts, art))
        .map(|[_, _, vivid]| vivid)
    else {
        return;
    };
    let (mut image, mut node) =
        bar::piece(&images, &assets, vivid, at.unwrap_or_default(), Vec2::ZERO);
    image.image_mode = NodeImageMode::Stretch;
    node.width = Val::Px(ON_CURSOR);
    node.height = Val::Px(ON_CURSOR);
    commands.spawn((
        OnCursor(art),
        image,
        node,
        GlobalZIndex(i32::MAX - 1),
        Pickable::IGNORE,
        DespawnOnExit(AppState::InGame),
    ));
}

/// A new game's slots empty, the cursor's art gone.
fn reset(mut slots: ResMut<ArtSlots>, mut held: ResMut<HeldArt>) {
    *slots = ArtSlots::default();
    held.0 = None;
}

/// The bar's art slots.
pub struct ArtSlotsPlugin;

impl Plugin for ArtSlotsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ArtSlots>()
            .init_resource::<HeldArt>()
            .add_systems(OnEnter(AppState::InGame), reset)
            .add_systems(
                Update,
                (let_go, choose, sync, draw)
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
    fn a_click_selects_drops_or_clears_a_slot() {
        let slots = ArtSlots {
            arts: [Some(1071), None, Some(13), None, None],
            selected: 0,
            seeded: true,
        };
        // The right button selects, the art kept, the cursor's art not dropped.
        let (after, dropped) = clicked(&slots, 2, (true, true), Some(20));
        assert_eq!(
            (after.selected, after.arts[2], dropped),
            (2, Some(13), false)
        );
        // The left outside the edit mode selects.
        let (after, _) = clicked(&slots, 2, (false, false), None);
        assert_eq!((after.selected, after.arts[2]), (2, Some(13)));
        // In the edit mode the cursor's art goes in.
        let (after, dropped) = clicked(&slots, 1, (false, true), Some(20));
        assert_eq!(
            (after.selected, after.arts[1], dropped),
            (1, Some(20), true)
        );
        // With the cursor empty the selected slot is cleared, another holding an art only
        // selected.
        let (after, _) = clicked(&slots, 0, (false, true), None);
        assert_eq!((after.selected, after.arts[0]), (0, None));
        let (after, _) = clicked(&slots, 2, (false, true), None);
        assert_eq!((after.selected, after.arts[2]), (2, Some(13)));
    }
}
