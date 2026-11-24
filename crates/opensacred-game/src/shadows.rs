//! The models' shadows (docs/re/drawing.md, "The models' shadows"). The original draws the shadow
//! of a creature or a chest with its model, at its place in the draw lists: the model's mesh,
//! deformed as the model's, flattened onto the plane at the object's height along a fixed light,
//! black at a third of the light left, through the stencil, so that every pixel is darkened once
//! (`cGranny::renderShadow`, `0x406FF0`). Each mesh of a model whose root is [`Casts`] has a copy
//! here that [`ShadowMaterial`] draws so (`shadows.wesl`): at its model's place among the sprites,
//! as the model is, in the shadows' layer (`blended::Layer::Shadows`), over what the
//! original draws before the model and under the model and what it draws after; the depth test
//! keeps a shadow's later triangles off the pixels its first ones darkened, where the original
//! has the stencil (docs/decisions/drawing.md, "Models' shadows by depth, not stencil"). The shadow
//! of a model under a liquid's surface (`models::Sunk`) lies with it, in the layer under the
//! liquids.
//!
//! The edges are soft (#2624; docs/decisions/drawing.md, "The models' shadows' soft edges"), as
//! the statics' shadows, silhouettes of 32 texels stretched over many pixels, look beside them:
//! each mesh has a second copy, of [`MaskMaterial`], that the mask's camera ([`MaskCamera`]), a
//! child of the world's, draws into [`ShadowMask`] before the world, wherever any shadow lies; a
//! shadow's pixel then takes its darkness from that mask blurred about it, full where the shadow
//! reaches a few pixels on every side and none at its edge. A shadow's mask is its own silhouette
//! and those of the shadows in front of its plane, by the nearest shadow's caster that the mask
//! keeps at each pixel: another's shadow behind it, a chest's own under the chest that it falls
//! on, which the chest hides, or a further creature's on the ground, kept its edge sharp (#2722,
//! #2737). At close zooms on a large screen, where the blur's samples lie pixels apart, they read
//! a level of the mask whose blocks are as wide, each with the casters that cover it and their
//! shares, which a compute pass makes from the mask after its camera draws it: on the mask's own
//! pixels a thin part's edge, a blade's, was striped across, falling between the samples in some
//! rows and not in the next (#2739). At the highest detail alone (#2628): at the medium the
//! shadows are the original's, sharp, the mask's camera off.

use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::camera::{CameraUpdateSystems, ClearColorConfig, RenderTarget};
use bevy::core_pipeline::schedule::Core3d;
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::core_pipeline::upscaling::upscaling;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::mesh::{MeshTag, MeshVertexBufferLayoutRef};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::diagnostic::RecordDiagnostics as _;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::binding_types::{texture_2d, texture_storage_2d};
use bevy::render::render_resource::{
    AsBindGroup, BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
    CachedComputePipelineId, CompareFunction, ComputePassDescriptor, ComputePipelineDescriptor,
    Extent3d, PipelineCache, RenderPipelineDescriptor, ShaderStages, SpecializedMeshPipelineError,
    StorageTextureAccess, TextureDimension, TextureFormat, TextureId, TextureSampleType,
    TextureUsages,
};
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::storage::ShaderBuffer;
use bevy::render::texture::GpuImage;
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::{ShaderDefVal, ShaderRef};

use crate::blended::{self, Layer};
use crate::frame::Step;
use crate::iso;
use crate::models::{Anchors, Fringe, ModelMaterial, Ordered, OrderedWith, Sunk};

/// The shader of [`ShadowMaterial`] and [`MaskMaterial`], embedded in the binary.
const SHADER: &str = "embedded://opensacred_game/shadows.wesl";
/// The shader that makes a level of the mask ([`ShadowMask`]), embedded in the binary.
const LEVEL_SHADER: &str = "embedded://opensacred_game/shadow_levels.wesl";
/// The spread of a shadow's edge, in world units: the shader's (`shadows.wesl`), by which
/// [`level_for`] chooses the level of the mask that its blur reads.
const SOFTNESS: f32 = 1.2;
/// How far apart, in pixels, the blur's samples may lie on the mask's own pixels: each takes in
/// two of them about it, and a thin part one or two pixels high, which falls between samples
/// further apart in some rows and not in the next, striped its edge across (#2739). Further apart
/// they lie on a level of the mask, a block apart.
const PIXELS_APART: f32 = 1.1;
/// The highest level of the mask: blocks of 64 pixels, as far apart as the blur's samples lie at
/// a spread of 128 pixels, some fifteen times the spread at zoom 0.5 on 3,840 × 2,160.
const LEVELS: u8 = 6;
/// The side of the groups of threads of the level's compute shader (`shadow_levels.wesl`), each
/// thread a texel of the level.
const GROUP: u32 = 8;

