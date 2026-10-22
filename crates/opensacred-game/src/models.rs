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

use std::collections::HashMap;

use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::mesh::MeshTag;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{
    ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
    MaterialPlugin,
};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;
use bevy::world_serialization::WorldInstanceReady;

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

/// A glTF material of a model, drawn at the model's place in the order.
pub type ModelMaterial = ExtendedMaterial<StandardMaterial, InOrder>;

/// The models' anchors and floors, which every [`ModelMaterial`] shares.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct InOrder {
    #[storage(100, read_only)]
    anchors: Handle<ShaderBuffer>,
    #[storage(101, read_only)]
    floors: Handle<ShaderBuffer>,
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
    /// the blended things (the shadows').
    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        blended::unbias(descriptor);
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

/// The model material of `base`: a mesh of it, tagged with a model's slot of the `anchors`, is
/// drawn at that model's place in the order.
pub fn in_order(base: StandardMaterial, anchors: &Anchors) -> ModelMaterial {
    ModelMaterial {
        base,
        extension: InOrder {
            anchors: anchors.buffer.clone(),
            floors: anchors.floor_buffer.clone(),
        },
    }
}

/// The root of a model drawn at its holder's place: an NPC's weapon.
#[derive(Component)]
pub struct OrderedWith(pub Entity);

/// A model whose scene is ready and whose meshes have their glTF materials still.
#[derive(Component)]
pub struct Unplaced;

/// The model materials made of the glTF ones, by the glTF material.
#[derive(Resource, Default)]
pub struct Converted(HashMap<AssetId<StandardMaterial>, Handle<ModelMaterial>>);

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

/// Gives the meshes of each model that has just spawned the model materials of their glTF
/// materials (after `npcs::retexture` has given them their item's texture) and their root's
/// slot of the anchors for a tag.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its queries and resources as arguments"
)]
pub fn convert(
    mut commands: Commands,
    models: Query<(Entity, Option<&OrderedWith>), With<Unplaced>>,
    roots: Query<&Ordered>,
    children: Query<&Children>,
    meshes: Query<&MeshMaterial3d<StandardMaterial>>,
    standard: Res<Assets<StandardMaterial>>,
    mut materials: ResMut<Assets<ModelMaterial>>,
    mut converted: ResMut<Converted>,
    anchors: Res<Anchors>,
) {
    for (model, with) in &models {
        commands.entity(model).remove::<Unplaced>();
        let root = with.map_or(model, |with| with.0);
        let Ok(Ordered(Some(slot))) = roots.get(root) else {
            continue;
        };
        for entity in children.iter_descendants(model) {
            let Ok(material) = meshes.get(entity) else {
                continue;
            };
            let Some(base) = standard.get(&material.0) else {
                continue;
            };
            let handle = converted
                .0
                .entry(material.0.id())
                .or_insert_with(|| materials.add(in_order(base.clone(), &anchors)))
                .clone();
            commands
                .entity(entity)
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert((MeshMaterial3d(handle), MeshTag::new(*slot)));
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
