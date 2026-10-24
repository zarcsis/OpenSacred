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

    /// The effects and their seconds, in their order.
    pub fn timed(&self) -> &[Timed] {
        &self.0
    }

    /// The effects themselves, as the recomputation takes them.
    pub fn list(&self) -> Vec<Effect> {
        self.0.iter().map(|timed| timed.effect).collect()
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

/// Counts each creature's effects down by the tick's seconds; the list counts as changed only
/// when one runs out, so that the values are made anew then.
fn wear(tick: Res<WorldTick>, mut creatures: Query<&mut Effects>) {
    let Some(seconds) = tick.seconds else {
        return;
    };
    for mut effects in &mut creatures {
        if effects.bypass_change_detection().wear(seconds) {
            effects.set_changed();
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
            (wear.run_if(crate::frame::ticked), refit_hero, refit_npcs)
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
