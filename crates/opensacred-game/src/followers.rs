//! The hero's followers (docs/re/ai.md, "Followers"): the creatures that its summonings make and
//! hire (`sdk_HIRE`, `0x54CF70`), the Wood Elf's unicorn (the spell 3) and the Vampiress's wolf
//! (1026 and 1027). A follower keeps the band of 150 to 240 about its master: farther it walks or
//! runs to it as the master goes, nearer it steps off, and farther than 1,500 it is put near it
//! (`0x4EF150` with the flag `0x100`, `0x4F4770`, `0x4EBF20`). It fights for its master: the
//! second of the hero's hostiles near the master while the master fights, else the master's
//! target, and a creature that comes at the master (`0x4F4044`, `0x4E0010`), as any creature
//! fights another (`ai::pursue`, #630), and no leash sends it home. No sector keeps it
//! (`npcs::Spawn::roams`); it stays by a fallen hero and takes its health back as the hero rises
//! (`0x53E460`). The decisions are `opensacred_rules::minds`'.

use bevy::prelude::*;
use opensacred_rules::minds::{self, Keeps, OnTheWay, Spot};

use crate::ai::Mind;
use crate::crowd::{InTheWay, Nearest};
use crate::fight::Health;
use crate::frame::Step;
use crate::game::AppState;
use crate::hero::{Body, Gait};
use crate::npcs::{Dies, PutAt, Walk};
use crate::spawning::Rng;
use crate::terrain::Heights;

/// A creature hired by its master, the hero, and whether it steps off from it (the walk's mode
/// 3, `+0x164`).
#[derive(Component, Clone, Copy, Debug)]
pub struct Follower {
    pub master: Entity,
    stepping_off: bool,
}

/// A summoning (the object 777, `FX_BIGSUMMON`, `cParticleSystem_bigsummon`): its master, the
/// creature it makes and its level, and the time until it makes it.
#[derive(Component, Clone, Copy, Debug)]
pub struct Summoning {
    master: Entity,
    creature: u32,
    level: u32,
    left: f32,
}

/// How long a summoning takes to make its creature: its phase, at 2 a second, passing 3.0
/// (`0x769C60`).
const SUMMONS: f32 = 1.5;
/// The highest level that a summoning gives (`0x564D60`).
const MOST_LEVEL: u32 = 250;
/// The side a hiring gives a follower of the animals' side 6 or hostile to its master, a friend
/// of the heroes' (`0x54CF70`).
const HIRED_SIDE: u8 = 3;

/// The followers' generator for their steps off.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(Rng::of(0, 0, 0x100))
    }
}

/// Starts a summoning at `at` for `master` that makes `creature` of `level`, at most 250, some
/// 1.5 s later.
pub fn summon(commands: &mut Commands, master: Entity, creature: u32, level: u32, at: Vec3) {
    info!("followers: {master} summons a {creature} of level {level} at {at}");
    commands.spawn((
        Summoning {
            master,
            creature,
            level: level.clamp(1, MOST_LEVEL),
            left: SUMMONS,
        },
        Transform::from_translation(at),
        DespawnOnExit(AppState::InGame),
    ));
}

/// Makes the creature of each summoning whose time is up, as the hero's follower (the action 1,
/// `0x5B2600` at `0x5B4216`).
fn make(
    mut commands: Commands,
    time: Res<Time>,
    mut summonings: Query<(Entity, &mut Summoning, &Transform)>,
) {
    for (entity, mut summoning, at) in &mut summonings {
        summoning.left -= time.delta_secs();
        if summoning.left > 0.0 {
            continue;
        }
        commands.entity(entity).despawn();
        let (summoning, at) = (*summoning, at.translation);
        commands.queue(move |world: &mut World| hire_summoned(world, summoning, at));
    }
}

/// Makes `summoning`'s creature at `at` on its master's floor, its master's other followers of
/// that type dying first (their health 0, `0x549480(0, 2)`), and hires it.
fn hire_summoned(world: &mut World, summoning: Summoning, at: Vec3) {
    let mut old = world.query_filtered::<(Entity, &Follower, &Mind, &mut Health), Without<Dies>>();
    let gone: Vec<Entity> = old
        .iter_mut(world)
        .filter(|(_, follower, mind, _)| {
            follower.master == summoning.master && mind.creature == summoning.creature
        })
        .map(|(entity, .., mut health)| {
            health.current = 0;
            entity
        })
        .collect();
    for entity in gone {
        info!("followers: {entity} goes for the new one");
        world.entity_mut(entity).insert(Dies::by_creature(0));
    }
    let level = world.get::<Body>(summoning.master).map(Body::level);
    let floor = world
        .get_resource::<Heights>()
        .zip(level)
        .and_then(|(heights, level)| heights.floor(level))
        .map_or(0, |(_, floor)| i32::from(floor));
    let (x, y) = crate::paths::cell_of(at.x, at.z);
    let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
        return;
    };
    let request = crate::npcs::Spawn {
        creature: summoning.creature,
        cells: vec![(x, y)],
        floor,
        level: Some(summoning.level),
        rank: crate::fight::Rank::Common,
        hostile: Some(false),
        immortal: false,
        roams: true,
    };
    let made = match world.run_system_cached_with(crate::npcs::spawn_creatures, request) {
        Ok(Ok(made)) => made,
        Ok(Err(e)) => {
            warn!("followers: no {} is made: {e}", summoning.creature);
            return;
        }
        Err(e) => {
            warn!("followers: no {} is made: {e}", summoning.creature);
            return;
        }
    };
    for entity in made {
        hire(world, entity, summoning.master);
    }
}

