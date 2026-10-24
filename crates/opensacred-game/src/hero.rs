//! The hero: a converted model (`opensacred-convert models`) that stands on the ground and runs or
//! walks where the left mouse button points, as in the original, playing its idle, walk and run
//! clips. The `R` key toggles walking and running; the original seems to toggle it from its
//! interface (the icon `sim_walk.tga`, **hypothesis**).

use std::f32::consts::FRAC_PI_4;
use std::time::Duration;

use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;

use crate::frame::{Control, Step, Unpaused};
use crate::game::AppState;
use crate::npcs::Dies;
use crate::sectors::Streamer;
use crate::terrain::{Heights, LANDING_REACH, Landing, Level};
use crate::{files, iso, models, paths, shadows};

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
        let text = files::read_to_string(&path)
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

/// The clip of an art's action by its name (`making`, #509): its style, the model's clip of the
/// slot that the class's executor plays for what the hero holds, none for one of its attacks'
/// clips; the hit points of the clip and its rate as a factor of the attack's clip speed.
#[derive(Clone, Debug, PartialEq)]
pub struct ArtClipName {
    pub style: opensacred_rules::arts::Style,
    pub clip: Option<String>,
    pub points: Vec<f32>,
    pub rate: f32,
    /// Whether it plays at its rate itself, not times the attack's clip speed (the kicks').
    pub fixed: bool,
    /// Whether its strikes' factors are halved, the hero holding two weapons (`0x4EA3F0`).
    pub halved: bool,
}

/// An art's action's clip in the hero's graph: its node and clip, none for one of its attacks'.
#[derive(Clone, Debug)]
pub struct ArtNode {
    pub style: opensacred_rules::arts::Style,
    pub node: Option<AnimationNodeIndex>,
    pub clip: Option<Handle<AnimationClip>>,
    pub points: Vec<f32>,
    pub rate: f32,
    pub fixed: bool,
    pub halved: bool,
}

/// Which model and clips to show, relative to the asset directory.
#[derive(Resource)]
pub struct Hero {
    /// World units a second when walking and when running: the creature's speed pair in
    /// `Creature.pak` (70 and 180 for the heroes but two) times its pace's factor ([`Self::pace`])
    /// times [`GameSpeed`] (`0x4F75F0`, #72, #599; **hypothesis**: the unit of a second).
    pub speeds: [f32; 2],
    /// The factor of its walk and run speeds and of their clips' rate, its pace's
    /// (`opensacred_rules::fight::Combat::pace_factor`): 1.32 for the Seraphim with her blade.
    pub pace: f32,
    pub model: String,
    pub idle: String,
    pub walk: String,
    pub run: String,
    /// The model's fight stance for what it holds, if it has one (`motions::Slots::stance`).
    pub stance: Option<String>,
    /// The model's attack clips for what it holds (`motions::Slots::attacks`) and the fractions
    /// of each at which it hits (`attack`).
    pub attacks: Vec<(String, [f32; 2])>,
    /// The clips of its arts' actions for what it holds (`making`, #509).
    pub arts: Vec<ArtClipName>,
    /// The items in its hands, its template's (`npcs::Npcs::hero_gear`, #488), and their
    /// equipment once loading.
    pub held: Vec<crate::npcs::Held>,
    equipment: Option<crate::npcs::Equipment>,
    /// Its attack, defence, damage and resistances, by its type's record (`fight::combat`).
    pub combat: Option<crate::fight::Combat>,
    /// Its health, full, and its regeneration, by its type's record (`fight::hero_health`).
    pub vitals: Option<(
        crate::fight::Health,
        crate::fight::Regenerates,
        crate::fight::Base,
        crate::fight::Holding,
    )>,
    /// The model's dying clips, the slots 173 and 174 (`npcs`).
    pub dying: Vec<String>,
    pub position: Vec3,
    /// Where to go first, if anywhere.
    pub target: Option<Vec3>,
    pub gait: Gait,
    /// Its size by its type and what it holds, of which and a creature's their reach is the
    /// mean (`opensacred_rules::minds::size`, `0x4DBC60`, #642).
    pub size: u32,
    /// Its type, the class's (`game::Class::creature`): whose voice it speaks with (`voices`).
    pub creature: u32,
    /// What it holds, for its weapons' sounds (`sfx`).
    pub armed: crate::sfx::Armed,
    /// The item in its hand that its hurled blades strike with (`making::Blade`).
    pub blade: Option<crate::making::Blade>,
    gltf: Option<Handle<Gltf>>,
    spawned: bool,
    /// Whether [`spawn`] gives the body that stands its new model rather than a body of its own
    /// (`forms`).
    worn: bool,
}

