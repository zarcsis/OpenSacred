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
use crate::daynight::{Clock, Phase};
use crate::frame::Step;
use crate::hero::Body;
use crate::npcs::{Door, Plays, PutAt, Talks, Walk};
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
/// How far a townsperson follows its partner, how near they meet, farther than which it walks
/// up to it, and how near it steps off to 220 from it (`0x4EF150` with the flag 4,
/// `0x4FC490`).
const FOLLOWS_WITHIN: f32 = 900.0;
const MEETS: (f32, f32) = (80.0, 200.0);
const CATCHES_UP: f32 = 240.0;
const STEPS_OFF: f32 = 220.0;
const STEP_OFF_TRIES: usize = 10;
/// How near a townsperson the second nearest trader is taken rather than the nearest, and how
/// near the trader it stops (`0x4EE660`, `+0x176`).
const TRADER_WITHIN: f32 = 800.0;
const AT_THE_TRADER: f32 = 150.0;
/// The children's play (`0x4F2AD7`): within 800 of home about itself, farther than 1,000 800
/// toward home, else about home, within 400 along each axis; the other idle children within 600
/// follow the one that plays; one farther than 349 from the hero within a building follows it,
/// one within 200 of it turns to it, and one now and then runs to 150 from it.
const PLAYS_ABOUT: i32 = 400;
const PLAYS_NEAR: f32 = 800.0;
const PLAYS_FAR: f32 = 1_000.0;
const PLAYMATES: f32 = 600.0;
const GATHERS: f32 = 349.0;
const LOOKS_UP: f32 = 200.0;
const RUNS_UP: f32 = 150.0;
/// The cells' flag of a building's inside (`+0x1E` bit 1, docs/02 §5.2).
const INSIDE: u8 = 0x2;
/// How near a hostile of the hero's puts a townsperson in danger, and how far it runs from it
/// without a door to go in by, within 100 more along each axis (`0x4F0BDA`, `0x4F0F5E`).
const DANGER: f32 = 800.0;
const RUNS_FROM: f32 = 700.0;
const RUNS_ASTRAY: i32 = 100;
/// How far a townsperson seeks a door, and the cells about a door it goes in to and out to, in
/// how many tries (`0x4EE300`, `0x4EEFA0`, `0x4F1AB9`).
const DOOR_WITHIN: f32 = 1_500.0;
const INDOORS: (i32, i32) = (2, 9);
const OUTDOORS: (i32, i32) = (-4, 5);
const DOOR_TRIES: usize = 15;
/// How far from a door a townsperson walks toward it by stages, how far a stage goes, within
/// 150 more along each axis, and how far it wanders without a door at dusk (`0x4F0F5E`).
const DOOR_FAR: f32 = 1_000.0;
const DOOR_STAGE: f32 = 700.0;
const DOOR_STAGE_ASTRAY: i32 = 150;
const WANDERS: i32 = 400;
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
    /// The partner it follows (`+0x251`, the flag 4), and whether it runs after it.
    partner: Option<Entity>,
    runs_after: bool,
    /// Whether it is a trader: a smith, a merchant or a wizard (`+0x200` bits `0x1000`, `0x2000`,
    /// `0x4000`, `0x4000000`, `CreateNPC`'s tags `0x23`, `0x24`, `0x25`, `0x64`), which the
    /// townsfolk visit, which does not part and which a meeting does not stop.
    pub trader: bool,
    /// The trader it walks to, at which it talks.
    visiting: Option<Entity>,
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
            partner: None,
            runs_after: false,
            trader: false,
            visiting: None,
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
    TakesPartner,
    Follows,
    StepsOff,
    Meets,
    Trades,
    Plays,
    Gathers,
    GoesIn,
    GoesOut,
    Wanders,
    PutIn,
    Juggles,
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

/// What a townsperson decides by day, by its roll (`0x4F22F0`).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Decision {
    /// It walks to `0` for `Why`, going to the creature `2` if any.
    Walk(Vec3, Why, Option<Entity>),
    /// It takes the creature for its partner, and walks home if it is given.
    Partner(Entity, Option<Vec3>),
    /// It walks to the trader at `1` and talks there.
    Trade(Entity, Vec3),
    /// A child's play.
    Child,
    /// It goes into the nearest house.
    House,
    /// A juggler plays its instrument.
    Juggle,
    Stop,
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
/// 800 (`0x4F0BDA`): without a door to go in by, 700 away from it along the line, within 100 more
/// along each axis (`0x4F0F5E`); `None` out of danger.
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
    let away = (here - threat).with_y(0.0).normalize_or_zero();
    Some(about(here + away * RUNS_FROM, RUNS_ASTRAY, rng))
}

