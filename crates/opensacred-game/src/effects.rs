//! The creatures' effects (#763; docs/re/arts.md, "The effects"): what the combat arts and the
//! spells give a creature for a time. Each is added as `0x582D50` adds it, at the list's end, so
//! that two of one id stack, and counted down on the world's tick; whenever the list changes the
//! creature's fight values are made anew with it (`0x5796A0`, `opensacred_rules::fight::Effect`):
//! the hero's attack, defence, damage, resistances, attack speed and regeneration, an NPC's with
//! its pace and its hits' share too. The effects' own side effects at their adding (#761) are left
//! out.

use bevy::prelude::*;
use opensacred_rules::fight::{Effect, combat_with, regeneration_with};

use crate::fight::{self, Base, Combat, Experience, Holding, Ranked, Regenerates, Rules};
use crate::frame::{Step, WorldTick};
use crate::hero::Body;
use crate::npcs::{self, Arms, Npc};

/// An effect and the seconds it still holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timed {
    pub effect: Effect,
    pub left: f32,
}

/// A creature's effects, in the order they were added.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct Effects(Vec<Timed>);

impl Effects {
    /// Adds `effect` for `seconds` at the list's end.
    pub fn add(&mut self, effect: Effect, seconds: f32) {
        self.0.push(Timed {
            effect,
            left: seconds,
        });
    }

    /// Whether an effect keyed by one of `arts` holds (`0x582B00`).
    pub fn holds_any(&self, arts: &[u16]) -> bool {
        self.0
            .iter()
            .any(|timed| arts.iter().any(|&art| timed.effect.art == u32::from(art)))
    }

    /// The first effect of `id` that holds, changed by `change`; whether one holds.
    pub fn change(&mut self, id: u32, change: impl FnOnce(&mut Effect)) -> bool {
        match self.0.iter_mut().find(|timed| timed.effect.id == id) {
            Some(timed) => {
                change(&mut timed.effect);
                true
            }
            None => false,
        }
    }

    /// Whether an effect of `id` holds.
    pub fn holds(&self, id: u32) -> bool {
        self.0.iter().any(|timed| timed.effect.id == id)
    }

    /// Takes off every effect of `id`.
    pub fn remove(&mut self, id: u32) {
        self.0.retain(|timed| timed.effect.id != id);
    }

    /// Takes off every effect keyed by one of `arts` (`0x5888F0`).
    pub fn take_off(&mut self, arts: &[u32]) {
        self.0.retain(|timed| !arts.contains(&timed.effect.art));
    }

    /// The effects and their seconds, in their order.
    pub fn timed(&self) -> &[Timed] {
        &self.0
    }

    /// The effects themselves, as the recomputation takes them.
    pub fn list(&self) -> Vec<Effect> {
        self.0.iter().map(|timed| timed.effect).collect()
    }

    /// A step of `seconds` of each bleeding (the effect 23, `opensacred_rules::arts::bleed`): the
    /// physical damage its strikes take in all.
    fn bleed(&mut self, seconds: f32) -> u32 {
        let mut struck = 0;
        for timed in &mut self.0 {
            if timed.effect.id == opensacred_rules::arts::BLEEDS {
                let (whole, strike) =
                    opensacred_rules::arts::bleed(timed.effect.whole, timed.effect.value, seconds);
                timed.effect.whole = whole;
                struck += strike.unwrap_or(0);
            }
        }
        struck
    }

    /// Takes `seconds` off each effect's time, and those whose time is out off the list: whether
    /// one went.
    fn wear(&mut self, seconds: f32) -> bool {
        let before = self.0.len();
        for timed in &mut self.0 {
            timed.left -= seconds;
        }
        self.0.retain(|timed| timed.left > 0.0);
        self.0.len() != before
    }
}

/// The effect whose going kills an NPC: the raised dead's and the spiders' time (`0x586D80`).
pub const LIFETIME: u32 = 29;

/// Counts each creature's effects down by the tick's seconds; the list counts as changed only
/// when one runs out, so that the values are made anew then. An NPC whose effect 29 goes dies,
/// its health set to 0 (`0x586D80`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn wear(
    mut commands: Commands,
    tick: Res<WorldTick>,
    mut creatures: Query<(
        Entity,
        &mut Effects,
        Option<&mut fight::Health>,
        Has<Body>,
        Has<npcs::Dies>,
    )>,
) {
    let Some(seconds) = tick.seconds else {
        return;
    };
    for (entity, mut effects, health, hero, dead) in &mut creatures {
        let lived = effects.holds(LIFETIME);
        if effects.bypass_change_detection().wear(seconds) {
            effects.set_changed();
            if lived && !effects.holds(LIFETIME) && !hero && !dead {
                debug!("effects: {entity}'s time is out");
                if let Some(mut health) = health {
                    health.current = 0;
                }
                commands.entity(entity).insert(npcs::Dies::by_creature(0));
            }
        }
    }
}

