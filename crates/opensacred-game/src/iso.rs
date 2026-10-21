//! The original's isometric view in 3D (docs/02 §5.3): a 2:1 dimetric orthographic camera over
//! the ground plane, so that a cell's diamond is 96 × 48 pixels at zoom 1 and the world keeps the
//! original's units (a sector is 3,434 units, docs/02 §5.1), the units of the models too.
//!
//! The ground is the XZ plane, Y up: global cell `(cx, cy)` (column, row) spans
//! `[cx, cx + 1] × [cy, cy + 1]` cells along X and Z. On screen, X runs right-down and Z
//! left-down, as `48 (cx − cy)`, `24 (cx + cy)` do in the original.

use std::f32::consts::{FRAC_1_SQRT_2, FRAC_PI_6, SQRT_2};

use bevy::camera::ScalingMode;
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::prelude::*;

/// World units per cell side: 24·√5, the edge of a cell's diamond in screen pixels, as the
/// original places a creature at `(cell + 0.5) · 53.66563` (`CreateNPC`, `0x482E9D`; docs/02
/// §5.1). A sector's 3,434 in `sectors.keyx` is 64 cells truncated.
pub const CELL: f32 = 53.665_63;
/// Screen pixels per world unit at zoom 1: the cell's diagonal is a diamond 96 pixels wide.
pub const PIXELS_PER_UNIT: f32 = 96.0 / (CELL * SQRT_2);
/// The original's screen height: the camera shows as much of the world at any resolution.
pub const SCREEN_HEIGHT: f32 = 768.0;
/// World units per unit of the models (and of the creatures' speeds). The original projects its
/// models with `ortho(±267, ±200)` onto 1024 × 768 pixels, 1.92 pixels a unit (docs/05 §1),
/// while its ground has 1.26 pixels a world unit: so a model is 1.52 world units a unit. A hero
/// then stands about 120 pixels tall at zoom 1, as tall as the well and the bench beside the
/// citizens of the village suggest.
pub const MODEL_SCALE: f32 = 1024.0 / 534.0 / PIXELS_PER_UNIT;

/// Screen pixels at zoom 1 per unit of the models' height in the original, whose view looks
/// down at atan(1/2) (`0x6284E0`) under `ortho(±200)` on 768 pixels: the original raises a point
/// of height `z` by this many pixels, `768 / 400 · 2 / √5` (docs/re/drawing.md, "Depth").
pub const PIXELS_PER_MODEL_HEIGHT: f32 = 1.717_300_2;

/// How far behind the objects the ground lies along the line of sight: it changes nothing on
/// screen, but the ground never hides an object, as in the original, which draws the ground
/// first and the objects over it.
pub const GROUND_BEHIND: f32 = 5_000.0;

/// The camera's pitch below the horizon: sin 30° = 1/2 squeezes a square into a 2:1 diamond.
const PITCH: f32 = FRAC_PI_6;
/// How far back the camera stands from what it looks at (also in `sprites.wesl`).
pub const DISTANCE: f32 = 20_000.0;

/// The point of the ground at the corner of global cell `(cx, cy)`, at height 0.
pub fn ground(cx: f32, cy: f32) -> Vec3 {
    Vec3::new(cx * CELL, 0.0, cy * CELL)
}

/// World units of a height given in screen pixels at zoom 1 (cell heights): a vertical unit
/// spans cos 30° units on screen.
pub fn height(pixels: f32) -> f32 {
    pixels / (PIXELS_PER_UNIT * PITCH.cos())
}

/// The direction the camera looks in: toward −X −Z, down at 30°.
pub fn forward() -> Vec3 {
    let horizontal = PITCH.cos() * FRAC_1_SQRT_2;
    Vec3::new(-horizontal, -PITCH.sin(), -horizontal)
}

/// The point at height `height` that shows on screen where `point` does: along the line of
/// sight, `√(3/2)` units along X and along Z for a unit of height.
pub fn at_height(point: Vec3, height: f32) -> Vec3 {
    let sight = forward();
    point + sight * ((height - point.y) / sight.y)
}

/// The camera's orientation.
fn view() -> Transform {
    Transform::IDENTITY.looking_to(forward(), Vec3::Y)
}

/// Screen right in the world: toward +X −Z.
pub fn right() -> Vec3 {
    view().right().as_vec3()
}

/// Screen up in the world.
pub fn up() -> Vec3 {
    view().up().as_vec3()
}

