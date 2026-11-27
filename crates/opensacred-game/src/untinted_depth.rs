//! The world's depth given back to the untinted camera without the models' shadows (#3901;
//! docs/decisions/drawing.md, "Models' shadows by depth, not stencil"). The models' shadows write
//! their depth in the world's blended pass, in place of the original's stencil, so that each
//! darkens a pixel once; the untinted camera (`iso::Untinted`), which shares the world's depth and
//! draws its effects after it, then lost every effect's fragment behind a shadow's depth: the
//! bubble prison of quest 209 was cut along its captive's shadow, where the original, whose
//! shadows write no depth, draws the effect whole. The world's camera ([`KeepsDepth`]) keeps its
//! depth after its opaque pass ([`keep`]) and gives it back after its blended pass ([`give_back`]),
//! and the models' blended passes that write their depth ([`writes_depth`]: the fringes, the
//! water form's and the visions') are drawn again, depth alone, by the untinted camera, by a
//! [`Twin`] of each such mesh on its layer, so that they still hide the effects behind them. Not
//! at the options' detail 0, where no model casts a shadow.

use bevy::core_pipeline::core_3d::{main_opaque_pass_3d, main_transparent_pass_3d};
use bevy::core_pipeline::schedule::{Core3d, Core3dSystems};
use bevy::mesh::MeshTag;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::render_resource::{TextureDescriptor, TextureDimension, TextureUsages};
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::texture::{CachedTexture, TextureCache};
use bevy::render::view::ViewDepthStencilTexture;
use bevy::render::{Render, RenderApp, RenderSystems};
use std::collections::HashMap;

use crate::iso;
use crate::models::{self, ModelMaterial};

/// The world's camera, whose depth after its opaque pass the untinted camera tests against: on it
/// while a model casts a shadow (the options' detail above 0).
#[derive(Component, Clone, Copy, Default, ExtractComponent)]
#[extract_app(RenderApp)]
pub struct KeepsDepth;

/// The usages of the world's depth that keeping it asks for: written into, and copied into.
pub const USAGES: TextureUsages = TextureUsages::RENDER_ATTACHMENT.union(TextureUsages::COPY_DST);

/// The copy of a view's depth that [`keep`] makes and [`give_back`] lays back, in the render
/// world.
#[derive(Component)]
struct Kept(CachedTexture);

/// A mesh's depth-alone copy, a child of it on the untinted camera's layer, of the material
/// [`models::depth_alone`] makes of its own.
#[derive(Component)]
struct Twin(Entity);

/// A [`Twin`] itself.
#[derive(Component)]
struct IsTwin;

/// The depth-alone materials made of the models' materials, by the model material.
#[derive(Resource, Default)]
struct Made(HashMap<AssetId<ModelMaterial>, Handle<ModelMaterial>>);

/// Whether a model material's pass is blended and writes its depth (`models::Pass::Fringe`,
/// `Water`, `Vision`), which the given-back depth lacks.
fn writes_depth(material: &ModelMaterial) -> bool {
    material.extension.blended_depth()
}

/// Puts [`KeepsDepth`] on the world's cameras while a model casts a shadow.
fn mark(
    mut commands: Commands,
    settings: Option<Res<crate::settings::Settings>>,
    cameras: Query<(Entity, Has<KeepsDepth>), iso::Eye<With<Camera3d>>>,
) {
    let keeps = crate::settings::detail(settings.as_deref()) >= 1;
    for (camera, has) in &cameras {
        if keeps && !has {
            commands.entity(camera).insert(KeepsDepth);
        } else if !keeps && has {
            commands.entity(camera).remove::<KeepsDepth>();
        }
    }
}

