// The original's order of drawing as depth (docs/re/drawing.md): a place in the order is an
// anchor, the centre of a cell at height 0 and its draw list's depth, and a place in the cell.
// The cell's height has no part in it: the original orders a list's objects by their cells alone
// (#112).
// Shared by the sprites' shader (`sprites.wesl`) and the models' (`models.wesl`).

import bevy_pbr::render::mesh_view_bindings::view;

/// How far the camera stands from the point it looks at (`iso::DISTANCE`).
const DISTANCE: f32 = 20000.0;
/// How far forward a place comes, in world units, per world unit that its cell lies right of
/// the point the camera looks at: of two cells on one diagonal, the original draws the one of
/// greater `cx` later, and it lies further right. Relative to that point, so that the shift
/// stays within a few units on screen.
const ACROSS: f32 = 0.002;
/// How far forward per place in a cell: the original draws a cell's chain of statics in order,
/// then its models. 32 places stay within the step between two cells of a diagonal, 76 units
/// apart on screen.
const CHAIN: f32 = 0.0045;

/// The reversed depth of the point `offset` units in front of `anchor`'s place in the order
/// (`anchor.w` its place in the cell).
fn depth(anchor: vec4<f32>, offset: f32) -> f32 {
    // Taken relative to the point the camera looks at: the world's coordinates run to hundreds
    // of thousands of units, and a depth computed from them in full carries rounding of
    // hundredths of a unit that changes as the camera moves.
    let right = view.world_from_view[0].xyz;
    let back = view.world_from_view[2].xyz;
    let relative = anchor.xyz - (view.world_position - back * DISTANCE);
    let toward = dot(relative, back) + ACROSS * dot(relative, right) + CHAIN * anchor.w + offset;
    // Reversed depth of an orthographic projection: the view's z times the depth per unit of it,
    // plus the far plane's offset, nearer greater; the looked-at point's depth first, so that
    // the small part keeps its precision.
    let per_unit = view.clip_from_view[2][2];
    let looked_at = view.clip_from_view[3][2] - DISTANCE * per_unit;
    return looked_at + toward * per_unit;
}
