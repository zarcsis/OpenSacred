//! What the hero's spells do as they act (#872, #873; docs/re/arts.md, "The spells' cast", "The
//! objects' actions", "The Mage's spells"): `making` casts a spell and, once the share of its
//! clip still to play falls below the spell's, sends [`Acts`]. The fireball leaves for its
//! target's place as a bolt (`bolts`); the regeneration gives its bearer its record's effects
//! (`opensacred_rules::spells::effects`) at once, and the stone skin as its shield's clock passes
//! [`SHIELD_TIME`] at the cast's rate, and the crusader's strength after [`STRENGTH_TIME`]; the
//! holy shield turns arrows aside (`arrows`); the irritation and the conversion hold a hostile
//! creature with the effect 13, under which it leaves the hero alone (`ai`), and 14, which turns
//! its sides over (`effects`); the healing adds its health to its bearer's, its most at most. A spell that lasts on its bearer gives it the effect `0x22` too. The Seraphim's
//! lightning strikes its target at once; her holy light stands at its point, and her blades of
//! light and her light go with their bearer, each striking about itself once a second for its
//! time ([`Aura`]). Their particle systems are stood in for by a flash and translucent discs
//! (docs/decisions/2026-10-24-bolts-look.md).

use bevy::prelude::*;
use opensacred_rules::arts::bolt_damages;
use opensacred_rules::fight::Effect;
use opensacred_rules::spells::{
    self, BEAT, BORNE, CONVERSION, CRUSADERS_STRENGTH, FIRE_SKIN, FIREBALL, HEALING, HOLY_SHIELD,
    IRRITATION, LIGHTNING, LIGHTNING_SHOWS, QUICKENING, REIKI, SHIELD_TIME, SHIELD_WALL, SPRING,
    STONE_SKIN, STRENGTH_TIME, WALL_TIME,
};

use crate::crowd::InTheWay;
use crate::effects::Effects;
use crate::fight::{self, Combat, Health};
use crate::frame::{Playing, Step};
use crate::game::AppState;
use crate::hero::Body;
use crate::npcs::Dies;
use crate::terrain::Level;
use crate::{ai, arrows};

/// A spell acting: its caster, the spell, its target (the caster for one on itself), the point
/// it was aimed at, and its level in play.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Acts {
    pub caster: Entity,
    pub spell: u16,
    pub target: Entity,
    pub point: Vec3,
    pub level: u32,
}

/// A spell's effects waiting for its object's clock, the stone skin's or the crusader's
/// strength's: what is left of it in the world's seconds.
#[derive(Component, Debug, Clone)]
struct Shielding {
    left: f32,
    effects: Vec<(Effect, f32)>,
}

/// An object that strikes about itself once a [`BEAT`] for its time (`opensacred_rules::spells::
/// area`): its caster, the bearer it follows, if any, its reach, the classes it strikes alone,
/// its damages, if any, the effects it gives a creature it strikes that holds none keyed by its
/// spell `key` (the Daemon's fear's), the level it strikes on, its time left and its cooldown.
#[derive(Component, Debug, Clone)]
struct Aura {
    caster: Entity,
    follows: Option<Entity>,
    reach: f32,
    classes: &'static [u8],
    damages: Option<[f32; 4]>,
    gives: Vec<(Effect, f32)>,
    key: u16,
    level: Level,
    left: f32,
    cooldown: f32,
    /// Whom it strikes: about itself, the creature it follows alone (the choir's), or one
    /// creature at random by the hellfire's choice.
    strikes: Strikes,
}

/// Whom an [`Aura`] strikes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Strikes {
    About,
    Bearer,
    OneAtRandom,
}

/// The Daemon's hell disk, the Mage's purgatory or a lost soul of the Wood Elf's: its caster, the
/// creature it homes on, the one it struck last, its damages, its level, its time left, its
/// cooldown, its speed, how near it strikes a creature with the creature's size, and whether it
/// strikes every creature within its reach rather than the first it touches (the purgatory's).
#[derive(Component, Debug, Clone)]
struct Disk {
    caster: Entity,
    target: Option<Entity>,
    last: Option<Entity>,
    damages: [f32; 4],
    level: Level,
    left: f32,
    cooldown: f32,
    speed: f32,
    reach: f32,
    all: bool,
}

/// A zone of the banishing circle (the kind 2, `0xAD2530`): its centre and cell, its level, and
/// its time left.
#[derive(Component, Debug, Clone)]
struct Banishing {
    centre: Vec3,
    cell: (i32, i32),
    level: Level,
    left: f32,
}

/// A creature that the Wood Elf's changeling changed into an animal (the effect 20's adder,
/// `0x5848A8`): hidden, its mind and its place in the way kept for its change back.
#[derive(Component, Clone, Debug)]
struct Changed {
    mind: Box<ai::Mind>,
    way: InTheWay,
}

/// The animal that a changed creature, `original`, is for its time left (the effect 20 keyed by
/// 36, `0x795E20`).
#[derive(Component, Clone, Copy, Debug)]
struct Changeling {
    original: Entity,
    left: f32,
}

/// The Dark Elf's trap's ball (867, `cParticleSystem_ball`): its caster, its art and level in
/// play, its damages' multipliers, where it flies from and to, its level, the part of its course
/// flown, and whether its look is on.
#[derive(Component, Debug, Clone)]
struct Ball {
    caster: Entity,
    art: u16,
    spell_level: u32,
    multipliers: [f32; 4],
    from: Vec3,
    to: Vec3,
    level: Level,
    s: f32,
    shown: bool,
}

/// The pentagram 782 of the trap 1050 (`0x772C40`): its caster, its damages, its level, the time
/// until it is armed and its life, and the level at or below which it slays outright.
#[derive(Component, Debug, Clone)]
struct Pentagram {
    caster: Entity,
    damages: [f32; 4],
    level: Level,
    arms: f32,
    left: f32,
    slays: u32,
}

/// The confusion 871 of the trap 1051 (`0x778F40`): the trap's level in play, its caster's level,
/// its level, its life and its cooldown.
#[derive(Component, Debug, Clone)]
struct Confusion {
    spell_level: u32,
    caster_level: u32,
    level: Level,
    left: f32,
    cooldown: f32,
}

/// The fog's zone of the trap 1053 (the kind 4, `0xAD2530`): its cell, its level, the trap's
/// level in play and its time left.
#[derive(Component, Debug, Clone)]
struct Fog {
    cell: (i32, i32),
    level: Level,
    spell_level: u32,
    left: f32,
}

/// Throws the trap `art` at the level in play `spell_level` of `caster` from `from` to `to` on
/// `level`, its damages' multipliers `multipliers` (`0x506770`): a ball that makes the trap
/// where it lands.
pub fn throw_trap(
    commands: &mut Commands,
    caster: Entity,
    (art, spell_level): (u16, u32),
    multipliers: [f32; 4],
    (from, to, level): (Vec3, Vec3, Level),
) {
    commands.spawn((
        Ball {
            caster,
            art,
            spell_level,
            multipliers,
            from,
            to,
            level,
            s: 0.0,
            shown: false,
        },
        Transform::from_translation(from),
        Visibility::Inherited,
        DespawnOnExit(AppState::InGame),
    ));
}

/// Flies each trap's ball on its arc to its point in 2/3 s (`0x5A706C`), gone without a trap at
/// a cell it may not cross, and makes its trap where it lands, of its spell's record at the
/// trap's level (`0x79E730`, `0x5B2600`'s action 1): the explosion 794 striking once within 80,
/// the pentagram 782, the confusion 871, the ground's poison 792 striking within 80 once a
/// second, or the fog's zone with its four sights 868.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their \
              filters"
)]
fn balls(
    mut commands: Commands,
    time: Res<Time>,
    arts: Option<Res<crate::arts::HeroArts>>,
    looks: Option<Res<Looks>>,
    heights: Res<crate::terrain::Heights>,
    experience: Res<fight::Experience>,
    mut hits: MessageWriter<fight::Hit>,
    mut flying: Query<(Entity, &mut Ball, &mut Transform)>,
    creatures: Query<
        (
            Entity,
            &Transform,
            &InTheWay,
            &ai::Mind,
            &Health,
            Option<&arrows::Size>,
        ),
        (Without<Body>, Without<Dies>, Without<Ball>),
    >,
) {
    let dt = time.delta_secs();
    for (entity, mut ball, mut transform) in &mut flying {
        if !ball.shown {
            ball.shown = true;
            if let Some(looks) = &looks {
                commands.entity(entity).with_child((
                    Mesh3d(looks.soul.0.clone()),
                    MeshMaterial3d(looks.flame.1.clone()),
                ));
            }
        }
        ball.s = (ball.s + opensacred_rules::arts::BALL_RATE * dt).min(1.0);
        let at = ball.from.lerp(ball.to, ball.s);
        let cell = crate::paths::cell_of(at.x, at.z);
        if !heights.walkable_on(cell, ball.level) {
            debug!("spells: the trap's ball {entity} breaks at {cell:?}");
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation = at + Vec3::Y * opensacred_rules::arts::ball_rise(ball.s);
        if ball.s < 1.0 {
            continue;
        }
        commands.entity(entity).despawn();
        let spell = opensacred_rules::arts::trap_spell(ball.art);
        let Some(record) = arts
            .as_ref()
            .and_then(|arts| arts.tables.spells.iter().find(|s| s.id == spell))
        else {
            continue;
        };
        let level = i32::try_from(ball.spell_level).unwrap_or(1);
        let damages = bolt_damages(record, level, ball.multipliers);
        let seconds = fight_seconds(spells::value(record, 0, ball.spell_level));
        let here = ball.to;
        info!(
            "spells: the trap {} lands at {here} of the spell {spell}",
            ball.art
        );
        let disc = |radius: f32, look: usize| {
            looks.as_ref().map(|looks| {
                (
                    Mesh3d(looks.disc.clone()),
                    MeshMaterial3d(looks.discs[look].clone()),
                    Transform::from_scale(Vec3::new(radius, 2.0, radius))
                        .with_translation(Vec3::Y * 4.0),
                )
            })
        };
        match ball.art {
            1049 => {
                // The explosion strikes once every hostile creature within 80 and its size.
                for (target, at, way, mind, health, size) in &creatures {
                    let reach = opensacred_rules::arts::EXPLOSION_REACH
                        + size.map_or(arrows::size(0), |size| size.0);
                    if way.creature
                        && way.level == ball.level
                        && mind.hostile()
                        && health.current > 0
                        && (at.translation - here).xz().length() < reach
                    {
                        hits.write(fight::Hit {
                            attacker: ball.caster,
                            target,
                            share: 1.0,
                            ranged: false,
                            art: None,
                            fixed: Some(damages),
                            sure: true,
                        });
                    }
                }
                if let Some(looks) = &looks {
                    commands.spawn((
                        Mesh3d(looks.blast.0.clone()),
                        MeshMaterial3d(looks.blast.1.clone()),
                        Transform::from_translation(here),
                        Fades(0.3),
                        DespawnOnExit(AppState::InGame),
                    ));
                }
            }
            1050 => {
                let mut made = commands.spawn((
                    Pentagram {
                        caster: ball.caster,
                        damages,
                        level: ball.level,
                        arms: opensacred_rules::arts::PENTAGRAM_ARMS,
                        left: seconds,
                        slays: spells::value(record, 1, ball.spell_level),
                    },
                    Transform::from_translation(here),
                    Visibility::Inherited,
                    DespawnOnExit(AppState::InGame),
                ));
                if let Some(disc) = disc(opensacred_rules::arts::PENTAGRAM_REACH, 5) {
                    made.with_child(disc);
                }
            }
            1051 => {
                let mut made = commands.spawn((
                    Confusion {
                        spell_level: ball.spell_level,
                        caster_level: experience.level,
                        level: ball.level,
                        left: seconds,
                        cooldown: 0.0,
                    },
                    Transform::from_translation(here),
                    Visibility::Inherited,
                    DespawnOnExit(AppState::InGame),
                ));
                if let Some(disc) = disc(opensacred_rules::arts::CONFUSION_REACH, 2) {
                    made.with_child(disc);
                }
            }
            1052 => {
                let mut made = commands.spawn((
                    Aura {
                        caster: ball.caster,
                        follows: None,
                        reach: opensacred_rules::arts::POISON_REACH,
                        classes: &[],
                        damages: Some(damages),
                        gives: Vec::new(),
                        key: ball.art,
                        level: ball.level,
                        left: seconds,
                        cooldown: 0.0,
                        strikes: Strikes::About,
                    },
                    Transform::from_translation(here),
                    Visibility::Inherited,
                    DespawnOnExit(AppState::InGame),
                ));
                if let Some(disc) = disc(opensacred_rules::arts::POISON_REACH, 6) {
                    made.with_child(disc);
                }
            }
            _ => {
                let mut made = commands.spawn((
                    Fog {
                        cell: crate::paths::cell_of(here.x, here.z),
                        level: ball.level,
                        spell_level: ball.spell_level,
                        left: seconds,
                    },
                    Transform::from_translation(here),
                    Visibility::Inherited,
                    DespawnOnExit(AppState::InGame),
                ));
                let from = opensacred_rules::arts::FOG_FROM;
                if let Some(looks) = &looks {
                    made.with_children(|sights| {
                        for (x, z) in [(from, from), (from, -from), (-from, from), (-from, -from)] {
                            sights.spawn((
                                Mesh3d(looks.disc.clone()),
                                MeshMaterial3d(looks.discs[4].clone()),
                                Transform::from_translation(Vec3::new(x, 4.0, z))
                                    .with_scale(Vec3::new(65.0, 2.0, 65.0)),
                            ));
                        }
                    });
                }
            }
        }
    }
}

/// Arms each pentagram 2 s after it is made, and bursts it on the first hostile creature within
/// 50 and its size, once (`0x772C40`, the hits `0x30E`): each hostile creature so near takes its
/// fire and poison, or one of a level at or below its value 1 is slain; past its life it goes.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn pentagrams(
    mut commands: Commands,
    time: Res<Time>,
    mut hits: MessageWriter<fight::Hit>,
    mut pentagrams: Query<(Entity, &mut Pentagram, &Transform)>,
    creatures: Query<
        (
            Entity,
            &Transform,
            &InTheWay,
            &ai::Mind,
            &Health,
            Option<&arrows::Size>,
        ),
        (Without<Body>, Without<Dies>, Without<Pentagram>),
    >,
) {
    let dt = time.delta_secs();
    for (entity, mut pentagram, at) in &mut pentagrams {
        pentagram.arms -= dt;
        pentagram.left -= dt;
        if pentagram.left <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        if pentagram.arms > 0.0 {
            continue;
        }
        let here = at.translation;
        let near: Vec<(Entity, u32)> = creatures
            .iter()
            .filter(|(_, there, way, mind, health, size)| {
                let reach = opensacred_rules::arts::PENTAGRAM_REACH
                    + size.map_or(arrows::size(0), |size| size.0);
                way.creature
                    && way.level == pentagram.level
                    && mind.hostile()
                    && health.current > 0
                    && (there.translation - here).xz().length() < reach
            })
            .map(|(target, _, _, _, health, _)| (target, health.level))
            .collect();
        if near.is_empty() {
            continue;
        }
        debug!(
            "spells: the pentagram {entity} bursts on {} creatures",
            near.len()
        );
        for (target, level) in near {
            // The event 9, the kill, for one at or below its value 1 (**hypothesis**: slain).
            let damages = if level <= pentagram.slays {
                [f32::from(u16::MAX - 1), 0.0, 0.0, 0.0]
            } else {
                pentagram.damages
            };
            hits.write(fight::Hit {
                attacker: pentagram.caster,
                target,
                share: 1.0,
                ranged: false,
                art: None,
                fixed: Some(damages),
                sure: true,
            });
        }
        commands.entity(entity).despawn();
    }
}

