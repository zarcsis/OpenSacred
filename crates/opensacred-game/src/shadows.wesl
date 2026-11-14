// The models' shadows' shader (`shadows::ShadowMaterial`): a model's mesh flattened onto the
// plane at its root's height along the original's light (`cGranny::renderShadow`, `0x406FF0`),
// black at its texture's alpha, every pixel of one shadow darkened once (docs/re/drawing.md,
// "The models' shadows"; docs/decisions/drawing.md, "Models' shadows by depth, not stencil"), its
// edges soft by the mask of every shadow blurred about the pixel (#2624; "The models' shadows'
// soft edges"), whose copies the same vertex stage flattens and `mask` draws, the nearest's
// caster's number kept where they overlap, so that on a model a shadow's edge is blurred by the
// silhouettes of the shadows that fall on it alone (#2722).
//
// The vertex stage is Bevy's own (`bevy_pbr::render::mesh`, MIT OR Apache-2.0) but for the
// flattening, as the models' is (`models.wesl`).

import bevy_pbr::render::{
    mesh_bindings::mesh,
    mesh_functions,
    mesh_view_bindings::view,
    skinning,
    morph::morph_position,
    forward_io::{Vertex, UncompressedVertex, decompress_vertex},
    view_transformations::position_world_to_clip,
};
import package::order;
@if(DEPTH_PREPASS)
import bevy_pbr::render::mesh_view_bindings::depth_prepass_texture;

/// Each model's anchor (`models::Anchors`), by the mesh's tag, as the models' shader takes it.
@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<storage, read> anchors: array<vec4<f32>>;
/// Each model's root (`shadows::Grounds`), by the mesh's tag: the plane of its shadow is at its
/// height, as the original's is at the object's (`+0xF0`).
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var<storage, read> grounds: array<vec4<f32>>;
/// Each model's floor (`models::Anchors`), by the mesh's tag, as the models' shader takes it: for
/// a model under a liquid's surface, how far back its place lies, third.
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var<storage, read> floors: array<vec4<f32>>;
/// Where any model's shadow lies (`shadows::ShadowMask`), the size of the view: its red whole
/// there and none elsewhere, its green and blue the nearest shadow's caster's number.
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var shadow_mask: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(4) var shadow_mask_sampler: sampler;
/// Its x 1 for the soft edges, at the highest detail, 0 for the original's sharp ones (#2628).
@group(constants::MATERIAL_BIND_GROUP) @binding(5) var<uniform> soft: vec4<f32>;

/// How far a point moves along X on the plane per unit of its height above it: the original's
/// shadow takes a point `z` above it to `(x + z/2, y − z/2)` in its frame of the models, `x`
/// right on the screen and `y` toward the viewer (`shadows::tests`).
const LEAN_X: f32 = 0.036930;
/// How far along Z, the same way.
const LEAN_Z: f32 = -0.670176;
/// The part of its own depth that a model keeps (`models.wesl`).
const SQUEEZE: f32 = 0.01;
/// How far behind the plane the shadow lies, in units of the model's own depth: the model's
/// soles, on the plane, cover it, as the original draws a model over its shadow.
const BEHIND: f32 = 1.0;
/// The shadow's alpha: its texture's (`WEATHER_RAIN01.TGA`, black at 170 everywhere) times its
/// vertices' (opaque black), 170 / 255.
const DARKNESS: f32 = 0.6666667;
/// The spread of a shadow's edge, in world units: the blur's standard deviation, a pixel and a
/// half at zoom 1 (1.2649 pixels a unit), so that the edge ramps over some four pixels, about as
/// the statics' stretched silhouettes do, and scales with the zoom as they do.
const SOFTNESS: f32 = 1.2;
/// How far a shadow's vertices at its silhouette move out of it on the screen, in spreads, so that
/// the blurred edge beyond the silhouette is drawn too: two spreads, and some to spare. Out of
/// it is down the mask's slope at the vertex, whatever the mesh's normals: a flat, one-sided part
/// (a blade, a wing) grows on both sides (#2630). The mask's copies stay as they are.
const REACH: f32 = 2.5;
/// How far apart the mask is read about a vertex for its slope, in pixels.
const SLOPE_STEP: f32 = 1.5;

struct Flattened {
    @builtin(position) position: vec4<f32>,
    /// The model's anchor.
    @location(0) @interpolate(flat) anchor: vec4<f32>,
    /// The height of the shadow's plane.
    @location(1) @interpolate(flat) ground: f32,
    /// How far back the model's place lies under a liquid's surface.
    @location(2) @interpolate(flat) sunk: f32,
    /// Its caster's number in the mask: the slot of its anchors, plus one.
    @location(3) @interpolate(flat) caster: u32,
};

