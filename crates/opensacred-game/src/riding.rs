//! The hero on horseback (#2299; docs/re/horses.md, "Mounting", "Getting off and being thrown
//! off", "The control and the gait"): a click on a horse sends the hero to the point by it that
//! it mounts from (`0x59A3E0`), where it plays its mount and sits on the horse's `horse_bone`;
//! riding, the hero goes at its horse's speeds, the horse under it playing its walk or run, by no
//! stair, teleport or entrance (`hero::run`); Shift with a click on its horse, or its horse gone,
//! gets it off at the point by the horse.

use std::time::Duration;

use bevy::prelude::*;
use opensacred_rules::horses;

use crate::ai::Mind;
use crate::attack::{self, Aim};
use crate::controls::{Action, Keys};
use crate::crowd::InTheWay;
use crate::fight::Health;
use crate::frame::Step;
use crate::hero::{self, Body, Clips, GameSpeed, Hero};
use crate::npcs::{Dies, Npcs, Steered, Walk};

/// A horse, a creature of `Creature.pak` class 4, of its class 1 to 9 (`+0x400`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Horse {
    pub class: u8,
}

/// The hero's seat on a horse, from its walk to the horse to its getting off.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub enum Seat {
    /// Walking to the point by its horse that it mounts from, and its heading there.
    Approaching {
        horse: Entity,
        point: Vec3,
        heading: Vec3,
    },
    /// Playing its mount at the point, for `left` seconds still.
    Mounting { horse: Entity, left: f32 },
    /// On its horse, at its speeds (`0x4F75F0`), its model shifted by `offset` onto the horse's
    /// bone, in its own units.
    Riding {
        horse: Entity,
        speeds: [f32; 2],
        offset: Vec3,
    },
    /// Playing its dismount at the point by the horse, for `left` seconds still.
    Dismounting { left: f32 },
}

impl Seat {
    /// Whether its clip plays, the hero going nowhere: mounting or getting off.
    pub fn busy(&self) -> bool {
        matches!(self, Self::Mounting { .. } | Self::Dismounting { .. })
    }

    /// Its speeds riding, walking and running.
    pub fn speeds(&self) -> Option<[f32; 2]> {
        match self {
            Self::Riding { speeds, .. } => Some(*speeds),
            _ => None,
        }
    }

    /// The horse it rides.
    pub fn ridden(&self) -> Option<Entity> {
        match self {
            Self::Riding { horse, .. } => Some(*horse),
            _ => None,
        }
    }
}

/// A horse that the hero rides, its place in the way kept aside: it stands under the hero.
#[derive(Component, Clone, Debug)]
pub struct Ridden {
    pub rider: Entity,
    way: Option<InTheWay>,
}

pub struct RidingPlugin;

impl Plugin for RidingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            pick.before(hero::steer)
                .before(crate::talk::pick)
                .after(attack::aim)
                .in_set(crate::frame::Unpaused),
        )
        .add_systems(
            Update,
            (approach, mount, dismount, carry)
                .chain()
                .after(hero::run)
                .in_set(Step::HeroMove),
        )
        .add_systems(
            PostUpdate,
            seat.after(bevy::app::AnimationSystems)
                .before(TransformSystems::Propagate),
        );
    }
}

/// The rider's type and skill 22, by which it may ride a horse (`0x4267D0`).
fn rider(hero: &Hero, arts: Option<&crate::arts::HeroArts>) -> (u32, u32) {
    let riding = arts
        .and_then(|arts| arts.skills.iter().find(|&&(skill, _)| skill == 22))
        .map_or(0, |&(_, level)| level);
    (hero.creature, riding)
}