/// Once a second gives each hostile creature within 160 and its size of each confusion, no boss
/// and without the effect 13 or 14, by a coin the effect 14, which turns its sides over, or 13,
/// under which it leaves the hero alone, for `3·min(1, 5·L/M)` s, stopping it (`0x5AD8D0`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn confusions(
    mut commands: Commands,
    time: Res<Time>,
    mut dice: ResMut<Dice>,
    mut clouds: Query<(Entity, &mut Confusion, &Transform)>,
    creatures: Query<
        (
            Entity,
            &Transform,
            &InTheWay,
            &ai::Mind,
            &Health,
            Option<&fight::Ranked>,
            Option<&Effects>,
            Option<&arrows::Size>,
        ),
        (Without<Body>, Without<Dies>, Without<Confusion>),
    >,
) {
    let dt = time.delta_secs();
    for (entity, mut cloud, at) in &mut clouds {
        cloud.left -= dt;
        cloud.cooldown -= dt;
        if cloud.left <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        if cloud.cooldown > 0.0 {
            continue;
        }
        cloud.cooldown = BEAT;
        let here = at.translation;
        for (target, there, way, mind, health, rank, effects, size) in &creatures {
            let reach = opensacred_rules::arts::CONFUSION_REACH
                + size.map_or(arrows::size(0), |size| size.0);
            let boss = rank.is_some_and(|rank| rank.0 == fight::Rank::Boss);
            let confused = effects.is_some_and(|effects| {
                effects.holds(opensacred_rules::arts::DOMINATED)
                    || effects.holds(opensacred_rules::spells::CONVERTED)
            });
            if !way.creature
                || way.level != cloud.level
                || !mind.hostile()
                || health.current == 0
                || boss
                || confused
                || (there.translation - here).xz().length() >= reach
            {
                continue;
            }
            let id = if dice.0.below(2) == 0 {
                opensacred_rules::spells::CONVERTED
            } else {
                opensacred_rules::arts::DOMINATED
            };
            let seconds =
                opensacred_rules::arts::confusion_seconds(cloud.spell_level, health.level);
            let effect = Effect {
                whole: i32::try_from(cloud.caster_level).unwrap_or(1),
                ..Effect::new(id, 0.0)
            };
            debug!("spells: the confusion gives {target} the effect {id} for {seconds} s");
            let mut confused = commands.entity(target);
            confused.remove::<(crate::npcs::Walk, crate::npcs::Strikes)>();
            confused
                .entry::<Effects>()
                .or_default()
                .and_modify(move |mut effects| effects.add(effect, seconds));
        }
    }
}

/// Counts each fog's zone down, and gives each hostile creature on its level that stands in it,
/// of no boss and a level up to `5·L + 3`, the effect 13 for 1 s, again while it stays, under
/// which it leaves the hero alone (the zone's kind 4, `0x59F960`, `0x5941A0`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn fogs(
    mut commands: Commands,
    time: Res<Time>,
    mut zones: Query<(Entity, &mut Fog)>,
    creatures: Query<
        (
            Entity,
            &Transform,
            &InTheWay,
            &ai::Mind,
            &Health,
            Option<&fight::Ranked>,
            Option<&Effects>,
        ),
        (Without<Body>, Without<Dies>),
    >,
) {
    let dt = time.delta_secs();
    for (entity, mut zone) in &mut zones {
        zone.left -= dt;
        if zone.left <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        for (creature, at, way, mind, health, rank, effects) in &creatures {
            let (x, y) = crate::paths::cell_of(at.translation.x, at.translation.z);
            let inside = (x - zone.cell.0).abs() <= spells::ZONE_REACH
                && (y - zone.cell.1).abs() <= spells::ZONE_REACH;
            // Given again before it goes, so that no tick sees it gone while the creature stays.
            let held = effects.is_some_and(|effects| {
                effects.timed().iter().any(|timed| {
                    timed.effect.id == opensacred_rules::arts::DOMINATED && timed.left > 0.5
                })
            });
            let boss = rank.is_some_and(|rank| rank.0 == fight::Rank::Boss);
            if !inside
                || held
                || way.level != zone.level
                || !mind.hostile()
                || !opensacred_rules::arts::fog_takes(zone.spell_level, health.level, boss)
            {
                continue;
            }
            let effect = Effect::new(opensacred_rules::arts::DOMINATED, 0.0);
            commands
                .entity(creature)
                .entry::<Effects>()
                .or_default()
                .and_modify(move |mut effects| effects.add(effect, 1.0));
        }
    }
}

/// The death's kiss's skull 905 (`FX_FLYINGSKULL`, `0x7A6370`): its caster, the creature it homes
/// on, its direction, its curse's value and its bleeding's, its time left, and its level.
#[derive(Component, Debug, Clone)]
struct Skull {
    caster: Entity,
    target: Option<Entity>,
    direction: Vec3,
    curse: f32,
    bleeds: f32,
    left: f32,
    level: Level,
}

/// How high over a creature's place the skull aims, for its neck (**estimate**).
const NECK_HEIGHT: f32 = 90.0;

/// Sends the death's kiss's skull of `caster` at `target` from `from`, her neck, with its curse's
/// value and its bleeding's, for `life` seconds on `level` (`0x51042C`, the FX event `0x75`): it
/// leaves her going straight up.
pub fn kiss(
    commands: &mut Commands,
    (caster, target): (Entity, Entity),
    from: Vec3,
    (curse, bleeds): (f32, f32),
    life: f32,
    level: Level,
) {
    commands.spawn((
        Skull {
            caster,
            target: (target != caster).then_some(target),
            direction: Vec3::Y,
            curse,
            bleeds,
            left: life,
            level,
        },
        Transform::from_translation(from),
        Visibility::Inherited,
        DespawnOnExit(AppState::InGame),
    ));
}

/// Flies each skull at 150 a second, turning toward its aim by at most 3 radians a second: its
/// target's neck, else the nearest hostile creature's within about 316, else hovering about its
/// caster's (`0x5A6680` with the flag `0x10000`); each hostile creature it touches (20 and its
/// size) that holds no effect keyed by 1033 takes the effects 2 and 1 of its curse and 23 of its
/// bleeding for 5 s keyed by 1033, and the skull turns to the nearest other one so uncursed within
/// 1,000, ending without one (`0x5ABD80` at `0x5AF091`); past its life or at a cell that may not
/// be walked on it ends.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their               filters"
)]
fn skulls(
    mut commands: Commands,
    time: Res<Time>,
    looks: Option<Res<Looks>>,
    mut dice: ResMut<Dice>,
    heights: Res<crate::terrain::Heights>,
    mut skulls: Query<(Entity, &mut Skull, &mut Transform, Has<Children>)>,
    casters: Query<&Transform, (With<Body>, Without<Skull>)>,
    creatures: Query<
        (
            Entity,
            &Transform,
            &InTheWay,
            &ai::Mind,
            &Health,
            Option<&Effects>,
            Option<&arrows::Size>,
        ),
        (Without<Body>, Without<Dies>, Without<Skull>),
    >,
) {
    let dt = time.delta_secs();
    let cursed =
        |effects: Option<&Effects>| effects.is_some_and(|effects| effects.holds_any(&[1033]));
    // The creatures cursed in this frame, whose effects the commands have not given yet.
    let mut given: Vec<Entity> = Vec::new();
    for (entity, mut skull, mut transform, shown) in &mut skulls {
        if !shown && let Some(looks) = &looks {
            commands.entity(entity).with_child((
                Mesh3d(looks.soul.0.clone()),
                MeshMaterial3d(looks.discs[5].clone()),
            ));
        }
        skull.left -= dt;
        let here = transform.translation;
        let level = skull.level;
        let foe = |target: Entity| {
            creatures
                .get(target)
                .ok()
                .filter(|(_, _, way, mind, health, ..)| {
                    way.level == level && mind.hostile() && health.current > 0
                })
        };
        let nearest = |within: f32, uncursed: bool, given: &[Entity]| {
            creatures
                .iter()
                .filter(|(target, at, way, mind, health, effects, _)| {
                    way.level == level
                        && mind.hostile()
                        && health.current > 0
                        && (!uncursed || (!cursed(*effects) && !given.contains(target)))
                        && (at.translation - here).length() < within
                })
                .min_by(|a, b| {
                    let d = |at: &Transform| (at.translation - here).length();
                    d(a.1).total_cmp(&d(b.1))
                })
                .map(|(target, ..)| target)
        };
        if skull.target.and_then(foe).is_none() {
            skull.target = nearest(opensacred_rules::arts::SKULL_LOOKS, false, &given);
        }
        // Its aim: its target's neck, else about its caster's, ±50 along each axis.
        let aim = match skull.target.and_then(foe) {
            Some((_, at, ..)) => at.translation + Vec3::Y * NECK_HEIGHT,
            None => {
                let Ok(caster) = casters.get(skull.caster) else {
                    commands.entity(entity).despawn();
                    continue;
                };
                let mut roll = || f32::from(u16::try_from(dice.0.below(101)).unwrap_or(50)) - 50.0;
                caster.translation + Vec3::new(roll(), NECK_HEIGHT, roll())
            }
        };
        let want = (aim - here).normalize_or_zero();
        let angle = skull.direction.angle_between(want);
        if want != Vec3::ZERO && angle > 0.001 {
            let axis = skull
                .direction
                .cross(want)
                .try_normalize()
                .unwrap_or(Vec3::Y);
            let turn = angle.min(opensacred_rules::arts::SKULL_TURNS * dt);
            skull.direction =
                (Quat::from_axis_angle(axis, turn) * skull.direction).normalize_or(Vec3::Y);
        }
        let next = here + skull.direction * opensacred_rules::arts::SKULL_SPEED * dt;
        transform.translation = next;
        let cell = crate::paths::cell_of(next.x, next.z);
        if skull.left <= 0.0 || !heights.walkable_on(cell, level) {
            commands.entity(entity).despawn();
            continue;
        }
        let touched: Vec<Entity> = creatures
            .iter()
            .filter(|(target, at, way, mind, health, effects, size)| {
                let reach = opensacred_rules::arts::SKULL_REACH
                    + size.map_or(arrows::size(0), |size| size.0);
                way.level == level
                    && mind.hostile()
                    && health.current > 0
                    && !cursed(*effects)
                    && !given.contains(target)
                    && (at.translation - next).xz().length() < reach
            })
            .map(|(target, ..)| target)
            .collect();
        if touched.is_empty() {
            continue;
        }
        let effect = |id, value| Effect {
            art: 1033,
            level: 1,
            ..Effect::new(id, value)
        };
        let curse = [
            (
                effect(opensacred_rules::fight::effects::DEFENCE, skull.curse),
                opensacred_rules::arts::CURSE_SECONDS,
            ),
            (
                effect(opensacred_rules::fight::effects::ATTACK, skull.curse),
                opensacred_rules::arts::CURSE_SECONDS,
            ),
            (
                effect(opensacred_rules::arts::BLEEDS, skull.bleeds),
                opensacred_rules::arts::CURSE_SECONDS,
            ),
        ];
        for target in touched {
            debug!("spells: the skull {entity} curses {target}");
            give(&mut commands, target, curse.to_vec());
            given.push(target);
        }
        // On to the nearest uncursed hostile creature within 1,000, else it ends.
        skull.target = nearest(opensacred_rules::arts::SKULL_NEXT, true, &given);
        if skull.target.is_none() {
            commands.entity(entity).despawn();
        }
    }
}

