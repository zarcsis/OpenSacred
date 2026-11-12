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
//! child of the world's, draws white into [`ShadowMask`] before the world, wherever any shadow
//! lies; a shadow's pixel then takes its darkness from that mask blurred about it, full where the
//! shadow reaches a few pixels on every side and none at its edge.

use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::camera::{CameraUpdateSystems, ClearColorConfig, RenderTarget};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::image::ImageSampler;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::mesh::{MeshTag, MeshVertexBufferLayoutRef};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, CompareFunction, Extent3d, RenderPipelineDescriptor, SpecializedMeshPipelineError,
    TextureFormat,
};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;

use crate::blended::{self, Layer};
use crate::frame::Step;
use crate::iso;
use crate::models::{Anchors, Fringe, ModelMaterial, Ordered, OrderedWith, Sunk};

/// The shader of [`ShadowMaterial`] and [`MaskMaterial`], embedded in the binary.
const SHADER: &str = "embedded://opensacred_game/shadows.wesl";

/// The render layer of the shadows' mask, which [`MaskCamera`] alone draws.
pub const MASK_LAYER: usize = 21;

/// The root of a model that casts a shadow: the hero, an NPC whose creature has no flag `0x10`
/// (`NOSHADOW`, `0x426D30`), a script's object (a `CreateObj` record) of category 4, which the
/// client makes a `cChest3D` (`0x451300`, whose draw `0x5C97B0` draws the shadow), and an arrow
/// (`cArrow3D`, `0x5BAF00`).
#[derive(Component)]
pub struct Casts;

/// The copy of a model's mesh that draws its shadow.
#[derive(Component)]
pub struct Shadow;

/// The copy of a model's mesh that draws its shadow into the [`ShadowMask`].
#[derive(Component)]
pub struct MaskCopy;

/// The camera of the [`ShadowMask`]: a child of the world's, with its projection, drawing the
/// layer [`MASK_LAYER`] before it.
#[derive(Component)]
pub struct MaskCamera;

/// Where any model's shadow lies on the screen, white on black, the size of the world camera's
/// viewport: drawn by [`MaskCamera`], read by [`ShadowMaterial`].
#[derive(Resource)]
pub struct ShadowMask {
    image: Handle<Image>,
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
    #[sampler(4)]
    mask: Handle<Image>,
    /// The shadows' layer, or the one under the liquids for the sunk models'.
    layer: Layer,
}

/// The material of the shadows' copies in the [`ShadowMask`]: flattened as the shadows are,
/// white, through the shader's `mask`.
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

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.entry_point = Some("mask".into());
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

/// The shadows' mask at `size`: drawn into and sampled, linearly between its pixels.
fn mask_image(size: UVec2) -> Image {
    let mut image = Image::new_target_texture(
        size.x.max(1),
        size.y.max(1),
        TextureFormat::Rgba8Unorm,
        None,
    );
    image.sampler = ImageSampler::linear();
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
    let mut in_layer = |layer| {
        materials.add(ShadowMaterial {
            anchors: anchors.buffer().clone(),
            grounds: grounds.buffer.clone(),
            floors: anchors.floor_buffer().clone(),
            mask: image.clone(),
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
    commands.insert_resource(ShadowMask { image });
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

/// Keeps the [`MaskCamera`] at its world camera's projection and the [`ShadowMask`] at half its
/// viewport's size each way: the blur spreads over a pixel and a half at zoom 1 on a frame of 768
/// lines, more on a larger one, so that a mask pixel to every four of the screen's loses nothing
/// of it, and the mask's pass costs a quarter.
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
        let Some(size) = camera
            .physical_viewport_size()
            .map(|size| (size + UVec2::ONE) / 2)
        else {
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
/// `0x407960` in their place, and shows them above it.
fn detail(
    settings: Option<Res<crate::settings::Settings>>,
    mut shadows: Query<&mut Visibility, Copies>,
) {
    let wanted = if crate::settings::detail(settings.as_deref()) == 0 {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut visibility in &mut shadows {
        visibility.set_if_neq(wanted);
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
            .add_plugins(MaterialPlugin::<MaskMaterial>::default())
            .init_resource::<Grounds>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (attach_mask, cast, soak, ground, detail)
                    .chain()
                    .in_set(Step::Shadows),
            )
            .add_systems(PostUpdate, follow_mask.before(CameraUpdateSystems));
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

    /// The world's camera, and it alone, takes the mask's camera as a child, out of the world
    /// cameras' queries and drawing the mask's layer (#2624).
    #[test]
    fn the_world_camera_takes_the_masks_camera() {
        let mut app = App::new();
        app.insert_resource(ShadowMask {
            image: Handle::default(),
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

    #[test]
    fn a_shadows_vertices_reach_past_its_blurred_edge() {
        // The blur reaches two spreads beyond the silhouette; on the screen the ground is
        // foreshortened by half along the line of sight, so that the vertices move twice that.
        let softness = shader_constant(SHADER, "SOFTNESS");
        assert!(shader_constant(SHADER, "REACH") >= 2.0 * 2.0 * softness);
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