impl Hero {
    pub fn new(model: String, [idle, walk, run]: [String; 3], position: Vec3) -> Self {
        Self {
            speeds: [70.0, 180.0],
            pace: 1.0,
            model,
            idle,
            walk,
            run,
            stance: None,
            attacks: Vec::new(),
            arts: Vec::new(),
            held: Vec::new(),
            equipment: None,
            combat: None,
            vitals: None,
            dying: Vec::new(),
            position,
            target: None,
            gait: Gait::Run,
            size: opensacred_rules::minds::SIZE,
            creature: 1,
            armed: crate::sfx::Armed::default(),
            blade: None,
            gltf: None,
            spawned: false,
            worn: false,
        }
    }

    /// Puts on `form` (`forms`): its type, size, model and clips, which [`spawn`] gives the
    /// hero's body in place of its own, and what it holds again on the new model where the form
    /// shows it.
    pub fn wear(&mut self, form: &crate::forms::Form, assets: &AssetServer) {
        self.creature = form.creature;
        self.size = form.size;
        self.model.clone_from(&form.model);
        [self.idle, self.walk, self.run] = form.clips.moving.clone();
        self.stance.clone_from(&form.clips.stance);
        self.attacks.clone_from(&form.clips.attacks);
        self.arts.clone_from(&form.clips.arts);
        self.dying.clone_from(&form.clips.dying);
        self.gltf = Some(assets.load(self.model.clone()));
        self.equipment = (form.armed && !self.held.is_empty())
            .then(|| crate::npcs::equipment(assets, &self.held));
        self.spawned = false;
        self.worn = true;
    }

    fn speed(&self) -> f32 {
        match self.gait {
            Gait::Walk => self.speeds[0],
            Gait::Run => self.speeds[1],
        }
    }
}

/// Whether the hero is in place and animating, or in place where its model has no clips to play
/// ([`still`]): the screenshot waits for it.
#[derive(Resource, Default)]
pub struct Ready(pub bool);

/// The hero's size (`opensacred_rules::minds::size`), of which and a creature's their reach is
/// the mean (#642).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size(pub u32);

/// The root of the hero's model and where it goes.
#[derive(Component)]
pub struct Body {
    /// The order it takes at the next tick of the world, where to go and on what level
    /// (`0x60F130`, #640).
    order: Option<(Vec3, Option<Level>)>,
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
    /// Arrived where it went, the ticks of the world it still runs in place, its moving action
    /// left only at the second tick after (#640).
    stopping: Option<u8>,
    /// Put somewhere whose ground had not come in: it stands on its level once it has.
    unplaced: bool,
    /// The cell a command put it on, where it lands once the sectors about it are in, and how
    /// ([`land`]).
    landing: Option<((i32, i32), Arrival)>,
    /// The cell a command stood it on at a floor it named: it keeps that level while it stays
    /// on the cell, whatever the cell's mass, entrance or exit would make of it ([`run`]).
    pinned: Option<(i32, i32)>,
}

/// A hero that flies, for a test (the command `fly`, #526): it goes straight where it is sent,
/// through walls and buildings and over water, on the ground's level, and goes nowhere else on its
/// own, through no door and up no stair ([`run`]).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flies;

