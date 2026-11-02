//! The creatures' casts (docs/re/ai.md, "The creatures' spells"; #1852): a caster's decision in
//! its melee (`ai`, `opensacred_rules::minds::Mind::cast`) orders a cast, whose clip of its spell
//! and type (`opensacred_rules::casts::clip`) plays at the creature's speed, and whose spell acts
//! at its share of the clip (`npcs::ClipActs`), at the level a creature makes a spell it lacks
//! (`fight::Rules::creature_spell_level`), its damage times `MagieAll`'s factor
//! (`fight::Rules::creature_spell_damage`), and whose clip's end gives the creature its melee
//! again from its start (`opensacred_rules::minds::Mind::cast_ends`). The game follows the mark
//! 1's spells as the hero's make them, striking the hero: the magic skeletons' fireball and the
//! mummies' whirlwind (`bolts`), the mummy 03's lightning and the orc shaman's meteors (`spells`),
//! and their flesh to stone, which holds the hero (`hero_held`); the dragons' meteors; the big
//! demons' eye lasers and their homing fire ([`HomingFire`]), whose records of no damage land the
//! demon's own strike (`fight::resolve`); the mark 8's magicians' spells: theirs of the hero's
//! and the mark 1's, the plant cage, which holds the hero, the fire bolt and the goblin mages' ball
//! ([`Ball`]), the purgatory, the bubble and the thorns, and their heals and shields of their
//! friends.

use bevy::prelude::*;
use opensacred_rules::arts::bolt_damages;
use opensacred_rules::{casts, spells};

use crate::ai::Mind;
use crate::crowd::InTheWay;
use crate::frame::Playing;
use crate::game::AppState;
use crate::hero::Body;
use crate::npcs::{ClipActs, Dies, ScriptClip};
use crate::terrain::{Heights, Level};

/// The spells of the casts that the game follows, the mark 1's and the magicians' of them.
const FOLLOWED: [u16; 17] = [
    casts::HEAL,
    casts::FIRE_SKIN,
    casts::STONE_SKIN,
    spells::PURGATORY,
    spells::BUBBLE,
    spells::THORNS,
    spells::FIREBALL,
    spells::WHIRLWIND,
    spells::LIGHTNING,
    spells::METEORS,
    spells::STONE,
    casts::EYE_LASERS,
    casts::DEMON_FIRE,
    spells::SPLINTERS,
    casts::CAGE,
    FIRE_BOLT,
    BALL,
];

/// The creatures' own fire bolt (52) and the goblin mages' ball (53).
const FIRE_BOLT: u16 = 52;
const BALL: u16 = 53;

/// Whether the game follows a creature's cast of the spell `spell`.
pub fn follows(spell: u16) -> bool {
    FOLLOWED.contains(&spell)
}

/// Whether a creature of type `creature` is a caster whose spell the game follows: of the mark 1,
/// a dragon, a big demon (#1853) or a magician of the mark 8 (#1854).
pub fn casts(creature: u32) -> bool {
    casts::dragon(creature)
        || casts::big_demon(creature)
        || casts::mark_8(creature)
        || casts::mark_1(creature).is_some_and(|(spell, _)| FOLLOWED.contains(&spell))
}

/// The height of a big demon's eyes over its feet, whence its lasers leave (**estimate**: its
/// bones `Left_EyeBone_FX` and `Right_EyeBone_FX` are not read).
const EYES: f32 = 250.0;

/// A homing fire of a creature (the big demons' `FX_DAEMONFIRE`, 910, of the spell 50; docs/re/
/// arts.md, "The creatures' own spells"): held at its caster for its charge, then flying at
/// [`casts::HOMING_SPEED`] toward its target, turning by at most [`casts::HOMING_TURN`] radians a
/// second, for [`casts::HOMING_FLIGHT`] seconds; at its first contact with the hero, on its level
/// and within its size and the hero's, it bursts and strikes it once with its damages.
#[derive(Component, Debug)]
pub struct HomingFire {
    caster: Entity,
    target: Entity,
    charge: f32,
    left: f32,
    direction: Vec2,
    size: f32,
    damages: [f32; 4],
    level: Level,
}

/// A creature's cast in course, until its clip ends: its spell and its variant, its target and
/// the target's place as the cast was ordered, and whether its clip has begun.
#[derive(Component, Debug)]
pub struct Casting {
    spell: u16,
    variant: u8,
    target: Entity,
    point: Vec3,
    begun: bool,
}

impl Casting {
    pub fn new(spell: u16, variant: u8, target: Entity, point: Vec3) -> Self {
        Self {
            spell,
            variant,
            target,
            point,
            begun: false,
        }
    }
}

