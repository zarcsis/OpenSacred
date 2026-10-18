// The night's darkness (`daynight::Night`): the frame, drawn, multiplied by the colour the
// original gives the world by night (docs/re/daynight.md), in its 8-bit values, as the original
// multiplies the art by its vertices' colour.

import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

@group(0) @binding(0) var screen: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;

struct Night {
    /// The night ramp's red, green and blue, and the part of the day's white, each out of 1.
    colour: vec4<f32>,
};
@group(0) @binding(2) var<uniform> night: Night;

fn to_srgb(linear: vec3<f32>) -> vec3<f32> {
    let low = linear * 12.92;
    let high = 1.055 * pow(linear, vec3(1.0 / 2.4)) - 0.055;
    return select(high, low, linear <= vec3(0.0031308));
}

fn to_linear(srgb: vec3<f32>) -> vec3<f32> {
    let low = srgb / 12.92;
    let high = pow((srgb + 0.055) / 1.055, vec3(2.4));
    return select(high, low, srgb <= vec3(0.04045));
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let colour = textureSample(screen, screen_sampler, in.uv);
    // The ramp with the day's part added to each channel, up to white.
    let light = min(night.colour.rgb + night.colour.a, vec3(1.0));
    return vec4(to_linear(to_srgb(colour.rgb) * light), colour.a);
}
