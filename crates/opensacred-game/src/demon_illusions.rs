//! The big demon's illusion (917, `FX_BIG_DEMON_ILLUSION`; #3059; docs/re/effects.md, "The big
//! demon's illusion"): the FX object that side quest 402's `take402` makes once (`npcs`), never
//! made again. A ghost of the big demon's model stands at its place facing the screen's lower
//! left, drawn added whole and unlit in `BIG_DEMON_TEX_FX`, its depth tested and not written and
//! untinted by the night, and plays its clip of the slot 154 once at half its speed, the game's
//! speed aside, its grey growing in over 1.94 s and fading over its last 1.94 s
//! (`opensacred_rules::particles::illusion_grey`) through twice its clip's length; it lights the
//! night within 200 of its place on the ground the while, and then goes with its object.

use bevy::camera::visibility::RenderLayers;
use bevy::gltf::Gltf;
use bevy::prelude::*;
use opensacred_rules::particles::{
    ILLUSION_FRONT, ILLUSION_LIGHT, ILLUSION_MODEL, ILLUSION_RATE, ILLUSION_SLOT, ILLUSION_TEXTURE,
    illusion_grey, illusion_life,
};

use crate::blended::Layer;
use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::game::{AppState, Data};
use crate::models::{self as model_looks, Anchors, ModelMaterial, Pass, in_order};
use crate::{daynight, iso};

/// An illusion that a script made, at its record's point on the plane of height 0.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Illusion;

/// What the illusions take, found at the first: the model, relative to the asset directory, the
/// name of its clip of the slot 154, and the texture.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    model: Option<String>,
    clip: Option<String>,
    texture: Option<Handle<Image>>,
}

/// An illusion that shows: its model and its lamp, its material, whether its meshes wear it, its
/// clip once it plays and its life then, and its clock.
#[derive(Component)]
struct Shown {
    model: Option<Entity>,
    lamp: Entity,
    material: Handle<ModelMaterial>,
    dressed: bool,
    life: Option<f32>,
    clock: f32,
}

/// An illusion's model or lamp, which goes with its object.
#[derive(Component)]
struct Of(Entity);

/// Finds the model, its clip and the texture at the first illusion (`0x7A8550`).
fn find_looks(looks: &mut Looks, data: &Data, server: &AssetServer) {
    looks.made = true;
    let assets = &data.0.assets;
    looks.model = crate::npcs::model_files(assets)
        .ok()
        .and_then(|files| files.get(ILLUSION_MODEL).cloned());
    let Some(model) = &looks.model else {
        warn!("demon illusions: no model {ILLUSION_MODEL}; the illusion is not drawn");
        return;
    };
    looks.clip = crate::motions::Slots::read(&assets.join(model))
        .ok()
        .and_then(|slots| slots.slot(ILLUSION_SLOT).map(str::to_owned));
    if looks.clip.is_none() {
        warn!("demon illusions: {ILLUSION_MODEL} has no clip of the slot {ILLUSION_SLOT}");
    }
    // Sampled at its first level alone, as `0x40D310` samples it: its levels below would spread
    // its bright veins.
    looks.texture = crate::fx::wrapped_texture(assets, server, ILLUSION_TEXTURE);
    if looks.texture.is_none() {
        warn!("demon illusions: no texture {ILLUSION_TEXTURE}");
    }
}