/// The camera that looks at `target` at `zoom` (clamped to the original's range).
pub fn camera(target: Vec3, zoom: f32) -> impl Bundle {
    let projection = OrthographicProjection {
        // Everything drawn lies within a few thousand units of the point looked at (the
        // ground 5,000 behind it): a narrow range keeps the depth buffer's steps small, a
        // thousandth of a unit, against which the sprites' shader orders sprites at one depth.
        near: 0.5 * DISTANCE,
        far: 1.5 * DISTANCE,
        scaling_mode: ScalingMode::FixedVertical {
            viewport_height: SCREEN_HEIGHT / PIXELS_PER_UNIT,
        },
        scale: zoom.clamp(*ZOOM.start(), *ZOOM.end()),
        ..OrthographicProjection::default_3d()
    };
    (
        Camera3d::default(),
        // The art's texels reach the screen as they are, as in the original, which has no
        // display transform: Bevy's default tone mapping dulled the bright tones (#28).
        Tonemapping::None,
        DebandDither::Disabled,
        // What is drawn blends with the frame's sRGB values as they are, as in the original.
        crate::srgb::Frame,
        Projection::Orthographic(projection),
        view_of(target),
    )
}

/// The original's zoom range. Its zoom scales its projection, `ortho(±267·zoom, ±200·zoom, …)`
/// (docs/02 §5.3), as it scales the camera's here: 2 shows twice the world, the widest view, and
/// 0.5 half of it.
pub const ZOOM: std::ops::RangeInclusive<f32> = 0.5..=2.0;

/// Zooms with the mouse wheel within the original's range, by a tenth a notch: the wheel turned
/// forward brings the world closer.
pub fn zoom(
    scroll: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    mut projections: Query<&mut Projection, With<Camera>>,
) {
    if scroll.delta.y == 0.0 {
        return;
    }
    for mut projection in &mut projections {
        if let Projection::Orthographic(ortho) = &mut *projection {
            // The scale is the zoom.
            ortho.scale = (ortho.scale * 1.1_f32.powf(-scroll.delta.y.signum()))
                .clamp(*ZOOM.start(), *ZOOM.end());
        }
    }
}

/// The camera's transform when it looks at `target`.
pub fn view_of(target: Vec3) -> Transform {
    Transform::from_translation(target - forward() * DISTANCE).looking_to(forward(), Vec3::Y)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Projects a world point to screen pixels at zoom 1, x right and y down, relative to the
    /// projection of the origin.
    fn screen(point: Vec3) -> Vec2 {
        Vec2::new(point.dot(right()), -point.dot(up())) * PIXELS_PER_UNIT
    }

    #[test]
    fn a_cell_is_a_diamond_of_96_by_48_pixels() {
        let close = |a: Vec2, b: Vec2| (a - b).length() < 1e-3;
        // One cell along X: half a diamond right and down.
        assert!(close(screen(ground(1.0, 0.0)), Vec2::new(48.0, 24.0)));
        // One cell along Z: half a diamond left and down.
        assert!(close(screen(ground(0.0, 1.0)), Vec2::new(-48.0, 24.0)));
        // A height of 10 pixels lifts a point 10 pixels.
        assert!(close(
            screen(Vec3::new(0.0, height(10.0), 0.0)),
            Vec2::new(0.0, -10.0)
        ));
        // The original's unit of the models' height: 400 units over 768 pixels, seen at
        // atan(1/2) from above.
        let unit = 768.0 / 400.0 * 2.0 / 5.0_f32.sqrt();
        assert!((PIXELS_PER_MODEL_HEIGHT - unit).abs() < 1e-5);
        // The ground pushed back stays where it was on screen.
        assert!(close(screen(forward() * GROUND_BEHIND), Vec2::ZERO));
        // A point raised along the line of sight stays where it was on screen, √(3/2) units
        // along X and Z for a unit of height.
        let point = Vec3::new(1000.0, 30.0, 2000.0);
        let raised = at_height(point, 130.0);
        assert!(close(screen(raised), screen(point)));
        assert!((raised.y - 130.0).abs() < 1e-3);
        let along = 100.0 * 1.5_f32.sqrt();
        assert!((raised.xz() - point.xz() - Vec2::splat(along)).length() < 1e-2);
    }

    #[test]
    fn zoom_2_shows_twice_the_world() {
        use bevy::camera::CameraProjection;

        let area = |zoom: f32| {
            let mut world = World::new();
            let camera = world.spawn(camera(Vec3::ZERO, zoom)).id();
            let Some(Projection::Orthographic(mut ortho)) =
                world.get::<Projection>(camera).cloned()
            else {
                panic!("the camera is not orthographic");
            };
            ortho.update(1024.0, 768.0);
            ortho.area.size()
        };
        assert_eq!(area(2.0), area(1.0) * 2.0);
        assert_eq!(area(0.5), area(1.0) / 2.0);
        // At zoom 1 the view is the original's screen, 768 pixels down.
        assert!((area(1.0).y * PIXELS_PER_UNIT - SCREEN_HEIGHT).abs() < 1e-2);
        // Beyond the original's range the zoom stops at its ends.
        assert_eq!(area(3.0), area(2.0));
    }
}