struct Shade {
    @location(0) colour: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@if(MORPH_TARGETS)
fn morph_vertex(vertex_in: UncompressedVertex, instance_index: u32) -> UncompressedVertex {
    var vertex = vertex_in;
    let first_vertex = mesh[instance_index].first_vertex_index;
    let vertex_index = vertex.index - first_vertex;
    let morph_descriptor_index = mesh[instance_index].morph_descriptor_index;
    let weight_count = bevy_pbr::render::morph::layer_count(morph_descriptor_index);
    for (var i: u32 = 0u; i < weight_count; i ++) {
        let weight = bevy_pbr::render::morph::weight_at(i, morph_descriptor_index);
        if weight == 0.0 {
            continue;
        }
        vertex.position += weight * morph_position(vertex_index, i, morph_descriptor_index);
    }
    return vertex;
}

/// The point where the light through `world` meets the plane at height `ground`.
fn flatten(world: vec3<f32>, ground: f32) -> vec3<f32> {
    let height = world.y - ground;
    return vec3(world.x + LEAN_X * height, ground, world.z + LEAN_Z * height);
}

@vertex
fn vertex(vertex_no_morph: Vertex) -> Flattened {
    let uncompressed_vertex_no_morph = decompress_vertex(vertex_no_morph, vertex_no_morph.instance_index);
@if(MORPH_TARGETS)
    var vertex = morph_vertex(uncompressed_vertex_no_morph, vertex_no_morph.instance_index);
@else
    var vertex = uncompressed_vertex_no_morph;

@if(SKINNED)
    var world_from_local = skinning::skin_model(
        vertex.joint_indices,
        vertex.joint_weights,
        vertex_no_morph.instance_index
    );
@else
    var world_from_local = mesh_functions::get_world_from_local(vertex_no_morph.instance_index);

    let world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    let tag = mesh_functions::get_tag(vertex_no_morph.instance_index);
    var out: Flattened;
    out.ground = grounds[tag].y;
    out.anchor = anchors[tag];
    out.sunk = floors[tag].z;
    out.caster = tag + 1u;
    out.position = position_world_to_clip(flatten(world.xyz, out.ground));
@if(SOFT_EDGES)
    {
        out.position = outward(out.position);
    }
    return out;
}

/// Pixels on the screen to a world unit: the projection is orthographic, as many across as up.
fn per_unit() -> f32 {
    return view.clip_from_view[1][1] * view.viewport.w * 0.5;
}

/// The vertex at `clip` moved [`REACH`] spreads out of the silhouette, down the mask's slope
/// about it, where the mask slopes: the vertices within a shadow, where it is level, stay.
fn outward(clip: vec4<f32>) -> vec4<f32> {
    if soft.x < 0.5 {
        return clip;
    }
    let ndc = clip.xy / clip.w;
    let at = vec2(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    let step = SLOPE_STEP / view.viewport.zw;
    let slope = vec2(
        masked(at + vec2(step.x, 0.0)) - masked(at - vec2(step.x, 0.0)),
        masked(at + vec2(0.0, step.y)) - masked(at - vec2(0.0, step.y)),
    );
    let steep = length(slope);
    if steep < 0.05 {
        return clip;
    }
    // In pixels, down the screen; then in the clip space's units, up it.
    let away = -slope / steep * REACH * SOFTNESS * per_unit();
    return vec4(clip.xy + vec2(away.x, -away.y) * 2.0 / view.viewport.zw * clip.w, clip.zw);
}

/// The depth of the plane at height `ground` in the pixel row `row`, from the row alone: every
/// triangle of a shadow over a pixel gives it the same depth to the bit, and the first one drawn
/// there keeps the others out, as the original's stencil does (`0x643470`, state `0x10000`).
/// The model's own depth of that point, as the models' shader takes it, squeezed, at the model's
/// place, `sunk` units further back under a liquid's surface.
fn plane_depth(row: f32, anchor: vec4<f32>, ground: f32, sunk: f32) -> f32 {
    let up = view.world_from_view[1].xyz;
    let back = view.world_from_view[2].xyz;
    // The row's height in the view, in world units: the projection is orthographic.
    let ndc = 1.0 - 2.0 * (row - view.viewport.y) / view.viewport.w;
    let along = (ndc - view.clip_from_view[3][1]) / view.clip_from_view[1][1];
    // How far in front of the camera the row's lines of sight meet the plane: the camera's right
    // is level, so the column does not matter.
    let ahead = (view.world_position.y + up.y * along - ground) / back.y;
    let own = dot(view.world_position - anchor.xyz, back) - ahead;
    let place = vec4(vec3(anchor.x, 0.0, anchor.z) - back * sunk, anchor.w);
    return order::depth(place, SQUEEZE * (own - BEHIND));
}

/// The mask at `at`, between its pixels.
fn masked(at: vec2<f32>) -> f32 {
    return textureSampleLevel(shadow_mask, shadow_mask_sampler, at, 0.0).r;
}

/// The depth of the nearest shadow's plane at `at` of the view, by the caster's number that the
/// mask holds at its pixel there; none where it holds none.
fn nearest_shadow(at: vec2<f32>) -> f32 {
    let size = vec2<i32>(textureDimensions(shadow_mask));
    let pixel = clamp(vec2<i32>(at * vec2<f32>(size)), vec2(0), size - 1);
    let texel = textureLoad(shadow_mask, pixel, 0);
    let caster = u32(round(texel.g * 255.0)) | (u32(round(texel.b * 255.0)) << 8u);
    if caster == 0u {
        return 0.0;
    }
    let tag = caster - 1u;
    let row = at.y * view.viewport.w + view.viewport.y;
    return plane_depth(row, anchors[tag], grounds[tag].y, floors[tag].z);
}

/// The depth of the model whose texel lies at `at` of the view: the models' prepass holds their
/// depths alone (`prepass.wesl`), none on the ground.
fn model_depth(at: vec2<f32>) -> f32 {
@if(DEPTH_PREPASS) {
    let size = vec2<i32>(textureDimensions(depth_prepass_texture));
    let pixel = clamp(vec2<i32>(at * view.viewport.zw + view.viewport.xy), vec2(0), size - 1);
    // Its first sample, multisampled or not.
    return textureLoad(depth_prepass_texture, pixel, 0);
}
@else {
    return 0.0;
}
}

/// The share of the mask at `at` that the shadows falling on a surface of the depth `receiver`
/// cover. On the ground (`receiver` none), every shadow's silhouette: the shadows join there, and
/// run on under the models that stand in them. On a model, only those of the shadows in front of
/// it, which darken it, as the nearest at `at` decides: the others lie behind the models that hide
/// them, and the edge of a shadow that fell on a model, an open chest, a creature standing in it,
/// stayed sharp within the silhouette of the model's own shadow under it (#2722).
fn covered(at: vec2<f32>, receiver: f32) -> f32 {
    let share = masked(at);
    if share <= 0.0 || receiver <= 0.0 || nearest_shadow(at) > receiver {
        return share;
    }
    return 0.0;
}

/// The share of the pixels about `pixel` that a shadow covers: the mask blurred by a Gaussian of
/// [`SOFTNESS`], sampled over a grid of 9 × 9 across two spreads each way, half a spread apart
/// and between its pixels, so that no pixel of a thin part's mask (a blade a few pixels wide)
/// falls between the samples (#2630). It is a half on a straight edge and one a few pixels
/// within. A pixel whose mask is whole two spreads away every way, the diagonals too, lies well
/// within a shadow, and the grid is spared; one near the edge, within or out, takes it whole. The
/// mask is that of the shadows that fall on what lies at the pixel, the ground or a model
/// ([`covered`]).
fn coverage(pixel: vec2<f32>) -> f32 {
    let spread = SOFTNESS * per_unit();
    let texel = 1.0 / view.viewport.zw;
    let at = (pixel - view.viewport.xy) * texel;
    let receiver = model_depth(at);
    let far = 2.0 * spread * texel;
    var probes = 0.0;
    for (var i = -1; i <= 1; i++) {
        for (var j = -1; j <= 1; j++) {
            probes += covered(at + vec2(f32(i), f32(j)) * far, receiver);
        }
    }
    if probes >= 8.999 {
        return 1.0;
    }
    let step = max(0.5 * spread, 1.0) * texel;
    var sum = 0.0;
    var weights = 0.0;
    for (var i = -4; i <= 4; i++) {
        for (var j = -4; j <= 4; j++) {
            let offset = vec2(f32(i), f32(j));
            let weight = exp(-0.125 * dot(offset, offset));
            sum += weight * covered(at + offset * step, receiver);
            weights += weight;
        }
    }
    return sum / weights;
}

@fragment
fn fragment(in: Flattened) -> Shade {
    var out: Shade;
    // Half at the silhouette's edge, full a few pixels within, none a few pixels beyond, where the
    // vertices moved out of the silhouette still reach; sharp, as the original's, below the
    // highest detail.
    var share = 1.0;
    if soft.x > 0.5 {
        share = coverage(in.position.xy);
    }
    out.colour = vec4(0.0, 0.0, 0.0, DARKNESS * share);
    out.depth = plane_depth(in.position.y, in.anchor, in.ground, in.sunk);
    return out;
}

/// The shadows' copies in the mask (`shadows::MaskMaterial`): wherever one lies, its red whole and
/// its caster's number in its green and blue, at the depth of its plane, which the depth test
/// keeps the nearest's of where they overlap.
@fragment
fn mask(in: Flattened) -> Shade {
    var out: Shade;
    let caster = in.caster & 0xFFFFu;
    out.colour = vec4(1.0, f32(caster & 0xFFu) / 255.0, f32(caster >> 8u) / 255.0, 1.0);
    out.depth = plane_depth(in.position.y, in.anchor, in.ground, in.sunk);
    return out;
}