/// Gives each mesh of a model whose material writes its depth in a blended pass a [`Twin`], and
/// keeps its material the depth-alone one of the mesh's as the mesh's changes (the fringe's under
/// the liquids, the water form's, a vision's); takes it away as the mesh's no longer writes so.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn twin(
    mut commands: Commands,
    meshes: Query<
        (
            Entity,
            &Mesh3d,
            &MeshMaterial3d<ModelMaterial>,
            Option<&SkinnedMesh>,
            Option<&MeshTag>,
            Option<&Twin>,
        ),
        (Changed<MeshMaterial3d<ModelMaterial>>, Without<IsTwin>),
    >,
    mut twins: Query<&mut MeshMaterial3d<ModelMaterial>, With<IsTwin>>,
    mut materials: ResMut<Assets<ModelMaterial>>,
    mut made: ResMut<Made>,
) {
    for (entity, mesh, material, skinned, tag, twin) in &meshes {
        let alone = materials
            .get(&material.0)
            .filter(|material| writes_depth(material))
            .cloned()
            .map(|own| {
                made.0
                    .entry(material.0.id())
                    .or_insert_with(|| materials.add(models::depth_alone(own)))
                    .clone()
            });
        match (alone, twin) {
            (Some(alone), Some(&Twin(copy))) => {
                if let Ok(mut current) = twins.get_mut(copy)
                    && current.0 != alone
                {
                    current.0 = alone;
                }
            }
            (Some(alone), None) => {
                let mut copy = commands.spawn((
                    Mesh3d(mesh.0.clone()),
                    MeshMaterial3d(alone),
                    Transform::IDENTITY,
                    Visibility::Inherited,
                    ChildOf(entity),
                    bevy::camera::visibility::RenderLayers::layer(iso::UNTINTED),
                    bevy::camera::visibility::NoFrustumCulling,
                    IsTwin,
                ));
                if let Some(skinned) = skinned {
                    copy.insert(skinned.clone());
                }
                if let Some(tag) = tag {
                    copy.insert(tag.clone());
                }
                let copy = copy.id();
                commands.entity(entity).insert(Twin(copy));
            }
            (None, Some(&Twin(copy))) => {
                commands.entity(copy).try_despawn();
                commands.entity(entity).remove::<Twin>();
            }
            (None, None) => {}
        }
    }
}

/// Makes each keeping view's [`Kept`] texture, its depth's like.
fn prepare(
    mut commands: Commands,
    mut cache: ResMut<TextureCache>,
    device: Res<RenderDevice>,
    views: Query<(Entity, &ViewDepthStencilTexture), With<KeepsDepth>>,
) {
    for (view, depth) in &views {
        let texture = depth.texture();
        let kept = cache.get(
            &device,
            TextureDescriptor {
                label: Some("the world's kept depth"),
                size: texture.size(),
                mip_level_count: 1,
                sample_count: texture.sample_count(),
                dimension: TextureDimension::D2,
                format: texture.format(),
                // A multisampled texture is a render attachment too, which wgpu asks of it.
                usage: TextureUsages::COPY_DST
                    | TextureUsages::COPY_SRC
                    | TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            },
        );
        commands.entity(view).insert(Kept(kept));
    }
}

/// Copies the world's depth after its opaque pass into its [`Kept`] texture: the ground, the
/// sprites' opaque texels and the models, before any shadow.
fn keep(
    view: ViewQuery<(&ViewDepthStencilTexture, &Kept), With<KeepsDepth>>,
    mut ctx: RenderContext,
) {
    let (depth, kept) = view.into_inner();
    let texture = depth.texture();
    ctx.command_encoder().copy_texture_to_texture(
        texture.as_image_copy(),
        kept.0.texture.as_image_copy(),
        texture.size(),
    );
}

/// Lays the [`Kept`] depth back over the world's after its blended pass, for the untinted camera,
/// which draws after it against the same depth.
fn give_back(
    view: ViewQuery<(&ViewDepthStencilTexture, &Kept), With<KeepsDepth>>,
    mut ctx: RenderContext,
) {
    let (depth, kept) = view.into_inner();
    let texture = depth.texture();
    ctx.command_encoder().copy_texture_to_texture(
        kept.0.texture.as_image_copy(),
        texture.as_image_copy(),
        texture.size(),
    );
}

/// The world's depth given back to the untinted camera without the models' shadows.
pub struct UntintedDepthPlugin;

impl Plugin for UntintedDepthPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<KeepsDepth>::default())
            .init_resource::<Made>()
            .add_systems(PostUpdate, (mark, twin));
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .add_systems(Render, prepare.in_set(RenderSystems::PrepareBindGroups))
                .add_systems(
                    Core3d,
                    (
                        keep.after(main_opaque_pass_3d)
                            .before(main_transparent_pass_3d),
                        give_back
                            .after(main_transparent_pass_3d)
                            .before(Core3dSystems::PostProcess),
                    ),
                );
        }
    }
}
