//! The archers' arrows (#391; docs/re/fight.md, "The archers"): a shot's arrow, `ARROW_LONG.GRN`
//! (type 3070), leaves its shooter's left hand (`Bip01 L Hand`) toward its target's neck
//! (`Bip01 Neck`, `0x44CBB0`), flies straight at 600 a second for 1.5 s, ends on a cell that may
//! not be walked on but the ground's nibble 2 or at the ground, and in each frame hits its target,
//! or the first creature, nearer its way than the sum of the two sizes, alive and hostile to the
//! shooter (`0x5BB010`): the hit is the fight's ([`fight::Hit`], at range), and the arrow sticks
//! in a creature of a size above 20 for 5 s, but not in the hero (`0x5BAB40`). The rules are
//! `opensacred_rules::arrows`. A creature's arrow hits the hero when its shooter is hostile to
//! the hero, and a creature of a side or type hostile to its shooter's (`0x548F60`, #670): the
//! arrows of the archers that fight a creature, and those that a creature comes in the way of.
//! The hero's shot of a special arrow ([`Special`], the Wood Elf's 1004, 1007 and 1008) looses the
//! art's own arrows, whose hits are the art's strikes, the blades homing on their foes for a time
//! (docs/re/arts.md, "The Wood Elf's", #756).

use bevy::prelude::*;
use opensacred_rules::arrows::{self, Point};
pub use opensacred_rules::arrows::{ARROW, size};
use opensacred_rules::minds::{self, Spot};

use crate::crowd::InTheWay;
use crate::fight::{self, Health};
use crate::frame::{Playing, Step};
use crate::game::AppState;
use crate::hero::Body;
use crate::npcs::{Dies, Npcs, Retexture};
use crate::terrain::{Heights, Level};
use crate::{ai, iso, models, paths};

/// The bones an arrow leaves from and flies toward: the shooter's left hand (`0x44CA30`) and
/// the target's bone of kind 3, its neck, else its root (the table of bones at `0x8E7200`).
const HAND: &str = "Bip01 L Hand";
const NECK: &str = "Bip01 Neck";
/// How high over its feet an arrow leaves a shooter whose model has no left hand, in world
/// units (**hypothesis**: about a hand's height).
const HAND_HEIGHT: f32 = 80.0;

/// A shot's arrow leaving its shooter's hand toward its target (`cEventArrow`, the event 9,
/// `0x4EB700`), which the shot's clip sends at its fraction (`npcs::Strikes`), and the combat art
/// it is a shot of, if any.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Loose {
    pub shooter: Entity,
    pub target: Entity,
    pub special: Option<Special>,
}

/// The combat art that a shot is of (the shooter's `+0x18E`, the art less 1000): its id, its
/// level in play, which the arrow carries (`+0x249`), its factor at that level, which the arrow's
/// strike carries (the record's `+0x1C`, `0x5BB010`), and for the blades 1008 how long they fly
/// (`0x5BD2D0`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Special {
    pub art: u16,
    pub level: u32,
    pub factor: f32,
    pub seconds: Option<f32>,
}

/// A homing blade's own (1008, `+0x1E4` bit `0x10`): how high over its target's feet it aims
/// (the target's neck where it left), and how long until it may strike again (`+0x240`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Homing {
    aim: f32,
    wait: f32,
}

/// A creature's size for the arrows' hits (`items.pak +0x14`, 50 for 0,
/// `opensacred_rules::arrows::size`); one without, as the hero, is of 50, the heroes' own.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Size(pub f32);

/// An arrow in flight: its flight, its shooter, its target, the level it flies on, its size, the
/// share of its hit (1, an arrowproof's value for one it sent back, `0x561090`'s `+0x230`), and
/// the combat art it was shot by.
#[derive(Component)]
struct Arrow {
    flight: arrows::Arrow,
    shooter: Entity,
    target: Entity,
    level: Level,
    size: f32,
    share: f32,
    special: Option<Special>,
    homing: Option<Homing>,
}

/// The dice of the arrows' deflection (`0x561090`'s `rand`).
#[derive(Resource)]
struct Dice(opensacred_rules::spawning::Rng);

/// An arrow stuck in a creature, and how long it stays there, in seconds.
#[derive(Component)]
struct Stuck(f32);

/// The rules' point of world point `at`.
fn point(at: Vec3) -> Point {
    Point::new(Spot::new(at.x, at.z), at.y)
}

/// The world point of the rules' point `point`.
fn world(point: Point) -> Vec3 {
    Vec3::new(point.ground.x, point.height, point.ground.y)
}