/// A click on a horse (`0x6172C0`, the creature event 7): on foot, the hero goes to mount it where
/// it may ride it (`0x53A170`'s sub-state 0); riding, Shift with a click on its own horse gets it
/// off (the event `0x101`). A horse of another rider, dead or lying beyond 1,879 takes no click.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn pick(
    mut commands: Commands,
    (buttons, keys): (Res<ButtonInput<MouseButton>>, Keys),
    pointer: Res<crate::hud::Pointer>,
    mut aim: ResMut<Aim>,
    camera: Single<(&Camera, &GlobalTransform), crate::iso::Eye<With<Camera>>>,
    (hero, npcs, arts): (
        Res<Hero>,
        Option<Res<Npcs>>,
        Option<Res<crate::arts::HeroArts>>,
    ),
    horses: Query<(Entity, &GlobalTransform, &Horse, &Health, Option<&Ridden>), Without<Dies>>,
    bones: (Query<&Children>, Query<(&Name, &GlobalTransform)>),
    mut heroes: Query<(Entity, &Transform, &mut Body, Option<&Seat>), Without<Dies>>,
) {
    if !buttons.just_pressed(MouseButton::Left) || aim.0 {
        return;
    }
    let (Some(cursor), Some(npcs)) = (pointer.0, npcs) else {
        return;
    };
    let (camera, view) = *camera;
    let Ok(ray) = camera.viewport_to_world(view, cursor) else {
        return;
    };
    let Some((horse, at, class, ridden)) = horses
        .iter()
        .filter(|(.., health, _)| health.current > 0)
        .map(|(entity, at, horse, _, ridden)| {
            let at = at.translation();
            (
                entity,
                at,
                horse.class,
                ridden.map(|r| r.rider),
                attack::from_line(ray, at),
            )
        })
        .filter(|&(.., distance)| distance <= attack::PICKED_WITHIN)
        .min_by(|a, b| a.4.total_cmp(&b.4))
        .map(|(entity, at, class, ridden, _)| (entity, at, class, ridden))
    else {
        return;
    };
    for (me, transform, mut body, seat) in &mut heroes {
        match seat {
            // Riding: Shift and a click on its own horse gets it off.
            Some(Seat::Riding { horse: own, .. }) if *own == horse => {
                aim.0 = true;
                if keys.pressed(Action::Walk) {
                    debug!("riding: the hero gets off {horse}");
                    commands.queue(move |world: &mut World| get_off(world, me, true));
                }
            }
            Some(seat) if seat.busy() || seat.ridden().is_some() => {}
            _ => {
                if ridden.is_some_and(|rider| rider != me)
                    || (at - transform.translation).xz().length() > horses::MOUNTS_FROM
                {
                    continue;
                }
                aim.0 = true;
                let (kind, riding) = rider(&hero, arts.as_deref());
                if !horses::rides(kind, riding, &npcs.horse_row(class)) {
                    debug!("riding: the hero may not ride {horse}");
                    continue;
                }
                let Ok((_, horse_at, ..)) = horses.get(horse) else {
                    continue;
                };
                let (point, heading) = place_by(&hero, horse, horse_at, &bones, false);
                debug!("riding: the hero goes to mount {horse}");
                body.send_onto(point, None);
                commands.entity(me).insert(Seat::Approaching {
                    horse,
                    point,
                    heading,
                });
            }
        }
    }
}

/// The point by the horse `horse` at `at` that the hero mounts from, or gets `off` at, and the
/// heading it turns to there (`0x59A3E0`): its start bone's for the riders of
/// [`horses::by_start_bone`], else by the hero's model (`horses::place_by`), the heading the
/// horse's facing turned by 90°.
fn place_by(
    hero: &Hero,
    horse: Entity,
    at: &GlobalTransform,
    (children, names): &(Query<&Children>, Query<(&Name, &GlobalTransform)>),
    off: bool,
) -> (Vec3, Vec3) {
    let facing = (at.rotation() * Vec3::X).with_y(0.0).normalize_or(Vec3::X);
    let centre = at.translation();
    let (offsets, bone) = if horses::by_start_bone(hero.creature) {
        let bone = children
            .iter_descendants(horse)
            .find_map(|e| {
                names
                    .get(e)
                    .ok()
                    .filter(|(n, _)| n.as_str() == horses::START_BONE)
            })
            .map(|(_, t)| t.translation());
        ([0.0, 0.0], bone)
    } else {
        (
            horses::place_by(hero.creature, &model_name(hero), off),
            None,
        )
    };
    let ([x, z], [hx, hz]) = horses::point_by([centre.x, centre.z], [facing.x, facing.z], offsets);
    let point = match bone {
        // Getting off, the bone's place along the horse's other axis, the horse's own along x.
        Some(bone) if off => Vec3::new(centre.x, centre.y, bone.z),
        Some(bone) => Vec3::new(bone.x, centre.y, bone.z),
        None => Vec3::new(x, centre.y, z),
    };
    (point, Vec3::new(hx, 0.0, hz))
}