/// A zone of the ring of frost (the kind 3, `0xAD2530`): its cell, its level, the ring's level
/// in play, and its time left.
#[derive(Component, Debug, Clone)]
struct Frosting {
    cell: (i32, i32),
    level: Level,
    spell: u32,
    left: f32,
}

/// A creature that a banishing circle about `from` sends away (`0x539F60`).
#[derive(Component, Clone, Copy, Debug)]
struct Banished {
    from: Vec3,
}

/// Whether world point `at` stands in `zone`: within 5 cells of its cell along each axis
/// (`0x59F960`).
fn inside(zone: &Banishing, at: Vec3) -> bool {
    let (x, y) = crate::paths::cell_of(at.x, at.z);
    (x - zone.cell.0).abs() <= spells::ZONE_REACH && (y - zone.cell.1).abs() <= spells::ZONE_REACH
}

/// A meteor of the Mage's hail: its caster, where it lands, its damages, its level, and how long
/// until it lands.
#[derive(Component, Debug, Clone)]
struct Meteor {
    caster: Entity,
    at: Vec3,
    damages: [f32; 4],
    level: Level,
    lands: f32,
}

/// The Mage's chain lightning: its caster, the creature it struck last, the strikes left, its
/// damages, its level, and how long until it leaps on.
#[derive(Component, Debug, Clone)]
struct Chain {
    caster: Entity,
    last: Entity,
    left: u32,
    damages: [f32; 4],
    level: Level,
    leaps: f32,
}

/// A flame of the Mage's fire spiral: its caster, its centre, its index, its angle and growth,
/// its damages, its level, its time left and its cooldown.
#[derive(Component, Debug, Clone)]
struct Flame {
    caster: Entity,
    centre: Vec3,
    index: u8,
    angle: f32,
    grows: f32,
    damages: [f32; 4],
    level: Level,
    left: f32,
    cooldown: f32,
}

/// The hero in its water form, which no creature takes for its target (`0x54A6D0`; docs/re/
/// arts.md, "The Mage's other spells"): the seconds until it starts, 1.6 over the cast's rate
/// after the act (its system's action 6), and how long it lasts then.
#[derive(Component, Debug, Clone, Copy)]
pub struct Unseen {
    pub after: f32,
    pub left: f32,
}

impl Unseen {
    /// Whether it holds now.
    pub fn holds(self) -> bool {
        self.after <= 0.0
    }
}

/// The hero teleporting: where to, and how long until it is put there.
#[derive(Component, Debug, Clone, Copy)]
struct Teleporting {
    to: Vec3,
    left: f32,
}

/// The dice of the spells' choices at random.
#[derive(Resource)]
struct Dice(crate::spawning::Rng);

/// A sight that goes after its time.
#[derive(Component, Debug, Clone, Copy)]
struct Fades(f32);

/// The Dwarf's sign of his mines, going with him: its caster, the mines it still carries, its
/// cooldown, the mines' damages and its level.
#[derive(Component, Debug, Clone)]
struct MineSign {
    caster: Entity,
    left: u32,
    cooldown: f32,
    damages: [f32; 4],
    level: Level,
}

/// A mine lying where the sign laid it: its caster, its damages, its level and its time left.
#[derive(Component, Debug, Clone)]
struct Mine {
    caster: Entity,
    damages: [f32; 4],
    level: Level,
    left: f32,
}

/// The stand-ins' meshes and materials: a disc of radius 1 and the holy light's, the blades of
/// light's, the light's, the fire skin's, the gust's, the fear's and a tentacle's; a column of
/// light and the lightning's;
/// a mine and its blast; the hell disk, a flame and a lost soul.
#[derive(Resource)]
struct Looks {
    disc: Handle<Mesh>,
    discs: [Handle<StandardMaterial>; 7],
    column: Handle<Mesh>,
    lightning: Handle<StandardMaterial>,
    mine: (Handle<Mesh>, Handle<StandardMaterial>),
    blast: (Handle<Mesh>, Handle<StandardMaterial>),
    disk: (Handle<Mesh>, Handle<StandardMaterial>),
    flame: (Handle<Mesh>, Handle<StandardMaterial>),
    soul: (Handle<Mesh>, Handle<StandardMaterial>),
}

/// How high over the ground the hell disk flies (**estimate**: a hand's height, as the bolts').
const DISK_HEIGHT: f32 = 80.0;

/// Makes the stand-ins' looks (**estimate**: the colours of their glows).
fn make_looks(
    mut commands: Commands,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        return;
    };
    let mut glow = |colour: Color| {
        materials.add(StandardMaterial {
            base_color: colour,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })
    };
    let discs = [
        glow(Color::srgba(1.0, 0.85, 0.4, 0.35)),
        glow(Color::srgba(0.75, 0.85, 1.0, 0.3)),
        glow(Color::srgba(1.0, 1.0, 0.75, 0.2)),
        glow(Color::srgba(1.0, 0.45, 0.1, 0.3)),
        glow(Color::srgba(0.85, 0.9, 0.85, 0.25)),
        glow(Color::srgba(0.45, 0.1, 0.5, 0.25)),
        glow(Color::srgba(0.3, 0.5, 0.2, 0.35)),
    ];
    let lightning = glow(Color::srgba(0.8, 0.9, 1.0, 0.8));
    let mine = (
        meshes.add(Cylinder::new(14.0, 6.0)),
        glow(Color::srgba(0.25, 0.22, 0.2, 1.0)),
    );
    let blast = (
        meshes.add(Sphere::new(spells::BLAST_REACH)),
        glow(Color::srgba(1.0, 0.6, 0.2, 0.45)),
    );
    let disk = (
        meshes.add(Cylinder::new(18.0, 4.0)),
        glow(Color::srgba(1.0, 0.3, 0.1, 0.9)),
    );
    let flame = (
        meshes.add(Sphere::new(20.0)),
        glow(Color::srgba(1.0, 0.5, 0.1, 0.85)),
    );
    let soul = (
        meshes.add(Sphere::new(10.0)),
        glow(Color::srgba(0.85, 0.95, 1.0, 0.7)),
    );
    commands.insert_resource(Looks {
        disk,
        flame,
        soul,
        disc: meshes.add(Cylinder::new(1.0, 1.0)),
        discs,
        column: meshes.add(Cylinder::new(8.0, 400.0)),
        lightning,
        mine,
        blast,
    });
}

