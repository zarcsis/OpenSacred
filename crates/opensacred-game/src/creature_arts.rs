//! The heroes' weapon arts that the fight's specials order creatures to make (#2564; docs/re/ai.md,
//! "The Underworld's bosses' specials"): the ice elf champion's `Attacke` and `Hardhit` of the
//! Dark Elf (1067, 1066), the hell golems' `Attacke` of the Gladiator (1059) and the queens'
//! of the mark `0x217` (1067).
//!
//! The order (the AI's state 11, `0x4FD0F0`) takes the action of the arts of the hero whose
//! executor the creature's type has (`opensacred_rules::casts::executor`, `0x5500D0`): its clip
//! is the slot of that class's art for what the creature holds (`opensacred_rules::arts::action`),
//! of the creature's own model (its `*_SPECIAL_MULTI_*` and `*_SPECIAL_HHIT_*`), played once at
//! its attack speed; its level in play `⌊L/5⌋ + 1`, times the balance's factor for a boss or a
//! champion (`0x580A10`), and its strikes carry the art's factor at that level, halved for two
//! blades (`0x4EA3F0`). An `Attacke` strikes its target at each of its clip's hit points while
//! the target stands within the two creatures' reach and 45; a `Hardhit` once, at its clip's hit
//! point or its half, whatever the distance (docs/re/arts.md, "The arts in a strike").
//!
//! The minimummy bosses' daggers (#2575, the Gladiator's hurled blades 1013 of a creature): a
//! volley of the dagger in its hand and 2 to 4 more ([`Volley`]), each thrown a while after it
//! takes it, flying on the arc of a creature's throw, which strikes its target alone, a player,
//! and is gone; a fresh dagger in its hand as each strikes or ends, while the volley lasts
//! (`0x4D93C1`, the runner `0x4FE870`).

use bevy::prelude::*;
use opensacred_rules::arts::{self, ArtClip, Style};
use opensacred_rules::{casts, minds};

use crate::ai::Mind;
use crate::fight::{self, Health};
use crate::npcs::{self, ArtHit, ScriptClip};

/// How far beyond the two creatures' reach an `Attacke` strikes its target (`0x5370B0` against
/// `0x4DBC60` and 45).
const ATTACKE_BEYOND: f32 = 45.0;

/// A creature ordered to make the weapon art `art` at `target` (the AI's state 11), which
/// [`start`] takes up.
#[derive(Component, Debug)]
pub struct Ordered {
    pub art: u16,
    pub target: Entity,
}

/// A creature making a weapon art: its target, its style, its level in play and the factor its
/// strikes carry, while its clip plays ([`ScriptClip::art`]).
#[derive(Component, Debug)]
pub struct Makes {
    target: Entity,
    style: Style,
    level: u32,
    factor: f32,
}

/// Starts each order: the class's slot of the art for what the creature holds, played once; an
/// art the game cannot make so (a shot with a bow, a type of no executor) is dropped.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's query takes its components one by one"
)]
fn start(
    mut commands: Commands,
    ordered: Query<(
        Entity,
        &Ordered,
        &Mind,
        Option<&fight::Ranked>,
        Option<&npcs::Arms>,
    )>,
    (arts_tables, rules): (Option<Res<crate::arts::HeroArts>>, Res<fight::Rules>),
) {
    for (entity, order, mind, rank, arms) in &ordered {
        commands.entity(entity).remove::<Ordered>();
        let (Some(kind), Some(style)) = (casts::executor(mind.creature), arts::style(order.art))
        else {
            continue;
        };
        let class = arms.map_or(0, npcs::Arms::class);
        let Some(action) = arts::action(kind, style, class) else {
            continue;
        };
        let slot = match action.clip {
            ArtClip::Slot(slot) => slot,
            ArtClip::Attack => crate::motions::Slots::attack_slot(class),
        };
        let rank = rank.map_or(fight::Rank::Common, |rank| rank.0);
        let level = rules.creature_spell_level(mind.level, rank);
        let factor = arts_tables
            .as_deref()
            .and_then(|tables| {
                tables
                    .tables
                    .weapon_arts
                    .iter()
                    .find(|record| record.id == order.art)
            })
            .map_or(1.0, |record| {
                arts::held_factor(arts::factor(record, level), class == 8)
            });
        debug!(
            "creature_arts: {entity} makes {} at {} (the slot {slot}, the level {level})",
            order.art, order.target
        );
        commands
            .entity(entity)
            .remove::<(npcs::Walk, npcs::Strikes)>()
            .insert((
                Makes {
                    target: order.target,
                    style,
                    level,
                    factor,
                },
                ScriptClip::art(slot, style == Style::Hardhit),
            ));
    }
}

