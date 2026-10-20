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
use crate::npcs::Dies;
use crate::terrain::{Heights, Level};
use crate::{iso, models, paths, shadows};

/// How close to its target the hero stops.
pub(crate) const ARRIVED: f32 = 4.0;
/// The blend between the idle and the run clips.
const BLEND: Duration = Duration::from_millis(150);
/// How near its nearest hostile puts the hero in its fight stance, in world units (`0x547070`).
const HERO_STANCE_WITHIN: f32 = 350.0;

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
    /// The model's fight stance for bare hands, if it has one (`motions::Slots::stance`).
    pub stance: Option<String>,
    /// The model's attack clips for bare hands (`motions::Slots::attacks`) and the fractions of
    /// each at which it hits (`attack`).
    pub attacks: Vec<(String, [f32; 2])>,
    /// Its attack, defence, damage and resistances, by its type's record (`fight::combat`).
    pub combat: Option<crate::fight::Combat>,
    /// Its health, full, and its regeneration, by its type's record (`fight::hero_health`).
    pub vitals: Option<(
        crate::fight::Health,
        crate::fight::Regenerates,
        crate::fight::Base,
    )>,
    /// The model's dying clips, the slots 173 and 174 (`npcs`).
    pub dying: Vec<String>,
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
            stance: None,
            attacks: Vec::new(),
            combat: None,
            vitals: None,
            dying: Vec::new(),
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
    /// The level the target lies on, when a click chose one (`0x617030`, docs/re/movement.md,
    /// "The cursor's point"): the way ends on it.
    onto: Option<Level>,
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

    /// The level of where it goes, when a click chose one.
    pub fn onto(&self) -> Option<Level> {
        self.onto
    }

    /// Sends it to `target`, on whatever level the way reaches it.
    pub fn send(&mut self, target: Vec3) {
        self.target = Some(target);
        self.onto = None;
    }

    /// Sends it to `target` on `level`.
    pub fn send_onto(&mut self, target: Vec3, level: Level) {
        self.target = Some(target);
        self.onto = Some(level);
    }

    /// Stops it where it stands.
    pub fn stop(&mut self) {
        self.target = None;
        self.onto = None;
    }

    /// Puts it at `position` on the ground, going nowhere; it stands on the ground there as soon
    /// as the ground's sector is in ([`run`]).
    pub fn teleport(&mut self, transform: &mut Transform, position: Vec3) {
        transform.translation = position;
        self.target = None;
        self.onto = None;
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
    /// The fight stance, where the model has one.
    stance: Option<AnimationNodeIndex>,
    /// The walk and the run clips and their nodes.
    moving: [(Handle<AnimationClip>, AnimationNodeIndex); 2],
    /// The attack clips' nodes, the clips and the fractions at which they hit.
    pub attacks: Vec<(AnimationNodeIndex, Handle<AnimationClip>, [f32; 2])>,
    /// The dying clips' nodes.
    dying: Vec<AnimationNodeIndex>,
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

    /// The clip of the hero moving at a gait, or standing: in its stance while `fight`, where
    /// its model has one, else idle.
    fn node(&self, moving: Option<Gait>, fight: bool) -> AnimationNodeIndex {
        match (moving, self.stance) {
            (Some(gait), _) => self.of(gait).1,
            (None, Some(stance)) if fight => stance,
            (None, _) => self.idle,
        }
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
        let stance = hero.stance.as_ref().and_then(|name| clip(name));
        let attacks: Vec<_> = hero
            .attacks
            .iter()
            .filter_map(|(name, fractions)| Some((clip(name)?, *fractions)))
            .collect();
        let dying: Vec<_> = hero.dying.iter().filter_map(|name| clip(name)).collect();
        let mut clips = vec![idle, walk.clone(), run.clone()];
        let stances = usize::from(stance.is_some());
        clips.extend(stance);
        clips.extend(attacks.iter().map(|(clip, _)| clip.clone()));
        let strikes = attacks.len();
        clips.extend(dying);
        let (graph, nodes) = AnimationGraph::from_clips(clips);
        commands.insert_resource(Clips {
            graph: graphs.add(graph),
            idle: nodes[0],
            stance: (stances > 0).then(|| nodes[3]),
            moving: [(walk, nodes[1]), (run, nodes[2])],
            attacks: attacks
                .into_iter()
                .zip(&nodes[3 + stances..])
                .map(|((clip, fractions), &node)| (node, clip, fractions))
                .collect(),
            dying: nodes[3 + stances + strikes..].to_vec(),
            player: None,
        });
    }
    let mut body = commands.spawn((
        WorldAssetRoot(scene),
        Transform::from_translation(hero.position)
            .with_rotation(facing(Vec3::new(1.0, 0.0, 1.0)))
            .with_scale(Vec3::splat(iso::MODEL_SCALE)),
        // The single player's hero, of id 1: it stops the others' steps (#157).
        crate::crowd::InTheWay::of(1, 0, 0, None),
        Body {
            target: hero.target,
            onto: None,
            level: None,
            climbed: false,
            moving: None,
            unplaced: false,
        },
        models::Ordered::default(),
        shadows::Casts,
    ));
    if let Some(combat) = hero.combat {
        body.insert(combat);
    }
    if let Some(vitals) = hero.vitals {
        body.insert(vitals);
    }
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

/// Points the hero where the left mouse button is held: the cell under the cursor, on its level,
/// as the client picks it (`pick`); where it picks none, the point of the cursor's line of sight
/// at the hero's height.
pub fn steer(
    buttons: Res<ButtonInput<MouseButton>>,
    pointer: Res<crate::hud::Pointer>,
    camera: Single<(&Camera, &GlobalTransform)>,
    heights: Res<Heights>,
    buildings: Res<crate::buildings::Buildings>,
    aim: Res<crate::attack::Aim>,
    mut bodies: Query<(&Transform, &mut Body), Without<Dies>>,
) {
    // A press that went to an attack does not steer (`attack::aim`).
    if !buttons.pressed(MouseButton::Left) || aim.0 {
        return;
    }
    let Some(cursor) = pointer.0 else {
        return;
    };
    let (camera, view) = *camera;
    let Ok(ray) = camera.viewport_to_world(view, cursor) else {
        return;
    };
    for (transform, mut body) in &mut bodies {
        let height = transform.translation.y;
        match crate::pick::pick(ray, &heights, &buildings, (height, body.level)) {
            Some(picked) => body.send_onto(picked.point, picked.level),
            None => {
                let along = (height - ray.origin.y) / ray.direction.y;
                body.send(ray.origin + ray.direction.as_vec3() * along);
            }
        }
    }
}

/// Moves the hero toward its target over the ground, turning to face it, and switches between
/// the idle, the walk and the run clips.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources one by one, its queries with their filters"
)]
pub fn run(
    time: Res<Time>,
    hero: Res<Hero>,
    heights: Res<Heights>,
    clips: Option<Res<Clips>>,
    crowd: Res<crate::crowd::Crowd>,
    nearest: Res<crate::crowd::Nearest>,
    mut bodies: Query<(
        Entity,
        &mut Transform,
        &mut Body,
        Has<crate::attack::Striking>,
        Has<Dies>,
    )>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    speed: Res<GameSpeed>,
) {
    for (me, mut transform, mut body, striking, dying) in &mut bodies {
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
            // A cell it may step into: walkable, no object and no hostile creature in it.
            let free = |cell, at| heights.walkable_on(cell, at) && !crowd.stops(cell, at, me, true);
            let walkable = |cell| free(cell, level);
            // Round what blocks the way and through the doors; straight on toward a target that
            // cannot be reached, up to what blocks it.
            // From where it stands it climbs no more: the stair it stands on, if any, it has
            // climbed already or arrived on.
            let onto = body.onto;
            let (toward, reachable) = match paths::next(
                here,
                (level, false),
                target,
                onto,
                |cell, (at, _)| free(cell, at),
                |place| {
                    let mut next = heights.neighbours(place);
                    next.retain(|&(cell, (at, _))| !crowd.stops(cell, at, me, true));
                    next
                },
                |(at, _)| at,
            ) {
                Some(paths::Step::Through((next, _))) => {
                    body.level = next;
                    (here, true)
                }
                Some(paths::Step::Toward(point)) => (point, true),
                None => (target, false),
            };
            let to = (toward - here).with_y(0.0);
            let distance = to.length();
            let left = (target - here).with_y(0.0).length();
            // There on the target's level, or as near as it gets to one it cannot reach.
            let on_level = !reachable || onto.is_none_or(|onto| onto == body.level);
            if left <= ARRIVED && on_level {
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
        // The state changes only with the clip: before the model's player exists, the next
        // frame tries again. A strike plays its own clip (`attack`), and a fall its own ([`fall`]).
        let Some(clips) = clips.as_ref() else {
            continue;
        };
        if striking || dying {
            body.moving = None;
            continue;
        }
        // Standing, in its stance while the last look found a hostile within 350 units (the
        // client's bit `0x8000`, `0x547070`; 1,000 with a bow, which the hero does not hold yet).
        let fight = nearest.found && nearest.distance < HERO_STANCE_WITHIN;
        let node = clips.node(moving, fight);
        let Some((mut player, mut transitions)) =
            clips.player.and_then(|e| players.get_mut(e).ok())
        else {
            continue;
        };
        if moving == body.moving && transitions.get_main_animation() == Some(node) {
            continue;
        }
        transitions
            .play(&mut player, node, BLEND)
            .repeat()
            .set_speed(speed.0);
        body.moving = moving;
    }
}

/// How long the hero lies dead before it rises again, in seconds.
const LIES: f32 = 5.0;

/// How long the fallen hero has lain.
#[derive(Component)]
pub struct Fallen(f32);

/// The hero at health 0 (`fight`, the AI's state 9): it stops, its attack ends, it plays one of
/// its dying clips once and lies there; after five seconds it rises at the game's start with its
/// health full (docs/decisions/2026-10-20-hero-health.md: the original's resurrection is not
/// read).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
pub fn fall(
    mut commands: Commands,
    time: Res<Time>,
    hero: Res<Hero>,
    clips: Option<Res<Clips>>,
    speed: Res<GameSpeed>,
    mut bodies: Query<(
        Entity,
        &mut Transform,
        &mut Body,
        &Dies,
        Option<&mut Fallen>,
        Option<&mut crate::fight::Health>,
    )>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    let Some(clips) = clips else {
        return;
    };
    for (entity, mut transform, mut body, dies, fallen, health) in &mut bodies {
        let player = clips.player.and_then(|player| players.get_mut(player).ok());
        let Some(mut fallen) = fallen else {
            body.stop();
            commands
                .entity(entity)
                .remove::<(crate::attack::Attack, crate::attack::Striking)>()
                .insert(Fallen(0.0));
            let index = usize::try_from(dies.choice).unwrap_or(0) % clips.dying.len().max(1);
            if let (Some((mut player, mut transitions)), Some(&node)) =
                (player, clips.dying.get(index))
            {
                transitions
                    .play(&mut player, node, BLEND)
                    .set_speed(speed.0);
            }
            info!("hero: the hero falls");
            continue;
        };
        fallen.0 += time.delta_secs();
        if fallen.0 < LIES {
            continue;
        }
        body.teleport(&mut transform, hero.position);
        if let Some(mut health) = health {
            health.current = health.most;
        }
        commands.entity(entity).remove::<(Dies, Fallen)>();
        if let Some((mut player, mut transitions)) = player {
            transitions
                .play(&mut player, clips.idle, BLEND)
                .repeat()
                .set_speed(speed.0);
        }
        body.moving = None;
        info!("hero: the hero rises again");
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
                ((toggle, steer).in_set(Unpaused).in_set(Control), run, fall)
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

    #[test]
    fn the_hero_stands_in_its_stance_only_when_near_a_hostile() {
        let node = AnimationNodeIndex::new;
        let clips = |stance| Clips {
            graph: Handle::default(),
            idle: node(1),
            stance,
            moving: [(Handle::default(), node(2)), (Handle::default(), node(3))],
            attacks: Vec::new(),
            dying: Vec::new(),
            player: None,
        };
        let with = clips(Some(node(4)));
        assert_eq!(with.node(None, true), node(4));
        assert_eq!(with.node(None, false), node(1));
        // Moving, its gait's clip; a model without a stance, its idle.
        assert_eq!(with.node(Some(Gait::Run), true), node(3));
        assert_eq!(clips(None).node(None, true), node(1));
    }
}
