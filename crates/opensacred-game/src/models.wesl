// The models' vertex shader (`models::ModelMaterial`): a model at its cell's place in the
// original's order of drawing, among the sprites (docs/re/drawing.md, "The models"). Every vertex
// takes the depth of the model's anchor, plus a small part of its own depth from it, which keeps
// the model's parts in their order among themselves; the sprites, at their cells' depths, then
// cover a model whole or not at all.
//
// Bevy's own vertex stage (`bevy_pbr::render::mesh`, MIT OR Apache-2.0) but for the depth.

import bevy_pbr::render::{
    mesh_bindings::mesh,
    mesh_functions,
    mesh_view_bindings::view,
    skinning,
    morph::{morph_position, morph_normal, morph_tangent},
    forward_io::{Vertex, UncompressedVertex, VertexOutput, decompress_vertex},
    view_transformations::position_world_to_clip,
};
import package::order;

/// Each model's anchor (`models::Anchors`), by the mesh's tag: the centre of its cell at the cell's
/// height, and its place in the cell.
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<storage, read> anchors: array<vec4<f32>>;

/// The part of a vertex's own depth from its model's anchor that it keeps: a model about 80 units
/// deep stays within half a unit of its place, near the places of the sprites of its own cell and
/// its neighbours on the diagonal (0.15 units apart) and far from the next diagonal's (33 units),
/// while its parts keep a tenth of a unit's steps of the depth buffer apart.
const SQUEEZE: f32 = 0.01;

@if(MORPH_TARGETS)
fn morph_vertex(vertex_in: UncompressedVertex, instance_index: u32) -> UncompressedVertex {
    var vertex = vertex_in;
    let first_vertex = mesh[instance_index].first_vertex_index;
    let vertex_index = vertex.index - first_vertex;
    let morph_descriptor_index = mesh[instance_index].morph_descriptor_index;
    let weight_count = bevy_pbr::render::morph::layer_count(morph_descriptor_index);
    for (var i: u32 = 0u; i < weight_count; i ++) {
        let weight = bevy_pbr::render::morph::weight_at(i, morph_descriptor_index);
        if weight == 0.0 {
            continue;
        }
        vertex.position += weight * morph_position(vertex_index, i, morph_descriptor_index);
        @if(VERTEX_NORMALS)
        vertex.normal += weight * morph_normal(vertex_index, i, morph_descriptor_index);
        @if(VERTEX_TANGENTS)
        vertex.tangent += vec4(weight * morph_tangent(vertex_index, i, morph_descriptor_index), 0.0);
    }
    return vertex;
}

@vertex
fn vertex(vertex_no_morph: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let uncompressed_vertex_no_morph = decompress_vertex(vertex_no_morph, vertex_no_morph.instance_index);
@if(MORPH_TARGETS)
    var vertex = morph_vertex(uncompressed_vertex_no_morph, vertex_no_morph.instance_index);
@else
    var vertex = uncompressed_vertex_no_morph;

    let mesh_world_from_local = mesh_functions::get_world_from_local(vertex_no_morph.instance_index);

@if(SKINNED)
    var world_from_local = skinning::skin_model(
        vertex.joint_indices,
        vertex.joint_weights,
        vertex_no_morph.instance_index
    );
@else
    var world_from_local = mesh_world_from_local;

@if(VERTEX_NORMALS && SKINNED)
    out.world_normal = skinning::skin_normals(world_from_local, vertex.normal);
@if(VERTEX_NORMALS && !SKINNED)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        vertex_no_morph.instance_index
    );

    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);
    // The anchor's place, and the vertex's own depth from the anchor, squeezed.
    let anchor = anchors[mesh_functions::get_tag(vertex_no_morph.instance_index)];
    let back = view.world_from_view[2].xyz;
    out.position.z = order::depth(anchor, SQUEEZE * dot(out.world_position.xyz - anchor.xyz, back));

@if(VERTEX_UVS_A)
    out.uv = vertex.uv;
@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;

@if(VERTEX_TANGENTS)
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local,
        vertex.tangent,
        vertex_no_morph.instance_index
    );

@if(VERTEX_COLORS)
    out.color = vertex.color;

@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_no_morph.instance_index;

@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex_no_morph.instance_index, mesh_world_from_local[3]);

    return out;
}
