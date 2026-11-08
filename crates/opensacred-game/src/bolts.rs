//! The bolts: the Seraphim's (#754; docs/re/arts.md, "Her shots with 1018's and 1019's effects"
//! and "The Seraphim's bolts") and the Daemon's 1103's (#760, "The Daemon's"). While her effect 17
//! holds each of her shots makes two railgun bolts, the second [`RAILGUN_SECOND`] after the first,
//! and while her effect 16 holds a shot beyond 200 makes one BFG bolt (`attack`); his 1103 makes a
//! fan of them (`making`). A bolt leaves where its shooter stands along its line, at a hand's
//! height over the ground, flies straight at its speed, which may slow, for its time at most and
//! ends at a cell that may not be walked on; in each frame it finds the creatures on its level,
//! alive and hostile to the hero, nearer it than the sum of their sizes, and hits them with its
//! damages (the fight's hit of fixed damages, which always lands, #854): the railgun's every one it
//! found and goes, the BFG's those of its first contact and flies on harmless, the Daemon's the
//! first alone and goes. The Mage's fireball flies as a bolt too, of its own speed and time by its
//! level, and strikes every creature it found at its first contact and goes (`making`, #872). The
//! railgun's, the BFG's, the fireball's and the Daemon's look as the original's systems
//! (`bolt_looks`, #1092), told of each one's end, the whirlwind's and the fire bolt's as their
//! trails (`bolt_trails`), the splinters' as their icicles (`ice_splinters`), the bubble's as
//! its bush (`bubble_traps`).

use bevy::prelude::*;
use opensacred_rules::arts::{Bolt, EMPEROR_COURSE, RAILGUN_SECOND, bolt_damages};

use crate::crowd::InTheWay;
use crate::effects::Effects;
use crate::fight::{self, Combat, Health};
use crate::frame::{Playing, Step};
use crate::game::AppState;
use crate::hero::Body;
use crate::npcs::Dies;
use crate::terrain::{Heights, Level};
use crate::{ai, arrows, iso, paths};

/// How high over the ground a bolt of no system's look flies, in world units, and one whose
/// shooter has not the bone it leaves from (**hypothesis**: about a hand's height, as the arrows
/// leave a shooter without a hand).
pub(crate) const HEIGHT: f32 = 80.0;

/// The bones that a bolt of a system's look leaves from, by its course's look, which it keeps the
/// height of (the FX handler's cases `0x5A52C3`, `0x5A52F1`, `0x5A49D3` and `0x5A5900`, the
/// system flag `0x40` placing it once at the bone, #2018): the railgun's and the Daemon's bolt the
/// shooter's right hand (bone 2), and the bubble (the event `0x47`), the fireball, the fire
/// bolt and the splinters the left (bone 1, the events `0x82` and `0x3F`), the BFG the staff's
/// `weapon_fx01` (bone 15), the whirlwind the shooter's root (bone 0, the event `0x37`).
fn bone_of(look: usize) -> Option<&'static [&'static str]> {
    use crate::bolt_looks::Kind;
    match (look_of(look), look) {
        (Some(Kind::Railgun | Kind::Emperor), _) | (None, BUBBLE_LOOK) => Some(&["Bip01 R Hand"]),
        (Some(Kind::Fireball), _) | (None, FIRE_BOLT_LOOK | SPLINTERS_LOOK) => {
            Some(&["Bip01 L Hand"])
        }
        (Some(Kind::Bfg), _) => Some(&["weapon_fx01"]),
        (None, WHIRLWIND_LOOK) => Some(&["__Root"]),
        (None, _) => None,
    }
}

/// Whether a bolt of the kind `kind` ends as it goes under the ground (the system flag `0x01`,
/// `0x5AA3CE`): the fireball's (`0x51`) and the Daemon's (`0x53`), not the railgun's or the BFG's
/// (`0x50`).
fn ends_underground(kind: crate::bolt_looks::Kind) -> bool {
    use crate::bolt_looks::Kind;
    matches!(kind, Kind::Fireball | Kind::Emperor)
}