/// Each hit point of a creature's art's clip ([`ArtHit`]): an `Attacke` strikes its target
/// within the two creatures' reach and 45, a `Hardhit` at its first, whatever the distance.
fn strike(
    mut points: MessageReader<ArtHit>,
    makers: Query<(&Makes, &GlobalTransform, &Mind)>,
    targets: Query<(
        &GlobalTransform,
        &Health,
        Option<&Mind>,
        Option<&crate::hero::Size>,
    )>,
    mut hits: MessageWriter<fight::Hit>,
) {
    for point in points.read() {
        let Ok((makes, at, mind)) = makers.get(point.creature) else {
            continue;
        };
        let Ok((target_at, health, target_mind, size)) = targets.get(makes.target) else {
            continue;
        };
        if health.current == 0 {
            continue;
        }
        let strikes = match makes.style {
            Style::Attacke => {
                let target_size = target_mind
                    .map(|mind| mind.size())
                    .or(size.map(|size| size.0))
                    .unwrap_or(minds::SIZE);
                let reach = minds::reach_f32(mind.size(), target_size) + ATTACKE_BEYOND;
                (target_at.translation() - at.translation()).xz().length() <= reach
            }
            Style::Hardhit => point.n == 0,
            _ => false,
        };
        if strikes {
            debug!(
                "creature_arts: {} strikes {} at its art's hit point {}",
                point.creature, makes.target, point.n
            );
            hits.write(fight::Hit {
                attacker: point.creature,
                target: makes.target,
                share: makes.factor,
                ranged: false,
                art: Some(makes.level),
                fixed: None,
                sure: false,
                poison: false,
                ward: false,
            });
        }
    }
}

/// An art whose clip is over, or gone, is over.
fn end(mut commands: Commands, ended: Query<Entity, (With<Makes>, Without<ScriptClip>)>) {
    for entity in &ended {
        commands.entity(entity).remove::<Makes>();
    }
}

/// How long the stare burns its target's sight (816, `FX_BURNINGLIGHT`): the whole seconds of
/// its starer's AI timer, at least 0.5 (`0x4DA3A0`; **estimate** of 2 for the timer's).
const STARE_BURNS: f32 = 2.0;

