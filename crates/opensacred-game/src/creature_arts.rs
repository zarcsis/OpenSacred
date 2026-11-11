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

/// The creatures' weapon arts.
pub struct CreatureArtsPlugin;

impl Plugin for CreatureArtsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (start, strike, end)
                .chain()
                .run_if(in_state(crate::game::AppState::InGame)),
        );
    }
}
