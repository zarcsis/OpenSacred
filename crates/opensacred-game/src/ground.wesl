// The ground's fragment shader (`terrain::GroundMaterial`): the atlas's texel as stored, in sRGB
// values, times the corners' light, as the original multiplies the 8-bit texel by the diffuse
// colour of its corners (docs/02 §5.3); the product is decoded to linear light for the frame.

import bevy_pbr::render::forward_io::VertexOutput;
import package::post::post_process;

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var atlas: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var atlas_sampler: sampler;

/// sRGB values to linear light.
fn decode(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3(2.4)), c / 12.92, c <= vec3(0.04045));
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(atlas, atlas_sampler, in.uv);
    return post_process(vec4(decode(texel.rgb * in.color.rgb), 1.0), in.position.xy);
}
