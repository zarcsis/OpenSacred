//! A model's vertices as last posed, for the systems whose start bursts their particles at them
//! (`0x767FB0`, `0x44FC30`: the souls' figures, the changeling's, the weapons', the level-up's
//! and the statues' power-up's sparks, the dusts of the bodies' ends), and for the bounds that
//! the original grows as it draws a model (`0x405C90`, #1425; `labels`): each mesh's vertices in
//! the world, a skinned mesh's by its joints' places and weights as the renderer poses it, its
//! worn items' left out. A mesh is the original's, one of its Granny model's: the converter
//! writes it as a glTF mesh of a primitive per pass that share its vertices, which Bevy's loader
//! gives each primitive whole, so the primitives of one mesh count once (#2763).

use bevy::ecs::system::SystemParam;
use bevy::mesh::VertexAttributeValues;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::prelude::*;

/// What a model's vertices are read from.
#[derive(SystemParam)]
pub(crate) struct ModelPoints<'w, 's> {
    children: Query<'w, 's, &'static Children>,
    shapes: Query<
        'w,
        's,
        (
            &'static Mesh3d,
            Option<&'static SkinnedMesh>,
            &'static GlobalTransform,
            Option<&'static ChildOf>,
        ),
    >,
    places: Query<'w, 's, &'static GlobalTransform>,
    worn: crate::making::Worn<'w, 's>,
    poses: Res<'w, Assets<SkinnedMeshInverseBindposes>>,
}

impl ModelPoints<'_, '_> {
    /// The vertices of each mesh of the model at `root` and its descendants, but its worn items',
    /// in the world, mesh by mesh: a skinned mesh's `Σ wᵢ · (jointᵢ · bindᵢ⁻¹) · v`, another's its
    /// entity's transform of `v`; the meshes of `meshes`. The primitives of one glTF mesh, the
    /// children of one node whose vertices are the same, count as one mesh.
    pub(crate) fn of(&self, root: Entity, meshes: &Assets<Mesh>) -> Vec<Vec<Vec3>> {
        let mut found = Vec::new();
        let mut taken: Vec<(Option<Entity>, &[[f32; 3]])> = Vec::new();
        let mut stack = vec![root];
        while let Some(entity) = stack.pop() {
            if entity != root && self.worn.contains(entity) {
                continue;
            }
            if let Ok((mesh, skin, place, parent)) = self.shapes.get(entity)
                && let Some(mesh) = meshes.get(&mesh.0)
                && let Some(VertexAttributeValues::Float32x3(positions)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            {
                let node = parent.map(ChildOf::parent);
                let shared = taken
                    .iter()
                    .any(|&(other, vertices)| other == node && vertices == positions.as_slice());
                if !shared {
                    taken.push((node, positions));
                    if let Some(points) = self.posed(mesh, positions, skin, place) {
                        found.push(points);
                    }
                }
            }
            if let Ok(kids) = self.children.get(entity) {
                stack.extend(kids.iter());
            }
        }
        found
    }

    /// The vertices `positions` of the mesh `mesh` in the world, skinned by `skin` where it is
    /// skinned, else at `place`.
    fn posed(
        &self,
        mesh: &Mesh,
        positions: &[[f32; 3]],
        skin: Option<&SkinnedMesh>,
        place: &GlobalTransform,
    ) -> Option<Vec<Vec3>> {
        let Some(skin) = skin else {
            return Some(
                positions
                    .iter()
                    .map(|&p| place.transform_point(Vec3::from_array(p)))
                    .collect(),
            );
        };
        let poses = self.poses.get(&skin.inverse_bindposes)?;
        let matrices: Vec<Mat4> = skin
            .joints
            .iter()
            .zip(poses.iter())
            .map(|(joint, pose)| {
                self.places
                    .get(*joint)
                    .map_or(Mat4::IDENTITY, |joint| joint.to_matrix() * *pose)
            })
            .collect();
        let Some(VertexAttributeValues::Uint16x4(indices)) =
            mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX)
        else {
            return None;
        };
        let Some(VertexAttributeValues::Float32x4(weights)) =
            mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT)
        else {
            return None;
        };
        Some(
            positions
                .iter()
                .zip(indices.iter().zip(weights))
                .map(|(&p, (index, weight))| {
                    let local = Vec3::from_array(p);
                    index
                        .iter()
                        .zip(weight)
                        .filter_map(|(&joint, &weight)| {
                            matrices
                                .get(usize::from(joint))
                                .map(|matrix| matrix.transform_point3(local) * weight)
                        })
                        .sum()
                })
                .collect(),
        )
    }

    /// The points of the model at `root` by which the original grows its bounds as it draws it
    /// (`0x405C90`, `0x40C790`; #1425): of each mesh of `n` vertices every one where `n` is below
    /// 12, every second below 23, every third below 33, else every `⌊n/11⌋`-th, posed in the world.
    pub(crate) fn bounds_points(&self, root: Entity, meshes: &Assets<Mesh>) -> Vec<Vec3> {
        self.of(root, meshes)
            .into_iter()
            .flat_map(|points| {
                let step = bounds_step(points.len());
                points.into_iter().step_by(step)
            })
            .collect()
    }

    /// The points of the model at `root` that a burst of at most 500 particles takes
    /// (`0x767FB0`, `opensacred_rules::particles::vertex_picks`), in the world.
    pub(crate) fn picked(&self, root: Entity, meshes: &Assets<Mesh>) -> Vec<Vec3> {
        self.picked_at_most(root, meshes, opensacred_rules::particles::GENERIC_PARTICLES)
    }

    /// The points of the model at `root` that a burst of at most `most` particles takes
    /// (`0x767FB0`, `opensacred_rules::particles::vertex_picks_at_most`), in the world.
    pub(crate) fn picked_at_most(
        &self,
        root: Entity,
        meshes: &Assets<Mesh>,
        most: usize,
    ) -> Vec<Vec3> {
        let model = self.of(root, meshes);
        let counts: Vec<usize> = model.iter().map(Vec::len).collect();
        opensacred_rules::particles::vertex_picks_at_most(&counts, most)
            .into_iter()
            .filter_map(|(mesh, vertex)| model.get(mesh)?.get(vertex).copied())
            .collect()
    }
}

