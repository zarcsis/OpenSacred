//! The skins over a creature's model, its overlay state (the event `0x109`, the creature's
//! `+0x60`, one at a time, a new one in place of the one before): the fire skin's skin of lava
//! (#2042; docs/re/effects.md, "The fire skin"), the state 10 with the texture `A_LAVA00` that the
//! fire skin's system asks at its start (`0x78BE10`, the action 6) for the action's seconds, its
//! bearer's model drawn a second time with that texture by the model's own coordinates, unlit,
//! added to the frame (`ONE`, `ONE`) and its depth not written, in a grey that pulses by the
//! seconds left (`0x44AD7B`, `0x40D310`; `opensacred_rules::spells::lava_skin_grey`), and the
//! quickening's, the same in `FX_SPEEDUP`; the crusader's strength's crosses (#1092; "The light
//! strikes"), the state 6, its model drawn again so in `FX_CROSS` by its coordinates times 3, in a
//! grey that pulses by the real clock (`0x44AD29`, `0x40CD30`); the stone skin's glow, the state
//! 7, its model drawn again so in `FX_MULTITEXGLOW01` by its coordinates scrolled by the real
//! clock, white (`0x44ACAA`, `0x40CA20`), then the state 8 for 2 s, the glow and the plain model
//! in turns every 0.2 s; and the flesh to stone's skin of stone (#1092; "The flesh to stone"), the
//! state 9 with the texture `FREEZE_STONE01` that the stone's action 7 asks for its hold's
//! seconds, its model drawn again with that texture, lit, laid over by its alpha (`0x44AD50`,
//! `0x40D060`), which, the texture opaque, hides the model under it: here the model's own
//! materials take the stone's texture while it lasts, and give it back. The items it wears,
//! merged onto its model (`GrannyWearSequence`), take the skin with it; those it holds in a hand or
//! carries on its back, objects of their own, keep their looks.

use std::collections::HashMap;

use bevy::camera::visibility::{DynamicSkinnedMeshBounds, NoFrustumCulling};
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::math::Affine2;
use bevy::mesh::MeshTag;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use opensacred_rules::spells::{
    CROSS_SKIN_SCALE, CROSS_SKIN_TEXTURE, GLOW_SKIN_SCROLL, GLOW_SKIN_TEXTURE, LAVA_SKIN_TEXTURE,
    SKIN_BLINK, SPEED_SKIN_TEXTURE, STONED_TEXTURE, cross_skin_grey, lava_skin_grey, skin_shows,
};

use crate::blended::Layer;
use crate::frame::{Playing, Unpaused};
use crate::game::Data;
use crate::models::{Anchors, ModelMaterial, Pass, in_order};

/// A skin over a creature's model: the fire skin's lava and the quickening's speed (the state
/// 10), the crusader's strength's crosses (6), the stone skin's glow (7, then 8) or the stone's
/// (9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Overlay {
    Lava,
    Speed,
    Cross,
    Glow,
    Stone,
}

impl Overlay {
    /// The texture its model is drawn in.
    fn texture(self) -> &'static str {
        match self {
            Self::Lava => LAVA_SKIN_TEXTURE,
            Self::Speed => SPEED_SKIN_TEXTURE,
            Self::Cross => CROSS_SKIN_TEXTURE,
            Self::Glow => GLOW_SKIN_TEXTURE,
            Self::Stone => STONED_TEXTURE,
        }
    }
}

/// An ask for a bearer's skin, of its seconds (`spells`, `stones`).
#[derive(Component, Clone, Copy)]
pub(crate) struct Asked(pub Overlay, pub f32);

/// A bearer's skin: its overlay, its seconds left, whether it blinks out (the stone skin's state
/// 8), the copies' material once made, whether its model is dressed in it, the copies of its
/// model's meshes that draw it, or its meshes that wear the stone, each with the material it wore
/// before.
#[derive(Component)]
struct Skin {
    overlay: Overlay,
    left: f32,
    blinking: bool,
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
            blinking: false,
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
            Some(mut skin) if skin.overlay == overlay => {
                skin.left = seconds;
                skin.blinking = false;
            }
            skin => {
                if let Some(mut skin) = skin {
                    skin.undress(&mut commands);
                }
                commands.entity(bearer).insert(Skin::new(overlay, seconds));
            }
        }
    }
}

/// The texture of `overlay`, loaded, where the assets have it: the stone's as a model's own; an
/// added skin's repeated along both its axes, as the crosses' and the glow's coordinates run past
/// its edges, and without its smaller levels, as the original's textures have one, so that its
/// streaks and crosses keep their contrast on a small model rather than averaging out (#2169).
fn texture(data: Option<&Data>, server: &AssetServer, overlay: Overlay) -> Option<Handle<Image>> {
    let name = overlay.texture();
    let path = data.and_then(|data| crate::fx::texture_path(&data.0.assets, name));
    let Some(path) = path else {
        warn!("skins: no texture {name}; the skin is not drawn");
        return None;
    };
    if overlay == Overlay::Stone {
        return Some(server.load(path));
    }
    Some(
        server
            .load_builder()
            .with_settings(|settings: &mut ImageLoaderSettings| {
                settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                    lod_max_clamp: 0.0,
                    address_mode_u: ImageAddressMode::Repeat,
                    address_mode_v: ImageAddressMode::Repeat,
                    ..ImageSamplerDescriptor::linear()
                });
            })
            .load::<Image>(path),
    )
}

