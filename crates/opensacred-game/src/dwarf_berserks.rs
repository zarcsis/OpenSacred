//! The look of the Dwarf's berserk (1082, its object 1019, `cParticleSystem_kriegsrausch`;
//! #1092; docs/re/effects.md, "The Dwarf's berserk"): over his head for the effect's seconds,
//! ribbons of `fx_kriegsrausch` along the nine points of its model's clip (`fx_dwarf_kriegsrausch`,
//! an arc sweeping round and widening), each ribbon turned and tilted its own way, as many and as
//! wide as his frenzy is high of the effect's whole value; fading in and out over a second, added
//! whole, untinted by the night, lighting the ground about him by night, just behind his model
//! (`fx::Orders`, #2178) (`opensacred_rules::particles::Berserk`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::gltf::Gltf;
use bevy::prelude::*;
use opensacred_rules::particles::{
    BERSERK_LIGHT, BERSERK_MODEL, BERSERK_POINTS, BERSERK_TEXTURE, Berserk,
};

use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::models as model_looks;
use crate::spawning::Rng;
use crate::{daynight, iso};

/// The bone whose height the ribbons stand at (the system's point 0, `+0x72` 20).
const HEAD: &str = "Bip01 Head";

/// The berserks' model, relative to the asset directory, and their ribbons' material, found at
/// the first berserk.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    model: Option<String>,
    material: Option<Handle<Effect>>,
}

/// The berserks' dice (`rand()`).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x1019))
    }
}

/// A berserk to make: its bearer, its seconds and the effect's whole value.
#[derive(Component)]
struct Unmade {
    bearer: Entity,
    seconds: f32,
    whole: f32,
}

/// A berserk that shows: its bearer, the effect's whole value, its look, its model, the model's
/// points once found, whether its clip plays, and its ribbons' mesh.
#[derive(Component)]
struct Shown {
    bearer: Entity,
    whole: f32,
    look: Berserk,
    model: Option<Entity>,
    points: Option<[Entity; 9]>,
    playing: bool,
    mesh: Handle<Mesh>,
}

/// Shows the berserk's ribbons over `bearer` for `seconds`, the effect 47's, of the whole value
/// `whole` (`0x5160AA`).
pub(crate) fn show(commands: &mut Commands, bearer: Entity, seconds: f32, whole: f32) {
    commands.spawn((
        Unmade {
            bearer,
            seconds,
            whole,
        },
        DespawnOnExit(AppState::InGame),
    ));
}

/// Gives each new berserk its model, playing out of sight at the world's origin, its mesh and its
/// light (`0x7B5950`, `0x7B64D0`).
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    places: Query<&GlobalTransform>,
    (data, assets, mut looks, mut dice): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Dice>,
    ),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, unmade) in &unmade {
        let Ok(at) = places.get(unmade.bearer).map(GlobalTransform::translation) else {
            commands.entity(entity).despawn();
            continue;
        };
        if !looks.made {
            looks.made = true;
            if let Some(data) = data.as_deref() {
                looks.model = crate::npcs::model_files(&data.0.assets)
                    .ok()
                    .and_then(|files| files.get(BERSERK_MODEL).cloned());
                if looks.model.is_none() {
                    warn!("dwarf berserks: no {BERSERK_MODEL}; their ribbons are not drawn");
                }
                let image = particle_texture(&data.0.assets, &assets, BERSERK_TEXTURE);
                if image.is_none() {
                    warn!("dwarf berserks: no texture {BERSERK_TEXTURE}; they are not drawn");
                }
                looks.material = image.map(|image| effects.add(whole_material(image)));
            }
        }
        let model = looks.model.as_ref().map(|model| {
            commands
                .spawn((
                    model_looks::scene(&assets, model),
                    Transform::IDENTITY,
                    Visibility::Hidden,
                    DespawnOnExit(AppState::InGame),
                ))
                .id()
        });
        let mesh = meshes.add(empty());
        let mut shown = commands.entity(entity);
        shown.remove::<Unmade>().insert((
            Shown {
                bearer: unmade.bearer,
                whole: unmade.whole,
                look: Berserk::new(unmade.seconds, &mut dice.0),
                model,
                points: None,
                playing: false,
                mesh: mesh.clone(),
            },
            Transform::from_translation(at),
            Visibility::default(),
            daynight::Light(BERSERK_LIGHT),
        ));
        if let Some(material) = looks.material.clone() {
            shown.with_child((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
            ));
        }
    }
}

