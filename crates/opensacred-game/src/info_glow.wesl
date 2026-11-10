// The glow of an info line's text (`info_lines`, `0x723020`): the line's glyphs drawn again 64
// times, at the offsets ±1, ±3, ±5 and ±7 of the original's pixels on both axes, each copy's
// coverage times the glow's colour added onto the screen (`SRCALPHA`, `ONE`). The node is the
// line with 7 pixels about it; the picture holds the glyphs' coverage in its alpha, `scale` of
// its texels a pixel; the colour is of the frame's sRGB values, which the original adds.

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

struct Glow {
    // The glow's colour, of the frame's values.
    colour: vec4<f32>,
    // The picture's texels a pixel, in `x`.
    scale: vec4<f32>,
}

@group(1) @binding(0) var<uniform> glow: Glow;
@group(1) @binding(1) var picture: texture_2d<f32>;
@group(1) @binding(2) var picture_sampler: sampler;

const REACH: i32 = 7;
const STEP: i32 = 2;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(picture));
    let scale = glow.scale.x;
    let reach = f32(REACH) * scale;
    // The point in the picture's texels.
    let at = in.uv * (size + 2.0 * reach) - reach;
    var coverage = 0.0;
    for (var dy = -REACH; dy <= REACH; dy += STEP) {
        for (var dx = -REACH; dx <= REACH; dx += STEP) {
            let source = at - vec2(f32(dx), f32(dy)) * scale;
            if (all(source >= vec2(0.0)) && all(source < size)) {
                coverage += textureSampleLevel(picture, picture_sampler, source / size, 0.0).a;
            }
        }
    }
    return vec4(glow.colour.rgb * coverage, 0.0);
}