/// How a hero that a command put on a cell lands there ([`land`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrival {
    /// Where it can stand and walk on (`sector`, #458): on the cell, on the ground or in a room
    /// of a building's floor, else on the cell of the ground nearest it.
    Nearest,
    /// On the cell itself (`cell`, #522): on the ground, where the original's rule for a hero
    /// on a building's mass then takes it onto floor 1 (`Heights::level_of`), or on the floor
    /// it names, 0 the ground even within a building's walls.
    On(Option<u8>),
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

    /// Whether it has an order still to take at the next tick of the world.
    pub fn ordered(&self) -> bool {
        self.order.is_some()
    }

    /// The level of where it goes, when a click chose one.
    pub fn onto(&self) -> Option<Level> {
        self.onto
    }

    /// Orders it to `target`, on whatever level the way reaches it, from the next tick of the
    /// world on ([`take_orders`]).
    pub fn send(&mut self, target: Vec3) {
        self.order = Some((target, None));
    }

    /// Orders it to `target` on `level`, from the next tick of the world on.
    pub fn send_onto(&mut self, target: Vec3, level: Level) {
        self.order = Some((target, Some(level)));
    }

    /// Stops it where it stands, with any order it has yet to take.
    pub fn stop(&mut self) {
        self.order = None;
        self.target = None;
        self.onto = None;
        self.stopping = None;
    }

    /// It has arrived, or can go no farther: its clip goes on to the second tick after (#640).
    fn arrives(&mut self) {
        self.target = None;
        if self.moving.is_some() {
            self.stopping = Some(STOPS);
        }
    }

    /// Puts it at `position` on the ground, going nowhere; it stands on the ground there as soon
    /// as the ground's sector is in ([`run`]).
    pub fn teleport(&mut self, transform: &mut Transform, position: Vec3) {
        transform.translation = position;
        self.order = None;
        self.target = None;
        self.onto = None;
        self.stopping = None;
        self.level = None;
        self.climbed = false;
        self.unplaced = true;
        self.landing = None;
        self.pinned = None;
    }

    /// Puts it at `position` by a command (`cell`, `sector`), going nowhere: it lands on the
    /// cell there as `arrival` says once the sectors about it are in ([`land`]).
    pub fn put(&mut self, transform: &mut Transform, position: Vec3, arrival: Arrival) {
        self.teleport(transform, position);
        self.landing = Some((paths::cell_of(position.x, position.z), arrival));
    }

    /// Whether a command put it somewhere where it has not landed yet.
    pub fn landing(&self) -> bool {
        self.landing.is_some()
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
    /// The arts' actions' clips (`making`, #509).
    pub arts: Vec<ArtNode>,
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
    if !hero.held.is_empty() {
        hero.equipment = Some(crate::npcs::equipment(&assets, &hero.held));
    }
}