/// Where a bolt of the course's look `look` leaves its shooter `shooter` standing at `at`: at
/// the bone of its system's look ([`bone_of`]), at whose height it flies on, else [`HEIGHT`] over
/// its place, following the ground.
fn leaving(
    shooter: Entity,
    at: Vec3,
    look: usize,
    (children, bones, worn): (
        &Query<&Children>,
        &Query<(&Name, &GlobalTransform)>,
        &crate::making::Worn,
    ),
) -> (Vec3, Option<f32>) {
    bone_of(look)
        .and_then(|names| crate::making::bone(shooter, names, children, (bones, worn)))
        .map_or((at + Vec3::Y * HEIGHT, None), |bone| (bone, Some(bone.y)))
}

/// A shot's bolts leaving their shooter toward `toward` (the FX events of `0x4DCB80`), which the
/// shot's clip sends at its first hit (`attack`).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Fire {
    pub shooter: Entity,
    pub toward: Vec3,
    pub bolt: Bolt,
}

/// What a bolt strikes as it meets creatures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strikes {
    /// Every creature it finds in a frame, once, and it goes (the railgun's).
    All,
    /// Those of its first contact, once, and it flies on harmless (the BFG's).
    FirstContact,
    /// The first creature it finds alone, and it goes (the Daemon's 1103's).
    First,
    /// Every creature it touches, once a second, flying on through them (the Mage's whirlwinds
    /// and splinters).
    Passing,
}

/// How a bolt flies and strikes, and its look: its speed in units a second, how much slower it
/// grows each second, its most time, what it strikes and its look's index.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Course {
    pub speed: f32,
    pub slows: f32,
    pub life: f32,
    pub strikes: Strikes,
    pub look: usize,
    /// How near it strikes a creature, with the creature's size: its own size (`items.pak
    /// +0x14`, 50 for 0).
    pub reach: f32,
    /// How long one passing through waits between its strikes (`+0x11C`).
    pub beat: f32,
    /// Where it ends at a cell it may not pass, the reach and the factor of its burst (the
    /// Wood Elf's bubble's, 906).
    pub bursts: Option<(f32, f32)>,
    /// How far it knocks back what it strikes, before its strike, and the level below which
    /// (the Mage's whirlwind's, `opensacred_rules::spells::whirlwind_knocks`).
    pub knocks: Option<(f32, f32)>,
    /// The sounds it plays, where read (#1173).
    pub sounds: Sounds,
}

/// A bolt's sounds, each at it, at the volume 1 and the sample's own rate: as it leaves, as it
/// hits, and looping while it flies.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sounds {
    pub start: Option<u32>,
    pub hit: Option<u32>,
    pub looping: Option<u32>,
}

/// The fireball's system's sounds (`cParticleSystem_firestrike`): `MAGE_FEUERBALL_CAST` as it
/// starts (`0x76BE80`) and `MAGE_FEUERBALL_HIT` as its object hits (`0x76BBD0`, the flag 2); its
/// time out plays none.
const FIREBALL_SOUNDS: Sounds = Sounds {
    start: Some(2000),
    hit: Some(2001),
    looping: None,
};
/// The railgun's shot's `SERA_PLASMASHOT` (`0x4DDE12`), and the BFG's system's
/// `SERA_PLASMASHOT_BIG` as it starts (`cParticleSystem_bfg`, `0x77F520`).
const RAILGUN_SOUND: u32 = 2072;
const BFG_SOUND: u32 = 2073;

impl Course {
    /// The course of the Seraphim's bolt `bolt`.
    pub fn of(bolt: Bolt) -> Self {
        let (speed, life) = bolt.flight();
        let (strikes, look) = if bolt.goes() {
            (Strikes::All, 0)
        } else {
            (Strikes::FirstContact, 1)
        };
        Self {
            speed,
            slows: 0.0,
            life,
            strikes,
            look,
            reach: arrows::size(0),
            beat: 1.0,
            bursts: None,
            knocks: None,
            sounds: Sounds {
                start: Some(if bolt.goes() {
                    RAILGUN_SOUND
                } else {
                    BFG_SOUND
                }),
                ..Sounds::default()
            },
        }
    }

    /// The course of the Mage's whirlwinds and ice splinters, of their speed, time and reach
    /// (`opensacred_rules::spells::WHIRLWIND_COURSE`, `SPLINTERS_COURSE`).
    pub fn passing((speed, life, reach): (f32, f32, f32), look: usize) -> Self {
        Self {
            speed,
            slows: 0.0,
            life,
            strikes: Strikes::Passing,
            look,
            reach,
            beat: 1.0,
            bursts: None,
            knocks: None,
            sounds: Sounds::default(),
        }
    }