/// Dresses each bearer's model in its skin once its meshes are converted (`models`): its own
/// meshes and the items it wears, not those it holds or carries on its back. The lava, the speed,
/// the crosses and the glow draw a copy of each, the same mesh skinned to the same joints, tagged
/// with the same slot, added in the objects' layer of the blended things; the stone gives each its
/// material with the stone's texture, lit, a material of the stone for each material the model
/// wears.
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
        let overlay = skin.overlay;
        if overlay == Overlay::Stone {
            // Without its texture the skin lasts unseen.
            let Some(stone) = texture(data.as_deref(), &server, Overlay::Stone) else {
                continue;
            };
            let mut made: HashMap<AssetId<ModelMaterial>, Handle<ModelMaterial>> = HashMap::new();
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
        } else {
            let left = skin.left;
            let material = skin
                .material
                .get_or_insert_with(|| {
                    let added = StandardMaterial {
                        base_color: grey(overlay, left, 0.0),
                        base_color_texture: texture(data.as_deref(), &server, overlay),
                        uv_transform: coordinates(overlay, 0.0),
                        unlit: true,
                        ..default()
                    };
                    materials.add(in_order(added, &anchors, Pass::Added(Layer::Objects)))
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
        debug!(
            "skins: {bearer} wears {:?} over {} meshes for {:.1} s",
            skin.overlay,
            skin.copies.len() + skin.swapped.len(),
            skin.left
        );
    }
}

/// An added skin's grey at `left` seconds still to run and `real` seconds of the real clock: the
/// lava's and the speed's by the seconds left, the crosses' by the real clock, the glow white. A
/// grey of the frame's values in linear light, as the model's material takes it: its texels are
/// decoded, multiplied by it and encoded again (`standard.wesl`), so that the frame takes the
/// texels' values times the grey, as the original's device adds them (`fx::linear`; #2169).
fn grey(overlay: Overlay, left: f32, real: f64) -> Color {
    let grey = match overlay {
        Overlay::Lava | Overlay::Speed => lava_skin_grey(left),
        Overlay::Cross => cross_skin_grey(real * 1000.0),
        Overlay::Glow | Overlay::Stone => 1.0,
    };
    Color::srgb(grey, grey, grey)
}

/// An added skin's texture coordinates at `real` seconds of the real clock: the crosses' times 3,
/// the glow's scrolled along both by 0.05 a second.
fn coordinates(overlay: Overlay, real: f64) -> Affine2 {
    match overlay {
        Overlay::Cross => Affine2::from_scale(Vec2::splat(CROSS_SKIN_SCALE)),
        Overlay::Glow => {
            let scroll = (real * f64::from(GLOW_SKIN_SCROLL)).fract();
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a share of the texture below 1"
            )]
            let scroll = scroll as f32;
            Affine2::from_translation(Vec2::splat(scroll))
        }
        Overlay::Lava | Overlay::Speed | Overlay::Stone => Affine2::IDENTITY,
    }
}

/// Runs each skin on: its seconds count down (`0x44A520`), the lava's and the speed's grey pulses
/// by them, the crosses' by the real clock, and the glow scrolls; its seconds over, the glow blinks
/// for 2 s more (the state 8), then the skin is taken off and its bearer is as before.
fn run(
    mut commands: Commands,
    (time, real): (Res<Time>, Res<Time<Real>>),
    mut skins: Query<(Entity, &mut Skin)>,
    mut visibilities: Query<&mut Visibility>,
    mut materials: ResMut<Assets<ModelMaterial>>,
) {
    let dt = time.delta_secs();
    let now = real.elapsed_secs_f64();
    for (bearer, mut skin) in &mut skins {
        skin.left -= dt;
        if skin.left < 0.0 && skin.overlay == Overlay::Glow && !skin.blinking {
            skin.blinking = true;
            skin.left += SKIN_BLINK;
        }
        if skin.left < 0.0 {
            skin.undress(&mut commands);
            commands.entity(bearer).remove::<Skin>();
            continue;
        }
        // Its bearer's model made anew (a quickening's pace, an item worn), its copies went with
        // its meshes: it is dressed again on the new ones.
        if skin.copies.iter().any(|&copy| !visibilities.contains(copy)) {
            skin.undress(&mut commands);
            skin.dressed = false;
            continue;
        }
        if skin.blinking {
            let shown = if skin_shows(skin.left) {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            for &copy in &skin.copies {
                if let Ok(mut visibility) = visibilities.get_mut(copy) {
                    visibility.set_if_neq(shown);
                }
            }
        }
        if let Some(mut material) = skin
            .material
            .as_ref()
            .and_then(|material| materials.get_mut(material))
        {
            material.base.base_color = grey(skin.overlay, skin.left, now);
            material.base.uv_transform = coordinates(skin.overlay, now);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The grey is the original's value in the frame (#2169): an sRGB colour, which the material
    /// takes in linear light and its encoding gives back, so that the texels' values are
    /// multiplied by it, not linear light, which the encoding would raise.
    #[test]
    fn grey_is_the_frames_value() {
        for left in [0.0, 0.3, 0.7, 1.2] {
            let grey = grey(Overlay::Speed, left, 0.0);
            assert!((grey.to_srgba().red - lava_skin_grey(left)).abs() < 1e-6);
            assert!(grey.to_linear().red < lava_skin_grey(left));
        }
        let crosses = grey(Overlay::Cross, 0.0, 0.25);
        assert!((crosses.to_srgba().red - cross_skin_grey(250.0)).abs() < 1e-6);
        assert_eq!(grey(Overlay::Glow, 0.0, 0.0).to_srgba().red, 1.0);
    }
}