/// The turn of the arrow's model, which points along its +Z, toward the velocity `velocity`.
fn pointing(velocity: Point) -> Quat {
    let direction = world(velocity).normalize_or_zero();
    if direction == Vec3::ZERO {
        Quat::IDENTITY
    } else {
        Quat::from_rotation_arc(Vec3::Z, direction)
    }
}

/// Whether an arrow of `shooter` hits what it comes on: the hero, alive when `hero` holds
/// `Some(true)`, when the shooter is hostile to it; a creature, alive when `other` holds its mind,
/// when the shooter is hostile to its side or type (`0x548F60`).
fn lands(shooter: &minds::Mind, hero: Option<bool>, other: Option<&minds::Mind>) -> bool {
    match hero {
        Some(alive) => alive && shooter.hostile(),
        None => other.is_some_and(|other| shooter.hostile_to(other)),
    }
}

/// Makes the arrow of each shot that leaves (`0x4EB700`): from the shooter's left hand, or its
/// place at a hand's height, toward its target's neck, or its place, on the shooter's level.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn loose(
    mut commands: Commands,
    mut shots: MessageReader<Loose>,
    npcs: Option<Res<Npcs>>,
    rules: Option<Res<fight::Rules>>,
    effects: Query<&crate::effects::Effects, With<Body>>,
    assets: Res<AssetServer>,
    children: Query<&Children>,
    names: Query<&Name>,
    places: Query<&GlobalTransform>,
    ways: Query<&InTheWay>,
) {
    let Some(npcs) = npcs else {
        shots.clear();
        return;
    };
    for shot in shots.read() {
        let (Ok(shooter), Ok(target)) = (places.get(shot.shooter), places.get(shot.target)) else {
            continue;
        };
        let bone = |root: Entity, name: &str| {
            children
                .iter_descendants(root)
                .find(|&e| names.get(e).is_ok_and(|n| n.as_str() == name))
                .and_then(|e| places.get(e).ok())
                .map(GlobalTransform::translation)
        };
        let from =
            bone(shot.shooter, HAND).unwrap_or(shooter.translation() + Vec3::Y * HAND_HEIGHT);
        let toward = bone(shot.target, NECK).unwrap_or(target.translation());
        let first = arrows::Arrow::new(point(from), point(toward));
        let level = ways.get(shot.shooter).ok().and_then(|way| way.level);
        let art = shot.special.map(|special| special.art);
        let Some((model, texture, size)) = npcs.arrow(arrows::projectile(art)) else {
            continue;
        };
        // A hero with the multi-shot looses as many arrows as its whole value, fanned, each of
        // `Multipfeil` to the power of their number (`0x4DD90E`, `0x56D173`).
        let count = arrows::volley(effects.get(shot.shooter).ok().and_then(|effects| {
            effects
                .list()
                .into_iter()
                .find(|effect| effect.id == arrows::MULTI_SHOT)
                .map(|effect| effect.whole)
        }));
        let share = rules
            .as_ref()
            .map_or(1.0, |rules| rules.0.multi_shot(count));
        // A special arrow's strike carries its art's factor, which a factor of 0 leaves out.
        let share = shot.special.map_or(share, |special| {
            opensacred_rules::fight::scaled(share, special.factor)
        });
        // The blades home on their target for their time (`0x5BD2D0`).
        let (first, homing) = match shot.special.and_then(|special| special.seconds) {
            Some(seconds) => (
                arrows::Arrow::homing(point(from), point(toward), seconds),
                Some(Homing {
                    aim: toward.y - target.translation().y,
                    wait: 0.0,
                }),
            ),
            None => (first, None),
        };
        for i in 0..count {
            let flight = first.turned_by(arrows::fanned_for(i, count, art));
            debug!(
                "arrows: {} looses an arrow at {} from {:.0},{:.0},{:.0} toward {:.0},{:.0},{:.0}",
                shot.shooter, shot.target, from.x, from.y, from.z, toward.x, toward.y, toward.z
            );
            let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(model.to_owned()));
            let mut arrow = commands.spawn((
                WorldAssetRoot(scene),
                Transform::from_translation(from)
                    .with_rotation(pointing(flight.velocity()))
                    .with_scale(Vec3::splat(iso::MODEL_SCALE)),
                models::Ordered::default(),
                Arrow {
                    flight,
                    shooter: shot.shooter,
                    target: shot.target,
                    level,
                    size,
                    share,
                    special: shot.special,
                    homing,
                },
                DespawnOnExit(AppState::InGame),
            ));
            if let Some(texture) = texture {
                arrow.insert(Retexture::load(&assets, texture));
            }
        }
    }
}

