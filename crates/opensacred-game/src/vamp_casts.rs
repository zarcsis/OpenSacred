//! The Vampiress's casts' sights (863, `cParticleSystem_vamp_sm_cast`, #1092; docs/re/effects.md,
//! "The Vampiress's casts"): as one of her night's arts starts (her `Rundum` 1038, `Hardhit` 1039
//! and `Attacke` 1040), a red glow at each of her claws, `Bip01 R Finger3` and `Bip01 L Finger3`,
//! left along their way for the art's time; and where a bat she calls comes (1035 to 1037), ten
//! red sparks bursting up and falling with their trails over the bats' runes flat on the ground,
//! two rings turning either way and fading (`opensacred_rules::particles::{VampClaw, VampBats,
//! spark_trail}`). None is tinted by the night.
//!
//! Not followed: her change's ribbons (1022 and 1023, the start's case 0).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    self, VAMP_CLAW_TEXTURE, VAMP_RUNES, VAMP_SPARK_TEXTURE, VampBats, VampClaw, spark_trail,
};

use crate::bolt_looks::{models, world};
use crate::frame::{Playing as Running, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;
use crate::making::{Making, Playing};
use crate::spawning::Rng;

/// The claws' bones, the first object's and the second's (22 and 21), each else her hand.
const CLAWS: [[&str; 2]; 2] = [
    ["Bip01 R Finger3", "Bip01 R Hand"],
    ["Bip01 L Finger3", "Bip01 L Hand"],
];

/// How far over the ground the runes lie, in world units.
const GROUND_LIFT: f32 = 0.5;

/// The sights' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x863))
    }
}

/// The sights' materials, made as first asked: the claws' glows and the sparks added whole, the
/// plain runes laid over by their alpha and the glowing runes added whole; none where a texture
/// is missing.
#[derive(Resource, Default)]
struct Looks(Option<[Option<Handle<Effect>>; 4]>);

impl Looks {
    fn get(
        &mut self,
        data: Option<&Data>,
        assets: &AssetServer,
        effects: &mut Assets<Effect>,
    ) -> [Option<Handle<Effect>>; 4] {
        self.0
            .get_or_insert_with(|| {
                let mut material = |name: &str, over: bool| {
                    let image =
                        data.and_then(|data| particle_texture(&data.0.assets, assets, name));
                    if image.is_none() {
                        warn!("vamp_casts: no texture {name}; its part is not drawn");
                    }
                    image.map(|image| {
                        effects.add(if over {
                            particle_material(image, AlphaMode::Blend)
                        } else {
                            whole_material(image)
                        })
                    })
                };
                [
                    material(VAMP_CLAW_TEXTURE, false),
                    material(VAMP_SPARK_TEXTURE, false),
                    material(VAMP_RUNES[0], true),
                    material(VAMP_RUNES[1], false),
                ]
            })
            .clone()
    }
}

