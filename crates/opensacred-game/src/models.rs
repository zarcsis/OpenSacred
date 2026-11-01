//! The models among the sprites (docs/re/drawing.md, "The models"): the client draws a creature
//! or an object of the world at its cell's place in the draw lists, after the cell's statics, and
//! an ordinary sprite without the depth test, so a sprite drawn later covers a model whole and one
//! drawn earlier lies under it whole. The game draws the sprites at the depths of their places
//! (`order.wesl`); a model's meshes take [`ModelMaterial`], whose vertex shader puts the model at
//! its anchor's place, the centre of the cell its root stands in (a script's object's own cell,
//! wherever its model stands), with a small part of its own depth (`models.wesl`). A model's root
//! carries [`Ordered`], with its slot of the [`Anchors`]
//! that its meshes' tags name; a model that another holds, an NPC's weapon, [`OrderedWith`] its
//! holder, whose slot its meshes take. A model standing on an upper storey's floor lies no
//! further back than the floor's plane, as the sprites standing on it do ([`statics::Floor`]).
//!
//! Among themselves the client's models are ordered by their true depths: it draws each with the
//! depth test and the depth write on, and the sprites with neither, so its depth buffer holds the
//! models' depths alone. The camera's depth prepass draws the models alone, at their true depths
//! (`prepass.wesl`), and over each sample a model's main pass keeps its fragment only if no model's
//! texel lies nearer there (`nearest.wesl`, `standard.wesl`): the nearest model then takes its
//! place among the sprites (#998).
//!
//! A mesh whose texture has translucent texels is drawn twice, as the sprites are
//! (docs/decisions/drawing.md, "Models' translucent meshes drawn twice"): its opaque texels through
//! the depth buffer, then, by a copy of the mesh ([`Fringe`]), the others that the original's alpha
//! test keeps, blended by their alpha in the objects' layer of the blended things ([`Pass`]).
//!
//! A model that the original draws under a liquid's surface ([`Sinks`]) lies, while it stands in
//! a liquid cell ([`Sunk`]), as far back as the liquid cells' miniatures, its translucent texels
//! in their layer.

use std::collections::HashMap;

use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::camera::visibility::{DynamicSkinnedMeshBounds, NoFrustumCulling};
use bevy::gltf::Gltf;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::mesh::{MeshTag, MeshVertexBufferLayoutRef};
use bevy::pbr::{
    ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
    MaterialPlugin,
};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError, TextureFormat,
};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::{ShaderDefVal, ShaderRef};
use bevy::world_serialization::WorldInstanceReady;

use crate::blended::Layer;
use crate::buildings::{self, Layered};
use crate::frame::Step;
use crate::hero::Body;
use crate::npcs::Object;
use crate::terrain::Heights;
use crate::{blended, iso, paths, statics};

/// The shader of [`ModelMaterial`], embedded in the binary.
const SHADER: &str = "embedded://opensacred_game/models.wesl";
/// The vertex stage of the prepass of [`ModelMaterial`], embedded in the binary.
const PREPASS: &str = "embedded://opensacred_game/prepass.wesl";

/// A model's place in its cell (`order.wesl`): after the statics of the cell's chain, which take
/// the places from 0, short of the next cell of the diagonal, at 33.8. The model's own depth
/// spreads it half a unit either way (`models.wesl`), over the places of a few cells of its
/// diagonal, and far from the next diagonal's.
pub(crate) const PLACE: f32 = 24.0;

/// The original's alpha test of a model's texels, which drops those of alpha 64 or less:
/// `cGranny::render` (`0x405C90`) turns it on for every model, at the device's `ALPHAREF` 64 and
/// `ALPHAFUNC` `GREATER` (`0x643190`), with the texture's alpha in the first stage (docs/re/
/// drawing.md, "The models"). Bevy keeps a texel of alpha at the cutoff or above it, which lies
/// between 64 and 65 of 255. The same as `CUTOFF` of `standard.wesl`, which tests the texels of
/// [`Pass::Fringe`].
pub(crate) const CUTOFF: f32 = 64.5 / 255.0;

/// The alpha of the opaque texels, which [`Pass::Body`] draws: all of it. The same as `OPAQUE`
/// of `standard.wesl`, which leaves them out of [`Pass::Fringe`].
const OPAQUE: f32 = 254.5 / 255.0;

/// A glTF material of a model, drawn at the model's place in the order.
pub type ModelMaterial = ExtendedMaterial<StandardMaterial, InOrder>;

/// The texels of its glTF material that a [`ModelMaterial`] draws.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pass {
    /// Those that the original's alpha test keeps ([`CUTOFF`]), opaque through the depth buffer:
    /// of a material without translucent texels.
    Tested,
    /// The opaque ones ([`OPAQUE`]), through the depth buffer: of a material with translucent
    /// texels, which [`Pass::Fringe`] draws.
    Body,
    /// The translucent ones that the alpha test keeps, blended by their alpha over what lies
    /// behind them, in the model's layer of the blended things (the objects', or under the
    /// liquids for a [`Sunk`] model), as the original blends them; their depth tested and
    /// written, as the original's (docs/re/drawing.md, "The models").
    Fringe(Layer),
}

/// The models' anchors and floors, which every [`ModelMaterial`] shares, and its pass.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
#[bind_group_data(InOrderKey)]
pub struct InOrder {
    #[storage(100, read_only)]
    anchors: Handle<ShaderBuffer>,
    #[storage(101, read_only)]
    floors: Handle<ShaderBuffer>,
    #[storage(102, read_only)]
    lights: Handle<ShaderBuffer>,
    pass: Pass,
}