/// Carries out each spell that acts.
#[expect(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its resources grouped; \
              the spells read best together"
)]
fn act(
    mut commands: Commands,
    mut acts: MessageReader<Acts>,
    (arts, rules, looks, experience): (
        Option<Res<crate::arts::HeroArts>>,
        Res<crate::fight::Rules>,
        Option<Res<Looks>>,
        Res<fight::Experience>,
    ),
    casters: Query<(&Transform, &Body, Option<&Combat>)>,
    places: Query<&GlobalTransform>,
    mut targets: Query<(&mut Health, Option<&ai::Mind>, Option<&fight::Ranked>)>,
    mut launches: MessageWriter<crate::bolts::Launch>,
    (mut hits, mut dice, heights): (
        MessageWriter<fight::Hit>,
        ResMut<Dice>,
        Res<crate::terrain::Heights>,
    ),
    hearers: Query<
        (Entity, &GlobalTransform, &ai::Mind, Option<&Effects>),
        (Without<Body>, Without<Dies>),
    >,
) {
    let Some(arts) = arts else {
        acts.clear();
        return;
    };
    for act in acts.read() {
        let Some(record) = arts.tables.spells.iter().find(|s| s.id == act.spell) else {
            warn!("spells: no record {} of the first table", act.spell);
            continue;
        };
        let Ok((at, body, combat)) = casters.get(act.caster) else {
            continue;
        };
        let here = at.translation;
        let multipliers = combat.map_or([1.0; 4], |combat| combat.spells);
        let level = i32::try_from(act.level).unwrap_or(1);
        let mut given = Vec::new();
        if spells::lasts(act.spell) && act.spell != spells::SOULS {
            given.push(spells::lasting(record, act.level));
        }
        match act.spell {
            FIREBALL => {
                // For its target's place, or the point where it is gone (`0x522480`).
                let toward = places
                    .get(act.target)
                    .ok()
                    .filter(|_| act.target != act.caster)
                    .map_or(act.point, GlobalTransform::translation);
                let direction = (toward - here)
                    .xz()
                    .try_normalize()
                    .unwrap_or_else(|| (at.rotation * Vec3::X).xz().normalize_or(Vec2::X));
                let damages = bolt_damages(record, level, multipliers);
                info!("spells: the fireball leaves for {toward} with {damages:?}");
                launches.write(crate::bolts::Launch {
                    shooter: act.caster,
                    direction,
                    course: crate::bolts::Course::fireball(act.level),
                    damages,
                    wait: 0.0,
                });
            }
            REIKI | SPRING => given.extend(spells::effects(record, act.level)),
            // The Daemon's: the call of death's, the force of hell's and the tentacles' effects
            // on him; the fear's five objects about him.
            spells::DOOM | spells::FORCE | spells::TENTACLES => {
                given.extend(spells::daemons(record, act.level));
            }
            spells::FEAR => {
                let fear = spells::fear(record, act.level);
                for k in 0..5u8 {
                    let angle = spells::FEAR_ANGLE + spells::FEAR_STEP * f32::from(k);
                    let at = here + Vec3::new(angle.cos(), 0.0, angle.sin()) * spells::FEAR_FROM;
                    let mut made = commands.spawn((
                        Aura {
                            caster: act.caster,
                            follows: None,
                            reach: spells::FEAR_REACH,
                            classes: &[],
                            damages: None,
                            gives: fear.clone(),
                            key: act.spell,
                            level: body.level(),
                            left: spells::FEAR_LIFE,
                            cooldown: 0.0,
                            strikes: Strikes::About,
                        },
                        Transform::from_translation(at),
                        Visibility::Inherited,
                        DespawnOnExit(AppState::InGame),
                    ));
                    if let Some(looks) = &looks {
                        made.with_child((
                            Mesh3d(looks.disc.clone()),
                            MeshMaterial3d(looks.discs[5].clone()),
                            Transform::from_scale(Vec3::new(
                                spells::FEAR_REACH,
                                2.0,
                                spells::FEAR_REACH,
                            ))
                            .with_translation(Vec3::Y * 4.0),
                        ));
                    }
                }
            }
            // The Wood Elf's summoning at the order's point, which makes her creature of
            // `0x426F60`, her unicorn, of the level `v₀` for her follower (`0x523413`).
            spells::SUMMON => crate::followers::summon(
                &mut commands,
                act.caster,
                spells::summoned(arts.kind),
                spells::value(record, 0, act.level),
                act.point,
            ),
            // The Wood Elf's: the speed's effects from each of its two objects, the second's 28
            // refused (`0x526625`).
            spells::SPEED => {
                let once = spells::effects(record, act.level);
                given.extend(once.iter().copied());
                given.extend(once.into_iter().filter(|(e, _)| e.id != 28));
            }
            spells::PLANT_HEAL => {
                let lore = opensacred_rules::fight::magic_lore(&arts.skills, &rules);
                let health = spells::healing(record, act.level, lore);
                if let Ok((mut bearer, ..)) = targets.get_mut(act.target) {
                    bearer.current = bearer.current.saturating_add(health).min(bearer.most);
                    info!("spells: the plants heal {health}, to {}", bearer.current);
                }
            }
            spells::BUBBLE => {
                let toward = places
                    .get(act.target)
                    .ok()
                    .filter(|_| act.target != act.caster)
                    .map_or(act.point, GlobalTransform::translation);
                let direction = (toward - here)
                    .xz()
                    .try_normalize()
                    .unwrap_or_else(|| (at.rotation * Vec3::X).xz().normalize_or(Vec2::X));
                let mut course = crate::bolts::Course::passing(spells::BUBBLE_COURSE, 6);
                course.beat = spells::BUBBLE_BEAT;
                course.bursts = Some(spells::BURST);
                launches.write(crate::bolts::Launch {
                    shooter: act.caster,
                    direction,
                    course,
                    damages: bolt_damages(record, level, multipliers),
                    wait: 0.0,
                });
            }
            spells::THORNS => {
                // About the target's place, else the point, each where it may stand.
                let centre = places
                    .get(act.target)
                    .ok()
                    .filter(|_| act.target != act.caster)
                    .map_or(act.point, GlobalTransform::translation);
                let damages = bolt_damages(record, level, multipliers);
                let seconds = fight_seconds(spells::value(record, 1, act.level));
                for _ in 0..spells::count(record, act.level) {
                    let mut off = || {
                        let span = 2 * spells::THORNS_SCATTER + 1;
                        f32::from(u16::try_from(dice.0.below(span)).unwrap_or(0))
                            - f32::from(u16::try_from(spells::THORNS_SCATTER).unwrap_or(0))
                    };
                    let mut place = centre + Vec3::new(off(), 0.0, off());
                    if !heights.walkable_on(crate::paths::cell_of(place.x, place.z), body.level()) {
                        place = centre;
                    }
                    let mut made = commands.spawn((
                        Aura {
                            caster: act.caster,
                            follows: None,
                            reach: spells::THORNS_REACH,
                            classes: &[],
                            damages: Some(damages),
                            gives: Vec::new(),
                            key: act.spell,
                            level: body.level(),
                            left: seconds,
                            cooldown: 0.0,
                            strikes: Strikes::About,
                        },
                        Transform::from_translation(place),
                        Visibility::Inherited,
                        DespawnOnExit(AppState::InGame),
                    ));
                    if let Some(looks) = &looks {
                        made.with_child((
                            Mesh3d(looks.disc.clone()),
                            MeshMaterial3d(looks.discs[6].clone()),
                            Transform::from_scale(Vec3::new(
                                spells::THORNS_REACH,
                                2.0,
                                spells::THORNS_REACH,
                            ))
                            .with_translation(Vec3::Y * 4.0),
                        ));
                    }
                }
            }
            spells::SOULS => {
                // Its souls about her, homing, each striking the first it touches once a
                // second; her effect `0x22` for their value 1 (`0x5268E5`).
                let seconds = fight_seconds(spells::value(record, 1, act.level));
                let effect = Effect {
                    id: spells::LASTS,
                    whole: 0,
                    value: 0.0,
                    art: u32::from(act.spell),
                    level: 0,
                };
                given.push((effect, seconds));
                let damages = bolt_damages(record, level, multipliers);
                let n = spells::count(record, act.level);
                for i in 0..n {
                    let angle = std::f32::consts::TAU * fight_seconds(i) / fight_seconds(n)
                        + spells::SOULS_ANGLE;
                    let place =
                        here + Vec3::new(angle.cos(), 0.0, angle.sin()) * spells::SOULS_FROM;
                    let mut made = commands.spawn((
                        Disk {
                            caster: act.caster,
                            target: None,
                            last: None,
                            damages,
                            level: body.level(),
                            left: seconds,
                            cooldown: 0.0,
                            speed: spells::SOULS_SPEED,
                            reach: spells::SOULS_REACH,
                            all: false,
                        },
                        Transform::from_translation(place + Vec3::Y * DISK_HEIGHT),
                        Visibility::Inherited,
                        DespawnOnExit(AppState::InGame),
                    ));
                    if let Some(looks) = &looks {
                        made.with_child((
                            Mesh3d(looks.soul.0.clone()),
                            MeshMaterial3d(looks.soul.1.clone()),
                        ));
                    }
                }
            }
            spells::TELEPORT => {
                // Along the line to the point, the last point where he may stand, some 3.35 s
                // over the cast's rate later (`0x522742`).
                let level = body.level();
                let passes = |[x, z]: [f32; 2]| {
                    let cell = crate::paths::cell_of(x, z);
                    heights.walkable_on(cell, level)
                };
                let stands = |[x, z]: [f32; 2]| heights.standing(x, z, level).is_some();
                let [x, z] = spells::teleport(
                    [here.x, here.z],
                    [act.point.x, act.point.z],
                    spells::value(record, 0, act.level),
                    passes,
                    stands,
                );
                let rate = combat.map_or(opensacred_rules::fight::CAST_SPEED, |c| c.cast) / 100.0;
                commands.entity(act.caster).insert(Teleporting {
                    to: Vec3::new(x, here.y, z),
                    left: spells::TELEPORT_TIME / rate.max(f32::EPSILON),
                });
            }
            spells::WATER_FORM => {
                let seconds = fight_seconds(spells::value(record, 0, act.level));
                let rate = combat.map_or(opensacred_rules::fight::CAST_SPEED, |c| c.cast) / 100.0;
                commands.entity(act.caster).insert(Unseen {
                    after: SHIELD_TIME / rate.max(f32::EPSILON),
                    left: seconds + spells::WATER_AFTER,
                });
            }
            spells::STONE | spells::CAGE => {
                // A creature of no big type held for its value 0 (the effect 15, action 7): the
                // Mage's stone, or the Wood Elf's cages about it.
                let Ok((health, Some(mind), _)) = targets.get(act.target) else {
                    continue;
                };
                if mind.big || health.current == 0 {
                    continue;
                }
                let seconds = fight_seconds(spells::value(record, 0, act.level));
                let effect = Effect {
                    id: spells::HELD,
                    whole: 0,
                    value: 0.0,
                    art: u32::from(act.spell),
                    level: 0,
                };
                given.push((effect, seconds));
                let mut held = commands.entity(act.target);
                held.remove::<(crate::npcs::Walk, crate::npcs::Strikes)>();
                held.entry::<ai::Mind>()
                    .and_modify(move |mut mind| mind.stunned(seconds));
            }
            spells::CHANGELING => {
                // The first 20 hostile creatures within about 179 of her that it may change, the
                // nearest first, each into an animal at random (`0x525C8C`, `0x795E20`).
                let mut changed: Vec<(Entity, f32, bool)> = hearers
                    .iter()
                    .filter(|(_, _, mind, _)| mind.hostile())
                    .filter_map(|(entity, at, mind, _)| {
                        let d = (at.translation() - here).xz().length();
                        let boss = targets
                            .get(entity)
                            .ok()
                            .and_then(|(.., rank)| rank)
                            .is_some_and(|rank| rank.0 == fight::Rank::Boss);
                        (d < spells::CHANGES_WITHIN
                            && spells::changes(
                                mind.class,
                                mind.big,
                                mind.level,
                                boss,
                                experience.level,
                                act.level,
                            ))
                        .then_some((entity, d, boss))
                    })
                    .collect();
                changed.sort_by(|a, b| a.1.total_cmp(&b.1));
                changed.truncate(spells::CHANGES_MOST);
                for (entity, _, boss) in changed {
                    let roll = dice
                        .0
                        .below(u32::try_from(spells::ANIMALS.len()).unwrap_or(1));
                    let animal = spells::ANIMALS[usize::try_from(roll).unwrap_or(0)];
                    let seconds = spells::changed_seconds(record, act.level, boss);
                    commands.queue(move |world: &mut World| change(world, entity, animal, seconds));
                }
            }
            spells::BANISHING => {
                // Eight runes at 200 about him, and a zone of 5 cells along each axis about his
                // cell for `v₀` seconds, which sends the creatures in it away (`0x522DE3`).
                let seconds = fight_seconds(spells::value(record, 0, act.level));
                let cell = crate::paths::cell_of(here.x, here.z);
                let zone = Banishing {
                    centre: here,
                    cell,
                    level: body.level(),
                    left: seconds,
                };
                // At its cast, the creatures in it of no boss and of a level up to 5·L + 3.
                for (entity, at, mind, _) in &hearers {
                    let boss = targets
                        .get(entity)
                        .ok()
                        .and_then(|(.., rank)| rank)
                        .is_some_and(|rank| rank.0 == fight::Rank::Boss);
                    if inside(&zone, at.translation())
                        && mind.hostile()
                        && spells::banishes(act.level, mind.level, boss)
                    {
                        commands.entity(entity).insert(Banished { from: here });
                    }
                }
                let mut made = commands.spawn((
                    zone,
                    Transform::from_translation(here),
                    Visibility::Inherited,
                    DespawnOnExit(AppState::InGame),
                ));
                if let Some(looks) = &looks {
                    made.with_children(|runes| {
                        for k in 0..8u8 {
                            let angle = std::f32::consts::FRAC_PI_4 * f32::from(k);
                            let off = Vec3::new(angle.cos(), 0.0, angle.sin()) * spells::RUNES_FROM;
                            runes.spawn((
                                Mesh3d(looks.disc.clone()),
                                MeshMaterial3d(looks.discs[2].clone()),
                                Transform::from_translation(off + Vec3::Y * 4.0)
                                    .with_scale(Vec3::new(20.0, 2.0, 20.0)),
                            ));
                        }
                    });
                }
            }
            spells::FROST_RING => {
                // And its zone, 5 cells along each axis about his cell for 25 s (`0x52555A`).
                commands.spawn((
                    Frosting {
                        cell: crate::paths::cell_of(here.x, here.z),
                        level: body.level(),
                        spell: act.level,
                        left: spells::FROST_LIFE,
                    },
                    DespawnOnExit(AppState::InGame),
                ));
                let damages = bolt_damages(record, level, multipliers);
                for k in 0..8u8 {
                    let angle = spells::FROST_ANGLE + spells::FROST_STEP * f32::from(k);
                    let at = here + Vec3::new(angle.cos(), 0.0, angle.sin()) * spells::FROST_FROM;
                    let mut made = commands.spawn((
                        Aura {
                            caster: act.caster,
                            follows: None,
                            reach: spells::FROST_REACH,
                            classes: &[],
                            damages: Some(damages),
                            gives: Vec::new(),
                            key: act.spell,
                            level: body.level(),
                            left: spells::FROST_LIFE,
                            cooldown: 0.0,
                            strikes: Strikes::About,
                        },
                        Transform::from_translation(at),
                        Visibility::Inherited,
                        DespawnOnExit(AppState::InGame),
                    ));
                    if let Some(looks) = &looks {
                        made.with_child((
                            Mesh3d(looks.disc.clone()),
                            MeshMaterial3d(looks.discs[1].clone()),
                            Transform::from_scale(Vec3::new(
                                spells::FROST_REACH,
                                2.0,
                                spells::FROST_REACH,
                            ))
                            .with_translation(Vec3::Y * 4.0),
                        ));
                    }
                }
            }
            spells::WHIRLWIND | spells::SPLINTERS => {
                // A fan of bolts along the line to the target's place, or the point, passing
                // through what they strike once a second (`0x524EA2`, `0x5258B5`).
                let toward = places
                    .get(act.target)
                    .ok()
                    .filter(|_| act.target != act.caster)
                    .map_or(act.point, GlobalTransform::translation);
                let line = (toward - here).xz();
                let base = line
                    .try_normalize()
                    .unwrap_or_else(|| (at.rotation * Vec3::X).xz().normalize_or(Vec2::X));
                let n = spells::count(record, act.level);
                let (course, look) = if act.spell == spells::WHIRLWIND {
                    (spells::WHIRLWIND_COURSE, 4)
                } else {
                    (spells::SPLINTERS_COURSE, 5)
                };
                let damages = bolt_damages(record, level, multipliers);
                let mut course = crate::bolts::Course::passing(course, look);
                if act.spell == spells::WHIRLWIND {
                    course.knocks = Some(spells::whirlwind_knocks(
                        record,
                        act.level,
                        experience.level,
                    ));
                }
                for angle in spells::fan(act.spell, n, line.length()) {
                    launches.write(crate::bolts::Launch {
                        shooter: act.caster,
                        direction: Vec2::from_angle(angle).rotate(base),
                        course,
                        damages,
                        wait: 0.0,
                    });
                }
            }
            spells::METEORS => {
                // Each about the point, falling after a wait at random (`0x524A83`).
                let damages = bolt_damages(record, level, multipliers);
                for _ in 0..spells::count(record, act.level) {
                    let mut off = || {
                        let span = u32::try_from(2 * spells::METEOR_SCATTER + 1).unwrap_or(1);
                        let roll = i32::try_from(dice.0.below(span)).unwrap_or(0);
                        f32::from(i16::try_from(spells::METEOR_SCATTER - roll).unwrap_or(0))
                    };
                    let (dx, dz) = (off(), off());
                    let wait = fight_seconds(dice.0.below(spells::METEOR_WAIT + 1)) / 1000.0;
                    commands.spawn((
                        Meteor {
                            caster: act.caster,
                            at: act.point + Vec3::new(dx, 0.0, dz),
                            damages,
                            level: body.level(),
                            lands: wait + spells::METEOR_FALLS,
                        },
                        Transform::from_translation(act.point + Vec3::new(dx, 500.0, dz)),
                        Visibility::Hidden,
                        DespawnOnExit(AppState::InGame),
                    ));
                }
            }
            spells::CHAIN => {
                // Its first strike on its target at once, then its leaps (`0x526CAA`).
                if act.target == act.caster || places.get(act.target).is_err() {
                    continue;
                }
                let damages = bolt_damages(record, level, multipliers);
                hits.write(fight::Hit {
                    attacker: act.caster,
                    target: act.target,
                    share: 1.0,
                    ranged: false,
                    art: None,
                    fixed: Some(damages),
                    sure: true,
                });
                if let (Some(looks), Ok(place)) = (&looks, places.get(act.target)) {
                    commands.spawn((
                        Mesh3d(looks.column.clone()),
                        MeshMaterial3d(looks.lightning.clone()),
                        Transform::from_translation(place.translation() + Vec3::Y * 200.0),
                        Fades(LIGHTNING_SHOWS),
                        DespawnOnExit(AppState::InGame),
                    ));
                }
                commands.spawn((
                    Chain {
                        caster: act.caster,
                        last: act.target,
                        left: spells::value(record, 0, act.level).saturating_sub(1),
                        damages,
                        level: body.level(),
                        leaps: spells::CHAIN_LEAPS,
                    },
                    DespawnOnExit(AppState::InGame),
                ));
            }
            spells::SPIRAL => {
                let damages = bolt_damages(record, level, multipliers);
                for index in 0..spells::SPIRAL_FLAMES {
                    let mut made = commands.spawn((
                        Flame {
                            caster: act.caster,
                            centre: here,
                            index,
                            angle: 0.0,
                            grows: 0.0,
                            damages,
                            level: body.level(),
                            left: fight_seconds(spells::value(record, 0, act.level)),
                            cooldown: 0.0,
                        },
                        Transform::from_translation(here + Vec3::Y * DISK_HEIGHT),
                        Visibility::Inherited,
                        DespawnOnExit(AppState::InGame),
                    ));
                    if let Some(looks) = &looks {
                        made.with_child((
                            Mesh3d(looks.flame.0.clone()),
                            MeshMaterial3d(looks.flame.1.clone()),
                        ));
                    }
                }
            }
            spells::PURGATORY => {
                let mut made = commands.spawn((
                    Disk {
                        caster: act.caster,
                        target: None,
                        last: None,
                        damages: bolt_damages(record, level, multipliers),
                        level: body.level(),
                        left: fight_seconds(spells::value(record, 0, act.level)),
                        cooldown: 0.0,
                        speed: spells::PURGATORY_SPEED,
                        reach: spells::DISK_REACH,
                        all: true,
                    },
                    Transform::from_translation(act.point + Vec3::Y * DISK_HEIGHT),
                    Visibility::Inherited,
                    DespawnOnExit(AppState::InGame),
                ));
                if let Some(looks) = &looks {
                    made.with_child((
                        Mesh3d(looks.flame.0.clone()),
                        MeshMaterial3d(looks.flame.1.clone()),
                    ));
                }
            }
            spells::DISK => {
                let damages = bolt_damages(record, level, multipliers);
                let mut made = commands.spawn((
                    Disk {
                        caster: act.caster,
                        target: None,
                        last: None,
                        damages,
                        level: body.level(),
                        left: fight_seconds(spells::value(record, 0, act.level)),
                        cooldown: 0.0,
                        speed: spells::DISK_SPEED,
                        reach: spells::DISK_REACH,
                        all: false,
                    },
                    Transform::from_translation(here + Vec3::Y * DISK_HEIGHT),
                    Visibility::Inherited,
                    DespawnOnExit(AppState::InGame),
                ));
                if let Some(looks) = &looks {
                    made.with_child((
                        Mesh3d(looks.disk.0.clone()),
                        MeshMaterial3d(looks.disk.1.clone()),
                    ));
                }
            }
            spells::HELLFIRE => {
                let mut made = commands.spawn((
                    Aura {
                        caster: act.caster,
                        follows: None,
                        reach: spells::HELLFIRE_NEAR,
                        classes: &[],
                        damages: Some(bolt_damages(record, level, multipliers)),
                        gives: Vec::new(),
                        key: act.spell,
                        level: body.level(),
                        left: fight_seconds(spells::value(record, 0, act.level)),
                        cooldown: 0.0,
                        strikes: Strikes::OneAtRandom,
                    },
                    Transform::from_translation(act.point),
                    Visibility::Inherited,
                    DespawnOnExit(AppState::InGame),
                ));
                if let Some(looks) = &looks {
                    made.with_child((
                        Mesh3d(looks.disc.clone()),
                        MeshMaterial3d(looks.discs[3].clone()),
                        Transform::from_scale(Vec3::new(60.0, 2.0, 60.0))
                            .with_translation(Vec3::Y * 4.0),
                    ));
                }
            }
            spells::CHOIR => {
                // The first 20 creatures hostile to the hero, alive, within its reach, of no
                // big type and without its effect: each with a choir following it, striking it
                // once a second, and the effect 35 keyed by the spell.
                let seconds = fight_seconds(spells::value(record, 0, act.level));
                let damages = bolt_damages(record, level, multipliers);
                let taken: Vec<Entity> = hearers
                    .iter()
                    .filter(|(hearer, at, mind, effects)| {
                        mind.hostile()
                            && targets
                                .get(*hearer)
                                .is_ok_and(|(health, ..)| health.current > 0)
                            && !mind.big
                            && !effects.is_some_and(|e| e.holds_any(&[act.spell]))
                            && (at.translation() - here).xz().length() < spells::CHOIR_REACH
                    })
                    .map(|(entity, ..)| entity)
                    .take(spells::CHOIR_MOST)
                    .collect();
                for hearer in taken {
                    let effect = Effect {
                        id: BORNE,
                        whole: 0,
                        value: 0.0,
                        art: u32::from(act.spell),
                        level: 0,
                    };
                    give(&mut commands, hearer, vec![(effect, seconds)]);
                    commands.spawn((
                        Aura {
                            caster: act.caster,
                            follows: Some(hearer),
                            reach: 0.0,
                            classes: &[],
                            damages: Some(damages),
                            gives: Vec::new(),
                            key: act.spell,
                            level: body.level(),
                            left: seconds,
                            cooldown: 0.0,
                            strikes: Strikes::Bearer,
                        },
                        Transform::from_translation(here),
                        DespawnOnExit(AppState::InGame),
                    ));
                }
            }
            spells::MINES => {
                // His sign, with the mines' count of its values' ratio; their damages by his
                // weapons' multipliers, the second array (`0x528995`).
                let damages = bolt_damages(
                    record,
                    level,
                    combat.map_or([1.0; 4], |combat| combat.multipliers),
                );
                commands.spawn((
                    MineSign {
                        caster: act.caster,
                        left: spells::ratio(record, 0, 1, act.level),
                        cooldown: 0.0,
                        damages,
                        level: body.level(),
                    },
                    Transform::from_translation(here),
                    DespawnOnExit(AppState::InGame),
                ));
            }
            // The Dwarf's: the greed's and the steel's effects at once; the burrowing's and the
            // armour's with their action 6, the armour's object's effect 35 too.
            spells::GREED | spells::STEEL => given.extend(spells::dwarfs(record, act.level)),
            spells::BURROWING | spells::ARMOUR => {
                given.extend(spells::dwarfs(record, act.level));
                given.extend(spells::effects(record, act.level));
                if act.spell == spells::ARMOUR {
                    let effect = Effect {
                        id: BORNE,
                        whole: 0,
                        value: 0.0,
                        art: u32::from(act.spell),
                        level: 0,
                    };
                    given.push((effect, fight_seconds(spells::value(record, 0, act.level))));
                }
            }
            QUICKENING => {
                // As the stone skin's, its clock at the cast's rate (`0x7883C0`).
                let rate = combat.map_or(opensacred_rules::fight::CAST_SPEED, |c| c.cast) / 100.0;
                commands.entity(act.target).insert(Shielding {
                    left: SHIELD_TIME / rate.max(f32::EPSILON),
                    effects: spells::effects(record, act.level),
                });
            }
            SHIELD_WALL => {
                given.extend(spells::turning(record, act.level));
                commands.entity(act.target).insert(Shielding {
                    left: WALL_TIME,
                    effects: spells::effects(record, act.level),
                });
            }
            HOLY_SHIELD => given.push(spells::holy_shield(record, act.level)),
            IRRITATION | CONVERSION => {
                // On a hostile creature alive, no boss nor champion, nor of a big type for the
                // conversion (`0x524196`, `0x5242D3`, the action 7); the effect's own refusal of
                // `+0x2B8` bit `0x40` is not followed.
                let Ok((health, Some(mind), rank)) = targets.get(act.target) else {
                    continue;
                };
                let ranked = rank.map(|rank| rank.0);
                let held = !matches!(
                    ranked,
                    Some(
                        opensacred_rules::fight::Rank::Boss
                            | opensacred_rules::fight::Rank::Champion
                    )
                );
                if !mind.hostile()
                    || health.current == 0
                    || !held
                    || (act.spell == CONVERSION && mind.big)
                {
                    info!("spells: {} does not hold {}", act.spell, act.target);
                    continue;
                }
                given.extend(spells::holding(
                    record,
                    act.level,
                    experience.level,
                    health.level,
                ));
            }
            CRUSADERS_STRENGTH => {
                commands.entity(act.target).insert(Shielding {
                    left: STRENGTH_TIME,
                    effects: spells::effects(record, act.level),
                });
            }
            STONE_SKIN => {
                // The shield's clock runs at the cast's rate (`+0x3E`).
                let rate = combat.map_or(opensacred_rules::fight::CAST_SPEED, |c| c.cast) / 100.0;
                commands.entity(act.target).insert(Shielding {
                    left: SHIELD_TIME / rate.max(f32::EPSILON),
                    effects: spells::effects(record, act.level),
                });
            }
            HEALING => {
                let lore = opensacred_rules::fight::magic_lore(&arts.skills, &rules);
                let health = spells::healing(record, act.level, lore);
                if let Ok((mut bearer, ..)) = targets.get_mut(act.target) {
                    bearer.current = bearer.current.saturating_add(health).min(bearer.most);
                    info!("spells: the healing gives {health}, to {}", bearer.current);
                }
            }
            LIGHTNING => {
                // Its object at its target strikes it alone at its first frame, of the record's
                // damages by the caster's spells' multipliers (`0x5B5069`).
                let Ok(place) = places.get(act.target) else {
                    continue;
                };
                let damages = bolt_damages(record, level, multipliers);
                info!(
                    "spells: the lightning strikes {} for {damages:?}",
                    act.target
                );
                hits.write(fight::Hit {
                    attacker: act.caster,
                    target: act.target,
                    share: 1.0,
                    ranged: false,
                    art: None,
                    fixed: Some(damages),
                    sure: true,
                });
                if let Some(looks) = &looks {
                    commands.spawn((
                        Mesh3d(looks.column.clone()),
                        MeshMaterial3d(looks.lightning.clone()),
                        Transform::from_translation(place.translation() + Vec3::Y * 200.0),
                        Fades(LIGHTNING_SHOWS),
                        DespawnOnExit(AppState::InGame),
                    ));
                }
            }
            spell => {
                let Some(area) = spells::area(spell) else {
                    continue;
                };
                let seconds = fight_seconds(spells::value(record, 0, act.level));
                let follows = area.follows.then_some(act.target);
                let centre = match follows {
                    Some(bearer) => places.get(bearer).map_or(here, |at| at.translation()),
                    None => act.point,
                };
                let damages = bolt_damages(record, level, multipliers);
                info!(
                    "spells: {spell} strikes within {} for {seconds} s with {damages:?}",
                    area.reach
                );
                let aura = Aura {
                    caster: act.caster,
                    follows,
                    reach: area.reach,
                    classes: area.classes,
                    damages: Some(damages),
                    gives: Vec::new(),
                    key: spell,
                    level: body.level(),
                    left: seconds,
                    cooldown: 0.0,
                    strikes: Strikes::About,
                };
                let mut made = commands.spawn((
                    aura,
                    Transform::from_translation(centre),
                    Visibility::Inherited,
                    DespawnOnExit(AppState::InGame),
                ));
                if let Some(looks) = &looks {
                    let look = match spell {
                        spells::HOLY_LIGHT => 0,
                        spells::BLADES_OF_LIGHT => 1,
                        FIRE_SKIN => 3,
                        spells::GUST => 4,
                        _ => 2,
                    };
                    made.with_child((
                        Mesh3d(looks.disc.clone()),
                        MeshMaterial3d(looks.discs[look].clone()),
                        Transform::from_scale(Vec3::new(area.reach, 2.0, area.reach))
                            .with_translation(Vec3::Y * 4.0),
                    ));
                }
                // The fire skin's effects of its record at once, and the turning aside of the
                // fire skin and the gust.
                given.extend(spells::turning(record, act.level));
                if spell == FIRE_SKIN {
                    given.extend(spells::effects(record, act.level));
                }
                if spells::borne(spell) {
                    let effect = Effect {
                        id: BORNE,
                        whole: 0,
                        value: 0.0,
                        art: u32::from(spell),
                        level: 0,
                    };
                    given.push((effect, seconds));
                }
            }
        }
        give(&mut commands, act.target, given);
    }
}

