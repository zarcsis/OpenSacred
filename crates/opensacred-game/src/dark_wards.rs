//! The look of the Dark Elf's two wards (#1092; docs/re/effects.md, "The Dark Elf's wards"): the
//! arrowproof (1045) shows `cParticleSystem_fernumleitung` (883) about its bearer, the model
//! `magicshield2` in its own texture of magenta and cyan runes, and the spellproof (1046)
//! `cParticleSystem_magieumleitung` (870), the model `magicshield` of cyan rings and magenta runes.
//! Each rises out of the ground at its bearer's root over π s, stands for its effect's seconds
//! turning clockwise on the screen at 0.5 rad a second, its clip looping, pulsing grey once a
//! second, the arrowproof's breathing a little wider and narrower, and sinks back over π s; added
//! whole and unlit, untinted by the night, lit by a light of 300, and laid under its bearer's
//! model, as the original draws its system before the creature (`models::Pass::Under`, #2178).
//! Each arrow the arrowproof sends back widens it by a tenth for a moment
//! (`opensacred_rules::particles::DarkWard`). Not followed: the spells the spellproof turns aside,
//! which nothing in the game turns aside yet.

use bevy::gltf::Gltf;
use bevy::prelude::*;
use opensacred_rules::particles::{DARK_WARD_LIGHT, DARK_WARD_MODELS, DarkWard};

use crate::blended::Layer;
use crate::frame::{Playing, Unpaused};
use crate::game::{AppState, Data};
use crate::models::{self as model_looks, Anchors, ModelMaterial, Pass, in_order};
use crate::{daynight, iso};

/// The arrowproof, whose turned aside arrows flash it (`0x5614E1`).
const ARROWPROOF: u32 = 1045;

/// A ward to show about its bearer, made with its effect (`making`): its bearer, whether it is
/// the arrowproof's, and its effect's seconds.
#[derive(Component, Clone, Copy)]
pub(crate) struct Ward {
    pub bearer: Entity,
    pub arrows: bool,
    pub seconds: f32,
}

/// The wards' models, relative to the asset directory, found at the first ward: the
/// arrowproof's and the spellproof's.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    models: [Option<String>; 2],
}

/// A ward that shows: its bearer, whether it is the arrowproof's, its look, its model, its
/// material once its meshes are dressed in it, and whether its clip plays.
#[derive(Component)]
struct Shown {
    bearer: Entity,
    arrows: bool,
    look: DarkWard,
    model: Entity,
    material: Option<Handle<ModelMaterial>>,
    playing: bool,
}

