// The models' shadows' shader (`shadows::ShadowMaterial`): a model's mesh flattened onto the
// plane at its root's height along the original's light (`cGranny::renderShadow`, `0x406FF0`),
// black at its texture's alpha, every pixel of one shadow darkened once (docs/re/drawing.md,
// "The models' shadows"; docs/decisions/drawing.md, "Models' shadows by depth, not stencil"), its
// edges soft by the mask of every shadow blurred about the pixel (#2624; "The models' shadows'
// soft edges"), whose copies the same vertex stage flattens and `mask` draws white.
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

/// Each model's anchor (`models::Anchors`), by the mesh's tag, as the models' shader takes it.
@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<storage, read> anchors: array<vec4<f32>>;
/// Each model's root (`shadows::Grounds`), by the mesh's tag: the plane of its shadow is at its
/// height, as the original's is at the object's (`+0xF0`).
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var<storage, read> grounds: array<vec4<f32>>;
/// Each model's floor (`models::Anchors`), by the mesh's tag, as the models' shader takes it: for
/// a model under a liquid's surface, how far back its place lies, third.
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var<storage, read> floors: array<vec4<f32>>;
/// Where any model's shadow lies (`shadows::ShadowMask`): white on black, the size of the view.
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var shadow_mask: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(4) var shadow_mask_sampler: sampler;

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
/// How far a shadow's vertices move out along their normals before they are flattened, in world
/// units, so that the blurred edge beyond the silhouette is drawn too: two spreads on the screen
/// where the ground is foreshortened by half, and some to spare. The mask's copies stay as they
/// are.
const REACH: f32 = 5.0;

struct Flattened {
    @builtin(position) position: vec4<f32>,
    /// The model's anchor.
    @location(0) @interpolate(flat) anchor: vec4<f32>,
    /// The height of the shadow's plane.
    @location(1) @interpolate(flat) ground: f32,
    /// How far back the model's place lies under a liquid's surface.
    @location(2) @interpolate(flat) sunk: f32,
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

    var world = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
@if(SOFT_EDGES && VERTEX_NORMALS)
    {
        let normal = (world_from_local * vec4(vertex.normal, 0.0)).xyz;
        if dot(normal, normal) > 0.0 {
            world = vec4(world.xyz + normalize(normal) * REACH, 1.0);
        }
    }
    let tag = mesh_functions::get_tag(vertex_no_morph.instance_index);
    var out: Flattened;
    out.ground = grounds[tag].y;
    out.anchor = anchors[tag];
    out.sunk = floors[tag].z;
    out.position = position_world_to_clip(flatten(world.xyz, out.ground));
    return out;
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

/// The share of the pixels about `pixel` that a shadow covers: the mask blurred by a Gaussian of
/// [`SOFTNESS`], sampled over a grid of 7 × 7 across two spreads each way, between its pixels.
/// It is a half on a straight edge and one a few pixels within. A pixel whose mask is the same
/// two spreads away on every side, well within a shadow or well out of every one, has no edge
/// near, and the grid is spared.
fn coverage(pixel: vec2<f32>) -> f32 {
    // The projection is orthographic: as many pixels to a unit across as up.
    let per_unit = view.clip_from_view[1][1] * view.viewport.w * 0.5;
    let step = SOFTNESS * per_unit * (2.0 / 3.0) / view.viewport.zw;
    let at = (pixel - view.viewport.xy) / view.viewport.zw;
    let far = 3.0 * step;
    let probes = masked(at) + masked(at + vec2(far.x, 0.0)) + masked(at - vec2(far.x, 0.0))
        + masked(at + vec2(0.0, far.y)) + masked(at - vec2(0.0, far.y));
    if probes >= 4.999 {
        return 1.0;
    }
    if probes <= 0.001 {
        return 0.0;
    }
    var sum = 0.0;
    var weights = 0.0;
    for (var i = -3; i <= 3; i++) {
        for (var j = -3; j <= 3; j++) {
            let offset = vec2(f32(i), f32(j));
            let weight = exp(-0.5 * dot(offset, offset) * (4.0 / 9.0));
            sum += weight * masked(at + offset * step);
            weights += weight;
        }
    }
    return sum / weights;
}

@fragment
fn fragment(in: Flattened) -> Shade {
    var out: Shade;
    // Half at the silhouette's edge, full a few pixels within, none a few pixels beyond, where the
    // vertices moved out along their normals still reach.
    out.colour = vec4(0.0, 0.0, 0.0, DARKNESS * coverage(in.position.xy));
    out.depth = plane_depth(in.position.y, in.anchor, in.ground, in.sunk);
    return out;
}

/// The shadows' copies in the mask (`shadows::MaskMaterial`): white wherever one lies.
@fragment
fn mask() -> @location(0) vec4<f32> {
    return vec4(1.0);
}
