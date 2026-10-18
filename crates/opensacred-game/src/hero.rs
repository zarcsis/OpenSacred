//! The hero: a converted model (`opensacred-convert models`) that stands on the ground and runs or
//! walks where the left mouse button points, as in the original, playing its idle, walk and run
//! clips. The `R` key toggles walking and running; the original seems to toggle it from its
//! interface (the icon `sim_walk.tga`, **hypothesis**).

use std::f32::consts::FRAC_PI_4;
use std::time::Duration;

use bevy::gltf::Gltf;
use bevy::prelude::*;

use crate::terrain::Heights;
use crate::{iso, models, paths};

/// How close to its target the hero stops.
const ARRIVED: f32 = 4.0;
/// The blend between the idle and the run clips.
const BLEND: Duration = Duration::from_millis(150);

/// How the hero moves when it moves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Gait {
    Walk,
    #[default]
    Run,
}

/// Which model and clips to show, relative to the asset directory.
#[derive(Resource)]
pub struct Hero {
    /// World units a second when walking and when running: the creature's speed pair in
    /// `Creature.pak` (70 and 180 for the heroes but two; **hypothesis**: the pair is walk and
    /// run, in the units of the models, `iso::MODEL_SCALE`).
    pub speeds: [f32; 2],
    pub model: String,
    pub idle: String,
    pub walk: String,
    pub run: String,
    pub position: Vec3,
    /// Where to go first, if anywhere.
    pub target: Option<Vec3>,
    pub gait: Gait,
    gltf: Option<Handle<Gltf>>,
    spawned: bool,
}

impl Hero {
    pub fn new(model: String, [idle, walk, run]: [String; 3], position: Vec3) -> Self {
        Self {
            speeds: [70.0, 180.0],
            model,
            idle,
            walk,
            run,
            position,
            target: None,
            gait: Gait::Run,
            gltf: None,
            spawned: false,
        }
    }

    fn speed(&self) -> f32 {
        match self.gait {
            Gait::Walk => self.speeds[0],
            Gait::Run => self.speeds[1],
        }
    }
}

/// Whether the hero is in place and animating: the screenshot waits for it.
#[derive(Resource, Default)]
pub struct Ready(pub bool);

/// The root of the hero's model and where it goes.
#[derive(Component)]
pub struct Body {
    target: Option<Vec3>,
    /// The clip playing: standing, walking or running.
    moving: Option<Gait>,
}

impl Body {
    pub fn moving(&self) -> Option<Gait> {
        self.moving
    }
}

/// The animation graph of the hero's clips, given to the model's player once it exists.
#[derive(Resource)]
pub struct Clips {
    graph: Handle<AnimationGraph>,
    idle: AnimationNodeIndex,
    /// The walk and the run clips and their nodes.
    moving: [(Handle<AnimationClip>, AnimationNodeIndex); 2],
    /// The entity of the model's animation player.
    pub player: Option<Entity>,
}

impl Clips {
    pub fn of(&self, gait: Gait) -> &(Handle<AnimationClip>, AnimationNodeIndex) {
        match gait {
            Gait::Walk => &self.moving[0],
            Gait::Run => &self.moving[1],
        }
    }

    fn node(&self, moving: Option<Gait>) -> AnimationNodeIndex {
        moving.map_or(self.idle, |gait| self.of(gait).1)
    }
}

pub fn load(mut hero: ResMut<Hero>, assets: Res<AssetServer>) {
    hero.gltf = Some(assets.load(hero.model.clone()));
}

/// Spawns the model once its glTF is loaded.
pub fn spawn(
    mut commands: Commands,
    mut hero: ResMut<Hero>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    if hero.spawned {
        return;
    }
    let Some(gltf) = hero.gltf.as_ref().and_then(|h| gltfs.get(h)) else {
        return;
    };
    hero.spawned = true;
    let Some(scene) = gltf.scenes.first().cloned() else {
        error!("{}: no scene", hero.model);
        return;
    };
    let clip = |name: &str| {
        let clip = gltf.named_animations.get(name).cloned();
        if clip.is_none() {
            error!("{}: no clip {name}", hero.model);
        }
        clip
    };
    if let (Some(idle), Some(walk), Some(run)) =
        (clip(&hero.idle), clip(&hero.walk), clip(&hero.run))
    {
        let (graph, nodes) = AnimationGraph::from_clips([idle, walk.clone(), run.clone()]);
        commands.insert_resource(Clips {
            graph: graphs.add(graph),
            idle: nodes[0],
            moving: [(walk, nodes[1]), (run, nodes[2])],
            player: None,
        });
    }
    commands.spawn((
        WorldAssetRoot(scene),
        Transform::from_translation(hero.position)
            .with_rotation(facing(Vec3::new(1.0, 0.0, 1.0)))
            .with_scale(Vec3::splat(iso::MODEL_SCALE)),
        Body {
            target: hero.target,
            moving: None,
        },
        models::Ordered::default(),
    ));
}