    /// The course of the Mage's fireball at the level `level` (the object 778, `0x76BE80`): every
    /// creature of its first contact, once, and it goes (`0x5ABD80` at `0x5AC015`), with its
    /// system's sounds ([`FIREBALL_SOUNDS`]).
    pub fn fireball(level: u32) -> Self {
        Self {
            speed: opensacred_rules::spells::fireball_speed(level),
            slows: 0.0,
            life: opensacred_rules::spells::fireball_life(level),
            strikes: Strikes::All,
            look: 3,
            reach: arrows::size(0),
            beat: 1.0,
            bursts: None,
            knocks: None,
            sounds: FIREBALL_SOUNDS,
        }
    }

    /// The course of the fire bolt of Anducar, his demon and Sharifo (the spell 52's
    /// `FX_SHADDAR_MAGIC`, 922; docs/re/arts.md, "The creatures' own spells"): straight on at 150 a
    /// second for 2 s, every creature of its first contact, once, its size 0 so 50; drawn as its
    /// trail (`bolt_trails`).
    pub fn fire_bolt() -> Self {
        Self {
            speed: 150.0,
            slows: 0.0,
            life: 2.0,
            strikes: Strikes::All,
            look: FIRE_BOLT_LOOK,
            reach: arrows::size(0),
            beat: 1.0,
            bursts: None,
            knocks: None,
            sounds: Sounds::default(),
        }
    }

    /// The course of a bolt of the Daemon's 1103 (`cParticleSystem_emperor`).
    pub fn emperor() -> Self {
        let (speed, slows, life) = EMPEROR_COURSE;
        Self {
            speed,
            slows,
            life,
            strikes: Strikes::First,
            look: 2,
            reach: arrows::size(0),
            beat: 1.0,
            bursts: None,
            knocks: None,
            sounds: Sounds::default(),
        }
    }
}

/// A bolt leaving its shooter along `direction` on the ground, of its course and its damages,
/// after `wait` seconds.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Launch {
    pub shooter: Entity,
    pub direction: Vec2,
    pub course: Course,
    pub damages: [f32; 4],
    pub wait: f32,
}

/// A bolt that an arrow's passing through bursts, the Wood Elf's bubble (847, `0x797D80`'s
/// signal 8; #2128): whether a hero cast it, its direction on the ground and its seconds, which
/// its look takes (`bubble_traps`).
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Bubble {
    pub(crate) hero: bool,
    pub(crate) direction: Vec2,
    pub(crate) seconds: f32,
}

/// The bursting of a [`Bubble`] by an arrow (`arrows`, `0x5BBCD0`).
#[derive(Message, Debug, Clone, Copy)]
pub(crate) struct Pops(pub(crate) Entity);

/// A bolt in flight: its course, its shooter, its direction on the ground, the level it flies on,
/// its damages, how long until it leaves, how long it has flown, and whether it hit already (a
/// BFG's, which flies on harmless).
#[derive(Component)]
struct Flying {
    course: Course,
    shooter: Entity,
    direction: Vec2,
    level: Level,
    damages: [f32; 4],
    wait: f32,
    flown: f32,
    spent: bool,
    /// How long until it strikes again, one passing through (`+0x11C`).
    cooldown: f32,
    /// Its system's look, and the height it flies level at where it left from its bone.
    kind: Option<crate::bolt_looks::Kind>,
    level_at: Option<f32>,
}

/// The look of a system of its own, by the course's look (`bolt_looks`).
fn look_of(look: usize) -> Option<crate::bolt_looks::Kind> {
    use crate::bolt_looks::Kind;
    match look {
        0 => Some(Kind::Railgun),
        1 => Some(Kind::Bfg),
        2 => Some(Kind::Emperor),
        3 => Some(Kind::Fireball),
        _ => None,
    }
}

/// The trail of a course's look (`bolt_trails`): the whirlwind's dust and the fire bolt's smoke.
fn trail_of(look: usize) -> Option<opensacred_rules::particles::TrailKind> {
    use opensacred_rules::particles::TrailKind;
    match look {
        WHIRLWIND_LOOK => Some(TrailKind::Windstrike),
        FIRE_BOLT_LOOK => Some(TrailKind::Shaddar),
        _ => None,
    }
}

/// The Mage's whirlwind's look (831).
const WHIRLWIND_LOOK: usize = 4;

/// The ice splinters' look (839).
const SPLINTERS_LOOK: usize = 5;