/// The render layer of the shadows' mask, which [`MaskCamera`] alone draws.
pub const MASK_LAYER: usize = 21;

/// The root of a model that casts a shadow: the hero, an NPC whose creature has no flag `0x10`
/// (`NOSHADOW`, `0x426D30`), a script's object (a `CreateObj` record) of category 4, which the
/// client makes a `cChest3D` (`0x451300`, whose draw `0x5C97B0` draws the shadow), an arrow
/// (`cArrow3D`, `0x5BAF00`), and the models of four effects: the demon fire's inner sphere, the
/// hell disk, the bubble's bush and the spikes, these at the highest detail alone (#3162;
/// docs/re/effects.md, "The effects' models' shadows").
#[derive(Component)]
pub struct Casts;

/// The height of the plane that a caster's shadow falls on, where it is not its root's: the
/// ground's under an effect's model that flies (the demon fire's sphere, the hell disk, the
/// bubble), as the effects' draws pass `0x406FF0` the system's height less its height over the
/// ground (`0x5A9A6D`), so that it casts on the ground under it, shifted by half its height.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Plane(pub f32);

/// The copy of a model's mesh that draws its shadow.
#[derive(Component)]
pub struct Shadow;

/// The copy of a model's mesh that draws its shadow into the [`ShadowMask`].
#[derive(Component)]
pub struct MaskCopy;

/// The camera of the [`ShadowMask`]: a child of the world's, with its projection, drawing the
/// layer [`MASK_LAYER`] before it; its view, in the render world, makes the mask's level.
#[derive(Component, Clone, ExtractComponent)]
#[extract_app(RenderApp)]
pub struct MaskCamera;

/// Where any model's shadow lies on the screen, the size of the world camera's viewport: its red
/// whole there and none elsewhere, its green and blue the number of the nearest shadow's caster
/// there, its slot of the [`Anchors`] plus one. Drawn by [`MaskCamera`], read by
/// [`ShadowMaterial`]. And the level of it that the zoom chooses ([`level_for`], #2739), which
/// [`reduce`] makes from it: each texel a block of the mask's pixels, two to the level across,
/// with the two casters that cover the most of it and their shares of it, over which the blur
/// samples a thin part alike in every row at close zooms.
#[derive(Resource, Clone, ExtractResource)]
#[extract_app(RenderApp)]
pub struct ShadowMask {
    image: Handle<Image>,
    level_image: Handle<Image>,
    /// The level, 0 for the mask's own pixels, which need no level made.
    level: u8,
}

/// The material of the shadows, which every one of a layer shares: the models' anchors, for their
/// places, their roots, for their planes, and their floors, for how far back the sunk ones lie;
/// the [`ShadowMask`], for their edges.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct ShadowMaterial {
    #[storage(0, read_only)]
    anchors: Handle<ShaderBuffer>,
    #[storage(1, read_only)]
    grounds: Handle<ShaderBuffer>,
    #[storage(2, read_only)]
    floors: Handle<ShaderBuffer>,
    #[texture(3)]
    mask: Handle<Image>,
    /// The level of the mask that the blur reads (#2739).
    #[texture(4, sample_type = "u_int")]
    level: Handle<Image>,
    /// Its x 1 for the soft edges, at the highest detail, and 0 for the original's sharp ones
    /// (#2628); its y the level of the mask that the blur reads, 0 for the mask's own pixels
    /// (#2739).
    #[uniform(5)]
    soft: Vec4,
    /// The shadows' layer, or the one under the liquids for the sunk models'.
    layer: Layer,
}

/// The material of the shadows' copies in the [`ShadowMask`]: flattened as the shadows are, each
/// its caster's number at the depth of its plane, through the shader's `mask`.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct MaskMaterial {
    #[storage(0, read_only)]
    anchors: Handle<ShaderBuffer>,
    #[storage(1, read_only)]
    grounds: Handle<ShaderBuffer>,
    #[storage(2, read_only)]
    floors: Handle<ShaderBuffer>,
}

