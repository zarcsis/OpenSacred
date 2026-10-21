//! The creatures' life and their way of fighting (docs/re/ai.md): each is dormant until its own
//! timer or the hero's look wakes it, and dormant again farther than 1,500 from the hero; a
//! hostile creature in the hero's reach steps about by the idle action (`0x54EA30`), a group's
//! members together; a peaceful animal awake flees, steps away and strolls about home, and a
//! guard keeps its post, and a townsperson rolls what it does next at each of the hero's looks,
//! by their behaviours (`0x4EF150`); a creature that shares its cell with another parts from it,
//! and one that walks into the hero's way steps aside. An awake hostile creature that the test of
//! the levels sends at the hero, or that has the hero within its reach, comes at it from up to
//! 800 along a straight line of walkable cells and keeps the band of its reach, where it strikes;
//! wounded it flees, and too far from home it goes back ([`pursue`],
//! docs/decisions/2026-10-20-pursuit.md); one with a ranged weapon keeps 300 to 800 from the hero
//! and shoots (docs/decisions/2026-10-21-archers.md); one that the test holds back goes dormant
//! again and steps about as one beyond it (docs/decisions/2026-10-20-idle.md). The decisions are
//! the rules' (`opensacred_rules::minds`); this module keeps the minds as components, reads the
//! world for them and carries them out: the walks, the strikes, the shots, the doors, the lines
//! and the turns.

use bevy::prelude::*;
pub use opensacred_rules::minds::of_type;
use opensacred_rules::minds::{
    self, BEHAVES_WITHIN, Child, Decision, Evening, Pursues, Pursuit, RANGE, Senses, Spot, Walking,
    Why, child, daemon_fear, daemon_near, evening_way, follow, runs_from, steps_off, townsperson,
    trader_for,
};

use crate::crowd::{Crowd, InTheWay};
use crate::daynight::{Clock, Phase};
use crate::frame::Step;
use crate::hero::Body;
use crate::lines::Says;
use crate::npcs::{Door, Plays, PutAt, Strikes, Swing, Talks, Walk};
use crate::paths;
use crate::spawning::Rng;
use crate::terrain::Heights;

/// How near a trader a townsperson that visits it stops (`0x4EE660`, `+0x176`).
const AT_THE_TRADER: f32 = 150.0;
/// How near a child at play the other idle children follow it (`0x4F2AD7`, `0x4EE820`).
const PLAYMATES: f32 = 600.0;
/// The cells' flag of a settlement's ground (`+0x1E` bit 1, docs/02 §5.2).
const SETTLEMENT: u8 = 0x2;
/// How near a hostile of the hero's puts a townsperson in danger (`0x4F0BDA`).
const DANGER: f32 = 800.0;
/// How far a townsperson seeks a door, and the cells about a door it goes in to and out to, in
/// how many tries (`0x4EE300`, `0x4EEFA0`, `0x4F1AB9`).
const DOOR_WITHIN: f32 = 1_500.0;
const INDOORS: (i32, i32) = (2, 9);
const OUTDOORS: (i32, i32) = (-4, 5);
const DOOR_TRIES: usize = 15;
/// The shade of the children's lines, green (**hypothesis**), and of the townsfolk's fear of the
/// Daemon, red (`0xFFFF3030`).
const CHILD_GREEN: Color = Color::srgb(0.19, 1.0, 0.19);
const FEAR_RED: Color = Color::srgb(1.0, 0.19, 0.19);

/// Whether the hero is the Daemon, its type 9, before whom the townsfolk are in danger and the
/// children flee (docs/re/ai.md, "Danger and the Daemon").
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct Daemon(pub bool);

/// A creature's mind (`opensacred_rules::minds::Mind`), and the creatures it goes to.
#[derive(Component, Clone, Debug, Deref, DerefMut)]
pub struct Mind {
    /// Its home, its flags word, its kind and speeds, its level, and its thought and its fight.
    #[deref]
    pub thinks: minds::Mind,
    /// The creature it went to last, which it does not go to next (`+0x24D`).
    last: Option<Entity>,
    /// The partner it follows (`+0x251`, the flag 4), and whether it runs after it.
    partner: Option<Entity>,
    runs_after: bool,
    /// The trader it walks to, at which it talks.
    visiting: Option<Entity>,
    /// The door it goes through, which it closes behind it (`+0x124`).
    door: Option<Entity>,
}

impl Mind {
    /// The mind of a creature made at `home` with the flags word `flags`, by an `Enter` function
    /// or not, by `CreateNPC` or a fill (`opensacred_rules::minds::Mind::new`).
    pub fn new(
        home: Vec3,
        flags: u32,
        made: (bool, bool),
        group: Option<u32>,
        speeds: [f32; 2],
        kind: (u32, u8),
    ) -> Self {
        Self {
            thinks: minds::Mind::new(spot(home), flags, made, group, speeds, kind),
            last: None,
            partner: None,
            runs_after: false,
            visiting: None,
            door: None,
        }
    }
}

/// The rules' point of the ground under world point `point`.
fn spot(point: Vec3) -> Spot {
    Spot::new(point.x, point.z)
}

