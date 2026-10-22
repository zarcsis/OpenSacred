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
//! A mesh whose texture has translucent texels is drawn twice, as the sprites are
//! (docs/decisions/2026-10-22-model-alpha.md): its opaque texels through the depth buffer, then,
//! by a copy of the mesh ([`Fringe`]), the others that the original's alpha test keeps, blended by
//! their alpha in the objects' layer of the blended things ([`Pass`]).

use std::collections::HashMap;

use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::camera::visibility::{DynamicSkinnedMeshBounds, NoFrustumCulling};
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
use bevy::shader::ShaderRef;
use bevy::world_serialization::WorldInstanceReady;

use crate::blended::Layer;
use crate::buildings::{self, Layered};
use crate::frame::Step;
use crate::hero::Body;
use crate::npcs::Object;
use crate::terrain::Heights;
use crate::{blended, iso, statics};

/// The shader of [`ModelMaterial`], embedded in the binary.
const SHADER: &str = "embedded://opensacred_game/models.wesl";

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
const CUTOFF: f32 = 64.5 / 255.0;

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
    /// behind them, in the objects' layer of the blended things, as the original blends them;
    /// their depth tested and written, as the original's (docs/re/drawing.md, "The models").
    Fringe,
}

/// The models' anchors and floors, which every [`ModelMaterial`] shares, and its pass.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
#[bind_group_data(InOrderKey)]
pub struct InOrder {
    #[storage(100, read_only)]
    anchors: Handle<ShaderBuffer>,
    #[storage(101, read_only)]
    floors: Handle<ShaderBuffer>,
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
            fringe: extension.pass == Pass::Fringe,
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

    /// The depth as the vertex shader puts it: a blended material's `depth_bias` is its layer of
    /// the blended things. The fringe's fragments keep the texels it draws, and write their depth.
    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        blended::unbias(descriptor);
        if key.bind_group_data.fringe {
            if let Some(fragment) = descriptor.fragment.as_mut() {
                fragment.shader_defs.push("FRINGE".into());
            }
            if let Some(depth) = descriptor.depth_stencil.as_mut() {
                depth.depth_write_enabled = Some(true);
            }
        }
        Ok(())
    }
}

/// Every model's anchor, by the slot its root holds, for the shader by its meshes' tags: the
/// centre of the model's cell at the cell's height, from which the model's own depth is measured,
/// and the place in the cell. The place in the order is that centre at height 0, as the sprites'
/// (#112). And the floor that the model stands on, if it stands on an upper storey's
/// ([`statics::Floor::under`]), zero for any other.
#[derive(Resource)]
pub struct Anchors {
    buffer: Handle<ShaderBuffer>,
    anchors: Vec<Vec4>,
    floor_buffer: Handle<ShaderBuffer>,
    floors: Vec<Vec4>,
    free: Vec<u32>,
    changed: bool,
}