/// Plays each berserk's clip, looping at its speed (**hypothesis**: the controlled animation's
/// parameters 2 and 3), once its player is there, and finds its nine points.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn play(
    mut commands: Commands,
    mut shown: Query<&mut Shown>,
    (children, sources, names, worn): (
        Query<&Children>,
        Query<&model_looks::Source>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (mut players, gltfs, mut graphs): (
        Query<&mut AnimationPlayer, Without<AnimationGraphHandle>>,
        Res<Assets<Gltf>>,
        ResMut<Assets<AnimationGraph>>,
    ),
) {
    for mut berserk in &mut shown {
        let Some(model) = berserk.model else {
            continue;
        };
        if !berserk.playing {
            let clip = sources
                .get(model)
                .ok()
                .and_then(|source| gltfs.get(&source.0))
                .and_then(|gltf| gltf.animations.first().cloned());
            if let Some(clip) = clip {
                for entity in children.iter_descendants(model) {
                    if let Ok(mut player) = players.get_mut(entity) {
                        let (graph, node) = AnimationGraph::from_clip(clip.clone());
                        player.play(node).repeat().set_speed(berserk.look.speed());
                        commands
                            .entity(entity)
                            .insert(AnimationGraphHandle(graphs.add(graph)));
                        berserk.playing = true;
                    }
                }
            }
        }
        if berserk.points.is_none() {
            let found: Option<Vec<Entity>> = BERSERK_POINTS
                .iter()
                .map(|name| crate::making::bone_entity(model, &[name], &children, (&names, &worn)))
                .collect();
            berserk.points = found.and_then(|found| found.try_into().ok());
        }
    }
}

/// Runs each berserk a frame on (`0x7B5F30`) at its bearer's frenzy and draws it (`0x7B60D0`): its
/// ribbons over his place at his head's height, each a strip of its nine pairs (`u` across, `v`
/// along by eighths); it goes, with its model, once it has faded out, or with its bearer.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &mut Transform)>,
    (places, children, bones, worn): (
        Query<&GlobalTransform>,
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    frenzies: Query<&crate::fight::Frenzy>,
    mut meshes: ResMut<Assets<Mesh>>,
    orders: Orders,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut berserk, mut transform) in &mut shown {
        let berserk = &mut *berserk;
        let frenzy = frenzies.get(berserk.bearer).map_or(0.0, |frenzy| frenzy.0);
        berserk
            .look
            .update(dt, frenzy / berserk.whole.max(f32::EPSILON));
        let root = places
            .get(berserk.bearer)
            .map(GlobalTransform::translation)
            .ok();
        let Some(root) = root.filter(|_| !berserk.look.done()) else {
            if let Some(model) = berserk.model {
                commands.entity(model).try_despawn();
            }
            commands.entity(entity).despawn();
            continue;
        };
        let head = crate::making::bone(berserk.bearer, &[HEAD], &children, (&bones, &worn))
            .map_or(root.y, |head| head.y);
        let origin = Vec3::new(root.x, head, root.z);
        transform.translation = origin;
        // Its clip's points in the model's frame, the model standing at the origin unscaled.
        let points = berserk.points.and_then(|points| {
            let mut found = [[0.0; 3]; 9];
            for (point, entity) in found.iter_mut().zip(points) {
                *point = models(places.get(entity).ok()?.translation() * s);
            }
            Some(found)
        });
        let mut quads = Quads::default();
        for ribbon in points
            .map(|points| berserk.look.ribbons(&points))
            .unwrap_or_default()
        {
            for k in 0..8u8 {
                let (v0, v1) = (f32::from(k) / 8.0, f32::from(k + 1) / 8.0);
                let ([a0, b0], [a1, b1]) = (ribbon[usize::from(k)], ribbon[usize::from(k + 1)]);
                quads.push_corners(
                    [world(a0), world(b0), world(b1), world(a1)],
                    [
                        Vec2::new(0.0, v0),
                        Vec2::new(1.0, v0),
                        Vec2::new(1.0, v1),
                        Vec2::new(0.0, v1),
                    ],
                    [1.0; 4],
                );
            }
        }
        if let Some(mut mesh) = meshes.get_mut(&berserk.mesh) {
            quads.in_order(orders.behind(berserk.bearer), origin);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The Dwarf's berserks' looks.
pub struct DwarfBerserksPlugin;

impl Plugin for DwarfBerserksPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>()
            .init_resource::<Dice>()
            .add_systems(
                Update,
                (make, play, run)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