impl Material for MaskMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    /// The depth of a shadow's plane that the fragment shader gives, written, and a pixel taken
    /// where it is as near as what lies there or nearer: where shadows overlap, the mask keeps the
    /// nearest's caster.
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.entry_point = Some("mask".into());
        }
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_write_enabled = Some(true);
            depth.depth_compare = Some(CompareFunction::GreaterEqual);
        }
        Ok(())
    }
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
        self.layer.bias()
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
        // The shader's other fragment stage is the mask's; the vertices move out for the edges.
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.entry_point = Some("fragment".into());
        }
        descriptor.vertex.shader_defs.push("SOFT_EDGES".into());
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_write_enabled = Some(true);
            depth.depth_compare = Some(CompareFunction::Greater);
        }
        Ok(())
    }
}

/// Every caster's root by the slot its root holds of the [`Anchors`], for the shader by its
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

/// The shared [`ShadowMaterial`]s: of the shadows' layer, and of the layer under the liquids; and
/// the [`MaskMaterial`].
#[derive(Resource)]
pub struct Shared {
    dry: Handle<ShadowMaterial>,
    wet: Handle<ShadowMaterial>,
    mask: Handle<MaskMaterial>,
}

impl Shared {
    /// The material of a shadow whose model is sunk or not.
    fn material(&self, sunk: bool) -> &Handle<ShadowMaterial> {
        if sunk { &self.wet } else { &self.dry }
    }
}

/// The shadows' mask at `size`: drawn into and read pixel by pixel, each a caster's (#2737).
fn mask_image(size: UVec2) -> Image {
    Image::new_target_texture(
        size.x.max(1),
        size.y.max(1),
        TextureFormat::Rgba8Unorm,
        None,
    )
}

/// The level of the shadows' mask whose blocks are as wide as the blur's samples lie apart at
/// `per_unit` pixels a world unit, half a spread ([`SOFTNESS`]), a pixel at least (#2739): 0, the
/// mask's own pixels, while that is no more than [`PIXELS_APART`], at zoom 1 on 1,920 × 1,080
/// and smaller; else the lowest whose blocks, two to the level pixels across, are as wide, at
/// most [`LEVELS`].
fn level_for(per_unit: f32) -> u8 {
    let step = (0.5 * SOFTNESS * per_unit).max(1.0);
    if step <= PIXELS_APART {
        return 0;
    }
    (1..LEVELS)
        .find(|&level| f32::from(1_u16 << level) >= step)
        .unwrap_or(LEVELS)
}

/// The size of the level `level` of the shadows' mask at `size`: a pixel at the level 0, which is
/// not made.
fn level_size(size: UVec2, level: u8) -> UVec2 {
    if level == 0 {
        return UVec2::ONE;
    }
    let block = 1_u32 << level;
    UVec2::new(size.x.div_ceil(block), size.y.div_ceil(block)).max(UVec2::ONE)
}

/// A level of the shadows' mask of `size` (#2739): written by [`reduce`], read pixel by pixel.
fn level_image(size: UVec2) -> Image {
    let mut image = Image::new_uninit(
        Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        TextureFormat::Rgba16Uint,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING;
    image
}

pub fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<ShadowMaterial>>,
    mut masks: ResMut<Assets<MaskMaterial>>,
    mut images: ResMut<Assets<Image>>,
    anchors: Res<Anchors>,
    grounds: Res<Grounds>,
) {
    let image = images.add(mask_image(UVec2::ONE));
    let level_image = images.add(self::level_image(UVec2::ONE));
    let mut in_layer = |layer| {
        materials.add(ShadowMaterial {
            anchors: anchors.buffer().clone(),
            grounds: grounds.buffer.clone(),
            floors: anchors.floor_buffer().clone(),
            mask: image.clone(),
            level: level_image.clone(),
            soft: Vec4::X,
            layer,
        })
    };
    commands.insert_resource(Shared {
        dry: in_layer(Layer::Shadows),
        wet: in_layer(Layer::UnderLiquids),
        mask: masks.add(MaskMaterial {
            anchors: anchors.buffer().clone(),
            grounds: grounds.buffer.clone(),
            floors: anchors.floor_buffer().clone(),
        }),
    });
    commands.insert_resource(ShadowMask {
        image,
        level_image,
        level: 0,
    });
}

