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
//! The setting `[graphics] shadows = "soft"` (#1436) draws none of those copies:
//! the sun, a directional light along the same lean ([`sun`]), casts the shadows of the models'
//! own meshes into a shadow map, which darkens the ground and the models to the same third of
//! their light (`soft.wesl`), so that a shadow follows the terrain, falls on other models, and a
//! model shades itself. The sun lights nothing; its layer ([`SUN_LAYER`]) holds the casters'
//! meshes alone, so that nothing else casts.

use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::light::{CascadeShadowConfig, CascadeShadowConfigBuilder, DirectionalLightShadowMap};
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
use crate::models::{Anchors, Fringe, ModelMaterial, Ordered, OrderedWith, Sunk};
use crate::settings::{Settings, Shadows};

/// The shader of [`ShadowMaterial`], embedded in the binary.
const SHADER: &str = "embedded://opensacred_game/shadows.wesl";

/// The root of a model that casts a shadow: the hero, an NPC whose creature has no flag `0x10`
/// (`NOSHADOW`, `0x426D30`), a script's object (a `CreateObj` record) of category 4, which the
/// client makes a `cChest3D` (`0x451300`, whose draw `0x5C97B0` draws the shadow), and an arrow
/// (`cArrow3D`, `0x5BAF00`).
#[derive(Component)]
pub struct Casts;

/// The copy of a model's mesh that draws its shadow.
#[derive(Component)]
pub struct Shadow;

/// The render layer of the sun ([`sun`]) and of the meshes that cast its shadows, which the
/// world's camera draws besides its own.
pub const SUN_LAYER: usize = 30;

/// The direction of the light of the models' shadows: a point `h` above its ground falls
/// `(LEAN_X·h, LEAN_Z·h)` from its foot (`shadows.wesl`).
const LEAN: Vec3 = Vec3::new(0.036_930, -1.0, -0.670_176);

/// The side of the sun's shadow map, in texels: at zoom 2 its one cascade spans some 7,000 units
/// of the world, under two a texel.
const SUN_MAP: usize = 4096;

/// How far the sun's shadows reach before and beyond the point the camera looks at, along its
/// line of sight, besides the view's own depth: the models' heights and the terrain's.
const SUN_MARGIN: f32 = 600.0;

/// The light of the soft shadows (#1436): a directional light along the shadows' lean, of no
/// illuminance, in its own layer, casting while `[graphics] shadows` is `soft` ([`switch`]);
/// spawned after the game's light, so that it follows it among the lights (`standard.wesl`'s
/// hover takes the first).
#[derive(Component)]
pub struct Sun;

/// The sun ([`Sun`]), not casting yet.
pub fn sun() -> impl Bundle {
    (
        Sun,
        DirectionalLight {
            illuminance: 0.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::IDENTITY.looking_to(LEAN, Vec3::Y),
        RenderLayers::layer(SUN_LAYER),
        sun_cascade(1.0),
    )
}

/// The sun's one cascade at the camera's zoom `zoom`: the depths of the view along the camera's
/// line of sight, which looks down at 30° at the point `iso::DISTANCE` ahead, and
/// [`SUN_MARGIN`] before and beyond them.
fn sun_cascade(zoom: f32) -> CascadeShadowConfig {
    let height = crate::iso::SCREEN_HEIGHT / crate::iso::PIXELS_PER_UNIT * zoom;
    // The view's height spans this much of the ground's depth: the ground lies at 30° to the
    // line of sight.
    let depth = 0.5 * height * 3.0_f32.sqrt() + SUN_MARGIN;
    CascadeShadowConfigBuilder {
        num_cascades: 1,
        minimum_distance: crate::iso::DISTANCE - depth,
        maximum_distance: crate::iso::DISTANCE + depth,
        ..default()
    }
    .build()
}

/// The material of the shadows, which every one of a layer shares: the models' anchors, for their
/// places, their roots, for their planes, and their floors, for how far back the sunk ones lie.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct ShadowMaterial {
    #[storage(0, read_only)]
    anchors: Handle<ShaderBuffer>,
    #[storage(1, read_only)]
    grounds: Handle<ShaderBuffer>,
    #[storage(2, read_only)]
    floors: Handle<ShaderBuffer>,
    /// The shadows' layer, or the one under the liquids for the sunk models'.
    layer: Layer,
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

/// The shared [`ShadowMaterial`]s: of the shadows' layer, and of the layer under the liquids.
#[derive(Resource)]
pub struct Shared {
    dry: Handle<ShadowMaterial>,
    wet: Handle<ShadowMaterial>,
}

impl Shared {
    /// The material of a shadow whose model is sunk or not.
    fn material(&self, sunk: bool) -> &Handle<ShadowMaterial> {
        if sunk { &self.wet } else { &self.dry }
    }
}

pub fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<ShadowMaterial>>,
    anchors: Res<Anchors>,
    grounds: Res<Grounds>,
) {
    let mut in_layer = |layer| {
        materials.add(ShadowMaterial {
            anchors: anchors.buffer().clone(),
            grounds: grounds.buffer.clone(),
            floors: anchors.floor_buffer().clone(),
            layer,
        })
    };
    commands.insert_resource(Shared {
        dry: in_layer(Layer::Shadows),
        wet: in_layer(Layer::UnderLiquids),
    });
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
    Option<&'static RenderLayers>,
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
    for (entity, mesh, skinned, tag, transform, parent, layers) in &meshes {
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
        // Into the sun's shadow map, as well as the world's frame.
        let layers = layers.cloned().unwrap_or_default().with(SUN_LAYER);
        commands.entity(entity).insert(layers);
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
    }
}

