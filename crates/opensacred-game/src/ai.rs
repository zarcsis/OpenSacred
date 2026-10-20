//! The creatures' idle life while no foe is near (docs/re/ai.md): each is dormant until its own
//! timer or the hero's look wakes it, and dormant again farther than 1,500 from the hero; a
//! hostile creature in the hero's reach steps about by the idle action (`0x54EA30`), a group's
//! members together; a peaceful animal awake flees, steps away and strolls about home, and a
//! guard keeps its post, and a townsperson rolls what it does next at each of the hero's looks,
//! by their behaviours (`0x4EF150`); a creature that shares its cell with another parts from it,
//! and one that walks into the hero's way steps aside. The fight is not followed: an awake hostile creature, which would
//! attack the hero by a test of the levels, goes dormant again and steps about as one beyond it
//! (docs/decisions/2026-10-20-idle.md).

use bevy::prelude::*;

use crate::crowd::{Crowd, InTheWay};
use crate::frame::Step;
use crate::hero::Body;
use crate::npcs::Walk;
use crate::paths;
use crate::spawning::Rng;
use crate::terrain::Heights;

/// How often the hero looks about, in seconds (`0x547070`), and how far, in world units.
const LOOK_EVERY: f32 = 0.3;
const RANGE: f32 = 1_875.0;
/// How near the hero wakes a hostile creature of bit 0, standing and while it moves.
const WAKES: f32 = 800.0;
const WAKES_MOVING: f32 = 400.0;
/// How far from the hero, along the grid's x and y, an awake creature goes dormant again
/// (`0x542B20`).
const FAR_X: f32 = 1_500.0;
const FAR_Y: f32 = 1_000.0;
/// How far from home along an axis sends a hostile creature home, in one case of six.
const AWAY: f32 = 200.0;
/// A step of the idle action: within 150 along each axis (`rand() % 301 − 150`).
const STEP: i32 = 150;
/// How far a group's member's step strays from its fellow's (`±50`).
const FELLOW: i32 = 50;
/// The periods of the creatures' timers, and the jitter of each firing (`0x5375C0`).
const PERIOD: f32 = 5.0;
const PERIOD_HOSTILE: f32 = 2.5;
const JITTER: f32 = 1.5;
/// The flags whose creatures' own timers wake them: the hostile ones and the townsfolk.
const TIMED: u32 = 0x3 | 0x408;
/// The behaviours' flags: the wild animals, the farm animals, the guards, the townsfolk
/// (`0x4EF150`); those that the game follows.
const WILD: u32 = 0x40;
const WILD_TOO: u32 = 0x80;
const FARM: u32 = 0x10;
const FARM_LOOSE: u32 = 0x20;
const GUARD: u32 = 0x4000;
const TOWNSFOLK: u32 = 0x8;
const CITIZEN: u32 = 0x400;
/// `CreateNPC`'s tag `0x4C`, which the behaviours take as 8 (`0x4EE030`).
const TOWNSFOLK_TOO: u32 = 0x8000;
const TOWNSFOLKS: u32 = TOWNSFOLK | CITIZEN | TOWNSFOLK_TOO | JUGGLER;
const BEHAVIOURS: u32 = WILD | WILD_TOO | FARM | FARM_LOOSE | GUARD | TOWNSFOLKS;
/// How far from the hero, times the camera's zoom, a townsperson goes dormant again
/// (`0x4F0A87`, the float at `0x891028`; the zoom `0x611400`).
const TOWNSFOLK_RANGE: f32 = 600.0;
/// The children's types (`0x43F670`), which always roll 20.
const CHILDREN: [u32; 7] = [229, 230, 231, 702, 703, 704, 705];
/// How far from the hero a creature of a behaviour goes dormant again (`0x5370B0`, the distance).
const BEHAVES_WITHIN: f32 = 1_500.0;
/// The jugglers' types, and the flags they take at their first update, which no behaviour reads
/// yet.
const JUGGLERS: [u32; 4] = [78, 79, 82, 104];
const JUGGLER: u32 = 0x1000_0000;
/// How near a creature other than an animal makes a wild animal flee and a farm animal step
/// away (800 when it attacks, which none does yet).
const FLEE: f32 = 300.0;
const SHY: f32 = 200.0;
/// How far from home along an axis brings a wild animal back, within 50 of it.
const ROAMS: f32 = 380.0;
const BACK: i32 = 50;
/// How far about home the animals stroll: the wild within 100, the farm within 150.
const STROLL_WILD: i32 = 100;
const STROLL_FARM: i32 = 150;
/// How far from its post along an axis sends a guard back to it, and how far about it a guard
/// strolls.
const POST: f32 = 90.0;
const STROLL_GUARD: i32 = 200;
/// The chance of a stroll, a frame's 1 in 101 at 30 frames a second, as a rate a second.
const STROLLS_A_SECOND: f32 = 30.0 / 101.0;
/// The chance that an idle creature sharing its cell parts, a frame's 1 in 6, as a rate a second
/// (`0x544890`), and how far about home or itself it goes.
const PARTS_A_SECOND: f32 = 30.0 / 6.0;
const PART: i32 = 100;
/// The flag that keeps a creature from parting (`CreateNPC`'s tag `0x74`).
const STAYS: u32 = 0x800_0000;
/// How near the hero a creature not hostile that walks steps aside, how far about itself it
/// looks for a point it reaches, and in how many tries (`0x547070`).
const ASIDE_WITHIN: f32 = 91.0;
const ASIDE: i32 = 200;
const ASIDE_TRIES: usize = 10;