/// The step of the vertices of a mesh of `count` that its bounds take ([`ModelPoints::bounds_points`]).
fn bounds_step(count: usize) -> usize {
    match count {
        0..12 => 1,
        12..23 => 2,
        23..33 => 3,
        _ => count / 11,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_primitives_of_one_mesh_count_once() {
        use bevy::asset::RenderAssetUsages;
        use bevy::ecs::system::SystemState;
        use bevy::mesh::PrimitiveTopology;
        let mut world = World::new();
        world.init_resource::<Assets<SkinnedMeshInverseBindposes>>();
        let triangle = |x: f32| {
            Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::all())
                .with_inserted_attribute(
                    Mesh::ATTRIBUTE_POSITION,
                    vec![[x, 0.0, 0.0], [x, 1.0, 0.0], [x, 0.0, 1.0]],
                )
        };
        let mut meshes = Assets::<Mesh>::default();
        // One glTF mesh's two primitives, which hold its vertices each, and another mesh of the
        // node; a mesh of the same vertices under another node.
        let [first, second, other, elsewhere] =
            [0.0, 0.0, 5.0, 0.0].map(|x| meshes.add(triangle(x)));
        world.insert_resource(meshes);
        let place = GlobalTransform::IDENTITY;
        let root = world.spawn(place).id();
        let node = world.spawn((place, ChildOf(root))).id();
        let apart = world.spawn((place, ChildOf(root))).id();
        for (mesh, parent) in [
            (first, node),
            (second, node),
            (other, node),
            (elsewhere, apart),
        ] {
            world.spawn((Mesh3d(mesh), place, ChildOf(parent)));
        }
        let mut state = SystemState::<(ModelPoints, Res<Assets<Mesh>>)>::new(&mut world);
        let (points, meshes) = state.get(&world).expect("the model's points");
        let model = points.of(root, &meshes);
        assert_eq!(model.len(), 3);
        assert_eq!(model.iter().filter(|mesh| mesh[0].x == 0.0).count(), 2);
        // A burst takes every one of their 9 vertices once.
        assert_eq!(points.picked(root, &meshes).len(), 9);
    }

    #[test]
    fn the_bounds_take_some_eleven_vertices_of_a_mesh() {
        assert_eq!(
            [5, 12, 22, 23, 32, 33, 1100].map(bounds_step),
            [1, 2, 2, 3, 3, 3, 100]
        );
    }
}
