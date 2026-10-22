//! The models' shadows (docs/re/drawing.md, "The models' shadows"). The original draws the shadow
//! of a creature or a chest with its model, at its place in the draw lists: the model's mesh,
//! deformed as the model's, flattened onto the plane at the object's height along a fixed light,
//! black at a third of the light left, through the stencil, so that every pixel is darkened once
//! (`cGranny::renderShadow`, `0x406FF0`). Each mesh of a model whose root is [`Casts`] has a copy
//! here that [`ShadowMaterial`] draws so (`shadows.wesl`): at its model's place among the sprites,
//! as the model is, in the shadows' layer (`blended::Layer::Shadows`), over what the
//! original draws before the model and under the model and what it draws after; the depth test
//! keeps a shadow's later triangles off the pixels its first ones darkened, where the original
//! has the stencil (docs/decisions/2026-10-19-model-shadows.md).

use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::mesh::{MeshTag, MeshVertexBufferLayoutRef};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, CompareFunction, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;

use crate::blended::{self, Layer};
use crate::frame::Step;
use crate::models::{Anchors, Fringe, ModelMaterial, Ordered, OrderedWith};

/// The shader of [`ShadowMaterial`], embedded in the binary.
const SHADER: &str = "embedded://opensacred_game/shadows.wesl";

/// The root of a model that casts a shadow: the hero, an NPC whose creature has no flag `0x10`
/// (`NOSHADOW`, `0x426D30`), a script's object (a `CreateObj` record) of category 4, which the
/// client makes a `cChest3D` (`0x451300`, whose draw `0x5C97B0` draws the shadow).
#[derive(Component)]
pub struct Casts;

/// The copy of a model's mesh that draws its shadow.
#[derive(Component)]
pub struct Shadow;

/// The material of the shadows, which every one shares: the models' anchors, for their places,
/// and their roots, for their planes.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct ShadowMaterial {
    #[storage(0, read_only)]
    anchors: Handle<ShaderBuffer>,
    #[storage(1, read_only)]
    grounds: Handle<ShaderBuffer>,
}

impl Material for ShadowMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn depth_bias(&self) -> f32 {
        Layer::Shadows.bias()
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    /// The depth that the fragment shader gives, written, and a pixel taken only where it is
    /// nearer than what lies there: the shadow's own triangles, which give a pixel the same
    /// depth, darken it once.
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        blended::unbias(descriptor);
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_write_enabled = Some(true);
            depth.depth_compare = Some(CompareFunction::Greater);
        }
        Ok(())
    }
}

/// Every model's root by the slot its root holds of the [`Anchors`], for the shader by its
/// meshes' tags: the height of the plane of its shadow.
#[derive(Resource)]
pub struct Grounds {
    buffer: Handle<ShaderBuffer>,
    roots: Vec<Vec4>,
}

impl FromWorld for Grounds {
    fn from_world(world: &mut World) -> Self {
        let buffer = world
            .resource_mut::<Assets<ShaderBuffer>>()
            .add(ShaderBuffer::new(
                vec![[0.0_f32; 4]],
                RenderAssetUsages::default(),
            ));
        Self {
            buffer,
            roots: vec![Vec4::ZERO],
        }
    }
}

/// The shared [`ShadowMaterial`].
#[derive(Resource)]
pub struct Shared(Handle<ShadowMaterial>);

pub fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<ShadowMaterial>>,
    anchors: Res<Anchors>,
    grounds: Res<Grounds>,
) {
    commands.insert_resource(Shared(materials.add(ShadowMaterial {
        anchors: anchors.buffer().clone(),
        grounds: grounds.buffer.clone(),
    })));
}

/// A model's root, of its own place or its holder's.
type Model = Or<(With<Ordered>, With<OrderedWith>)>;
/// A mesh of a model, its parent and what its copy takes of it.
type ModelMesh = (
    Entity,
    &'static Mesh3d,
    Option<&'static SkinnedMesh>,
    &'static MeshTag,
    &'static Transform,
    &'static ChildOf,
);