/// What the pipeline of a [`ModelMaterial`] depends on beyond Bevy's own material.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct InOrderKey {
    fringe: bool,
}

impl From<&InOrder> for InOrderKey {
    fn from(extension: &InOrder) -> Self {
        Self {
            fringe: matches!(extension.pass, Pass::Fringe(_)),
        }
    }
}

impl MaterialExtension for InOrder {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }

    /// Bevy's own, its colour encoded for the frame (`srgb`).
    fn fragment_shader() -> ShaderRef {
        crate::shaders::STANDARD.into()
    }

    /// The models' true depths, which Bevy's own prepass fragment stage tests by their alpha as the
    /// main pass does.
    fn prepass_vertex_shader() -> ShaderRef {
        PREPASS.into()
    }

    /// None: the game's light casts no shadows, the models' own are drawn by `shadows`, and the
    /// prepass's vertex stage, which Bevy's shadow passes would take, squeezes the depth.
    fn enable_shadows() -> bool {
        false
    }

    /// The depth as the vertex shader puts it: a blended material's `depth_bias` is its layer of
    /// the blended things. In the main pass, a model's fragments take only the samples over which
    /// no model's texel lies nearer; the fringe's keep the texels it draws, and write their depth.
    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        blended::unbias(descriptor);
        if descriptor
            .vertex
            .shader_defs
            .contains(&ShaderDefVal::from("PREPASS_PIPELINE"))
        {
            return Ok(());
        }
        // The fragment stage reads the model's light by its mesh's tag (`standard.wesl`).
        descriptor
            .vertex
            .shader_defs
            .push("VERTEX_OUTPUT_INSTANCE_INDEX".into());
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader_defs.push("NEAREST".into());
            fragment
                .shader_defs
                .push("VERTEX_OUTPUT_INSTANCE_INDEX".into());
            fragment.shader_defs.push("LIT_BY_SLOT".into());
            if key.bind_group_data.fringe {
                fragment.shader_defs.push("FRINGE".into());
            }
        }
        if key.bind_group_data.fringe
            && let Some(depth) = descriptor.depth_stencil.as_mut()
        {
            depth.depth_write_enabled = Some(true);
        }
        Ok(())
    }
}

/// Every model's anchor, by the slot its root holds, for the shader by its meshes' tags: the
/// centre of the model's cell at the cell's height, from which the model's own depth is measured,
/// and the place in the cell. The place in the order is that centre at height 0, as the sprites'
/// (#112). And the floor that the model stands on, if it stands on an upper storey's
/// ([`statics::Floor::under`]), zero for any other; but for a [`Sunk`] model, how far back along
/// the line of sight its place lies, in the third coordinate. And the share of its light's
/// colour that its ambient light takes while the cursor is over it, first, zero for none
/// (`hover`, #1460).
#[derive(Resource)]
pub struct Anchors {
    buffer: Handle<ShaderBuffer>,
    anchors: Vec<Vec4>,
    floor_buffer: Handle<ShaderBuffer>,
    floors: Vec<Vec4>,
    light_buffer: Handle<ShaderBuffer>,
    lights: Vec<Vec4>,
    free: Vec<u32>,
    changed: bool,
}

impl Anchors {
    /// The shader's buffer of the anchors, by slot.
    pub fn buffer(&self) -> &Handle<ShaderBuffer> {
        &self.buffer
    }

    /// The shader's buffer of the floors and of how far back the sunk models' places lie, by slot.
    pub fn floor_buffer(&self) -> &Handle<ShaderBuffer> {
        &self.floor_buffer
    }

    /// Gives the model of `slot` the ambient share `share` of its light, 0 for none.
    pub fn light(&mut self, slot: u32, share: f32) {
        if let Some(light) = usize::try_from(slot)
            .ok()
            .and_then(|slot| self.lights.get_mut(slot))
            && light.x != share
        {
            light.x = share;
            self.changed = true;
        }
    }
}

impl FromWorld for Anchors {
    fn from_world(world: &mut World) -> Self {
        let mut buffers = world.resource_mut::<Assets<ShaderBuffer>>();
        let mut zero = || {
            buffers.add(ShaderBuffer::new(
                vec![[0.0_f32; 4]],
                RenderAssetUsages::default(),
            ))
        };
        Self {
            buffer: zero(),
            anchors: vec![Vec4::ZERO],
            floor_buffer: zero(),
            floors: vec![Vec4::ZERO],
            light_buffer: zero(),
            lights: vec![Vec4::ZERO],
            free: Vec::new(),
            changed: false,
        }
    }
}

/// The root of a model drawn at the place of the cell it stands in, and its slot of the
/// [`Anchors`].
#[derive(Component, Default)]
pub struct Ordered(Option<u32>);

impl Ordered {
    /// The model's slot of the [`Anchors`], which the tags of the meshes at its place name.
    pub fn slot(&self) -> Option<u32> {
        self.0
    }
}

/// The root of a model that the original draws under a liquid's surface while it lies in a
/// liquid cell: the row walk sends a world object of such a cell whose type passes `0x434070`, of
/// the category `0x20` (the waters' crab and fish) or an item of a `Weapon.pak` record, to the
/// liquid cells' objects rather than to the lists, unless it is a chest's (`0x62AE90` at
/// `0x62CB04`), and `0x62E410` draws those before the surfaces (docs/re/drawing.md, "The liquid
/// cells").
#[derive(Component)]
pub struct Sinks;

