//! The skins over a creature's model, its overlay state (the event `0x109`, the creature's
//! `+0x60`, one at a time, a new one in place of the one before): the fire skin's skin of lava
//! (#2042; docs/re/effects.md, "The fire skin"), the state 10 with the texture `A_LAVA00` that the
//! fire skin's system asks at its start (`0x78BE10`, the action 6) for the action's seconds, its
//! bearer's model drawn a second time with that texture by the model's own coordinates, unlit,
//! added to the frame (`ONE`, `ONE`) and its depth not written, in a grey that pulses by the
//! seconds left (`0x44AD7B`, `0x40D310`; `opensacred_rules::spells::lava_skin_grey`); and the
//! flesh to stone's skin of stone (#1092; "The flesh to stone"), the state 9 with the texture
//! `FREEZE_STONE01` that the stone's action 7 asks for its hold's seconds, its model drawn again
//! with that texture, lit, laid over by its alpha (`0x44AD50`, `0x40D060`), which, the texture
//! opaque, hides the model under it: here the model's own materials take the stone's texture
//! while it lasts, and give it back. The items it wears, merged onto its model
//! (`GrannyWearSequence`), take the skin with it; those it holds in a hand or carries on its back,
//! objects of their own, keep their looks.

use std::collections::HashMap;

use bevy::camera::visibility::{DynamicSkinnedMeshBounds, NoFrustumCulling};
use bevy::mesh::MeshTag;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use opensacred_rules::spells::{LAVA_SKIN_TEXTURE, STONED_TEXTURE, lava_skin_grey};

use crate::blended::Layer;
use crate::frame::{Playing, Unpaused};
use crate::game::Data;
use crate::models::{Anchors, ModelMaterial, Pass, in_order};

/// A skin over a creature's model: the fire skin's lava (the state 10) or the stone's (9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Overlay {
    Lava,
    Stone,
}

/// An ask for a bearer's skin, of its seconds (`spells`, `stones`).
#[derive(Component, Clone, Copy)]
pub(crate) struct Asked(pub Overlay, pub f32);

/// A bearer's skin: its overlay, its seconds left, the lava's material once made, whether its
/// model is dressed in it, the copies of its model's meshes that draw the lava, or its meshes that
/// wear the stone, each with the material it wore before.
#[derive(Component)]
struct Skin {
    overlay: Overlay,
    left: f32,
    material: Option<Handle<ModelMaterial>>,
    dressed: bool,
    copies: Vec<Entity>,
    swapped: Vec<(Entity, Handle<ModelMaterial>)>,
}

impl Skin {
    fn new(overlay: Overlay, left: f32) -> Self {
        Self {
            overlay,
            left,
            material: None,
            dressed: false,
            copies: Vec::new(),
            swapped: Vec::new(),
        }
    }

    /// Takes the skin off its bearer's meshes: its copies go, and its meshes wear their own
    /// materials again.
    fn undress(&mut self, commands: &mut Commands) {
        for copy in self.copies.drain(..) {
            commands.entity(copy).try_despawn();
        }
        for (mesh, own) in self.swapped.drain(..) {
            if let Ok(mut mesh) = commands.get_entity(mesh) {
                mesh.insert(MeshMaterial3d(own));
            }
        }
    }
}

/// A mesh of a model, its material converted: its mesh, its material, its joints, its tag and
/// its bounds.
type ModelMesh = (
    &'static Mesh3d,
    &'static MeshMaterial3d<ModelMaterial>,
    Option<&'static SkinnedMesh>,
    &'static MeshTag,
    Has<DynamicSkinnedMeshBounds>,
    Has<NoFrustumCulling>,
);

/// Gives each asking bearer its skin, or its skin its new seconds, as the event `0x109` sets the
/// state and its timer anew: a skin of another overlay in place of the one it wore.
fn ask(mut commands: Commands, mut asked: Query<(Entity, &Asked, Option<&mut Skin>)>) {
    for (bearer, &Asked(overlay, seconds), skin) in &mut asked {
        commands.entity(bearer).remove::<Asked>();
        match skin {
            Some(mut skin) if skin.overlay == overlay => skin.left = seconds,
            skin => {
                if let Some(mut skin) = skin {
                    skin.undress(&mut commands);
                }
                commands.entity(bearer).insert(Skin::new(overlay, seconds));
            }
        }
    }
}

