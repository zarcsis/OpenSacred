// A quad of the interface's sparkles (`sparkles`, `0x40B980`): its texture times its colour, added
// onto the screen (`SRCBLEND` and `DESTBLEND` `ONE`). The texels hold the frame's sRGB values,
// which the original adds.

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

@group(1) @binding(0) var<uniform> colour: vec4<f32>;
@group(1) @binding(1) var picture: texture_2d<f32>;
@group(1) @binding(2) var picture_sampler: sampler;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSampleLevel(picture, picture_sampler, in.uv, 0.0);
    return vec4(texel.rgb * colour.rgb, 0.0);
}
