//! The heroes' weapon arts that the fight's specials order creatures to make (#2564; docs/re/ai.md,
//! "The Underworld's bosses' specials"): the ice elf champion's `Attacke` and `Hardhit` of the
//! Dark Elf (1067, 1066), the hell golems' `Attacke` of the Gladiator (1059), the queens' of the
//! mark `0x217` (1067), and the ice druid's BFG and energy bolts of the Seraphim (1018, 1019).
//!
//! The ice druid's arts (#2899, [`give`], [`at_range`]): its own clip, the slot 154, at the rate
//! 1.0, the BFG's charge at its hand as it starts, and at the art's point the effect 16 or 17 of
//! its level in play on itself, the energy bolts' sights at its hands with 17's, and the art
//! ends; while either effect holds it fights at range as an archer, its shots its bolts
//! (`npcs::Strikes::bolts`).
//!
//! The order (the AI's state 11, `0x4FD0F0`) takes the action of the arts of the hero whose
//! executor the creature's type has (`opensacred_rules::casts::executor`, `0x5500D0`): its clip
//! is the slot of that class's art for what the creature holds (`opensacred_rules::arts::action`),
//! the hell golems' and the minimummies' `Attacke` their own (`casts::attacke_slot`, #3428),
//! of the creature's own model (its `*_SPECIAL_MULTI_*` and `*_SPECIAL_HHIT_*`), played once at
//! its attack speed; its level in play `⌊L/5⌋ + 1`, times the balance's factor for a boss or a
//! champion (`0x580A10`), and its strikes carry the art's factor at that level, halved for two
//! blades (`0x4EA3F0`). An `Attacke` strikes its target at each of its clip's hit points while
//! the target stands within the two creatures' reach and 45; a `Hardhit` once, at its clip's hit
//! point or its half, whatever the distance (docs/re/arts.md, "An art in a strike").
//!
//! The minimummy bosses' daggers (#2575, the Gladiator's hurled blades 1013 of a creature): a
//! volley of the dagger in its hand and 2 to 4 more ([`Volley`]), each thrown a while after it
//! takes it, flying on the arc of a creature's throw, which strikes its target alone, a player,
//! and is gone; a fresh dagger in its hand as each strikes or ends, while the volley lasts
//! (`0x4D93C1`, the runner `0x4FE870`).
//!
//! The brain-footers' stare (#2567, [`stare`]): its beam and its sound, the hero's arts and combos
//! further from ready and their regeneration frozen, and its target's sight burning. A dryad's
//! leap by the art 1003 (#2566, [`Leaps`]): its clip, its flight to its point and its mark as its
//! art is over; it strikes nothing.

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
#[derive(Component, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Ordered {
    pub art: u16,
    #[serde(with = "crate::names::required")]
    pub target: Entity,
}

impl bevy::ecs::entity::MapEntities for Ordered {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.target = mapper.get_mapped(self.target);
    }
}

/// A creature's order to make an art, which a save keeps with its target by its name (#3766), a
/// target that the load finds not none's, as a target gone in play.
impl crate::doings::Saves for Ordered {
    type Saved = Ordered;

    fn saved(&self, _: &crate::doings::Moment) -> Option<Self> {
        Some(*self)
    }

    fn again(saved: Self, _: &crate::doings::Moment) -> Option<Self> {
        Some(saved)
    }
}

/// A creature making a weapon art: its target, its style, its level in play and the factor its
/// strikes carry, while its clip plays ([`ScriptClip::art`]), and what a boon gives it at its
/// point ([`ScriptClip::boon`]).
#[derive(Component, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Makes {
    #[serde(with = "crate::names::required")]
    target: Entity,
    style: Style,
    level: u32,
    factor: f32,
    boon: Option<(u16, arts::Boon)>,
}

impl bevy::ecs::entity::MapEntities for Makes {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.target = mapper.get_mapped(self.target);
    }
}

/// A creature's art in course, which a save keeps with its target by its name (#3766), a target
/// that the load finds not none's, as a target gone in play; its clip, the scripts' clip, plays
/// anew at a load ([`crate::npcs::SavedScriptClip`]).
impl crate::doings::Saves for Makes {
    type Saved = Makes;

    fn saved(&self, _: &crate::doings::Moment) -> Option<Self> {
        Some(self.clone())
    }