/// Gives each new illusion its model, turned to the screen's lower left at its place, its
/// material and its lamp of 200 on the ground (`0x7A8A40`, `0x7A87B0`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn show(
    mut commands: Commands,
    illusions: Query<(Entity, &GlobalTransform), (With<Illusion>, Without<Shown>)>,
    (data, server, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (anchors, mut materials): (Res<Anchors>, ResMut<Assets<ModelMaterial>>),
) {
    for (entity, place) in &illusions {
        if !looks.made {
            let Some(data) = data.as_deref() else {
                return;
            };
            find_looks(&mut looks, data, &server);
        }
        let place = place.translation();
        let model = looks.model.as_ref().map(|file| {
            commands
                .spawn((
                    model_looks::scene(&server, file),
                    Transform::from_translation(place)
                        .with_rotation(crate::hero::facing(world(ILLUSION_FRONT)))
                        .with_scale(Vec3::splat(iso::MODEL_SCALE)),
                    model_looks::Ordered::default(),
                    Of(entity),
                    DespawnOnExit(AppState::InGame),
                ))
                .id()
        });
        let lamp = commands
            .spawn((
                Transform::from_translation(place.with_y(0.0)),
                daynight::Light(ILLUSION_LIGHT),
                Of(entity),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        // Its texture added whole by its grey, unlit, its depth tested and not written
        // (`0x40D310`).
        let material = materials.add(in_order(
            StandardMaterial {
                base_color: Color::BLACK,
                base_color_texture: looks.texture.clone(),
                unlit: true,
                ..default()
            },
            &anchors,
            Pass::Added(Layer::Objects),
        ));
        debug!("demon illusions: an illusion shows at {place}");
        commands.entity(entity).insert(Shown {
            model,
            lamp,
            material,
            dressed: false,
            life: None,
            clock: 0.0,
        });
    }
}

/// Dresses each illusion's model in its material, untinted by the night, once its meshes are
/// there, and plays its clip once at [`ILLUSION_RATE`] once its player is there, its life then
/// twice the clip's length (`0x7A8550`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn dress(
    mut commands: Commands,
    mut shown: Query<&mut Shown>,
    (children, meshes, sources): (
        Query<&Children>,
        Query<(), With<MeshMaterial3d<ModelMaterial>>>,
        Query<&model_looks::Source>,
    ),
    (mut players, gltfs, clips, mut graphs): (
        Query<&mut AnimationPlayer, Without<AnimationGraphHandle>>,
        Res<Assets<Gltf>>,
        Res<Assets<AnimationClip>>,
        ResMut<Assets<AnimationGraph>>,
    ),
    looks: Res<Looks>,
) {
    for mut look in &mut shown {
        let Some(model) = look.model else {
            continue;
        };
        if !look.dressed {
            let own: Vec<Entity> = children
                .iter_descendants(model)
                .filter(|&entity| meshes.contains(entity))
                .collect();
            if !own.is_empty() {
                look.dressed = true;
                for entity in own {
                    commands.entity(entity).insert((
                        MeshMaterial3d(look.material.clone()),
                        RenderLayers::layer(iso::UNTINTED),
                    ));
                }
            }
        }
        if look.life.is_some() {
            continue;
        }
        let Some(clip) = sources
            .get(model)
            .ok()
            .and_then(|source| gltfs.get(&source.0))
            .zip(looks.clip.as_deref())
            .and_then(|(gltf, name)| gltf.named_animations.get(name).cloned())
        else {
            continue;
        };
        let Some(length) = clips.get(&clip).map(AnimationClip::duration) else {
            continue;
        };
        for entity in children.iter_descendants(model) {
            if let Ok(mut player) = players.get_mut(entity) {
                let (graph, node) = AnimationGraph::from_clip(clip.clone());
                player.play(node).set_speed(ILLUSION_RATE);
                commands
                    .entity(entity)
                    .insert(AnimationGraphHandle(graphs.add(graph)));
                look.life = Some(illusion_life(length));
                debug!("demon illusions: its clip of {length:.2} s plays");
            }
        }
    }
}

/// Runs each illusion a frame on (`0x7A8780`, `0x7A87B0`): its grey by its clock, and past its
/// life its end with its model and its lamp.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown)>,
    mut materials: ResMut<Assets<ModelMaterial>>,
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut look) in &mut shown {
        let Some(life) = look.life else {
            continue;
        };
        look.clock += dt;
        if look.clock >= life {
            debug!("demon illusions: an illusion ends");
            if let Some(model) = look.model {
                commands.entity(model).try_despawn();
            }
            commands.entity(look.lamp).try_despawn();
            commands.entity(entity).despawn();
            continue;
        }
        let grey = illusion_grey(look.clock, life);
        if let Some(mut material) = materials.get_mut(&look.material) {
            material.base.base_color = Color::srgb(grey, grey, grey);
        }
    }
}

/// Takes away the models and the lamps of the illusions gone with their sectors.
fn orphans(mut commands: Commands, parts: Query<(Entity, &Of)>, shown: Query<(), With<Shown>>) {
    for (part, &Of(object)) in &parts {
        if !shown.contains(object) {
            commands.entity(part).try_despawn();
        }
    }
}

/// The big demon's illusions.
pub struct DemonIllusionsPlugin;

impl Plugin for DemonIllusionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>().add_systems(
            Update,
            (show, dress, run, orphans)
                .chain()
                .in_set(Playing)
                .in_set(Unpaused)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