/// A brain-footer's stare at `target` of the strength `strength` (`0x4D9C2F`, #2567): its beam
/// of its type's variant from its bone 24 to its target's bone of `bone_roll` (the bones' table,
/// else their places), the brain-footer 203's sound, every art of its kinds of the hero's and
/// its combos `10·w` seconds further from ready (`0x562470`), the effect 57 for as long, which
/// freezes their regeneration, and its target's sight burning.
pub(crate) fn stare(
    world: &mut World,
    (starer, target): (Entity, Entity),
    (creature, strength): (u32, f32),
    bone_roll: u32,
) {
    let Some(kind) = casts::stare(creature) else {
        return;
    };
    let seconds = casts::STARE_SECONDS * strength;
    let bones = &crate::beams::BONES;
    let from = std::slice::from_ref(&bones[casts::STARE_BONE]);
    let to_bone = usize::try_from(bone_roll).unwrap_or(0).min(bones.len() - 1);
    let to = std::slice::from_ref(&bones[to_bone]);
    let at = world
        .get::<GlobalTransform>(starer)
        .map(GlobalTransform::translation);
    {
        let mut commands = world.commands();
        crate::beams::lay(
            &mut commands,
            (
                crate::beams::End::Bone(starer, from, 0.0),
                crate::beams::End::Bone(target, to, 0.0),
            ),
            kind.variant,
        );
        crate::holding_sights::burn(&mut commands, target, STARE_BURNS);
        let effect = opensacred_rules::fight::Effect {
            id: casts::FREEZES_REGENERATION,
            whole: kind.whole,
            value: 0.0,
            art: 0,
            level: 0,
        };
        commands
            .entity(target)
            .entry::<crate::effects::Effects>()
            .or_default()
            .and_modify(move |mut effects| effects.add(effect, seconds));
    }
    if creature == 203
        && let Some(at) = at
    {
        world.write_message(crate::sfx::PlayAt {
            sound: crate::sfx::Of::Id(casts::STARE_SOUND),
            point: at,
            rate: (1.0, crate::sfx::Roll::None),
        });
    }
    // The drain is the hero's alone, its arts' and its combos' regeneration.
    if world.get::<crate::hero::Body>(target).is_none() {
        return;
    }
    if let Some(mut arts) = world.get_resource_mut::<crate::arts::HeroArts>() {
        use opensacred_rules::arts::Table;
        for (drains, table) in [
            (kind.spells, Table::Spells),
            (kind.weapon_arts, Table::WeaponArts),
        ] {
            if drains {
                arts.arts.start(seconds, table, None, 0);
            }
        }
    }
    if kind.combos
        && let Some(mut combos) = world.get_resource_mut::<crate::combos::HeroCombos>()
    {
        opensacred_rules::combos::used(&mut combos.0, None, seconds);
    }
}

/// The seconds of a dryad's leap's clip where its model has none (`DRF1_ATTACK_1H_A`'s, an
/// **estimate** for another).
const LEAP_CLIP: f32 = 1.22;
/// How far past the share of its flight a dryad's leap is along its way (`0x44DFE0`'s last of
/// its six steps, `s = 1.0667·u`).
const LEAP_AHEAD: f32 = 16.0 / 15.0;

/// A dryad's leap by the art 1003 (`0x50AC00`, #2566): its clip of the slot 84 at the jumps' rate
/// by the distance (`arts::jump_rate`); once less than 0.8 of it is left, its flight from where
/// it stands to the point over a quarter of the clip's time, along the line, `d/12` high at its
/// middle (`0x44DED0`, `0x44DFE0`), where it lands; once less than 0.6 is left, its art over and
/// its mark `0x210`, whose bow it takes (`0x50B52F`). It strikes nothing.
#[derive(Component, Debug)]
pub struct Leaps {
    to: Vec3,
    rate: f32,
    height: f32,
    flight: Option<(Vec3, f32, f32)>,
    over: bool,
}

impl Leaps {
    /// The leap from `here` to `to`, and its clip.
    pub fn start(here: Vec3, to: Vec3) -> (Self, ScriptClip) {
        let d = (to - here).xz().length();
        let rate = arts::jump_rate(whole_up(d));
        (
            Self {
                to,
                rate,
                height: d / 12.0,
                flight: None,
                over: false,
            },
            ScriptClip::leap(casts::LEAP_SLOT, rate, casts::LEAP_FLIES),
        )
    }
}

/// A distance on the ground rounded up to a whole number, as the leap's rate takes it.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a distance on the ground, whole and positive, far below the integers' range"
)]
fn whole_up(d: f32) -> u32 {
    d.ceil() as u32
}

