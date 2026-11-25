// The fragment stage of the game's extensions of Bevy's own material (`models::ModelMaterial`,
// `blended::Sorted`, `fx::Effect`): its colour encoded for the frame, which holds sRGB values
// (`srgb::Frame`). A model's second pass (`models::Pass::Fringe`) keeps the translucent texels
// that the original's alpha test keeps (`FRINGE`). A model's fragment (`NEAREST`) takes only the
// samples over which no model's texel lies nearer (`nearest.wesl`), and is lit by the fixed
// function's sum in the frame's values rather than Bevy's physically based light (`FIXED_LIGHT`,
// docs/re/drawing.md, "The models' light", #3692), in the world's colour at its place, which the
// night's pass then leaves by the frame's alpha (`LEAVES_NIGHT`, #3693); its other passes lay its
// effect under its texels
// (`EFFECT`, `HOLOGRAM`) or draw it in the water form's look or a vision's (`WATER`, `VISION`).
// A particle's colour (`fx::Effect`, `PARTICLES`) is encoded before its alpha premultiplies an
// added one, and a system's added and laid layers drawn as one mesh are premultiplied each by its
// own (`MIXED`); one of list 4 (`LIST_4`, `fx::over_all`) takes the samples over which no model's
// texel lies nearer than its true point. A model drawn apart from the world by a camera of its own, whose frame takes linear
// light (`model_effects::ApartMaterial`, `LINEAR`), keeps its colour linear, its effect laid
// under it in sRGB values all the same.
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
@if(LINEAR || VISION || FIXED_LIGHT)
import bevy_render::color_operations::{linear_to_srgb, srgb_to_linear};
@if(FIXED_LIGHT)
import bevy_pbr::render::{mesh_functions, mesh_view_bindings::lights};
@if(NEAREST && DEPTH_PREPASS)
import bevy_pbr::render::mesh_view_bindings::depth_prepass_texture;
@if(NEAREST && DEPTH_PREPASS)
import package::nearest;

/// The alpha of a model's opaque texels, which its first pass draws (`models::OPAQUE`).
const OPAQUE: f32 = 254.5 / 255.0;
/// The original's alpha test of a model's texels, which drops those of alpha 64 or less
/// (`models::CUTOFF`).
const CUTOFF: f32 = 64.5 / 255.0;

/// The device's default material's specular power (`models::specular`).
const SPECULAR_POWER: f32 = 30.0;
/// The world's ambient light, of which a model takes its share `s` (docs/re/drawing.md, "The
/// models' light"): 0.7 times the world's colour at its place.
const AMBIENT: f32 = 0.7;
/// Every effect's light's range and its quadratic attenuation, in the models' units (`0x642B90`,
/// `fx_lights`).
const EFFECT_RANGE: f32 = 200.0;
const EFFECT_QUADRATIC: f32 = 0.000125;
/// How much nearer the camera a particle of list 4 is laid than its true point
/// (`statics::LIST_4_FORWARD`, `fx::Quads::ahead`).
const LIST_4_FORWARD: f32 = 1000.0;

/// Each model's light by its mesh's tag (`models::Anchors`): its ambient share `s` of the world's
/// colour in `x`, 0.8 for an object and 0.3 for a creature, pulsing under the cursor (`hover`;
/// #1460, #3108), and the world's colour at its place `g` in `yzw`, white by day.
@if(FIXED_LIGHT)
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var<storage, read> slot_lights: array<vec4<f32>>;
/// The effects' lights that shine (`fx_lights`, at most 7), two entries each: its place in the
/// world with the reciprocal of the models' scale, then its colour, a colour of 0 shining none.
@if(FIXED_LIGHT)
@group(constants::MATERIAL_BIND_GROUP) @binding(109) var<storage, read> effect_lights: array<vec4<f32>>;