/// Mounts a hero that has come to its point (within 25, `0x53A170`'s sub-state 4): it stands on
/// the point, turns to its heading and plays its mount at the rate 1.0. The point goes with the
/// horse, the hero sent after it where the horse moved; a hero sent elsewhere meanwhile, stopped
/// short of the point, or whose horse went or was taken, goes on foot.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn approach(
    mut commands: Commands,
    (clips, hero, speed): (Option<Res<Clips>>, Res<Hero>, Res<GameSpeed>),
    horses: Query<&GlobalTransform, (With<Horse>, Without<Dies>, Without<Ridden>)>,
    bones: (Query<&Children>, Query<(&Name, &GlobalTransform)>),
    mut heroes: Query<(Entity, &mut Transform, &mut Body, &mut Seat)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    clip_assets: Res<Assets<AnimationClip>>,
) {
    for (me, mut transform, mut body, mut seat) in &mut heroes {
        let Seat::Approaching { horse, point, .. } = *seat else {
            continue;
        };
        let Ok(at) = horses.get(horse) else {
            commands.entity(me).remove::<Seat>();
            continue;
        };
        if body.ordered() {
            continue;
        }
        let (now, heading) = place_by(&hero, horse, at, &bones, false);
        let elsewhere = body
            .target()
            .is_some_and(|target| (target - point).xz().length() > FOLLOWS_BY);
        let stopped = body.target().is_none() && body.moving().is_none();
        let near = (transform.translation - now).xz().length() <= horses::MOUNTS_WITHIN;
        if elsewhere || (stopped && !near && (now - point).xz().length() <= FOLLOWS_BY) {
            commands.entity(me).remove::<Seat>();
            continue;
        }
        if !near {
            // The horse moved: the hero goes after its new point.
            if (now - point).xz().length() > FOLLOWS_BY {
                body.send_onto(now, None);
                *seat = Seat::Approaching {
                    horse,
                    point: now,
                    heading,
                };
            }
            continue;
        }
        body.stop();
        transform.translation.x = now.x;
        transform.translation.z = now.z;
        transform.rotation = hero::facing(heading);
        let mount = clips.as_ref().and_then(|clips| clips.riding[0].clone());
        let left = mount.as_ref().map_or(0.0, |(_, clip)| {
            clip_assets.get(clip).map_or(0.0, AnimationClip::duration) / speed.0.max(f32::EPSILON)
        });
        if let (Some((node, _)), Some(clips)) = (mount, clips.as_ref())
            && let Some((mut player, mut transitions)) =
                clips.player.and_then(|p| players.get_mut(p).ok())
        {
            transitions
                .play(&mut player, node, Duration::ZERO)
                .set_speed(speed.0);
        }
        debug!("riding: the hero mounts {horse} for {left:.2} s");
        *seat = Seat::Mounting { horse, left };
        // The horse stands for its rider from the clip's start (both take `+0x200` bit `0x10`):
        // its walk ends and it leaves the way.
        commands.queue(move |world: &mut World| hold(world, horse, me));
    }
}

/// How far a horse's point may move before the hero walking to it is sent after it anew.
const FOLLOWS_BY: f32 = 10.0;