/// A whole number of seconds as a float.
fn fight_seconds(seconds: u32) -> f32 {
    u16::try_from(seconds).map_or(f32::MAX, f32::from)
}

/// Runs each object that strikes about itself: it goes with its bearer, ends with its time or
/// its bearer, and once its cooldown is over strikes the creatures on its level that are
/// hostile to the hero, alive, of its classes where it names some, and nearer it than its reach
/// and their size, each with the fight's sure strike of its damages or its effects, then waits a
/// [`BEAT`] (`0x5A85A8`, `0x5AB490`, `0x5ABD80`); the choir's strikes the creature it follows
/// alone, and the hellfire one creature at random ([`Strikes`]).
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn strike(
    mut commands: Commands,
    time: Res<Time>,
    mut dice: ResMut<Dice>,
    heroes: Query<&GlobalTransform, (With<Body>, Without<Aura>)>,
    mut auras: Query<(Entity, &mut Aura, &mut Transform)>,
    bearers: Query<&GlobalTransform, (Without<Aura>, Without<Dies>)>,
    creatures: Query<
        (
            Entity,
            &GlobalTransform,
            &InTheWay,
            &ai::Mind,
            &Health,
            Option<&arrows::Size>,
            Option<&Effects>,
        ),
        (Without<Body>, Without<Aura>, Without<Dies>),
    >,
    mut hits: MessageWriter<fight::Hit>,
) {
    let dt = time.delta_secs();
    // The creatures given an effect keyed by a spell in this frame, which the adder would find
    // holding it for the next object.
    let mut given: Vec<(Entity, u16)> = Vec::new();
    for (entity, mut aura, mut transform) in &mut auras {
        aura.left -= dt;
        let bearer = aura.follows.map(|bearer| bearers.get(bearer));
        if aura.left <= 0.0 || bearer.as_ref().is_some_and(Result::is_err) {
            commands.entity(entity).despawn();
            continue;
        }
        if let Some(Ok(bearer)) = bearer {
            transform.translation = bearer.translation();
        }
        aura.cooldown = (aura.cooldown - dt).max(0.0);
        if aura.cooldown > 0.0 {
            continue;
        }
        let centre = transform.translation.xz();
        let hero = heroes.iter().next().map(|at| at.translation().xz());
        if aura.strikes == Strikes::OneAtRandom {
            // The hellfire: one creature at random among those near the hero and it, weighed
            // by a ranged weapon, else among those within its reach (`0x5AFF86`).
            let alive = |(_, at, way, mind, health, ..): &(
                Entity,
                &GlobalTransform,
                &InTheWay,
                &ai::Mind,
                &Health,
                Option<&arrows::Size>,
                Option<&Effects>,
            )| {
                way.creature
                    && way.level == aura.level
                    && mind.hostile()
                    && health.current > 0
                    && at.translation().y.is_finite()
            };
            let mut weighed: Vec<(Entity, u32)> = creatures
                .iter()
                .filter(|c| alive(c))
                .filter(|(_, at, ..)| {
                    let d = at.translation().xz() - centre;
                    hero.is_some_and(|hero| {
                        (at.translation().xz() - hero).length() <= spells::HELLFIRE_HERO
                    }) && d.x.abs() <= spells::HELLFIRE_BOX
                        && d.y.abs() <= spells::HELLFIRE_BOX
                })
                .map(|(target, _, _, mind, ..)| (target, if mind.ranged { 4 } else { 1 }))
                .collect();
            if weighed.is_empty() {
                weighed = creatures
                    .iter()
                    .filter(|c| alive(c))
                    .filter(|(_, at, .., size, _)| {
                        (at.translation().xz() - centre).length()
                            < aura.reach + size.map_or(arrows::size(0), |size| size.0)
                    })
                    .map(|(target, ..)| (target, 1))
                    .collect();
            }
            let total: u32 = weighed.iter().map(|&(_, w)| w).sum();
            if total == 0 {
                continue;
            }
            let mut roll = dice.0.below(total);
            let picked = weighed.iter().find(|&&(_, w)| {
                let here = roll < w;
                roll = roll.saturating_sub(w);
                here
            });
            if let (Some(&(target, _)), Some(damages)) = (picked, aura.damages) {
                debug!("spells: the hellfire strikes {target} for {damages:?}");
                hits.write(fight::Hit {
                    attacker: aura.caster,
                    target,
                    share: 1.0,
                    ranged: false,
                    art: None,
                    fixed: Some(damages),
                    sure: true,
                });
            }
            aura.cooldown = BEAT;
            continue;
        }
        let only = (aura.strikes == Strikes::Bearer)
            .then_some(aura.follows)
            .flatten();
        let struck: Vec<Entity> = creatures
            .iter()
            .filter(|(target, at, way, mind, health, size, _)| {
                only.is_none_or(|only| only == *target)
                    && way.creature
                    && way.level == aura.level
                    && mind.hostile()
                    && health.current > 0
                    && (aura.classes.is_empty() || aura.classes.contains(&mind.class))
                    && (only.is_some()
                        || (at.translation().xz() - centre).length()
                            < aura.reach + size.map_or(arrows::size(0), |size| size.0))
            })
            .map(|(target, .., effects)| {
                (target, effects.is_some_and(|e| e.holds_any(&[aura.key])))
            })
            .collect::<Vec<_>>()
            .into_iter()
            .filter_map(|(target, keyed)| {
                let keyed = keyed || given.contains(&(target, aura.key));
                (aura.damages.is_some() || !keyed).then_some(target)
            })
            .collect();
        if struck.is_empty() {
            continue;
        }
        for target in struck {
            if let Some(damages) = aura.damages {
                debug!("spells: {entity} strikes {target} for {damages:?}");
                hits.write(fight::Hit {
                    attacker: aura.caster,
                    target,
                    share: 1.0,
                    ranged: false,
                    art: None,
                    fixed: Some(damages),
                    sure: true,
                });
            }
            if !aura.gives.is_empty() && !given.contains(&(target, aura.key)) {
                give(&mut commands, target, aura.gives.clone());
                given.push((target, aura.key));
            }
        }
        aura.cooldown = BEAT;
    }
}