/// A claw's glow at its caster's bone: its look, its bone's place now and a frame before, in the
/// world, and its mesh.
#[derive(Component)]
struct Claw {
    caster: Entity,
    bones: [&'static str; 2],
    look: VampClaw,
    place: Vec3,
    before: Option<Vec3>,
    mesh: Handle<Mesh>,
}

/// A bat's sight at its place: its look and the meshes of its sparks, its plain runes and its
/// glowing runes.
#[derive(Component)]
struct Bats {
    look: VampBats,
    meshes: [Handle<Mesh>; 3],
}

/// A bat's sight to make at its entity's place ([`make_bats`]).
#[derive(Component)]
struct UnmadeBats;

/// Starts a bat's sight at `place`, the bat's as it comes, from a world, as the bats' call runs
/// (`followers`).
pub fn bats_in(world: &mut World, place: Vec3) {
    world.spawn((
        UnmadeBats,
        Transform::from_translation(place),
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
}

/// A mesh of `material`, a child of `parent`, untinted by the night.
fn part(parent: &mut EntityCommands, mesh: &Handle<Mesh>, material: Option<Handle<Effect>>) {
    if let Some(material) = material {
        parent.with_child((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            NoFrustumCulling,
            RenderLayers::layer(iso::UNTINTED),
        ));
    }
}

/// Makes the two claws' glows of each night's art that the Vampiress starts (the start's cases 2
/// to 4, `0x513E3B`), the right's first.
fn claws(
    mut commands: Commands,
    started: Query<(Entity, &Making, &GlobalTransform), Added<Playing>>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (caster, making, at) in &started {
        if particles::vamp_claws_until(making.art).is_none() {
            continue;
        }
        let [glow, ..] = looks.get(data.as_deref(), &assets, &mut effects);
        for bones in CLAWS {
            let Some(look) = VampClaw::new(making.art) else {
                continue;
            };
            let mesh = meshes.add(empty());
            let mut claw = commands.spawn((
                Transform::IDENTITY,
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
            ));
            part(&mut claw, &mesh, glow.clone());
            claw.insert(Claw {
                caster,
                bones,
                look,
                place: at.translation(),
                before: None,
                mesh,
            });
        }
        debug!("vamp_casts: {caster}'s claws glow for {}", making.art);
    }
}

/// Runs each claw a frame on at its bone (the flag `0x200`, `0x5A9C50`): its glows left where
/// they are born, each a quad facing the camera; it goes with its caster or once it is over.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn run_claws(
    mut commands: Commands,
    time: Res<Time>,
    mut claws: Query<(Entity, &mut Claw)>,
    (places, children, bones, worn): (
        Query<&GlobalTransform>,
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (mut dice, mut meshes): (ResMut<Dice>, ResMut<Assets<Mesh>>),
) {
    let dt = time.delta_secs().min(0.1);
    let ramp = particles::vamp_red();
    for (entity, mut claw) in &mut claws {
        let claw = &mut *claw;
        let Ok(at) = places.get(claw.caster) else {
            commands.entity(entity).despawn();
            continue;
        };
        claw.place = crate::making::bone(claw.caster, &claw.bones, &children, (&bones, &worn))
            .unwrap_or(at.translation());
        let before = claw.before.replace(claw.place).unwrap_or(claw.place);
        claw.look
            .update(dt, (models(before), models(claw.place)), &mut dice.0);
        if claw.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut quads = Quads::default();
        for particle in &claw.look.system.particles {
            quads.push(
                world(particle.place),
                particle.size * iso::MODEL_SCALE,
                0.0,
                (Vec2::ZERO, 1.0),
                ramp.at(particle.life),
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&claw.mesh) {
            quads.into_mesh(&mut mesh);
        }
    }
}

/// Gives each new bat's sight its look, its meshes and its materials.
fn make_bats(
    mut commands: Commands,
    unmade: Query<Entity, With<UnmadeBats>>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes, mut dice): (
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
        ResMut<Dice>,
    ),
) {
    for entity in &unmade {
        let [_, sparks, plain, glowing] = looks.get(data.as_deref(), &assets, &mut effects);
        let parts = [
            meshes.add(empty()),
            meshes.add(empty()),
            meshes.add(empty()),
        ];
        let mut sight = commands.entity(entity);
        sight.remove::<UnmadeBats>().insert(Bats {
            look: VampBats::new(&mut dice.0),
            meshes: parts.clone(),
        });
        // The runes first, then the sparks over them (`0x791AB0`).
        part(&mut sight, &parts[1], plain);
        part(&mut sight, &parts[2], glowing);
        part(&mut sight, &parts[0], sparks);
        debug!("vamp_casts: a bat's runes at {entity}");
    }
}

/// Runs each bat's sight a frame on (`0x7918D0`, `0x791AB0`): its sparks with their trails facing
/// the camera, its runes flat on the ground; it goes with its last spark.
fn run_bats(
    mut commands: Commands,
    time: Res<Time>,
    mut sights: Query<(Entity, &mut Bats)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs().min(0.1);
    let ramp = particles::vamp_red();
    let layer = VampBats::layer();
    let s = iso::MODEL_SCALE;
    for (entity, mut sight) in &mut sights {
        sight.look.update(dt);
        if sight.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut sparks = Quads::default();
        for particle in &sight.look.system.particles {
            let colour = ramp.at(particle.life);
            sparks.push(
                world(particle.place),
                particle.size * s,
                0.0,
                (Vec2::ZERO, 1.0),
                colour,
            );
            for (place, half) in spark_trail(particle, &layer) {
                sparks.push(world(place), half * s, 0.0, (Vec2::ZERO, 1.0), colour);
            }
        }
        let mut runes = [Quads::default(), Quads::default()];
        for rune in sight.look.runes() {
            let (sin, cos) = rune.turn.sin_cos();
            let corner = |(x, y): (f32, f32)| {
                world([
                    rune.half * (x * cos - y * sin),
                    rune.half * (x * sin + y * cos),
                    0.0,
                ]) + Vec3::Y * GROUND_LIFT
            };
            if let Some(quads) = runes.get_mut(rune.texture) {
                quads.push_corners(
                    [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(corner),
                    [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
                    rune.colour,
                );
            }
        }
        let [plain, glowing] = runes;
        for (handle, quads) in sight.meshes.iter().zip([sparks, plain, glowing]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The Vampiress's casts' sights.
pub struct VampCastsPlugin;

impl Plugin for VampCastsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (claws, run_claws, make_bats, run_bats)
                    .chain()
                    .in_set(Running)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