/// A creature's mind while no foe is near.
#[derive(Component, Clone, Debug)]
pub struct Mind {
    /// Where it was made, the point it goes home to.
    pub home: Vec3,
    /// Its flags word (`+0x1F4`, `crowd::flags_word`).
    pub flags: u32,
    /// Made by a sector's `Enter` function (`+0x14` bit `0x80000000`): its steps turn about the
    /// hero and move its home.
    pub enter: bool,
    /// Its group, if it is in one (`+0x249`).
    pub group: Option<u32>,
    /// Its walk speed in world units a second, before the game's speed, and its run speed.
    pub speed: f32,
    pub run: f32,
    /// Its type, and its `Creature.pak` class: the animals (4 and 6) flee from the others.
    pub creature: u32,
    pub class: u8,
    /// Whether it has a dialogue (`+0x245`, `sdk_STAMP_IDX` `0x5498F0`), which holds it: a
    /// townsperson keeps still, a guard does not stroll, a crowded creature does not part.
    pub dialogue: bool,
    /// The creature it went to last, which it does not go to next (`+0x24D`).
    last: Option<Entity>,
    awake: bool,
    /// A peaceful creature awake, which takes its behaviour at every frame while it stands.
    active: bool,
    clock: f32,
    period: f32,
}

impl Mind {
    /// The mind of a creature made at `home` with the flags word `flags`, by an `Enter` function
    /// or not, by `CreateNPC` (`created`) or a fill: `CreateNPC` wakes its hostile creatures as
    /// it makes them (`sdk_WAKE`) and gives them a timer of 2.5 s.
    pub fn new(
        home: Vec3,
        flags: u32,
        (enter, created): (bool, bool),
        group: Option<u32>,
        [speed, run]: [f32; 2],
        (creature, class): (u32, u8),
    ) -> Self {
        let hostile = flags & 0x3 != 0;
        // A creature of an `Enter` function still without flags takes them at its first update
        // (`0x5375C0`): the animals of class 6 the loose farm animals', the jugglers theirs, the
        // others the townsfolk's.
        let flags = match (enter, flags, class) {
            (true, 0, 6) => FARM_LOOSE,
            (true, 0, _) if JUGGLERS.contains(&creature) => JUGGLER,
            (true, 0, _) => TOWNSFOLK,
            _ => flags,
        };
        Self {
            home,
            flags,
            enter,
            group,
            speed,
            run,
            creature,
            class,
            dialogue: false,
            last: None,
            active: false,
            awake: hostile && created,
            clock: 0.0,
            period: if hostile && created {
                PERIOD_HOSTILE
            } else {
                PERIOD
            },
        }
    }

    fn hostile(&self) -> bool {
        self.flags & 0x3 != 0
    }
}

/// The flags the creature maker (`0x480D40`) gives a peaceful creature of type `creature` made
/// without flags, as a sector's fill makes them: the wild animals' to the wolves, the crows, the
/// bats, the first rabbit, the deer, the bears, the bulls and the wild boars; the farm animals'
/// to the pigs, the cows, the sheep, the horses and the chickens; none to the others, the two
/// other rabbits among them. `CreateNPC` writes its own flags over them (`0x4839F6`).
pub fn of_type(creature: u32) -> u32 {
    const WILD_TYPES: [u32; 25] = [
        512, 513, 514, 515, 516, 530, 531, 532, 533, 536, 537, 538, 540, 541, 543, 545, 567, 568,
        569, 570, 589, 590, 594, 595, 596,
    ];
    const FARM_TYPES: [u32; 25] = [
        520, 521, 522, 523, 524, 525, 526, 527, 528, 544, 550, 551, 552, 553, 554, 555, 556, 557,
        558, 560, 561, 562, 563, 565, 566,
    ];
    if WILD_TYPES.contains(&creature) {
        WILD
    } else if FARM_TYPES.contains(&creature) {
        FARM
    } else {
        0
    }
}