/// Each sign of mines goes with its Dwarf and lays a mine where it stands once a second, unless
/// another lies within [`spells::MINES_APART`], until it carries none (`0x5B0719`); each mine goes
/// off as a hostile creature comes within its reach while its Dwarf is gone or stands
/// [`spells::MINE_SAFE`] or more off, or after [`spells::MINE_LIFE`] by itself, striking once the
/// hostile creatures within [`spells::BLAST_REACH`] and their size (`0x5B094A`, the object 794).
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn mine(
    mut commands: Commands,
    time: Res<Time>,
    looks: Option<Res<Looks>>,
    mut signs: Query<(Entity, &mut MineSign, &mut Transform), Without<Mine>>,
    mut mines: Query<(Entity, &mut Mine, &Transform), Without<MineSign>>,
    casters: Query<&GlobalTransform, (With<Body>, Without<Dies>)>,
    creatures: Query<
        (
            Entity,
            &GlobalTransform,
            &InTheWay,
            &ai::Mind,
            &Health,
            Option<&arrows::Size>,
        ),
        (Without<Body>, Without<Dies>),
    >,
    mut hits: MessageWriter<fight::Hit>,
) {
    let dt = time.delta_secs();
    let laid: Vec<Vec3> = mines.iter().map(|(_, _, at)| at.translation).collect();
    for (entity, mut sign, mut transform) in &mut signs {
        let Ok(caster) = casters.get(sign.caster) else {
            commands.entity(entity).despawn();
            continue;
        };
        transform.translation = caster.translation();
        sign.cooldown = (sign.cooldown - dt).max(0.0);
        let here = transform.translation;
        let near = laid
            .iter()
            .any(|at| (*at - here).xz().length() < spells::MINES_APART);
        if sign.cooldown > 0.0 || near {
            continue;
        }
        let mut made = commands.spawn((
            Mine {
                caster: sign.caster,
                damages: sign.damages,
                level: sign.level,
                left: spells::MINE_LIFE,
            },
            Transform::from_translation(here),
            Visibility::Inherited,
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(looks) = &looks {
            made.with_child((
                Mesh3d(looks.mine.0.clone()),
                MeshMaterial3d(looks.mine.1.clone()),
            ));
        }
        sign.left = sign.left.saturating_sub(1);
        sign.cooldown = BEAT;
        if sign.left == 0 {
            commands.entity(entity).despawn();
        }
    }
    // The creatures on `level`, alive and hostile to the hero, nearer `centre` than `reach` and
    // their size.
    let near = |level: Level, centre: Vec2, reach: f32| {
        creatures
            .iter()
            .filter(move |(_, at, way, mind, health, size)| {
                way.creature
                    && way.level == level
                    && mind.hostile()
                    && health.current > 0
                    && (at.translation().xz() - centre).length()
                        < reach + size.map_or(arrows::size(0), |size| size.0)
            })
            .map(|(target, ..)| target)
    };
    for (entity, mut mine, at) in &mut mines {
        mine.left -= dt;
        let centre = at.translation.xz();
        let away = casters.get(mine.caster).map_or(true, |caster| {
            (caster.translation().xz() - centre).length() >= spells::MINE_SAFE
        });
        let touched = away
            && near(mine.level, centre, spells::MINE_REACH)
                .next()
                .is_some();
        if !touched && mine.left > 0.0 {
            continue;
        }
        debug!("spells: a mine of {} goes off", mine.caster);
        for target in near(mine.level, centre, spells::BLAST_REACH) {
            hits.write(fight::Hit {
                attacker: mine.caster,
                target,
                share: 1.0,
                ranged: false,
                art: None,
                fixed: Some(mine.damages),
                sure: true,
            });
        }
        if let Some(looks) = &looks {
            commands.spawn((
                Mesh3d(looks.blast.0.clone()),
                MeshMaterial3d(looks.blast.1.clone()),
                Transform::from_translation(at.translation),
                Fades(0.3),
                DespawnOnExit(AppState::InGame),
            ));
        }
        commands.entity(entity).despawn();
    }
}

/// Each creature that the hero kills (its fall not a creature's) while the hero holds the
/// tentacles' effect raises a tentacle at its corpse, which strikes about itself once a second for
/// the record's value 1, 10 s (`0x533818`); while it holds the call of death's, it raises its
/// value (`0x5339F3`).
fn killed(
    died: On<Insert<Dies>>,
    mut commands: Commands,
    arts: Option<Res<crate::arts::HeroArts>>,
    looks: Option<Res<Looks>>,
    dead: Query<(&Dies, &GlobalTransform, Option<&InTheWay>), Without<Body>>,
    mut heroes: Query<(Entity, &mut Effects, Option<&Combat>), With<Body>>,
) {
    let Ok((dies, at, way)) = dead.get(died.entity) else {
        return;
    };
    if dies.by_creature {
        return;
    }
    let Some((hero, mut effects, combat)) = heroes.iter_mut().next() else {
        return;
    };
    effects.change(spells::DOOMED, |doom| {
        doom.value = spells::doom_kills(doom.value, doom.level);
    });
    let tentacles = effects
        .timed()
        .iter()
        .find(|timed| timed.effect.id == spells::TENTACLED)
        .map(|timed| u32::try_from(timed.effect.whole).unwrap_or(1));
    let (Some(level), Some(arts)) = (tentacles, arts) else {
        return;
    };
    let Some(record) = arts
        .tables
        .spells
        .iter()
        .find(|s| s.id == spells::TENTACLES)
    else {
        return;
    };
    let multipliers = combat.map_or([1.0; 4], |combat| combat.spells);
    let damages = bolt_damages(record, i32::try_from(level).unwrap_or(1), multipliers);
    let mut made = commands.spawn((
        Aura {
            caster: hero,
            follows: None,
            reach: spells::TENTACLE_REACH,
            classes: &[],
            damages: Some(damages),
            gives: Vec::new(),
            key: spells::TENTACLES,
            level: way.and_then(|way| way.level),
            left: fight_seconds(spells::value(record, 1, level)),
            cooldown: 0.0,
            strikes: Strikes::About,
        },
        Transform::from_translation(at.translation()),
        Visibility::Inherited,
        DespawnOnExit(AppState::InGame),
    ));
    if let Some(looks) = &looks {
        made.with_child((
            Mesh3d(looks.disc.clone()),
            MeshMaterial3d(looks.discs[6].clone()),
            Transform::from_scale(Vec3::new(
                spells::TENTACLE_REACH,
                2.0,
                spells::TENTACLE_REACH,
            ))
            .with_translation(Vec3::Y * 4.0),
        ));
    }
    debug!(
        "spells: a tentacle rises at {} for {damages:?}",
        died.entity
    );
}

/// The call of death's value falls with the time to no less than 1 (`0x56373E`); the effects'
/// recomputation does not take it, so its change is not one of theirs.
fn doom(time: Res<Time>, mut heroes: Query<&mut Effects, With<Body>>) {
    let dt = time.delta_secs();
    for mut effects in &mut heroes {
        effects
            .bypass_change_detection()
            .change(spells::DOOMED, |doom| {
                doom.value = spells::doom_fades(doom.value, doom.whole, dt);
            });
    }
}

/// Flies each hell disk on its target at its speed, the nearest hostile creature within its
/// look else, hovering by its caster with none; once a second it strikes the first creature it
/// touches and turns to the nearest other, else the same (`0x5A7DE0`, `0x5AF93F`). The cells it
/// may not enter, which turn it back, are not tested.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn disk(
    mut commands: Commands,
    time: Res<Time>,
    mut disks: Query<(Entity, &mut Disk, &mut Transform)>,
    casters: Query<&GlobalTransform, (With<Body>, Without<Disk>)>,
    creatures: Query<
        (
            Entity,
            &GlobalTransform,
            &InTheWay,
            &ai::Mind,
            &Health,
            Option<&arrows::Size>,
        ),
        (Without<Body>, Without<Disk>, Without<Dies>),
    >,
    mut hits: MessageWriter<fight::Hit>,
) {
    let dt = time.delta_secs();
    for (entity, mut disk, mut transform) in &mut disks {
        disk.left -= dt;
        disk.cooldown = (disk.cooldown - dt).max(0.0);
        if disk.left <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let here = transform.translation.xz();
        let level = disk.level;
        let live = |target: Entity| {
            creatures
                .get(target)
                .ok()
                .filter(|(_, _, way, mind, health, _)| {
                    way.creature && way.level == level && mind.hostile() && health.current > 0
                })
        };
        let nearest = |not: Option<Entity>| {
            creatures
                .iter()
                .filter(|(target, at, way, mind, health, _)| {
                    Some(*target) != not
                        && way.creature
                        && way.level == level
                        && mind.hostile()
                        && health.current > 0
                        && (at.translation().xz() - here).length() < spells::DISK_LOOKS
                })
                .min_by(|a, b| {
                    let d = |at: &GlobalTransform| (at.translation().xz() - here).length();
                    d(a.1).total_cmp(&d(b.1))
                })
                .map(|(target, ..)| target)
        };
        if disk.target.and_then(live).is_none() {
            disk.target = nearest(None);
        }
        let toward = match disk.target.and_then(live) {
            Some((_, at, ..)) => Some(at.translation().xz()),
            None => casters
                .get(disk.caster)
                .ok()
                .map(|at| at.translation().xz()),
        };
        if let Some(toward) = toward {
            let to = toward - here;
            let step = (disk.speed * dt).min(to.length());
            let next = here + to.normalize_or_zero() * step;
            transform.translation = Vec3::new(next.x, transform.translation.y, next.y);
        }
        if disk.cooldown > 0.0 {
            continue;
        }
        let here = transform.translation.xz();
        let touched: Vec<Entity> = creatures
            .iter()
            .filter(|(_, at, way, mind, health, size)| {
                way.creature
                    && way.level == level
                    && mind.hostile()
                    && health.current > 0
                    && (at.translation().xz() - here).length()
                        < disk.reach + size.map_or(arrows::size(0), |size| size.0)
            })
            .map(|(target, ..)| target)
            .take(if disk.all { usize::MAX } else { 1 })
            .collect();
        let Some(&struck) = touched.first() else {
            continue;
        };
        for &target in &touched {
            debug!("spells: {entity} strikes {target} for {:?}", disk.damages);
            hits.write(fight::Hit {
                attacker: disk.caster,
                target,
                share: 1.0,
                ranged: false,
                art: None,
                fixed: Some(disk.damages),
                sure: true,
            });
        }
        disk.cooldown = BEAT;
        disk.last = Some(struck);
        disk.target = nearest(Some(struck)).or(Some(struck));
    }
}

/// Lands each meteor after its wait, striking once the creatures within its reach and their
/// size (`0x5ACDC6`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn meteor(
    mut commands: Commands,
    time: Res<Time>,
    looks: Option<Res<Looks>>,
    mut meteors: Query<(Entity, &mut Meteor, &mut Transform, &mut Visibility)>,
    creatures: Query<
        (
            Entity,
            &GlobalTransform,
            &InTheWay,
            &ai::Mind,
            &Health,
            Option<&arrows::Size>,
        ),
        (Without<Body>, Without<Meteor>, Without<Dies>),
    >,
    mut hits: MessageWriter<fight::Hit>,
) {
    let dt = time.delta_secs();
    for (entity, mut meteor, mut transform, mut visibility) in &mut meteors {
        meteor.lands -= dt;
        if meteor.lands > spells::METEOR_FALLS {
            continue;
        }
        if meteor.lands > 0.0 {
            // Falling from 500 at 800 a second.
            *visibility = Visibility::Inherited;
            transform.translation =
                meteor.at + Vec3::Y * (meteor.lands / spells::METEOR_FALLS * 500.0);
            continue;
        }
        let centre = meteor.at.xz();
        for (target, at, way, mind, health, size) in &creatures {
            if way.creature
                && way.level == meteor.level
                && mind.hostile()
                && health.current > 0
                && (at.translation().xz() - centre).length()
                    < spells::METEOR_REACH + size.map_or(arrows::size(0), |size| size.0)
            {
                hits.write(fight::Hit {
                    attacker: meteor.caster,
                    target,
                    share: 1.0,
                    ranged: false,
                    art: None,
                    fixed: Some(meteor.damages),
                    sure: true,
                });
            }
        }
        if let Some(looks) = &looks {
            commands.spawn((
                Mesh3d(looks.blast.0.clone()),
                MeshMaterial3d(looks.blast.1.clone()),
                Transform::from_translation(meteor.at).with_scale(Vec3::splat(0.8)),
                Fades(0.3),
                DespawnOnExit(AppState::InGame),
            ));
        }
        commands.entity(entity).despawn();
    }
}

