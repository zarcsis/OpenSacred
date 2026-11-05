//! The look of the Wood Elf's bubble (847, `FX_BUBBLETRAP`, #1092; docs/re/effects.md, "The
//! bubble"): each bubble bolt shows its bush, its clip whirling in its loop, grown to twice its
//! size in a third of a second, turned along its flight, lit, in the hero's texture or another
//! caster's; it shrinks away over its last half second, or where a cell stops it; an arrow that
//! bursts it leaves green glows drifting where it was and a green sonar ring spreading on the
//! ground under it (906), both added and untinted (`opensacred_rules::particles::{BubbleTrap,
//! SonarRing}`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::gltf::Gltf;
use bevy::prelude::*;
use opensacred_rules::particles::{self, BubbleTrap, SonarRing};

use crate::bolt_looks::{models, world};
use crate::bolts::{Bubble, Pops};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_texture, texture_path, whole_material};
use crate::game::{AppState, Data};
use crate::iso;
use crate::models as model_looks;
use crate::npcs::Retexture;
use crate::spawning::Rng;
use crate::terrain::Heights;

/// How far over the ground the sonar ring lies, in world units.
const GROUND_LIFT: f32 = 0.5;

/// The sonar ring's segments (`0x763530`).
const SONAR_SEGMENTS: u16 = 50;

/// The bubbles' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x847))
    }
}

/// What the looks take, found at the first bubble: its bush's model and its textures, a hero's
/// and another's, relative to the asset directory, the glows' material and the sonar ring's.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    model: Option<String>,
    textures: [Option<String>; 2],
    glows: Option<Handle<Effect>>,
    sonar: Option<Handle<Effect>>,
}

impl Looks {
    /// Finds the looks' files at the first look asked.
    fn make(&mut self, data: Option<&Data>, assets: &AssetServer, effects: &mut Assets<Effect>) {
        if self.made {
            return;
        }
        self.made = true;
        let Some(data) = data else {
            return;
        };
        let shared = &data.0;
        self.model = crate::npcs::model_files(&shared.assets)
            .ok()
            .and_then(|files| files.get(particles::BUBBLE_MODEL).cloned());
        if self.model.is_none() {
            warn!(
                "bubbles: no {}; their bushes are not drawn",
                particles::BUBBLE_MODEL
            );
        }
        self.textures = [particles::BUBBLE_TEXTURE, particles::BUBBLE_OTHERS_TEXTURE]
            .map(|name| texture_path(&shared.assets, name));
        let texture = |name: &str| {
            let image = particle_texture(&shared.assets, assets, name);
            if image.is_none() {
                warn!("bubbles: no texture {name}; its part is not drawn");
            }
            image
        };
        self.glows =
            texture(particles::BUBBLE_GLOW_TEXTURE).map(|image| effects.add(whole_material(image)));
        self.sonar =
            texture(particles::SONAR_TEXTURE).map(|image| effects.add(whole_material(image)));
    }
}

/// A bubble's look, which outlives its bolt while its glows live: its bolt while it flies, its
/// system, its bush, its place, its direction in the world, whether an arrow burst it, its
/// glows' mesh, and whether its clip plays.
#[derive(Component)]
struct Shown {
    bolt: Option<Entity>,
    look: BubbleTrap,
    model: Option<Entity>,
    place: Vec3,
    along: Vec3,
    popped: bool,
    mesh: Handle<Mesh>,
    playing: bool,
}

/// A bubble's burst's sonar ring on the ground: its system, its centre and its mesh.
#[derive(Component)]
struct Sonar {
    ring: SonarRing,
    centre: Vec3,
    mesh: Handle<Mesh>,
}