/// The texture of `name`, loaded, where the assets have it.
fn texture(data: Option<&Data>, server: &AssetServer, name: &str) -> Option<Handle<Image>> {
    let path = data.and_then(|data| crate::fx::texture_path(&data.0.assets, name));
    if path.is_none() {
        warn!("skins: no texture {name}; the skin is not drawn");
    }
    path.map(|path| server.load(path))
}

/// Dresses each bearer's model in its skin once its meshes are converted (`models`): its own
/// meshes and the items it wears, not those it holds or carries on its back. The lava draws a copy
/// of each, the same mesh skinned to the same joints, tagged with the same slot, added in the
/// objects' layer of the blended things; the stone gives each its material with the stone's
/// texture, lit, a material of the stone for each material the model wears.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries name their components"
)]
fn dress(
    mut commands: Commands,
    mut skins: Query<(Entity, &mut Skin)>,
    (children, meshes, held): (
        Query<&Children>,
        Query<ModelMesh>,
        Query<(), Or<(With<crate::npcs::Wielded>, With<crate::npcs::Shouldered>)>>,
    ),
    (data, server, anchors): (Option<Res<Data>>, Res<AssetServer>, Res<Anchors>),
    mut materials: ResMut<Assets<ModelMaterial>>,
) {
    for (bearer, mut skin) in &mut skins {
        if skin.dressed {
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
        skin.dressed = true;
        match skin.overlay {
            Overlay::Lava => {
                let left = skin.left;
                let material = skin
                    .material
                    .get_or_insert_with(|| {
                        let lava = StandardMaterial {
                            base_color: grey(left),
                            base_color_texture: texture(
                                data.as_deref(),
                                &server,
                                LAVA_SKIN_TEXTURE,
                            ),
                            unlit: true,
                            ..default()
                        };
                        materials.add(in_order(lava, &anchors, Pass::Added(Layer::Objects)))
                    })
                    .clone();
                for entity in own {
                    let Ok((mesh, _, skinned, tag, dynamic, unculled)) = meshes.get(entity) else {
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
            }
            Overlay::Stone => {
                // Without its texture the skin lasts unseen.
                let Some(stone) = texture(data.as_deref(), &server, STONED_TEXTURE) else {
                    continue;
                };
                let mut made: HashMap<AssetId<ModelMaterial>, Handle<ModelMaterial>> =
                    HashMap::new();
                for entity in own {
                    let Ok((_, worn, ..)) = meshes.get(entity) else {
                        continue;
                    };
                    let Some(base) = materials.get(&worn.0).cloned() else {
                        continue;
                    };
                    let material = made
                        .entry(worn.0.id())
                        .or_insert_with(|| {
                            let mut base = base;
                            base.base.base_color = Color::WHITE;
                            base.base.base_color_texture = Some(stone.clone());
                            base.base.unlit = false;
                            materials.add(base)
                        })
                        .clone();
                    skin.swapped.push((entity, worn.0.clone()));
                    commands.entity(entity).insert(MeshMaterial3d(material));
                }
            }
        }
        debug!(
            "skins: {bearer} wears {:?} over {} meshes for {:.1} s",
            skin.overlay,
            skin.copies.len() + skin.swapped.len(),
            skin.left
        );
    }
}

/// The lava's grey at `left` seconds still to run, as the frame's raw values.
fn grey(left: f32) -> Color {
    let grey = lava_skin_grey(left);
    Color::linear_rgb(grey, grey, grey)
}

/// Runs each skin on: its seconds count down (`0x44A520`) and the lava's grey pulses by them; its
/// seconds over, it is taken off and its bearer is as before.
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
            skin.undress(&mut commands);
            commands.entity(bearer).remove::<Skin>();
            continue;
        }
        if skin.overlay == Overlay::Lava
            && let Some(mut material) = skin
                .material
                .as_ref()
                .and_then(|material| materials.get_mut(material))
        {
            material.base.base_color = grey(skin.left);
        }
    }
}

/// The skins over the creatures' models.
pub struct SkinsPlugin;

impl Plugin for SkinsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (ask, dress, run).chain().in_set(Playing).in_set(Unpaused),
        );
    }
}