/// Leaps each chain lightning on, after its wait, to the nearest creature hostile to the hero
/// within its look of the last it struck, the last aside, striking it, while strikes are left
/// (`0x5B46CA`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn chain(
    mut commands: Commands,
    time: Res<Time>,
    looks: Option<Res<Looks>>,
    mut chains: Query<(Entity, &mut Chain)>,
    creatures: Query<
        (Entity, &GlobalTransform, &InTheWay, &ai::Mind, &Health),
        (Without<Body>, Without<Dies>),
    >,
    places: Query<&GlobalTransform>,
    mut hits: MessageWriter<fight::Hit>,
) {
    let dt = time.delta_secs();
    for (entity, mut chain) in &mut chains {
        chain.leaps -= dt;
        if chain.leaps > 0.0 {
            continue;
        }
        let Ok(from) = places.get(chain.last).map(GlobalTransform::translation) else {
            commands.entity(entity).despawn();
            continue;
        };
        let next = creatures
            .iter()
            .filter(|(target, at, way, mind, health)| {
                *target != chain.last
                    && way.creature
                    && way.level == chain.level
                    && mind.hostile()
                    && health.current > 0
                    && (at.translation() - from).xz().length() < spells::CHAIN_LOOKS
            })
            .min_by(|a, b| {
                let d = |at: &GlobalTransform| (at.translation() - from).xz().length();
                d(a.1).total_cmp(&d(b.1))
            });
        let Some((target, at, ..)) = (chain.left > 0).then_some(next).flatten() else {
            commands.entity(entity).despawn();
            continue;
        };
        debug!("spells: the chain lightning leaps to {target}");
        hits.write(fight::Hit {
            attacker: chain.caster,
            target,
            share: 1.0,
            ranged: false,
            art: None,
            fixed: Some(chain.damages),
            sure: true,
        });
        if let Some(looks) = &looks {
            commands.spawn((
                Mesh3d(looks.column.clone()),
                MeshMaterial3d(looks.lightning.clone()),
                Transform::from_translation(at.translation() + Vec3::Y * 200.0),
                Fades(LIGHTNING_SHOWS),
                DespawnOnExit(AppState::InGame),
            ));
        }
        chain.last = target;
        chain.left -= 1;
        chain.leaps = spells::CHAIN_LEAPS;
    }
}