/// Begins each cast's clip: its slot by spell and type, its spell acting at the spell's share
/// (`0x521D60`).
fn begin(mut commands: Commands, mut casting: Query<(Entity, &mut Casting, &Mind), Without<Dies>>) {
    for (entity, mut cast, mind) in &mut casting {
        if cast.begun {
            continue;
        }
        cast.begun = true;
        let slot = casts::clip(cast.spell, mind.creature, casts::dragon(mind.creature));
        let share = spells::share(cast.spell).unwrap_or(1.0);
        debug!(
            "creature_casts: {entity} casts {} (variant {}) by its clip {slot}",
            cast.spell, cast.variant
        );
        commands
            .entity(entity)
            .insert(ScriptClip::casting(slot, share));
    }
}

/// Makes each cast's spell act once its clip passes its share, at the creature's spell's level,
/// its damage `MagieAll`'s share of the hero's, as the hero's spell acts (`spells`): the
/// fireball a bolt toward its target's place, the whirlwind a fan of bolts along the line to it,
/// the lightning a strike on its target, the meteors a hail about its target's place as the cast
/// was ordered, the stone the effect 15 on the hero for its value 0 in seconds (`0x52494F`),
/// refused on a hero that an effect of the stone holds already (`0x582B00`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn act(
    mut commands: Commands,
    acting: Query<
        (
            Entity,
            &Casting,
            &Transform,
            (&Mind, &InTheWay),
            Option<&crate::fight::Ranked>,
            Option<&crate::fight::Combat>,
        ),
        With<ClipActs>,
    >,
    places: Query<&GlobalTransform>,
    held: Query<Option<&crate::effects::Effects>, With<crate::hero::Body>>,
    (heights, bolt_looks, mut weather, mut sounds): (
        Res<Heights>,
        Option<Res<crate::bolts::Looks>>,
        Option<ResMut<crate::weather::Weather>>,
        MessageWriter<crate::sfx::Play>,
    ),
    (arts, rules, looks, mut dice): (
        Option<Res<crate::arts::HeroArts>>,
        Res<crate::fight::Rules>,
        Option<Res<crate::spells::Looks>>,
        ResMut<crate::spells::Dice>,
    ),
    (mut launches, mut hits, mut healths): (
        MessageWriter<crate::bolts::Launch>,
        MessageWriter<crate::fight::Hit>,
        Query<&mut crate::fight::Health>,
    ),
) {
    for (entity, cast, at, (mind, way), rank, combat) in &acting {
        commands.entity(entity).remove::<ClipActs>();
        let Some(record) = arts
            .as_ref()
            .and_then(|arts| arts.tables.spells.iter().find(|s| s.id == cast.spell))
        else {
            continue;
        };
        let rank = rank.map_or(crate::fight::Rank::Common, |rank| rank.0);
        let level = rules.0.creature_spell_level(mind.level, rank);
        let factor = rules.0.creature_spell_damage(level);
        let multipliers = combat.map_or([1.0; 4], |combat| combat.spells);
        let damages = bolt_damages(record, i32::try_from(level).unwrap_or(1), multipliers)
            .map(|damage| scaled(damage, factor));
        let here = at.translation;
        let target = places.get(cast.target).map(GlobalTransform::translation);
        let toward = target.unwrap_or(cast.point);
        let line = (toward - here).xz();
        let direction = line
            .try_normalize()
            .unwrap_or_else(|| (at.rotation * Vec3::X).xz().normalize_or(Vec2::X));
        info!(
            "creature_casts: {entity} casts {} at {toward} with {damages:?}",
            cast.spell
        );
        match cast.spell {
            spells::FIREBALL => {
                launches.write(crate::bolts::Launch {
                    shooter: entity,
                    direction,
                    course: crate::bolts::Course::fireball(level),
                    damages,
                    wait: 0.0,
                });
            }
            spells::WHIRLWIND | spells::SPLINTERS => {
                // The hero's fan (`0x524EA2`, `0x5258B5`), the whirlwind's without its knockback,
                // which strikes creatures.
                let course = if cast.spell == spells::WHIRLWIND {
                    crate::bolts::Course::passing(spells::WHIRLWIND_COURSE, 4)
                } else {
                    crate::bolts::Course::passing(spells::SPLINTERS_COURSE, 5)
                };
                for angle in spells::fan(cast.spell, spells::count(record, level), line.length()) {
                    launches.write(crate::bolts::Launch {
                        shooter: entity,
                        direction: Vec2::from_angle(angle).rotate(direction),
                        course,
                        damages,
                        wait: 0.0,
                    });
                }
            }
            spells::LIGHTNING => {
                let Ok(place) = target else {
                    continue;
                };
                hits.write(crate::fight::Hit {
                    attacker: entity,
                    target: cast.target,
                    share: 1.0,
                    ranged: false,
                    art: None,
                    fixed: Some(damages),
                    sure: true,
                    poison: false,
                });
                if let Some(looks) = &looks {
                    crate::spells::lightning(&mut commands, looks, place);
                }
            }
            spells::STONE | casts::CAGE => {
                // The stone, or the plant cage about the hero (its object 849, `0x52494F`): the
                // effect 15 keyed by its spell, refused on a hero held by it already.
                let Ok(effects) = held.get(cast.target) else {
                    continue;
                };
                if effects.is_some_and(|effects| effects.holds_any(&[cast.spell])) {
                    continue;
                }
                let seconds =
                    f32::from(u16::try_from(spells::value(record, 0, level)).unwrap_or(u16::MAX));
                let effect = opensacred_rules::fight::Effect {
                    id: spells::HELD,
                    whole: 0,
                    value: 0.0,
                    art: u32::from(cast.spell),
                    level: 0,
                };
                commands
                    .entity(cast.target)
                    .entry::<crate::effects::Effects>()
                    .or_default()
                    .and_modify(move |mut effects| effects.add(effect, seconds));
            }
            casts::HEAL => {
                // Its friend's health by the record's value 0 at its level (`0x5229EF`; no magic
                // lore for a creature, **hypothesis**).
                let health = spells::healing(record, level, 0.0);
                if let Ok(mut friend) = healths.get_mut(cast.target) {
                    friend.current = friend.current.saturating_add(health).min(friend.most);
                    info!(
                        "creature_casts: {entity} heals {} by {health}, to {}",
                        cast.target, friend.current
                    );
                }
            }
            casts::FIRE_SKIN | casts::STONE_SKIN => {
                let rate = combat.map_or(opensacred_rules::fight::CAST_SPEED, |combat| combat.cast)
                    / 100.0;
                crate::spells::skin(
                    &mut commands,
                    looks.as_deref(),
                    (record, level),
                    (entity, cast.target, way.level),
                    rate,
                    damages,
                );
            }
            spells::PURGATORY => {
                crate::spells::purgatory(
                    &mut commands,
                    looks.as_deref(),
                    (record, level),
                    (entity, way.level),
                    cast.point,
                    damages,
                );
            }
            spells::THORNS => {
                crate::spells::thorns(
                    &mut commands,
                    (&mut dice, &heights, looks.as_deref()),
                    (record, level),
                    (entity, way.level),
                    toward,
                    damages,
                );
            }
            spells::BUBBLE => {
                let mut course = crate::bolts::Course::passing(spells::BUBBLE_COURSE, 6);
                course.beat = spells::BUBBLE_BEAT;
                course.bursts = Some(spells::BURST);
                launches.write(crate::bolts::Launch {
                    shooter: entity,
                    direction,
                    course,
                    damages,
                    wait: 0.0,
                });
            }
            FIRE_BOLT => {
                launches.write(crate::bolts::Launch {
                    shooter: entity,
                    direction,
                    course: crate::bolts::Course::fire_bolt(),
                    damages,
                    wait: 0.0,
                });
            }
            BALL => {
                // Thrown to the order's point in the traps' arc (`0x527949`), the goblin mage
                // 127's jittered by up to 100 along each axis.
                let mut to = cast.point;
                if mind.creature == 127 {
                    let mut jitter = || {
                        let roll = i16::try_from(dice.0.below(201)).unwrap_or(100);
                        f32::from(roll - 100)
                    };
                    to += Vec3::new(jitter(), 0.0, jitter());
                }
                let mut ball = commands.spawn((
                    Ball {
                        caster: entity,
                        from: here,
                        to,
                        flown: 0.0,
                        damages,
                        level: way.level,
                    },
                    Transform::from_translation(here + Vec3::Y * crate::bolts::HEIGHT),
                    Visibility::Inherited,
                    DespawnOnExit(AppState::InGame),
                ));
                if let Some((mesh, material)) = bolt_looks
                    .as_ref()
                    .and_then(|looks| looks.0.get(BALL_LOOK).cloned())
                {
                    ball.insert((Mesh3d(mesh), MeshMaterial3d(material)));
                }
                sounds.write(crate::sfx::Play {
                    sound: crate::sfx::Of::Id(BALL_SOUND),
                    at: Some(entity),
                    rate: (1.0, crate::sfx::Roll::None),
                });
            }
            casts::EYE_LASERS => {
                // Its two beams to the target's place, which land the demon's own strike where a
                // straight line of cells reaches it (`0x604030`).
                let Ok(place) = target else {
                    continue;
                };
                let reaches = crate::paths::straight(here.xz(), place.xz(), |cell| {
                    heights.walkable_on(cell, way.level)
                });
                if let Some(looks) = &looks {
                    crate::spells::beam(
                        &mut commands,
                        looks,
                        here + Vec3::Y * EYES,
                        place + Vec3::Y * crate::bolts::HEIGHT,
                    );
                }
                if reaches {
                    hits.write(crate::fight::Hit {
                        attacker: entity,
                        target: cast.target,
                        share: 1.0,
                        ranged: false,
                        art: None,
                        fixed: Some(damages),
                        sure: true,
                        poison: false,
                    });
                }
            }
            casts::DEMON_FIRE => {
                // Its fire charging at its hand, a jolt of the screen and its sound (`0x5274ED`).
                let mut fire = commands.spawn((
                    HomingFire {
                        caster: entity,
                        target: cast.target,
                        charge: casts::DEMON_FIRE_CHARGE,
                        left: casts::HOMING_FLIGHT,
                        direction,
                        size: casts::DEMON_FIRE_SIZE,
                        damages,
                        level: way.level,
                    },
                    Transform::from_translation(here + Vec3::Y * crate::bolts::HEIGHT),
                    Visibility::Inherited,
                    DespawnOnExit(AppState::InGame),
                ));
                if let Some((mesh, material)) = bolt_looks
                    .as_ref()
                    .and_then(|looks| looks.0.get(FIRE_LOOK).cloned())
                {
                    fire.insert((Mesh3d(mesh), MeshMaterial3d(material)));
                }
                if let Some(weather) = weather.as_mut() {
                    weather.set(opensacred_rules::weather::Event::Jolt(DEMON_FIRE_JOLT));
                }
                sounds.write(crate::sfx::Play {
                    sound: crate::sfx::Of::Id(DEMON_FIRE_SOUND),
                    at: Some(entity),
                    rate: (0.49, crate::sfx::Roll::Share(0.02)),
                });
            }
            spells::METEORS => {
                crate::spells::meteors(
                    &mut commands,
                    &mut dice,
                    (record, level),
                    (entity, way.level),
                    cast.point,
                    damages,
                );
            }
            _ => {}
        }
    }
}

