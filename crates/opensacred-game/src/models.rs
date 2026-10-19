//! The models among the sprites (docs/re/drawing.md, "The models"): the client draws a creature
//! or an object of the world at its cell's place in the draw lists, after the cell's statics, and
//! an ordinary sprite without the depth test, so a sprite drawn later covers a model whole and one
//! drawn earlier lies under it whole. The game draws the sprites at the depths of their places
//! (`order.wesl`); a model's meshes take [`ModelMaterial`], whose vertex shader puts the model at
//! its anchor's place, the centre of the cell its root stands in, with a small part of its own
//! depth (`models.wesl`). A model's root carries [`Ordered`], with its slot of the [`Anchors`]
//! that its meshes' tags name; a model that another holds, an NPC's weapon, [`OrderedWith`] its
//! holder, whose slot its meshes take.

use std::collections::HashMap;

use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::mesh::MeshTag;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;
use bevy::world_serialization::WorldInstanceReady;

use crate::frame::Step;
use crate::iso;
use crate::terrain::Heights;

/// The shader of [`ModelMaterial`], embedded in the binary.
const SHADER: &str = "embedded://opensacred_game/models.wesl";

/// A model's place in its cell (`order.wesl`): after the statics of the cell's chain, which take
/// the places from 0, short of the next cell of the diagonal, at 33.8. The model's own depth
/// spreads it half a unit either way (`models.wesl`), over the places of a few cells of its
/// diagonal, and far from the next diagonal's.
const PLACE: f32 = 24.0;

/// A glTF material of a model, drawn at the model's place in the order.
pub type ModelMaterial = ExtendedMaterial<StandardMaterial, InOrder>;

/// The models' anchors, which every [`ModelMaterial`] shares.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct InOrder {
    #[storage(100, read_only)]
    anchors: Handle<ShaderBuffer>,
}

impl MaterialExtension for InOrder {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }
}

/// Every model's anchor, by the slot its root holds, for the shader by its meshes' tags: the
/// centre of the model's cell at the cell's height, and the place in the cell.
#[derive(Resource)]
pub struct Anchors {
    buffer: Handle<ShaderBuffer>,
    anchors: Vec<Vec4>,
    free: Vec<u32>,
    changed: bool,
}

impl FromWorld for Anchors {
    fn from_world(world: &mut World) -> Self {
        let buffer = world
            .resource_mut::<Assets<ShaderBuffer>>()
            .add(ShaderBuffer::new(
                vec![[0.0_f32; 4]],
                RenderAssetUsages::default(),
            ));
        Self {
            buffer,
            anchors: vec![Vec4::ZERO],
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
/// height (the mean of its corners', as the sprites' anchors), after the cell's statics.
fn anchor(heights: &Heights, position: Vec3) -> Vec4 {
    let centre = iso::ground(
        (position.x / iso::CELL).floor() + 0.5,
        (position.z / iso::CELL).floor() + 0.5,
    );
    let height = heights.at(centre.x, centre.z).unwrap_or(position.y);
    Vec4::new(centre.x, height, centre.z, PLACE)
}

/// Gives a new model root a slot of the anchors.
fn hold(added: On<Add<Ordered>>, mut anchors: ResMut<Anchors>, mut roots: Query<&mut Ordered>) {
    let Ok(mut ordered) = roots.get_mut(added.entity) else {
        return;
    };
    let slot = anchors.free.pop().or_else(|| {
        let slot = u32::try_from(anchors.anchors.len()).ok()?;
        anchors.anchors.push(Vec4::ZERO);
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

/// Keeps each model's anchor at the cell its root stands in, and the shader's buffer with them.
pub fn place(
    heights: Res<Heights>,
    roots: Query<(&Transform, &Ordered)>,
    mut anchors: ResMut<Anchors>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
) {
    for (transform, ordered) in &roots {
        let Some(slot) = ordered.0.and_then(|slot| usize::try_from(slot).ok()) else {
            continue;
        };
        let at = anchor(&heights, transform.translation);
        if let Some(anchor) = anchors.anchors.get_mut(slot)
            && *anchor != at
        {
            *anchor = at;
            anchors.changed = true;
        }
    }
    if anchors.changed {
        anchors.changed = false;
        let data: Vec<[f32; 4]> = anchors.anchors.iter().map(|a| a.to_array()).collect();
        if let Some(mut buffer) = buffers.get_mut(&anchors.buffer) {
            *buffer = ShaderBuffer::new(data, RenderAssetUsages::default());
        }
    }
}

#[cfg(test)]
mod tests {
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