/// Hires `entity` as `master`'s follower (`sdk_HIRE`, `0x54CF70`): its flags word kept to the
/// follower's behaviour (the bits `0xEFF62000`, none of which the game reads, and `0x100`), its
/// side 3 where it was the animals' or hostile to its master, out of the hero's hostiles.
fn hire(world: &mut World, entity: Entity, master: Entity) {
    let Ok(mut creature) = world.get_entity_mut(entity) else {
        return;
    };
    if let Some(mut mind) = creature.get_mut::<Mind>() {
        mind.flags = minds::FOLLOWS;
        if mind.side == 6 || mind.hostile() {
            mind.side = HIRED_SIDE;
        }
    }
    if let Some(mut way) = creature.get_mut::<InTheWay>() {
        way.stands.hostile = false;
    }
    creature.insert(Follower {
        master,
        stepping_off: false,
    });
    info!("followers: {master} hires {entity}");
}

/// The heading of `transform`, of a model facing +X (`hero::facing`), in radians from the grid's
/// x toward its y.
fn heading(transform: &Transform) -> f32 {
    let ahead = transform.rotation * Vec3::X;
    ahead.z.atan2(ahead.x)
}

/// The rules' point of the ground under world point `point`.
fn spot(point: Vec3) -> Spot {
    Spot::new(point.x, point.z)
}