/// The Wood Elf's bubble's look (847).
const BUBBLE_LOOK: usize = 6;

/// The fire bolt's look (922).
const FIRE_BOLT_LOOK: usize = 7;

/// Whether a bolt of the look `look` asks its target's magic defence before it strikes
/// (`0x561860`, #2277; `opensacred_rules::spells::warded`): the railgun's (801), the BFG's (875),
/// the fireball's (778), the whirlwind's (831), the splinters' (839) and the bubble's (847) do;
/// the Daemon's of 1103 (1159) and the fire bolt (922) do not.
fn warded(look: usize) -> bool {
    !matches!(look, 2 | FIRE_BOLT_LOOK)
}

/// Makes the launches of each of the Seraphim's shots: its bolts along the line on the ground to
/// `toward` (with none, her facing), of the damages of the bolt's record by the whole value of
/// its effect and her multipliers, the second railgun bolt [`RAILGUN_SECOND`] after the first.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn fire(
    mut shots: MessageReader<Fire>,
    mut launches: MessageWriter<Launch>,
    arts: Option<Res<crate::arts::HeroArts>>,
    shooters: Query<(&Transform, Option<&Combat>, Option<&Effects>), With<Body>>,
) {
    let Some(arts) = arts else {
        shots.clear();
        return;
    };
    for shot in shots.read() {
        let Ok((at, combat, effects)) = shooters.get(shot.shooter) else {
            continue;
        };
        let level = effects
            .and_then(|effects| {
                effects
                    .list()
                    .into_iter()
                    .find(|effect| effect.id == shot.bolt.effect())
            })
            .map_or(1, |effect| effect.whole);
        let Some(spell) = arts
            .tables
            .spells
            .iter()
            .find(|spell| spell.id == shot.bolt.spell())
        else {
            warn!("bolts: no record {} of the first table", shot.bolt.spell());
            continue;
        };
        let multipliers = combat.map_or([1.0; 4], |combat| combat.multipliers);
        let damages = bolt_damages(spell, level, multipliers);
        let direction = (shot.toward - at.translation)
            .xz()
            .try_normalize()
            .unwrap_or_else(|| (at.rotation * Vec3::X).xz().normalize_or(Vec2::X));
        debug!(
            "bolts: {} fires {} {:?} of {damages:?}",
            shot.shooter,
            shot.bolt.count(),
            shot.bolt
        );
        for i in 0..shot.bolt.count() {
            launches.write(Launch {
                shooter: shot.shooter,
                direction,
                course: Course::of(shot.bolt),
                damages,
                wait: if i == 0 { 0.0 } else { RAILGUN_SECOND },
            });
        }
    }
}

/// Makes the bolt of each launch where its shooter stands, on its level, at its bone or at
/// [`HEIGHT`] ([`leaving`]): the hero's, or a creature's that casts (#1852); its system's start's
/// sound at it, where it has one.
fn launch(
    mut commands: Commands,
    mut launches: MessageReader<Launch>,
    shooters: Query<(&Transform, Option<&Body>, Option<&InTheWay>)>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    mut sounds: MessageWriter<crate::sfx::Play>,
) {
    for launched in launches.read() {
        let Ok((at, body, way)) = shooters.get(launched.shooter) else {
            continue;
        };
        let level = match (body, way) {
            (Some(body), _) => body.level(),
            (None, Some(way)) => way.level,
            (None, None) => None,
        };
        let system = look_of(launched.course.look);
        let trail = trail_of(launched.course.look);
        let splinters = launched.course.look == SPLINTERS_LOOK;
        // Every course's look is drawn as its own; a look of none makes no bolt.
        let own =
            system.is_some() || trail.is_some() || splinters || launched.course.look == BUBBLE_LOOK;
        if !own {
            continue;
        }
        let (start, level_at) = leaving(
            launched.shooter,
            at.translation,
            launched.course.look,
            (&children, &bones, &worn),
        );
        debug!(
            "bolts: {:?} of {} leaves at {start}, its shooter at {}",
            system, launched.shooter, at.translation
        );
        let mut bolt = commands.spawn((
            Transform::from_translation(start),
            if launched.wait > 0.0 {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            },
            Flying {
                course: launched.course,
                shooter: launched.shooter,
                direction: launched.direction.normalize_or(Vec2::Y),
                level,
                damages: launched.damages,
                wait: launched.wait,
                flown: 0.0,
                spent: false,
                cooldown: 0.0,
                kind: system,
                level_at,
            },
            DespawnOnExit(AppState::InGame),
        ));
        if launched.course.bursts.is_some() {
            bolt.insert(Bubble {
                hero: body.is_some(),
                direction: launched.direction,
                seconds: launched.course.life,
            });
        }
        if let Some(trail) = trail {
            bolt.insert(crate::bolt_trails::Trail(trail));
        }
        if splinters {
            bolt.insert(crate::ice_splinters::Splinters {
                hero: body.is_some(),
                direction: launched.direction,
            });
        }
        if let Some(kind) = system {
            let id = bolt.id();
            crate::bolt_looks::spawn(
                bolt.commands_mut(),
                id,
                kind,
                launched.direction,
                body.is_some(),
            );
        }
        // Its system's loop while it flies (`crate::hums`).
        if let Some(looping) = launched.course.sounds.looping {
            bolt.insert(crate::hums::Hum(looping));
        }
        // The railgun's once a shot, with its first bolt (`0x4DDE12` after `0x4DDD2A`'s object).
        if let Some(start) = launched.course.sounds.start
            && launched.wait <= 0.0
        {
            sounds.write(crate::sfx::Play {
                sound: crate::sfx::Of::Id(start),
                at: Some(bolt.id()),
                rate: (0.5, crate::sfx::Roll::None),
            });
        }
    }
}