/// Spawns the model once its glTF is loaded, or gives the body that stands a form's new model
/// (`Hero::wear`).
pub fn spawn(
    mut commands: Commands,
    mut hero: ResMut<Hero>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    bodies: Query<Entity, With<Body>>,
    wielded: Query<(Entity, &crate::models::OrderedWith), With<crate::npcs::Wielded>>,
) {
    if hero.spawned {
        return;
    }
    // A form's model goes on a frame after what the old one held is gone: the spawner would
    // put an item's instance back under a holder that the old model took away with it.
    if hero.worn {
        let mut stripped = false;
        for (item, holder) in &wielded {
            if bodies.contains(holder.0) {
                commands.entity(item).despawn();
                stripped = true;
            }
        }
        if stripped {
            return;
        }
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
        let arts: Vec<_> = hero
            .arts
            .iter()
            .filter_map(|art| {
                let clip = match &art.clip {
                    Some(name) => Some(clip(name)?),
                    None => None,
                };
                Some((art, clip))
            })
            .collect();
        let mut clips = vec![idle, walk.clone(), run.clone()];
        let stances = usize::from(stance.is_some());
        clips.extend(stance);
        clips.extend(attacks.iter().map(|(clip, _)| clip.clone()));
        let strikes = attacks.len();
        clips.extend(dying);
        let dyings = clips.len() - 3 - stances - strikes;
        clips.extend(arts.iter().filter_map(|(_, clip)| clip.clone()));
        let (graph, nodes) = AnimationGraph::from_clips(clips);
        let mut art_nodes = nodes[3 + stances + strikes + dyings..].iter();
        let art_nodes = arts
            .into_iter()
            .map(|(art, clip)| ArtNode {
                style: art.style,
                node: clip.as_ref().and_then(|_| art_nodes.next().copied()),
                clip,
                points: art.points.clone(),
                rate: art.rate,
                fixed: art.fixed,
                halved: art.halved,
            })
            .collect();
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
            dying: nodes[3 + stances + strikes..3 + stances + strikes + dyings].to_vec(),
            arts: art_nodes,
            player: None,
        });
    }
    if hero.worn {
        hero.worn = false;
        for body in &bodies {
            let mut body = commands.entity(body);
            body.insert((WorldAssetRoot(scene.clone()), Size(hero.size)));
            if let Some(equipment) = hero.equipment.take() {
                body.insert(equipment);
            }
        }
        return;
    }
    let mut body = commands.spawn((
        WorldAssetRoot(scene),
        Transform::from_translation(hero.position)
            .with_rotation(facing(Vec3::new(1.0, 0.0, 1.0)))
            .with_scale(Vec3::splat(iso::MODEL_SCALE)),
        // The single player's hero, of id 1: it stops the others' steps (#157).
        crate::crowd::InTheWay::of(0, 0, None),
        Body {
            order: None,
            target: hero.target,
            onto: None,
            level: None,
            climbed: false,
            moving: None,
            stopping: None,
            unplaced: false,
            landing: None,
            pinned: None,
        },
        models::Ordered::default(),
        shadows::Casts,
        Size(hero.size),
    ));
    if let Some(combat) = hero.combat {
        body.insert(combat);
    }
    if let Some(vitals) = hero.vitals.clone() {
        body.insert(vitals);
    }
    if let Some(equipment) = hero.equipment.take() {
        body.insert(equipment);
    }
    if let Some(blade) = hero.blade {
        body.insert(blade);
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

/// Makes the hero ready once its scene has spawned where [`animate`] has nothing to play: its
/// model has no animation player, as a glTF without animations has none (`--hero`, #538), or not
/// the hero's clips.
fn still(
    spawned: On<WorldInstanceReady>,
    clips: Option<Res<Clips>>,
    bodies: Query<(), With<Body>>,
    children: Query<&Children>,
    players: Query<(), With<AnimationPlayer>>,
    mut ready: ResMut<Ready>,
) {
    if !bodies.contains(spawned.entity) {
        return;
    }
    let animated = clips.is_some()
        && children
            .iter_descendants(spawned.entity)
            .any(|entity| players.contains(entity));
    if !animated {
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

/// Points the hero where the left mouse button is pressed: the cell under the cursor, on its
/// level, as the client picks it (`pick`); where it picks none, the point of the cursor's line of
/// sight at the hero's height. A press orders it once; held, it orders it again at every tick
/// once it has been held longer than 0.4 s (`0x60F130`, #640).
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
pub fn steer(
    time: Res<Time>,
    mut held: Local<f32>,
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
        *held = 0.0;
        return;
    }
    let pressed = buttons.just_pressed(MouseButton::Left);
    *held = if pressed {
        0.0
    } else {
        *held + time.delta_secs()
    };
    if !pressed && *held <= HELD {
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

/// How long the left mouse button is held before the hero follows the cursor at every tick
/// (`0x60F130`).
const HELD: f32 = 0.4;

/// How many ticks of the world the hero's moving action outlasts its arrival: the original's
/// left it at the second tick after the last step, its run clip playing in place until then
/// (#640).
const STOPS: u8 = 2;

/// Gives the hero the order it has yet to take, at a tick of the world (`0x60F130`, which takes
/// the mouse's orders, and the creatures' update, which takes the events of walking somewhere,
/// on the tick alone; #640): a click's, a held button's, a command's, an attack's approach; and
/// ends its moving action at the second tick after it arrived.
pub fn take_orders(mut bodies: Query<&mut Body>) {
    for mut body in &mut bodies {
        if let Some((target, onto)) = body.order.take() {
            body.target = Some(target);
            body.onto = onto;
            body.stopping = None;
        } else if let Some(left) = body.stopping {
            body.stopping = left.checked_sub(1).filter(|&left| left > 0);
        }
    }
}

/// Lands a hero that a command put on a cell, once the sectors about it are in, as its
/// [`Arrival`] says: on the cell itself, at the floor it names (#522), or where it can stand and
/// walk (#458, [`Heights::landing`]): on the cell, on the ground or in a room of a building's
/// floor, else on the cell of the ground nearest it, which the log tells.
pub fn land(
    heights: Res<Heights>,
    streamer: Option<Res<Streamer>>,
    mut bodies: Query<(&mut Transform, &mut Body)>,
) {
    for (mut transform, mut body) in &mut bodies {
        let Some((cell, arrival)) = body.landing else {
            continue;
        };
        if streamer
            .as_ref()
            .is_some_and(|streamer| !streamer.settled(transform.translation))
        {
            continue;
        }
        body.landing = None;
        let floor = match arrival {
            Arrival::Nearest => None,
            Arrival::On(floor) => Some(floor),
        };
        if let Some(floor) = floor {
            let level = match floor {
                None | Some(0) => None,
                Some(floor) => {
                    let level = heights
                        .building_on_floor(cell, usize::from(floor))
                        .and_then(|building| heights.floor_level(building, usize::from(floor)));
                    if level.is_none() {
                        warn!(
                            "hero: no building's floor {floor} holds cell {},{}; the hero stands \
                             on the ground",
                            cell.0, cell.1
                        );
                    }
                    level
                }
            };
            body.level = level;
            body.pinned = floor.is_some().then_some(cell);
            // As though it had climbed onto a stair it stands on, so that it stays on its floor.
            body.climbed = body.pinned.is_some();
            let position = transform.translation;
            if let Some(y) = heights.standing(position.x, position.z, level) {
                transform.translation.y = y;
                body.unplaced = false;
            }
            continue;
        }
        let (mut position, level) = match heights.landing(cell) {
            Some(Landing::On(level)) => (transform.translation, level),
            Some(Landing::Near(near)) => {
                let (x, y) = near;
                match heights.building(transform.translation.x, transform.translation.z) {
                    Some(((ax, ay), _)) => info!(
                        "hero: cell {},{} lies in the building at {ax},{ay}, outside its rooms; \
                         the hero stands at {x},{y} on the ground",
                        cell.0, cell.1
                    ),
                    None => info!(
                        "hero: the hero cannot walk on from cell {},{}; it stands at {x},{y} on \
                         the ground",
                        cell.0, cell.1
                    ),
                }
                (iso::ground(x as f32 + 0.5, y as f32 + 0.5), None)
            }
            None => {
                warn!(
                    "hero: no cell within {LANDING_REACH} of {},{} may be walked on; the hero \
                     stays there",
                    cell.0, cell.1
                );
                continue;
            }
        };
        if let Some(y) = heights.standing(position.x, position.z, level) {
            position.y = y;
            body.unplaced = false;
        }
        transform.translation = position;
        body.level = level;
        body.target = None;
        body.onto = None;
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
        Has<crate::making::Playing>,
        Has<Dies>,
        Has<Flies>,
    )>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    speed: Res<GameSpeed>,
) {
    for (me, mut transform, mut body, striking, making, dying, flies) in &mut bodies {
        let mut moving = None;
        if body.unplaced
            && let Some(y) =
                heights.standing(transform.translation.x, transform.translation.z, body.level)
        {
            transform.translation.y = y;
            body.unplaced = false;
        }
        let here = transform.translation;
        let cell = paths::cell_of(here.x, here.z);
        if body.pinned.is_some_and(|pinned| pinned != cell) {
            body.pinned = None;
        }
        // A hero on a building's mass is inside it, on its layer; one on an entrance goes in, one
        // on an exit out; one on a stair it stepped onto from off the stairs to the stair's
        // floor, at the cell the stair gives (`0x5FCCB0`). One that a command put there lands
        // as [`land`] says, and one it stood on a floor it named stays there.
        if flies {
            // Straight on, on the ground's level, at the ground's height where it is in.
            body.level = None;
            body.pinned = None;
            body.climbed = true;
            if let Some(target) = body.target {
                let to = (target - here).with_y(0.0);
                let left = to.length();
                if left <= ARRIVED {
                    body.arrives();
                } else {
                    let step = (hero.speed() * time.delta_secs()).min(left);
                    let mut position = here + to / left * step;
                    if let Some(y) = heights.standing(position.x, position.z, None) {
                        position.y = y;
                    }
                    transform.translation = position;
                    transform.rotation = facing(to);
                    moving = Some(hero.gait);
                }
            }
        } else {
            if body.pinned.is_none() {
                if body.level.is_none() && !heights.walkable(cell) && body.landing.is_none() {
                    body.level = heights.level_of(cell);
                }
                body.level = heights.transition(cell, body.level);
            }
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
                if sent_to_stair(body.target, cell) {
                    body.target = None;
                }
            }
            let level = body.level;
            if let Some(target) = body.target {
                // A cell it may step into: walkable, no object and no hostile creature in it.
                let free =
                    |cell, at| heights.walkable_on(cell, at) && !crowd.stops(cell, at, me, true);
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
                    body.arrives();
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
                        body.arrives();
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
        }
        // The state changes only with the clip: before the model's player exists, the next
        // frame tries again. A strike plays its own clip (`attack`), an art its own (`making`), and
        // a fall its own ([`fall`]).
        let Some(clips) = clips.as_ref() else {
            continue;
        };
        if striking || making || dying {
            body.moving = None;
            body.stopping = None;
            continue;
        }
        // Arrived, it runs in place until its moving action ends (#640).
        if moving.is_none() && body.stopping.is_some() {
            moving = body.moving;
        }
        // Standing, in its stance while the last look found a hostile within 350 units (the
        // client's bit `0x8000`, `0x547070`; 1,000 with a bow, which the hero does not hold yet).
        let fight = nearest.found && nearest.distance < opensacred_rules::fight::IN_FIGHT_WITHIN;
        let node = clips.node(moving, fight);
        let Some((mut player, mut transitions)) =
            clips.player.and_then(|e| players.get_mut(e).ok())
        else {
            continue;
        };
        if moving == body.moving && transitions.get_main_animation() == Some(node) {
            continue;
        }
        // A walk's and a run's clip play at its pace too (`0x413610`, #599).
        let rate = if moving.is_some() { hero.pace } else { 1.0 };
        transitions
            .play(&mut player, node, BLEND)
            .repeat()
            .set_speed(speed.0 * rate);
        body.moving = moving;
    }
}

/// Whether a stair the hero climbs at `cell` is where it was sent, `target`: it has arrived, and
/// stands where the stair put it, rather than walking back onto the stair, which would take it
/// away again, round and round (#373).
fn sent_to_stair(target: Option<Vec3>, cell: (i32, i32)) -> bool {
    target.is_some_and(|t| paths::cell_of(t.x, t.z) == cell)
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
                .remove::<(
                    crate::attack::Attack,
                    crate::attack::Striking,
                    crate::making::Making,
                    crate::making::Playing,
                )>()
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
/// ready until its model animates, or has spawned where it has nothing to play.
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
            .add_observer(still)
            .add_systems(OnEnter(AppState::InGame), load)
            .add_systems(OnExit(AppState::InGame), leave)
            .add_systems(Update, (spawn, animate).chain().in_set(Step::HeroSpawn))
            .add_systems(
                Update,
                (
                    (toggle, steer).in_set(Unpaused).in_set(Control),
                    take_orders.run_if(crate::frame::ticked),
                    land,
                    run,
                    fall,
                )
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
            ..Default::default()
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
    fn a_stair_it_was_sent_to_ends_its_way() {
        let stair = (1105, 991);
        let centre = |(x, y): (i32, i32)| {
            let ground = iso::ground(x as f32 + 0.5, y as f32 + 0.5);
            Vec3::new(ground.x, 0.0, ground.z)
        };
        assert_eq!(paths::cell_of(centre(stair).x, centre(stair).z), stair);
        assert!(sent_to_stair(Some(centre(stair)), stair));
        assert!(!sent_to_stair(Some(centre((1110, 999))), stair));
        assert!(!sent_to_stair(None, stair));
    }

    /// Whether a hero whose model's scene holds an animation player (`player`), where the hero has
    /// its clips (`clips`), is ready once the scene has spawned, before [`animate`] plays a clip.
    fn ready_once_spawned(player: bool, clips: bool) -> bool {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::world_serialization::WorldSerializationPlugin,
        ))
        .register_type::<ChildOf>()
        .register_type::<Children>()
        .register_type::<AnimationPlayer>()
        .init_resource::<Ready>()
        .add_observer(still);
        if clips {
            app.insert_resource(Clips {
                graph: Handle::default(),
                idle: AnimationNodeIndex::new(0),
                stance: None,
                moving: [
                    (Handle::default(), AnimationNodeIndex::new(1)),
                    (Handle::default(), AnimationNodeIndex::new(2)),
                ],
                attacks: Vec::new(),
                arts: Vec::new(),
                dying: Vec::new(),
                player: None,
            });
        }
        app.update();
        let mut scene = World::new();
        let root = scene.spawn_empty().id();
        let mut armature = scene.spawn(ChildOf(root));
        if player {
            armature.insert(AnimationPlayer::default());
        }
        let scene = app
            .world_mut()
            .resource_mut::<Assets<bevy::world_serialization::WorldAsset>>()
            .add(bevy::world_serialization::WorldAsset::new(scene));
        app.world_mut().spawn((
            WorldAssetRoot(scene),
            Body {
                order: None,
                target: None,
                onto: None,
                level: None,
                climbed: false,
                moving: None,
                stopping: None,
                unplaced: false,
                landing: None,
                pinned: None,
            },
        ));
        app.update();
        app.world().resource::<Ready>().0
    }

    #[test]
    fn a_hero_with_nothing_to_play_is_ready_once_its_scene_spawns() {
        // A model without animations (#538), or without the hero's clips.
        assert!(ready_once_spawned(false, false));
        assert!(ready_once_spawned(true, false));
        assert!(ready_once_spawned(false, true));
        // One that animates waits for its player to play the idle clip.
        assert!(!ready_once_spawned(true, true));
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
            arts: Vec::new(),
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