/// The effect's texture and the glow's under a model's texels, their offsets and the glow's share
/// of its colour, in `x` (`models::Pass::Effect`, `model_effects`; #1834, #1848); in the effect's
/// place, the texture of the water form's look or of a vision's (`models::Pass::Water`,
/// `models::Pass::Vision`).
@if(EFFECT || WATER || VISION)
@group(constants::MATERIAL_BIND_GROUP) @binding(103) var effect_texture: texture_2d<f32>;
@if(EFFECT || WATER || VISION)
@group(constants::MATERIAL_BIND_GROUP) @binding(104) var effect_sampler: sampler;
@if(EFFECT)
@group(constants::MATERIAL_BIND_GROUP) @binding(105) var glow_texture: texture_2d<f32>;
@if(EFFECT)
@group(constants::MATERIAL_BIND_GROUP) @binding(106) var glow_sampler: sampler;
@if(EFFECT)
@group(constants::MATERIAL_BIND_GROUP) @binding(107) var<uniform> effect_offsets: vec4<f32>;
@if(EFFECT)
@group(constants::MATERIAL_BIND_GROUP) @binding(108) var<uniform> effect_glows: vec4<f32>;

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
    var world = in.world_position.xyz;
@if(LIST_4) {
    world = nearest::further(world, LIST_4_FORWARD);
}
    out.samples = nearest_samples(in.position.xy, world);
    if out.samples == 0u {
        discard;
    }
}
    var pbr_input = pbr_input_from_standard_material(in, is_front);
@if(HOLOGRAM) {
    // The kind 8's texture is added whole, whatever its alpha (`ONE`, `ONE`).
    pbr_input.material.base_color.a = 1.0;
}
@if(WATER) {
    // The water form's look (`models::Pass::Water`, `0x40BD70`): the texture taken by the
    // model's own normal, in the model's frame, its x and its y the texture's coordinates, as the
    // original hands its vertices' normals to the device as their texture's; white, its alpha
    // tested as a model's texels are.
    let world_from_local = mesh_functions::get_world_from_local(in.instance_index);
    let turn = mat3x3<f32>(
        normalize(world_from_local[0].xyz),
        normalize(world_from_local[1].xyz),
        normalize(world_from_local[2].xyz),
    );
    let local = transpose(turn) * normalize(in.world_normal);
    // glTF's frame is the model's turned by −90° about x: the model's y is its −z.
    let texel = textureSample(effect_texture, effect_sampler, vec2<f32>(local.x, -local.z));
    if texel.a < CUTOFF {
        discard;
    }
    pbr_input.material.base_color = texel;
}
@if(VISION) {
    // A vision's look (`models::Pass::Vision`, `0x40C320`, #2519): the model's texel times its
    // grey, its alpha tested, then times twice the vision's texture taken by the model's own
    // normal as the water form's, in the frame's sRGB values as the original's stages modulate
    // them (`MODULATE`, `MODULATE2X`), added whole to the frame and unlit.
    let world_from_local = mesh_functions::get_world_from_local(in.instance_index);
    let turn = mat3x3<f32>(
        normalize(world_from_local[0].xyz),
        normalize(world_from_local[1].xyz),
        normalize(world_from_local[2].xyz),
    );
    let local = transpose(turn) * normalize(in.world_normal);
    let pattern = textureSample(effect_texture, effect_sampler, vec2<f32>(local.x, -local.z));
    let texel = pbr_input.material.base_color;
    if texel.a < CUTOFF {
        discard;
    }
    let added = min(2.0 * linear_to_srgb(texel.rgb) * linear_to_srgb(pattern.rgb), vec3(1.0));
    pbr_input.material.base_color = vec4<f32>(srgb_to_linear(added), 1.0);
}
    // The texture's alpha, over a model's effect, before an opaque pass makes it whole.
    let over = pbr_input.material.base_color.a;
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
@if(FIXED_LIGHT) {
    // The fixed function's light of the model (docs/re/drawing.md, "The models' light"): the
    // world's ambient share `0.7·s` and the sun's diffuse `max(n·l, 0)`, both in the world's
    // colour `g` at its place, and each effect's light's diffuse in its colour, falling by
    // `1 / (1 + 0.000125 d²)` within 200 of its own place (`0x642B90`); each channel held at 1,
    // times the texel in the frame's sRGB values, as the device modulates them. Where the draw
    // turns the specular on (`models::specular`, `SPECULARENABLE`), the Blinn-Phong term of the
    // default material's power 30 in each light's colour is added after the texture. The normal is
    // the vertices' own, unflipped, as the device takes it; the sun is the world's directional
    // light, its direction the original's.
        let slot = slot_lights[mesh_functions::get_tag(in.instance_index)];
        let g = slot.yzw;
        let n = normalize(in.world_normal);
        let sun = lights.directional_lights[0].direction_to_light;
        var light = g * (AMBIENT * slot.x + max(dot(n, sun), 0.0));
        var highlight = vec3<f32>(0.0);
@if(SPECULAR) {
        highlight = g * pow(max(dot(n, normalize(sun + pbr_input.V)), 0.0), SPECULAR_POWER);
}
        for (var i = 0u; i < arrayLength(&effect_lights) / 2u; i++) {
            let place = effect_lights[2u * i];
            let colour = effect_lights[2u * i + 1u].rgb;
            let to = place.xyz - in.world_position.xyz;
            let reach = length(to);
            let d = reach * place.w;
            if d < EFFECT_RANGE && any(colour > vec3<f32>(0.0)) && reach > 0.0 {
                let l = to / reach;
                let fall = 1.0 / (1.0 + EFFECT_QUADRATIC * d * d);
                light += colour * max(dot(n, l), 0.0) * fall;
@if(SPECULAR) {
                let lit = pow(max(dot(n, normalize(l + pbr_input.V)), 0.0), SPECULAR_POWER);
                highlight += colour * lit * fall;
}
            }
        }
        let texel = linear_to_srgb(pbr_input.material.base_color.rgb);
        let frame = min(texel * min(light, vec3<f32>(1.0)) + highlight, vec3<f32>(1.0));
        out.color = vec4<f32>(srgb_to_linear(frame), pbr_input.material.base_color.a);
}
@else {
        out.color = apply_pbr_lighting(pbr_input);
}
    } else {
        out.color = pbr_input.material.base_color;
    }