/// What a child does by its roll of 20 (`0x4F2AD7`).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Child {
    /// It runs to the point, and its playmates follow it.
    Plays(Vec3),
    /// It follows the hero at a run.
    FollowsHero,
    /// It turns to the hero and stands.
    LooksUp,
    /// It runs to the point by the hero.
    RunsUp(Vec3),
}

/// What a child at `here` does by its roll, the hero at `hero`, `inside` a building (`0x4F2AD7`):
/// in one case of 13, or with the hero outside, it plays: within 800 of home a point within 400
/// of itself, farther than 1,000 a point 800 toward home, else one within 400 of home; else it
/// follows the hero from farther than 349, and nearer turns to it, but in one case of 15 runs to
/// a point 150 from it at random.
fn child(mind: &Mind, here: Vec3, hero: Vec3, inside: bool, rng: &mut Rng) -> Child {
    if rng.below(13) == 0 || !inside {
        let from_home = (mind.home - here).xz().length();
        let to = if from_home <= PLAYS_NEAR {
            about(here, PLAYS_ABOUT, rng)
        } else if from_home > PLAYS_FAR {
            here + (mind.home - here).with_y(0.0).normalize_or_zero() * PLAYS_NEAR
        } else {
            about(mind.home, PLAYS_ABOUT, rng)
        };
        return Child::Plays(to);
    }
    let d = (hero - here).xz().length();
    if d > GATHERS {
        Child::FollowsHero
    } else if (rng.below(9) == 0 && d < LOOKS_UP) || rng.below(15) != 0 {
        Child::LooksUp
    } else {
        let degrees = f32::from(u16::try_from(rng.below(360)).unwrap_or(0));
        Child::RunsUp(hero + Quat::from_rotation_y(degrees.to_radians()) * (Vec3::X * RUNS_UP))
    }
}

/// What a townsperson standing does when it decides by day, by its roll (`0x4F22F0`): 0 takes
/// the nearest human, `human`, for its partner, 2 too and walks home, 1 walks to the nearest
/// animal, `animal`, 3 walks home, 5 and 8 visit the trader `trader`, one of 8 in one case of
/// three only (in the other two its field's branch finds no field, `0x4F2643`); 4, 6 and 7 go
/// into a house, 9 plays an instrument and 20 a child's play, which wait for their features
/// (#248, #253, #252); the rest stop.
fn townsperson(
    mind: &Mind,
    human: Option<Entity>,
    animal: Option<(Entity, Vec3)>,
    trader: Option<(Entity, Vec3)>,
    rng: &mut Rng,
) -> Decision {
    match (roll(mind, rng), human, animal) {
        (0, Some(human), _) => Decision::Partner(human, None),
        (2, Some(human), _) => Decision::Partner(human, Some(mind.home)),
        (1, _, Some((entity, at))) => Decision::Walk(at, Why::Visits, Some(entity)),
        (2 | 3, ..) => Decision::Walk(mind.home, Why::GoesHome, None),
        (4 | 6 | 7, ..) => Decision::House,
        (9, ..) => Decision::Juggle,
        // An adult's 20 stops (`0x4F231D`).
        (20, ..) if CHILDREN.contains(&mind.creature) => Decision::Child,
        (5 | 8, ..) => {
            let eight = mind.flags & (TOWNSFOLK | TOWNSFOLK_TOO) != 0;
            match trader {
                Some((entity, at)) if !eight || rng.below(3) == 0 => Decision::Trade(entity, at),
                _ => Decision::Stop,
            }
        }
        _ => Decision::Stop,
    }
}

/// The trader a townsperson at `here` visits among `traders` (`0x4EE660`): the second nearest
/// when it is within 800, else the nearest.
fn trader_for(here: Vec3, mut traders: Vec<(Entity, Vec3)>) -> Option<(Entity, Vec3)> {
    let d = |at: Vec3| (at - here).xz().length();
    traders.sort_by(|a, b| d(a.1).total_cmp(&d(b.1)));
    match traders.as_slice() {
        [_, second, ..] if d(second.1) <= TRADER_WITHIN => Some(*second),
        [nearest, ..] => Some(*nearest),
        [] => None,
    }
}