/// Seats a hero whose mount has played on its horse (`0x5559E0`): the hero takes the horse's place
/// and heading, the horse stops and leaves the way, and the hero goes at the horse's speeds, its
/// `Creature.pak` walk and run times its class's pace (the horse data's first word) times the
/// game's speed (`0x4F75F0`), playing its ride (`0x5467A0`).
fn mount(
    mut commands: Commands,
    time: Res<Time>,
    (npcs, speed, clips): (Option<Res<Npcs>>, Res<GameSpeed>, Option<Res<Clips>>),
    mut heroes: Query<(Entity, &mut Transform, &mut Seat), Without<Horse>>,
    horses: Query<(&Transform, &Horse, &Mind), Without<Dies>>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    let Some(npcs) = npcs else {
        return;
    };
    for (me, mut transform, mut seat) in &mut heroes {
        let Seat::Mounting { horse, left } = *seat else {
            continue;
        };
        let left = left - time.delta_secs();
        if left > 0.0 {
            *seat = Seat::Mounting { horse, left };
            continue;
        }
        let Ok((at, class, mind)) = horses.get(horse) else {
            commands.entity(me).remove::<Seat>();
            continue;
        };
        let pace = npcs.horse_row(class.class).pace_factor();
        let speeds = npcs
            .speeds_of(mind.creature)
            .map(|s| f32::from(s) * pace * speed.0);
        transform.translation = at.translation;
        transform.rotation = at.rotation;
        *seat = Seat::Riding {
            horse,
            speeds,
            offset: Vec3::ZERO,
        };
        commands.entity(horse).insert(Steered(None));
        if let Some(clips) = clips.as_ref()
            && let Some((node, _)) = &clips.riding[1]
            && let Some((mut player, mut transitions)) =
                clips.player.and_then(|p| players.get_mut(p).ok())
        {
            transitions
                .play(&mut player, *node, Duration::ZERO)
                .repeat()
                .set_speed(speed.0);
        }
        info!("riding: the hero rides {horse}, at {speeds:?}");
    }
}

/// A hero that gets off: its dismount plays at the point by the horse, then it stands there.
fn dismount(mut commands: Commands, time: Res<Time>, mut heroes: Query<(Entity, &mut Seat)>) {
    for (me, mut seat) in &mut heroes {
        let Seat::Dismounting { left } = *seat else {
            continue;
        };
        let left = left - time.delta_secs();
        if left > 0.0 {
            *seat = Seat::Dismounting { left };
        } else {
            commands.entity(me).remove::<Seat>();
        }
    }
}

/// Carries each ridden horse under its rider, its walk or run playing as the rider moves
/// (`0x5941A0` copies the horse's place into its rider; here the rider's into its horse); a
/// rider whose horse is dead or gone, or who dies, is thrown off at once (`0x53ECA0`, `0x5375C0`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn carry(
    mut commands: Commands,
    heroes: Query<(Entity, &Transform, &Body, &Seat, Has<Dies>), Without<Horse>>,
    mut horses: Query<(&mut Transform, &mut Steered, Has<Dies>), With<Horse>>,
) {
    for (me, transform, body, seat, dying) in &heroes {
        let Some(horse) = seat.ridden() else {
            continue;
        };
        match horses.get_mut(horse) {
            Ok((mut at, mut steered, false)) if !dying => {
                at.translation = transform.translation;
                at.rotation = transform.rotation;
                let gait = body.moving().map(|gait| gait == hero::Gait::Run);
                if steered.0 != gait {
                    steered.0 = gait;
                }
            }
            _ => {
                debug!("riding: the hero is thrown off {horse}");
                commands.queue(move |world: &mut World| get_off(world, me, false));
            }
        }
    }
}

/// Fells the horse `horse` in its rider's place, as a lethal blow on the rider fells it
/// (`0x56F8DB`, the horse's action set to the dying one, 6): it dies with no killer, giving
/// nobody experience and dropping nothing, its dying clip the one of two that `choice` names,
/// and its death throws its rider off ([`carry`]).
pub(crate) fn fell(world: &mut World, horse: Entity, choice: u32) {
    let Ok(mut entity) = world.get_entity_mut(horse) else {
        return;
    };
    if entity.contains::<Dies>() {
        return;
    }
    if let Some(mut health) = entity.get_mut::<Health>() {
        health.current = 0;
    }
    entity.insert(Dies::by_creature(choice));
    info!("riding: a lethal blow on its rider fells {horse}");
}