/// The root of a [`Sinks`] model that stands in a liquid cell: its place in the order lies as far
/// back as the liquid cells' miniatures ([`statics::UNDER_LIQUIDS_BACK`]), under the surfaces,
/// and its translucent texels and its shadow are drawn in their layer ([`Layer::UnderLiquids`]).
#[derive(Component)]
pub struct Sunk;

/// The model material of `base` that draws its texels of `pass`: a mesh of it, tagged with a
/// model's slot of the `anchors`, is drawn at that model's place in the order.
pub fn in_order(base: StandardMaterial, anchors: &Anchors, pass: Pass) -> ModelMaterial {
    let (alpha_mode, depth_bias) = match pass {
        Pass::Tested => (AlphaMode::Mask(CUTOFF), base.depth_bias),
        Pass::Body => (AlphaMode::Mask(OPAQUE), base.depth_bias),
        Pass::Fringe(layer) => (AlphaMode::Blend, layer.bias()),
    };
    ModelMaterial {
        base: StandardMaterial {
            alpha_mode,
            depth_bias,
            ..base
        },
        extension: InOrder {
            anchors: anchors.buffer.clone(),
            floors: anchors.floor_buffer.clone(),
            lights: anchors.light_buffer.clone(),
            pass,
        },
    }
}

/// Whether some texels of `base` are translucent, of alpha under 1 and so drawn in two passes:
/// those of its colour, or of its texture's image (`None` while the image loads; an image that
/// failed to load has none).
fn translucent(
    base: &StandardMaterial,
    images: &Assets<Image>,
    server: &AssetServer,
) -> Option<bool> {
    if base.base_color.alpha() < 1.0 {
        return Some(true);
    }
    let Some(texture) = &base.base_color_texture else {
        return Some(false);
    };
    match images.get(texture) {
        Some(image) => Some(translucent_texels(image)),
        None if server.load_state(texture).is_loading() => None,
        None => Some(false),
    }
}

/// Whether some texels of `image` are of alpha under 255: of 8-bit RGBA, as the converter writes
/// every texture with such a texel (`opensacred_data::texture`), by their alpha; of BC7, which it
/// writes only of opaque images, none; of another format or without its data, taken to be.
fn translucent_texels(image: &Image) -> bool {
    match (image.texture_descriptor.format, &image.data) {
        (TextureFormat::Rgba8UnormSrgb | TextureFormat::Rgba8Unorm, Some(data)) => data
            .as_chunks::<4>()
            .0
            .iter()
            .any(|&[.., alpha]| alpha < u8::MAX),
        (TextureFormat::Bc7RgbaUnormSrgb | TextureFormat::Bc7RgbaUnorm, _) => false,
        _ => true,
    }
}

/// The root of a model drawn at its holder's place: an NPC's weapon.
#[derive(Component)]
pub struct OrderedWith(pub Entity);

/// The glTF that a model's scene comes from, held while the model stands (#1626): a glTF that no
/// handle holds is unloaded while its scene stays with the models that show it, and a load of it
/// again puts a new scene in the old one's place, which Bevy's spawner then makes anew in every
/// model of it, for a frame as the glTF has it (its own materials, a worn item's skinned meshes on
/// the item's own skeleton, at the wearer's feet).
#[derive(Component)]
pub struct Source(
    #[expect(
        dead_code,
        reason = "held, never read: the handle keeps the glTF loaded"
    )]
    pub Handle<Gltf>,
);

/// The scene of the glTF `model` for a model's root, with the glTF it comes from ([`Source`]).
pub fn scene(assets: &AssetServer, model: &str) -> (WorldAssetRoot, Source) {
    (
        WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(model.to_owned()))),
        Source(assets.load(model.to_owned())),
    )
}

/// A model whose scene is ready and whose meshes have their glTF materials still.
#[derive(Component)]
pub struct Unplaced;

/// A mesh of a model whose scene has just spawned, hidden until [`convert`] gives it its model
/// material (#1628). Drawn before, it showed for a frame the glTF's material, lit by the scene's
/// lights and out of the order, and a worn item's mesh on the item's own skeleton, which
/// `npcs::bind_worn` binds to its wearer's bones in the frame [`convert`] runs in.
#[derive(Component)]
pub struct Veiled;

/// The copy of a model's mesh that draws the translucent texels of its material
/// ([`Pass::Fringe`]), a child of the mesh: its material in the objects' layer, and in the layer
/// under the liquids, which it takes while its model is [`Sunk`].
#[derive(Component, Clone)]
pub struct Fringe {
    dry: Handle<ModelMaterial>,
    wet: Handle<ModelMaterial>,
}

impl Fringe {
    /// The material of a fringe whose model is sunk or not.
    fn material(&self, sunk: bool) -> &Handle<ModelMaterial> {
        if sunk { &self.wet } else { &self.dry }
    }
}

/// The model materials made of a glTF material: of its meshes, and of their [`Fringe`] copies if
/// it has translucent texels.
#[derive(Clone)]
struct Made {
    mesh: Handle<ModelMaterial>,
    fringe: Option<Fringe>,
}

/// The model materials made of the glTF ones, by the glTF material.
#[derive(Resource, Default)]
pub struct Converted(HashMap<AssetId<StandardMaterial>, Made>);