/// Gives each new ward its model at its bearer's root and its light (`0x7A0790`, `0x7A0D10`); a
/// bearer's new ward takes the place of the one of its kind it had.
fn show(
    mut commands: Commands,
    wards: Query<(Entity, &Ward), Added<Ward>>,
    shown: Query<(Entity, &Shown)>,
    places: Query<&GlobalTransform>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
) {
    for (entity, ward) in &wards {
        let Ok(at) = places.get(ward.bearer) else {
            commands.entity(entity).despawn();
            continue;
        };
        if !looks.made {
            looks.made = true;
            let files = data
                .as_deref()
                .and_then(|data| crate::npcs::model_files(&data.0.assets).ok())
                .unwrap_or_default();
            looks.models = DARK_WARD_MODELS.map(|name| {
                let file = files.get(name).cloned();
                if file.is_none() {
                    warn!("dark wards: no {name}; its ward is not drawn");
                }
                file
            });
        }
        let Some(model) = looks.models[usize::from(!ward.arrows)].clone() else {
            commands.entity(entity).despawn();
            continue;
        };
        for (old, sight) in &shown {
            if sight.bearer == ward.bearer && sight.arrows == ward.arrows {
                commands.entity(sight.model).try_despawn();
                commands.entity(old).despawn();
            }
        }
        let model = commands
            .spawn((
                model_looks::scene(&assets, &model),
                Transform::from_translation(at.translation()).with_scale(Vec3::ZERO),
                model_looks::Ordered::default(),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        commands.entity(entity).remove::<Ward>().insert((
            Shown {
                bearer: ward.bearer,
                arrows: ward.arrows,
                look: DarkWard::new(ward.seconds, ward.arrows),
                model,
                material: None,
                playing: false,
            },
            Transform::from_translation(at.translation()),
            daynight::Light(DARK_WARD_LIGHT),
        ));
    }
}

/// Dresses each ward's model in its own texture added whole and unlit, laid under its bearer
/// (`0x40D310`), once its meshes are there, and plays its clip's loop once its player is there
/// (**hypothesis**: the original's control loops for ever, as the hell disk's).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn dress(
    mut commands: Commands,
    mut shown: Query<&mut Shown>,
    (children, meshes, sources): (
        Query<&Children>,
        Query<(Entity, &MeshMaterial3d<ModelMaterial>)>,
        Query<&model_looks::Source>,
    ),
    (mut players, gltfs, mut graphs): (
        Query<&mut AnimationPlayer, Without<AnimationGraphHandle>>,
        Res<Assets<Gltf>>,
        ResMut<Assets<AnimationGraph>>,
    ),
    (anchors, mut materials): (Res<Anchors>, ResMut<Assets<ModelMaterial>>),
) {
    for mut look in &mut shown {
        let model = look.model;
        if !look.playing {
            let clip = sources
                .get(model)
                .ok()
                .and_then(|source| gltfs.get(&source.0))
                .and_then(|gltf| gltf.animations.first().cloned());
            if let Some(clip) = clip {
                for entity in children.iter_descendants(model) {
                    if let Ok(mut player) = players.get_mut(entity) {
                        let (graph, node) = AnimationGraph::from_clip(clip.clone());
                        player.play(node).repeat();
                        commands
                            .entity(entity)
                            .insert(AnimationGraphHandle(graphs.add(graph)));
                        look.playing = true;
                    }
                }
            }
        }
        if look.material.is_some() {
            continue;
        }
        let own: Vec<_> = children
            .iter_descendants(model)
            .filter_map(|entity| meshes.get(entity).ok())
            .collect();
        let Some(base) = own
            .first()
            .and_then(|(_, material)| materials.get(&material.0))
            .map(|material| material.base.clone())
        else {
            continue;
        };
        let material = materials.add(in_order(
            StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: base.base_color_texture,
                unlit: true,
                cull_mode: base.cull_mode,
                double_sided: base.double_sided,
                ..default()
            },
            &anchors,
            Pass::Under(Layer::Objects),
        ));
        for (entity, _) in own {
            commands
                .entity(entity)
                .insert(MeshMaterial3d(material.clone()));
        }
        look.material = Some(material);
    }
}

/// Runs each ward a frame on (`0x7A0940`, `0x7A0380`) and draws it (`0x7A0A60`, `0x7A0490`): its
/// model at its bearer's root, scaled across by its breadth and upright by its rise, turned
/// about the upright, in its grey; an arrow the arrowproof sends back flashes it; it goes once it
/// has sunk, or with its bearer.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &mut Transform)>,
    places: Query<&GlobalTransform>,
    mut models: Query<&mut Transform, Without<Shown>>,
    (mut flashes, mut materials): (
        MessageReader<crate::shields::Flash>,
        ResMut<Assets<ModelMaterial>>,
    ),
) {
    let dt = time.delta_secs();
    let flashed: Vec<Entity> = flashes
        .read()
        .filter(|flash| flash.1 == ARROWPROOF)
        .map(|flash| flash.0)
        .collect();
    for (entity, mut ward, mut transform) in &mut shown {
        let ward = &mut *ward;
        let Ok(place) = places.get(ward.bearer).map(GlobalTransform::translation) else {
            commands.entity(ward.model).try_despawn();
            commands.entity(entity).despawn();
            continue;
        };
        if ward.arrows && flashed.contains(&ward.bearer) {
            ward.look.flash();
        }
        ward.look.update(dt);
        if ward.look.done() {
            commands.entity(ward.model).try_despawn();
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation = place;
        if let Ok(mut model) = models.get_mut(ward.model) {
            let (across, upright) = (ward.look.breadth(), ward.look.height());
            *model = Transform::from_translation(place)
                .with_rotation(Quat::from_rotation_y(ward.look.turn()))
                .with_scale(Vec3::new(across, upright, across) * iso::MODEL_SCALE);
        }
        if let Some(mut material) = ward
            .material
            .as_ref()
            .and_then(|material| materials.get_mut(material))
        {
            // The grey as the frame's value: its texels times it (`skins`' grey, #2169).
            let grey = ward.look.grey();
            material.base.base_color = Color::srgb(grey, grey, grey);
        }
    }
}

/// The Dark Elf's wards' looks.
pub struct DarkWardsPlugin;

impl Plugin for DarkWardsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>().add_systems(
            Update,
            (show, dress, run)
                .chain()
                .in_set(Playing)
                .in_set(Unpaused)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
