//! The equipment window's set icon (#1670; docs/re/inventory.md, §4): while the hero wears a
//! piece of a set (its `+0x1F4` bit 31, which `0x5711F0` sets for every piece counted), the
//! window shows the image 167 (`gui_main_04`, 45 × 45) at the screen's (678, 400), the image 168
//! while the pointer is over it, whose hover lists the sets worn (`0x6B7F0F`; `tooltip`).
//!
//! Not followed: the dominance bonus, which sets the bit too (docs/re/sets.md §1).

use bevy::prelude::*;

use crate::bar::{self, UiImages};
use crate::fight::Holding;
use crate::game::AppState;
use crate::hero::Body;
use crate::hud::Pointer;
use crate::inventory::{EQUIPMENT, EquipmentWindow, screen_point};

/// The icon's images, as it is and under the pointer.
const ICON: u32 = 167;
const LIT: u32 = 168;
/// The icon's place on the original's screen, and its side.
const AT: Vec2 = Vec2::new(678.0, 400.0);
const SIDE: f32 = 45.0;

/// The icon, lit or not.
#[derive(Component)]
struct Icon(bool);

/// Whether the point `at` of the original's screen is over the icon.
pub(crate) fn over(at: Vec2) -> bool {
    Rect::from_corners(AT, AT + Vec2::splat(SIDE)).contains(at)
}

/// Whether `holding` holds a set's piece, which the icon shows for.
pub(crate) fn shows(holding: &Holding) -> bool {
    holding.0.sets.worn.sets().next().is_some()
}

/// Puts the icon into the equipment window while the hero wears a set's piece, lit under the
/// pointer, and takes it away as the hero takes its last piece off; the window's going takes it
/// with it.
fn icon(
    mut commands: Commands,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    (images, assets): (Res<UiImages>, Res<AssetServer>),
    equipment: Query<Entity, With<EquipmentWindow>>,
    icons: Query<(Entity, &Icon)>,
    holding: Query<&Holding, With<Body>>,
) {
    let Some(window) = equipment.iter().next() else {
        return;
    };
    let wanted = holding
        .iter()
        .next()
        .is_some_and(shows)
        .then(|| screen_point(&pointer, &ui_scale, &windows).is_some_and(over));
    let shown = icons.iter().next();
    if shown.map(|(_, icon)| icon.0) == wanted {
        return;
    }
    if let Some((entity, _)) = shown {
        commands.entity(entity).despawn();
    }
    let Some(lit) = wanted else {
        return;
    };
    let index = if lit { LIT } else { ICON };
    let child = commands
        .spawn((
            bar::piece(&images, &assets, index, AT - EQUIPMENT.min, Vec2::ZERO),
            Icon(lit),
            // Over the frames and the worn items' picture, whichever comes first.
            ZIndex(1),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(window).add_child(child);
}

/// The equipment window's set icon.
pub struct SetIconPlugin;

impl Plugin for SetIconPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            icon.after(crate::inventory::Opened)
                .run_if(in_state(AppState::InGame)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_lies_at_its_place_in_the_equipment_window() {
        assert!(over(Vec2::new(678.0, 400.0)) && over(Vec2::new(722.0, 444.0)));
        assert!(!over(Vec2::new(677.0, 420.0)) && !over(Vec2::new(700.0, 446.0)));
        assert!(EQUIPMENT.contains(AT) && EQUIPMENT.contains(AT + Vec2::splat(SIDE)));
    }
}