/// The models' material, its shader and the buffer of the anchors, and the models' places.
pub struct ModelsPlugin;

impl Plugin for ModelsPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "models.wesl");
        embedded_asset!(app, "prepass.wesl");
        crate::shaders::embed_standard(app);
        app.add_plugins(MaterialPlugin::<ModelMaterial>::default())
            .init_resource::<Anchors>()
            .init_resource::<Converted>()
            .add_observer(hold)
            .add_observer(release)
            .add_observer(ready)
            .add_systems(Update, (convert, place, soak).chain().in_set(Step::Models));
    }
}

/// The anchor of a model whose root stands at `position`: the centre of its cell, at the cell's
/// height and as high again over it as the root stands over its own ground (on a building's
/// floor), from which the model's own depth is measured, after the cell's statics.
fn anchor(heights: &Heights, position: Vec3) -> Vec4 {
    let centre = iso::ground(
        (position.x / iso::CELL).floor() + 0.5,
        (position.z / iso::CELL).floor() + 0.5,
    );
    let lift = heights
        .at(position.x, position.z)
        .map_or(0.0, |ground| position.y - ground);
    let height = heights
        .at(centre.x, centre.z)
        .map_or(position.y, |ground| ground + lift);
    Vec4::new(centre.x, height, centre.z, PLACE)
}

/// The place in the order of a model whose root stands at `position`: the centre of its cell at
/// height 0, after the cell's statics, as `models.wesl` takes its anchor.
pub fn place_of(position: Vec3) -> Vec4 {
    let centre = iso::ground(
        (position.x / iso::CELL).floor() + 0.5,
        (position.z / iso::CELL).floor() + 0.5,
    );
    Vec4::new(centre.x, 0.0, centre.z, PLACE)
}

/// Gives a new model root a slot of the anchors.
fn hold(added: On<Add<Ordered>>, mut anchors: ResMut<Anchors>, mut roots: Query<&mut Ordered>) {
    let Ok(mut ordered) = roots.get_mut(added.entity) else {
        return;
    };
    let slot = anchors.free.pop().or_else(|| {
        let slot = u32::try_from(anchors.anchors.len()).ok()?;
        anchors.anchors.push(Vec4::ZERO);
        anchors.floors.push(Vec4::ZERO);
        anchors.lights.push(Vec4::ZERO);
        Some(slot)
    });
    // A slot taken again lights its new model plainly.
    if let Some(slot) = slot {
        anchors.light(slot, 0.0);
    }
    ordered.0 = slot;
}

/// Frees the slot of a model root that goes.
fn release(removed: On<Remove<Ordered>>, mut anchors: ResMut<Anchors>, roots: Query<&Ordered>) {
    if let Ok(Ordered(Some(slot))) = roots.get(removed.entity) {
        anchors.free.push(*slot);
    }
}

/// A model's root, of its own place or its holder's.
type Model = Or<(With<Ordered>, With<OrderedWith>)>;

/// Marks a model whose scene has spawned, to give its meshes their places, and veils its meshes
/// until then ([`Veiled`]). A model of no slot, which [`convert`] leaves as it is (the items of the
/// choice's figures, whose holders are not in the order), is shown at once.
fn ready(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    models: Query<Option<&OrderedWith>, Model>,
    roots: Query<&Ordered>,
    children: Query<&Children>,
    raw: Query<(), With<MeshMaterial3d<StandardMaterial>>>,
) {
    let Ok(with) = models.get(ready.entity) else {
        return;
    };
    commands.entity(ready.entity).insert(Unplaced);
    let root = with.map_or(ready.entity, |with| with.0);
    if !roots.get(root).is_ok_and(|ordered| ordered.0.is_some()) {
        return;
    }
    for entity in children.iter_descendants(ready.entity) {
        if raw.contains(entity) {
            commands.entity(entity).insert((Veiled, Visibility::Hidden));
        }
    }
}

/// Shows the meshes of `model` that [`ready`] veiled.
fn unveil(
    commands: &mut Commands,
    children: &Query<&Children>,
    veiled: &Query<(), With<Veiled>>,
    model: Entity,
) {
    for entity in children.iter_descendants(model) {
        if veiled.contains(entity) {
            commands
                .entity(entity)
                .remove::<Veiled>()
                .insert(Visibility::Inherited);
        }
    }
}

/// A mesh of a model as [`convert`] reads it: its glTF material, and what its [`Fringe`] copy
/// takes of it.
type ModelMesh = (
    &'static MeshMaterial3d<StandardMaterial>,
    &'static Mesh3d,
    Option<&'static SkinnedMesh>,
    Has<DynamicSkinnedMeshBounds>,
    Has<NoFrustumCulling>,
);

