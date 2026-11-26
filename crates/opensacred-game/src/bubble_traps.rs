//! The look of the Wood Elf's bubble (847, `FX_BUBBLETRAP`, #1092; docs/re/effects.md, "The
//! bubble"): each bubble bolt shows its bush, its clip whirling in its loop, grown to twice its
//! size in a third of a second, turned along its flight, lit, in the hero's texture or another
//! caster's; it shrinks away over its last half second, or where a cell stops it; an arrow that
//! bursts it leaves green glows drifting from its bush's bone `Bone01` as its clip has it and a
//! green sonar ring spreading on the ground under it (906), both added and untinted
//! (`opensacred_rules::particles::{BubbleTrap, SonarRing}`).
//! The bubble lights the world on the ground under it by 100, by 150 once burst, and its ring by
//! 200 (`daynight::Light`, #3153; docs/re/effects.md, "Lights"). Its bush casts its shadow on the
//! ground under it as it flies, shifted by half its height (`shadows::Plane`, `0x79817A`, #3162).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::gltf::Gltf;
use bevy::prelude::*;
use opensacred_rules::particles::{self, BubbleTrap, SonarRing};

use crate::bolt_looks::{models, world};
use crate::bolts::{Bubble, Pops};
use crate::daynight::Light;
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
/// system, its bush, its light, its place, its direction in the world, whether an arrow burst
/// it, its glows' mesh, and whether its clip plays.
#[derive(Component)]
struct Shown {
    bolt: Option<Entity>,
    look: BubbleTrap,
    model: Option<Entity>,
    lamp: Entity,
    place: Vec3,
    along: Vec3,
    popped: bool,
    mesh: Handle<Mesh>,
    playing: bool,
    /// Whether a hero cast it, which its bush's texture tells.
    hero: bool,
}

/// A bubble's burst's sonar ring on the ground: its system, its centre and its mesh.
#[derive(Component)]
struct Sonar {
    ring: SonarRing,
    centre: Vec3,
    mesh: Handle<Mesh>,
}

/// A look carried alone by a load, its bolt gone, which [`show`] makes and then lets go (#3814).
#[derive(Component)]
struct Alone;

/// A bubble's look as a save keeps it (#3814, the epic #3753): its system, its place and its
/// direction, whether an arrow burst it and whether a hero cast it. A load gives a flying
/// bubble's to its bolt made again (`bolts::SavedBolt`), and makes one that outlives its bolt
/// again on its own.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename = "SavedBubble")]
pub struct SavedLook {
    look: BubbleTrap,
    place: Vec3,
    along: Vec3,
    popped: bool,
    hero: bool,
}

impl SavedLook {
    /// The look `shown` as a save keeps it.
    fn of(shown: &Shown) -> Self {
        Self {
            look: shown.look.clone(),
            place: shown.place,
            along: shown.along,
            popped: shown.popped,
            hero: shown.hero,
        }
    }
}

/// A sonar ring as a save keeps it (#3814): its system and its centre.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedSonar {
    ring: SonarRing,
    centre: Vec3,
}

/// The look of the bubble's bolt `bolt` as a save keeps it with its bolt, if it shows (#3814).
pub(crate) fn saved_look(world: &World, bolt: Entity) -> Option<SavedLook> {
    let mut shown = world.try_query::<&Shown>()?;
    shown
        .iter(world)
        .find(|shown| shown.bolt == Some(bolt))
        .map(SavedLook::of)
}

/// The bubbles' looks in `world` that outlive their bolts, and the sonar rings, as a save keeps
/// them (#3814).
pub(crate) fn saved(world: &mut World) -> (Vec<SavedLook>, Vec<SavedSonar>) {
    let mut shown = world.query::<&Shown>();
    let looks = shown
        .iter(world)
        .filter(|shown| shown.bolt.is_none())
        .map(SavedLook::of)
        .collect();
    let mut rings = world.query::<&Sonar>();
    let rings = rings
        .iter(world)
        .map(|sonar| SavedSonar {
            ring: sonar.ring,
            centre: sonar.centre,
        })
        .collect();
    (looks, rings)
}

/// Makes a loaded save's bubbles' looks that outlived their bolts again, each on its own, and its
/// sonar rings (#3814).
pub(crate) fn again(world: &mut World, looks: Vec<SavedLook>, rings: Vec<SavedSonar>) {
    for saved in looks {
        world.spawn((
            Alone,
            Transform::from_translation(saved.place),
            crate::fx::Resumes(saved),
            DespawnOnExit(AppState::InGame),
        ));
    }
    if rings.is_empty() {
        return;
    }
    if let Err(e) = world.run_system_cached_with(rings_again, rings) {
        warn!("bubbles: the saved sonar rings are not made: {e}");
    }
}

/// Makes the saved sonar rings `rings` again ([`again`]).
fn rings_again(
    In(rings): In<Vec<SavedSonar>>,
    mut commands: Commands,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    looks.make(data.as_deref(), &assets, &mut effects);
    for saved in rings {
        sonar(
            &mut commands,
            (saved.centre, saved.ring),
            looks.sonar.clone(),
            &mut meshes,
        );
    }
}