/// The game's generator for the creatures' chances, the time since the hero last looked, and
/// whether it looked this frame.
#[derive(Resource)]
struct Minds {
    rng: Rng,
    since: f32,
    looked: bool,
}

impl Default for Minds {
    fn default() -> Self {
        Self {
            rng: Rng::of(0, 0, 0),
            since: 0.0,
            looked: false,
        }
    }
}

/// A step to `target` on level `level` at `speed`.
fn walk(target: Vec3, speed: f32, way: &InTheWay) -> Walk {
    Walk::to(target, way.level, speed)
}

/// What the idle action makes a creature at `here` do: in one case of ten nothing yet (the
/// original fidgets), else in one case of four a step: within 150 along each axis, or home in one
/// case of four, for a creature of an `Init` function or the start script; its place turned about
/// the hero by −30° to +30°, for one of an `Enter` function; `None` otherwise.
fn idle_step(mind: &Mind, here: Vec3, hero: Vec3, rng: &mut Rng) -> Option<Vec3> {
    if rng.below(10) == 0 || rng.below(4) != 0 {
        return None;
    }
    if mind.enter {
        let degrees = f32::from(u16::try_from(rng.below(61)).ok()?) - 30.0;
        let turn = Quat::from_rotation_y(degrees.to_radians());
        return Some(hero + turn * (here - hero));
    }
    if rng.below(4) == 0 {
        return Some(mind.home);
    }
    let offset = |rng: &mut Rng| {
        let d = i32::try_from(rng.below(u32::try_from(2 * STEP + 1).ok()?)).ok()? - STEP;
        Some(f32::from(i16::try_from(d).ok()?))
    };
    Some(here + Vec3::new(offset(rng)?, 0.0, offset(rng)?))
}

/// Runs the creatures' timers, the hero's look every 0.3 s, and the awake creatures' thought.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn think(
    mut commands: Commands,
    time: Res<Time>,
    mut minds: ResMut<Minds>,
    hero: Query<(&Transform, &Body)>,
    mut creatures: Query<(Entity, &Transform, &InTheWay, &mut Mind, Has<Walk>), Without<Body>>,
) {
    let Some((hero, body)) = hero.iter().next() else {
        return;
    };
    let hero = hero.translation;
    let dt = time.delta_secs();
    let minds = &mut *minds;
    minds.since += dt;
    let look = minds.since >= LOOK_EVERY;
    if look {
        minds.since = 0.0;
    }
    minds.looked = look;
    let side = crate::iso::CELL * 64.0;
    let sector = |p: Vec3| ((p.x / side).floor(), (p.z / side).floor());
    let (sx, sy) = sector(hero);
    let wakes = if body.moving().is_some() {
        WAKES_MOVING
    } else {
        WAKES
    };
    // The steps taken this frame: a creature, its target, and its group's step to share.
    let mut steps: Vec<(Entity, Vec3)> = Vec::new();
    let mut grouped: Vec<(u32, Vec3, Entity)> = Vec::new();
    for (entity, transform, _, mut mind, walking) in &mut creatures {
        let here = transform.translation;
        // Its own timer.
        if mind.flags & TIMED != 0 {
            mind.clock += dt;
            if mind.clock >= mind.period {
                let jitter = f32::from(u16::try_from(minds.rng.below(31)).unwrap_or(15)) * 0.1;
                mind.clock = jitter - JITTER;
                mind.awake = true;
            }
        }
        let distance = (here - hero).xz().length();
        let (x, y) = sector(here);
        let near = (x - sx).abs() <= 1.0 && (y - sy).abs() <= 1.0 && distance < RANGE;
        let mut idle = false;
        if look && near {
            if !mind.hostile() || (mind.flags & 0x1 != 0 && distance < wakes) {
                mind.awake = true;
            } else {
                idle = true;
            }
        }
        if mind.awake {
            mind.awake = false;
            let away = (here - hero).abs();
            if away.x > FAR_X || away.z > FAR_Y {
                mind.clock = -JITTER;
            } else if mind.hostile() {
                let from_home = (here - mind.home).abs();
                if minds.rng.below(6) == 0 && (from_home.x > AWAY || from_home.z > AWAY) {
                    if !walking {
                        steps.push((entity, mind.home));
                    }
                } else if minds.rng.below(3) == 0 {
                    // The fight is not followed: dormant again, by chance stepping about.
                    idle = true;
                }
            } else if mind.flags & BEHAVIOURS != 0 {
                mind.active = true;
            }
        }
        if idle
            && !walking
            && let Some(target) = idle_step(&mind, here, hero, &mut minds.rng)
        {
            if mind.enter {
                mind.home = target;
            }
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
            let stray = |rng: &mut Rng| {
                let d = i32::try_from(rng.below(u32::try_from(2 * FELLOW + 1).unwrap_or(1)))
                    .unwrap_or(FELLOW)
                    - FELLOW;
                f32::from(i16::try_from(d).unwrap_or(0))
            };
            let target = transform.translation
                + step
                + Vec3::new(stray(&mut minds.rng), 0.0, stray(&mut minds.rng));
            mind.home = target;
            steps.push((entity, target));
        }
    }
    for (entity, target) in steps {
        if let Ok((_, _, way, mind, _)) = creatures.get(entity) {
            commands
                .entity(entity)
                .insert(walk(target, mind.speed, way));
        }
    }
}