/// Gives the meshes of each model that has just spawned the model materials of their glTF
/// materials (after `npcs::retexture` has given them their item's texture) and their root's
/// slot of the anchors for a tag; and each mesh whose material has translucent texels a
/// [`Fringe`] copy, the same mesh skinned to the same joints, tagged with the same slot, in its
/// root's layer; then shows its veiled meshes ([`Veiled`]). A model waits, veiled, for the images
/// of its materials, whose alpha says which have them.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its queries and resources as arguments"
)]
pub fn convert(
    mut commands: Commands,
    models: Query<(Entity, Option<&OrderedWith>), With<Unplaced>>,
    roots: Query<(&Ordered, Has<Sunk>)>,
    children: Query<&Children>,
    meshes: Query<ModelMesh>,
    veiled: Query<(), With<Veiled>>,
    standard: Res<Assets<StandardMaterial>>,
    images: Res<Assets<Image>>,
    server: Res<AssetServer>,
    mut materials: ResMut<Assets<ModelMaterial>>,
    mut converted: ResMut<Converted>,
    anchors: Res<Anchors>,
) {
    'models: for (model, with) in &models {
        let root = with.map_or(model, |with| with.0);
        let Ok((Ordered(Some(slot)), sunk)) = roots.get(root) else {
            commands.entity(model).remove::<Unplaced>();
            unveil(&mut commands, &children, &veiled, model);
            continue;
        };
        let mut made = Vec::new();
        for entity in children.iter_descendants(model) {
            let Ok((material, ..)) = meshes.get(entity) else {
                continue;
            };
            let Some(base) = standard.get(&material.0) else {
                continue;
            };
            if let Some(both) = converted.0.get(&material.0.id()) {
                made.push((entity, both.clone()));
                continue;
            }
            let Some(translucent) = translucent(base, &images, &server) else {
                continue 'models;
            };
            let both = if translucent {
                let mut in_layer =
                    |layer| materials.add(in_order(base.clone(), &anchors, Pass::Fringe(layer)));
                let fringe = Fringe {
                    dry: in_layer(Layer::Objects),
                    wet: in_layer(Layer::UnderLiquids),
                };
                Made {
                    mesh: materials.add(in_order(base.clone(), &anchors, Pass::Body)),
                    fringe: Some(fringe),
                }
            } else {
                Made {
                    mesh: materials.add(in_order(base.clone(), &anchors, Pass::Tested)),
                    fringe: None,
                }
            };
            converted.0.insert(material.0.id(), both.clone());
            made.push((entity, both));
        }
        commands.entity(model).remove::<Unplaced>();
        unveil(&mut commands, &children, &veiled, model);
        for (entity, Made { mesh: own, fringe }) in made {
            commands
                .entity(entity)
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert((MeshMaterial3d(own), MeshTag::new(*slot)));
            let (Some(fringe), Ok((_, mesh, skinned, dynamic, unculled))) =
                (fringe, meshes.get(entity))
            else {
                continue;
            };
            let mut copy = commands.spawn((
                Mesh3d(mesh.0.clone()),
                MeshMaterial3d(fringe.material(sunk).clone()),
                MeshTag::new(*slot),
                ChildOf(entity),
                fringe,
            ));
            if let Some(skinned) = skinned {
                copy.insert(skinned.clone());
            }
            if dynamic {
                copy.insert(DynamicSkinnedMeshBounds);
            }
            if unculled {
                copy.insert(NoFrustumCulling);
            }
        }
    }
}

/// The building's part whose floor a model stands on, if it stands on an upper storey's: the
/// hero's by its level, an NPC's or an object's by its layer. A model on the ground or on a
/// building's floor 1 stands on none.
fn storey(
    heights: &Heights,
    layered: Option<&Layered>,
    body: Option<&Body>,
) -> Option<(buildings::Anchor, u8)> {
    let (building, floor) = match (body, layered) {
        (Some(body), _) => heights.floor(body.level())?,
        (
            None,
            Some(&Layered {
                building: Some(building),
                layer,
                ..
            }),
        ) => (building, u8::try_from(layer).ok()?),
        _ => return None,
    };
    let level = heights.floor_level(building, usize::from(floor));
    if heights.lift(level) <= 0.0 {
        return None;
    }
    Some((building, 1u8.checked_shl(u32::from(floor))?))
}

/// A model's root as [`place`] reads it: where it stands, its slot, its layer, the hero's body, a
/// script's object, and whether it sinks under a liquid's surface and is sunk now.
type Root = (
    Entity,
    &'static Transform,
    &'static Ordered,
    Option<&'static Layered>,
    Option<&'static Body>,
    Option<&'static Object>,
    (Has<Sinks>, Has<Sunk>),
);

/// Whether a model that [`Sinks`] whose root stands at `root` lies under a liquid's surface: in a
/// liquid cell, of the natures `WATERA` and `WATERB` (`0x62AE90`).
fn sunk(heights: &Heights, root: Vec3) -> bool {
    heights.liquid_bed(paths::cell_of(root.x, root.z)).is_some()
}

