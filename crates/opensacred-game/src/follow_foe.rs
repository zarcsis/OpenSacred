//! The cursor that follows the foe, the options' "follow opponents with the mouse"
//! (`AUTOTRACKENEMY`, `[interface] follow_foe`; #2343, docs/re/menus.md): while the hero walks to
//! the foe it attacks, each scroll of the view by the hero moves the system's cursor as far on the
//! screen as it moves the world, so that the cursor stays on the foe it pointed at
//! (`cEngine_scrollViewAndWarpCursor`, `0x6105B0`, `GetCursorPos`, `SetCursorPos`). The original
//! does it while neither the saves' window, the options nor its third window shows, which pause
//! the game here.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::attack::Attack;
use crate::hero::Body;
use crate::iso;
use crate::pause::Play;
use crate::settings::Settings;

/// The scroll on the screen, in the window's logical pixels, of the view that follows the hero
/// moved by `moved` in the world, a window `height` high at `zoom`: the world moves against the
/// view, right on the screen by the move along its left and down by the move along its up.
fn scroll(moved: Vec3, height: f32, zoom: f32) -> Vec2 {
    let pixels = height * iso::PIXELS_PER_UNIT / (iso::SCREEN_HEIGHT * zoom.max(f32::EPSILON));
    Vec2::new(-moved.dot(iso::right()), moved.dot(iso::up())) * pixels
}

/// Moves the cursor by the view's scroll while the hero walks to the foe it attacks, the options
/// asking for it; the hero's place of the frame before kept, which the view follows.
fn follow(
    settings: Option<Res<Settings>>,
    heroes: Query<(&Transform, &Body, Option<&Attack>)>,
    cameras: Query<&Projection, iso::Eye<With<Camera3d>>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut before: Local<Option<Vec3>>,
) {
    let Some((at, body, attack)) = heroes.iter().next() else {
        *before = None;
        return;
    };
    let Some(previous) = before.replace(at.translation) else {
        return;
    };
    let on = settings
        .as_deref()
        .is_some_and(|settings| settings.interface.follow_foe);
    let walks_to_foe =
        attack.is_some_and(|attack| attack.target().is_some()) && body.moving().is_some();
    if !(on && walks_to_foe) {
        return;
    }
    let zoom = match cameras.iter().next() {
        Some(Projection::Orthographic(ortho)) => ortho.scale,
        _ => 1.0,
    };
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let moved = scroll(at.translation - previous, window.height(), zoom);
    if moved == Vec2::ZERO {
        return;
    }
    debug!("follow foe: the view scrolls by {moved}");
    // Only a window that has the focus moves the system's cursor.
    if !window.focused {
        return;
    }
    if let Some(cursor) = window.cursor_position() {
        window.set_cursor_position(Some(cursor + moved));
    }
}

/// The cursor that follows the foe.
pub struct FollowFoePlugin;

impl Plugin for FollowFoePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, follow.run_if(in_state(Play::Running)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_world_scrolls_against_the_heros_walk() {
        // A cell's diagonal across the screen, 96 pixels at zoom 1 on the original's height.
        let east = iso::right() * iso::CELL * std::f32::consts::SQRT_2;
        let moved = scroll(east, iso::SCREEN_HEIGHT, 1.0);
        assert!(
            (moved.x + 96.0).abs() < 1e-3 && moved.y.abs() < 1e-3,
            "{moved}"
        );
        // Up the screen: the world comes down; at zoom 2 half as far.
        let north = iso::up() * 10.0;
        assert!(scroll(north, iso::SCREEN_HEIGHT, 1.0).y > 0.0);
        let near = scroll(east, iso::SCREEN_HEIGHT, 2.0);
        assert!((near.x + 48.0).abs() < 1e-3);
    }
}
