// The effects' vertex shader (`fx::Effect`): the quads of the particle systems, which the game lays
// at their depth in the original's order on the CPU (`fx::Quads::in_order`), a quad drawn over a
// model 0.02 units of depth before the model's surface (`fx::Orders::over`). An orthographic view
// takes their depth relative to the point the camera looks at, as the models' and the sprites'
// (`order.wesl`): from the world's coordinates in full, hundreds of thousands of units, it carried
// rounding of hundredths of a unit, as much as that margin, and a weapon's glow flickered over and
// under its blade as the blade moved (#3935). The effects' meshes are neither skinned nor morphed.
//
// Bevy's own vertex stage (`bevy_pbr::render::mesh`, MIT OR Apache-2.0) but for the depth.

import bevy_pbr::render::{
    mesh_functions,
    mesh_view_bindings::view,
    forward_io::{Vertex, VertexOutput, decompress_vertex},
    view_transformations::position_world_to_clip,
};
import package::order;

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);

@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        vertex_in.instance_index
    );

    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);
    // An orthographic view's depth (its projection's `w` 1) from the mesh's origin and the vertex's
    // offset from it, apart.
    if view.clip_from_view[3][3] == 1.0 {
        let offset = (world_from_local * vec4<f32>(vertex.position, 0.0)).xyz;
        out.position.z = order::point(world_from_local[3].xyz, offset);
    }

@if(VERTEX_UVS_A)
    out.uv = vertex.uv;
@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;

@if(VERTEX_TANGENTS)
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local,
        vertex.tangent,
        vertex_in.instance_index
    );

@if(VERTEX_COLORS)
    out.color = vertex.color;

@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;

@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex_in.instance_index, world_from_local[3]);

    return out;
}
