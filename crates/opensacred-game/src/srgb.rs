//! The frame in sRGB values (docs/re/drawing.md, "Blending"). The original's Direct3D 7 device
//! blends what it draws with the 8-bit values its frame holds, as they are: black at half its
//! alpha over a grey of 128 leaves 64. An sRGB target decodes what lies in it, blends in linear
//! light and encodes the result again, and left 92 (#109).
//!
//! So the camera renders into a target of `Rgba8Unorm` that holds sRGB values ([`Frame`]): Bevy's
//! own sRGB compositing (`CompositingSpace::Srgb`), whose last pass decodes the frame onto the
//! window. Bevy grants it to 2D cameras alone, since its 3D materials write linear light; the
//! game's materials encode their colour last (`post.wesl`, `standard.wesl`), the night's pass
//! multiplies the values as they are (`night.wesl`), and the HUD gives Bevy's interface its
//! colours and images in those values (`hud`).

use bevy::camera::CompositingSpace;
use bevy::prelude::{App, Component, IntoScheduleConfigs, Plugin, Query, With};
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::view::{ResolveCompositingSpaces, ResolvedCompositingSpace};
use bevy::render::{Render, RenderApp, RenderSystems};

/// A camera whose frame holds sRGB values and blends them as they are.
#[derive(Component, ExtractComponent, Clone, Copy, Default)]
#[extract_app(RenderApp)]
#[require(CompositingSpace = CompositingSpace::Srgb)]
pub struct Frame;

/// Bevy's sRGB compositing for the 3D cameras of a [`Frame`].
pub struct SrgbPlugin;

impl Plugin for SrgbPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<Frame>::default());
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app.add_systems(
            Render,
            (
                withhold.before(ResolveCompositingSpaces),
                grant.after(ResolveCompositingSpaces),
            )
                .in_set(RenderSystems::CreateViews),
        );
    }
}

/// Takes the request of a [`Frame`] back from Bevy's resolution, which would turn it down with a
/// warning; its camera's main texture is `Rgba8Unorm` all the same, as extracted.
fn withhold(mut views: Query<&mut ResolvedCompositingSpace, With<Frame>>) {
    for mut space in &mut views {
        space.0 = None;
    }
}

/// Gives the views of a [`Frame`] the sRGB compositing that Bevy's resolution left out: its clear
/// colour in sRGB values, and the last pass decoding the frame onto the window.
fn grant(mut views: Query<&mut ResolvedCompositingSpace, With<Frame>>) {
    for mut space in &mut views {
        space.0 = Some(CompositingSpace::Srgb);
    }
}
