//! A model's vertices as last posed, for the systems whose start bursts their particles at them
//! (`0x767FB0`, `0x44FC30`: the souls' figures, the changeling's, the weapons' sparks): each
//! mesh's vertices in the world, a skinned mesh's by its joints' places and weights as the
//! renderer poses it, its worn items' left out.

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
        ),
    >,
    places: Query<'w, 's, &'static GlobalTransform>,
    worn: crate::making::Worn<'w, 's>,
    poses: Res<'w, Assets<SkinnedMeshInverseBindposes>>,
}

impl ModelPoints<'_, '_> {
    /// The vertices of each mesh of the model at `root` and its descendants, but its worn items',
    /// in the world, mesh by mesh: a skinned mesh's `Σ wᵢ · (jointᵢ · bindᵢ⁻¹) · v`, another's its
    /// entity's transform of `v`; the meshes of `meshes`.
    pub(crate) fn of(&self, root: Entity, meshes: &Assets<Mesh>) -> Vec<Vec<Vec3>> {
        let mut found = Vec::new();
        let mut stack = vec![root];
        while let Some(entity) = stack.pop() {
            if entity != root && self.worn.contains(entity) {
                continue;
            }
            if let Ok((mesh, skin, place)) = self.shapes.get(entity)
                && let Some(points) = self.posed(meshes.get(&mesh.0), skin, place)
            {
                found.push(points);
            }
            if let Ok(kids) = self.children.get(entity) {
                stack.extend(kids.iter());
            }
        }
        found
    }

    /// The vertices of the mesh `mesh` in the world, skinned by `skin` where it is skinned, else
    /// at `place`.
    fn posed(
        &self,
        mesh: Option<&Mesh>,
        skin: Option<&SkinnedMesh>,
        place: &GlobalTransform,
    ) -> Option<Vec<Vec3>> {
        let mesh = mesh?;
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            return None;
        };
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
        let model = self.of(root, meshes);
        let counts: Vec<usize> = model.iter().map(Vec::len).collect();
        opensacred_rules::particles::vertex_picks(&counts)
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
    fn the_bounds_take_some_eleven_vertices_of_a_mesh() {
        assert_eq!(
            [5, 12, 22, 23, 32, 33, 1100].map(bounds_step),
            [1, 2, 2, 3, 3, 3, 100]
        );
    }
}