/// The world point over the rules' point of the ground `spot`, at height `y`.
fn point(spot: Spot, y: f32) -> Vec3 {
    Vec3::new(spot.x, y, spot.y)
}

/// The game's generator for the creatures' chances, the hero's look, and whether it looked this
/// frame.
#[derive(Resource)]
struct Minds {
    rng: Rng,
    look: minds::Look,
    looked: bool,
}

impl Default for Minds {
    fn default() -> Self {
        Self {
            rng: Rng::of(0, 0, 0),
            look: minds::Look::default(),
            looked: false,
        }
    }
}

/// A step to `target` on level `level` at `speed`.
fn walk(target: Vec3, speed: f32, way: &InTheWay) -> Walk {
    Walk::to(target, way.level, speed)
}

/// Runs the creatures' timers, the hero's look every 0.3 s, and the awake creatures' thought
/// (`opensacred_rules::minds::Mind::think`), and walks them where it sends them.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn think(
    mut commands: Commands,
    time: Res<Time>,
    mut minds: ResMut<Minds>,
    heights: Res<Heights>,
    balance: Res<FightRules>,
    experience: Res<crate::fight::Experience>,
    hero: Query<(&Transform, &Body)>,
    mut creatures: Query<(Entity, &Transform, &InTheWay, &mut Mind, Has<Walk>), Without<Body>>,
) {
    let Some((hero, body)) = hero.iter().next() else {
        return;
    };
    let hero = hero.translation;
    let dt = time.delta_secs();
    let minds = &mut *minds;
    let look = minds.look.looks(dt);
    minds.looked = look;
    let side = crate::iso::CELL * 64.0;
    let sector = |p: Vec3| ((p.x / side).floor(), (p.z / side).floor());
    let (sx, sy) = sector(hero);
    let hero_moves = body.moving().is_some();
    // The steps taken this frame: a creature, its target, and its group's step to share; the
    // groups whose alarm goes.
    let mut steps: Vec<(Entity, Vec3)> = Vec::new();
    let mut alarms: Vec<u32> = Vec::new();
    let mut grouped: Vec<(u32, Vec3, Entity)> = Vec::new();
    for (entity, transform, way, mut mind, walking) in &mut creatures {
        let here = transform.translation;
        let (x, y) = sector(here);
        let senses = Senses {
            dt,
            seen: look && (x - sx).abs() <= 1.0 && (y - sy).abs() <= 1.0,
            here: spot(here),
            hero: spot(hero),
            hero_moves,
            takes_on: crate::fight::takes_on(mind.level, experience.level, balance.experience),
            walking,
        };
        // A straight line of walkable cells to the hero.
        let clear = || {
            paths::clear(here.xz(), hero.xz(), |cell| {
                heights.walkable_on(cell, way.level)
            })
        };
        let thought = mind.think(&senses, clear, &mut minds.rng);
        if let Some(home) = thought.home {
            steps.push((entity, point(home, here.y)));
        }
        if thought.attacks {
            let distance = (here - hero).xz().length();
            debug!(
                "ai: {entity} {:?} the hero from {distance:.0}",
                Why::Attacks
            );
            if let (true, Some(group)) = (thought.alarm, mind.group) {
                alarms.push(group);
            }
        }
        if let Some(step) = thought.step {
            let target = point(step, here.y);
            steps.push((entity, target));
            if let (Some(group), true) = (mind.group, mind.enter) {
                grouped.push((group, target - here, entity));
            }
        }
    }
    // A group's other members step by the same step, each within 50 of it.
    for (group, step, leader) in grouped {
        for (entity, transform, _, mut mind, walking) in &mut creatures {
            if entity == leader || mind.group != Some(group) || walking {
                continue;
            }
            let here = transform.translation;
            let target = mind.fellow_steps(spot(here), spot(step), &mut minds.rng);
            steps.push((entity, point(target, here.y)));
        }
    }
    for (entity, target) in steps {
        if let Ok((_, _, way, mind, _)) = creatures.get(entity) {
            commands
                .entity(entity)
                .insert(walk(target, mind.speed, way));
        }
    }
    // The alarm sends a group's idle hostile members at the hero too.
    for group in alarms {
        for (_, _, _, mut mind, _) in &mut creatures {
            mind.hears_alarm(group);
        }
    }
}

/// The balance's values of the fight that the AI takes: the threshold of the test of the levels
/// and its full share (`UnterExpAnteil`, `UnterExpVoll`).
#[derive(Resource, Clone, Copy, Debug)]
pub struct FightRules {
    pub experience: (f32, f32),
}

impl Default for FightRules {
    /// The original's values of bronze, where the balance lacks them (docs/re/fight.md).
    fn default() -> Self {
        Self {
            experience: (0.85, 1.0),
        }
    }
}

