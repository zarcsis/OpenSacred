// The veil's measure of depth (`veil`, #3851): how far toward the camera, in world units from the
// point it looks at, a fragment lies, from its depth in the frame, which the veil's camera writes
// for the fading sprites' faded texels (`sprites.wesl`'s `fragment_veil`) and the effects drawn
// over the night compare their own with (`standard.wesl`'s `VEILED`). Taken from the depth's
// scale alone, as the cameras that share it have one projection's depth: the world's coordinates
// run to hundreds of thousands of units, and the veil's half floats keep a unit's precision only
// within a few thousand of the point looked at.

import bevy_pbr::render::mesh_view_bindings::view;
import package::order;

/// How far toward the camera, in world units from the point it looks at, a fragment of the depth
/// `depth` lies: the orthographic projection's depth is linear, its reverse growing toward the
/// camera, by its scale per unit (`clip_from_view[2][2]`).
fn toward(depth: f32) -> f32 {
    let scale = view.clip_from_view[2][2];
    let looked = view.clip_from_view[3][2] - order::DISTANCE * scale;
    return (depth - looked) / scale;
}