/// Each leap a frame on: its flight begun as its clip passes its share, its way and height, its
/// mark `0x210` as its art is over, and on the point once its flight's time is past.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn leaps(
    mut commands: Commands,
    (time, speed, heights, clips): (
        Res<Time>,
        Res<crate::hero::GameSpeed>,
        Res<crate::terrain::Heights>,
        Res<Assets<AnimationClip>>,
    ),
    mut leapers: Query<(
        Entity,
        &mut Leaps,
        &mut Transform,
        &mut Mind,
        Option<&ScriptClip>,
        Has<npcs::ClipActs>,
        Option<&crate::crowd::InTheWay>,
    )>,
) {
    for (entity, mut leap, mut transform, mut mind, clip, acts, way) in &mut leapers {
        if leap.flight.is_none() {
            if !acts {
                continue;
            }
            commands.entity(entity).remove::<npcs::ClipActs>();
            let length = clip
                .and_then(|clip| clip.length(&clips))
                .unwrap_or(LEAP_CLIP);
            let flight = length / (speed.0 * leap.rate).max(f32::EPSILON) / 4.0;
            leap.flight = Some((transform.translation, flight, 0.0));
            debug!("creature_arts: {entity} leaps off for {flight:.2} s");
        }
        let Some((from, flight, flown)) = leap.flight else {
            continue;
        };
        let flown = flown + time.delta_secs();
        leap.flight = Some((from, flight, flown));
        // Its art over at less than 0.6 of its clip left, 0.8 of its flight's time in.
        if !leap.over && flown >= 0.8 * flight {
            leap.over = true;
            mind.phase = Some(casts::DRYAD_LANDED);
        }
        let level = way.and_then(|way| way.level);
        if flown >= flight {
            let at = leap.to;
            let ground = heights.standing(at.x, at.z, level).unwrap_or(at.y);
            transform.translation = Vec3::new(at.x, ground, at.z);
            mind.phase = Some(casts::DRYAD_LANDED);
            commands.entity(entity).remove::<Leaps>();
            debug!("creature_arts: {entity} lands at {}", transform.translation);
            continue;
        }
        let share = (LEAP_AHEAD * flown / flight).min(1.0);
        let along = from.xz() + (leap.to.xz() - from.xz()) * share;
        let ground = heights
            .standing(along.x, along.y, level)
            .unwrap_or(transform.translation.y);
        let rise = leap.height * (1.0 - 4.0 * (share - 0.5).powi(2));
        transform.translation = Vec3::new(
            along.x,
            ground + crate::iso::height(rise * crate::iso::PIXELS_PER_MODEL_HEIGHT),
            along.y,
        );
    }
}

/// A minimummy's volley of daggers at `target` (#2575): the throws left after the one in its hand,
/// the seconds until it throws that one, and whether a dagger it threw still flies.
#[derive(Component, Debug)]
pub struct Volley {
    target: Entity,
    more: u8,
    wait: f32,
    flying: bool,
}

impl Volley {
    /// The volley of the dagger in its hand at `target`, and `more` after it.
    pub fn new(target: Entity, more: u8) -> Self {
        Self {
            target,
            more,
            wait: casts::DAGGER_THROWN_AFTER,
            flying: false,
        }
    }
}

/// A creature's thrown dagger in flight: its arc's ends on the ground, its height, its time and
/// the time flown, the level it flies on, its thrower and the one it strikes, the art's level and
/// factor, its damages' terms and its size.
#[derive(Component)]
struct Thrown {
    from: Vec2,
    to: Vec2,
    height: f32,
    time: f32,
    flown: f32,
    level: crate::terrain::Level,
    thrower: Entity,
    target: Entity,
    art: (u32, f32),
    damage: [[f32; 2]; opensacred_rules::fight::KINDS],
    times: f64,
    size: f32,
}

/// The dice of the thrown daggers' damages.
#[derive(Resource)]
struct DaggerDice(crate::spawning::Rng);