/// The look of a homing fire, the fireball's (`bolts`), and of a ball, the railgun's white one.
const FIRE_LOOK: usize = 3;
const BALL_LOOK: usize = 0;
/// The ball's sound as it starts (`SOUND_FX_DELF_WURFCAST`).
const BALL_SOUND: u32 = 2057;

/// A goblin mage's ball (the spell 53's `FX_BALL`, 867; docs/re/arts.md, "The creatures' own
/// spells"): it flies the traps' arc from its caster to its point over 2/3 s, and landing strikes
/// once the hero within its size 20 and the hero's, on its level, with its damages.
#[derive(Component, Debug)]
pub struct Ball {
    caster: Entity,
    from: Vec3,
    to: Vec3,
    flown: f32,
    damages: [f32; 4],
    level: Level,
}

/// How fast a ball's share of its arc grows a second, how high its arc rises, and its size.
const BALL_RATE: f32 = 1.5;
const BALL_ARC: f32 = 60.0;
const BALL_SIZE: f32 = 20.0;

/// Flies each ball along its arc, `start + (point − start)·s` and `60·(1 − 4(s − 0.5)²)` higher, and
/// lands it at `s` 1 (the update at `0x5A7179`).
fn throw(
    mut commands: Commands,
    time: Res<Time>,
    mut balls: Query<(Entity, &mut Ball, &mut Transform)>,
    heroes: Query<(Entity, &GlobalTransform, &Body, &crate::fight::Health), Without<Dies>>,
    mut hits: MessageWriter<crate::fight::Hit>,
) {
    for (entity, mut ball, mut transform) in &mut balls {
        ball.flown = (ball.flown + BALL_RATE * time.delta_secs()).min(1.0);
        let s = ball.flown;
        let rise = BALL_ARC * (1.0 - 4.0 * (s - 0.5) * (s - 0.5));
        transform.translation =
            ball.from + (ball.to - ball.from) * s + Vec3::Y * (crate::bolts::HEIGHT + rise);
        if s < 1.0 {
            continue;
        }
        for (hero, at, body, health) in &heroes {
            if body.level() == ball.level
                && health.current > 0
                && (at.translation().xz() - ball.to.xz()).length()
                    < BALL_SIZE + opensacred_rules::arrows::size(0)
            {
                debug!(
                    "creature_casts: the ball of {} lands on {hero}",
                    ball.caster
                );
                hits.write(crate::fight::Hit {
                    attacker: ball.caster,
                    target: hero,
                    share: 1.0,
                    ranged: false,
                    art: None,
                    fixed: Some(ball.damages),
                    sure: true,
                    poison: false,
                });
            }
        }
        commands.entity(entity).despawn();
    }
}
/// The jolt of the screen as a big demon's fire leaves (`0x527716`) and its sound
/// (`SOUND_FX_VAMP_SPRUNG_HIT`).
const DEMON_FIRE_JOLT: u32 = 18;
const DEMON_FIRE_SOUND: u32 = 2084;

