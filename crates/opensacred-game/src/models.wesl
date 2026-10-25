// The models' vertex shader (`models::ModelMaterial`): a model at its cell's place in the
// original's order of drawing, among the sprites (docs/re/drawing.md, "The models"). Every vertex
// takes the depth of the model's anchor, plus a small part of its own depth from it, which keeps
// the model's parts in their order among themselves; the sprites, at their cells' depths, then
// cover a model whole or not at all. A model standing on an upper storey's floor lies no further
// back than the floor's plane where it shows (`on_floor`).
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
/// height, and its place in the cell. The order takes the centre at height 0, as it does the
/// sprites' (`order.wesl`); the model's own depth is measured from the centre at its height.
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<storage, read> anchors: array<vec4<f32>>;

/// For each model standing on an upper storey's floor (`statics::Floor::under`), by the mesh's
/// tag: the height of the floor's plane, how far behind the ground point under a point of the
/// plane the model lies at most, and 1 last; for a model under a liquid's surface
/// (`models::Sunk`), how far back along the line of sight its place lies, third; zero for any
/// other model.
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var<storage, read> floors: array<vec4<f32>>;

/// The part of a vertex's own depth from its model's anchor that it keeps: a model about 80 units
/// deep stays within half a unit of its place, near the places of the sprites of its own cell and
/// its neighbours on the diagonal (0.15 units apart) and far from the next diagonal's (33 units),
/// while its parts keep a tenth of a unit's steps of the depth buffer apart.
const SQUEEZE: f32 = 0.01;
/// The part of its own depth that a vertex keeps on a floor's plane (`on_floor`): a model stays
/// within a few tenths of a unit of its place among the quads standing on the floor, while its
/// parts keep half a unit's steps of the depth buffer apart.
const ON_FLOOR_SQUEEZE: f32 = 0.002;

/// The depth no further back than which a vertex at `world` of a model standing on an upper
/// storey's floor lies: the floor's plane where the vertex shows, as the floor's own quads have it
/// (`statics::Spot::anchor_at`), brought forward to the model's place among the quads standing on
/// the floor (`floor.y`), and a little of the vertex's own depth `own`, which keeps the model's
/// parts in their order. The original draws the model after the floor, which tests the depth in
/// its plane, and the model above the plane shows whole (docs/re/drawing.md, "Depth"). Per
/// vertex, which the models' small triangles make nearly exact: the plane's depth is linear
/// across a triangle, so the triangle stays in front of it between its vertices too.
fn on_floor(world: vec3<f32>, floor: vec4<f32>, own: f32) -> f32 {
    let back = view.world_from_view[2].xyz;
    // The point of the plane on the vertex's line of sight, and the ground point under it.
    let plane = world + back * ((floor.x - world.y) / back.y);
    let under = vec3(plane.x, 0.0, plane.z) - back * floor.y;
    return order::depth(vec4(under, 0.0), ON_FLOOR_SQUEEZE * own);
}

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
    // The anchor's place, under a liquid's surface as far back as its sunk models lie, and the
    // vertex's own depth from the anchor, squeezed; on a floor, no further back than its plane.
    let tag = mesh_functions::get_tag(vertex_no_morph.instance_index);
    let anchor = anchors[tag];
    let floor = floors[tag];
    let back = view.world_from_view[2].xyz;
    let place = vec4(vec3(anchor.x, 0.0, anchor.z) - back * floor.z, anchor.w);
    let own = dot(out.world_position.xyz - anchor.xyz, back);
    out.position.z = order::depth(place, SQUEEZE * own);
    if floor.w > 0.5 {
        out.position.z = max(out.position.z, on_floor(out.world_position.xyz, floor, own));
    }

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