/// Each volley a frame on: its dagger thrown once its wait is over and its hand holds one, the
/// creature turned to its target; a volley whose target or hand is gone ends.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn volleys(
    mut commands: Commands,
    time: Res<Time>,
    (npcs, rules, arts_tables): (
        Res<npcs::Npcs>,
        Res<fight::Rules>,
        Option<Res<crate::arts::HeroArts>>,
    ),
    assets: Res<AssetServer>,
    mut throwers: Query<(
        Entity,
        &mut Volley,
        &mut Transform,
        &Mind,
        &npcs::Arms,
        &fight::Combat,
        Option<&fight::Ranked>,
        Option<&crate::crowd::InTheWay>,
    )>,
    targets: Query<&GlobalTransform, With<crate::hero::Body>>,
) {
    for (entity, mut volley, mut transform, mind, arms, combat, rank, way) in &mut throwers {
        if volley.flying {
            continue;
        }
        volley.wait -= time.delta_secs();
        if volley.wait > 0.0 {
            continue;
        }
        let (Ok(target_at), Some((dagger, values))) = (targets.get(volley.target), arms.right())
        else {
            commands.entity(entity).remove::<Volley>();
            continue;
        };
        let here = transform.translation;
        let at = target_at.translation();
        transform.rotation = crate::hero::facing(at - here);
        // The art's level in play and its factor at it, halved for two blades (`0x4EA3F0`).
        let class = arms.class();
        let rank = rank.map_or(fight::Rank::Common, |rank| rank.0);
        let level = rules.creature_spell_level(mind.level, rank);
        let factor = arts_tables
            .as_deref()
            .and_then(|tables| {
                tables
                    .tables
                    .weapon_arts
                    .iter()
                    .find(|record| record.id == casts::HURLED_BLADES)
            })
            .map_or(1.0, |record| {
                arts::held_factor(arts::factor(record, level), class == 8)
            });
        let (to, d) = arts::throw_end(here.xz().into(), at.xz().into(), false);
        let (model, texture, size) = npcs.held_model(dagger).map_or(
            (None, None, crate::arrows::size(0)),
            |(model, texture, size)| (Some(model.to_owned()), texture.map(str::to_owned), size),
        );
        debug!(
            "creature_arts: {entity} throws its dagger at {}, {} more",
            volley.target, volley.more
        );
        // Its own damages, weighed by its skills and its strength (`0x56ACD0`, as the hero's).
        let times = rules.glance_factor(&arms.skills(mind.level), combat.attributes[0]);
        let mut flight = commands.spawn((
            Transform::from_translation(here).with_scale(Vec3::splat(crate::iso::MODEL_SCALE)),
            Thrown {
                from: here.xz(),
                to: to.into(),
                height: arts::throw_height(false),
                time: arts::throw_time(d),
                flown: 0.0,
                level: way.and_then(|way| way.level),
                thrower: entity,
                target: volley.target,
                art: (level, factor),
                damage: values.map_or([[0.0; 2]; opensacred_rules::fight::KINDS], |values| {
                    values.damage
                }),
                times,
                size,
            },
            DespawnOnExit(crate::game::AppState::InGame),
        ));
        if let Some(model) = model {
            flight.insert((
                crate::models::scene(&assets, &model),
                crate::models::Ordered::default(),
            ));
        }
        if let Some(texture) = texture {
            flight.insert(npcs::Retexture::load(&assets, &texture));
        }
        commands.entity(entity).insert(npcs::Rearms::throws());
        volley.flying = true;
    }
}