    fn again(saved: Self, _: &crate::doings::Moment) -> Option<Self> {
        Some(saved)
    }
}

/// Starts each order: the class's slot of the art for what the creature holds, played once; an
/// art the game cannot make so (a shot with a bow, a type of no executor, a boon that takes
/// effects off or gives them as it starts) is dropped. A boon's clip plays at its action's rate
/// where that is fixed, the BFG's charge showing at the creature's hand as it starts.
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
    (arts_tables, rules, mut dice): (
        Option<Res<crate::arts::HeroArts>>,
        Res<fight::Rules>,
        ResMut<ClipDice>,
    ),
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
        // The ice druid's own clip of the Seraphim's arts (`0x506017`, `0x506137`), and the hell
        // golems' and the minimummies' of the Gladiator's `Attacke` (`0x502667`, #3428).
        let slot = casts::ice_druids_slot(mind.creature, order.art)
            .or_else(|| casts::attacke_slot(mind.creature, class, order.art, &mut dice.0))
            .unwrap_or(slot);
        let rank = rank.map_or(fight::Rank::Common, |rank| rank.0);
        let level = rules.creature_spell_level(mind.level, rank);
        let record = arts_tables.as_deref().and_then(|tables| {
            tables
                .tables
                .weapon_arts
                .iter()
                .find(|record| record.id == order.art)
        });
        let factor = record.map_or(1.0, |record| {
            arts::held_factor(arts::factor(record, level), class == 8)
        });
        // The Seraphim's 1018 and 1019 give their effects at their point (`0x5038F0`), as the
        // hero's boons do.
        let boon = match style {
            Style::Boon | Style::Bolts => {
                let Some(boon) = record
                    .and_then(|record| arts::boon(record, level, 1.0))
                    .filter(|b| {
                        b.clears.is_empty() && b.starts.is_empty() && b.replaces.is_empty()
                    })
                else {
                    continue;
                };
                Some((order.art, boon))
            }
            _ => None,
        };
        debug!(
            "creature_arts: {entity} makes {} at {} (the slot {slot}, the level {level})",
            order.art, order.target
        );
        let clip = match &boon {
            Some((_, boon)) => {
                ScriptClip::boon(slot, action.fixed.then_some(action.rate), boon.point)
            }
            None => ScriptClip::art(slot, style == Style::Hardhit),
        };
        if order.art == casts::ICE_DRUIDS_BFG {
            crate::sera_sights::charge_on(&mut commands, entity);
        }
        commands
            .entity(entity)
            .remove::<(npcs::Walk, npcs::Strikes)>()
            .insert((
                Makes {
                    target: order.target,
                    style,
                    level,
                    factor,
                    boon,
                },
                clip,
            ));
        // The weapons' streaks of the arts that its class's executor streaks, while it makes the
        // art (#3454).
        crate::weapon_streaks::lay(
            &mut commands,
            entity,
            (order.art, kind, false),
            crate::weapon_streaks::Lasts::WhileMaking,
        );
    }
}

/// Gives each creature whose boon's clip has passed its point the boon's effects, on itself
/// (`0x582D50`), the energy bolts' sights at its hands with 1019's; the art ends with them where
/// it does so, the Seraphim's 1018 and 1019 (`0x5038F0`, #2899).
fn give(mut commands: Commands, makers: Query<(Entity, &Makes), With<npcs::ClipActs>>) {
    for (entity, makes) in &makers {
        let Some((art, boon)) = &makes.boon else {
            continue;
        };
        debug!("creature_arts: {entity} takes {:?} of {art}", boon.effects);
        let effects = boon.effects.clone();
        commands
            .entity(entity)
            .remove::<npcs::ClipActs>()
            .entry::<crate::effects::Effects>()
            .or_default()
            .and_modify(move |mut list| {
                for (effect, seconds) in effects {
                    list.add(effect, seconds);
                }
            });
        if *art == casts::ICE_DRUIDS_BOLTS
            && let Some(&(_, seconds)) = boon.effects.first()
        {
            crate::sera_sights::bolts(&mut commands, entity, seconds);
        }
        if boon.ends {
            commands.entity(entity).remove::<(Makes, ScriptClip)>();
        }
    }
}