/// Gets the hero `me` off its horse (the sub-state 5, `0x551E40`, which the horse's death takes
/// too, `0x53ECA0`): its model off the horse's bone, the horse back in the way, standing; the
/// hero stands at the point by the horse, and plays its dismount where `clip`; a horse gone
/// leaves it where it is.
fn get_off(world: &mut World, me: Entity, clip: bool) {
    let Some(Seat::Riding { horse, .. }) = world.get::<Seat>(me).copied() else {
        return;
    };
    if let Ok(mut horse_entity) = world.get_entity_mut(horse) {
        let way = horse_entity.get::<Ridden>().and_then(|ridden| ridden.way);
        horse_entity.remove::<(Ridden, Steered)>();
        if let Some(way) = way {
            horse_entity.insert(way);
        }
    }
    let speed = world.get_resource::<GameSpeed>().map_or(1.0, |s| s.0);
    let placed = place(world, horse, true);
    let dismount = clip
        .then(|| {
            world
                .get_resource::<Clips>()
                .and_then(|clips| clips.riding[2].clone())
        })
        .flatten();
    let left = dismount.as_ref().map_or(0.0, |(_, handle)| {
        world
            .get_resource::<Assets<AnimationClip>>()
            .and_then(|assets| assets.get(handle))
            .map_or(0.0, AnimationClip::duration)
            / speed.max(f32::EPSILON)
    });
    if let Some((point, heading)) = placed
        && let Some(mut transform) = world.get_mut::<Transform>(me)
    {
        transform.translation.x = point.x;
        transform.translation.z = point.z;
        transform.rotation = hero::facing(heading);
    }
    if let Some(mut body) = world.get_mut::<Body>(me) {
        body.stop();
    }
    let player = world.get_resource::<Clips>().and_then(|clips| clips.player);
    if let (Some((node, _)), Some(player)) = (dismount, player)
        && let Ok(mut entity) = world.get_entity_mut(player)
        && let Some(mut transitions) = entity.take::<AnimationTransitions>()
    {
        if let Some(mut animation) = entity.get_mut::<AnimationPlayer>() {
            transitions
                .play(&mut animation, node, Duration::ZERO)
                .set_speed(speed);
        }
        entity.insert(transitions);
    }
    if left > 0.0 {
        world.entity_mut(me).insert(Seat::Dismounting { left });
    } else {
        world.entity_mut(me).remove::<Seat>();
    }
    info!("riding: the hero gets off {horse}");
}

/// The point by the horse `horse` that the hero mounts from, or gets `off` at, and the heading it
/// turns to there, as [`place_by`] finds them, of the world.
fn place(world: &World, horse: Entity, off: bool) -> Option<(Vec3, Vec3)> {
    let hero = world.get_resource::<Hero>()?;
    let at = world.get::<GlobalTransform>(horse)?;
    let facing = (at.rotation() * Vec3::X).with_y(0.0).normalize_or(Vec3::X);
    let centre = at.translation();
    let point = if horses::by_start_bone(hero.creature) {
        let bone = bone_of(world, horse, horses::START_BONE).unwrap_or(centre);
        if off {
            Vec3::new(centre.x, centre.y, bone.z)
        } else {
            Vec3::new(bone.x, centre.y, bone.z)
        }
    } else {
        let ([x, z], _) = horses::point_by(
            [centre.x, centre.z],
            [facing.x, facing.z],
            horses::place_by(hero.creature, &model_name(hero), off),
        );
        Vec3::new(x, centre.y, z)
    };
    Some((point, Vec3::new(-facing.z, 0.0, facing.x)))
}

