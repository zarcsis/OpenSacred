//! The hero: a converted model (`opensacred-convert models`) that stands on the ground and runs or
//! walks where the left mouse button points, as in the original, playing its idle, walk and run
//! clips. The `R` key toggles walking and running; the original seems to toggle it from its
//! interface (the icon `sim_walk.tga`, **hypothesis**).

use std::f32::consts::FRAC_PI_4;
use std::time::Duration;

use bevy::gltf::Gltf;
use bevy::prelude::*;

use crate::frame::{Control, Step, Unpaused};
use crate::game::AppState;
use crate::terrain::{Heights, Level};
use crate::{iso, models, paths, shadows};

/// How close to its target the hero stops.
pub(crate) const ARRIVED: f32 = 4.0;
/// The blend between the idle and the run clips.
const BLEND: Duration = Duration::from_millis(150);

/// How the hero moves when it moves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Gait {
    Walk,
    #[default]
    Run,
}

/// The balance variable `GameSpeed` (`Balance.bin`, 1.25, docs/02 §6.3): the original scales
/// every creature's speed and every clip's rate by it (`0x4F75F0`, `0x413610`, #72), so that
/// the clips play a quarter faster than they were made and the steps still match the ground.
#[derive(Resource, Clone, Copy)]
pub struct GameSpeed(pub f32);

impl Default for GameSpeed {
    fn default() -> Self {
        Self(1.0)
    }
}

impl GameSpeed {
    /// Reads `GameSpeed` from `tables/balance.ron`.
    pub fn read(assets: &std::path::Path) -> anyhow::Result<Self> {
        let path = assets.join("tables").join(opensacred_data::tables::BALANCE);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;
        let balance: opensacred_data::tables::Balance =
            ron::from_str(&text).map_err(|e| anyhow::anyhow!("parsing {}: {e}", path.display()))?;
        let speed = balance
            .f32("GameSpeed")
            .ok_or_else(|| anyhow::anyhow!("no GameSpeed in {}", path.display()))?;
        anyhow::ensure!(
            speed.is_finite() && speed > 0.0,
            "GameSpeed {speed} in {}",
            path.display()
        );
        Ok(Self(speed))
    }
}

/// Which model and clips to show, relative to the asset directory.
#[derive(Resource)]
pub struct Hero {
    /// World units a second when walking and when running: the creature's speed pair in
    /// `Creature.pak` (70 and 180 for the heroes but two), in the units of the models
    /// (`iso::MODEL_SCALE`), times [`GameSpeed`] (`0x4F75F0`, #72; **hypothesis**: the unit of a
    /// second, and the hero's speed attribute 100).
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
    /// Where it walks: the ground or a building's inside layer.
    level: Level,
    /// Whether it came onto the stairs it stands on by climbing: it climbs again only from a
    /// stair it stepped onto from off the stairs (the client's flag `0x100`, `0x5FCCB0`).
    climbed: bool,
    /// The clip playing: standing, walking or running.
    moving: Option<Gait>,
    /// Put somewhere whose ground had not come in: it stands on the ground once it has.
    unplaced: bool,
}

impl Body {
    pub fn moving(&self) -> Option<Gait> {
        self.moving
    }

    pub fn level(&self) -> Level {
        self.level
    }

    /// Where it goes, if anywhere.
    pub fn target(&self) -> Option<Vec3> {
        self.target
    }

    /// Sends it to `target`.
    pub fn send(&mut self, target: Vec3) {
        self.target = Some(target);
    }