/// Gives each mesh that has just taken its model material, of a model that casts a shadow, a
/// copy beside it that draws the shadow: the same mesh, skinned to the same joints, tagged with
/// the same slot, shown while the mesh is. A held model, an NPC's weapon, casts none, as the
/// client flattens the creature's own model alone.
pub fn cast(
    mut commands: Commands,
    shared: Res<Shared>,
    meshes: Query<ModelMesh, (Added<MeshMaterial3d<ModelMaterial>>, Without<Fringe>)>,
    parents: Query<&ChildOf>,
    models: Query<(), Model>,
    casters: Query<(), (With<Casts>, With<Ordered>)>,
) {
    for (entity, mesh, skinned, tag, transform, parent) in &meshes {
        let Some(root) = parents
            .iter_ancestors(entity)
            .find(|&ancestor| models.contains(ancestor))
        else {
            continue;
        };
        if !casters.contains(root) {
            continue;
        }
        let mut shadow = commands.spawn((
            Mesh3d(mesh.0.clone()),
            MeshMaterial3d(shared.0.clone()),
            tag.clone(),
            *transform,
            ChildOf(parent.parent()),
            // The flattened mesh reaches past the model's bounds, toward the back of the screen.
            NoFrustumCulling,
            Shadow,
        ));
        if let Some(skinned) = skinned {
            shadow.insert(skinned.clone());
        }
    }
}

/// Keeps every caster's root in the shader's buffer.
pub fn ground(
    roots: Query<(&Transform, &Ordered), With<Casts>>,
    mut grounds: ResMut<Grounds>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
) {
    let mut changed = false;
    for (transform, ordered) in &roots {
        let Some(slot) = ordered.slot().and_then(|slot| usize::try_from(slot).ok()) else {
            continue;
        };
        if grounds.roots.len() <= slot {
            grounds.roots.resize(slot + 1, Vec4::ZERO);
        }
        let root = transform.translation.extend(0.0);
        if grounds.roots[slot] != root {
            grounds.roots[slot] = root;
            changed = true;
        }
    }
    if changed {
        let data: Vec<[f32; 4]> = grounds.roots.iter().map(|r| r.to_array()).collect();
        if let Some(mut buffer) = buffers.get_mut(&grounds.buffer) {
            *buffer = ShaderBuffer::new(data, RenderAssetUsages::default());
        }
    }
}

/// The models' shadows.
pub struct ShadowsPlugin;

impl Plugin for ShadowsPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shadows.wesl");
        app.add_plugins(MaterialPlugin::<ShadowMaterial>::default())
            .init_resource::<Grounds>()
            .add_systems(Startup, setup)
            .add_systems(Update, (cast, ground).chain().in_set(Step::Shadows));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::iso;
    use crate::statics::tests::shader_constant;

    const SHADER: &str = include_str!("shadows.wesl");

    #[test]
    fn a_shadow_leans_as_the_originals() {
        // The original's frame of the models (`0x6284E0`): x right on the screen, y toward the
        // viewer along the ground, z up; the camera looks down at atan(1/2) from 1,200 units
        // back and 600 up, and `ortho(±267, ±200)` puts 1,024 pixels across 534 units and 768 up
        // 400. Its shadow takes a point `h` above the plane to `(x + h/2, y − h/2)` on it
        // (`0x406FF0`): on the screen, from the point's foot, right and up by these pixels a
        // unit of the model's height.
        let right = 1024.0 / 534.0 * 0.5;
        let up = 768.0 / 400.0 * 0.5 / 5.0_f32.sqrt();
        // The game's ground moved by the shader's lean per unit of height, a model's unit being
        // `iso::MODEL_SCALE` of the game's, on the screen at zoom 1.
        let lean = Vec3::new(
            shader_constant(SHADER, "LEAN_X"),
            0.0,
            shader_constant(SHADER, "LEAN_Z"),
        ) * iso::MODEL_SCALE;
        let on_screen =
            Vec2::new(lean.dot(iso::right()), lean.dot(iso::up())) * iso::PIXELS_PER_UNIT;
        assert!(
            (on_screen - Vec2::new(right, up)).length() < 1e-4,
            "{on_screen}"
        );
    }

    #[test]
    fn a_shadow_is_as_dark_as_its_texture() {
        // `WEATHER_RAIN01.TGA`, black at alpha 170 in all its 16 × 16 texels, under vertices of
        // opaque black (`0xFF000000`).
        assert!((shader_constant(SHADER, "DARKNESS") - 170.0 / 255.0).abs() < 1e-7);
    }

    #[test]
    fn a_shadow_keeps_the_models_squeeze() {
        assert_eq!(
            shader_constant(SHADER, "SQUEEZE"),
            shader_constant(include_str!("models.wesl"), "SQUEEZE")
        );
    }
}