/// Each creature that fights the hero keeps the band of its reach, flees wounded and goes home
/// from too far, as the rules decide (`opensacred_rules::minds::Mind::pursue`, `0x4E0300`): it
/// comes on, after the hero as it moves, steps back, flees and goes home at its walk or run, and
/// in the band stands turned to the hero, where it strikes (#280). One with a ranged weapon keeps
/// 300 to 800 from the hero (`0x4DBFC0`, #391): it backs off, comes on, goes round where no
/// straight line of walkable cells leads to the hero, and shoots, standing turned to the hero.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn pursue(
    mut commands: Commands,
    mut minds: ResMut<Minds>,
    heights: Res<Heights>,
    hero: Query<
        (Entity, &Transform, Has<crate::attack::Striking>),
        (With<Body>, Without<crate::npcs::Dies>),
    >,
    mut creatures: Query<
        (
            Entity,
            &mut Transform,
            &InTheWay,
            &mut Mind,
            Option<&Walk>,
            Has<Strikes>,
            Option<&crate::fight::Health>,
        ),
        Without<Body>,
    >,
) {
    let Some((the_hero, hero, hero_strikes)) = hero.iter().next() else {
        return;
    };
    let hero = hero.translation;
    for (entity, mut transform, way, mut mind, walk, striking, health) in &mut creatures {
        let here = transform.translation;
        let sight = Pursuit {
            here: spot(here),
            hero: spot(hero),
            walk: walk.map(|walk| Walking {
                target: spot(walk.target()),
                comes_on: walk.comes_on(),
                runs: walk.runs(),
            }),
            striking,
            health: health.map(|health| (health.current, health.most)),
            hero_strikes,
        };
        // A straight line of walkable cells to the hero.
        let clear = || {
            paths::clear(here.xz(), hero.xz(), |cell| {
                heights.walkable_on(cell, way.level)
            })
        };
        let Some(pursues) = mind.pursue(&sight, clear, &mut minds.rng) else {
            continue;
        };
        match pursues {
            Pursues::GivesUp => {
                let d = (hero - here).xz().length();
                debug!("ai: {entity} {:?} at {d:.0}", Why::GivesUp);
            }
            Pursues::Flees(to) => {
                debug!("ai: {entity} {:?} the hero", Why::Flees);
                commands
                    .entity(entity)
                    .insert(Walk::run(point(to, here.y), way.level, mind.run));
            }
            Pursues::ComesOn { runs, within } => {
                commands
                    .entity(entity)
                    .insert(come_on(&mind, hero, way, runs, within));
            }
            Pursues::StepsBack(to) => {
                commands
                    .entity(entity)
                    .insert(Walk::to(point(to, here.y), way.level, mind.speed));
            }
            Pursues::GoesBack => {
                debug!("ai: {entity} {:?} from the fight", Why::GoesBack);
                let home = point(mind.home, here.y);
                commands
                    .entity(entity)
                    .insert(Walk::to(home, way.level, mind.speed));
            }
            // In the band, turned to the hero, it strikes.
            Pursues::Strikes(choice) => {
                transform.rotation = crate::hero::facing(hero - here);
                if let Some(choice) = choice {
                    commands
                        .entity(entity)
                        .insert(Strikes::at(the_hero, choice));
                }
            }
            Pursues::BacksOff { to, runs } => {
                let d = (hero - here).xz().length();
                debug!("ai: {entity} backs off from the hero at {d:.0}, running {runs}");
                let to = point(to, here.y);
                commands.entity(entity).insert(if runs {
                    Walk::run(to, way.level, mind.run)
                } else {
                    Walk::to(to, way.level, mind.speed)
                });
            }
            Pursues::GoesRound { to, runs, within } => {
                debug!("ai: {entity} goes round the hero, no straight line to it");
                let to = point(to, here.y);
                let walk = if runs {
                    Walk::run(to, way.level, mind.run)
                } else {
                    Walk::to(to, way.level, mind.speed)
                };
                commands.entity(entity).insert(walk.within(within));
            }
            // It stops where it stands, turns to the hero and shoots.
            Pursues::Shoots { choice, chain } => {
                let d = (hero - here).xz().length();
                debug!("ai: {entity} shoots at the hero from {d:.0}, its chain {chain:.1}");
                transform.rotation = crate::hero::facing(hero - here);
                commands
                    .entity(entity)
                    .remove::<Walk>()
                    .insert(Strikes::shot(the_hero, choice, chain));
            }
        }
    }
}

/// A walk or a run at the hero at `hero`, stopping within `within` of it, in the band.
fn come_on(mind: &Mind, hero: Vec3, way: &InTheWay, runs: bool, within: f32) -> Walk {
    let walk = if runs {
        Walk::run(hero, way.level, mind.run)
    } else {
        Walk::to(hero, way.level, mind.speed)
    };
    walk.within(within).coming_on()
}

/// The world point at the centre of global cell `cell`, on the ground plane.
fn centre((x, y): (i32, i32)) -> Option<Vec3> {
    let (x, y) = (
        f32::from(i16::try_from(x).ok()?),
        f32::from(i16::try_from(y).ok()?),
    );
    Some(crate::iso::ground(x + 0.5, y + 0.5))
}

/// A whole number at random from `low` to `high`, both included.
fn among(low: i32, high: i32, rng: &mut Rng) -> i32 {
    let span = u32::try_from(high - low + 1).unwrap_or(1);
    low + i32::try_from(rng.below(span)).unwrap_or(0)
}