@if(PARTICLES) {
    // A particle's colour in the frame's values, then times its alpha where Bevy's last steps
    // premultiply an added one: the texel's and the colour's values laid or added as they are,
    // as the original's device blends its particles (`SRCALPHA` with `INVSRCALPHA` or `ONE`,
    // `fx::Particles`); one added whole takes no alpha (`ONE`, `ONE`).
@if(WHOLE) {
    out.color = vec4<f32>(out.color.rgb, 1.0);
}
    out.color = main_pass_post_lighting_processing(pbr_input, encode(out.color));
@if(MIXED) {
    // A system's added and laid layers in one mesh, in their particles' slots' order, as the
    // original switches the blend at each change of layer (`0x761B00`, `fx::mixed_material`),
    // premultiplied: a particle's colour times its alpha, an added one's (its alpha negative)
    // taking nothing of the frame away, a laid one's covering it by its alpha.
    let alpha = abs(out.color.a);
    out.color = vec4<f32>(out.color.rgb * alpha, select(alpha, 0.0, out.color.a < 0.0));
}
}
@else {
@if(LINEAR) {
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
}
@else {
    out.color = encode(main_pass_post_lighting_processing(pbr_input, out.color));
}
}
// Under the texture the effect's colour, the glow's added to it, both unlit, each moved by its
// offsets, laid in the frame's sRGB values as the original's passes blend them (`0x406510`).
@if(EFFECT) {
    let effect = encode(textureSample(effect_texture, effect_sampler, in.uv + effect_offsets.xy));
    let glow = encode(textureSample(glow_texture, glow_sampler, in.uv + effect_offsets.zw));
    let under = min(effect.rgb + glow.rgb * effect_glows.x, vec3(1.0));
@if(HOLOGRAM) {
    // The kind 8's: the effect blended at half its alpha, the texture added over it, laid as one
    // premultiplied colour (`models::Pass::Hologram`).
    let half = effect.a * 0.5;
    out.color = vec4(min(effect.rgb * half + out.color.rgb, vec3(1.0)), half);
}
@else {
@if(LINEAR) {
    out.color = vec4(srgb_to_linear(mix(under, linear_to_srgb(out.color.rgb), over)), 1.0);
}
@else {
    out.color = vec4(mix(under, out.color.rgb, over), 1.0);
}
}
}
// A lit model took the world's colour at its place in its light (`daynight`, #3693): the night's
// pass leaves its pixel, the frame's alpha the share of a pixel that it still darkens; of a kind
// 1's, the effect laid unlit under its texels, the share that the texture leaves.
@if(LEAVES_NIGHT) {
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
@if(EFFECT) {
        out.color.a = 1.0 - over;
}
@else {
        out.color.a = 0.0;
}
    }
}
    return out;
}
