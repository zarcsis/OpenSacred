// The glow of the health's red ring (`low_health`, `0x732220`): the part of the picture drawn
// again at the offsets −6 to +6 by 2 on both axes, 49 times, each copy's colours by its alpha
// times the copies' alpha, their sum added onto the screen. The node is the part with 6 pixels
// about it; the picture's texels hold the frame's sRGB values, which the original adds.

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

struct Glow {
    // The part of the picture drawn: its corner and its size in texels.
    part: vec4<f32>,
    // The alpha of each copy.
    alpha: f32,
}

@group(1) @binding(0) var<uniform> glow: Glow;
@group(1) @binding(1) var picture: texture_2d<f32>;
@group(1) @binding(2) var picture_sampler: sampler;

const REACH: i32 = 6;
const STEP: i32 = 2;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(picture));
    let reach = f32(REACH);
    // The point in the part's texels.
    let at = in.uv * (glow.part.zw + 2.0 * reach) - reach;
    var light = vec3(0.0);
    for (var dy = -REACH; dy <= REACH; dy += STEP) {
        for (var dx = -REACH; dx <= REACH; dx += STEP) {
            let source = at - vec2(f32(dx), f32(dy));
            if (all(source >= vec2(0.0)) && all(source < glow.part.zw)) {
                let texel = textureSampleLevel(
                    picture,
                    picture_sampler,
                    (glow.part.xy + source) / size,
                    0.0,
                );
                light += texel.rgb * texel.a;
            }
        }
    }
    return vec4(light * glow.alpha, 0.0);
}
