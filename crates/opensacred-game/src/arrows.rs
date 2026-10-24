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

use bevy::prelude::*;
pub use opensacred_rules::arrows::size;
use opensacred_rules::arrows::{self, Point};
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
/// `0x4EB700`), which the shot's clip sends at its fraction (`npcs::Strikes`).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Loose {
    pub shooter: Entity,
    pub target: Entity,
}

/// A creature's size for the arrows' hits (`items.pak +0x14`, 50 for 0,
/// `opensacred_rules::arrows::size`); one without, as the hero, is of 50, the heroes' own.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Size(pub f32);

/// An arrow in flight: its flight, its shooter, its target, the level it flies on, its size.
#[derive(Component)]
struct Arrow {
    flight: arrows::Arrow,
    shooter: Entity,
    target: Entity,
    level: Level,
    size: f32,
}

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
        let flight = arrows::Arrow::new(point(from), point(toward));
        let level = ways.get(shot.shooter).ok().and_then(|way| way.level);
        let Some((model, texture, size)) = npcs.arrow() else {
            continue;
        };
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
            },
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(texture) = texture {
            arrow.insert(Retexture::load(&assets, texture));
        }
    }
}

/// Flies each arrow a frame on (`0x5BB010`): it ends where it may not fly on and is gone when its
/// time is out; it comes on the hero, its target or the first creature near the part of its way it
/// flew, and hits it when it is alive and hostile to the shooter, alive too (the hero to a shooter
/// hostile to it, a creature to one of a hostile side or type), sending the fight's hit at range
/// with the shooter's values (`fight::Hit`, the event 5 with the bow), and is gone, or sticks in a
/// creature of a size above 20; else it flies on.
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
    mut flying: Query<(Entity, &mut Arrow, &mut Transform)>,
    shooters: Query<&ai::Mind, Without<Dies>>,
    hero: Query<(Entity, &Transform, &Body, &Health), (Without<Arrow>, Without<Dies>)>,
    creatures: Query<
        (Entity, &Transform, &InTheWay, Option<&Size>),
        (Without<Body>, Without<Arrow>),
    >,
    places: Query<&GlobalTransform>,
) {
    let dt = time.delta_secs();
    let hero = hero.iter().next();
    for (entity, mut arrow, mut transform) in &mut flying {
        let level = arrow.level;
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
            .filter(|(_, _, body, _)| body.level() == level)
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
        // or type; the shooter alive.
        let Ok(shooter) = shooters.get(arrow.shooter) else {
            continue;
        };
        let hero_hit = hero.is_some_and(|(e, ..)| e == on);
        let the_hero = hero
            .filter(|_| hero_hit)
            .map(|(.., health)| health.current > 0);
        let other = shooters.get(on).ok().map(|other| &other.thinks);
        if !lands(&shooter.thinks, the_hero, other) {
            continue;
        }
        let distance = match (places.get(arrow.shooter), places.get(on)) {
            (Ok(a), Ok(b)) => (a.translation() - b.translation()).xz().length(),
            _ => 0.0,
        };
        debug!(
            "arrows: {entity} of {} hits {on} {distance:.0} from its shooter",
            arrow.shooter
        );
        hits.write(fight::Hit {
            attacker: arrow.shooter,
            target: on,
            share: 1.0,
            ranged: true,
            art: None,
        });
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
        app.add_message::<Loose>().add_systems(
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