/// Whether a chance of `rate` a second comes in a frame of `dt` seconds.
fn chance(rng: &mut Rng, rate: f32, dt: f32) -> bool {
    let p = 1.0 - (-rate * dt).exp();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a probability between 0 and 1, in parts of 10,000"
    )]
    let parts = (p.clamp(0.0, 1.0) * 10_000.0) as u32;
    rng.below(10_000) < parts
}

/// A point within `within` of `at` along each axis, at random.
fn about(at: Vec3, within: i32, rng: &mut Rng) -> Vec3 {
    let mut offset = || {
        let span = u32::try_from(2 * within + 1).unwrap_or(1);
        let d = i32::try_from(rng.below(span)).unwrap_or(within) - within;
        f32::from(i16::try_from(d).unwrap_or(0))
    };
    at + Vec3::new(offset(), 0.0, offset())
}

/// Why a creature walks by itself: it flees at a run, steps away, goes back home or to its post,
/// strolls, parts from another in its cell, or steps out of the hero's way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Why {
    Flees,
    StepsAway,
    GoesBack,
    Strolls,
    Parts,
    StepsAside,
    Visits,
    GoesHome,
}

/// A townsperson's roll of 26 (`0x4F22F0`): a child always rolls 20, a juggler of `0x10000000`
/// always 9, one of the jugglers' types 9 in one case of six, and anyone else's 9 counts as 10.
fn roll(mind: &Mind, rng: &mut Rng) -> u32 {
    if CHILDREN.contains(&mind.creature) {
        return 20;
    }
    let roll = rng.below(26);
    if mind.flags & JUGGLER != 0 || (JUGGLERS.contains(&mind.creature) && rng.below(6) == 0) {
        9
    } else if roll == 9 {
        10
    } else {
        roll
    }
}

/// What a townsperson standing does when it decides by day, by its roll (`0x4F22F0`): 1 walks to
/// the nearest animal, `animal`, 2 and 3 walk home; 0 and 2 take a partner, 4, 6 and 7 go into a
/// house, 5 and 8 visit a trader, 9 plays an instrument and 20 a child's play, which wait for
/// their features (#250, #248, #251, #253, #252); the rest stop. The walk's target, why it walks
/// and whom it goes to.
fn townsperson(
    mind: &Mind,
    animal: Option<(Entity, Vec3)>,
    rng: &mut Rng,
) -> Option<(Vec3, Why, Option<Entity>)> {
    match roll(mind, rng) {
        1 => animal.map(|(entity, at)| (at, Why::Visits, Some(entity))),
        2 | 3 => Some((mind.home, Why::GoesHome, None)),
        _ => None,
    }
}

/// Where a creature at `here` that shares its cell parts to (`0x544890`): within 100 of its home
/// when the original's measure of its distance from home lies between 30 and 400, else within
/// 100 of itself. The measure takes, of each point, its y from its x (`0x545B30`), not the home
/// from the creature, and gives up past ±32,000, so that a creature parts about itself but near
/// the world's diagonal.
fn parting(home: Vec3, here: Vec3, rng: &mut Rng) -> Vec3 {
    let (a, b) = (here.x - here.z, home.x - home.z);
    let measure = (a * a + b * b).sqrt();
    let measured = a.abs() <= 32_000.0 && b.abs() <= 32_000.0;
    if measured && measure > 30.0 && measure < 400.0 {
        about(home, PART, rng)
    } else {
        about(here, PART, rng)
    }
}

