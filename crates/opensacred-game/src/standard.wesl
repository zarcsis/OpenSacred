// The fragment stage of the game's extensions of Bevy's own material (`models::ModelMaterial`,
// `blended::Sorted`): its colour encoded for the frame, which holds sRGB values (`srgb::Frame`).
// A model's second pass (`models::Pass::Fringe`) keeps the translucent texels that the original's
// alpha test keeps (`FRINGE`). A model's fragment (`NEAREST`) takes only the samples over which
// no model's texel lies nearer (`nearest.wesl`).
//
// Bevy's own fragment stage (`bevy_pbr::render::pbr`, MIT OR Apache-2.0) but for the encoding,
// without the meshlets, the decals and the order-independent transparency, which the game does
// not use.

import bevy_pbr::render::{
    forward_io::VertexOutput,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
};
import package::post::encode;
@if(NEAREST && DEPTH_PREPASS)
import bevy_pbr::render::mesh_view_bindings::depth_prepass_texture;
@if(NEAREST && DEPTH_PREPASS)
import package::nearest;

/// The alpha of a model's opaque texels, which its first pass draws (`models::OPAQUE`).
const OPAQUE: f32 = 254.5 / 255.0;
/// The original's alpha test of a model's texels, which drops those of alpha 64 or less
/// (`models::CUTOFF`).
const CUTOFF: f32 = 64.5 / 255.0;

/// The fragment's colour and, for a model's, the samples it takes.
struct Output {
    @location(0) color: vec4<f32>,
    @if(NEAREST && DEPTH_PREPASS)
    @builtin(sample_mask) samples: u32,
}

/// The samples of the pixel at `position` over which no model's texel lies nearer than the point
/// `world` of this one, as a mask: the prepass holds the nearest model's depth over each sample
/// (`prepass.wesl`). The point is the pixel's centre; its depth over the samples may differ from
/// it by half its change across the pixel.
@if(NEAREST && DEPTH_PREPASS)
fn nearest_samples(position: vec2<f32>, world: vec3<f32>) -> u32 {
    let own = nearest::depth(world);
    let slack = 0.5 * fwidth(own) + nearest::tolerance();
    let pixel = vec2<i32>(position);
@if(MULTISAMPLED) {
    var samples = 0u;
    for (var i = 0u; i < textureNumSamples(depth_prepass_texture); i ++) {
        if !nearest::nearer(textureLoad(depth_prepass_texture, pixel, i32(i)), own, slack) {
            samples |= 1u << i;
        }
    }
    return samples;
}
@else {
    if nearest::nearer(textureLoad(depth_prepass_texture, pixel, 0), own, slack) {
        return 0u;
    }
    return 0xFFFFFFFFu;
}
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> Output {
    var out: Output;
@if(NEAREST && DEPTH_PREPASS) {
    out.samples = nearest_samples(in.position.xy, in.world_position.xyz);
    if out.samples == 0u {
        discard;
    }
}
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color
    );
@if(FRINGE) {
    let alpha = pbr_input.material.base_color.a;
    if alpha >= OPAQUE || alpha < CUTOFF {
        discard;
    }
}
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }
    out.color = encode(main_pass_post_lighting_processing(pbr_input, out.color));
    return out;
}
