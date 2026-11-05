//! The fire skin's skin of lava (#2042; docs/re/effects.md, "The fire skin"): the fire skin's
//! system asks at its start for its bearer's overlay state 10 with the texture `A_LAVA00`
//! (`0x78BE10`, the action 6's event `0x109`) for the action's seconds, and while it runs the
//! bearer's model is drawn a second time with that texture by the model's own coordinates, unlit,
//! added to the frame (`ONE`, `ONE`) and its depth not written, in a grey that pulses by the
//! seconds left (`0x44AD7B`, `0x40D310`; `opensacred_rules::spells::lava_skin_grey`). The items it
//! wears, merged onto its model (`GrannyWearSequence`), take the lava with it; those it holds in a
//! hand or carries on its back, objects of their own, keep their looks.

use bevy::camera::visibility::{DynamicSkinnedMeshBounds, NoFrustumCulling};
use bevy::mesh::MeshTag;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use opensacred_rules::spells::{LAVA_SKIN_TEXTURE, lava_skin_grey};

use crate::blended::Layer;
use crate::frame::{Playing, Unpaused};
use crate::game::Data;
use crate::models::{Anchors, ModelMaterial, Pass, in_order};

/// A fire skin's ask for its bearer's skin of lava, of its seconds (`spells`).
#[derive(Component, Clone, Copy)]
pub(crate) struct Asked(pub f32);

/// A bearer's skin of lava: its seconds left, its material once made, and the copies of its
/// model's meshes that draw it.
#[derive(Component)]
struct Skin {
    left: f32,
    material: Option<Handle<ModelMaterial>>,
    copies: Vec<Entity>,
}

/// A mesh of a model, its material converted: its mesh, its joints, its tag and its bounds.
type ModelMesh = (
    &'static Mesh3d,
    Option<&'static SkinnedMesh>,
    &'static MeshTag,
    Has<DynamicSkinnedMeshBounds>,
    Has<NoFrustumCulling>,
);

/// Gives each asking bearer its skin, or its skin its new seconds, as the event `0x109` sets the
/// state and its timer anew.
fn ask(mut commands: Commands, mut asked: Query<(Entity, &Asked, Option<&mut Skin>)>) {
    for (bearer, &Asked(seconds), skin) in &mut asked {
        commands.entity(bearer).remove::<Asked>();
        match skin {
            Some(mut skin) => skin.left = seconds,
            None => {
                commands.entity(bearer).insert(Skin {
                    left: seconds,
                    material: None,
                    copies: Vec::new(),
                });
            }
        }
    }
}

/// Draws each bearer's model again with the lava once its meshes are converted (`models`): a copy
/// of each of its own meshes and of the items it wears, not of those it holds or carries on its
/// back, the same mesh skinned to the same joints, tagged with the same slot, added in the objects'
/// layer of the blended things.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries name their components"
)]
fn dress(
    mut commands: Commands,
    mut skins: Query<(Entity, &mut Skin)>,
    (children, meshes, held): (
        Query<&Children>,
        Query<ModelMesh, With<MeshMaterial3d<ModelMaterial>>>,
        Query<(), Or<(With<crate::npcs::Wielded>, With<crate::npcs::Shouldered>)>>,
    ),
    (data, server, anchors): (Option<Res<Data>>, Res<AssetServer>, Res<Anchors>),
    mut materials: ResMut<Assets<ModelMaterial>>,
) {
    for (bearer, mut skin) in &mut skins {
        if !skin.copies.is_empty() {
            continue;
        }
        let mut own = Vec::new();
        let mut stack = vec![bearer];
        while let Some(entity) = stack.pop() {
            if entity != bearer && held.contains(entity) {
                continue;
            }
            if meshes.contains(entity) {
                own.push(entity);
            }
            stack.extend(children.get(entity).into_iter().flatten());
        }
        if own.is_empty() {
            continue;
        }
        let left = skin.left;
        let material = skin
            .material
            .get_or_insert_with(|| {
                let texture = data
                    .as_ref()
                    .and_then(|data| crate::fx::texture_path(&data.0.assets, LAVA_SKIN_TEXTURE));
                if texture.is_none() {
                    warn!("lava skins: no texture {LAVA_SKIN_TEXTURE}; the skin is not drawn");
                }
                let lava = StandardMaterial {
                    base_color: grey(left),
                    base_color_texture: texture.map(|path| server.load(path)),
                    unlit: true,
                    ..default()
                };
                materials.add(in_order(lava, &anchors, Pass::Added(Layer::Objects)))
            })
            .clone();
        for entity in own {
            let Ok((mesh, skinned, tag, dynamic, unculled)) = meshes.get(entity) else {
                continue;
            };
            let mut copy = commands.spawn((
                Mesh3d(mesh.0.clone()),
                MeshMaterial3d(material.clone()),
                tag.clone(),
                ChildOf(entity),
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
            skin.copies.push(copy.id());
        }
        debug!(
            "lava skins: {bearer} wears lava over {} meshes for {:.1} s",
            skin.copies.len(),
            skin.left
        );
    }
}

/// The lava's grey at `left` seconds still to run, as the frame's raw values.
fn grey(left: f32) -> Color {
    let grey = lava_skin_grey(left);
    Color::linear_rgb(grey, grey, grey)
}

/// Runs each skin on: its seconds count down and its grey pulses by them (`0x44A520`, the state
/// 10); its seconds over, its copies go and its bearer is as before.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut skins: Query<(Entity, &mut Skin)>,
    mut materials: ResMut<Assets<ModelMaterial>>,
) {
    let dt = time.delta_secs();
    for (bearer, mut skin) in &mut skins {
        skin.left -= dt;
        if skin.left < 0.0 {
            for &copy in &skin.copies {
                commands.entity(copy).try_despawn();
            }
            commands.entity(bearer).remove::<Skin>();
            continue;
        }
        if let Some(mut material) = skin
            .material
            .as_ref()
            .and_then(|material| materials.get_mut(material))
        {
            material.base.base_color = grey(skin.left);
        }
    }
}

/// The fire skin's skin of lava.
pub struct LavaSkinsPlugin;

impl Plugin for LavaSkinsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (ask, dress, run).chain().in_set(Playing).in_set(Unpaused),
        );
    }
}