/// Flies each arrow a frame on (`0x5BB010`): it ends where it may not fly on and is gone when its
/// time is out; it comes on the hero, its target or the first creature near the part of its way it
/// flew, and hits it when it is alive and hostile to the shooter, alive too (the hero to a shooter
/// hostile to it, a creature to one of a hostile side or type), sending the fight's hit at range
/// with the shooter's values (`fight::Hit`, the event 5 with the bow), and is gone, or sticks in a
/// creature of a size above 20; else it flies on. A special arrow's hit is of its art at its
/// level, and the club arrow's may first knock its creature back (the event 3). A homing blade
/// turns toward its target first and ends when the target is gone or dead; it passes through
/// what it hits, striking at most every 0.5 s, and after each strike takes the nearest other
/// creature hostile to the hero within 316 of the one it struck, else keeps its target.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    heights: Res<Heights>,
    mut hits: MessageWriter<fight::Hit>,
    mut dice: ResMut<Dice>,
    mut flying: Query<(Entity, &mut Arrow, &mut Transform)>,
    shooters: Query<(&ai::Mind, &Health), Without<Dies>>,
    hero: Query<
        (
            Entity,
            &Transform,
            &Body,
            &Health,
            Option<&crate::effects::Effects>,
        ),
        (Without<Arrow>, Without<Dies>),
    >,
    creatures: Query<
        (Entity, &Transform, &InTheWay, Option<&Size>),
        (Without<Body>, Without<Arrow>),
    >,
    places: Query<&GlobalTransform>,
    (experience, tables): (Res<fight::Experience>, Option<Res<crate::arts::HeroArts>>),
) {
    let dt = time.delta_secs();
    let hero = hero.iter().next();
    for (entity, mut arrow, mut transform) in &mut flying {
        let level = arrow.level;
        if let Some(homing) = arrow.homing.as_mut() {
            homing.wait = (homing.wait - dt).max(0.0);
        }
        // A blade turns toward its target alive, and ends without one (`0x5BB010`).
        if let Some(homing) = arrow.homing {
            let target = shooters
                .get(arrow.target)
                .ok()
                .filter(|(_, health)| health.current > 0)
                .and_then(|_| places.get(arrow.target).ok());
            let Some(target) = target else {
                debug!("arrows: {entity} of {} has no target", arrow.shooter);
                commands.entity(entity).despawn();
                continue;
            };
            let toward = point(target.translation() + Vec3::Y * homing.aim);
            let walkable = |p: Point| {
                let at = world(p);
                let cell = paths::cell_of(at.x, at.z);
                heights.walkable_on(cell, level) || heights.flown_over(cell, level)
            };
            let ground = |p: Point| {
                let at = world(p);
                heights.standing(at.x, at.z, level)
            };
            arrow.flight.home(dt, toward, walkable, ground);
            transform.rotation = pointing(arrow.flight.velocity());
        }
        let open = |p: Point| {
            let at = world(p);
            let cell = paths::cell_of(at.x, at.z);
            arrows::open(
                heights.walkable_on(cell, level),
                heights.flown_over(cell, level),
                p.height,
                heights.standing(at.x, at.z, level),
            )
        };
        let Some(way) = arrow.flight.fly(dt, open) else {
            let at = transform.translation;
            debug!(
                "arrows: {entity} of {} ends at {:.0},{:.0},{:.0}",
                arrow.shooter, at.x, at.y, at.z
            );
            commands.entity(entity).despawn();
            continue;
        };
        transform.translation = world(arrow.flight.at());
        // The creatures on its level, and the hero, but its shooter.
        let near = hero
            .iter()
            .filter(|&&(e, _, body, ..)| body.level() == level && e != arrow.shooter)
            .map(|&(e, at, ..)| (e, Spot::new(at.translation.x, at.translation.z), 50.0))
            .chain(
                creatures
                    .iter()
                    .filter(|&(e, _, way, _)| {
                        e != arrow.shooter && way.creature && way.level == level
                    })
                    .map(|(e, at, _, size)| {
                        let size = size.map_or(arrows::size(0), |size| size.0);
                        (e, Spot::new(at.translation.x, at.translation.z), size)
                    }),
            );
        let Some(on) = arrows::picks(way, arrow.size, near, arrow.target) else {
            continue;
        };
        // The hero, alive, to a shooter hostile to it; a creature, alive, to one of a hostile side
        // or type; the shooter alive. One the hero sent back hits a creature hostile to the hero.
        let hero_shot = hero.is_some_and(|(e, ..)| e == arrow.shooter);
        let hero_hit = hero.is_some_and(|(e, ..)| e == on);
        let other = shooters.get(on).ok().map(|(other, _)| &other.thinks);
        let shooter = shooters.get(arrow.shooter).ok().map(|(mind, _)| mind);
        let hits_it = match (hero_shot, shooter) {
            (true, _) => other.is_some_and(minds::Mind::hostile),
            (false, Some(shooter)) => {
                let the_hero = hero
                    .filter(|_| hero_hit)
                    .map(|(_, _, _, health, _)| health.current > 0);
                lands(&shooter.thinks, the_hero, other)
            }
            (false, None) => false,
        };
        if !hits_it {
            continue;
        }
        // A blade strikes at most every half second, passing through.
        if arrow.homing.is_some_and(|homing| homing.wait > 0.0) {
            continue;
        }
        // A hero with the effect of deflection may turn it aside by its chance (`0x561090`).
        if hero_hit
            && let Some((hero_entity, at, _, _, Some(effects))) = hero
            && let Some(ward) = effects
                .list()
                .into_iter()
                .find(|e| e.id == arrows::DEFLECTION)
        {
            let shooter_level = shooter.map_or(0, |shooter| shooter.thinks.level);
            let chance = arrows::deflection_chance(ward.level, ward.art, shooter_level);
            let roll = f32::from(u16::try_from(dice.0.below(32_768)).unwrap_or(0)) / 32_767.0;
            if roll <= chance {
                let random = |dice: &mut Dice| {
                    let mut unit = || {
                        f32::from(u16::try_from(dice.0.below(32_768)).unwrap_or(0)) / 32_767.0 * 2.0
                            - 1.0
                    };
                    Point::new(Spot::new(unit(), unit()), 0.0)
                };
                let deflected = arrows::deflected(ward.art);
                debug!(
                    "arrows: {entity} of {} turned aside by the hero ({deflected:?}, {:.0} %)",
                    arrow.shooter,
                    chance * 100.0
                );
                match deflected {
                    arrows::Deflected::Back => {
                        let old = arrow.shooter;
                        let direction = match places.get(old) {
                            Ok(to) => {
                                let d = to.translation() - at.translation;
                                Point::new(Spot::new(d.x, d.z), d.y)
                            }
                            Err(_) => random(&mut dice),
                        };
                        arrow.flight.turn(direction);
                        arrow.shooter = hero_entity;
                        arrow.target = old;
                        arrow.share = ward.value;
                        transform.rotation = pointing(arrow.flight.velocity());
                    }
                    arrows::Deflected::Away => {
                        let direction = random(&mut dice);
                        arrow.flight.turn(direction);
                        arrow.shooter = hero_entity;
                        transform.rotation = pointing(arrow.flight.velocity());
                    }
                    arrows::Deflected::Gone => commands.entity(entity).despawn(),
                    arrows::Deflected::Passes => {}
                }
                continue;
            }
        }
        let distance = match (places.get(arrow.shooter), places.get(on)) {
            (Ok(a), Ok(b)) => (a.translation() - b.translation()).xz().length(),
            _ => 0.0,
        };
        debug!(
            "arrows: {entity} of {} hits {on} {distance:.0} from its shooter",
            arrow.shooter
        );
        // The club arrow knocks back a creature below its bound whose type a kick may move, by
        // the event 3 of its record's value, before its strike (`0x5BB010`).
        if let Some(special) = arrow.special.filter(|special| special.art == 1007)
            && let Some(mind) = other
            && arrows::club_knocks(mind.level, experience.level, special.level)
            && crate::making::may_knock(special.level, mind)
            && let Some(record) = tables.as_ref().and_then(|tables| {
                tables
                    .tables
                    .weapon_arts
                    .iter()
                    .find(|a| a.id == special.art)
            })
            && let Ok(from) = places.get(arrow.shooter)
        {
            let left = opensacred_rules::arts::club_knockback(record, special.level);
            debug!(
                "arrows: {entity} of {} knocks {on} back by {left}",
                arrow.shooter
            );
            commands
                .entity(on)
                .remove::<(crate::npcs::Walk, crate::npcs::Strikes)>()
                .insert(crate::making::Knocked {
                    by: arrow.shooter,
                    from: from.translation(),
                    left: f32::from(u16::try_from(left).unwrap_or(u16::MAX)),
                    held: 0.0,
                });
        }
        hits.write(fight::Hit {
            fixed: None,
            attacker: arrow.shooter,
            target: on,
            share: arrow.share,
            ranged: true,
            art: arrow.special.map(|special| special.level),
        });
        // A blade flies on, its next target the nearest other foe within its reach of the one it
        // struck, else the one it had (`0x600570`).
        if let Some(homing) = arrow.homing.as_mut() {
            homing.wait = arrows::BLADE_STRIKES_EVERY;
            if let Ok(struck) = places.get(on) {
                let struck = struck.translation();
                let current = arrow.target;
                let next = creatures
                    .iter()
                    .filter(|&(e, _, way, _)| e != current && way.creature && way.level == level)
                    .filter(|&(e, ..)| {
                        shooters
                            .get(e)
                            .is_ok_and(|(mind, health)| mind.hostile() && health.current > 0)
                    })
                    .map(|(e, at, ..)| (e, (at.translation - struck).xz().length()))
                    .filter(|&(_, distance)| distance < arrows::BLADE_NEXT_WITHIN)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(e, _)| e);
                arrow.target = next.unwrap_or(current);
            }
            continue;
        }
        let size = creatures
            .get(on)
            .ok()
            .and_then(|(.., size)| size.map(|size| size.0))
            .unwrap_or(arrows::size(0));
        match places.get(on) {
            Ok(holder) if arrows::sticks(size, hero_hit) => {
                let stuck = GlobalTransform::from(*transform).reparented_to(holder);
                commands.entity(entity).remove::<Arrow>().insert((
                    Stuck(arrows::STICKS_FOR),
                    stuck,
                    ChildOf(on),
                ));
            }
            _ => commands.entity(entity).despawn(),
        }
    }
}

