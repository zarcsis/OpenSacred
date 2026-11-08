//! The Vampiress's casts' sights (863, `cParticleSystem_vamp_sm_cast`, #1092; docs/re/effects.md,
//! "The Vampiress's casts"): as one of her night's arts starts (her `Rundum` 1038, `Hardhit` 1039
//! and `Attacke` 1040), a red glow at each of her claws, `Bip01 R Finger3` and `Bip01 L Finger3`,
//! left along their way for the art's time; and where a bat she calls comes (1035 to 1037), ten
//! red sparks bursting up and falling with their trails over the bats' runes flat on the ground,
//! two rings turning either way and fading (`opensacred_rules::particles::{VampClaw, VampBats,
//! spark_trail}`); and as her change starts (1022 and 1023), four ribbons about her, each along
//! the way of the bone `particle_source_01` of the model `cast01` on its clip, turned by a
//! quarter of a turn from the one before, magenta to orange, a red glow and a white spark at its
//! head lighting the models pale blue (`particles::VampRibbon`), the first lighting the night by
//! 250. None is tinted by the night. Each lies in the original's order of drawing (`fx::Orders`,
//! #2220): the claws' glows just behind her model, the bats' sights and the ribbons at their
//! places, where their objects stand.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    self, VAMP_CLAW_TEXTURE, VAMP_HEAD_GLOW_LOOK, VAMP_HEAD_SPARK, VAMP_HEAD_SPARK_LOOK,
    VAMP_RIBBON_ANGLES, VAMP_RIBBON_BONE, VAMP_RIBBON_LIGHT, VAMP_RIBBON_MODEL,
    VAMP_RIBBON_SECONDS, VAMP_RIBBON_TEXTURE, VAMP_RUNES, VAMP_SPARK_TEXTURE, VampBats, VampClaw,
    VampRibbon, spark_trail,
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

/// The sights' materials, made as first asked: the claws' glows (and the heads') and the sparks
/// added whole, the plain runes laid over by their alpha, the glowing runes, the ribbons' grid and
/// the heads' sparks added whole; none where a texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<[Option<Handle<Effect>>; 6]>);

