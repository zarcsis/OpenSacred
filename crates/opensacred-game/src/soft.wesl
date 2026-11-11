// The models' soft shadows (#1436, `shadows::Kind::Soft`): the sun's shadow map, which the models
// that cast a shadow draw into (`shadows::Casts`), darkens the ground and the models where it
// shades them, leaving the same third of the light as the original's flattened shadows
// (`shadows.wesl`), so that the two differ in their shape and softness alone. The sun is the one
// directional light that casts (`shadows::sun`); while none does, nothing is darkened.
// Shared by the models' fragment stage (`standard.wesl`), the ground's (`ground.wesl`), the upper
// storeys' floors' (`sprites.wesl`) and the effects' laid on the ground (#2605).

import bevy_pbr::render::{
    mesh_view_bindings::{lights, view},
    mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT,
    shadows::fetch_directional_shadow,
};
import bevy_render::color_operations::{linear_to_srgb, srgb_to_linear};

/// The light that a shadow leaves, as the original's black at two thirds' alpha does
/// (`shadows.wesl`, `DARKNESS`).
const LEFT: f32 = 1.0 / 3.0;

/// The share of the light at the point `world` of a surface of normal `normal`, drawn at the
/// pixel `frag`: 1 in the sun, a third in its full shadow, between them across the shadow's soft
/// edge; 1 while no light casts.
fn lit(world: vec4<f32>, normal: vec3<f32>, frag: vec2<f32>) -> f32 {
    let view_z = dot(
        vec4<f32>(
            view.view_from_world[0].z,
            view.view_from_world[1].z,
            view.view_from_world[2].z,
            view.view_from_world[3].z,
        ),
        world,
    );
    for (var i = 0u; i < lights.n_directional_lights; i ++) {
        if (lights.directional_lights[i].flags & DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u {
            return mix(LEFT, 1.0, fetch_directional_shadow(i, world, normal, view_z, frag));
        }
    }
    return 1.0;
}

/// The share of the light on the level plane at `height`, where the line of sight through the
/// point `world` meets it, drawn at the pixel `frag`: the place of a flat thing that the order of
/// drawing moved along the line of sight (a floor's sprite, an effect on the ground). The plane's
/// normal bias is the ground's.
fn lit_on_level(world: vec3<f32>, height: f32, frag: vec2<f32>) -> f32 {
    let back = view.world_from_view[2].xyz;
    let place = world - back * ((world.y - height) / back.y);
    return lit(vec4(place, 1.0), vec3(0.0, NORMAL_BIAS, 0.0), frag);
}

/// A point of the world on the line of sight of the pixel at `frag` (its position, its depth
/// any), for a quad of the screen's plane, whose world positions are not its texels' places.
fn on_pixel(frag: vec4<f32>) -> vec3<f32> {
    let ndc = vec2(
        (frag.x - view.viewport.x) / view.viewport.z * 2.0 - 1.0,
        1.0 - (frag.y - view.viewport.y) / view.viewport.w * 2.0,
    );
    let point = view.world_from_clip * vec4(ndc, frag.z, 1.0);
    return point.xyz / point.w;
}

/// The colour `linear`, in linear light, darkened to the share `share` of its sRGB values, as the
/// original's shadows darken the frame's.
fn darkened(linear: vec3<f32>, share: f32) -> vec3<f32> {
    return srgb_to_linear(linear_to_srgb(linear) * share);
}

/// The share of the sun's normal bias that a level surface takes: the models' own, against their
/// shading themselves, would lift its samples off their feet (`ground.wesl`).
const NORMAL_BIAS: f32 = 0.2;