/// Flies each thrown dagger on its arc a frame on: it strikes its target, the hero, once it
/// touches it by the sum of their sizes (`0x600340`; the item's `+0xB0` bit 8, players alone),
/// and is gone there or at its arc's end; its thrower then takes a fresh dagger while its volley
/// lasts (`0x4FE870`), else its volley ends.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn daggers(
    mut commands: Commands,
    time: Res<Time>,
    heights: Res<crate::terrain::Heights>,
    mut dice: ResMut<DaggerDice>,
    mut hits: MessageWriter<fight::Hit>,
    mut flying: Query<(Entity, &mut Transform, &mut Thrown)>,
    mut volleys: Query<&mut Volley>,
    targets: Query<
        (&GlobalTransform, &Health, Option<&crate::hero::Size>),
        (With<crate::hero::Body>, Without<Thrown>),
    >,
) {
    for (entity, mut transform, mut flight) in &mut flying {
        flight.flown += time.delta_secs();
        let share = flight.flown / flight.time.max(f32::EPSILON);
        let (at, rise) =
            arts::throw_point(flight.from.into(), flight.to.into(), share, flight.height);
        let at = Vec2::from(at);
        let ground = heights
            .standing(at.x, at.y, flight.level)
            .unwrap_or(transform.translation.y);
        transform.translation = Vec3::new(
            at.x,
            ground + crate::iso::height(rise * crate::iso::PIXELS_PER_MODEL_HEIGHT),
            at.y,
        );
        let struck = targets
            .get(flight.target)
            .ok()
            .filter(|(place, health, size)| {
                let size = size.map_or(minds::SIZE, |size| size.0);
                health.current > 0
                    && (place.translation().xz() - at).length()
                        <= flight.size + crate::arrows::size(size)
            })
            .is_some();
        if struck {
            let fixed = arts::thrown_damage(&flight.damage, flight.times, &mut dice.0);
            debug!(
                "creature_arts: {}'s dagger strikes {}, {:?}",
                flight.thrower,
                flight.target,
                fixed.map(|d| d.round())
            );
            hits.write(fight::Hit {
                attacker: flight.thrower,
                target: flight.target,
                share: flight.art.1,
                ranged: false,
                art: Some(flight.art.0),
                fixed: Some(fixed),
                sure: true,
                poison: false,
                ward: false,
            });
        } else if share < 1.0 {
            continue;
        }
        commands.entity(entity).despawn();
        if let Ok(mut volley) = volleys.get_mut(flight.thrower) {
            if volley.more == 0 {
                commands.entity(flight.thrower).remove::<Volley>();
            } else {
                volley.more -= 1;
                volley.wait = casts::DAGGER_THROWN_AFTER;
                volley.flying = false;
                commands
                    .entity(flight.thrower)
                    .insert(npcs::Rearms::dagger());
            }
        }
    }
}

/// The creatures' weapon arts.
pub struct CreatureArtsPlugin;