    /// Puts it at `position` on the ground, going nowhere; it stands on the ground there as soon
    /// as the ground's sector is in ([`run`]).
    pub fn teleport(&mut self, transform: &mut Transform, position: Vec3) {
        transform.translation = position;
        self.target = None;
        self.level = None;
        self.climbed = false;
        self.unplaced = true;
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
            level: None,
            climbed: false,
            moving: None,
            unplaced: false,
        },
        models::Ordered::default(),
        shadows::Casts,
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
    speed: Res<GameSpeed>,
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
            .repeat()
            .set_speed(speed.0);
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
    speed: Res<GameSpeed>,
) {
    for (mut transform, mut body) in &mut bodies {
        let mut moving = None;
        if body.unplaced
            && let Some(y) =
                heights.standing(transform.translation.x, transform.translation.z, None)
        {
            transform.translation.y = y;
            body.unplaced = false;
        }
        let here = transform.translation;
        let cell = paths::cell_of(here.x, here.z);
        // A hero on a building's mass is inside it, on its layer; one on an entrance goes in, one
        // on an exit out; one on a stair it stepped onto from off the stairs to the stair's
        // floor, at the cell the stair gives (`0x5FCCB0`).
        if body.level.is_none() && !heights.walkable(cell) {
            body.level = heights.level_of(cell);
        }
        body.level = heights.transition(cell, body.level);
        if !heights.on_stairs(cell, body.level) {
            body.climbed = false;
        }
        let mut here = here;
        if !body.climbed
            && let Some((to, level)) = heights.climb(cell, body.level)
        {
            let shift = iso::ground((to.0 - cell.0) as f32, (to.1 - cell.1) as f32);
            here += shift.with_y(0.0);
            if let Some(y) = heights.standing(here.x, here.z, level) {
                here.y = y;
            }
            transform.translation = here;
            body.level = level;
            body.climbed = true;
        }
        let level = body.level;
        if let Some(target) = body.target {
            let walkable = |cell| heights.walkable_on(cell, level);
            // Round what blocks the way and through the doors; straight on toward a target that
            // cannot be reached, up to what blocks it.
            // From where it stands it climbs no more: the stair it stands on, if any, it has
            // climbed already or arrived on.
            let toward = match paths::next(
                here,
                (level, false),
                target,
                |cell, (at, _)| heights.walkable_on(cell, at),
                |place| heights.neighbours(place),
                |(at, _)| at,
            ) {
                Some(paths::Step::Through((next, _))) => {
                    body.level = next;
                    here
                }
                Some(paths::Step::Toward(point)) => point,
                None => target,
            };
            let to = (toward - here).with_y(0.0);
            let distance = to.length();
            let left = (target - here).with_y(0.0).length();
            if left <= ARRIVED {
                body.target = None;
            } else if distance <= f32::EPSILON {
                // Through a door: on, the next frame.
                moving = body.moving;
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
                    // On its level's heights: a bridge's deck, a building's floor.
                    if let Some(y) = heights.standing(position.x, position.z, body.level) {
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
            .repeat()
            .set_speed(speed.0);
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

/// Despawns the hero of the game left, with its model; its clips go, and the next hero is not
/// ready until its model animates.
pub fn leave(mut commands: Commands, bodies: Query<Entity, With<Body>>, mut ready: ResMut<Ready>) {
    for body in &bodies {
        commands.entity(body).despawn();
    }
    commands.remove_resource::<Clips>();
    ready.0 = false;
}

/// The hero: its model and clips, its gait and its run, and the camera following it.
pub struct HeroPlugin;

impl Plugin for HeroPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Ready>()
            .add_systems(OnEnter(AppState::InGame), load)
            .add_systems(OnExit(AppState::InGame), leave)
            .add_systems(Update, (spawn, animate).chain().in_set(Step::HeroSpawn))
            .add_systems(
                Update,
                ((toggle, steer).in_set(Unpaused).in_set(Control), run)
                    .chain()
                    .in_set(Step::HeroMove),
            )
            .add_systems(Update, follow.in_set(Step::Follow));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_speed_is_the_balance_variable_of_its_name() {
        let dir =
            std::env::temp_dir().join(format!("opensacred-game-speed-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("tables")).unwrap();
        let variable = |name: &str, value: f32| opensacred_data::tables::BalanceVariable {
            name: name.to_owned(),
            offset: 0,
            value: opensacred_data::tables::BalanceValue::F32(value),
        };
        let balance = opensacred_data::tables::Balance {
            variables: vec![variable("GameSpeed", 1.25)],
            image: Vec::new(),
            rest: Vec::new(),
        };
        std::fs::write(
            dir.join("tables").join(opensacred_data::tables::BALANCE),
            ron::to_string(&balance).unwrap(),
        )
        .unwrap();
        let speed = GameSpeed::read(&dir).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(speed.0, 1.25);
    }
}