/// Keeps each model's anchor at the cell its root stands in, and the floor it stands on, or how
/// far back it lies under a liquid's surface ([`Sunk`]), and the shader's buffers with them. A
/// script's object takes its own cell's place, its logical position's, wherever its model stands:
/// the client chains the object into the list of the cell at its position on its layer
/// (`0x5FCCB0`), which the row walk draws after the cell's statics (`0x62AE90`), while it draws
/// the model at the floats `+0xCC…+0xD4` (docs/re/drawing.md, "The models"); so the leaf of an
/// upper storey's door, whose model stands in the cell before its own, is drawn over the back
/// wall's sprites of its cell (#459).
pub fn place(
    mut commands: Commands,
    heights: Res<Heights>,
    roots: Query<Root>,
    floors: Query<&statics::Floor>,
    mut anchors: ResMut<Anchors>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
) {
    let floors: HashMap<(buildings::Anchor, u8), &statics::Floor> =
        floors.iter().map(|floor| (floor.part, floor)).collect();
    for (entity, transform, ordered, layered, body, object, (sinks, was)) in &roots {
        let Some(slot) = ordered.0.and_then(|slot| usize::try_from(slot).ok()) else {
            continue;
        };
        let root = object.map_or(transform.translation, |object| object.at);
        let at = anchor(&heights, root);
        let under = sinks && sunk(&heights, root);
        if under != was {
            if under {
                commands.entity(entity).insert(Sunk);
            } else {
                commands.entity(entity).remove::<Sunk>();
            }
        }
        let on = if under {
            Vec4::new(0.0, 0.0, statics::UNDER_LIQUIDS_BACK, 0.0)
        } else {
            storey(&heights, layered, body)
                .and_then(|part| floors.get(&part))
                .map_or(Vec4::ZERO, |floor| floor.under(place_of(root)))
        };
        let anchors = &mut *anchors;
        if let (Some(anchor), Some(floor)) =
            (anchors.anchors.get_mut(slot), anchors.floors.get_mut(slot))
            && (*anchor, *floor) != (at, on)
        {
            (*anchor, *floor) = (at, on);
            anchors.changed = true;
        }
    }
    if anchors.changed {
        anchors.changed = false;
        for (handle, values) in [
            (&anchors.buffer, &anchors.anchors),
            (&anchors.floor_buffer, &anchors.floors),
            (&anchors.light_buffer, &anchors.lights),
        ] {
            let data: Vec<[f32; 4]> = values.iter().map(|a| a.to_array()).collect();
            if let Some(mut buffer) = buffers.get_mut(handle) {
                *buffer = ShaderBuffer::new(data, RenderAssetUsages::default());
            }
        }
    }
}

