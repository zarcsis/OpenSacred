//! The dryad ghosts (#1964; docs/re/effects.md, "The dryad ghost"): for its type 1249
//! (`TYPE_NPC_DUMMY`) a sector's fill makes the FX object 1162, which makes a dummy 1249 at its
//! place and hunts as its caster (`0x480D40`, the FX event `0xA6`). The dummy draws nothing
//! (`0x44B6F4`), though the pointer picks it ([`Dummy`], `under_cursor`), and as the maker returns
//! it past the creature maker's flags (`+0x1F4`) the hero's look does not wake it: hostile without
//! the flag 1, it stays idle (`minds`) until struck (`ai`). The ghost rises from the ground and
//! flies at the nearest creature hostile to the dummy on its floor within about 316, the hero or a
//! follower, at its neck, turning toward it at 3 radians a second at most, and hovers where it is
//! without one; it keeps its target on any floor while it lives, lets it go as it dies, and ends
//! as it is destroyed (`0x5A7DFA`); it turns back from a cell it may not walk on and off the
//! ground (`0x5A7DBF`). At the end of each run it puts its dummy at its own place (`0x5A9AE0`), so
//! that the dummy rides with it into the cells and sectors it crosses, neither stepping nor
//! striking of its own once struck awake. Once a second it strikes
//! every such creature within its size and theirs (`0x5B17AB`,
//! `opensacred_rules::fight::Rules::dryad_ghost_damages`). It looks as a lost soul with blue dots
//! (`lost_souls`) and loops `HELF_AHNEN_LOOP` (2058); it ends at once with its dummy, destroyed or
//! dead (`0x5A9B87`, `0x5ABDF0`).

use bevy::prelude::*;
use opensacred_rules::particles::{
    DRYAD_GHOST_BEAT, DRYAD_GHOST_DUMMY, DRYAD_GHOST_HOVER, DRYAD_GHOST_LOOKS, DRYAD_GHOST_LOOP,
    DRYAD_GHOST_SIZE, DRYAD_GHOST_SPEED, DRYAD_GHOST_TURN,
};

use crate::crowd::InTheWay;
use crate::fight::{Health, Hit};
use crate::frame::{Playing, Step, Unpaused};
use crate::game::AppState;
use crate::hero::Body;
use crate::npcs::Dies;
use crate::terrain::{Heights, Level};
use crate::{ai, arrows, iso};

/// The bones a ghost aims at, the first its target has (`0x44CBB0` of 1: the bones 3 and 0).
const AIM: [&str; 2] = ["Bip01 Neck", "__Root"];

/// A ghost's dummy (1249), which draws nothing and which the pointer picks at its ghost's place
/// while it lives (`0x44CF20`; docs/re/effects.md, "The dryad ghost").
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Dummy;

/// A ghost: its dummy, its level, whether it stands at its height yet, the creature it hunts, its
/// heading and its time to its next strike.
#[derive(Component)]
struct Ghost {
    dummy: Entity,
    level: Level,
    placed: bool,
    target: Option<Entity>,
    heading: Vec3,
    cooldown: f32,
}

/// The flags of the creature maker's that a fill's dummy goes without (`0x480D40` returns it at
/// `0x4812FD`, before them): its behaviours' and the hostile's 1.
const MAKERS_FLAGS: u32 = 0xFFFF;