/// Takes each stuck arrow away when its time is out.
fn unstick(mut commands: Commands, time: Res<Time>, mut stuck: Query<(Entity, &mut Stuck)>) {
    for (entity, mut stuck) in &mut stuck {
        stuck.0 -= time.delta_secs();
        if stuck.0 <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

/// The archers' arrows.
pub struct ArrowsPlugin;

impl Plugin for ArrowsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Dice(opensacred_rules::spawning::Rng::of(0, 0, 3)))
            .add_message::<Loose>()
            .add_systems(
                Update,
                (loose, fly, unstick)
                    .chain()
                    .after(Step::Npcs)
                    .in_set(Playing),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_arrow_hits_the_hero_or_a_creature_hostile_to_its_shooter() {
        let mind = |creature, side| {
            let mut mind = minds::Mind::new(
                Spot::default(),
                0,
                (false, false),
                None,
                [100.0, 200.0],
                (creature, 0),
            );
            mind.side = side;
            mind
        };
        // A thief of the side 2, hostile to the hero; the boss of the start, of the side 3,
        // peaceful to it; a bear of the side 2, hostile to the boss (the sides' table).
        let thief = mind(313, 2);
        let boss = mind(689, 3);
        let bear = mind(537, 2);
        assert!(thief.hostile() && !boss.hostile() && boss.hostile_to(&bear));
        // The thief's arrow hits the hero alive, not dead; the boss's never hits the hero.
        assert!(lands(&thief, Some(true), None));
        assert!(!lands(&thief, Some(false), None));
        assert!(!lands(&boss, Some(true), None));
        // The boss's arrow hits the bear, alive, not a creature of its own side nor a dead bear;
        // the thief's flies on through the bear, of its side.
        assert!(lands(&boss, None, Some(&bear)));
        assert!(!lands(&boss, None, Some(&mind(689, 3))));
        assert!(!lands(&boss, None, None));
        assert!(!lands(&thief, None, Some(&bear)));
    }

    #[test]
    fn an_arrow_points_its_tip_along_its_flight() {
        let velocity = Point::new(Spot::new(600.0, 0.0), 0.0);
        let tip = pointing(velocity) * Vec3::Z;
        assert!((tip - Vec3::X).length() < 1e-5, "{tip}");
        let up = pointing(Point::new(Spot::new(0.0, 300.0), 300.0)) * Vec3::Z;
        assert!(
            (up - Vec3::new(0.0, 1.0, 1.0).normalize()).length() < 1e-5,
            "{up}"
        );
        assert_eq!(pointing(Point::default()), Quat::IDENTITY);
    }
}
