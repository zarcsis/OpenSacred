// The models' soft shadows (#1436, `shadows::Kind::Soft`): the sun's shadow map, which the models
// that cast a shadow draw into (`shadows::Casts`), darkens the ground and the models where it
// shades them, leaving the same third of the light as the original's flattened shadows
// (`shadows.wesl`), so that the two differ in their shape and softness alone. The sun is the one
// directional light that casts (`shadows::sun`); while none does, nothing is darkened.
// Shared by the models' fragment stage (`standard.wesl`) and the ground's (`ground.wesl`).

import bevy_pbr::render::{
    mesh_view_bindings::{lights, view},
    mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT,
    shadows::fetch_directional_shadow,
};

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