/// What a creature standing at `here` does about its partner at `partner` (`0x4EF150` with the
/// flag 4, `0x4FC490`): farther than 900 it forgets it; between 80 and 200 it meets it and
/// forgets it; farther than 240 it walks up to it; within 80 it steps off to 220 from it; else
/// it forgets it.
fn follow(here: Vec3, partner: Vec3) -> Why {
    let d = (partner - here).xz().length();
    if d > FOLLOWS_WITHIN {
        Why::GoesBack
    } else if d > MEETS.0 && d < MEETS.1 {
        Why::Meets
    } else if d > CATCHES_UP {
        Why::Follows
    } else if d <= MEETS.0 {
        Why::StepsOff
    } else {
        Why::GoesBack
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
    doors: Query<&Transform, (With<Door>, Without<Mind>)>,
    clock: Res<Clock>,
    mut creatures: Query<(Entity, &mut Transform, &InTheWay, &mut Mind, Has<Walk>), Without<Body>>,
) {
    let Some((the_hero, hero)) = hero.iter().next() else {
        return;
    };
    let hero = hero.translation;
    let dt = time.delta_secs();
    // Whether the hero stands within a building, where the children gather about it.
    let inside = heights
        .flags(paths::cell_of(hero.x, hero.z))
        .is_some_and(|flags| flags & INSIDE != 0);
    // The children at play this frame, whom the others follow.
    let mut playing: Vec<(Entity, Vec3)> = Vec::new();
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
            of(|mind| mind.class == 6 && mind.flags & (WILD | WILD_TOO | FARM | FARM_LOOSE) != 0),
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
    let doors: Vec<Vec3> = if minds.looked {
        doors
            .iter()
            .map(|transform| transform.translation)
            .collect()
    } else {
        Vec::new()
    };
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
        // At the trader it walked to, it talks once (the sub-states 12 and 13), and would then
        // go into a house (#248).
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
            // A child follows the hero only while it stands within a building.
            let after_hero = partner == the_hero;
            let found = if after_hero {
                inside.then_some((hero, false, false))
            } else {
                followed.get(&partner).copied()
            };
            let Some((at, partner_walks, partner_trades)) = found else {
                mind.partner = None;
                continue;
            };
            // The hero, a player, is never left behind for its distance.
            let why = match follow(here, at) {
                Why::GoesBack if after_hero && (at - here).xz().length() > FOLLOWS_WITHIN => {
                    Why::Follows
                }
                why => why,
            };
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
                    let target = (0..STEP_OFF_TRIES)
                        .map(|_| {
                            let degrees =
                                f32::from(u16::try_from(minds.rng.below(360)).unwrap_or(0));
                            let turn = Quat::from_rotation_y(degrees.to_radians());
                            at + turn * Vec3::new(STEPS_OFF, 0.0, 0.0)
                        })
                        .find(|&to| {
                            let cell = paths::cell_of(to.x, to.z);
                            heights.walkable_on(cell, way.level)
                                && !crowd.stops(cell, way.level, entity, false)
                        });
                    match target {
                        Some(target) => {
                            walks.push((entity, Walk::to(target, way.level, mind.speed)))
                        }
                        None => mind.partner = None,
                    }
                }
                Why::Meets => {
                    // They meet: an adult, in one case of two, turns to its partner, which stops
                    // and turns back, and talks.
                    mind.last = Some(partner);
                    mind.partner = None;
                    if !CHILDREN.contains(&mind.creature) && minds.rng.below(2) == 0 {
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
        let folk = mind.flags & TOWNSFOLKS != 0;
        let range = if folk {
            TOWNSFOLK_RANGE * zoom
        } else {
            BEHAVES_WITHIN
        };
        if mind.active && (here - hero).xz().length() > range {
            mind.active = false;
            // At dusk and by night one on the ground goes dormant in a house: by the nearest door
            // within 1,500 of it, put there at once (`0x4F0ADC`, `0x4EEFA0`).
            if folk && evening && way.level.is_none() {
                let door = doors
                    .iter()
                    .copied()
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
            let door = doors
                .iter()
                .copied()
                .filter(|&at| {
                    let (ax, ay) = sector(at);
                    (ax - x).abs() <= 1.0
                        && (ay - y).abs() <= 1.0
                        && (at - here).xz().length() <= DOOR_WITHIN
                })
                .min_by(|a, b| {
                    (*a - here)
                        .length_squared()
                        .total_cmp(&(*b - here).length_squared())
                });
            // At dusk and by night, on a free cell, it takes the evening's way (`0x4F0F5E`):
            // indoors it walks among the building's cells; outdoors it goes into the house of
            // the nearest door, by stages from afar, and without a door it wanders.
            if evening && !crowded {
                let stage = if way.level.is_some() {
                    indoors(here, &heights, &mut minds.rng).map(|(to, _)| (to, Why::Wanders))
                } else if let Some(door) = door {
                    if (door - here).xz().length() > DOOR_FAR {
                        let toward = (door - here).with_y(0.0).normalize_or_zero();
                        let to = about(
                            here + toward * DOOR_STAGE,
                            DOOR_STAGE_ASTRAY,
                            &mut minds.rng,
                        );
                        Some((to, Why::GoesIn))
                    } else {
                        indoors(door, &heights, &mut minds.rng).map(|(to, _)| (to, Why::GoesIn))
                    }
                } else {
                    Some((about(here, WANDERS, &mut minds.rng), Why::Wanders))
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
            let (human, animal) = (nearest(&humans).map(|(e, _)| e), nearest(&animals));
            let near_traders = traders
                .iter()
                .filter(|&&(other, at)| {
                    let (ax, ay) = sector(at);
                    other != entity && (ax - x).abs() <= 1.0 && (ay - y).abs() <= 1.0
                })
                .copied()
                .collect();
            let trader = trader_for(here, near_traders);
            match townsperson(&mind, human, animal, trader, &mut minds.rng) {
                Decision::Walk(target, why, whom) => {
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
                Decision::Partner(partner, home) => {
                    debug!("ai: {entity} {:?} {partner}", Why::TakesPartner);
                    mind.partner = Some(partner);
                    mind.runs_after = false;
                    if let Some(home) = home {
                        walks.push((entity, Walk::to(home, way.level, mind.speed)));
                    }
                    continue;
                }
                Decision::Trade(trader, at) => {
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
                    let does = child(&mind, here, hero, inside, &mut minds.rng);
                    debug!("ai: {entity} {:?} {does:?}", Why::Plays);
                    match does {
                        Child::Plays(to) => {
                            walks.push((entity, Walk::run(to, way.level, mind.run)));
                            playing.push((entity, here));
                        }
                        Child::FollowsHero => {
                            mind.partner = Some(the_hero);
                            mind.runs_after = true;
                        }
                        Child::LooksUp => turns.push((entity, hero)),
                        Child::RunsUp(to) => {
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
                        walks.push((entity, Walk::to(target, way.level, mind.speed)));
                        continue;
                    }
                }
                Decision::Juggle => {
                    // Its instrument, if its hands are empty, at random (`0x4F29E2`).
                    let instrument = usize::try_from(minds.rng.below(4)).unwrap_or(0);
                    let on = mind.flags & JUGGLER != 0;
                    debug!("ai: {entity} {:?} {instrument} on {on}", Why::Juggles);
                    commands.entity(entity).insert(Plays { instrument, on });
                    continue;
                }
                Decision::Stop => {}
            }
        }
        if !mind.active || folk {
            if crowded
                && mind.flags & STAYS == 0
                && !mind.dialogue
                && !mind.trader
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
    let sent: std::collections::HashSet<Entity> = walks.iter().map(|&(entity, _)| entity).collect();
    for (entity, walk) in walks {
        commands.entity(entity).insert(walk);
    }
    for entity in stops {
        commands.entity(entity).remove::<Walk>();
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
                && CHILDREN.contains(&mind.creature)
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
        let wife = Entity::from_raw_u32(8).unwrap();
        let (mut partners, mut walking, mut houses) = (0, 0, 0);
        for _ in 0..26_000 {
            match townsperson(&farmer, Some(wife), Some(pig), None, &mut rng) {
                Decision::Walk(to, Why::GoesHome, None) if to == home => homes += 1,
                Decision::Walk(to, Why::Visits, Some(_)) if to == pig.1 => visits += 1,
                Decision::Partner(partner, home) if partner == wife => {
                    partners += 1;
                    walking += usize::from(home.is_some());
                }
                Decision::House => houses += 1,
                Decision::Stop => stops += 1,
                other => panic!("{other:?}"),
            }
        }
        // Home in one roll of 26, the animal in one, a partner in two, one of them walking home.
        assert!((800..1_200).contains(&homes), "{homes}");
        assert!((800..1_200).contains(&visits), "{visits}");
        assert!((1_700..2_300).contains(&partners) && (800..1_200).contains(&walking));
        // A house in three rolls of 26.
        assert!((2_700..3_300).contains(&houses), "{houses}");
        assert_eq!(homes + visits + partners + houses + stops, 26_000);
        // Without a human or an animal those rolls walk home or stop.
        assert!((0..100).all(|_| !matches!(
            townsperson(&farmer, None, None, None, &mut rng),
            Decision::Partner(..) | Decision::Walk(_, Why::Visits, _)
        )));
        // Without an animal the roll of 1 stops; a child always rolls 20, a juggler 9.
        assert!((0..100).all(|_| roll(&farmer, &mut rng) != 9));
        let child = Mind::new(home, TOWNSFOLK, (true, true), None, SPEEDS, (702, 3));
        assert!((0..100).all(|_| roll(&child, &mut rng) == 20));
        let juggler = Mind::new(home, 0, (true, true), None, SPEEDS, (78, 3));
        assert_eq!(juggler.flags, JUGGLER);
        assert!((0..100).all(|_| roll(&juggler, &mut rng) == 9));
        assert!((0..100).all(|_| {
            townsperson(&juggler, Some(wife), Some(pig), None, &mut rng) == Decision::Juggle
        }));
    }

    #[test]
    fn a_townsperson_visits_the_second_nearest_trader_within_800() {
        let home = Vec3::ZERO;
        let smith = (Entity::from_raw_u32(9).unwrap(), Vec3::new(300.0, 0.0, 0.0));
        let wizard = (
            Entity::from_raw_u32(10).unwrap(),
            Vec3::new(700.0, 0.0, 0.0),
        );
        let merchant = (
            Entity::from_raw_u32(11).unwrap(),
            Vec3::new(900.0, 0.0, 0.0),
        );
        assert_eq!(
            trader_for(home, vec![wizard, smith, merchant]),
            Some(wizard)
        );
        assert_eq!(trader_for(home, vec![merchant, smith]), Some(smith));
        assert_eq!(trader_for(home, vec![merchant]), Some(merchant));
        assert_eq!(trader_for(home, Vec::new()), None);
        // A citizen visits it in two rolls of 26, a farmer of the flag 8 in a third of those.
        let count = |flags| {
            let mind = Mind::new(home, flags, (true, true), None, SPEEDS, (652, 3));
            let mut rng = Rng::of(50, 39, 8);
            (0..26_000)
                .filter(|_| {
                    matches!(
                        townsperson(&mind, None, None, Some(smith), &mut rng),
                        Decision::Trade(..)
                    )
                })
                .count()
        };
        assert!(
            (1_700..2_300).contains(&count(CITIZEN)),
            "{}",
            count(CITIZEN)
        );
        assert!(
            (500..850).contains(&count(TOWNSFOLK)),
            "{}",
            count(TOWNSFOLK)
        );
    }

    #[test]
    fn a_child_plays_outside_and_gathers_about_the_hero_in_a_building() {
        let mut rng = Rng::of(50, 39, 9);
        let home = Vec3::ZERO;
        let kid = Mind::new(home, TOWNSFOLK, (true, true), None, SPEEDS, (702, 3));
        let hero = Vec3::new(1000.0, 0.0, 0.0);
        // Outside, it plays about itself near home, toward home from afar.
        for _ in 0..50 {
            let Child::Plays(to) = child(&kid, home, hero, false, &mut rng) else {
                panic!("plays");
            };
            assert!(to.x.abs() <= 400.0 && to.z.abs() <= 400.0, "{to}");
            let far = Vec3::new(2000.0, 0.0, 0.0);
            let Child::Plays(to) = child(&kid, far, hero, false, &mut rng) else {
                panic!("plays");
            };
            assert!((to - Vec3::new(1200.0, 0.0, 0.0)).length() < 0.01, "{to}");
        }
        // Inside, from afar it follows the hero (but in one case of 13 it plays).
        let follows = (0..1300)
            .filter(|_| child(&kid, home, hero, true, &mut rng) == Child::FollowsHero)
            .count();
        assert!((1_150..1_250).contains(&follows), "{follows}");
        // Near, it mostly turns to the hero.
        let near = Vec3::new(100.0, 0.0, 0.0);
        let looks = (0..1300)
            .filter(|_| child(&kid, home, near, true, &mut rng) == Child::LooksUp)
            .count();
        assert!((1_050..1_250).contains(&looks), "{looks}");
    }

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
    fn a_townsperson_follows_its_partner_and_meets_it() {
        let here = Vec3::ZERO;
        let at = |d: f32| Vec3::new(d, 0.0, 0.0);
        assert_eq!(follow(here, at(950.0)), Why::GoesBack);
        assert_eq!(follow(here, at(500.0)), Why::Follows);
        assert_eq!(follow(here, at(220.0)), Why::GoesBack);
        assert_eq!(follow(here, at(150.0)), Why::Meets);
        assert_eq!(follow(here, at(81.0)), Why::Meets);
        assert_eq!(follow(here, at(60.0)), Why::StepsOff);
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