/// How many world units a bolt goes along the world's unit heading `heading` on the ground for
/// each unit of its speed (#2018; docs/re/effects.md, "Their flight"): its system flies in a frame
/// whose axes lie along the screen's horizontal and vertical on the ground, the one of
/// `MODEL_SCALE` world units a unit, the other foreshortened by the camera's tilt, `2/√5` of that
/// (`0x5AA330` taking the system's place to the ground by `0x623AD0`): 1.43 along a world axis,
/// 1.52 across the screen, 1.36 up and down it (**seen**: the railgun of 400 going 570 a second
/// along the world's x and along its y).
pub(crate) fn flight_scale(heading: Vec2) -> f32 {
    let across = iso::right().xz().normalize_or(Vec2::X);
    let up = Vec2::new(across.y, -across.x);
    let (a, b) = (heading.dot(across), heading.dot(up));
    let (x, y) = (iso::MODEL_SCALE, iso::MODEL_SCALE * 2.0 / 5.0_f32.sqrt());
    1.0 / ((a / x).powi(2) + (b / y).powi(2)).sqrt().max(f32::EPSILON)
}

/// Flies each bolt a frame on (`0x5A6680`) and hits what it comes on (`0x5AB490`, `0x5ABD80`),
/// telling its look of its end (`bolt_looks::Ended`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    heights: Res<Heights>,
    (mut hits, ranks, mut sounds, mut ends, mut pops): (
        MessageWriter<fight::Hit>,
        Query<&fight::Ranked>,
        MessageWriter<crate::sfx::Play>,
        MessageWriter<crate::bolt_looks::Ended>,
        MessageReader<Pops>,
    ),
    mut flying: Query<(Entity, &mut Flying, &mut Transform, &mut Visibility)>,
    (shooters, heroes, children, bones, worn): (
        Query<&Transform, (With<Body>, Without<Flying>)>,
        Query<(Entity, &Transform, &Body, &Health), (Without<Flying>, Without<Dies>)>,
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    creatures: Query<
        (
            Entity,
            &Transform,
            &InTheWay,
            &ai::Mind,
            &Health,
            Option<&arrows::Size>,
        ),
        (Without<Body>, Without<Flying>, Without<Dies>),
    >,
) {
    let dt = time.delta_secs();
    let popped: Vec<Entity> = pops.read().map(|pops| pops.0).collect();
    for (entity, mut bolt, mut transform, mut visibility) in &mut flying {
        // A bolt that waits leaves its shooter's place as its time comes.
        if bolt.wait > 0.0 {
            bolt.wait -= dt;
            if let Ok(shooter) = shooters.get(bolt.shooter) {
                let (start, level_at) = leaving(
                    bolt.shooter,
                    shooter.translation,
                    bolt.course.look,
                    (&children, &bones, &worn),
                );
                transform.translation = start;
                bolt.level_at = level_at;
            }
            if bolt.wait <= 0.0 {
                *visibility = Visibility::Inherited;
            }
            continue;
        }
        let course = bolt.course;
        let speed =
            (course.speed - course.slows * bolt.flown).max(0.0) * flight_scale(bolt.direction);
        bolt.flown += dt;
        let step = bolt.direction * speed * dt;
        let ground = transform.translation.xz() + step;
        let cell = paths::cell_of(ground.x, ground.y);
        let standing = heights.standing(ground.x, ground.y, bolt.level);
        // A fireball or the Daemon's bolt under the ground ends at once (`+0x74` 1).
        let under = bolt.kind.is_some_and(ends_underground)
            && bolt
                .level_at
                .zip(standing)
                .is_some_and(|(flies, ground)| flies < ground);
        if under {
            debug!(
                "bolts: {entity} goes under the ground and ends, at {:?} over {standing:?}",
                bolt.level_at
            );
            ends.write(crate::bolt_looks::Ended {
                bolt: entity,
                at: transform.translation,
                hit: None,
                timed: false,
                at_once: true,
            });
            commands.entity(entity).despawn();
            continue;
        }
        let bursts = popped.contains(&entity);
        if bursts || bolt.flown >= course.life || !heights.passes(cell, bolt.level) {
            debug!(
                "bolts: {entity} of {} ends at {:.0},{:.0}",
                bolt.shooter, ground.x, ground.y
            );
            // A bubble that an arrow passes through bursts, striking about it at its factor; a
            // cell that stops it ends it without (#2128). A creature's (#1854) on the hero alone.
            if let Some((reach, factor)) = course.bursts.filter(|_| bursts) {
                let here = transform.translation.xz();
                let burst: Vec<Entity> = if shooters.contains(bolt.shooter) {
                    creatures
                        .iter()
                        .filter(|(_, at, way, mind, health, size)| {
                            way.creature
                                && way.level == bolt.level
                                && mind.hostile()
                                && health.current > 0
                                && (at.translation.xz() - here).length()
                                    < reach + size.map_or(arrows::size(0), |size| size.0)
                        })
                        .map(|(target, ..)| target)
                        .collect()
                } else {
                    heroes
                        .iter()
                        .filter(|(_, at, body, health)| {
                            body.level() == bolt.level
                                && health.current > 0
                                && (at.translation.xz() - here).length() < reach + arrows::size(0)
                        })
                        .map(|(target, ..)| target)
                        .collect()
                };
                for target in burst {
                    debug!("bolts: {entity} bursts on {target}");
                    hits.write(fight::Hit {
                        attacker: bolt.shooter,
                        target,
                        share: 1.0,
                        ranged: false,
                        art: None,
                        fixed: Some(bolt.damages.map(|d| (d * factor).trunc())),
                        sure: true,
                        poison: false,
                        ward: true,
                    });
                }
            }
            ends.write(crate::bolt_looks::Ended {
                bolt: entity,
                at: transform.translation,
                hit: None,
                timed: bolt.flown >= course.life,
                at_once: false,
            });
            commands.entity(entity).despawn();
            continue;
        }
        let height = bolt
            .level_at
            .unwrap_or_else(|| standing.unwrap_or(transform.translation.y - HEIGHT) + HEIGHT);
        transform.translation = Vec3::new(ground.x, height, ground.y);
        bolt.cooldown = (bolt.cooldown - dt).max(0.0);
        if bolt.spent || bolt.cooldown > 0.0 {
            continue;
        }
        // The creatures on its level, alive and hostile to the hero, nearer than the two sizes;
        // a creature's bolt (a cast's, #1852) the hero alone.
        let own = course.reach;
        let mut struck: Vec<Entity> = if shooters.contains(bolt.shooter) {
            creatures
                .iter()
                .filter(|(_, at, way, mind, health, size)| {
                    way.creature
                        && way.level == bolt.level
                        && mind.hostile()
                        && health.current > 0
                        && (at.translation.xz() - ground).length()
                            < own + size.map_or(arrows::size(0), |size| size.0)
                })
                .map(|(e, ..)| e)
                .collect()
        } else {
            heroes
                .iter()
                .filter(|(_, at, body, health)| {
                    body.level() == bolt.level
                        && health.current > 0
                        && (at.translation.xz() - ground).length() < own + arrows::size(0)
                })
                .map(|(e, ..)| e)
                .collect()
        };
        if struck.is_empty() {
            continue;
        }
        if course.strikes == Strikes::First {
            struck.truncate(1);
        }
        // Its system's hit's sound, at the creature it hits first, the bolt going in this frame.
        if let Some(hit) = course.sounds.hit {
            sounds.write(crate::sfx::Play {
                sound: crate::sfx::Of::Id(hit),
                at: struck.first().copied(),
                rate: (0.5, crate::sfx::Roll::None),
            });
        }
        // The whirlwind's knockback first, on each of no big type below its level (`0x5AD135`).
        if let Some((left, below)) = course.knocks {
            for &target in &struck {
                let boss = ranks
                    .get(target)
                    .is_ok_and(|rank| rank.0 == fight::Rank::Boss);
                if let Ok((_, _, _, mind, ..)) = creatures.get(target)
                    && opensacred_rules::spells::whirlwind_knocks_back(
                        below, mind.level, boss, mind.big,
                    )
                {
                    debug!("bolts: {entity} knocks {target} back {left:.0}");
                    commands
                        .entity(target)
                        .remove::<(crate::npcs::Walk, crate::npcs::Strikes)>()
                        .insert(crate::making::Knocked {
                            by: bolt.shooter,
                            from: transform.translation,
                            left,
                            held: 0.0,
                            throw: None,
                        });
                }
            }
        }
        for &target in &struck {
            debug!(
                "bolts: {entity} of {} hits {target} for {:?}",
                bolt.shooter, bolt.damages
            );
            hits.write(fight::Hit {
                attacker: bolt.shooter,
                target,
                share: 1.0,
                ranged: false,
                art: None,
                fixed: Some(bolt.damages),
                sure: true,
                poison: false,
                ward: warded(course.look),
            });
        }
        match course.strikes {
            Strikes::FirstContact => bolt.spent = true,
            Strikes::Passing => bolt.cooldown = course.beat,
            Strikes::All | Strikes::First => {
                ends.write(crate::bolt_looks::Ended {
                    bolt: entity,
                    at: transform.translation,
                    hit: struck.first().copied(),
                    timed: false,
                    at_once: false,
                });
                commands.entity(entity).despawn();
            }
        }
    }
}