impl Plugin for CreatureArtsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DaggerDice(crate::seed::dice(2575)))
            .add_systems(
                Update,
                (start, strike, end)
                    .chain()
                    .run_if(in_state(crate::game::AppState::InGame)),
            )
            .add_systems(
                Update,
                (volleys, daggers, leaps)
                    .chain()
                    .after(crate::frame::Step::Npcs)
                    .in_set(crate::frame::Playing)
                    .in_set(crate::frame::Unpaused),
            );
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::time::TimeUpdateStrategy;

    use super::*;

    #[derive(Resource, Default)]
    struct Struck(Vec<fight::Hit>);

    fn collect(mut hits: MessageReader<fight::Hit>, mut struck: ResMut<Struck>) {
        struck.0.extend(hits.read().cloned());
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                50,
            )))
            .insert_resource(crate::terrain::Heights::default())
            .insert_resource(DaggerDice(crate::spawning::Rng::of(1, 2, 3)))
            .init_resource::<Struck>()
            .add_message::<fight::Hit>()
            .add_systems(Update, (daggers, collect).chain());
        app
    }

    fn thrown(thrower: Entity, target: Entity) -> Thrown {
        Thrown {
            from: Vec2::ZERO,
            to: Vec2::new(750.0, 0.0),
            height: 55.0,
            time: 0.3,
            flown: 0.0,
            level: None,
            thrower,
            target,
            art: (3, 1.5),
            damage: [[2.0, 4.0], [0.0; 2], [0.0; 2], [0.0; 2]],
            times: 1.0,
            size: 50.0,
        }
    }

    #[test]
    fn a_dryad_leaps_off_flies_high_and_lands_on_its_point_of_the_mark_0x210() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                50,
            )))
            .insert_resource(crate::hero::GameSpeed(1.0))
            .insert_resource(crate::terrain::Heights::default())
            .insert_resource(Assets::<AnimationClip>::default())
            .add_systems(Update, leaps);
        let mind = Mind::new(
            Vec3::ZERO,
            0x1,
            (false, false),
            None,
            [10.0, 20.0],
            (218, 2),
        );
        let to = Vec3::new(400.0, 0.0, 0.0);
        let (leap, _) = Leaps::start(Vec3::ZERO, to);
        // 400 off: the rate 1, so a quarter of the clip's 1.22 s, 0.305 s, `d/12` high at most.
        assert_eq!((leap.rate, leap.height), (1.0, 400.0 / 12.0));
        let dryad = app
            .world_mut()
            .spawn((Transform::default(), mind, leap))
            .id();
        // Nothing before its clip passes its share.
        app.update();
        app.update();
        assert_eq!(
            app.world().get::<Transform>(dryad).unwrap().translation,
            Vec3::ZERO
        );
        app.world_mut().entity_mut(dryad).insert(npcs::ClipActs);
        app.update();
        // 0.15 s in, the frame it took off its first: along the way past its share, aloft, its
        // mark still `0x20E`.
        app.update();
        app.update();
        let at = app.world().get::<Transform>(dryad).unwrap().translation;
        let share = 16.0 / 15.0 * 0.15 / 0.305;
        assert!((at.x - 400.0 * share).abs() < 1.0, "{at}");
        assert!(at.y > 0.0, "{at}");
        assert_eq!(app.world().get::<Mind>(dryad).unwrap().mark(), 0x20E);
        // 0.3 s in, past 0.8 of its flight: its art is over, its mark `0x210`.
        for _ in 0..3 {
            app.update();
        }
        assert!(app.world().get::<Leaps>(dryad).is_some());
        assert_eq!(
            app.world().get::<Mind>(dryad).unwrap().mark(),
            casts::DRYAD_LANDED
        );
        // Past its flight's time, on its point, on the ground.
        app.update();
        app.update();
        assert!(app.world().get::<Leaps>(dryad).is_none());
        assert_eq!(app.world().get::<Transform>(dryad).unwrap().translation, to);
    }

    #[test]
    fn a_thrown_dagger_strikes_its_target_and_its_thrower_takes_another_while_its_volley_lasts() {
        let mut app = app();
        let hero = app
            .world_mut()
            .spawn((
                crate::hero::Body::standing(),
                GlobalTransform::from_translation(Vec3::new(300.0, 0.0, 0.0)),
                Health {
                    current: 100,
                    most: 100,
                    level: 1,
                },
            ))
            .id();
        let thrower = app.world_mut().spawn(Volley::new(hero, 1)).id();
        app.world_mut().get_mut::<Volley>(thrower).unwrap().flying = true;
        let dagger = app
            .world_mut()
            .spawn((Transform::default(), thrown(thrower, hero)))
            .id();
        // The first update's time is none; then 50 ms a frame along an arc of 0.3 s, which
        // reaches the hero 300 off by the sum of the sizes, 100, at a fifth of it.
        for _ in 0..4 {
            app.update();
        }
        let struck = &app.world().resource::<Struck>().0;
        assert_eq!(struck.len(), 1, "{struck:?}");
        assert_eq!((struck[0].attacker, struck[0].target), (thrower, hero));
        assert_eq!((struck[0].art, struck[0].share), (Some(3), 1.5));
        let fixed = struck[0].fixed.unwrap();
        assert!((2.0..=4.0).contains(&fixed[0]), "{fixed:?}");
        assert!(app.world().get_entity(dagger).is_err());
        // A fresh dagger in its hand, and one more to throw after it.
        let volley = app.world().get::<Volley>(thrower).unwrap();
        assert!(!volley.flying && volley.more == 0);
        assert!((volley.wait - casts::DAGGER_THROWN_AFTER).abs() < 1e-6);
        assert_eq!(
            app.world().get::<npcs::Rearms>(thrower),
            Some(&npcs::Rearms::dagger())
        );
        // The last flies off past a hero that is gone, and the volley ends at its arc's end.
        app.world_mut().entity_mut(thrower).remove::<npcs::Rearms>();
        app.world_mut().get_mut::<Volley>(thrower).unwrap().flying = true;
        app.world_mut().entity_mut(hero).despawn();
        let last = app
            .world_mut()
            .spawn((Transform::default(), thrown(thrower, hero)))
            .id();
        for _ in 0..6 {
            app.update();
        }
        assert!(app.world().get_entity(last).is_err());
        assert!(app.world().get::<Volley>(thrower).is_none());
        assert!(app.world().get::<npcs::Rearms>(thrower).is_none());
        assert_eq!(app.world().resource::<Struck>().0.len(), 1);
    }
}