impl Looks {
    fn get(
        &mut self,
        data: Option<&Data>,
        assets: &AssetServer,
        effects: &mut Assets<Effect>,
    ) -> [Option<Handle<Effect>>; 6] {
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
                    material(VAMP_RIBBON_TEXTURE, false),
                    material(VAMP_HEAD_SPARK, false),
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
    orders: crate::fx::Orders,
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
            // Just behind her model, as the original draws the system before the creature (#2188).
            quads.in_order(orders.behind(claw.caster), Vec3::ZERO);
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
        let [_, sparks, plain, glowing, ..] = looks.get(data.as_deref(), &assets, &mut effects);
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
/// the camera, its runes flat on the ground, at its place in the order; it goes with its last
/// spark.
fn run_bats(
    mut commands: Commands,
    time: Res<Time>,
    mut sights: Query<(Entity, &mut Bats, &Transform)>,
    (mut meshes, orders): (ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let ramp = particles::vamp_red();
    let layer = VampBats::layer();
    let s = iso::MODEL_SCALE;
    for (entity, mut sight, at) in &mut sights {
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
        let order = orders.at(at.translation);
        for (handle, mut quads) in sight.meshes.iter().zip([sparks, plain, glowing]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                quads.in_order(order, at.translation);
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The model of the change's ribbons, sampled for its bone: a scene of no mesh at the world's
/// origin, so that its bone's place is its point in the model's frame, playing its clip once, and
/// how long it lives on.
#[derive(Component)]
struct RibbonPath {
    playing: bool,
    left: f32,
}

/// How long a ribbons' model outlives their clip: their particles' longest life (1.7 s).
const PATH_OUTLIVES: f32 = 2.0;

/// A ribbon of the change about its caster's root, where it was placed: its model, its look, its
/// head's point a frame before in its frame, its meshes (its particles, its head's glow and its
/// head's spark) and its Direct3D light.
#[derive(Component)]
struct Ribbon {
    path: Entity,
    look: VampRibbon,
    before: Option<[f32; 3]>,
    meshes: [Handle<Mesh>; 3],
    light: Entity,
}

/// The change's model's file, once looked for.
#[derive(Resource, Default)]
struct RibbonModel(Option<Option<String>>);

/// Makes the four ribbons of each change the Vampiress starts (the start's case 0, `0x51431A`),
/// at her root, and the model whose bone they follow.
fn change(
    mut commands: Commands,
    started: Query<(Entity, &Making, &GlobalTransform), Added<Playing>>,
    (data, assets, mut looks, mut model): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<RibbonModel>,
    ),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (caster, making, at) in &started {
        if !matches!(making.art, 1022 | 1023) {
            continue;
        }
        let file = model
            .0
            .get_or_insert_with(|| {
                let file = data.as_deref().and_then(|data| {
                    crate::npcs::model_files(&data.0.assets)
                        .ok()
                        .and_then(|files| files.get(VAMP_RIBBON_MODEL).cloned())
                });
                if file.is_none() {
                    warn!("vamp_casts: no {VAMP_RIBBON_MODEL}; the change's ribbons are not drawn");
                }
                file
            })
            .clone();
        let Some(file) = file else {
            continue;
        };
        let [glow, _, _, _, grid, spark] = looks.get(data.as_deref(), &assets, &mut effects);
        let path = commands
            .spawn((
                crate::models::scene(&assets, &file),
                Transform::IDENTITY,
                Visibility::Hidden,
                RibbonPath {
                    playing: false,
                    left: VAMP_RIBBON_SECONDS + PATH_OUTLIVES,
                },
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let place = at.translation();
        for (index, angle) in VAMP_RIBBON_ANGLES.into_iter().enumerate() {
            let parts = [
                meshes.add(empty()),
                meshes.add(empty()),
                meshes.add(empty()),
            ];
            let mut ribbon = commands.spawn((
                Transform::from_translation(place),
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
            ));
            if index == 0 {
                ribbon.insert(crate::daynight::Light(VAMP_RIBBON_LIGHT));
            }
            for (mesh, material) in parts
                .iter()
                .zip([grid.clone(), glow.clone(), spark.clone()])
            {
                part(&mut ribbon, mesh, material);
            }
            let owner = ribbon.id();
            let light = commands
                .spawn(crate::fx_lights::light(
                    crate::fx_lights::PALE_BLUE,
                    crate::fx_lights::LOW,
                    place,
                    owner,
                    (None, Vec3::ZERO),
                ))
                .id();
            commands.entity(owner).insert(Ribbon {
                path,
                look: VampRibbon::new(angle, VAMP_RIBBON_SECONDS),
                before: None,
                meshes: parts,
                light,
            });
        }
        debug!(
            "vamp_casts: {caster}'s change makes its ribbons for {}",
            making.art
        );
    }
}

/// Plays each ribbons' model's clip once as its player is there, and takes the model off once its
/// ribbons are over.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn play_paths(
    mut commands: Commands,
    time: Res<Time>,
    mut paths: Query<(Entity, &mut RibbonPath, &crate::models::Source)>,
    (children, mut players, gltfs, mut graphs): (
        Query<&Children>,
        Query<&mut AnimationPlayer, Without<AnimationGraphHandle>>,
        Res<Assets<Gltf>>,
        ResMut<Assets<AnimationGraph>>,
    ),
) {
    for (entity, mut path, source) in &mut paths {
        path.left -= time.delta_secs();
        if path.left <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        if path.playing {
            continue;
        }
        let Some(clip) = gltfs
            .get(&source.0)
            .and_then(|gltf| gltf.animations.first().cloned())
        else {
            continue;
        };
        for entity in children.iter_descendants(entity) {
            if let Ok(mut player) = players.get_mut(entity) {
                let (graph, node) = AnimationGraph::from_clip(clip.clone());
                player.play(node);
                commands
                    .entity(entity)
                    .insert(AnimationGraphHandle(graphs.add(graph)));
                path.playing = true;
            }
        }
    }
}

/// Runs each ribbon a frame on (`0x7918D0`, `0x791AB0`): its head at its model's bone, in the
/// models' frame the glTF's (x, z, y) turned by its angle, its particles born along its way, each a
/// quad of its frame of the grid facing the camera, and while it shows its head's glow and spark,
/// its light with them, at its place in the order, where its object stands; it goes once it is
/// over. It waits for its model's bone.
fn run_ribbons(
    mut commands: Commands,
    time: Res<Time>,
    mut ribbons: Query<(Entity, &mut Ribbon, &Transform)>,
    mut lights: Query<&mut Transform, (With<crate::fx_lights::FxLight>, Without<Ribbon>)>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (mut dice, mut meshes, orders): (ResMut<Dice>, ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let ramps = VampRibbon::ramps();
    for (entity, mut ribbon, at) in &mut ribbons {
        let ribbon = &mut *ribbon;
        let point =
            crate::making::bone(ribbon.path, &[VAMP_RIBBON_BONE], &children, (&bones, &worn))
                .map(|point| ribbon.look.turned([point.x, point.z, point.y]))
                .or(ribbon.before);
        let Some(head) = point else {
            continue;
        };
        let before = ribbon.before.replace(head).unwrap_or(head);
        ribbon.look.update(dt, (before, head), &mut dice.0);
        if ribbon.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut grid = Quads::default();
        for particle in &ribbon.look.system.particles {
            let cell = Vec2::new(
                f32::from(particle.frame % 2),
                f32::from(particle.frame / 2 % 2),
            ) * 0.5;
            let colour = ramps
                .get(usize::from(particle.layer))
                .map_or([0.0; 4], |ramp| ramp.at(particle.life));
            grid.push(
                world(particle.place),
                particle.size * s,
                0.0,
                (cell, 0.5),
                colour,
            );
        }
        let (mut glow, mut spark) = (Quads::default(), Quads::default());
        if ribbon.look.heads() {
            let (half, colour) = VAMP_HEAD_GLOW_LOOK;
            glow.push(world(head), half * s, 0.0, (Vec2::ZERO, 1.0), colour);
            let (half, colour) = VAMP_HEAD_SPARK_LOOK;
            spark.push(world(head), half * s, 0.0, (Vec2::ZERO, 1.0), colour);
        }
        if let Ok(mut light) = lights.get_mut(ribbon.light) {
            light.translation = at.translation + world(head);
        }
        let order = orders.at(at.translation);
        for (handle, mut quads) in ribbon.meshes.iter().zip([grid, glow, spark]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                quads.in_order(order, at.translation);
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
            .init_resource::<RibbonModel>()
            .add_systems(
                Update,
                (
                    claws,
                    run_claws,
                    make_bats,
                    run_bats,
                    change,
                    play_paths,
                    run_ribbons,
                )
                    .chain()
                    .in_set(Running)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