/// The cell a townsperson goes in to by the door at `door` (`0x4EEFA0`): of 15 cells 2 to 9
/// cells from the door's along each axis, either way, that a building's ground floor holds and
/// that one may stand on, the farthest from the door; and its level.
fn indoors(door: Vec3, heights: &Heights, rng: &mut Rng) -> Option<(Vec3, crate::terrain::Level)> {
    let (dx, dy) = paths::cell_of(door.x, door.z);
    (0..DOOR_TRIES)
        .filter_map(|_| {
            let mut offset = || {
                let d = among(INDOORS.0, INDOORS.1, rng);
                if rng.below(2) == 0 { -d } else { d }
            };
            let cell = (dx + offset(), dy + offset());
            let level = heights.level_of(cell)?;
            heights
                .walkable_on(cell, Some(level))
                .then_some((centre(cell)?, Some(level)))
        })
        .max_by(|a, b| {
            let (da, db) = ((a.0 - door).length_squared(), (b.0 - door).length_squared());
            da.total_cmp(&db)
        })
}

/// The cell a townsperson goes out to by the door at `door` (`0x4F1AB9`): the first of 15 cells
/// -4 to 5 cells from the door's along each axis that no building covers and that one may walk
/// on.
fn outdoors(door: Vec3, heights: &Heights, rng: &mut Rng) -> Option<Vec3> {
    let (dx, dy) = paths::cell_of(door.x, door.z);
    (0..DOOR_TRIES).find_map(|_| {
        let cell = (
            dx + among(OUTDOORS.0, OUTDOORS.1, rng),
            dy + among(OUTDOORS.0, OUTDOORS.1, rng),
        );
        let at = centre(cell)?;
        (heights.building(at.x, at.z).is_none() && heights.walkable_on(cell, None)).then_some(at)
    })
}

/// Where a townsperson at `here` in danger runs from its threat, the nearest of `hostiles` within
/// 800 (`0x4F0BDA`), as the rules send it (`opensacred_rules::minds::runs_from`); `None` out of
/// danger.
fn danger(here: Vec3, hostiles: &[Vec3], rng: &mut Rng) -> Option<Vec3> {
    let threat = hostiles
        .iter()
        .copied()
        .filter(|at| (*at - here).xz().length() < DANGER)
        .min_by(|a, b| {
            (*a - here)
                .length_squared()
                .total_cmp(&(*b - here).length_squared())
        })?;
    Some(point(runs_from(spot(here), spot(threat), rng), here.y))
}

/// Sends the creatures not hostile that walk within 91 of the hero, at its look, to a point
/// within 200 of themselves that they reach, the first of 10 tries
/// (`opensacred_rules::minds::Mind::steps_aside`, `0x547070`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn aside(
    mut commands: Commands,
    mut minds: ResMut<Minds>,
    heights: Res<Heights>,
    crowd: Res<Crowd>,
    hero: Query<&Transform, With<Body>>,
    creatures: Query<(Entity, &Transform, &InTheWay, &Mind), (With<Walk>, Without<Body>)>,
) {
    let (Some(hero), true) = (hero.iter().next(), minds.looked) else {
        return;
    };
    let hero = hero.translation;
    for (entity, transform, way, mind) in &creatures {
        let here = transform.translation;
        let from = (paths::cell_of(here.x, here.z), (way.level, false));
        let reaches = |to: Spot| {
            paths::path_onto(
                from,
                paths::cell_of(to.x, to.y),
                |_| true,
                |place| {
                    let mut next = heights.neighbours(place);
                    next.retain(|&(cell, (at, _))| !crowd.stops(cell, at, entity, false));
                    next
                },
            )
            .is_some()
        };
        let Some(target) = mind.steps_aside(spot(here), spot(hero), &mut minds.rng, reaches) else {
            continue;
        };
        let target = point(target, here.y);
        debug!(
            "ai: {entity} {:?} from {:.0},{:.0} to {:.0},{:.0}",
            Why::StepsAside,
            here.x,
            here.z,
            target.x,
            target.z
        );
        commands
            .entity(entity)
            .insert(Walk::to(target, way.level, mind.speed));
    }
}

