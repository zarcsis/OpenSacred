//! The Gladiator's arts' sights (#1092; docs/re/effects.md, "The Gladiator's sights"): as the
//! clip of his hand of god (1012), his awe (1014) or his heroism (1015) starts,
//! `cParticleSystem_glad_sm_cast` (803) shows at him: the hand of god's pale green sparks shed at
//! his right hand for 2.6 s, each left where it is born, under a swelling green glow; the awe's five
//! white snowflake auras on the ground about his feet, one after another, each held a second and
//! fading; the heroism's burst of sparks from his right hand, drawn back to it under a green
//! star. All untinted by the night and drawn just behind his model, as the original draws the
//! system before the creature it follows (`fx::Orders`, #2178)
//! (`opensacred_rules::particles::GladSight`).
//!
//! Not followed: the awe at a follower or an allied hero that its action targets (the game makes
//! it at him), and the items' glows of his arts with empty hands.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{GLAD_BONE, GLAD_GREEN, GLAD_TEXTURES, GladSight, glad_green};

use crate::bolt_looks::{models, world};
use crate::frame::{Playing as Running, Unpaused};
use crate::fx::{
    Effect, Orders, Quads, empty, particle_material, particle_texture, whole_material,
};
use crate::game::{AppState, Data};
use crate::iso;
use crate::making::{Making, Playing};
use crate::spawning::Rng;

/// The sights' dice (`rand()`).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x803))
    }
}

/// The sights' materials, made as the first asks, none where a texture is missing: the sparks,
/// the hand of god's glow and the heroism's star added whole, the awe's auras laid over by their
/// alpha.
#[derive(Resource, Default)]
struct Looks(Option<[Option<Handle<Effect>>; 4]>);

/// A sight that shows: its Gladiator, its look, where its bone was last, and its meshes, the
/// sparks', the glow's, the auras' and the star's.
#[derive(Component)]
struct Sight {
    caster: Entity,
    look: GladSight,
    before: Option<Vec3>,
    meshes: [Handle<Mesh>; 4],
}

/// Makes the sight of each of the Gladiator's three arts whose clip starts (`0x50329F`: the
/// event `0x41`, at the art's first run).
fn start(
    mut commands: Commands,
    started: Query<(Entity, &Making), Added<Playing>>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (caster, making) in &started {
        let Some(look) = GladSight::new(making.art) else {
            continue;
        };
        let materials = looks
            .0
            .get_or_insert_with(|| {
                let mut material = |name: &str, whole: bool| {
                    let image = data
                        .as_deref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("glad sights: no texture {name}; its part is not drawn");
                    }
                    image.map(|image| {
                        effects.add(if whole {
                            whole_material(image)
                        } else {
                            particle_material(image, AlphaMode::Blend)
                        })
                    })
                };
                let [sparks, glow, aura, star] = GLAD_TEXTURES;
                [
                    material(sparks, true),
                    material(glow, true),
                    material(aura, false),
                    material(star, true),
                ]
            })
            .clone();
        let parts: [Handle<Mesh>; 4] = std::array::from_fn(|_| meshes.add(empty()));
        let mut sight = commands.spawn((
            Transform::IDENTITY,
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        for (mesh, material) in parts.iter().zip(materials) {
            if let Some(material) = material {
                sight.with_child((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                ));
            }
        }
        sight.insert(Sight {
            caster,
            look,
            before: None,
            meshes: parts,
        });
        debug!("glad sights: {caster}'s {} shows", making.art);
    }
}

/// Runs each sight a frame on (`0x78F6E0`) and draws it (`0x78F890`): the sparks facing the
/// camera in the green ramp of their life, the glow or the star at his right hand, the auras flat
/// on the ground at his root, white at their alpha; it goes once it is over, or with him.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut sights: Query<(Entity, &mut Sight)>,
    (places, children, bones, worn): (
        Query<&GlobalTransform>,
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (mut dice, mut meshes): (ResMut<Dice>, ResMut<Assets<Mesh>>),
    orders: Orders,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let ramp = glad_green();
    let (across, depth) = iso::fx_axes();
    for (entity, mut sight) in &mut sights {
        let sight = &mut *sight;
        let Ok(root) = places.get(sight.caster).map(GlobalTransform::translation) else {
            commands.entity(entity).despawn();
            continue;
        };
        let hand = crate::making::bone(sight.caster, &[GLAD_BONE], &children, (&bones, &worn))
            .unwrap_or(root);
        let before = sight.before.replace(hand).unwrap_or(hand);
        sight
            .look
            .update(dt, (models(before), models(hand)), &mut dice.0);
        if sight.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let whole = (Vec2::ZERO, 1.0);
        let mut quads: [Quads; 4] = std::array::from_fn(|_| Quads::default());
        for spark in &sight.look.system.particles {
            quads[0].push(
                world(spark.place),
                spark.size * s,
                0.0,
                whole,
                ramp.at(spark.life),
            );
        }
        if let Some((half, star)) = sight.look.glow() {
            quads[if star { 3 } else { 1 }].push(hand, half * s, 0.0, whole, GLAD_GREEN);
        }
        for (half, turn, alpha) in sight.look.auras() {
            let (sin, cos) = turn.sin_cos();
            let corner = |x: f32, y: f32| {
                root + (across * (x * cos + y * sin) + depth * (y * cos - x * sin)) * half * s
            };
            quads[2].push_corners(
                [
                    corner(-1.0, -1.0),
                    corner(1.0, -1.0),
                    corner(1.0, 1.0),
                    corner(-1.0, 1.0),
                ],
                [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
                [1.0, 1.0, 1.0, alpha],
            );
        }
        let order = orders.behind(sight.caster);
        for (handle, mut part) in sight.meshes.iter().zip(quads) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.in_order(order, Vec3::ZERO);
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The Gladiator's arts' sights.
pub struct GladSightsPlugin;

impl Plugin for GladSightsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>()
            .init_resource::<Dice>()
            .add_systems(
                Update,
                (start, run)
                    .chain()
                    .in_set(Running)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
