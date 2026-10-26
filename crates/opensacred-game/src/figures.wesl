// The heroes' figures of the choice of the hero (`choice`), drawn by their own camera into a
// texture that holds linear light, laid on the interface in the frame's sRGB values
// (`srgb::Frame`): each texel unmultiplied from the coverage of the edges, then encoded.

import bevy_ui_render::ui_vertex_output::UiVertexOutput;
import bevy_render::color_operations::linear_to_srgb;

@group(1) @binding(0) var figures: texture_2d<f32>;
@group(1) @binding(1) var figures_sampler: sampler;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(figures, figures_sampler, in.uv);
    if (texel.a <= 0.0) {
        return vec4(0.0);
    }
    return vec4(linear_to_srgb(texel.rgb / texel.a), texel.a);
}
