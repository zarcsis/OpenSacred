// The flash over a newly taught art on the spells page (`spells_page`, `0x6C2D50`): a frame of the
// texture's 4 × 4, its part's corner and size in the texture's units, added onto the screen
// (`SRCBLEND` and `DESTBLEND` `ONE`).

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

@group(1) @binding(0) var<uniform> part: vec4<f32>;
@group(1) @binding(1) var picture: texture_2d<f32>;
@group(1) @binding(2) var picture_sampler: sampler;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSampleLevel(picture, picture_sampler, part.xy + in.uv * part.zw, 0.0);
    return vec4(texel.rgb, 0.0);
}
