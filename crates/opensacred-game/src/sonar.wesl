// The red alarm's ring about the hero's portrait (`low_health`, `0x6D70F0`, `0x75FC70`): the
// texture's top row laid across the ring, its left at the outer edge and its right at the inner,
// tinted and blended by its alpha. The original's ring is 90 straight pieces of 4°.

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

struct Ring {
    inner: f32,
    outer: f32,
    colour: vec4<f32>,
}

@group(1) @binding(0) var<uniform> ring: Ring;
@group(1) @binding(1) var sonar: texture_2d<f32>;
@group(1) @binding(2) var sonar_sampler: sampler;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    // The node is the ring's square, 2 × its outer radius a side.
    let distance = length((in.uv - 0.5) * 2.0 * ring.outer);
    if (distance < ring.inner || distance > ring.outer) {
        return vec4(0.0);
    }
    let across = (ring.outer - distance) / (ring.outer - ring.inner);
    let texel = textureSampleLevel(sonar, sonar_sampler, vec2(across, 0.0), 0.0);
    return texel * ring.colour;
}