/// Hides each fill's dummy as it is made, clears the creature maker's flags off it, and makes its
/// ghost at its place, heading straight up (`0x5A593F`).
fn make(
    mut commands: Commands,
    mut dummies: Query<(Entity, &mut ai::Mind, &Transform, Option<&InTheWay>), Added<ai::Mind>>,
) {
    for (dummy, mut mind, place, way) in &mut dummies {
        if mind.thinks.creature != DRYAD_GHOST_DUMMY {
            continue;
        }
        mind.thinks.flags &= !MAKERS_FLAGS;
        commands.entity(dummy).insert((Visibility::Hidden, Dummy));
        let ghost = commands
            .spawn((
                Ghost {
                    dummy,
                    level: way.and_then(|way| way.level),
                    placed: false,
                    target: None,
                    heading: Vec3::Y,
                    cooldown: 0.0,
                },
                Transform::from_translation(place.translation),
                Visibility::default(),
                crate::lost_souls::Look(f32::INFINITY),
                crate::lost_souls::Ghostly,
                crate::hums::Hum(DRYAD_GHOST_LOOP),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        debug!("dryad ghosts: {ghost} rises for the dummy {dummy}");
    }
}

/// A creature a ghost may hunt and strike: the hero or a follower, alive, on its level.
type Prey<'a> = (
    Entity,
    &'a GlobalTransform,
    &'a Health,
    Option<&'a arrows::Size>,
);

/// The ghost's hunt as its target stands: kept while it lives on any floor, let go as it dies
/// for the nearest sought at once, and the ghost ended where it is destroyed (`0x5A7DFA`–
/// `0x5A7E7C`).
#[derive(Debug, PartialEq, Eq)]
enum Hunt {
    Keeps,
    Seeks,
    Ends,
}

/// What becomes of a ghost's hunt of `target`, of which `exists` tells whether it is in the world
/// and `lives` whether it lives there.
fn hunt(
    target: Option<Entity>,
    exists: impl Fn(Entity) -> bool,
    lives: impl Fn(Entity) -> bool,
) -> Hunt {
    match target {
        Some(target) if !exists(target) => Hunt::Ends,
        Some(target) if lives(target) => Hunt::Keeps,
        _ => Hunt::Seeks,
    }
}

/// Flies each ghost a frame on and strikes about it (`0x5A6680` under the flag `0x10000`,
/// `0x5AB490`, `0x5ABD80`), and puts its dummy at its place (`0x5A9AE0`); a ghost whose dummy or
/// target is destroyed, or whose dummy dies, ends at once, its dots fading after it.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn fly(
    (mut commands, time, mut hits): (Commands, Res<Time>, MessageWriter<Hit>),
    mut ghosts: Query<(Entity, &mut Ghost, &mut Transform)>,
    mut dummies: Query<
        (&ai::Mind, Option<&crate::npcs::Arms>, &mut Transform),
        (Without<Dies>, Without<Ghost>),
    >,
    (targets, heroes, followers): (
        Query<(&Health, Has<Dies>, &GlobalTransform)>,
        Query<(Entity, &GlobalTransform, &Health, &Body), Without<Dies>>,
        Query<
            (
                Entity,
                &GlobalTransform,
                &Health,
                &InTheWay,
                Option<&arrows::Size>,
            ),
            (With<crate::followers::Follower>, Without<Dies>),
        >,
    ),
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (heights, rules): (Option<Res<Heights>>, Option<Res<crate::fight::Rules>>),
) {
    let dt = time.delta_secs();
    for (entity, mut ghost, mut transform) in &mut ghosts {
        if !dummies.contains(ghost.dummy) {
            debug!("dryad ghosts: {entity} ends, its dummy gone or dead");
            commands.entity(entity).despawn();
            continue;
        }
        let level = ghost.level;
        // It starts on the ground at its dummy's place, once its ground is in (`0x480D40`).
        if !ghost.placed {
            let place = transform.translation;
            let Some(ground) = heights
                .as_deref()
                .and_then(|heights| heights.standing(place.x, place.z, level))
            else {
                continue;
            };
            transform.translation.y = ground;
            ghost.placed = true;
        }
        ghost.cooldown = (ghost.cooldown - dt).max(0.0);
        let prey: Vec<Prey> = heroes
            .iter()
            .filter(|(.., body)| body.level() == level)
            .map(|(prey, at, health, _)| (prey, at, health, None))
            .chain(
                followers
                    .iter()
                    .filter(|(.., way, _)| way.level == level)
                    .map(|(prey, at, health, _, size)| (prey, at, health, size)),
            )
            .filter(|(_, _, health, _)| health.current > 0)
            .collect();
        let here = transform.translation;
        let distance = |at: &GlobalTransform| (at.translation().xz() - here.xz()).length();
        // Its target while it lives on any floor; else the nearest on its own within its look.
        let state = hunt(
            ghost.target,
            |target| targets.contains(target),
            |target| {
                targets
                    .get(target)
                    .is_ok_and(|(health, dies, _)| !dies && health.current > 0)
            },
        );
        if state == Hunt::Ends {
            debug!("dryad ghosts: {entity} ends, its target gone");
            commands.entity(entity).despawn();
            continue;
        }
        if state == Hunt::Seeks {
            ghost.target = prey
                .iter()
                .filter(|(_, at, ..)| distance(at) < DRYAD_GHOST_LOOKS)
                .min_by(|a, b| distance(a.1).total_cmp(&distance(b.1)))
                .map(|&(p, ..)| p);
            if let Some(target) = ghost.target {
                debug!("dryad ghosts: {entity} at {here} hunts {target}");
            }
        }
        let aim = ghost.target.and_then(|target| {
            crate::making::bone(target, &AIM, &children, (&bones, &worn))
                .or_else(|| targets.get(target).ok().map(|(.., at)| at.translation()))
        });
        // Without one it hovers, aiming 10 over its place.
        let (aim, speed) = match aim {
            Some(aim) => (aim, DRYAD_GHOST_SPEED),
            None => (here + Vec3::Y * DRYAD_GHOST_HOVER * iso::MODEL_SCALE, 0.0),
        };
        let heading = ghost.heading;
        let to = (aim - here).normalize_or_zero();
        let angle = (heading.dot(to) * 0.9999).clamp(-1.0, 1.0).acos();
        let axis = heading.cross(to).normalize_or_zero();
        let mut heading = if axis == Vec3::ZERO {
            heading
        } else {
            (Quat::from_axis_angle(axis, angle.min(DRYAD_GHOST_TURN * dt)) * heading)
                .normalize_or(heading)
        };
        let mut next = here + heading * speed * dt;
        if let Some(heights) = heights.as_deref() {
            // A cell it may not walk on turns it back where it was; the ground turns it up.
            if !heights.walkable_at(crate::paths::cell_of(next.x, next.z), level) {
                heading = Vec3::new(-heading.x, heading.y, -heading.z);
                next = here;
            }
            if heights
                .standing(next.x, next.z, level)
                .is_some_and(|ground| ground >= next.y)
            {
                heading.y = -heading.y;
            }
        }
        ghost.heading = heading;
        transform.translation = next;
        // Its dummy rides with it, at its place (`0x5A9AE0`–`0x5A9B95`), which makes nothing of
        // the fight it takes up when struck: no step and no strike of its own (seen, #3443).
        if let Ok((.., mut place)) = dummies.get_mut(ghost.dummy) {
            place.translation = next;
            commands
                .entity(ghost.dummy)
                .remove::<(crate::npcs::Walk, crate::npcs::Strikes)>();
        }
        if ghost.cooldown > 0.0 {
            continue;
        }
        let struck: Vec<Entity> = prey
            .iter()
            .filter(|(_, at, _, size)| {
                distance(at) < DRYAD_GHOST_SIZE + size.map_or(arrows::size(0), |size| size.0)
            })
            .map(|&(p, ..)| p)
            .collect();
        if struck.is_empty() {
            continue;
        }
        let Ok((mind, arms, _)) = dummies.get(ghost.dummy) else {
            continue;
        };
        let skill_2 = arms.map_or(0, |arms| arms.skill_level(mind.level, 2));
        let damages = rules
            .as_ref()
            .map_or([0.0; 4], |rules| rules.0.dryad_ghost_damages(skill_2));
        for target in struck {
            debug!("dryad ghosts: {entity} strikes {target} for {damages:?}");
            hits.write(Hit {
                attacker: ghost.dummy,
                target,
                share: 1.0,
                ranged: false,
                art: None,
                fixed: Some(damages),
                sure: true,
                poison: false,
                ward: false,
                hurled: false,
                object: true,
                mount: false,
            });
        }
        ghost.cooldown = DRYAD_GHOST_BEAT;
    }
}

/// The dryad ghosts.
pub struct DryadGhostsPlugin;

impl Plugin for DryadGhostsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (make, fly)
                .chain()
                .after(Step::Npcs)
                .before(Step::Models)
                .in_set(Playing)
                .in_set(Unpaused),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ghost_keeps_a_living_target_lets_a_dead_one_go_and_ends_with_a_destroyed_one() {
        let mut world = World::new();
        let [living, dead, gone] = [(); 3].map(|()| world.spawn_empty().id());
        world.despawn(gone);
        let exists = |target| target != gone;
        let lives = |target| target == living;
        assert_eq!(hunt(Some(living), exists, lives), Hunt::Keeps);
        assert_eq!(hunt(Some(dead), exists, lives), Hunt::Seeks);
        assert_eq!(hunt(Some(gone), exists, lives), Hunt::Ends);
        assert_eq!(hunt(None, exists, lives), Hunt::Seeks);
    }
}