/// The bolts' systems, after which their looks follow them (`bolt_looks`).
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Flown;

/// The bolts.
pub struct BoltsPlugin;

impl Plugin for BoltsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Fire>()
            .add_message::<Launch>()
            .add_message::<Pops>()
            .add_systems(
                Update,
                (fire, launch, fly)
                    .chain()
                    .after(Step::Npcs)
                    .in_set(Playing)
                    .in_set(Flown),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bolts_leave_from_their_systems_bones() {
        use crate::bolt_looks::Kind;
        assert_eq!(bone_of(0), Some(&["Bip01 R Hand"][..]));
        assert_eq!(bone_of(2), Some(&["Bip01 R Hand"][..]));
        assert_eq!(bone_of(3), Some(&["Bip01 L Hand"][..]));
        assert_eq!(bone_of(1), Some(&["weapon_fx01"][..]));
        assert_eq!(bone_of(WHIRLWIND_LOOK), Some(&["__Root"][..]));
        assert_eq!(bone_of(FIRE_BOLT_LOOK), Some(&["Bip01 L Hand"][..]));
        assert_eq!(bone_of(SPLINTERS_LOOK), Some(&["Bip01 L Hand"][..]));
        assert_eq!(bone_of(BUBBLE_LOOK), Some(&["Bip01 R Hand"][..]));
        assert_eq!(bone_of(8), None);
        // The flag 0x01: the fireball's and the Daemon's end under the ground.
        assert!(ends_underground(Kind::Fireball) && ends_underground(Kind::Emperor));
        assert!(!ends_underground(Kind::Railgun) && !ends_underground(Kind::Bfg));
    }

    #[test]
    fn a_bolt_flies_faster_across_the_screen_than_up_it() {
        // Along a world axis, as the railgun of 400 went 570 a second (#2018).
        assert!(
            (flight_scale(Vec2::X) - 1.429).abs() < 0.002,
            "{}",
            flight_scale(Vec2::X)
        );
        assert!((flight_scale(Vec2::Y) - 1.429).abs() < 0.002);
        let across = iso::right().xz().normalize();
        assert!((flight_scale(across) - iso::MODEL_SCALE).abs() < 1e-4);
        let up = Vec2::new(across.y, -across.x);
        let tilted = iso::MODEL_SCALE * 2.0 / 5.0_f32.sqrt();
        assert!((flight_scale(up) - tilted).abs() < 1e-4);
    }
}