/// Makes each creature that holds the Seraphim's effect 16 or 17 fight at range as one with a
/// ranged weapon does, its shots its bolts (`ai`, `npcs::Strikes::bolts`), and once neither
/// holds, as its weapons have it (the AI's states 4 and 8 by `+0x200`'s bits `0x600000`,
/// `0x4E03C1` and `0x4DC146`, #2899).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's query takes its components one by one"
)]
fn at_range(
    mut commands: Commands,
    mut creatures: Query<(
        Entity,
        &mut Mind,
        Option<&crate::effects::Effects>,
        Option<&npcs::Arms>,
        Has<Bolted>,
    )>,
) {
    for (entity, mut mind, effects, arms, bolted) in &mut creatures {
        let bolts =
            effects.is_some_and(|effects| effects.holds(arts::BFG) || effects.holds(arts::RAILGUN));
        if bolts {
            if !bolted {
                commands.entity(entity).insert(Bolted);
            }
            if !mind.ranged {
                mind.ranged = true;
            }
        } else if bolted {
            commands.entity(entity).remove::<Bolted>();
            mind.ranged = arms.is_some_and(npcs::Arms::ranged);
        }
    }
}

/// A creature that fights at range by the Seraphim's effect 16 or 17 ([`at_range`]).
#[derive(Component, Debug)]
struct Bolted;

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
                hurled: false,
                object: false,
                mount: false,
                marks: true,
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
            // On past a cell that does not pass (`0x4D4E50`).
            kind.variant | crate::beams::THROUGH,
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
            factor: 1.0,
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

/// A dryad's leap by the art 1003 (`0x50AC00`, #2566): its clip of the slot 84 at the jumps' rate
/// by the distance (`arts::jump_rate`); once less than 0.8 of it is left, its flight from where
/// it stands to the point as a hero's jump flies (`making::Leap`: a quarter of the clip's time,
/// `d/12` high at its middle, `0x44DED0`, `0x44DFE0`), where it lands; once less than 0.6 is
/// left, its art over and its mark `0x210`, whose bow it takes (`0x50B52F`). It strikes nothing.
#[derive(Component, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Leaps {
    to: Vec3,
    rate: f32,
    /// Once it flies, the seconds until its art is over.
    over: Option<f32>,
}

impl bevy::ecs::entity::MapEntities for Leaps {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, _: &mut E) {}
}

/// A creature's leap of an art, which a save keeps (#3766).
impl crate::doings::Saves for Leaps {
    type Saved = Leaps;

    fn saved(&self, _: &crate::doings::Moment) -> Option<Self> {
        Some(*self)
    }

    fn again(saved: Self, _: &crate::doings::Moment) -> Option<Self> {
        Some(saved)
    }
}

impl Leaps {
    /// The leap from `here` to `to`, and its clip.
    pub fn start(here: Vec3, to: Vec3) -> (Self, ScriptClip) {
        let rate = arts::jump_rate(whole_up((to - here).xz().length()));
        (
            Self {
                to,
                rate,
                over: None,
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

/// Each leap a frame on: its flight begun as its clip passes its share, its mark `0x210` as its
/// art is over, and the leap done once it has landed.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn leaps(
    mut commands: Commands,
    (time, speed, clips): (
        Res<Time>,
        Res<crate::hero::GameSpeed>,
        Res<Assets<AnimationClip>>,
    ),
    mut leapers: Query<(
        Entity,
        &mut Leaps,
        &Transform,
        &mut Mind,
        Option<&ScriptClip>,
        Has<npcs::ClipActs>,
        Has<crate::making::Leap>,
    )>,
) {
    for (entity, mut leap, transform, mut mind, clip, acts, flying) in &mut leapers {
        let Some(over) = leap.over else {
            if !acts {
                continue;
            }
            commands.entity(entity).remove::<npcs::ClipActs>();
            let played = clip
                .and_then(|clip| clip.length(&clips))
                .unwrap_or(LEAP_CLIP)
                / (speed.0 * leap.rate).max(f32::EPSILON);
            let here = transform.translation;
            let to = Vec3::new(leap.to.x, here.y, leap.to.z);
            let time = played * arts::JUMP_FLIGHT;
            commands.entity(entity).insert(crate::making::Leap {
                from: here,
                to,
                time,
                flown: 0.0,
                peak: arts::jump_height((to - here).xz().length(), 0.5),
                stops: arts::flight_stops(mind.thinks.creature),
            });
            leap.over = Some(played * (casts::LEAP_ENDS - casts::LEAP_FLIES));
            debug!("creature_arts: {entity} leaps off for {time:.2} s");
            continue;
        };
        let over = over - time.delta_secs();
        leap.over = Some(over);
        if over <= 0.0 {
            mind.phase = Some(casts::DRYAD_LANDED);
            if !flying {
                commands.entity(entity).remove::<Leaps>();
                debug!("creature_arts: {entity} lands at {}", transform.translation);
            }
        }
    }
}

/// A minimummy's volley of daggers at `target` (#2575): the throws left after the one in its hand,
/// the seconds until it throws that one, and whether a dagger it threw still flies.
#[derive(Component, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Volley {
    #[serde(with = "crate::names::required")]
    target: Entity,
    more: u8,
    wait: f32,
    flying: bool,
}

impl bevy::ecs::entity::MapEntities for Volley {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.target = mapper.get_mapped(self.target);
    }
}

/// A creature's volley in course, which a save keeps with its target by its name (#3766), a
/// target that the load finds not none's, as a target gone in play.
impl crate::doings::Saves for Volley {
    type Saved = Volley;