/// Flies each homing fire a frame on (the object update's flag `0x10000`, `0x5A7DBF`): held at its
/// caster while it charges, then toward its target, turning by at most its turn a second, ending at
/// its time, at a cell it may not pass, or as it bursts on the hero.
fn home(
    mut commands: Commands,
    time: Res<Time>,
    heights: Res<Heights>,
    mut fires: Query<(Entity, &mut HomingFire, &mut Transform)>,
    places: Query<&GlobalTransform, Without<HomingFire>>,
    heroes: Query<(Entity, &GlobalTransform, &Body, &crate::fight::Health), Without<Dies>>,
    mut hits: MessageWriter<crate::fight::Hit>,
) {
    let dt = time.delta_secs();
    for (entity, mut fire, mut transform) in &mut fires {
        if fire.charge > 0.0 {
            fire.charge -= dt;
            if let Ok(caster) = places.get(fire.caster) {
                transform.translation = caster.translation() + Vec3::Y * crate::bolts::HEIGHT;
            }
            continue;
        }
        fire.left -= dt;
        let here = transform.translation.xz();
        if let Ok(target) = places.get(fire.target)
            && let Some(aim) = (target.translation().xz() - here).try_normalize()
        {
            let turn = fire.direction.angle_to(aim);
            let most = casts::HOMING_TURN * dt;
            fire.direction = Vec2::from_angle(turn.clamp(-most, most)).rotate(fire.direction);
        }
        let ground = here + fire.direction * casts::HOMING_SPEED * dt;
        let cell = crate::paths::cell_of(ground.x, ground.y);
        if fire.left <= 0.0 || !heights.walkable_on(cell, fire.level) {
            commands.entity(entity).despawn();
            continue;
        }
        let height = heights
            .standing(ground.x, ground.y, fire.level)
            .unwrap_or(transform.translation.y - crate::bolts::HEIGHT);
        transform.translation = Vec3::new(ground.x, height + crate::bolts::HEIGHT, ground.y);
        let struck: Vec<Entity> = heroes
            .iter()
            .filter(|(_, at, body, health)| {
                body.level() == fire.level
                    && health.current > 0
                    && (at.translation().xz() - ground).length()
                        < fire.size + opensacred_rules::arrows::size(0)
            })
            .map(|(hero, ..)| hero)
            .collect();
        if struck.is_empty() {
            continue;
        }
        for target in struck {
            debug!(
                "creature_casts: the fire of {} bursts on {target}",
                fire.caster
            );
            hits.write(crate::fight::Hit {
                attacker: fire.caster,
                target,
                share: 1.0,
                ranged: false,
                art: None,
                fixed: Some(fire.damages),
                sure: true,
                poison: false,
            });
        }
        commands.entity(entity).despawn();
    }
}

/// Ends each cast whose clip has ended (`0x51D150`'s sub-state 4), the creature taking its melee
/// up again from its start.
fn end(
    mut commands: Commands,
    mut ended: Query<(Entity, &Casting, &mut Mind), Without<ScriptClip>>,
) {
    for (entity, cast, mut mind) in &mut ended {
        if cast.begun {
            commands.entity(entity).remove::<Casting>();
            mind.cast_ends();
        }
    }
}

/// A damage times a factor, a float as the strikes' damages are.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a spell's damage, far within the floats' range"
)]
fn scaled(damage: f32, factor: f64) -> f32 {
    (f64::from(damage) * factor) as f32
}

/// The creatures' casts.
pub struct CreatureCastsPlugin;

impl Plugin for CreatureCastsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            ((begin, act, end).chain(), home, throw).in_set(Playing),
        );
    }
}