/// Turns each flame of the fire spiral about its centre, its radius growing, and strikes the
/// creatures it touches once a second (`0x5A6B1E`, `0x5ACB9C`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn spiral(
    mut commands: Commands,
    time: Res<Time>,
    mut flames: Query<(Entity, &mut Flame, &mut Transform)>,
    creatures: Query<
        (
            Entity,
            &GlobalTransform,
            &InTheWay,
            &ai::Mind,
            &Health,
            Option<&arrows::Size>,
        ),
        (Without<Body>, Without<Flame>, Without<Dies>),
    >,
    mut hits: MessageWriter<fight::Hit>,
) {
    let dt = time.delta_secs();
    for (entity, mut flame, mut transform) in &mut flames {
        flame.left -= dt;
        if flame.left <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        flame.angle += spells::SPIRAL_TURNS * dt;
        flame.grows = (flame.grows + spells::SPIRAL_GROWS * dt).min(spells::SPIRAL_MOST);
        let radius = spells::spiral_radius(flame.grows, flame.angle);
        let at = flame.angle
            + std::f32::consts::TAU * f32::from(flame.index) / f32::from(spells::SPIRAL_FLAMES);
        let place = flame.centre + Vec3::new(at.cos(), 0.0, at.sin()) * radius;
        transform.translation = place.with_y(transform.translation.y);
        flame.cooldown = (flame.cooldown - dt).max(0.0);
        if flame.cooldown > 0.0 {
            continue;
        }
        let mut struck = false;
        for (target, at, way, mind, health, size) in &creatures {
            if way.creature
                && way.level == flame.level
                && mind.hostile()
                && health.current > 0
                && (at.translation() - place).xz().length()
                    < spells::SPIRAL_REACH + size.map_or(arrows::size(0), |size| size.0)
            {
                hits.write(fight::Hit {
                    attacker: flame.caster,
                    target,
                    share: 1.0,
                    ranged: false,
                    art: None,
                    fixed: Some(flame.damages),
                    sure: true,
                });
                struck = true;
            }
        }
        if struck {
            flame.cooldown = BEAT;
        }
    }
}

/// Counts each banishing circle down, and sends away each hostile creature on its level that
/// stands in it, but one that runs out of it already (a creature's step, `0x5941A0`); each
/// banished creature leaves its fight and runs 700 beyond its place away from the circle's centre
/// (`0x539F60`), where it comes on again as it will.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn banish(
    mut commands: Commands,
    time: Res<Time>,
    mut zones: Query<(Entity, &mut Banishing)>,
    mut creatures: Query<
        (
            Entity,
            &Transform,
            &InTheWay,
            &mut ai::Mind,
            Option<&crate::npcs::Walk>,
            Option<&Banished>,
        ),
        (Without<Body>, Without<Dies>),
    >,
) {
    let dt = time.delta_secs();
    for (entity, mut zone) in &mut zones {
        zone.left -= dt;
        if zone.left <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        for (creature, at, way, mind, walk, banished) in &creatures {
            let runs_out = walk.is_some_and(|walk| !inside(&zone, walk.target()));
            if banished.is_none()
                && !runs_out
                && way.level == zone.level
                && mind.hostile()
                && inside(&zone, at.translation)
            {
                commands
                    .entity(creature)
                    .insert(Banished { from: zone.centre });
            }
        }
    }
    for (creature, at, way, mut mind, _, banished) in &mut creatures {
        let Some(banished) = banished else {
            continue;
        };
        let here = at.translation;
        let [x, z] = spells::banished_to([here.x, here.z], [banished.from.x, banished.from.z]);
        debug!("spells: {creature} is banished to {x:.0},{z:.0}");
        mind.called_off();
        commands
            .entity(creature)
            .remove::<(Banished, crate::npcs::Strikes)>()
            .insert(crate::npcs::Walk::run(
                Vec3::new(x, here.y, z),
                way.level,
                mind.run,
            ));
    }
}

/// Changes `original` into an `animal` for `seconds` (the effect 20's adder, `0x5848A8`): an
/// animal of its level and health where it stands, it hidden, its mind and its place in the way
/// kept.
fn change(world: &mut World, original: Entity, animal: u32, seconds: f32) {
    let (Some(at), Some(mind), Some(way), Some(health)) = (
        world.get::<Transform>(original).map(|at| at.translation),
        world.get::<ai::Mind>(original).cloned(),
        world.get::<InTheWay>(original).copied(),
        world.get::<Health>(original).copied(),
    ) else {
        return;
    };
    let Some(heights) = world.get_resource::<crate::terrain::Heights>() else {
        return;
    };
    let floor = heights
        .floor(way.level)
        .map_or(0, |(_, floor)| i32::from(floor));
    let (x, y) = crate::paths::cell_of(at.x, at.z);
    let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
        return;
    };
    let request = crate::npcs::Spawn {
        creature: animal,
        cells: vec![(x, y)],
        floor,
        level: Some(health.level),
        rank: fight::Rank::Common,
        hostile: Some(false),
        immortal: false,
        roams: true,
    };
    let made = match world.run_system_cached_with(crate::npcs::spawn_creatures, request) {
        Ok(Ok(made)) => made,
        Ok(Err(e)) => {
            warn!("spells: {original} is not changed: {e}");
            return;
        }
        Err(e) => {
            warn!("spells: {original} is not changed: {e}");
            return;
        }
    };
    let Some(&made) = made.first() else {
        return;
    };
    info!("spells: {original} changes into {made}, a {animal}, for {seconds} s");
    world.entity_mut(made).insert((
        health,
        Changeling {
            original,
            left: seconds,
        },
    ));
    world
        .entity_mut(original)
        .remove::<(ai::Mind, InTheWay, crate::npcs::Walk, crate::npcs::Strikes)>()
        .insert((
            Visibility::Hidden,
            Changed {
                mind: Box::new(mind),
                way,
            },
        ));
}

/// Changes each changeling's creature back once its time is out, where the animal stands with
/// the animal's health, the animal gone; one whose animal died dies there (the effect 20's
/// taker, `0x587F6A`).
fn changelings(
    mut commands: Commands,
    time: Res<Time>,
    mut animals: Query<(Entity, &mut Changeling, &Transform, &Health, Has<Dies>)>,
) {
    let dt = time.delta_secs();
    for (animal, mut changeling, at, health, dead) in &mut animals {
        changeling.left -= dt;
        if !dead && changeling.left > 0.0 {
            continue;
        }
        let (original, place, health) = (changeling.original, at.translation, *health);
        commands.entity(animal).remove::<Changeling>();
        if !dead {
            commands.entity(animal).despawn();
        }
        commands.queue(move |world: &mut World| {
            let Ok(mut entity) = world.get_entity_mut(original) else {
                return;
            };
            let Some(changed) = entity.take::<Changed>() else {
                return;
            };
            info!("spells: {original} changes back, dead {dead}");
            let level = changed.way.level;
            entity.insert((
                *changed.mind,
                changed.way,
                Visibility::Inherited,
                crate::npcs::PutAt { at: place, level },
            ));
            if let Some(mut kept) = entity.get_mut::<Health>() {
                kept.current = if dead {
                    0
                } else {
                    health.current.min(kept.most)
                };
            }
            if dead {
                entity.insert(Dies::new(0));
            }
        });
    }
}

/// Counts each ring of frost's zone down, and gives each hostile creature on its level that stands
/// in it the effect 15 keyed by the ring for 1 s, again while it stays, whose value is its time's
/// factor (`0x59F960`, `0x5941A0`, `opensacred_rules::spells::frost_slow`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn frost(
    mut commands: Commands,
    time: Res<Time>,
    mut zones: Query<(Entity, &mut Frosting)>,
    creatures: Query<
        (
            Entity,
            &Transform,
            &InTheWay,
            &ai::Mind,
            &Health,
            Option<&fight::Ranked>,
            Option<&Effects>,
        ),
        (Without<Body>, Without<Dies>),
    >,
) {
    let dt = time.delta_secs();
    for (entity, mut zone) in &mut zones {
        zone.left -= dt;
        if zone.left <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        for (creature, at, way, mind, health, rank, effects) in &creatures {
            let (x, y) = crate::paths::cell_of(at.translation.x, at.translation.z);
            let inside = (x - zone.cell.0).abs() <= spells::ZONE_REACH
                && (y - zone.cell.1).abs() <= spells::ZONE_REACH;
            let slowed = effects.is_some_and(|effects| effects.holds_any(&[spells::FROST_RING]));
            if !inside || slowed || way.level != zone.level || !mind.hostile() {
                continue;
            }
            let boss = rank.is_some_and(|rank| rank.0 == fight::Rank::Boss);
            let value = spells::frost_slow(zone.spell, health.level, boss, mind.class == 10);
            let effect = Effect {
                art: u32::from(spells::FROST_RING),
                ..Effect::new(opensacred_rules::fight::TIME, value)
            };
            debug!("spells: the frost slows {creature} to {value:.2}");
            commands
                .entity(creature)
                .entry::<Effects>()
                .or_default()
                .and_modify(move |mut effects| effects.add(effect, spells::FROST_SLOWS_FOR));
        }
    }
}

/// Puts each teleporting hero at its point once its time comes, and counts the water form down.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their components"
)]
fn teleport(
    mut commands: Commands,
    time: Res<Time>,
    mut heroes: Query<(
        Entity,
        &mut Transform,
        &mut Body,
        Option<&mut Teleporting>,
        Option<&mut Unseen>,
    )>,
) {
    let dt = time.delta_secs();
    for (hero, mut transform, mut body, teleporting, unseen) in &mut heroes {
        if let Some(mut teleporting) = teleporting {
            teleporting.left -= dt;
            if teleporting.left <= 0.0 {
                debug!("spells: the hero teleports to {}", teleporting.to);
                body.teleport(&mut transform, teleporting.to);
                commands.entity(hero).remove::<Teleporting>();
            }
        }
        if let Some(mut unseen) = unseen {
            if unseen.after > 0.0 {
                unseen.after -= dt;
            } else {
                unseen.left -= dt;
                if unseen.left <= 0.0 {
                    commands.entity(hero).remove::<Unseen>();
                }
            }
        }
    }
}

/// Takes each sight away after its time.
fn fade(time: Res<Time>, mut commands: Commands, mut sights: Query<(Entity, &mut Fades)>) {
    for (entity, mut fades) in &mut sights {
        fades.0 -= time.delta_secs();
        if fades.0 <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

/// Gives `bearer` the effects `given`, each for its seconds.
fn give(commands: &mut Commands, bearer: Entity, given: Vec<(Effect, f32)>) {
    if given.is_empty() {
        return;
    }
    debug!("spells: {bearer} takes {given:?}");
    commands
        .entity(bearer)
        .entry::<Effects>()
        .or_default()
        .and_modify(move |mut effects| {
            for (effect, seconds) in given {
                effects.add(effect, seconds);
            }
        });
}

/// Runs each stone skin's shield and gives its effects as its clock passes (its system's update
/// and the action 6).
fn shield(time: Res<Time>, mut commands: Commands, mut shields: Query<(Entity, &mut Shielding)>) {
    for (bearer, mut shielding) in &mut shields {
        shielding.left -= time.delta_secs();
        if shielding.left > 0.0 {
            continue;
        }
        let given = std::mem::take(&mut shielding.effects);
        commands.entity(bearer).remove::<Shielding>();
        give(&mut commands, bearer, given);
    }
}

/// The spells' acts.
pub struct SpellsPlugin;

impl Plugin for SpellsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Acts>()
            .add_observer(killed)
            .insert_resource(Dice(crate::spawning::Rng::of(0, 0, 64)))
            .add_systems(Startup, make_looks)
            .add_systems(
                Update,
                (
                    act,
                    shield,
                    strike,
                    mine,
                    disk,
                    meteor,
                    chain,
                    spiral,
                    banish,
                    frost,
                    changelings,
                    (balls, pentagrams, confusions, fogs),
                    skulls,
                    teleport,
                    doom,
                    fade,
                )
                    .chain()
                    .after(Step::Npcs)
                    .in_set(Playing),
            );
    }
}