/// Gives the world's camera, as it comes, the [`MaskCamera`] as a child.
fn attach_mask(
    mut commands: Commands,
    mask: Res<ShadowMask>,
    cameras: Query<Entity, iso::Eye<Added<Camera3d>>>,
) {
    for camera in &cameras {
        commands.entity(camera).with_child((
            MaskCamera,
            Camera3d::default(),
            Camera {
                order: -1,
                clear_color: ClearColorConfig::Custom(Color::BLACK),
                ..default()
            },
            RenderTarget::from(mask.image.clone()),
            Msaa::Off,
            Tonemapping::None,
            DebandDither::Disabled,
            // It draws no light: one cluster for the lights it does not use.
            bevy::light::cluster::ClusterConfig::Single,
            RenderLayers::layer(MASK_LAYER),
            // Out of the world's cameras' queries (`iso::Eye`), its zoom kept by `follow_mask`.
            iso::Apart,
        ));
    }
}

/// Keeps the [`MaskCamera`] at its world camera's projection and the [`ShadowMask`] at its
/// viewport's size: at half of it a thin part, a blade a few pixels wide, became a staircase of
/// the mask's pixels, which the blur showed as blocks (#2630).
fn follow_mask(
    eyes: Query<(&Camera, Ref<Projection>), iso::Eye<Without<MaskCamera>>>,
    mut cameras: Query<(&ChildOf, &mut Projection), With<MaskCamera>>,
    mask: Res<ShadowMask>,
    shared: Res<Shared>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<ShadowMaterial>>,
) {
    for (parent, mut projection) in &mut cameras {
        let Ok((camera, eye)) = eyes.get(parent.parent()) else {
            continue;
        };
        // A new mask camera has Bevy's default perspective until it takes the world's.
        if eye.is_changed() || !matches!(*projection, Projection::Orthographic(_)) {
            *projection = Projection::clone(&eye);
        }
        let Some(size) = camera.physical_viewport_size() else {
            continue;
        };
        let current = images
            .get(&mask.image)
            .map(|image| image.texture_descriptor.size);
        if current
            != Some(Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            })
        {
            if let Err(error) = images.insert(&mask.image, mask_image(size)) {
                warn!("shadows: the mask at {size}: {error}");
                continue;
            }
            // The materials take the new texture: given it again, they are prepared again, as an
            // asset borrowed alone is not.
            for material in [&shared.dry, &shared.wet] {
                if let Some(mut material) = materials.get_mut(material) {
                    material.mask = mask.image.clone();
                }
            }
        }
    }
}