/// Gives the [`Fringe`] copies of each model that has just sunk under a liquid's surface, or come
/// out of one, the material of its layer.
fn soak(
    sunk: Query<Entity, Added<Sunk>>,
    mut dried: RemovedComponents<Sunk>,
    wet: Query<(), With<Sunk>>,
    children: Query<&Children>,
    mut fringes: Query<(&Fringe, &mut MeshMaterial3d<ModelMaterial>)>,
) {
    for root in sunk.iter().chain(dried.read()) {
        let under = wet.contains(root);
        for entity in children.iter_descendants(root) {
            if let Ok((fringe, mut material)) = fringes.get_mut(entity) {
                let wanted = fringe.material(under);
                if material.0 != *wanted {
                    material.0 = wanted.clone();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::render::render_resource::{Extent3d, TextureDimension};
    use opensacred_data::world::{Building, Sector};

    use super::*;
    use crate::statics::tests::{ORDER, shader_constant};

    #[test]
    fn a_models_gltf_stays_loaded_while_the_model_stands() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::world_serialization::WorldSerializationPlugin,
        ))
        .init_asset::<Gltf>();
        let model = "models/item.glb";
        let server = app.world().resource::<AssetServer>().clone();
        let root = app.world_mut().spawn(scene(&server, model)).id();
        let held = || server.get_handle::<Gltf>(model).map(|held| held.id());
        // Its load, of no file, fails on the server's tasks, which hold the glTF while they run:
        // the frames go on until it has failed, however long that takes (#1632).
        let gltf = held().expect("a load");
        for _ in 0..10_000 {
            if matches!(server.load_state(gltf), bevy::asset::LoadState::Failed(_)) {
                break;
            }
            app.update();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(matches!(
            server.load_state(gltf),
            bevy::asset::LoadState::Failed(_)
        ));
        // While the model stands its glTF is held, so that a load of it again finds the same
        // glTF rather than loading it anew, which would replace the model's scene (#1626).
        assert_eq!(held(), Some(gltf));
        app.world_mut().entity_mut(root).despawn();
        app.update();
        assert_eq!(held(), None);
    }

    #[test]
    fn a_models_meshes_wait_hidden_for_their_model_material() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::world_serialization::WorldSerializationPlugin,
        ))
        .register_type::<ChildOf>()
        .register_type::<Children>()
        .init_asset::<Image>()
        .init_asset::<StandardMaterial>()
        .init_asset::<ModelMaterial>()
        .init_asset::<ShaderBuffer>()
        .init_resource::<Anchors>()
        .init_resource::<Converted>()
        .add_observer(hold)
        .add_observer(ready)
        .add_systems(Update, convert);
        let mut empty = World::new();
        empty.spawn_empty();
        let world = app.world_mut();
        let scene = world
            .resource_mut::<Assets<bevy::world_serialization::WorldAsset>>()
            .add(bevy::world_serialization::WorldAsset::new(empty));
        let material = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let mesh_of = |world: &mut World, model| {
            world
                .spawn((
                    Mesh3d(Handle::default()),
                    MeshMaterial3d(material.clone()),
                    Visibility::default(),
                    ChildOf(model),
                ))
                .id()
        };
        // A model in the order, and an item held by a figure that is not (the choice's).
        let model = world
            .spawn((WorldAssetRoot(scene.clone()), Ordered::default()))
            .id();
        let mesh = mesh_of(world, model);
        let figure = world.spawn(Transform::default()).id();
        let item = world
            .spawn((WorldAssetRoot(scene), OrderedWith(figure)))
            .id();
        let held = mesh_of(world, item);
        let visibility = |app: &App, entity| *app.world().get::<Visibility>(entity).unwrap();
        // The frame the scenes spawn in.
        app.update();
        assert_eq!(visibility(&app, mesh), Visibility::Hidden);
        assert_eq!(visibility(&app, held), Visibility::Inherited);
        // The next, which gives the model's mesh its material.
        app.update();
        assert_eq!(visibility(&app, mesh), Visibility::Inherited);
        assert!(
            app.world()
                .get::<MeshMaterial3d<ModelMaterial>>(mesh)
                .is_some()
        );
        assert!(app.world().get::<Veiled>(mesh).is_none());
    }

    #[test]
    fn a_model_keeps_within_its_cells_place() {
        let squeeze = shader_constant(include_str!("models.wesl"), "SQUEEZE");
        let (chain, across) = (
            shader_constant(ORDER, "CHAIN"),
            shader_constant(ORDER, "ACROSS"),
        );
        // The place: after a chain of 16 statics, short of the next cell of the diagonal, a
        // cell's diagonal apart along the screen's right.
        let next = across * iso::CELL * std::f32::consts::SQRT_2;
        assert!(chain * 16.0 < chain * PLACE && chain * PLACE < next);
        // A model 80 units deep, half of it either way of its place, stays far from the places
        // of the next diagonal, a cell along the grid further back.
        let half = squeeze * 40.0;
        let diagonal = iso::CELL * Vec3::X.dot(-iso::forward());
        assert!(chain * PLACE + half < diagonal / 10.0);
        // Its parts a tenth of a unit apart stay a step of the depth buffer (a thousandth of a
        // unit, docs/05 §3.6) apart.
        assert!(squeeze * 0.1 >= 0.001);
    }

    #[test]
    fn the_models_true_depths_lie_behind_all_that_is_drawn() {
        let mut world = World::new();
        let camera = world.spawn(iso::camera(Vec3::ZERO, 1.0)).id();
        let Some(Projection::Orthographic(ortho)) = world.get::<Projection>(camera) else {
            panic!("the camera's projection is not orthographic");
        };
        // The reversed depth of a point so far from the camera.
        let reversed = |distance: f32| (ortho.far - distance) / (ortho.far - ortho.near);
        let nearest = include_str!("nearest.wesl");
        let part = shader_constant(nearest, "PART");
        // The prepass's depths of the models lie behind the ground, the farthest of what the main
        // pass draws, with 2,000 units to spare for the ground's heights about the point looked at.
        assert!(part < reversed(iso::DISTANCE + iso::GROUND_BEHIND + 2_000.0));
        // The tolerance, half a unit of the world, spans a hundred of the depth buffer's steps
        // there, and a step is a few thousandths of a unit.
        let tolerance = shader_constant(nearest, "TOLERANCE") * part / (ortho.far - ortho.near);
        assert!(tolerance > 100.0 * part * f32::EPSILON);
    }

    /// The anchors of a world of their own.
    fn anchors() -> Anchors {
        let mut world = World::new();
        world.init_resource::<Assets<ShaderBuffer>>();
        Anchors::from_world(&mut world)
    }

    /// The least alpha of 255 that a mask of `material` keeps: Bevy keeps a texel of alpha at its
    /// cutoff or above.
    fn mask(material: &ModelMaterial) -> u8 {
        let AlphaMode::Mask(cutoff) = material.base.alpha_mode else {
            panic!("{:?}", material.base.alpha_mode);
        };
        (0..=u8::MAX)
            .find(|&alpha| f32::from(alpha) / 255.0 >= cutoff)
            .unwrap()
    }

    #[test]
    fn a_models_texels_are_tested_as_the_originals() {
        // A glTF material without an alpha mode, OPAQUE, as the converter writes every one; the
        // original keeps the texels of alpha above 64.
        let material = StandardMaterial::default();
        let tested = in_order(material.clone(), &anchors(), Pass::Tested);
        assert_eq!(mask(&tested), 65);
        // In two passes, the opaque texels through the depth buffer, and the others that the test
        // keeps blended in the objects' layer: the fringe's shader keeps those of alpha 65 to 254.
        let body = in_order(material.clone(), &anchors(), Pass::Body);
        assert_eq!(mask(&body), 255);
        let fringe = in_order(material.clone(), &anchors(), Pass::Fringe(Layer::Objects));
        assert_eq!(fringe.base.alpha_mode, AlphaMode::Blend);
        assert_eq!(fringe.base.depth_bias, Layer::Objects.bias());
        // A sunk model's, under the liquids' surfaces.
        let wet = in_order(material, &anchors(), Pass::Fringe(Layer::UnderLiquids));
        assert_eq!(wet.base.depth_bias, Layer::UnderLiquids.bias());
        let shader = include_str!("standard.wesl");
        let kept = |alpha: u8| {
            let alpha = f32::from(alpha) / 255.0;
            alpha >= shader_constant(shader, "CUTOFF") && alpha < shader_constant(shader, "OPAQUE")
        };
        assert!(!kept(64) && kept(65) && kept(254) && !kept(255));
    }

    #[test]
    fn a_material_with_translucent_texels_takes_two_passes() {
        let image = |alpha: u8, format| {
            Image::new_fill(
                Extent3d {
                    width: 2,
                    height: 2,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &[10, 20, 30, alpha],
                format,
                RenderAssetUsages::default(),
            )
        };
        assert!(!translucent_texels(&image(
            255,
            TextureFormat::Rgba8UnormSrgb
        )));
        assert!(translucent_texels(&image(
            200,
            TextureFormat::Rgba8UnormSrgb
        )));
        // Texels of alpha 0, which the alpha test drops, count too: the filtered alpha at their
        // edges lies between.
        assert!(translucent_texels(&image(0, TextureFormat::Rgba8Unorm)));
        // BC7, which the converter writes of opaque images alone.
        let mut bc7 = image(0, TextureFormat::Rgba8UnormSrgb);
        bc7.texture_descriptor.format = TextureFormat::Bc7RgbaUnormSrgb;
        assert!(!translucent_texels(&bc7));
        // A format the converter does not write.
        assert!(translucent_texels(&image(
            255,
            TextureFormat::Bgra8UnormSrgb
        )));
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>();
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        let opaque = images.add(image(255, TextureFormat::Rgba8UnormSrgb));
        let ghostly = images.add(image(128, TextureFormat::Rgba8UnormSrgb));
        let (images, server) = (
            app.world().resource::<Assets<Image>>(),
            app.world().resource::<AssetServer>(),
        );
        let of = |texture: Option<Handle<Image>>, base_color| StandardMaterial {
            base_color,
            base_color_texture: texture,
            ..default()
        };
        assert_eq!(
            translucent(&of(None, Color::WHITE), images, server),
            Some(false)
        );
        assert_eq!(
            translucent(&of(Some(opaque.clone()), Color::WHITE), images, server),
            Some(false)
        );
        assert_eq!(
            translucent(&of(Some(ghostly), Color::WHITE), images, server),
            Some(true)
        );
        assert_eq!(
            translucent(
                &of(Some(opaque), Color::srgba(1.0, 1.0, 1.0, 0.5)),
                images,
                server
            ),
            Some(true)
        );
    }

    #[test]
    fn a_model_stands_on_its_buildings_upper_storeys() {
        // A building anchored at the cell (5, 5), its floor 1 on the ground, its floor 2 140 units
        // of the models' height up.
        let sector = Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells: Vec::new(),
            statics: Vec::new(),
            layers: Vec::new(),
            buildings: vec![Building {
                cell: 5 * 64 + 5,
                floors: vec![1, 2],
                heights: vec![0, 140],
                open: false,
            }],
        };
        let mut heights = Heights::default();
        heights.add(&sector);
        let on = |building, layer| {
            storey(
                &heights,
                Some(&Layered::of(&heights, building, layer)),
                None,
            )
        };
        // On floor 2, its part of mask 4; on floor 1 and on the ground, no floor's.
        assert_eq!(on(Some((5, 5)), 2), Some(((5, 5), 4)));
        assert_eq!(on(Some((5, 5)), 1), None);
        assert_eq!(on(Some((5, 5)), 0), None);
        assert_eq!(on(None, 2), None);
        assert_eq!(storey(&heights, None, None), None);
    }

    #[test]
    fn a_model_takes_the_place_of_its_cells_centre() {
        let heights = Heights::default();
        // Anywhere in cell (3250, 2520): its centre, after the cell's statics.
        let inside = iso::ground(3250.8, 2520.1) + Vec3::Y * 7.0;
        let centre = iso::ground(3250.5, 2520.5);
        assert_eq!(
            anchor(&heights, inside),
            Vec4::new(centre.x, 7.0, centre.z, PLACE)
        );
    }

    #[test]
    fn a_sinking_model_lies_under_the_surface_in_a_liquid_cell() {
        use bevy::ecs::system::RunSystemOnce;
        use opensacred_data::world::{Cell, SIDE};

        // A sector whose cell (3, 0) lies under the water of its first slot, over a bed of −10.
        let cell = |ground| Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [-10; 4],
            light: [255; 4],
            heights2: [0; 4],
            offsets: [0; 2],
            flags: 0,
            ground,
        };
        let mut cells = vec![cell(0x40); SIDE * SIDE];
        cells[3] = cell(0x92);
        let sector = Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [1, 0],
            cells,
            statics: Vec::new(),
            layers: Vec::new(),
            buildings: Vec::new(),
        };
        let mut heights = Heights::default();
        heights.add(&sector);
        let (wet, dry) = (iso::ground(3.5, 0.5), iso::ground(4.5, 0.5));
        assert!(sunk(&heights, wet) && !sunk(&heights, dry));

        let mut world = World::new();
        world.init_resource::<Assets<ShaderBuffer>>();
        world.insert_resource(heights);
        world.init_resource::<Anchors>();
        world.add_observer(hold);
        // A fish in the water, and a model that does not sink beside it.
        let root = |world: &mut World, at: Vec3| {
            world
                .spawn((Transform::from_translation(at), Ordered::default()))
                .id()
        };
        let fish = root(&mut world, wet);
        world.entity_mut(fish).insert(Sinks);
        let other = root(&mut world, wet);
        world.run_system_once(place).unwrap();
        let floor = |world: &World, entity: Entity| {
            let slot = world.get::<Ordered>(entity).unwrap().slot().unwrap();
            world.resource::<Anchors>().floors[usize::try_from(slot).unwrap()]
        };
        assert!(world.entity(fish).contains::<Sunk>());
        assert_eq!(
            floor(&world, fish),
            Vec4::new(0.0, 0.0, statics::UNDER_LIQUIDS_BACK, 0.0)
        );
        assert!(!world.entity(other).contains::<Sunk>());
        assert_eq!(floor(&world, other), Vec4::ZERO);
        // Out of the water, it lies among the lists again.
        world.get_mut::<Transform>(fish).unwrap().translation = dry;
        world.run_system_once(place).unwrap();
        assert!(!world.entity(fish).contains::<Sunk>());
        assert_eq!(floor(&world, fish), Vec4::ZERO);
    }
}
