// The static objects' shader (`statics::SpriteMaterial`): a sprite's quads in the plane of the
// screen at the depth of their cell, brought forward a little in the original's order of drawing,
// so that sprites at the same depth do not fight (docs/re/drawing.md), and placed on screen by the
// original's pixels, so that abutting sprites meet (`on_screen`). A sprite is drawn twice
// (docs/decisions/2026-10-18-sprite-alpha.md): its opaque texels through the depth buffer, then the
// others (`FRINGE`) blended by their alpha over what lies behind them, as the original blends
// them, a sprite's texels of alpha 64 or less dropped (`ALPHA_TEST`).

import bevy_pbr::render::mesh_view_bindings::view;
import package::{magnify, order, post::post_process};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var atlas: texture_2d<f32>;
/// What the texels are multiplied by (`statics::SpriteMaterial::tint`): white, but for the
/// statics' shadows, which fade by night.
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var<uniform> tint: vec4<f32>;
/// What the sprites of the fading types fade about (`statics::Fade`): the hero's point on the
/// original's screen, the radius and the alpha there; then its place in the order.
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var<storage, read> fade: array<vec4<f32>>;

struct Vertex {
    /// The vertex in the world, from its sprite's cell: unused here (`on_screen`).
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    /// The centre of the sprite's cell, at height 0 and its draw list's depth, and its place in
    /// the cell's chain.
    @location(2) anchor: vec4<f32>,
    /// The piece's texels in the atlas: its first column and row, and its last.
    @location(3) source: vec4<f32>,
    /// The vertex on the original's screen, in its pixels at zoom 1.
    @location(4) screen: vec2<f32>,
    /// 1 for a sprite of a type that fades about the hero.
    @location(5) fades: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) source: vec4<f32>,
    /// The point on the original's screen.
    @location(2) screen: vec2<f32>,
    /// 1 where the sprite fades about the hero: of a fading type, drawn after the hero.
    @location(3) @interpolate(flat) fades: f32,
};

/// The alpha of the opaque texels, which the first pass draws: all of it, the atlases' 4 bits of
/// alpha at 15.
const OPAQUE: f32 = 254.5 / 255.0;
/// The original's alpha test, which drops a sprite's texels of alpha 64 or less: `ALPHAREF` 64,
/// `ALPHAFUNC` `GREATER` (`0x643190`). The miniatures are drawn without it.
const CUTOFF: f32 = 64.0 / 255.0;
/// The least alpha that the second pass blends.
const CLEAR: f32 = 0.5 / 255.0;

/// Screen pixels at zoom 1 per world unit (`iso::PIXELS_PER_UNIT`): a cell's diagonal is 96 pixels.
const PIXELS_PER_UNIT: f32 = 1.2649111;

/// Where the vertex at `screen`, a point of the original's screen in its pixels at zoom 1, lies
/// in clip space. Taken from the screen's pixels rather than the vertex's world position: two
/// sprites whose edges meet at one pixel of the original then put their edges at one place, where
/// their world positions, worked out from their own cells, rounded apart and could leave a column
/// of pixels between them (#113). The camera's projection is orthographic and centred.
fn on_screen(screen: vec2<f32>) -> vec2<f32> {
    let right = view.world_from_view[0].xyz;
    let up = view.world_from_view[1].xyz;
    let back = view.world_from_view[2].xyz;
    // The point the camera looks at, on the original's screen, the same for every vertex.
    let looked_at = view.world_position - back * order::DISTANCE;
    let centre = vec2(dot(looked_at, right), -dot(looked_at, up)) * PIXELS_PER_UNIT;
    // World units right and up of that point, the view's x and y.
    let along = vec2(screen.x - centre.x, centre.y - screen.y) / PIXELS_PER_UNIT;
    return vec2(
        view.clip_from_view[0][0] * along.x + view.clip_from_view[3][0],
        view.clip_from_view[1][1] * along.y + view.clip_from_view[3][1],
    );
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    // The depth of the anchor's place in the order, the same for every vertex of a sprite. The
    // mesh's world positions, from the sprite's cell, serve Bevy's culling and order alone.
    out.position = vec4(on_screen(vertex.screen), order::depth(vertex.anchor, 0.0), 1.0);
    out.uv = vertex.uv;
    out.source = vertex.source;
    out.screen = vertex.screen;
    // Drawn after the hero: nearer than its place in the reversed depth.
    let after = order::depth(vertex.anchor, 0.0) > order::depth(fade[1], 0.0);
    out.fades = select(0.0, 1.0, vertex.fades > 0.5 && after);
    return out;
}

/// The atlas's texel at `at`, held within the piece's texels, its colour weighted by its alpha;
/// none where the alpha test drops it, as the original tests each texel before it blends.
fn texel(at: vec2<i32>, source: vec4<i32>, alpha: f32) -> vec4<f32> {
    let texel = textureLoad(atlas, clamp(at, source.xy, source.zw), 0);
    // The vertices' alpha times the texel's, as the original's texture stage modulates them,
    // before its alpha test.
    let a = texel.a * alpha;
@if(ALPHA_TEST) {
    if a <= CUTOFF {
        return vec4(0.0);
    }
}
    return vec4(texel.rgb * a, a);
}

/// The atlas at `uv`, filtered linearly between the piece's four nearest texels, their colours
/// weighted by their alphas: a transparent texel, black in the atlases, adds nothing to the colour
/// of its neighbours, which a sampler's filtering darkened. Past the piece's edge the atlas holds
/// other pieces; its edge's texels stand in for them.
fn filtered(uv: vec2<f32>, source: vec4<i32>, alpha: f32) -> vec4<f32> {
    let at = uv * vec2<f32>(textureDimensions(atlas)) - 0.5;
    let corner = floor(at);
    let t = magnify::weights(at);
    let i = vec2<i32>(corner);
    let top = mix(texel(i, source, alpha), texel(i + vec2(1, 0), source, alpha), t.x);
    let bottom = mix(
        texel(i + vec2(0, 1), source, alpha),
        texel(i + vec2(1, 1), source, alpha),
        t.x,
    );
    return mix(top, bottom, t.y);
}

/// The alpha out of 1 of a fading sprite's point at `screen` (`0x62FF60`): within the radius of
/// the hero's point across and down, from the alpha at the point to 1 at the radius' distance;
/// else 1. Per point here, where the original gives each corner of its sprites' pieces of 64
/// pixels this alpha and lets the rasterizer blend it between them
/// (docs/decisions/2026-10-19-tree-fade.md). The same as `statics::TreeFade::at`.
fn faded(screen: vec2<f32>) -> f32 {
    let point = fade[0].xy;
    let radius = fade[0].z;
    let offset = screen - point;
    if radius <= 0.0 || abs(offset.x) >= radius || abs(offset.y) >= radius {
        return 1.0;
    }
    let t = min(length(offset) / radius, 1.0);
    return (fade[0].w + (255.0 - fade[0].w) * t) / 255.0;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let alpha = select(1.0, faded(in.screen), in.fades > 0.5);
    let weighted = filtered(in.uv, vec4<i32>(in.source), alpha);
    var colour = vec4(weighted.rgb / max(weighted.a, 0.0001), weighted.a) * tint;
@if(FRINGE) {
    if colour.a >= OPAQUE || colour.a < CLEAR {
        discard;
    }
}
@else {
    if colour.a < OPAQUE {
        discard;
    }
    colour.a = 1.0;
}
    return post_process(colour, in.position.xy);
}
