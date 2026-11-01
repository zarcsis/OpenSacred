//! The interface's controls' glow under the pointer (#1595; docs/re/interface.md, "The buttons"):
//! a control of the flag `0x8000000` ends its draw with `0x731F00`, which, while the control's
//! own hit test holds the cursor's point, draws its picture again as the glow `0x732220(0.3)`:
//! 49 copies shifted by −6 to +6 pixels in steps of 2, added onto the screen at the alpha
//! ⌊12.75 · 0.3⌋/255 each (`low_health::Glow`). The game gives it to the bar's buttons, the
//! belt's slots, the inventory's sort button and the statistics window's close button
//! ([`Glows`]).
//!
//! Not followed yet: the other controls of the flag (the statistics window's second button, the
//! quest book's, the world map's, the column's two and the options' buttons, and four not named).

use bevy::prelude::*;
use bevy::ui_render::prelude::MaterialNode;

use crate::game::AppState;
use crate::hud::Pointer;
use crate::low_health::{GLOW_REACH, Glow};

/// The alpha of each of the glow's copies: ⌊12.75 · 0.3⌋/255.
const ALPHA: f32 = 3.0 / 255.0;

/// A control of the interface that glows while the pointer is over it.
#[derive(Component, Default)]
pub(crate) struct Glows;

/// A control's glow, by its control.
#[derive(Component)]
struct GlowOf(Entity);

/// Gives each control that comes its glow, hidden, over its picture and the reach about it.
fn attach(
    mut commands: Commands,
    controls: Query<(Entity, &Node), Added<Glows>>,
    mut glows: ResMut<Assets<Glow>>,
) {
    for (control, node) in &controls {
        let (Val::Px(width), Val::Px(height)) = (node.width, node.height) else {
            continue;
        };
        let glow = commands
            .spawn((
                GlowOf(control),
                MaterialNode(glows.add(Glow::empty())),
                crate::bar::placed(
                    Vec2::splat(-GLOW_REACH),
                    Vec2::new(width, height) + Vec2::splat(2.0 * GLOW_REACH),
                ),
                Visibility::Hidden,
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(control).add_child(glow);
    }
}

/// Shows a control's glow of its picture as it is while the pointer is over the control, and
/// hides it as the pointer leaves.
fn draw(
    pointer: Res<Pointer>,
    windows: Query<&Window>,
    controls: Query<
        (
            &ImageNode,
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
        ),
        With<Glows>,
    >,
    mut glowing: Query<(&GlowOf, &MaterialNode<Glow>, &mut Visibility)>,
    mut glows: ResMut<Assets<Glow>>,
) {
    let point = pointer
        .0
        .zip(windows.single().ok())
        .map(|(at, window)| at * window.scale_factor());
    for (of, material, mut visibility) in &mut glowing {
        let under = controls
            .get(of.0)
            .ok()
            .filter(|(_, node, transform, shown)| {
                shown.get() && point.is_some_and(|point| node.contains_point(**transform, point))
            });
        let Some((image, ..)) = under else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        visibility.set_if_neq(Visibility::Inherited);
        if let (Some(part), Some(mut glow)) = (image.rect, glows.get_mut(&material.0)) {
            glow.of(&image.image, part, ALPHA);
        }
    }
}

/// The controls' glow under the pointer.
pub struct ControlGlowPlugin;

impl Plugin for ControlGlowPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (attach, draw).chain().run_if(in_state(AppState::InGame)),
        );
    }
}