/// Chooses the level of the [`ShadowMask`] that the shadows' blur reads, by the world camera's
/// pixels a unit at its zoom and its viewport's size ([`level_for`]), and keeps the level's image
/// at its size and the materials reading it (#2739): once its projection is updated for the
/// frame.
fn choose_level(
    eyes: Query<&Camera, iso::Eye<Without<MaskCamera>>>,
    cameras: Query<&ChildOf, With<MaskCamera>>,
    mut mask: ResMut<ShadowMask>,
    shared: Res<Shared>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<ShadowMaterial>>,
) {
    for parent in &cameras {
        let Ok(camera) = eyes.get(parent.parent()) else {
            continue;
        };
        let Some(size) = camera.physical_viewport_size() else {
            continue;
        };
        // As the shader's `per_unit`: the projection is orthographic, as many pixels across as up.
        let per_unit = camera.clip_from_view().y_axis.y * size.as_vec2().y * 0.5;
        let level = level_for(per_unit);
        let wanted = level_size(size, level);
        let current = images
            .get(&mask.level_image)
            .map(|image| image.texture_descriptor.size);
        if mask.level == level
            && current
                == Some(Extent3d {
                    width: wanted.x,
                    height: wanted.y,
                    depth_or_array_layers: 1,
                })
        {
            continue;
        }
        if let Err(error) = images.insert(&mask.level_image, level_image(wanted)) {
            warn!("shadows: the mask's level {level} at {wanted}: {error}");
            continue;
        }
        mask.level = level;
        for material in [&shared.dry, &shared.wet] {
            if let Some(mut material) = materials.get_mut(material) {
                material.level = mask.level_image.clone();
                material.soft.y = f32::from(level);
            }
        }
    }
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
/// the same slot, shown while the mesh is, in its model's layer. A held model, a weapon, a shield
/// or the wings, casts with its holder, flattened at its holder's height through the holder's
/// slot that its meshes take, as the client draws an item's shadow at its holder's height
/// (`0x44B230`, #2610).
pub fn cast(
    mut commands: Commands,
    shared: Res<Shared>,
    meshes: Query<ModelMesh, (Added<MeshMaterial3d<ModelMaterial>>, Without<Fringe>)>,
    parents: Query<&ChildOf>,
    models: Query<Option<&OrderedWith>, Model>,
    casters: Query<Has<Sunk>, (With<Casts>, With<Ordered>)>,
) {
    for (entity, mesh, skinned, tag, transform, parent) in &meshes {
        let Some(root) = parents
            .iter_ancestors(entity)
            .find(|&ancestor| models.contains(ancestor))
        else {
            continue;
        };
        let caster = models
            .get(root)
            .ok()
            .flatten()
            .map_or(root, |&OrderedWith(holder)| holder);
        let Ok(sunk) = casters.get(caster) else {
            continue;
        };
        let mut shadow = commands.spawn((
            Mesh3d(mesh.0.clone()),
            MeshMaterial3d(shared.material(sunk).clone()),
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
        // And into the mask, for its edges (#2624).
        let mut copy = commands.spawn((
            Mesh3d(mesh.0.clone()),
            MeshMaterial3d(shared.mask.clone()),
            tag.clone(),
            *transform,
            ChildOf(parent.parent()),
            NoFrustumCulling,
            RenderLayers::layer(MASK_LAYER),
            MaskCopy,
        ));
        if let Some(skinned) = skinned {
            copy.insert(skinned.clone());
        }
    }
}

/// A shadow's copies: the one drawn, and the one in the mask.
type Copies = Or<(With<Shadow>, With<MaskCopy>)>;

/// Hides the models' shadows at the options' lowest detail, where `shadow_dots` lays the dots of
/// `0x407960` in their place, and shows them above it: sharp, as the original's, at the medium
/// detail, the mask's camera off and its copies hidden, and with their soft edges at the highest
/// (#2628), which pays for that camera.
fn detail(
    settings: Option<Res<crate::settings::Settings>>,
    mut copies: Query<(&mut Visibility, Has<MaskCopy>), Copies>,
    mut cameras: Query<&mut Camera, With<MaskCamera>>,
    shared: Option<Res<Shared>>,
    mut materials: ResMut<Assets<ShadowMaterial>>,
) {
    let detail = crate::settings::detail(settings.as_deref());
    let soft = detail >= 2;
    for (mut visibility, masked) in &mut copies {
        let shown = if masked { soft } else { detail >= 1 };
        visibility.set_if_neq(if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    for mut camera in &mut cameras {
        if camera.is_active != soft {
            camera.is_active = soft;
        }
    }
    let wanted = if soft { 1.0 } else { 0.0 };
    for material in shared.iter().flat_map(|shared| [&shared.dry, &shared.wet]) {
        // Borrowed mutably only to change it: a material borrowed so is prepared again.
        if materials
            .get(material)
            .is_some_and(|known| known.soft.x != wanted)
            && let Some(mut material) = materials.get_mut(material)
        {
            material.soft.x = wanted;
        }
    }
}

/// Gives the shadow of each caster that has just sunk under a liquid's surface, or come out of
/// one, the material of its layer.
fn soak(
    sunk: Query<Entity, (Added<Sunk>, With<Casts>)>,
    mut dried: RemovedComponents<Sunk>,
    wet: Query<(), With<Sunk>>,
    shared: Res<Shared>,
    children: Query<&Children>,
    held: Query<(Entity, &OrderedWith)>,
    mut shadows: Query<&mut MeshMaterial3d<ShadowMaterial>, With<Shadow>>,
) {
    for root in sunk.iter().chain(dried.read()) {
        let wanted = shared.material(wet.contains(root));
        // Its own meshes' shadows, and those of the models it holds (#2610).
        let models = std::iter::once(root).chain(
            held.iter()
                .filter(|&(_, &OrderedWith(holder))| holder == root)
                .map(|(model, _)| model),
        );
        for entity in models.flat_map(|model| children.iter_descendants(model)) {
            if let Ok(mut material) = shadows.get_mut(entity)
                && material.0 != *wanted
            {
                material.0 = wanted.clone();
            }
        }
    }
}

/// Keeps every caster's root in the shader's buffer, its height its [`Plane`]'s where it has one.
pub fn ground(
    roots: Query<(&Transform, &Ordered, Option<&Plane>), With<Casts>>,
    mut grounds: ResMut<Grounds>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
) {
    let mut changed = false;
    for (transform, ordered, plane) in &roots {
        let Some(slot) = ordered.slot().and_then(|slot| usize::try_from(slot).ok()) else {
            continue;
        };
        if grounds.roots.len() <= slot {
            grounds.roots.resize(slot + 1, Vec4::ZERO);
        }
        let at = transform.translation;
        let root = plane.map_or(at, |plane| at.with_y(plane.0)).extend(0.0);
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

/// The pipelines that make a level of the mask, each level's, from the first on, and their
/// bind group's layout.
#[derive(Resource)]
struct LevelPipelines {
    layout: BindGroupLayoutDescriptor,
    levels: Vec<CachedComputePipelineId>,
}

/// Queues the [`LevelPipelines`].
fn init_levels(
    mut commands: Commands,
    assets: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
) {
    let shader = assets.load(LEVEL_SHADER);
    let layout = BindGroupLayoutDescriptor::new(
        "shadow mask's level",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_storage_2d(TextureFormat::Rgba16Uint, StorageTextureAccess::WriteOnly),
            ),
        ),
    );
    let levels = (1..=LEVELS)
        .map(|level| {
            pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some(format!("shadow mask's level {level}").into()),
                layout: vec![layout.clone()],
                shader: shader.clone(),
                shader_defs: vec![ShaderDefVal::UInt("BLOCK".into(), 1 << level)],
                entry_point: Some("reduce".into()),
                ..default()
            })
        })
        .collect();
    commands.insert_resource(LevelPipelines { layout, levels });
}

/// The bind group of the pass that makes the mask's level, from the mask into the level's image,
/// with the level's size: made again only when either texture is new, at the window's new size
/// or a new level.
#[derive(Resource, Default)]
struct LevelBindGroup {
    textures: Option<(TextureId, TextureId)>,
    made: Option<(BindGroup, UVec2)>,
}

/// Keeps the [`LevelBindGroup`] for the textures of the [`ShadowMask`].
fn prepare_level(
    mask: Option<Res<ShadowMask>>,
    pipelines: Option<Res<LevelPipelines>>,
    pipeline_cache: Res<PipelineCache>,
    images: Res<RenderAssets<GpuImage>>,
    device: Res<RenderDevice>,
    mut bind_group: ResMut<LevelBindGroup>,
) {
    let (Some(mask), Some(pipelines)) = (mask, pipelines) else {
        return;
    };
    let (Some(source), Some(level)) = (images.get(&mask.image), images.get(&mask.level_image))
    else {
        *bind_group = LevelBindGroup::default();
        return;
    };
    let textures = (source.texture.id(), level.texture.id());
    if bind_group.textures == Some(textures) {
        return;
    }
    let size = level.texture_descriptor.size;
    *bind_group = LevelBindGroup {
        textures: Some(textures),
        made: Some((
            device.create_bind_group(
                "shadow mask's level",
                &pipeline_cache.get_bind_group_layout(&pipelines.layout),
                &BindGroupEntries::sequential((&source.texture_view, &level.texture_view)),
            ),
            UVec2::new(size.width, size.height),
        )),
    };
}

/// Makes the [`ShadowMask`]'s level from the mask, once the [`MaskCamera`] has drawn it, before
/// the world's camera draws the shadows that read it (#2739): none at the level 0, where they read
/// the mask's own pixels.
fn reduce(
    _view: ViewQuery<(), With<MaskCamera>>,
    mask: Option<Res<ShadowMask>>,
    pipelines: Option<Res<LevelPipelines>>,
    pipeline_cache: Res<PipelineCache>,
    bind_group: Res<LevelBindGroup>,
    mut ctx: RenderContext,
) {
    let (Some(mask), Some(pipelines), Some((bind_group, size))) =
        (mask, pipelines, bind_group.made.as_ref())
    else {
        return;
    };
    let Some(pipeline) = usize::from(mask.level)
        .checked_sub(1)
        .and_then(|index| pipelines.levels.get(index))
        .and_then(|&id| pipeline_cache.get_compute_pipeline(id))
    else {
        return;
    };
    // Timed with the render's passes where its diagnostics are on: some 0.06 ms of the GPU at
    // 3,840 × 2,160, the mask read whole once.
    let diagnostics = ctx.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = ctx
        .command_encoder()
        .begin_compute_pass(&ComputePassDescriptor {
            label: Some("shadow mask's level"),
            timestamp_writes: None,
        });
    let span = diagnostics.pass_span(&mut pass, "shadow_mask_level");
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[]);
    pass.dispatch_workgroups(size.x.div_ceil(GROUP), size.y.div_ceil(GROUP), 1);
    span.end(&mut pass);
}

/// The models' shadows.
pub struct ShadowsPlugin;

impl Plugin for ShadowsPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shadows.wesl");
        embedded_asset!(app, "shadow_levels.wesl");
        app.add_plugins(MaterialPlugin::<ShadowMaterial>::default())
            .add_plugins(MaterialPlugin::<MaskMaterial>::default())
            .add_plugins(ExtractComponentPlugin::<MaskCamera>::default())
            .add_plugins(ExtractResourcePlugin::<ShadowMask>::default())
            .init_resource::<Grounds>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (attach_mask, cast, soak, ground, detail)
                    .chain()
                    .in_set(Step::Shadows),
            )
            .add_systems(PostUpdate, follow_mask.before(CameraUpdateSystems))
            .add_systems(PostUpdate, choose_level.after(CameraUpdateSystems));
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<LevelBindGroup>()
                .add_systems(RenderStartup, init_levels)
                .add_systems(
                    Render,
                    prepare_level.in_set(RenderSystems::PrepareBindGroups),
                )
                // The mask's camera draws into the mask at its upscaling, the last of its passes.
                .add_systems(Core3d, reduce.after(upscaling));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::iso;
    use crate::statics::tests::shader_constant;

    const SHADER: &str = include_str!("shadows.wesl");

    #[test]
    fn a_held_model_casts_with_its_holder() {
        let mut app = App::new();
        app.insert_resource(Shared {
            dry: Handle::default(),
            wet: Handle::default(),
            mask: Handle::default(),
        })
        .add_systems(Update, cast);
        // A caster, the sword it holds and a model that casts nothing, each with a mesh.
        let world = app.world_mut();
        let hero = world.spawn((Casts, Ordered::default())).id();
        let sword = world.spawn(OrderedWith(hero)).id();
        let barrel = world.spawn(Ordered::default()).id();
        let mesh = |world: &mut World, root| {
            world
                .spawn((
                    Mesh3d::default(),
                    MeshMaterial3d::<ModelMaterial>::default(),
                    MeshTag::new(0),
                    Transform::default(),
                    ChildOf(root),
                ))
                .id()
        };
        for root in [hero, sword, barrel] {
            mesh(world, root);
        }
        app.update();
        let world = app.world_mut();
        // A flattened copy beside the hero's body and its sword (#2610), none beside the barrel.
        let mut copies = world.query_filtered::<&ChildOf, With<Shadow>>();
        let mut parents: Vec<Entity> = copies.iter(world).map(ChildOf::parent).collect();
        parents.sort();
        let mut wanted = vec![hero, sword];
        wanted.sort();
        assert_eq!(parents, wanted);
        // And a copy into the mask beside each, in the mask's layer alone (#2624).
        let mask = RenderLayers::layer(MASK_LAYER);
        let mut masks = world.query_filtered::<(&ChildOf, &RenderLayers), With<MaskCopy>>();
        let mut parents: Vec<Entity> = masks
            .iter(world)
            .map(|(parent, layers)| {
                assert_eq!(*layers, mask);
                parent.parent()
            })
            .collect();
        parents.sort();
        assert_eq!(parents, wanted);
    }

    /// The highest detail draws the soft edges and pays for the mask's camera; the medium draws
    /// the original's sharp shadows with the camera off and its copies hidden; the lowest none
    /// (#2628).
    #[test]
    fn the_detail_chooses_the_shadows_edges() {
        let mut app = App::new();
        let mut materials = Assets::<ShadowMaterial>::default();
        let material = || ShadowMaterial {
            anchors: Handle::default(),
            grounds: Handle::default(),
            floors: Handle::default(),
            mask: Handle::default(),
            level: Handle::default(),
            soft: Vec4::X,
            layer: Layer::Shadows,
        };
        let (dry, wet) = (materials.add(material()), materials.add(material()));
        app.insert_resource(materials)
            .insert_resource(Shared {
                dry: dry.clone(),
                wet,
                mask: Handle::default(),
            })
            .add_systems(Update, detail);
        let world = app.world_mut();
        let camera = world.spawn((MaskCamera, Camera::default())).id();
        let shadow = world.spawn((Shadow, Visibility::Inherited)).id();
        let copy = world.spawn((MaskCopy, Visibility::Inherited)).id();
        for (level, active, shadow_shown, soft) in [
            (1, false, Visibility::Inherited, 0.0),
            (2, true, Visibility::Inherited, 1.0),
            (0, false, Visibility::Hidden, 0.0),
        ] {
            let mut settings = crate::settings::Settings::default();
            settings.graphics.detail = level;
            app.insert_resource(settings);
            app.update();
            let world = app.world();
            assert_eq!(
                world.get::<Camera>(camera).unwrap().is_active,
                active,
                "{level}"
            );
            assert_eq!(
                world.get::<Visibility>(shadow),
                Some(&shadow_shown),
                "{level}"
            );
            let copy_shown = if active {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            assert_eq!(world.get::<Visibility>(copy), Some(&copy_shown), "{level}");
            let soft_now = world
                .resource::<Assets<ShadowMaterial>>()
                .get(&dry)
                .unwrap()
                .soft
                .x;
            assert_eq!(soft_now, soft, "{level}");
        }
    }

    /// The world's camera, and it alone, takes the mask's camera as a child, out of the world
    /// cameras' queries and drawing the mask's layer (#2624).
    #[test]
    fn the_world_camera_takes_the_masks_camera() {
        let mut app = App::new();
        app.insert_resource(ShadowMask {
            image: Handle::default(),
            level_image: Handle::default(),
            level: 0,
        })
        .add_systems(Update, attach_mask);
        let world = app.world_mut();
        let eye = world.spawn(Camera3d::default()).id();
        world.spawn((Camera3d::default(), iso::Untinted));
        world.spawn((Camera3d::default(), iso::Apart));
        app.update();
        app.update();
        let world = app.world_mut();
        let mut masks = world
            .query_filtered::<(&ChildOf, &RenderLayers), (With<MaskCamera>, With<iso::Apart>)>();
        let found: Vec<(Entity, RenderLayers)> = masks
            .iter(world)
            .map(|(parent, layers)| (parent.parent(), layers.clone()))
            .collect();
        assert_eq!(found, vec![(eye, RenderLayers::layer(MASK_LAYER))]);
    }

    /// The blur reads the mask's own pixels where its samples lie a pixel or so apart, and a level
    /// whose blocks are as wide as they lie apart at closer zooms on larger screens (#2739).
    #[test]
    fn the_blur_reads_the_level_of_its_samples() {
        assert_eq!(shader_constant(SHADER, "SOFTNESS"), SOFTNESS);
        // The world's pixels a unit on a screen `height` pixels high at `zoom`.
        let per_unit =
            |height: f32, zoom: f32| iso::PIXELS_PER_UNIT * height / iso::SCREEN_HEIGHT / zoom;
        let step = |per_unit: f32| (0.5 * SOFTNESS * per_unit).max(1.0);
        // 1,920 × 1,080 at zoom 1 and further: the mask's own pixels, as before.
        for zoom in [1.0, 1.5, 2.0] {
            assert_eq!(level_for(per_unit(1080.0, zoom)), 0, "{zoom}");
        }
        // Closer, and on larger screens: the lowest level whose blocks are as wide as the samples
        // lie apart, a block apart then.
        for (height, zoom, level) in [
            (1080.0, 0.5, 2),
            (2160.0, 1.0, 2),
            (2160.0, 0.5, 3),
            (1440.0, 0.75, 1),
            (4320.0, 0.5, 4),
        ] {
            let per_unit = per_unit(height, zoom);
            assert_eq!(level_for(per_unit), level, "{height} at {zoom}");
            let block = f32::from(1_u16 << level);
            assert!(block >= step(per_unit) && block < 2.0 * step(per_unit));
        }
        assert_eq!(level_for(1e6), LEVELS);
        assert_eq!(level_size(UVec2::new(3840, 2160), 3), UVec2::new(480, 270));
        assert_eq!(level_size(UVec2::new(1001, 7), 2), UVec2::new(251, 2));
        assert_eq!(level_size(UVec2::new(1920, 1080), 0), UVec2::ONE);
    }

    #[test]
    fn a_shadows_vertices_reach_past_its_blurred_edge() {
        // The blur reaches two spreads beyond the silhouette; the vertices move out of it on the
        // screen, in spreads (#2630).
        assert!(shader_constant(SHADER, "REACH") >= 2.0);
    }

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
