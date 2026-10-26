// The models' true depths, which the original's depth buffer holds alone (docs/re/drawing.md,
// "The models"): the models' prepass draws each model's opaque texels at them (`prepass.wesl`),
// and over each sample the models' main pass keeps only the nearest model (`standard.wesl`),
// which takes its place among the sprites by its cell (`models.wesl`). The true depth is squeezed
// into the farthest part of the range (`PART`), behind all that the main pass draws, which starts
// on the prepass's depth buffer.
// Shared by the models' prepass and the fragment stage of their main pass.

import bevy_pbr::render::{
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip,
};

/// The part of the reversed depth's range that the models' true depths take, from the far plane:
/// a sixteenth, 1,250 units of the camera's 20,000, over 3,000 behind the ground
/// (`iso::GROUND_BEHIND`), the farthest of what the main pass draws.
const PART: f32 = 0.0625;
/// How much nearer than a model's own depth a model's texel must lie over a sample to take it, in
/// units of the world: the two passes' roundings of one point's depth, far smaller, stay within.
const TOLERANCE: f32 = 0.5;

/// The prepass's depth of the point `world` of a model: its true reversed depth, squeezed into the
/// models' part of the range.
fn depth(world: vec3<f32>) -> f32 {
    return PART * position_world_to_clip(world).z;
}

/// [`TOLERANCE`] in the prepass's depth.
fn tolerance() -> f32 {
    return TOLERANCE * PART * abs(view.clip_from_view[2][2]);
}

/// Whether the prepass's depth `prepass` over a sample is a model's texel, nearer than `own`, a
/// model's own depth there, by more than `slack`. A depth beyond the models' part is not a
/// model's: Bevy's own material, the bolts' and the souls' spheres, writes its true depth.
fn nearer(prepass: f32, own: f32, slack: f32) -> bool {
    return prepass > own + slack && prepass <= PART;
}