impl Anchors {
    /// The shader's buffer of the anchors, by slot.
    pub fn buffer(&self) -> &Handle<ShaderBuffer> {
        &self.buffer
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

/// The model material of `base` that draws its texels of `pass`: a mesh of it, tagged with a
/// model's slot of the `anchors`, is drawn at that model's place in the order.
pub fn in_order(base: StandardMaterial, anchors: &Anchors, pass: Pass) -> ModelMaterial {
    let (alpha_mode, depth_bias) = match pass {
        Pass::Tested => (AlphaMode::Mask(CUTOFF), base.depth_bias),
        Pass::Body => (AlphaMode::Mask(OPAQUE), base.depth_bias),
        Pass::Fringe => (AlphaMode::Blend, Layer::Objects.bias()),
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
/// the models' textures (`opensacred-convert textures`, `models`), by their alpha; of another
/// format or without its data, taken to be.
fn translucent_texels(image: &Image) -> bool {
    match (image.texture_descriptor.format, &image.data) {
        (TextureFormat::Rgba8UnormSrgb | TextureFormat::Rgba8Unorm, Some(data)) => data
            .as_chunks::<4>()
            .0
            .iter()
            .any(|&[.., alpha]| alpha < u8::MAX),
        _ => true,
    }
}

/// The root of a model drawn at its holder's place: an NPC's weapon.
#[derive(Component)]
pub struct OrderedWith(pub Entity);

/// A model whose scene is ready and whose meshes have their glTF materials still.
#[derive(Component)]
pub struct Unplaced;

/// The copy of a model's mesh that draws the translucent texels of its material
/// ([`Pass::Fringe`]), a child of the mesh.
#[derive(Component)]
pub struct Fringe;

/// The model materials made of a glTF material: of its meshes, and of their [`Fringe`] copies if
/// it has translucent texels.
#[derive(Clone)]
struct Made {
    mesh: Handle<ModelMaterial>,
    fringe: Option<Handle<ModelMaterial>>,
}

/// The model materials made of the glTF ones, by the glTF material.
#[derive(Resource, Default)]
pub struct Converted(HashMap<AssetId<StandardMaterial>, Made>);

/// The models' material, its shader and the buffer of the anchors, and the models' places.
pub struct ModelsPlugin;

impl Plugin for ModelsPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "models.wesl");
        crate::shaders::embed_standard(app);
        app.add_plugins(MaterialPlugin::<ModelMaterial>::default())
            .init_resource::<Anchors>()
            .init_resource::<Converted>()
            .add_observer(hold)
            .add_observer(release)
            .add_observer(ready)
            .add_systems(Update, (convert, place).chain().in_set(Step::Models));
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
        Some(slot)
    });
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

/// Marks a model whose scene has spawned, to give its meshes their places.
fn ready(ready: On<WorldInstanceReady>, mut commands: Commands, models: Query<(), Model>) {
    if models.contains(ready.entity) {
        commands.entity(ready.entity).insert(Unplaced);
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
/// [`Fringe`] copy, the same mesh skinned to the same joints, tagged with the same slot. A model
/// waits for the images of its materials, whose alpha says which have them.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its queries and resources as arguments"
)]
pub fn convert(
    mut commands: Commands,
    models: Query<(Entity, Option<&OrderedWith>), With<Unplaced>>,
    roots: Query<&Ordered>,
    children: Query<&Children>,
    meshes: Query<ModelMesh>,
    standard: Res<Assets<StandardMaterial>>,
    images: Res<Assets<Image>>,
    server: Res<AssetServer>,
    mut materials: ResMut<Assets<ModelMaterial>>,
    mut converted: ResMut<Converted>,
    anchors: Res<Anchors>,
) {
    'models: for (model, with) in &models {
        let root = with.map_or(model, |with| with.0);
        let Ok(Ordered(Some(slot))) = roots.get(root) else {
            commands.entity(model).remove::<Unplaced>();
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
                Made {
                    mesh: materials.add(in_order(base.clone(), &anchors, Pass::Body)),
                    fringe: Some(materials.add(in_order(base.clone(), &anchors, Pass::Fringe))),
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
                MeshMaterial3d(fringe),
                MeshTag::new(*slot),
                ChildOf(entity),
                Fringe,
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

/// A model's root as [`place`] reads it: where it stands, its slot, its layer, the hero's body and
/// a script's object.
type Root = (
    &'static Transform,
    &'static Ordered,
    Option<&'static Layered>,
    Option<&'static Body>,
    Option<&'static Object>,
);

/// Keeps each model's anchor at the cell its root stands in, and the floor it stands on, and the
/// shader's buffers with them. A script's object takes its own cell's place, its logical
/// position's, wherever its model stands: the client chains the object into the list of the cell
/// at its position on its layer (`0x5FCCB0`), which the row walk draws after the cell's statics
/// (`0x62AE90`), while it draws the model at the floats `+0xCC…+0xD4` (docs/re/drawing.md, "The
/// models"); so the leaf of an upper storey's door, whose model stands in the cell before its
/// own, is drawn over the back wall's sprites of its cell (#459).
pub fn place(
    heights: Res<Heights>,
    roots: Query<Root>,
    floors: Query<&statics::Floor>,
    mut anchors: ResMut<Anchors>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
) {
    let floors: HashMap<(buildings::Anchor, u8), &statics::Floor> =
        floors.iter().map(|floor| (floor.part, floor)).collect();
    for (transform, ordered, layered, body, object) in &roots {
        let Some(slot) = ordered.0.and_then(|slot| usize::try_from(slot).ok()) else {
            continue;
        };
        let root = object.map_or(transform.translation, |object| object.at);
        let at = anchor(&heights, root);
        let on = storey(&heights, layered, body)
            .and_then(|part| floors.get(&part))
            .map_or(Vec4::ZERO, |floor| floor.under(place_of(root)));
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
        ] {
            let data: Vec<[f32; 4]> = values.iter().map(|a| a.to_array()).collect();
            if let Some(mut buffer) = buffers.get_mut(handle) {
                *buffer = ShaderBuffer::new(data, RenderAssetUsages::default());
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
        let fringe = in_order(material, &anchors(), Pass::Fringe);
        assert_eq!(fringe.base.alpha_mode, AlphaMode::Blend);
        assert_eq!(fringe.base.depth_bias, Layer::Objects.bias());
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
}
