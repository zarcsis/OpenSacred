//! The creatures' idle life while no foe is near (docs/re/ai.md): each is dormant until its own
//! timer or the hero's look wakes it, and dormant again farther than 1,500 from the hero; a
//! hostile creature in the hero's reach steps about by the idle action (`0x54EA30`), a group's
//! members together. The fight is not followed: an awake hostile creature, which would attack
//! the hero by a test of the levels, goes dormant again and steps about as one beyond it
//! (docs/decisions/2026-10-20-idle.md).

use bevy::prelude::*;

use crate::crowd::InTheWay;
use crate::frame::Step;
use crate::hero::Body;
use crate::npcs::Walk;
use crate::spawning::Rng;

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
    /// Its walk speed in world units a second, before the game's speed.
    pub speed: f32,
    awake: bool,
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
        speed: f32,
    ) -> Self {
        let hostile = flags & 0x3 != 0;
        Self {
            home,
            flags,
            enter,
            group,
            speed,
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

/// The game's generator for the creatures' chances, and the time since the hero last looked.
#[derive(Resource)]
struct Minds {
    rng: Rng,
    since: f32,
}

impl Default for Minds {
    fn default() -> Self {
        Self {
            rng: Rng::of(0, 0, 0),
            since: 0.0,
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

/// The creatures' idle life.
pub struct AiPlugin;

impl Plugin for AiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Minds>()
            .add_systems(Update, think.in_set(Step::Crowd));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hostile_creature_of_a_script_wakes_as_it_is_made_and_looks_every_2_5_s() {
        let home = Vec3::ZERO;
        let script = Mind::new(home, 0x1, (false, true), None, 70.0);
        assert!(script.awake && script.period == PERIOD_HOSTILE);
        // A fill's hostile creature and a peaceful one sleep until woken, on a timer of 5 s.
        let fill = Mind::new(home, 0x1, (true, false), Some(7), 70.0);
        assert!(!fill.awake && fill.period == PERIOD);
        let villager = Mind::new(home, 0x400, (true, true), None, 70.0);
        assert!(!villager.awake && !villager.hostile());
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
            70.0,
        );
        let fill = Mind::new(here, 0x1, (true, false), None, 70.0);
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
