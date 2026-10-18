// The night's darkness (`daynight::Night`): the frame, drawn, multiplied by the colour the
// original gives the world by night (docs/re/daynight.md), in its 8-bit values, as the original
// multiplies the art by its vertices' colour.

import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

@group(0) @binding(0) var screen: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;

struct Night {
    /// The darkness, the dark's level, the day's part out of 1, the light sources' count.
    params: vec4<f32>,
    /// The original's screen pixels per unit of the texture coordinates across and down.
    scale: vec4<f32>,
    /// The light sources: texture coordinates, and radius in the original's pixels.
    lights: array<vec4<f32>, 32>,
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
    // The light's level: the dark's, and each source's radius less its distance, at most 223
    // each and 255 in all (`0x41B510`).
    var level = night.params.y;
    let count = u32(night.params.w);
    for (var i = 0u; i < count; i++) {
        let source = night.lights[i];
        let reach = source.z - length((in.uv - source.xy) * night.scale.xy);
        if reach >= 1.0 {
            level += min(reach, 223.0);
        }
    }
    level = min(level, 255.0);
    // The night's ramp of the level (`0x417F00`), with the day's part added to each channel, up
    // to white.
    let base = 64.0 + 64.0 * night.params.x;
    let ramp = vec3(base + level / 4.0, base + level / 4.0, base + level / 3.0) / 255.0;
    let light = min(ramp + night.params.z, vec3(1.0));
    return vec4(to_linear(to_srgb(colour.rgb) * light), colour.a);
}