/// Gives each new bubble its look (`0x797B30`, `0x798380`): its bush and its glows' mesh.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn show(
    mut commands: Commands,
    (bolts, heights): (
        Query<
            (
                Entity,
                Option<&Bubble>,
                &Transform,
                Option<&crate::fx::Resumes<SavedLook>>,
                Has<Alone>,
            ),
            Or<(Added<Bubble>, Added<Alone>)>,
        >,
        Res<Heights>,
    ),
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (bolt, bubble, place, resumed, alone) in &bolts {
        // A loaded bubble's look goes on as it went, one whose bolt had ended on its own (#3814).
        let kept = match (resumed, bubble) {
            (Some(crate::fx::Resumes(saved)), _) => saved.clone(),
            (None, Some(bubble)) => SavedLook {
                look: BubbleTrap::new(bubble.seconds),
                place: place.translation,
                along: Vec3::new(bubble.direction.x, 0.0, bubble.direction.y).normalize_or(Vec3::X),
                popped: false,
                hero: bubble.hero,
            },
            (None, None) => continue,
        };
        looks.make(data.as_deref(), &assets, &mut effects);
        let (at, along) = (kept.place, kept.along);
        let texture = looks.textures[usize::from(!kept.hero)].clone();
        let model = looks.model.as_ref().map(|model| {
            let mut made = commands.spawn((
                model_looks::scene(&assets, model),
                bush_transform(at, along, 0.0),
                model_looks::Ordered::default(),
                // Its shadow on the ground under it (`0x406FF0`).
                crate::shadows::Casts,
                crate::shadows::Plane(ground_of(&heights, at)),
                DespawnOnExit(AppState::InGame),
            ));
            // The hero's bush in its texture, another caster's in its own (`0x401BD0`).
            if let Some(texture) = &texture {
                made.insert(Retexture::load(&assets, texture));
            }
            made.id()
        });
        let mesh = meshes.add(empty());
        // Its light on the ground at the height 0 under it (`0x5AA26E`), a child of its look,
        // whose place is the world's origin.
        let lamp = commands
            .spawn((
                Transform::from_translation(at.with_y(0.0)),
                Light(particles::BUBBLE_LIGHT),
            ))
            .id();
        let mut shown = commands.spawn((
            Transform::IDENTITY,
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        shown.add_child(lamp);
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
            bolt: (!alone).then_some(bolt),
            look: kept.look,
            model,
            lamp,
            place: at,
            along,
            popped: kept.popped,
            mesh,
            playing: false,
            hero: kept.hero,
        });
        if alone {
            commands.entity(bolt).despawn();
        } else if resumed.is_some() {
            commands
                .entity(bolt)
                .remove::<crate::fx::Resumes<SavedLook>>();
        }
    }
}

/// The ground's height under `at`, where its bush's shadow falls (`0x5A9A6D`), else a bolt's
/// height under it.
fn ground_of(heights: &Heights, at: Vec3) -> f32 {
    heights
        .at(at.x, at.z)
        .unwrap_or(at.y - crate::bolts::HEIGHT)
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
/// its growth and its shrinking, its burst where an arrow bursts it, its glows from its bush's
/// bone, with its sonar ring, its light under it, and its end.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn fly(
    mut commands: Commands,
    (time, heights): (Res<Time>, Res<Heights>),
    mut pops: MessageReader<Pops>,
    mut shown: Query<(Entity, &mut Shown)>,
    (bolts, mut bushes): (
        Query<&Transform, (With<Bubble>, Without<Shown>)>,
        Query<&mut Transform, (Without<Bubble>, Without<Shown>, Without<Light>)>,
    ),
    (mut lamps, children, bones, worn, mut planes): (
        Query<(&mut Transform, &mut Light), (Without<Bubble>, Without<Shown>)>,
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
        Query<&mut crate::shadows::Plane>,
    ),
    (looks, mut dice, mut meshes, orders): (
        Res<Looks>,
        ResMut<Dice>,
        ResMut<Assets<Mesh>>,
        crate::fx::Orders,
    ),
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
                        // An arrow burst it (`+0x74` bit 8): its glows from its bush's bone as its
                        // clip has it, else from its place, and its sonar ring 906.
                        let from = look
                            .model
                            .and_then(|model| {
                                crate::making::bone(
                                    model,
                                    &[particles::BUBBLE_BONE],
                                    &children,
                                    (&bones, &worn),
                                )
                            })
                            .unwrap_or(look.place);
                        debug!(
                            "bubbles: {bolt} bursts at {}, its glows from {from}",
                            look.place
                        );
                        look.look.burst(models(from), &mut dice.0);
                        let ground = ground_of(&heights, look.place);
                        sonar(
                            &mut commands,
                            (
                                Vec3::new(look.place.x, ground + GROUND_LIFT, look.place.z),
                                SonarRing::default(),
                            ),
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
                    if let Ok(mut plane) = planes.get_mut(model) {
                        plane.0 = ground_of(&heights, look.place);
                    }
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
        if let Ok((mut transform, mut light)) = lamps.get_mut(look.lamp) {
            transform.translation = look.place.with_y(0.0);
            light.0 = look.look.light();
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
            // At its object's place in the order, as the original draws the system (#2220).
            quads.in_order(orders.at(look.place), Vec3::ZERO);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// Makes the sonar ring `ring` at `centre` on the ground (`0x798460`).
fn sonar(
    commands: &mut Commands,
    (centre, ring): (Vec3, SonarRing),
    material: Option<Handle<Effect>>,
    meshes: &mut Assets<Mesh>,
) {
    let mesh = meshes.add(empty());
    let mut ring = commands.spawn((
        Sonar {
            ring,
            centre,
            mesh: mesh.clone(),
        },
        Transform::IDENTITY,
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
    // Its light of 200 on the ground at the height 0 under it, while it lasts.
    ring.with_child((
        Transform::from_translation(centre.with_y(0.0)),
        Light(particles::SONAR_LIGHT),
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
    (mut meshes, orders): (ResMut<Assets<Mesh>>, crate::fx::Orders),
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
            // At its object's place in the order, as the original draws the system (#2220).
            quads.in_order(orders.at(sonar.centre), Vec3::ZERO);
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