/// Sends the creatures not hostile that walk within 91 of the hero, at its look, to a point
/// within 200 of themselves that they reach, the first of 10 tries (`0x547070`).
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
        if mind.hostile() || (here - hero).xz().length() >= ASIDE_WITHIN {
            continue;
        }
        let from = (paths::cell_of(here.x, here.z), (way.level, false));
        let reaches = |to: Vec3| {
            paths::path_onto(
                from,
                paths::cell_of(to.x, to.z),
                |_| true,
                |place| {
                    let mut next = heights.neighbours(place);
                    next.retain(|&(cell, (at, _))| !crowd.stops(cell, at, entity, false));
                    next
                },
            )
            .is_some()
        };
        let Some(target) = (0..ASIDE_TRIES)
            .map(|_| about(here, ASIDE, &mut minds.rng))
            .find(|&to| reaches(to))
        else {
            continue;
        };
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

/// A number between `low` and `high`, at random.
fn between(low: f32, high: f32, rng: &mut Rng) -> f32 {
    let part = f32::from(u16::try_from(rng.below(10_001)).unwrap_or(0)) / 10_000.0;
    low + (high - low) * part
}

/// Where a wild animal at `here` runs from `fear` (`0x4EF150`): along each axis of the grid away
/// from it by half to twice its distance along that axis, and across by up to half its distance
/// along the other either way.
fn flight(here: Vec3, fear: Vec3, rng: &mut Rng) -> Vec3 {
    let d = (here - fear).xz();
    let (x, y) = (d.x.abs(), d.y.abs());
    let along = |offset: f32, rng: &mut Rng| {
        let away = between(offset.abs() / 2.0, offset.abs() * 2.0, rng);
        if offset < 0.0 { -away } else { away }
    };
    let dx = along(d.x, rng) + between(-y / 2.0, y / 2.0, rng);
    let dz = along(d.y, rng) + between(-x / 2.0, x / 2.0, rng);
    here + Vec3::new(dx, 0.0, dz)
}

/// What a peaceful creature awake does now by its behaviour, standing at `here` (`0x4EF150`): a
/// wild animal (flags `0x40`, `0x80`) runs from the nearest creature other than an animal within
/// 300 of it, `fear`, the hero among them ([`flight`]), walks back within 50 of home from farther
/// than 380 along an axis, and strolls within 100 of home by chance; a farm animal (`0x10`,
/// `0x20`) steps away from such a creature within 200, to the point beyond itself, and strolls
/// within 150 of home; a guard (`0x4000`) strolls within 200 of its post by chance, else walks
/// back to it from farther than 90 along an axis. The walk's target and why it walks.
fn behaviour(
    mind: &Mind,
    here: Vec3,
    fear: Option<Vec3>,
    strolls: bool,
    rng: &mut Rng,
) -> Option<(Vec3, Why)> {
    let near = |within: f32| fear.filter(|f| (*f - here).xz().length() < within);
    if mind.flags & (WILD | WILD_TOO) != 0 {
        if let Some(fear) = near(FLEE) {
            return Some((flight(here, fear, rng), Why::Flees));
        }
        let from_home = (here - mind.home).abs();
        if from_home.x > ROAMS || from_home.z > ROAMS {
            return Some((about(mind.home, BACK, rng), Why::GoesBack));
        }
        return strolls.then(|| (about(mind.home, STROLL_WILD, rng), Why::Strolls));
    }
    if mind.flags & (FARM | FARM_LOOSE) != 0 {
        if let Some(fear) = near(SHY) {
            return Some((here + (here - fear).with_y(0.0), Why::StepsAway));
        }
        return strolls.then(|| (about(mind.home, STROLL_FARM, rng), Why::Strolls));
    }
    if mind.flags & GUARD != 0 {
        if strolls && !mind.dialogue {
            return Some((about(mind.home, STROLL_GUARD, rng), Why::Strolls));
        }
        let from_post = (here - mind.home).abs();
        return (from_post.x > POST || from_post.z > POST).then_some((mind.home, Why::GoesBack));
    }
    None
}