/// Each follower that does not fight keeps by its master, at every tick of the world, as the
/// rules decide (`opensacred_rules::minds::keeps`, `on_the_way`, `Mind::helps`): its home moves
/// with the master, and it is sent at a creature that comes at the master within 350 (600 at
/// range), else, standing while the master attacks, at the second of the hero's hostiles near the
/// master or at the master's target; else it is put near the master from farther than 1,500,
/// walks or runs to it from farther than 240 as the master goes, and steps off from nearer than
/// 150, on its way heading for the master again from farther than 270 and stopping within 170.
#[expect(
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their \
              filters"
)]
fn keep(
    mut commands: Commands,
    mut dice: ResMut<Dice>,
    heights: Res<Heights>,
    nearest: Res<Nearest>,
    cameras: Query<&Projection, With<Camera3d>>,
    masters: Query<
        (
            &Transform,
            &Body,
            Option<&crate::attack::Attack>,
            Option<&crate::making::Making>,
            Option<&crate::making::Playing>,
        ),
        Without<Follower>,
    >,
    others: Query<(Entity, &GlobalTransform, &Mind), (Without<Follower>, Without<Dies>)>,
    mut followers: Query<
        (
            Entity,
            &Transform,
            &InTheWay,
            &mut Mind,
            &mut Follower,
            Option<&Walk>,
        ),
        (Without<Body>, Without<Dies>),
    >,
) {
    let zoom = cameras
        .iter()
        .find_map(|projection| match projection {
            Projection::Orthographic(ortho) => Some(ortho.scale),
            _ => None,
        })
        .unwrap_or(1.0);
    // The creatures that fight the hero, which come at it.
    let attackers: Vec<(Entity, Vec3, bool)> = others
        .iter()
        .filter(|(_, _, mind)| mind.hostile() && mind.fighting() && mind.foe().is_none())
        .map(|(entity, at, mind)| (entity, at.translation(), mind.ranged))
        .collect();
    // The foes the followers are sent at in this tick, each at one follower.
    let mut sent: Vec<Entity> = followers
        .iter()
        .filter_map(|(.., mind, _, _)| mind.foe())
        .collect();
    for (entity, transform, way, mut mind, mut follower, walk) in &mut followers {
        let Ok((master_at, body, attack, making, playing)) = masters.get(follower.master) else {
            continue;
        };
        let master = master_at.translation;
        let here = transform.translation;
        mind.home = spot(master);
        // The first point behind the master, round about it, where it may stand with a way to
        // the master; else it stays (`0x4EBF20`).
        let level = body.level();
        let of = follower.master;
        let put_near = |commands: &mut Commands| {
            let put = minds::put_near(spot(master), heading(master_at), zoom).find(|&to| {
                let cell = crate::paths::cell_of(to.x, to.y);
                heights.walkable_on(cell, level)
                    && heights.standing(to.x, to.y, level).is_some()
                    && crate::ai::way_to(
                        Vec3::new(to.x, master.y, to.y),
                        level,
                        spot(master),
                        &heights,
                    )
            });
            if let Some(to) = put {
                debug!("followers: {entity} is put near {} at {to:?}", of);
                commands.entity(entity).remove::<Walk>().insert(PutAt {
                    at: Vec3::new(to.x, master.y, to.y),
                    level,
                });
            }
            if put.is_none() {
                debug!("followers: {entity} finds no point near {of} to be put at");
            }
            put.is_some()
        };
        // Farther than 2,000 it leaves its fight to be put near the master (`0x545E0E`).
        if minds::pulled(spot(here), spot(master)) {
            if put_near(&mut commands) {
                mind.called_off();
            }
            continue;
        }
        if mind.fighting() {
            continue;
        }
        // A creature that comes at the master sends the first follower not fighting at it,
        // unless one fights it already (`0x4E0010`).
        let defends = attackers.iter().find(|&&(foe, at, ranged)| {
            !sent.contains(&foe) && minds::defends(spot(master), spot(at), ranged)
        });
        // Standing while the master attacks: the second of the hero's hostiles near the master,
        // else the master's target (`0x4F4044`).
        let target = attack
            .and_then(crate::attack::Attack::target)
            .or(playing.map(crate::making::Playing::target))
            .or(making.map(|making| making.target))
            .filter(|&target| target != follower.master && others.contains(target));
        let second = nearest
            .hostiles
            .get(1)
            .and_then(|&second| others.get(second).ok())
            .map(|(second, at, _)| (second, spot(at.translation())));
        let foe = defends.map(|&(foe, ..)| foe).or_else(|| {
            walk.is_none()
                .then(|| mind.helps(spot(master), target, second))
                .flatten()
        });
        if let Some(foe) = foe
            && mind.send_at(foe)
        {
            debug!("followers: {entity} goes at {foe} for {}", follower.master);
            sent.push(foe);
            follower.stepping_off = false;
            commands.entity(entity).remove::<Walk>();
            continue;
        }
        let runs = body.moving() == Some(Gait::Run);
        let to_master = |within: f32| {
            let walk = if runs {
                Walk::run(master, way.level, mind.run)
            } else {
                Walk::to(master, way.level, mind.speed)
            };
            walk.within(within)
        };
        if walk.is_some() {
            match minds::on_the_way(spot(here), spot(master), follower.stepping_off) {
                Some(OnTheWay::PutNear) => {
                    put_near(&mut commands);
                }
                Some(OnTheWay::HeadsAgain) => {
                    commands
                        .entity(entity)
                        .insert(to_master(minds::FOLLOWER_STOPS));
                }
                Some(OnTheWay::Stops) => {
                    commands.entity(entity).remove::<Walk>();
                }
                None => {}
            }
            continue;
        }
        follower.stepping_off = false;
        match minds::keeps(spot(here), spot(master)) {
            Keeps::PutNear => {
                put_near(&mut commands);
            }
            Keeps::Walks => {
                let d = (master - here).xz().length();
                debug!(
                    "followers: {entity} walks to {} from {d:.0}",
                    follower.master
                );
                commands
                    .entity(entity)
                    .insert(to_master(minds::FOLLOWER_STOPS));
            }
            Keeps::Stands => {}
            Keeps::StepsOff => {
                // The first of its points with a way to it, else the last (`0x4F7710`).
                let points = minds::stepping_off(spot(master), heading(master_at), &mut dice.0);
                let to = points
                    .iter()
                    .copied()
                    .find(|&to| crate::ai::way_to(here, way.level, to, &heights))
                    .unwrap_or(points[3]);
                follower.stepping_off = true;
                let to = Vec3::new(to.x, here.y, to.y);
                commands.entity(entity).insert(if runs {
                    Walk::run(to, way.level, mind.run)
                } else {
                    Walk::to(to, way.level, mind.speed)
                });
            }
        }
    }
}

/// As the fallen hero rises, each follower of its takes its health back (`0x53E460`).
fn risen(
    rose: On<Remove<Dies>>,
    heroes: Query<(), With<Body>>,
    mut followers: Query<(&Follower, &mut Health), Without<Dies>>,
) {
    if !heroes.contains(rose.entity) {
        return;
    }
    for (follower, mut health) in &mut followers {
        if follower.master == rose.entity {
            health.current = health.most;
        }
    }
}

/// The followers and the summonings that make them.
pub struct FollowersPlugin;

impl Plugin for FollowersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .add_systems(Update, make.in_set(crate::frame::Playing))
            .add_systems(
                Update,
                keep.after(crate::ai::Thinks)
                    .in_set(Step::Crowd)
                    .run_if(crate::frame::ticked),
            )
            .add_observer(risen);
    }
}