/// The rotation that turns a model facing +X in its bind pose, as `SERAPHIM.GRN` does (docs/02
/// §4.5), toward `direction` on the ground.
pub fn facing(direction: Vec3) -> Quat {
    if direction.xz().length_squared() < f32::EPSILON {
        return Quat::from_rotation_y(-FRAC_PI_4);
    }
    Quat::from_rotation_y((-direction.z).atan2(direction.x))
}

/// Starts the idle clip on the model's animation player when the scene creates it; the players
/// of other models (the NPCs') are not the hero's.
pub fn animate(
    mut commands: Commands,
    clips: Option<ResMut<Clips>>,
    mut players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
    parents: Query<&ChildOf>,
    bodies: Query<(), With<Body>>,
    mut ready: ResMut<Ready>,
) {
    let Some(mut clips) = clips else {
        return;
    };
    for (entity, mut player) in &mut players {
        if !parents
            .iter_ancestors(entity)
            .any(|ancestor| bodies.contains(ancestor))
        {
            continue;
        }
        let mut transitions = AnimationTransitions::new();
        transitions
            .play(&mut player, clips.idle, Duration::ZERO)
            .repeat();
        commands
            .entity(entity)
            .insert((AnimationGraphHandle(clips.graph.clone()), transitions));
        clips.player = Some(entity);
        ready.0 = true;
    }
}

/// Toggles walking and running.
pub fn toggle(keys: Res<ButtonInput<KeyCode>>, mut hero: ResMut<Hero>) {
    if keys.just_pressed(KeyCode::KeyR) {
        hero.gait = match hero.gait {
            Gait::Walk => Gait::Run,
            Gait::Run => Gait::Walk,
        };
    }
}

/// Points the hero where the left mouse button is held: the ground under the cursor, taken at
/// the hero's height.
pub fn steer(
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform)>,
    mut bodies: Query<(&Transform, &mut Body)>,
) {
    if !buttons.pressed(MouseButton::Left) {
        return;
    }
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let (camera, view) = *camera;
    let Ok(ray) = camera.viewport_to_world(view, cursor) else {
        return;
    };
    for (transform, mut body) in &mut bodies {
        let height = transform.translation.y;
        let along = (height - ray.origin.y) / ray.direction.y;
        body.target = Some(ray.origin + ray.direction.as_vec3() * along);
    }
}

/// Moves the hero toward its target over the ground, turning to face it, and switches between
/// the idle, the walk and the run clips.
pub fn run(
    time: Res<Time>,
    hero: Res<Hero>,
    heights: Res<Heights>,
    clips: Option<Res<Clips>>,
    mut bodies: Query<(&mut Transform, &mut Body)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for (mut transform, mut body) in &mut bodies {
        let mut moving = None;
        if let Some(target) = body.target {
            let here = transform.translation;
            let walkable = |cell| heights.walkable(cell);
            // Round what blocks the way; straight on toward a target that cannot be reached,
            // up to what blocks it.
            let toward = paths::next(here, target, walkable).unwrap_or(target);
            let to = (toward - here).with_y(0.0);
            let distance = to.length();
            let left = (target - here).with_y(0.0).length();
            if left <= ARRIVED || distance <= f32::EPSILON {
                body.target = None;
            } else {
                let step = (hero.speed() * time.delta_secs()).min(distance);
                let mut position = here + to / distance * step;
                // A step into a cell it may not walk on, out of one it may, is not taken.
                let (from, into) = (
                    paths::cell_of(here.x, here.z),
                    paths::cell_of(position.x, position.z),
                );
                if into != from && !walkable(into) && walkable(from) {
                    body.target = None;
                } else {
                    if let Some(y) = heights.at(position.x, position.z) {
                        position.y = y;
                    }
                    transform.translation = position;
                    transform.rotation = facing(to);
                    moving = Some(hero.gait);
                }
            }
        }
        if moving == body.moving {
            continue;
        }
        // The state changes only with the clip: before the model's player exists, the next
        // frame tries again.
        let Some(clips) = clips.as_ref() else {
            continue;
        };
        let Some((mut player, mut transitions)) =
            clips.player.and_then(|e| players.get_mut(e).ok())
        else {
            continue;
        };
        transitions
            .play(&mut player, clips.node(moving), BLEND)
            .repeat();
        body.moving = moving;
    }
}

/// Keeps the camera on the hero.
pub fn follow(
    bodies: Query<&Transform, (With<Body>, Without<Camera>)>,
    mut cameras: Query<&mut Transform, With<Camera>>,
) {
    let Some(hero) = bodies.iter().next() else {
        return;
    };
    for mut camera in &mut cameras {
        *camera = iso::view_of(hero.translation);
    }
}