/// The peaceful creatures' behaviours while they stand awake, at every frame (`0x4EF150`); a
/// creature farther than 1,500 from the hero goes dormant again, a townsperson farther than 600
/// times the camera's zoom. A townsperson decides at each of the hero's looks, which wake it, as
/// the original's goes dormant after most of its rolls and decides again when woken. An idle
/// creature, or a townsperson between its rolls, that shares its cell with another parts from it
/// by chance (`0x544890`); an animal awake strolls at once.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn behave(
    mut commands: Commands,
    time: Res<Time>,
    mut minds: ResMut<Minds>,
    crowd: Res<Crowd>,
    hero: Query<&Transform, With<Body>>,
    cameras: Query<&Projection, With<Camera3d>>,
    mut creatures: Query<(Entity, &Transform, &InTheWay, &mut Mind, Has<Walk>), Without<Body>>,
) {
    let Some(hero) = hero.iter().next() else {
        return;
    };
    let hero = hero.translation;
    let dt = time.delta_secs();
    // The camera's zoom, the inverse of its scale.
    let zoom = cameras
        .iter()
        .find_map(|projection| match projection {
            Projection::Orthographic(ortho) => Some(1.0 / ortho.scale),
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
    // What the townsfolk visit: the farm and wild animals, at the hero's looks.
    let animals: Vec<(Entity, Vec3)> = if minds.looked {
        creatures
            .iter()
            .filter(|(_, _, _, mind, _)| {
                mind.class == 6 && mind.flags & (WILD | WILD_TOO | FARM | FARM_LOOSE) != 0
            })
            .map(|(entity, transform, ..)| (entity, transform.translation))
            .collect()
    } else {
        Vec::new()
    };
    let mut walks = Vec::new();
    for (entity, transform, way, mut mind, walking) in &mut creatures {
        if walking {
            continue;
        }
        let here = transform.translation;
        let crowded = crowd.crowded(paths::cell_of(here.x, here.z), way.level, entity);
        let folk = mind.flags & TOWNSFOLKS != 0;
        let range = if folk {
            TOWNSFOLK_RANGE * zoom
        } else {
            BEHAVES_WITHIN
        };
        if mind.active && (here - hero).xz().length() > range {
            mind.active = false;
        }
        // A townsperson awake decides at a look of the hero's, unless its dialogue holds it.
        if mind.active && folk && minds.looked && !mind.dialogue {
            let (x, y) = sector(here);
            let animal = animals
                .iter()
                .filter(|&&(other, at)| {
                    let (ax, ay) = sector(at);
                    Some(other) != mind.last && (ax - x).abs() <= 1.0 && (ay - y).abs() <= 1.0
                })
                .min_by(|a, b| {
                    let da = (a.1 - here).length_squared();
                    da.total_cmp(&(b.1 - here).length_squared())
                })
                .copied();
            if let Some((target, why, whom)) = townsperson(&mind, animal, &mut minds.rng) {
                if whom.is_some() {
                    mind.last = whom;
                }
                debug!(
                    "ai: {entity} {why:?} from {:.0},{:.0} to {:.0},{:.0}",
                    here.x, here.z, target.x, target.z
                );
                walks.push((entity, Walk::to(target, way.level, mind.speed)));
                continue;
            }
        }
        if !mind.active || folk {
            if crowded
                && mind.flags & STAYS == 0
                && !mind.dialogue
                && chance(&mut minds.rng, PARTS_A_SECOND, dt)
            {
                let target = parting(mind.home, here, &mut minds.rng);
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
        // An animal strolls by chance, or at once where another shares its cell.
        let animal = mind.flags & (WILD | WILD_TOO | FARM | FARM_LOOSE) != 0;
        let strolls = chance(&mut minds.rng, STROLLS_A_SECOND, dt) || (crowded && animal);
        let Some((target, why)) = behaviour(&mind, here, fear, strolls, &mut minds.rng) else {
            continue;
        };
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
    for (entity, walk) in walks {
        commands.entity(entity).insert(walk);
    }
}

/// The creatures' idle life.
pub struct AiPlugin;

impl Plugin for AiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Minds>().add_systems(
            Update,
            (think, aside, behave)
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
    fn a_hostile_creature_of_a_script_wakes_as_it_is_made_and_looks_every_2_5_s() {
        let home = Vec3::ZERO;
        let script = Mind::new(home, 0x1, (false, true), None, SPEEDS, (0, 0));
        assert!(script.awake && script.period == PERIOD_HOSTILE);
        // A fill's hostile creature and a peaceful one sleep until woken, on a timer of 5 s.
        let fill = Mind::new(home, 0x1, (true, false), Some(7), SPEEDS, (0, 0));
        assert!(!fill.awake && fill.period == PERIOD);
        let villager = Mind::new(home, 0x400, (true, true), None, SPEEDS, (0, 0));
        assert!(!villager.awake && !villager.hostile());
    }

    const SPEEDS: [f32; 2] = [70.0, 140.0];

    #[test]
    fn a_peaceful_creature_of_an_enter_function_takes_a_behaviour_by_its_class() {
        let made = |flags, created, creature, class| {
            let made = (true, created);
            Mind::new(Vec3::ZERO, flags, made, None, SPEEDS, (creature, class)).flags
        };
        // A fill's animals by the creature maker's lists: the first rabbit wild, a pig farm, the
        // other rabbits without, so of class 6 the loose farm animals' at the first update.
        assert_eq!(made(of_type(516), false, 516, 6), WILD);
        assert_eq!(made(of_type(520), false, 520, 6), FARM);
        assert_eq!(made(of_type(592), false, 592, 6), FARM_LOOSE);
        // `CreateNPC`'s without a tag: class 6 the loose farm animals', the jugglers theirs, the
        // others the townsfolk's; a tag or hostility keeps its own.
        assert_eq!(made(0, true, 560, 6), FARM_LOOSE);
        assert_eq!(made(0, true, 78, 3), JUGGLER);
        assert_eq!(made(0, true, 640, 3), TOWNSFOLK);
        assert_eq!(made(0x4000, true, 640, 3), 0x4000);
        assert_eq!(made(0x1, false, 512, 6), 0x1);
        // An `Init` function's creature without a tag keeps none and stands.
        let init = Mind::new(Vec3::ZERO, 0, (false, true), None, SPEEDS, (560, 6));
        assert_eq!(init.flags, 0);
    }

    #[test]
    fn a_wild_animal_flees_at_a_run_and_a_farm_animal_steps_away() {
        let mut rng = Rng::of(48, 39, 2);
        let home = Vec3::new(1000.0, 0.0, 1000.0);
        let rabbit = Mind::new(home, of_type(516), (true, false), None, SPEEDS, (516, 6));
        let pig = Mind::new(home, of_type(520), (true, false), None, SPEEDS, (520, 6));
        let hero = Vec3::new(1100.0, 0.0, 1000.0);
        // Within 300 of the hero the rabbit runs from it, by half to twice its distance along x
        // and across by up to half of none; beyond, it stands.
        for _ in 0..100 {
            let (to, why) = behaviour(&rabbit, home, Some(hero), false, &mut rng).unwrap();
            assert_eq!(why, Why::Flees);
            assert!(
                (800.0..=950.0).contains(&to.x) && (950.0..=1050.0).contains(&to.z),
                "{to}"
            );
        }
        let far = Vec3::new(1400.0, 0.0, 1000.0);
        assert_eq!(behaviour(&rabbit, home, Some(far), false, &mut rng), None);
        // Within 200 the pig walks to the point beyond itself; at 300 it stands.
        let (to, why) = behaviour(&pig, home, Some(hero), false, &mut rng).unwrap();
        assert!(
            why == Why::StepsAway && (to - Vec3::new(900.0, 0.0, 1000.0)).length() < 0.01,
            "{to}"
        );
        let near = Vec3::new(1300.0, 0.0, 1000.0);
        assert_eq!(behaviour(&pig, home, Some(near), false, &mut rng), None);
        // Farther than 380 from home the rabbit walks back within 50 of it.
        let strayed = home + Vec3::new(0.0, 0.0, 400.0);
        let (to, why) = behaviour(&rabbit, strayed, None, false, &mut rng).unwrap();
        let d = to - home;
        assert!(
            why == Why::GoesBack && d.x.abs() <= 50.0 && d.z.abs() <= 50.0,
            "{d}"
        );
        // A stroll stays within 100 of home for the rabbit, 150 for the pig.
        for _ in 0..200 {
            let (to, _) = behaviour(&rabbit, home, None, true, &mut rng).unwrap();
            let d = to - home;
            assert!(d.x.abs() <= 100.0 && d.z.abs() <= 100.0, "{d}");
            let (to, _) = behaviour(&pig, home, None, true, &mut rng).unwrap();
            let d = to - home;
            assert!(d.x.abs() <= 150.0 && d.z.abs() <= 150.0, "{d}");
        }
    }

    #[test]
    fn a_guard_strolls_about_its_post_and_goes_back_to_it() {
        let mut rng = Rng::of(50, 39, 4);
        let post = Vec3::new(1000.0, 0.0, 1000.0);
        let guard = Mind::new(post, 0x4000, (false, true), None, SPEEDS, (0, 3));
        // Within 90 of its post along each axis it stands; farther it walks back to it.
        let near = post + Vec3::new(90.0, 0.0, -90.0);
        assert_eq!(behaviour(&guard, near, None, false, &mut rng), None);
        let far = post + Vec3::new(20.0, 0.0, 95.0);
        assert_eq!(
            behaviour(&guard, far, None, false, &mut rng),
            Some((post, Why::GoesBack))
        );
        // By chance it strolls within 200 of its post, from wherever it stands; it fears no one.
        let hero = Some(far + Vec3::X);
        for _ in 0..100 {
            let (to, why) = behaviour(&guard, far, hero, true, &mut rng).unwrap();
            let d = to - post;
            assert!(
                why == Why::Strolls && d.x.abs() <= 200.0 && d.z.abs() <= 200.0,
                "{d}"
            );
        }
    }

    #[test]
    fn a_crowded_creature_parts_about_itself_but_near_the_worlds_diagonal() {
        let mut rng = Rng::of(50, 39, 5);
        let near = |to: Vec3, at: Vec3| (to - at).x.abs() <= 100.0 && (to - at).z.abs() <= 100.0;
        // At the start of the Seraphim, far from the diagonal: about itself, wherever home is.
        let here = Vec3::new(173_688.0, 0.0, 134_781.0);
        let home = here + Vec3::new(250.0, 0.0, 0.0);
        for _ in 0..50 {
            assert!(near(parting(home, here, &mut rng), here));
        }
        // Near the diagonal, its measure 30 to 400: about home.
        let here = Vec3::new(10_000.0, 0.0, 10_000.0);
        let home = Vec3::new(10_100.0, 0.0, 10_000.0);
        for _ in 0..50 {
            assert!(near(parting(home, here, &mut rng), home));
        }
        // On the diagonal at home, its measure 0: about itself.
        assert!(near(parting(here, here, &mut rng), here));
    }

    #[test]
    fn a_townsperson_rolls_home_an_animal_or_a_stop() {
        let mut rng = Rng::of(50, 39, 6);
        let home = Vec3::new(1000.0, 0.0, 1000.0);
        let farmer = Mind::new(home, TOWNSFOLK, (true, true), None, SPEEDS, (640, 3));
        let pig = (
            Entity::from_raw_u32(7).unwrap(),
            Vec3::new(1300.0, 0.0, 900.0),
        );
        let (mut homes, mut visits, mut stops) = (0, 0, 0);
        for _ in 0..26_000 {
            match townsperson(&farmer, Some(pig), &mut rng) {
                Some((to, Why::GoesHome, None)) if to == home => homes += 1,
                Some((to, Why::Visits, Some(_))) if to == pig.1 => visits += 1,
                None => stops += 1,
                other => panic!("{other:?}"),
            }
        }
        // Home in two rolls of 26, the animal in one, else none yet.
        assert!((1_700..2_300).contains(&homes), "{homes}");
        assert!((800..1_200).contains(&visits), "{visits}");
        assert_eq!(homes + visits + stops, 26_000);
        // Without an animal the roll of 1 stops; a child always rolls 20, a juggler 9.
        assert!((0..100).all(|_| roll(&farmer, &mut rng) != 9));
        let child = Mind::new(home, TOWNSFOLK, (true, true), None, SPEEDS, (702, 3));
        assert!((0..100).all(|_| roll(&child, &mut rng) == 20));
        let juggler = Mind::new(home, 0, (true, true), None, SPEEDS, (78, 3));
        assert_eq!(juggler.flags, JUGGLER);
        assert!((0..100).all(|_| roll(&juggler, &mut rng) == 9));
    }

    #[test]
    fn a_rate_a_second_comes_about_as_often_at_any_frame_rate() {
        let mut rng = Rng::of(0, 0, 3);
        for frames in [30.0_f32, 144.0] {
            let dt = 1.0 / frames;
            // 1,000 seconds' frames, about 297 strolls.
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a whole count of frames"
            )]
            let count = (1_000.0 * frames) as u32;
            let strolls = (0..count)
                .filter(|_| chance(&mut rng, STROLLS_A_SECOND, dt))
                .count();
            assert!((240..360).contains(&strolls), "{frames}: {strolls}");
        }
    }

    #[test]
    fn the_idle_action_steps_about_home_or_turns_about_the_hero() {
        let hero = Vec3::new(1000.0, 0.0, 1000.0);
        let here = Vec3::new(1500.0, 0.0, 1000.0);
        let mut rng = Rng::of(48, 39, 1);
        let script = Mind::new(
            Vec3::new(1480.0, 0.0, 990.0),
            0x1,
            (false, true),
            None,
            SPEEDS,
            (0, 0),
        );
        let fill = Mind::new(here, 0x1, (true, false), None, SPEEDS, (0, 0));
        let (mut steps, mut homes) = (0, 0);
        for _ in 0..2000 {
            if let Some(target) = idle_step(&script, here, hero, &mut rng) {
                if target == script.home {
                    homes += 1;
                } else {
                    let d = target - here;
                    assert!(d.x.abs() <= 150.0 && d.z.abs() <= 150.0, "{d}");
                    steps += 1;
                }
            }
            // An `Enter` function's creature keeps its distance from the hero.
            if let Some(target) = idle_step(&fill, here, hero, &mut rng) {
                let (before, after) = ((here - hero).length(), (target - hero).length());
                assert!((before - after).abs() < 0.01, "{before} {after}");
            }
        }
        // A step in about 22 % of the actions, home in a quarter of them.
        assert!((350..550).contains(&(steps + homes)), "{steps} {homes}");
        assert!(homes > 50 && homes < steps, "{steps} {homes}");
    }
}