    fn saved(&self, _: &crate::doings::Moment) -> Option<Self> {
        Some(*self)
    }

    fn again(saved: Self, _: &crate::doings::Moment) -> Option<Self> {
        Some(saved)
    }
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
/// factor, its damages' terms (the dagger's values, its thrower's attributes and type, and the
/// factor) and its size.
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
    values: Option<fight::ItemValues>,
    attributes: [f32; 6],
    thrower_type: u32,
    times: f64,
    size: f32,
}

/// The dice of the thrown daggers' damages.
#[derive(Resource)]
struct DaggerDice(crate::spawning::Rng);

/// The dice of the clips that some types play for an art in place of their class's
/// (`casts::attacke_slot`).
#[derive(Resource)]
struct ClipDice(crate::spawning::Rng);

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
        // Its own damages and its slots', weighed by its skills and its strength (`0x56ACD0`, as
        // the hero's).
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
                values,
                attributes: combat.attributes,
                thrower_type: mind.creature,
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

/// Whether a thrown dagger's hit takes the arts' roll: while its thrower, alive and not knocked,
/// still makes its volley, the Gladiator's action 8 of the types that make 1013 (`0x56BF8E`..
/// `0x56BFCB`; the tick of the action 0 after each throw, `0x4FE870`, which no hit some 0.2 s
/// later meets, left out); none once it is out of the art.
fn dagger_in_art(volley: bool, alive: bool, knocked: bool) -> bool {
    volley && alive && !knocked
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
    throwers: Query<(Option<&Health>, Has<crate::making::Knocked>), Without<crate::hero::Body>>,
    targets: Query<
        (
            &GlobalTransform,
            &Health,
            Option<&crate::hero::Size>,
            Option<&fight::Holding>,
            Option<&crate::riding::Seat>,
        ),
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
            .filter(|(place, health, size, ..)| {
                let size = size.map_or(minds::SIZE, |size| size.0);
                health.current > 0
                    && (place.translation().xz() - at).length()
                        <= flight.size + crate::arrows::size(size)
            });
        if let Some((.., holding, seat)) = struck {
            // Its slots against the hero's class count in the hit (`0x56ACD0`, #3383): the
            // heroes' side, a hero with a ranged weapon in hand, one on a horse.
            let at = opensacred_rules::fight::HurledAt {
                thrower: flight.thrower_type,
                side: minds::HEROES,
                ranged: holding.is_some_and(|holding| holding.0.ranged),
                mounted: seat.and_then(crate::riding::Seat::ridden).is_some(),
            };
            let damage = flight
                .values
                .map_or([[0.0; 2]; opensacred_rules::fight::KINDS], |values| {
                    values.hurled_damage(flight.attributes, Some(at))
                });
            let fixed = arts::thrown_damage(&damage, flight.times, &mut dice.0);
            let in_art = throwers.get(flight.thrower).is_ok_and(|(health, knocked)| {
                dagger_in_art(
                    volleys.contains(flight.thrower),
                    health.is_none_or(|health| health.current > 0),
                    knocked,
                )
            });
            debug!(
                "creature_arts: {}'s dagger strikes {}, {:?}, {}",
                flight.thrower,
                flight.target,
                fixed.map(|d| d.round()),
                if in_art { "the arts' roll" } else { "no roll" }
            );
            hits.write(fight::Hit {
                // The thrower, where the original's event names the first of the local hero's
                // hostiles (`0x55D260`'s query `0x47` at `0x44E487`), a repair
                // (docs/decisions/fight.md, "A creature's thrown dagger struck by its thrower").
                attacker: flight.thrower,
                target: flight.target,
                share: flight.art.1,
                ranged: false,
                art: Some(flight.art.0),
                fixed: Some(fixed),
                // The arts' roll by the thrower's `A` while it makes its volley, none after
                // (`0x56BF8E`..`0x56BFCB`, #3416).
                sure: !in_art,
                poison: false,
                ward: false,
                hurled: true,
                object: false,
                mount: false,
                marks: true,
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
            .insert_resource(ClipDice(crate::seed::dice(3428)))
            .add_systems(
                Update,
                (at_range, start, strike, give, end)
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
            values: Some(fight::ItemValues {
                damage: [[2.0, 4.0], [0.0; 2], [0.0; 2], [0.0; 2]],
                ..fight::ItemValues::default()
            }),
            attributes: [0.0; 6],
            thrower_type: 0,
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
            .add_message::<crate::hit_sparks::Spark>()
            .add_systems(Update, (leaps, crate::making::leap).chain());
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
        // 400 off: the rate 1.
        assert_eq!(leap.rate, 1.0);
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
        // Its flight a quarter of its clip's 1.22 s, `d/12` high at its middle.
        let flight = *app.world().get::<crate::making::Leap>(dryad).unwrap();
        assert!((flight.time - 0.305).abs() < 1e-4, "{flight:?}");
        assert!((flight.peak - 400.0 / 12.0).abs() < 1e-3, "{flight:?}");
        // Aloft, its mark still `0x20E`.
        app.update();
        app.update();
        let at = app.world().get::<Transform>(dryad).unwrap().translation;
        assert!(at.x > 0.0 && at.x < 400.0 && at.y > 0.0, "{at}");
        assert_eq!(app.world().get::<Mind>(dryad).unwrap().mark(), 0x20E);
        // 0.244 s after its flight began, 0.2 of its clip: its art over, its mark `0x210`, still
        // flying.
        for _ in 0..3 {
            app.update();
        }
        assert!(app.world().get::<crate::making::Leap>(dryad).is_some());
        assert_eq!(
            app.world().get::<Mind>(dryad).unwrap().mark(),
            casts::DRYAD_LANDED
        );
        // Landed on its point, on the ground, its leap done.
        for _ in 0..3 {
            app.update();
        }
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
        // Its thrower makes its volley still: the arts' roll.
        assert!(!struck[0].sure);
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

    #[test]
    fn a_dagger_takes_the_arts_roll_while_its_thrower_makes_its_volley_and_none_after() {
        assert!(dagger_in_art(true, true, false));
        assert!(!dagger_in_art(false, true, false));
        assert!(!dagger_in_art(true, false, false));
        assert!(!dagger_in_art(true, true, true));
        // A thrower whose volley is over: the dagger lands without a roll.
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
        let thrower = app.world_mut().spawn_empty().id();
        app.world_mut()
            .spawn((Transform::default(), thrown(thrower, hero)));
        for _ in 0..4 {
            app.update();
        }
        let struck = &app.world().resource::<Struck>().0;
        assert_eq!(struck.len(), 1, "{struck:?}");
        assert_eq!(struck[0].attacker, thrower);
        assert!(struck[0].sure);
    }

    #[test]
    fn the_druids_energy_bolts_give_it_their_effect_at_their_point_and_take_it_to_range() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Update, (at_range, give).chain());
        let effect = opensacred_rules::fight::Effect {
            id: arts::RAILGUN,
            whole: 3,
            value: 0.0,
            art: u32::from(casts::ICE_DRUIDS_BOLTS),
            level: 0,
        };
        let boon = arts::Boon {
            point: 0.65,
            effects: vec![(effect, 31.0)],
            clears: Vec::new(),
            starts: Vec::new(),
            replaces: Vec::new(),
            ends: true,
        };
        let mind = Mind::new(
            Vec3::ZERO,
            0x1,
            (false, false),
            None,
            [10.0, 20.0],
            (141, 10),
        );
        let druid = app
            .world_mut()
            .spawn((
                mind,
                Makes {
                    target: Entity::PLACEHOLDER,
                    style: Style::Bolts,
                    level: 3,
                    factor: 1.0,
                    boon: Some((casts::ICE_DRUIDS_BOLTS, boon)),
                },
                ScriptClip::boon(152, Some(1.0), 0.65),
            ))
            .id();
        // Nothing before its clip passes its point.
        app.update();
        assert!(app.world().get::<crate::effects::Effects>(druid).is_none());
        // Then the effect 17 on itself, and the art ends.
        app.world_mut().entity_mut(druid).insert(npcs::ClipActs);
        app.update();
        let effects = app.world().get::<crate::effects::Effects>(druid).unwrap();
        assert!(effects.holds(arts::RAILGUN));
        assert!(app.world().get::<Makes>(druid).is_none());
        assert!(app.world().get::<ScriptClip>(druid).is_none());
        // While it holds, the druid fights at range, whatever turns it to melee.
        app.update();
        assert!(app.world().get::<Mind>(druid).unwrap().ranged);
        app.world_mut().get_mut::<Mind>(druid).unwrap().ranged = false;
        app.update();
        assert!(app.world().get::<Mind>(druid).unwrap().ranged);
        // Once it is gone, as its weapons have it: none in hand.
        app.world_mut()
            .entity_mut(druid)
            .insert(crate::effects::Effects::default());
        app.update();
        assert!(!app.world().get::<Mind>(druid).unwrap().ranged);
    }

    #[test]
    fn a_creatures_art_saved_strikes_the_creature_of_its_targets_name() {
        use crate::doings::Doing;
        use crate::names::{Id, Loading, Named, Names, NamesPlugin, Refs, Saving, mapped};
        let mut saved = App::new();
        saved.add_plugins(NamesPlugin);
        let hero = saved.world_mut().spawn(Named(Id::Hero)).id();
        let gone = saved.world_mut().spawn_empty().id();
        let boon = arts::Boon {
            point: 0.65,
            effects: vec![(opensacred_rules::fight::Effect::new(17, 1.0), 31.0)],
            clears: Vec::new(),
            starts: Vec::new(),
            replaces: vec![3],
            ends: true,
        };
        let makes = Makes {
            target: hero,
            style: Style::Bolts,
            level: 3,
            factor: 1.5,
            boon: Some((casts::ICE_DRUIDS_BOLTS, boon)),
        };
        let volley = Volley {
            target: gone,
            more: 2,
            wait: 0.4,
            flying: true,
        };
        let druid = saved
            .world_mut()
            .spawn((Named(Id::Born(7)), makes.clone(), volley))
            .id();
        let mut refs = Refs::default();
        let doing = mapped(
            Doing::of(saved.world(), druid).unwrap(),
            &mut Saving {
                world: saved.world(),
                refs: &mut refs,
            },
        );
        let bytes = postcard::to_stdvec(&(&refs, &doing)).unwrap();
        let (refs, doing): (Refs, Doing) = postcard::from_bytes(&bytes).unwrap();
        // The loaded world makes the hero at another slot; the unnamed one a save names not.
        let mut loaded = App::new();
        loaded.add_plugins(NamesPlugin);
        loaded.world_mut().spawn_batch((0..5).map(|_| ()));
        let again = loaded.world_mut().spawn(Named(Id::Hero)).id();
        let druid = loaded.world_mut().spawn(Named(Id::Born(7))).id();
        let doing = mapped(
            doing,
            &mut Loading {
                refs: &refs,
                names: loaded.world().resource::<Names>(),
                lost: 0,
            },
        );
        doing.give(loaded.world_mut(), druid);
        let world = loaded.world();
        assert_eq!(
            world.get::<Makes>(druid),
            Some(&Makes {
                target: again,
                ..makes
            })
        );
        // The volley at none, as at a target gone in play.
        assert_eq!(
            world.get::<Volley>(druid),
            Some(&Volley {
                target: Entity::PLACEHOLDER,
                ..volley
            })
        );
    }
}