/// The most of a tick's time that the bleeding takes (`0x5375C0`).
const BLEEDS_AT_MOST: f32 = 0.3;

/// Each NPC's bleeding at each tick of the world (the creature's update `0x5375C0` at
/// `0x537F07`, `0x560340`): each strike sends it the strike of its physical damage from the hero,
/// the death's kiss's giver, which rolls no hit (the event 5 with fixed damage).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn bleed(
    tick: Res<WorldTick>,
    mut hits: MessageWriter<fight::Hit>,
    heroes: Query<Entity, With<Body>>,
    mut creatures: Query<(Entity, &mut Effects), (Without<Body>, Without<npcs::Dies>)>,
) {
    let (Some(seconds), Some(hero)) = (tick.seconds, heroes.iter().next()) else {
        return;
    };
    for (entity, mut effects) in &mut creatures {
        let struck = effects
            .bypass_change_detection()
            .bleed(seconds.min(BLEEDS_AT_MOST));
        if struck > 0 {
            debug!("effects: {entity} bleeds {struck}");
            hits.write(fight::Hit {
                attacker: hero,
                target: entity,
                share: 1.0,
                ranged: false,
                art: None,
                fixed: Some([
                    f32::from(u16::try_from(struck).unwrap_or(u16::MAX)),
                    0.0,
                    0.0,
                    0.0,
                ]),
                sure: true,
            });
        }
    }
}

/// Makes the hero's attack, defence, damage, resistances, attack speed and regeneration anew with
/// its effects whenever they change, at its level and with what it holds and wears, as its
/// level-up makes them (`fight::level_up`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn refit_hero(
    experience: Res<Experience>,
    rules: Res<Rules>,
    mut hero: Option<ResMut<crate::hero::Hero>>,
    mut heroes: Query<
        (
            &Base,
            Option<&Holding>,
            &Effects,
            &mut Combat,
            &mut Regenerates,
        ),
        (With<Body>, Changed<Effects>),
    >,
) {
    for (base, holding, effects, mut combat, mut regenerates) in &mut heroes {
        let level = experience.level;
        let skills = fight::skills(base, level, true);
        let holds = holding.map(|holding| holding.0.clone()).unwrap_or_default();
        let list = effects.list();
        *combat = Combat(combat_with(base, level, holds, &skills, &list, &rules));
        regenerates.value = regeneration_with(base, level, &skills, &list, &rules);
        // Its walk and run at its pace's factor, which the effects 28, 30, 52 and 58 change.
        if let Some(hero) = hero.as_mut() {
            hero.pace = combat.pace_factor();
        }
    }
}

/// Makes an NPC's fight values anew with its effects whenever they change, at its level and of
/// its rank (`npcs::refit`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn refit_npcs(
    rules: Res<Rules>,
    mut creatures: Query<
        (
            &Effects,
            &Arms,
            &fight::Health,
            Option<&Ranked>,
            &mut crate::ai::Mind,
            &mut Combat,
            &mut Npc,
            &mut Regenerates,
        ),
        (Without<Body>, Changed<Effects>),
    >,
) {
    for (effects, arms, health, rank, mut mind, mut combat, mut npc, mut regenerates) in
        &mut creatures
    {
        // The effect 14 turns its sides over while it holds (its adder sets `+0x1F4` bit
        // `0x40000`, its taker clears it).
        let turned = effects.holds(opensacred_rules::spells::CONVERTED);
        if mind.turned != turned {
            mind.turned = turned;
            // Its fight with the hero, a friend now, it leaves (**hypothesis**).
            if turned && mind.fighting() && mind.foe().is_none() {
                mind.loses_foe();
            }
        }
        let rank = rank.map_or_else(fight::Rank::default, |rank| rank.0);
        regenerates.value = npcs::refit(
            arms,
            health.level,
            rank,
            &rules,
            &effects.list(),
            (&mut mind, &mut combat, &mut npc),
        );
    }
}

/// The creatures' effects.
pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                wear.run_if(crate::frame::ticked),
                bleed.run_if(crate::frame::ticked),
                refit_hero,
                refit_npcs,
            )
                .chain()
                .in_set(crate::frame::Unpaused)
                .in_set(Step::Crowd),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_stack_and_wear_off_with_their_time() {
        let mut effects = Effects::default();
        effects.add(Effect::new(1, 0.2), 0.25);
        effects.add(Effect::new(1, 0.2), 1.0);
        assert_eq!(effects.list(), vec![Effect::new(1, 0.2); 2]);
        // A tick takes its seconds off each; the first runs out at its third.
        assert!(!effects.wear(0.1));
        assert!(!effects.wear(0.1));
        assert!(effects.wear(0.1));
        assert_eq!(effects.timed().len(), 1);
        assert!((effects.timed()[0].left - 0.7).abs() < 1e-6);
    }
}