/// The peaceful creatures' behaviours while they stand awake, at every frame (`0x4EF150`); a
/// creature farther than 1,500 from the hero goes dormant again, a townsperson farther than 600
/// times the camera's zoom. A townsperson decides at each of the hero's looks, which wake it, as
/// the original's goes dormant after most of its rolls and decides again when woken. An idle
/// creature, or a townsperson between its rolls, that shares its cell with another parts from it
/// by chance (`0x544890`); an animal awake strolls at once. The decisions are the rules'
/// (`opensacred_rules::minds`); this system finds the creatures, the doors and the cells they
/// take, and walks, turns and stops them.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn behave(
    mut commands: Commands,
    time: Res<Time>,
    mut minds: ResMut<Minds>,
    crowd: Res<Crowd>,
    heights: Res<Heights>,
    hero: Query<(Entity, &Transform), With<Body>>,
    cameras: Query<&Projection, With<Camera3d>>,
    doors: Query<(Entity, &Transform), (With<Door>, Without<Mind>)>,
    clock: Res<Clock>,
    daemon: Res<Daemon>,
    mut creatures: Query<(Entity, &mut Transform, &InTheWay, &mut Mind, Has<Walk>), Without<Body>>,
) {
    let Some((the_hero, hero)) = hero.iter().next() else {
        return;
    };
    let hero = hero.translation;
    let dt = time.delta_secs();
    // Whether the hero stands on a settlement's ground, where the children gather about it.
    let in_settlement = heights
        .flags(paths::cell_of(hero.x, hero.z))
        .is_some_and(|flags| flags & SETTLEMENT != 0);
    // The children at play this frame, whom the others follow.
    let mut playing: Vec<(Entity, Vec3)> = Vec::new();
    // The camera's zoom, its scale.
    let zoom = cameras
        .iter()
        .find_map(|projection| match projection {
            Projection::Orthographic(ortho) => Some(ortho.scale),
            _ => None,
        })
        .unwrap_or(1.0);
    let side = crate::iso::CELL * 64.0;
    let sector = |p: Vec3| ((p.x / side).floor(), (p.z / side).floor());
    // What the animals fear: every creature but the animals, and the hero.
    let feared: Vec<(Entity, Vec3)> = creatures
        .iter()
        .filter(|(_, _, _, mind, _)| !matches!(mind.class, 4 | 6))
        .map(|(entity, transform, ..)| (entity, transform.translation))
        .chain(std::iter::once((Entity::PLACEHOLDER, hero)))
        .collect();
    // What the townsfolk visit and take for partners: the farm and wild animals and the humans
    // of class 3, at the hero's looks.
    let [animals, humans, traders]: [Vec<(Entity, Vec3)>; 3] = if minds.looked {
        let of = |wanted: fn(&Mind) -> bool| -> Vec<(Entity, Vec3)> {
            creatures
                .iter()
                .filter(|(_, _, _, mind, _)| wanted(mind))
                .map(|(entity, transform, ..)| (entity, transform.translation))
                .collect()
        };
        [
            of(|mind| mind.class == 6 && mind.is_animal()),
            of(|mind| mind.class == 3),
            of(|mind| mind.trader),
        ]
    } else {
        [Vec::new(), Vec::new(), Vec::new()]
    };
    // The partners' places, whether they walk, and whether they are traders.
    let followed: std::collections::HashMap<Entity, (Vec3, bool, bool)> = creatures
        .iter()
        .filter_map(|(_, _, _, mind, _)| mind.partner)
        .filter_map(|partner| {
            let (_, transform, _, mind, walking) = creatures.get(partner).ok()?;
            Some((partner, (transform.translation, walking, mind.trader)))
        })
        .collect();
    // The hero's hostiles, which put the townsfolk near them in danger: the creatures of flag
    // bit 0 that its look takes, in the 3 × 3 sectors about it within its range.
    let (hx, hy) = sector(hero);
    let hostiles: Vec<Vec3> = creatures
        .iter()
        .filter(|(_, _, _, mind, _)| mind.flags & crate::crowd::HOSTILE != 0)
        .map(|(_, transform, ..)| transform.translation)
        .filter(|&at| {
            let (x, y) = sector(at);
            (x - hx).abs() <= 1.0 && (y - hy).abs() <= 1.0 && (at - hero).xz().length() < RANGE
        })
        .collect();
    // The doors, at the hero's looks.
    let doors: Vec<(Entity, Vec3)> = if minds.looked {
        doors
            .iter()
            .map(|(entity, transform)| (entity, transform.translation))
            .collect()
    } else {
        Vec::new()
    };
    // The doors to open and to close this frame.
    let mut swings: Vec<(Entity, bool)> = Vec::new();
    // At dusk and by night the townsfolk take the evening's way (`0x41BB50`, `0x41BAB0`).
    let evening = matches!(clock.phase(), Phase::Dusk | Phase::Night);
    let mut walks = Vec::new();
    let mut turns: Vec<(Entity, Vec3)> = Vec::new();
    let mut stops: Vec<Entity> = Vec::new();
    for (entity, transform, way, mut mind, walking) in &mut creatures {
        if walking {
            continue;
        }
        let here = transform.translation;
        // Through its door, in or out, it closes it behind it (the sub-states 2 and 3).
        if let Some(door) = mind.door.take() {
            swings.push((door, false));
        }
        // At the trader it walked to, it talks once (the sub-states 12 and 13), and stays, where
        // the original's goes into a house (docs/decisions/2026-10-20-townsfolk.md).
        if mind.visiting.take().is_some() {
            commands.entity(entity).insert(Talks);
            continue;
        }
        // A creature with a partner follows it, before its own behaviour, while the hero is
        // within 1,500 of it.
        if let Some(partner) = mind.partner {
            if (here - hero).xz().length() > BEHAVES_WITHIN {
                mind.active = false;
                continue;
            }
            // A child follows the hero only while it stands on a settlement's ground.
            let after_hero = partner == the_hero;
            let found = if after_hero {
                in_settlement.then_some((hero, false, false))
            } else {
                followed.get(&partner).copied()
            };
            let Some((at, partner_walks, partner_trades)) = found else {
                mind.partner = None;
                continue;
            };
            let why = follow(spot(here), spot(at), after_hero);
            debug!(
                "ai: {entity} {why:?} from {:.0},{:.0} about {partner} at {:.0},{:.0}",
                here.x, here.z, at.x, at.z
            );
            match why {
                Why::Follows if mind.runs_after => {
                    walks.push((entity, Walk::run(at, way.level, mind.run)));
                }
                Why::Follows => walks.push((entity, Walk::to(at, way.level, mind.speed))),
                Why::StepsOff => {
                    let free = |to: Spot| {
                        let cell = paths::cell_of(to.x, to.y);
                        heights.walkable_on(cell, way.level)
                            && !crowd.stops(cell, way.level, entity, false)
                    };
                    match steps_off(spot(at), &mut minds.rng, free) {
                        Some(target) => walks
                            .push((entity, Walk::to(point(target, at.y), way.level, mind.speed))),
                        None => mind.partner = None,
                    }
                }
                Why::Meets => {
                    // They meet: an adult, in one case of two, turns to its partner, which stops
                    // and turns back, and talks.
                    mind.last = Some(partner);
                    mind.partner = None;
                    if mind.talks_at_meeting(&mut minds.rng) {
                        turns.push((entity, at));
                        if !partner_trades {
                            turns.push((partner, here));
                            if partner_walks {
                                stops.push(partner);
                            }
                        }
                        commands.entity(entity).insert(Talks);
                    }
                }
                _ => {
                    mind.last = Some(partner);
                    mind.partner = None;
                }
            }
            continue;
        }
        let crowded = crowd.crowded(paths::cell_of(here.x, here.z), way.level, entity);
        let folk = mind.is_townsperson();
        if mind.active && (here - hero).xz().length() > mind.range(zoom) {
            mind.active = false;
            // At dusk and by night one on the ground goes dormant in a house: by the nearest door
            // within 1,500 of it, put there at once (`0x4F0ADC`, `0x4EEFA0`).
            if folk && evening && way.level.is_none() {
                let door = doors
                    .iter()
                    .map(|&(_, at)| at)
                    .filter(|&at| (at - here).xz().length() <= DOOR_WITHIN)
                    .min_by(|a, b| {
                        (*a - here)
                            .length_squared()
                            .total_cmp(&(*b - here).length_squared())
                    });
                if let Some((at, level)) =
                    door.and_then(|door| indoors(door, &heights, &mut minds.rng))
                {
                    debug!(
                        "ai: {entity} {:?} from {:.0},{:.0} to {:.0},{:.0}",
                        Why::PutIn,
                        here.x,
                        here.z,
                        at.x,
                        at.z
                    );
                    commands.entity(entity).insert(PutAt { at, level });
                }
            }
        }
        // A townsperson awake in danger runs from its threat, at every frame it stands.
        if mind.active
            && folk
            && !mind.dialogue
            && let Some(target) = danger(here, &hostiles, &mut minds.rng)
        {
            debug!(
                "ai: {entity} {:?} from {:.0},{:.0} to {:.0},{:.0}",
                Why::Flees,
                here.x,
                here.z,
                target.x,
                target.z
            );
            walks.push((entity, Walk::run(target, way.level, mind.run)));
            continue;
        }
        // A townsperson awake decides at a look of the hero's, unless its dialogue holds it.
        if mind.active && folk && minds.looked && !mind.dialogue {
            let (x, y) = sector(here);
            // The nearest door within 1,500 of the 3 × 3 sectors (`0x4EE300`).
            let nearest_door = doors
                .iter()
                .copied()
                .filter(|&(_, at)| {
                    let (ax, ay) = sector(at);
                    (ax - x).abs() <= 1.0
                        && (ay - y).abs() <= 1.0
                        && (at - here).xz().length() <= DOOR_WITHIN
                })
                .min_by(|a, b| {
                    (a.1 - here)
                        .length_squared()
                        .total_cmp(&(b.1 - here).length_squared())
                });
            let door = nearest_door.map(|(_, at)| at);
            // Going through it, it opens its door (`0x4FC8D0`).
            let mut through = |mind: &mut Mind| {
                if let Some((door, _)) = nearest_door {
                    mind.door = Some(door);
                    swings.push((door, true));
                }
            };
            // At dusk and by night, or with the Daemon near, on a free cell, it takes the
            // evening's way (`0x4F0F5E`): indoors it walks among the building's cells; outdoors
            // it goes into the house of the nearest door, by stages from afar, and without a
            // door it wanders.
            let before_daemon = daemon_near(daemon.0, spot(here), spot(hero));
            // Before the Daemon, in one case of three, it shows its fear over itself
            // (`0x4F0DAC`, red while the hero has solved fewer than 25 quests).
            if before_daemon
                && !crowded
                && let Some(line) = daemon_fear(&mut minds.rng)
            {
                commands.entity(entity).insert(Says::new(line, FEAR_RED));
            }
            if (evening || before_daemon) && !crowded {
                let inside = way.level.is_some();
                let stage = match evening_way(spot(here), inside, door.map(spot), &mut minds.rng) {
                    Evening::Indoors => {
                        indoors(here, &heights, &mut minds.rng).map(|(to, _)| (to, Why::Wanders))
                    }
                    Evening::Toward(to) => Some((point(to, here.y), Why::GoesIn)),
                    Evening::GoesIn => {
                        let inside = door.and_then(|door| indoors(door, &heights, &mut minds.rng));
                        if inside.is_some() {
                            through(&mut mind);
                        }
                        inside.map(|(to, _)| (to, Why::GoesIn))
                    }
                    Evening::Wanders(to) => Some((point(to, here.y), Why::Wanders)),
                };
                if let Some((target, why)) = stage {
                    debug!(
                        "ai: {entity} {why:?} from {:.0},{:.0} to {:.0},{:.0}",
                        here.x, here.z, target.x, target.z
                    );
                    walks.push((entity, Walk::to(target, way.level, mind.speed)));
                }
                continue;
            }
            // Indoors on a free cell, it leaves by day (`0x4F1AB9`).
            if way.level.is_some()
                && !crowded
                && let Some(target) = door.and_then(|door| outdoors(door, &heights, &mut minds.rng))
            {
                debug!(
                    "ai: {entity} {:?} from {:.0},{:.0} to {:.0},{:.0}",
                    Why::GoesOut,
                    here.x,
                    here.z,
                    target.x,
                    target.z
                );
                through(&mut mind);
                walks.push((entity, Walk::to(target, way.level, mind.speed)));
                continue;
            }
            let nearest = |of: &[(Entity, Vec3)]| {
                of.iter()
                    .filter(|&&(other, at)| {
                        let (ax, ay) = sector(at);
                        other != entity
                            && Some(other) != mind.last
                            && (ax - x).abs() <= 1.0
                            && (ay - y).abs() <= 1.0
                    })
                    .min_by(|a, b| {
                        let da = (a.1 - here).length_squared();
                        da.total_cmp(&(b.1 - here).length_squared())
                    })
                    .copied()
            };
            let (human, animal) = (nearest(&humans), nearest(&animals));
            let near_traders = traders
                .iter()
                .filter(|&&(other, at)| {
                    let (ax, ay) = sector(at);
                    other != entity && (ax - x).abs() <= 1.0 && (ay - y).abs() <= 1.0
                })
                .map(|&trader| (trader, spot(trader.1)))
                .collect();
            let trader = trader_for(spot(here), near_traders);
            match townsperson(&mind, human, animal, trader, &mut minds.rng) {
                Decision::Visits((animal, target)) => {
                    mind.last = Some(animal);
                    debug!(
                        "ai: {entity} {:?} from {:.0},{:.0} to {:.0},{:.0}",
                        Why::Visits,
                        here.x,
                        here.z,
                        target.x,
                        target.z
                    );
                    walks.push((entity, Walk::to(target, way.level, mind.speed)));
                    continue;
                }
                Decision::GoesHome => {
                    let target = point(mind.home, here.y);
                    debug!(
                        "ai: {entity} {:?} from {:.0},{:.0} to {:.0},{:.0}",
                        Why::GoesHome,
                        here.x,
                        here.z,
                        target.x,
                        target.z
                    );
                    walks.push((entity, Walk::to(target, way.level, mind.speed)));
                    continue;
                }
                Decision::Partner((partner, _), home) => {
                    debug!("ai: {entity} {:?} {partner}", Why::TakesPartner);
                    mind.partner = Some(partner);
                    mind.runs_after = false;
                    if home {
                        let home = point(mind.home, here.y);
                        walks.push((entity, Walk::to(home, way.level, mind.speed)));
                    }
                    continue;
                }
                Decision::Trade((trader, at)) => {
                    debug!(
                        "ai: {entity} {:?} from {:.0},{:.0} at {trader} at {:.0},{:.0}",
                        Why::Trades,
                        here.x,
                        here.z,
                        at.x,
                        at.z
                    );
                    mind.last = Some(trader);
                    mind.visiting = Some(trader);
                    let walk = Walk::to(at, way.level, mind.speed).within(AT_THE_TRADER);
                    walks.push((entity, walk));
                    continue;
                }
                Decision::Child => {
                    let does = child(
                        &mind,
                        spot(here),
                        (spot(hero), daemon.0),
                        in_settlement,
                        &mut minds.rng,
                    );
                    debug!("ai: {entity} {:?} {does:?}", Why::Plays);
                    match does {
                        Child::Plays(to) => {
                            let to = point(to, here.y);
                            walks.push((entity, Walk::run(to, way.level, mind.run)));
                            playing.push((entity, here));
                        }
                        Child::FollowsHero => {
                            mind.partner = Some(the_hero);
                            mind.runs_after = true;
                        }
                        Child::LooksUp => turns.push((entity, hero)),
                        Child::Speaks(line) => {
                            turns.push((entity, hero));
                            commands.entity(entity).insert(Says::new(line, CHILD_GREEN));
                        }
                        Child::RunsUp(to) | Child::Flees(to) => {
                            let to = point(to, here.y);
                            walks.push((entity, Walk::run(to, way.level, mind.run)));
                        }
                    }
                    continue;
                }
                Decision::House => {
                    if let Some((target, _)) =
                        door.and_then(|door| indoors(door, &heights, &mut minds.rng))
                    {
                        debug!(
                            "ai: {entity} {:?} from {:.0},{:.0} to {:.0},{:.0}",
                            Why::GoesIn,
                            here.x,
                            here.z,
                            target.x,
                            target.z
                        );
                        through(&mut mind);
                        walks.push((entity, Walk::to(target, way.level, mind.speed)));
                        continue;
                    }
                }
                Decision::Juggle => {
                    let (instrument, on) = mind.juggles(&mut minds.rng);
                    debug!("ai: {entity} {:?} {instrument} on {on}", Why::Juggles);
                    commands.entity(entity).insert(Plays { instrument, on });
                    continue;
                }
                Decision::Stop => {}
            }
        }
        if !mind.active || folk {
            if let Some(target) = mind.parts(spot(here), crowded, dt, &mut minds.rng) {
                let target = point(target, here.y);
                debug!(
                    "ai: {entity} {:?} from {:.0},{:.0} to {:.0},{:.0}",
                    Why::Parts,
                    here.x,
                    here.z,
                    target.x,
                    target.z
                );
                walks.push((entity, Walk::to(target, way.level, mind.speed)));
            }
            continue;
        }
        // The nearest it fears in its own sector.
        let fear = feared
            .iter()
            .filter(|&&(other, at)| other != entity && sector(at) == sector(here))
            .map(|&(_, at)| at)
            .min_by(|a, b| {
                let (da, db) = ((*a - here).length_squared(), (*b - here).length_squared());
                da.total_cmp(&db)
            });
        let Some((target, why)) =
            mind.behaves(spot(here), fear.map(spot), crowded, dt, &mut minds.rng)
        else {
            continue;
        };
        let target = point(target, here.y);
        debug!(
            "ai: {entity} {why:?} from {:.0},{:.0} to {:.0},{:.0}",
            here.x, here.z, target.x, target.z
        );
        walks.push((
            entity,
            if why == Why::Flees {
                Walk::run(target, way.level, mind.run)
            } else {
                Walk::to(target, way.level, mind.speed)
            },
        ));
    }
    let sent: std::collections::HashSet<Entity> = walks.iter().map(|&(entity, _)| entity).collect();
    for (entity, walk) in walks {
        commands.entity(entity).insert(walk);
    }
    for entity in stops {
        commands.entity(entity).remove::<Walk>();
    }
    for (door, open) in swings {
        commands.entity(door).insert(Swing::to(open));
    }
    // The other idle children within 600 of one at play, without a dialogue, run after it
    // (`0x4EE820`); not one sent off this frame (`sent`).
    for (leader, at) in playing {
        let (lx, ly) = sector(at);
        for (entity, transform, _, mut mind, walking) in &mut creatures {
            let (x, y) = sector(transform.translation);
            if entity != leader
                && !walking
                && !sent.contains(&entity)
                && mind.is_child()
                && mind.partner.is_none()
                && !mind.dialogue
                && (x - lx).abs() <= 1.0
                && (y - ly).abs() <= 1.0
                && (transform.translation - at).xz().length() <= PLAYMATES
            {
                debug!("ai: {entity} {:?} {leader}", Why::Gathers);
                mind.partner = Some(leader);
                mind.runs_after = true;
            }
        }
    }
    for (entity, at) in turns {
        if let Ok((_, mut transform, ..)) = creatures.get_mut(entity) {
            transform.rotation = crate::hero::facing(at - transform.translation);
        }
    }
}

