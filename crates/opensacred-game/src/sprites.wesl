// The static objects' shader (`statics::SpriteMaterial`): a sprite's quads in the plane of the
// screen at the depth of their cell, brought forward a little in the original's order of drawing,
// so that sprites at the same depth do not fight (docs/re/drawing.md).

import bevy_pbr::render::{
    mesh_functions,
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip,
};
import package::post::post_process;

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var atlas: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var atlas_sampler: sampler;

/// How far the camera stands from the point it looks at (`iso::DISTANCE`).
const DISTANCE: f32 = 20000.0;
/// How far forward a sprite comes, in world units, per world unit that its cell lies right of
/// the point the camera looks at: of two cells on one diagonal, the original draws the one of
/// greater `cx` later, and it lies further right. Relative to that point, so that the shift
/// stays within a few units on screen.
const ACROSS: f32 = 0.002;
/// How far forward per place in its cell's chain: the original draws a cell's chain in order. 32
/// places stay within the step between two cells of a diagonal, 76 units apart on screen.
const CHAIN: f32 = 0.0045;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    /// The centre of the sprite's cell, at its height and its draw list's depth, and its place
    /// in the cell's chain.
    @location(2) anchor: vec4<f32>,
    /// The part of the atlas the piece may sample: its source half a texel in from each edge.
    @location(3) source: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) source: vec4<f32>,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let world = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4(vertex.position, 1.0),
    );
    var out: VertexOutput;
    out.position = position_world_to_clip(world.xyz);
    // The depth of the anchor, the same for every vertex of a sprite and of every sprite of a
    // cell, taken relative to the point the camera looks at: the world's coordinates run to
    // hundreds of thousands of units, and a depth computed from them in full carries rounding
    // of hundredths of a unit that changes as the camera moves.
    let right = view.world_from_view[0].xyz;
    let back = view.world_from_view[2].xyz;
    let relative = vertex.anchor.xyz - (view.world_position - back * DISTANCE);
    let toward = dot(relative, back) + ACROSS * dot(relative, right) + CHAIN * vertex.anchor.w;
    // Reversed depth of an orthographic projection: the view's z times the depth per unit of it,
    // plus the far plane's offset, nearer greater; the looked-at point's depth first, so that
    // the small part keeps its precision.
    let per_unit = view.clip_from_view[2][2];
    let looked_at = view.clip_from_view[3][2] - DISTANCE * per_unit;
    out.position.z = looked_at + toward * per_unit;
    out.uv = vertex.uv;
    out.source = vertex.source;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Within the piece's own texels: past its edge the atlas holds other pieces, and the
    // piece's texture coordinates map its texels one to one, so the edge's texels reach to
    // the piece's border instead of being blended with their neighbours in the atlas.
    var colour = textureSample(atlas, atlas_sampler, clamp(in.uv, in.source.xy, in.source.zw));
    // Sharpened edges for alpha to coverage, as Bevy's own materials have them, around a cutoff
    // of a half.
    @if(ALPHA_TO_COVERAGE)
    colour.a = (colour.a - 0.5) / max(fwidth(colour.a), 0.0001) + 0.5;
    // Without multisampling, alpha to coverage falls back to a mask, as Bevy's own materials do.
    @if(MAY_DISCARD)
    if colour.a < 0.5 {
        discard;
    }
    @if(MAY_DISCARD)
    colour.a = 1.0;
    return post_process(colour, in.position.xy);
}