/// Gives each new bubble its look (`0x797B30`, `0x798380`): its bush and its glows' mesh.
fn show(
    mut commands: Commands,
    bolts: Query<(Entity, &Bubble, &Transform), Added<Bubble>>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (bolt, bubble, place) in &bolts {
        looks.make(data.as_deref(), &assets, &mut effects);
        let at = place.translation;
        let along = Vec3::new(bubble.direction.x, 0.0, bubble.direction.y).normalize_or(Vec3::X);
        let texture = looks.textures[usize::from(!bubble.hero)].clone();
        let model = looks.model.as_ref().map(|model| {
            let mut made = commands.spawn((
                model_looks::scene(&assets, model),
                bush_transform(at, along, 0.0),
                model_looks::Ordered::default(),
                DespawnOnExit(AppState::InGame),
            ));
            // The hero's bush in its texture, another caster's in its own (`0x401BD0`).
            if let Some(texture) = &texture {
                made.insert(Retexture::load(&assets, texture));
            }
            made.id()
        });
        let mesh = meshes.add(empty());
        let mut shown = commands.spawn((
            Transform::IDENTITY,
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(material) = looks.glows.clone() {
            shown.with_child((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
            ));
        }
        shown.insert(Shown {
            bolt: Some(bolt),
            look: BubbleTrap::new(bubble.seconds),
            model,
            place: at,
            along,
            popped: false,
            mesh,
            playing: false,
        });
    }
}

/// A bush's transform at `at` (`0x798030`): along `along`, at twice its size times `scale`, at
/// the models' scale.
fn bush_transform(at: Vec3, along: Vec3, scale: f32) -> Transform {
    Transform::from_translation(at)
        .looking_to(-along, Vec3::Y)
        .with_scale(Vec3::splat(2.0 * scale * iso::MODEL_SCALE))
}

/// Plays each bush's clip's loop (`0x4040D0`) once its player is there.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn play(
    mut commands: Commands,
    mut shown: Query<&mut Shown>,
    (children, sources): (Query<&Children>, Query<&model_looks::Source>),
    (mut players, gltfs, mut graphs): (
        Query<&mut AnimationPlayer, Without<AnimationGraphHandle>>,
        Res<Assets<Gltf>>,
        ResMut<Assets<AnimationGraph>>,
    ),
) {
    for mut look in &mut shown {
        let Some(model) = look.model.filter(|_| !look.playing) else {
            continue;
        };
        let clip = sources
            .get(model)
            .ok()
            .and_then(|source| gltfs.get(&source.0))
            .and_then(|gltf| gltf.animations.first().cloned());
        let Some(clip) = clip else {
            continue;
        };
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

/// Runs each bubble's look a frame on (`0x797D80`, `0x798030`): its bush along its bolt's way,
/// its growth and its shrinking, its burst where an arrow bursts it, with its sonar ring, and
/// its end.
fn fly(
    mut commands: Commands,
    (time, heights): (Res<Time>, Res<Heights>),
    mut pops: MessageReader<Pops>,
    mut shown: Query<(Entity, &mut Shown)>,
    bolts: Query<&Transform, (With<Bubble>, Without<Shown>)>,
    mut bushes: Query<&mut Transform, (Without<Bubble>, Without<Shown>)>,
    (looks, mut dice, mut meshes): (Res<Looks>, ResMut<Dice>, ResMut<Assets<Mesh>>),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let popped: Vec<Entity> = pops.read().map(|pop| pop.0).collect();
    let ramp = BubbleTrap::ramp();
    for (entity, mut look) in &mut shown {
        let look = &mut *look;
        if look.bolt.is_some_and(|bolt| popped.contains(&bolt)) {
            look.popped = true;
        }
        match look.bolt.map(|bolt| bolts.get(bolt)) {
            Some(Ok(place)) => look.place = place.translation,
            _ => {
                if let Some(bolt) = look.bolt.take() {
                    if look.popped {
                        // An arrow burst it (`+0x74` bit 8): its glows and its sonar ring 906.
                        debug!("bubbles: {bolt} bursts at {}", look.place);
                        look.look.burst(models(look.place), &mut dice.0);
                        let ground = heights
                            .at(look.place.x, look.place.z)
                            .unwrap_or(look.place.y - crate::bolts::HEIGHT);
                        sonar(
                            &mut commands,
                            Vec3::new(look.place.x, ground + GROUND_LIFT, look.place.z),
                            looks.sonar.clone(),
                            &mut meshes,
                        );
                    } else {
                        // Its time's end or a cell that stops it: it shrinks away.
                        look.look.stop();
                    }
                }
            }
        }
        look.look.update(dt);
        if let Some(model) = look.model {
            if look.look.drawn() {
                if let Ok(mut transform) = bushes.get_mut(model) {
                    *transform = bush_transform(look.place, look.along, look.look.scale);
                }
            } else {
                commands.entity(model).despawn();
                look.model = None;
            }
        }
        if look.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut quads = Quads::default();
        for glow in &look.look.system.particles {
            quads.push(
                world(glow.place),
                glow.size * s,
                glow.angle,
                (Vec2::ZERO, 1.0),
                ramp.at(glow.life),
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&look.mesh) {
            quads.into_mesh(&mut mesh);
        }
    }
}

/// Makes a sonar ring at `centre` on the ground (`0x798460`).
fn sonar(
    commands: &mut Commands,
    centre: Vec3,
    material: Option<Handle<Effect>>,
    meshes: &mut Assets<Mesh>,
) {
    let mesh = meshes.add(empty());
    let mut ring = commands.spawn((
        Sonar {
            ring: SonarRing::default(),
            centre,
            mesh: mesh.clone(),
        },
        Transform::IDENTITY,
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
    // Unlit and untinted, added whole (`0x763530`).
    if let Some(material) = material {
        ring.with_child((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            NoFrustumCulling,
            RenderLayers::layer(iso::UNTINTED),
        ));
    }
}

/// Spreads each sonar ring a frame on (`0x7984F0`, `0x798530`), and ends it past half a second.
fn spread(
    mut commands: Commands,
    time: Res<Time>,
    mut rings: Query<(Entity, &mut Sonar)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut sonar) in &mut rings {
        sonar.ring.update(dt);
        if sonar.ring.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let (inner, outer) = sonar.ring.radii();
        let mut quads = Quads::default();
        quads.push_ring(
            sonar.centre,
            (inner * s, outer * s),
            SONAR_SEGMENTS,
            sonar.ring.colour(),
        );
        if let Some(mut mesh) = meshes.get_mut(&sonar.mesh) {
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The bubbles' looks.
pub struct BubbleTrapsPlugin;

impl Plugin for BubbleTrapsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (show, play, fly, spread)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
