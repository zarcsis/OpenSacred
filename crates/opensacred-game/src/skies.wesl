// The weather drawn over the world (`skies`, docs/re/weather.md, "What is drawn and heard"):
// the fog's two layers of `WEATHER_FOG01`, the rain's drops and the lightning's flash, in the
// frame's sRGB values (`srgb::Frame`), over the world and under the interface. The colour is
// premultiplied by its alpha, as the material blends it, so that the flash adds to the frame.
//
// The rain's drops lie anew at random each frame, as the original's: the screen is cut into
// cells of `CELL` of its pixels, each of which holds its share of the drops, placed and sized by
// a hash of the frame's seed and the cell, so that a pixel looks only at the drops of the cells
// about it.

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

struct Params {
    /// The original's screen across and down in its pixels, the camera's zoom (the fog's span)
    /// and the frame's seed.
    screen: vec4<f32>,
    /// The rain's drops and their slant across per pixel down.
    rain: vec4<f32>,
    /// The fog's alpha and its first layer's offset.
    fog: vec4<f32>,
    /// The fog's second layer's offset.
    fog2: vec4<f32>,
    /// The light's colour, and the grey the lightning's flash adds.
    tint: vec4<f32>,
}

@group(1) @binding(0) var<uniform> params: Params;
@group(1) @binding(1) var fog_texture: texture_2d<f32>;
@group(1) @binding(2) var fog_sampler: sampler;

/// The side of a cell of the rain, in the original's pixels: longer than a drop and its slant.
const CELL: f32 = 32.0;
/// The most drops a cell holds, whatever the rain.
const MOST: u32 = 12u;

/// A hash of three numbers (PCG), as a number from 0 to 1.
fn hash(a: u32, b: u32, c: u32) -> f32 {
    var v = a * 747796405u + b * 2891336453u + c * 277803737u + 2654435769u;
    v = v ^ (v >> 16u);
    v = v * 2246822519u;
    v = v ^ (v >> 13u);
    v = v * 3266489917u;
    v = v ^ (v >> 16u);
    return f32(v) / 4294967295.0;
}

/// `src` laid over `dst`, both of straight alpha.
fn over(dst: vec4<f32>, src: vec4<f32>) -> vec4<f32> {
    let a = src.a + dst.a * (1.0 - src.a);
    if (a <= 0.0) {
        return vec4(0.0);
    }
    return vec4((src.rgb * src.a + dst.rgb * dst.a * (1.0 - src.a)) / a, a);
}

/// The rain's alpha at the pixel `p`: of the drops whose line passes within half a pixel of it,
/// each fading in from its top, the most.
fn rain(p: vec2<f32>) -> f32 {
    let drops = params.rain.x;
    if (drops <= 0.0) {
        return 0.0;
    }
    let share = drops * CELL * CELL / (params.screen.x * params.screen.y);
    let seed = u32(params.screen.w);
    let here = vec2<i32>(floor(p / CELL));
    var alpha = 0.0;
    for (var dy = -1; dy <= 0; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let cell = here + vec2(dx, dy);
            let cx = u32(cell.x + 4096);
            let cy = u32(cell.y + 4096);
            let count = min(u32(floor(share)) + select(0u, 1u, hash(cx, cy, seed) < fract(share)), MOST);
            for (var i = 0u; i < count; i++) {
                let key = seed * 16u + i;
                let start = (vec2<f32>(cell) + vec2(hash(cx, key, 1u), hash(cy, key, 2u))) * CELL;
                let length = 5.0 + 15.0 * hash(cx + cy, key, 3u);
                let line = vec2(params.rain.y * length, length);
                let t = clamp(dot(p - start, line) / dot(line, line), 0.0, 1.0);
                if (distance(p, start + line * t) < 0.5) {
                    alpha = max(alpha, t);
                }
            }
        }
    }
    return alpha;
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    var colour = vec4(0.0);
    // The fog: two layers of its texture, the screen spanning the zoom of it, each moved by the
    // camera and its drift, of the fog's alpha and the light's colour.
    if (params.fog.x > 0.0) {
        for (var layer = 0; layer < 2; layer++) {
            let offset = select(params.fog2.xy, params.fog.yz, layer == 0);
            let texel = textureSample(fog_texture, fog_sampler, offset + in.uv * params.screen.z);
            colour = over(colour, vec4(texel.rgb * params.tint.rgb, texel.a * params.fog.x));
        }
    }
    // The rain's drops, of the light's colour.
    let drop = rain(in.uv * params.screen.xy);
    if (drop > 0.0) {
        colour = over(colour, vec4(params.tint.rgb, drop));
    }
    // Premultiplied, and the lightning's flash added over all.
    return vec4(colour.rgb * colour.a + vec3(params.tint.a), colour.a);
}
