// What Bevy's own materials do to their colour last, for a camera without HDR: tone mapping and
// debanding (`main_pass_post_lighting_processing` of `bevy_pbr`), so that the game's materials
// match them.

@if(TONEMAP_IN_SHADER) import bevy_core_pipeline::tonemapping::{tone_mapping, screen_space_dither};
@if(TONEMAP_IN_SHADER) import bevy_pbr::render::mesh_view_bindings::view;
@if(TONEMAP_IN_SHADER) import bevy_render::maths::powsafe;

/// The colour in linear light as the frame takes it, at the fragment's position.
fn post_process(colour: vec4<f32>, frag_coord: vec2<f32>) -> vec4<f32> {
    var out = colour;
@if(TONEMAP_IN_SHADER) {
    out = tone_mapping(out, view.color_grading);
@if(DEBAND_DITHER) {
    var rgb = powsafe(out.rgb, 1.0 / 2.2);
    rgb += screen_space_dither(frag_coord);
    out = vec4(powsafe(rgb, 2.2), out.a);
}
}
    return out;
}