/// The command `ride`: riding, the hero gets off; else it goes to mount the horse `id`, or the
/// nearest within 1,879, as a click on it sends it, where it may ride it; a line of what it does.
pub fn ride(world: &mut World, id: Option<Entity>) -> Result<String, String> {
    let mut heroes = world.query_filtered::<(Entity, &Transform, Option<&Seat>), With<Body>>();
    let (me, at, seat) = heroes
        .iter(world)
        .next()
        .map(|(me, at, seat)| (me, at.translation, seat.copied()))
        .ok_or("the hero is not in the world yet")?;
    if let Some(Seat::Riding { horse, .. }) = seat {
        get_off(world, me, true);
        return Ok(format!("the hero gets off {horse}"));
    }
    let mut horses = world
        .query_filtered::<(Entity, &Transform, &Horse, &Health), (Without<Dies>, Without<Ridden>)>(
        );
    let (horse, class) = horses
        .iter(world)
        .filter(|(entity, .., health)| health.current > 0 && id.is_none_or(|id| id == *entity))
        .map(|(entity, t, horse, _)| (entity, horse.class, (t.translation - at).xz().length()))
        .filter(|&(.., distance)| distance <= horses::MOUNTS_FROM)
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(entity, class, _)| (entity, class))
        .ok_or("no horse within 1,879")?;
    let hero = world.get_resource::<Hero>().ok_or("no hero")?;
    let (kind, riding) = rider(hero, world.get_resource::<crate::arts::HeroArts>());
    let row = world
        .get_resource::<Npcs>()
        .map(|npcs| npcs.horse_row(class))
        .unwrap_or_default();
    if !horses::rides(kind, riding, &row) {
        return Err(format!(
            "the hero may not ride {horse}: it needs riding {}",
            row.riding
        ));
    }
    let (point, heading) = place(world, horse, false).ok_or("the horse has no place yet")?;
    if let Some(mut body) = world.get_mut::<Body>(me) {
        body.send_onto(point, None);
    }
    world.entity_mut(me).insert(Seat::Approaching {
        horse,
        point,
        heading,
    });
    Ok(format!("the hero goes to mount {horse} of class {class}"))
}

/// Holds the horse `horse` for its rider `me`: its walk ended, its place in the way kept aside.
fn hold(world: &mut World, horse: Entity, me: Entity) {
    let Ok(mut entity) = world.get_entity_mut(horse) else {
        return;
    };
    let way = entity.take::<InTheWay>();
    entity.remove::<Walk>().insert(Ridden { rider: me, way });
}

/// The global place of the bone `wanted` among the descendants of `root`.
fn bone_of(world: &World, root: Entity, wanted: &str) -> Option<Vec3> {
    let mut queue = vec![root];
    while let Some(entity) = queue.pop() {
        if world
            .get::<Name>(entity)
            .is_some_and(|name| name.as_str() == wanted)
        {
            return world
                .get::<GlobalTransform>(entity)
                .map(GlobalTransform::translation);
        }
        if let Some(children) = world.get::<Children>(entity) {
            queue.extend(children.iter());
        }
    }
    None
}

/// The hero's model's name as the original's file: `0661-seraphim.glb` is `seraphim.grn`.
fn model_name(hero: &Hero) -> String {
    let file = hero.model.rsplit('/').next().unwrap_or(&hero.model);
    let name = file.split_once('-').map_or(file, |(_, name)| name);
    name.replace(".glb", ".grn")
}

/// Seats the rider on its horse's bone (`0x555300(18)`: Granny's `AttachSequence` of its `Bip01`
/// to the horse's `horse_bone`): its model's root (`__Root`), which its clips set at every frame,
/// shifted after them by where its `Bip01` stood from the horse's bone, the shift kept and
/// bettered from frame to frame.
fn seat(
    mut heroes: Query<(Entity, &mut Seat)>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    names: Query<&Name>,
    globals: Query<&GlobalTransform>,
    mut transforms: Query<&mut Transform>,
) {
    for (me, mut seat) in &mut heroes {
        let Seat::Riding { horse, offset, .. } = &mut *seat else {
            continue;
        };
        let named = |root: Entity, wanted: &str| {
            children
                .iter_descendants(root)
                .find(|&e| names.get(e).is_ok_and(|name| name.as_str() == wanted))
        };
        let Some(root) = named(me, MODEL_ROOT) else {
            continue;
        };
        let space = parents
            .get(root)
            .ok()
            .and_then(|parent| globals.get(parent.parent()).ok())
            .map(|parent| parent.affine().inverse());
        if let (Some(saddle), Some(pelvis), Some(space)) = (
            named(*horse, horses::HORSE_BONE).and_then(|e| globals.get(e).ok()),
            named(me, horses::RIDER_BONE).and_then(|e| globals.get(e).ok()),
            space,
        ) {
            *offset += space.transform_vector3(saddle.translation() - pelvis.translation());
        }
        if let Ok(mut transform) = transforms.get_mut(root) {
            transform.translation += *offset;
        }
    }
}

/// The root node of the models' scenes, which their clips move (`__Root`).
const MODEL_ROOT: &str = "__Root";