/// Hides the models' shadows at the options' lowest detail, where `shadow_dots` lays the dots of
/// `0x407960` in their place, and shows them above it.
fn detail(settings: Option<Res<Settings>>, mut shadows: Query<&mut Visibility, With<Shadow>>) {
    let wanted = if crate::settings::detail(settings.as_deref()) == 0 || soft(settings.as_deref()) {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut visibility in &mut shadows {
        visibility.set_if_neq(wanted);
    }
}

/// Whether the soft shadows are on (#1436).
fn soft(settings: Option<&Settings>) -> bool {
    settings.is_some_and(|settings| settings.graphics.shadows == Shadows::Soft)
}

/// The sun casts while the soft shadows are on, its cascade at the camera's zoom; the world's
/// camera draws its layer.
fn switch(
    mut commands: Commands,
    settings: Option<Res<Settings>>,
    mut suns: Query<(&mut DirectionalLight, &mut CascadeShadowConfig), With<Sun>>,
    eyes: Query<(Entity, &Projection, Option<&RenderLayers>), crate::iso::Eye>,
) {
    let soft = soft(settings.as_deref());
    let mut zoom = 1.0;
    for (eye, projection, layers) in &eyes {
        if let Projection::Orthographic(ortho) = projection {
            zoom = ortho.scale;
        }
        let layers = layers.cloned().unwrap_or_default();
        if !layers.intersects(&RenderLayers::layer(SUN_LAYER)) {
            commands.entity(eye).insert(layers.with(SUN_LAYER));
        }
    }
    for (mut light, mut cascade) in &mut suns {
        if light.shadow_maps_enabled != soft {
            light.shadow_maps_enabled = soft;
        }
        let wanted = sun_cascade(zoom);
        if soft && cascade.bounds != wanted.bounds {
            *cascade = wanted;
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
            .insert_resource(DirectionalLightShadowMap { size: SUN_MAP })
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (cast, soak, ground, detail, switch)
                    .chain()
                    .in_set(Step::Shadows),
            );
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
        let (body, blade, staves) = (mesh(world, hero), mesh(world, sword), mesh(world, barrel));
        app.update();
        let world = app.world_mut();
        let sun = RenderLayers::layer(SUN_LAYER);
        let mut layers = world.query::<&RenderLayers>();
        // The hero's body and its sword in the sun's layer (#2610), the barrel not.
        for (mesh, casts) in [(body, true), (blade, true), (staves, false)] {
            let cast = layers.get(world, mesh).is_ok_and(|l| l.intersects(&sun));
            assert_eq!(cast, casts, "{mesh}");
        }
        // And a flattened copy beside each of the two.
        let mut copies = world.query_filtered::<&ChildOf, With<Shadow>>();
        let mut parents: Vec<Entity> = copies.iter(world).map(ChildOf::parent).collect();
        parents.sort();
        let mut wanted = vec![hero, sword];
        wanted.sort();
        assert_eq!(parents, wanted);
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
    fn the_ground_finds_its_place_for_the_sun() {
        assert_eq!(
            shader_constant(include_str!("ground.wesl"), "GROUND_BEHIND"),
            iso::GROUND_BEHIND
        );
    }

    #[test]
    fn the_sun_leans_as_the_shadows_and_leaves_their_light() {
        assert_eq!(LEAN.x, shader_constant(SHADER, "LEAN_X"));
        assert_eq!(LEAN.z, shader_constant(SHADER, "LEAN_Z"));
        let left = shader_constant(include_str!("soft.wesl"), "LEFT");
        assert!((left - (1.0 - shader_constant(SHADER, "DARKNESS"))).abs() < 1e-6);
    }

    #[test]
    fn a_shadow_keeps_the_models_squeeze() {
        assert_eq!(
            shader_constant(SHADER, "SQUEEZE"),
            shader_constant(include_str!("models.wesl"), "SQUEEZE")
        );
    }
}