/// The creatures' idle life.
pub struct AiPlugin;

impl Plugin for AiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Minds>()
            .init_resource::<Daemon>()
            .init_resource::<FightRules>()
            .add_systems(
                Update,
                (think, aside, behave, pursue)
                    .chain()
                    .after(crate::crowd::gather)
                    .in_set(Step::Crowd),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_townsperson_runs_700_from_a_hostile_within_800() {
        let mut rng = Rng::of(50, 39, 10);
        let here = Vec3::ZERO;
        let goblin = Vec3::new(500.0, 0.0, 0.0);
        let far = Vec3::new(900.0, 0.0, 0.0);
        assert_eq!(danger(here, &[far], &mut rng), None);
        for _ in 0..50 {
            let to = danger(here, &[far, goblin], &mut rng).unwrap();
            assert!((to.x + 700.0).abs() <= 100.0 && to.z.abs() <= 100.0, "{to}");
        }
    }

    #[test]
    fn a_mind_keeps_its_home_on_the_ground() {
        let home = Vec3::new(1000.0, 50.0, -300.0);
        let mind = Mind::new(home, 0x1, (true, false), None, [70.0, 140.0], (76, 2));
        assert_eq!(mind.home, Spot::new(1000.0, -300.0));
        assert_eq!(point(mind.home, home.y), home);
    }
}
