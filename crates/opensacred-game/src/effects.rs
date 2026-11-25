//! The creatures' effects (#763; docs/re/arts.md, "The effects"): what the combat arts and the
//! spells give a creature for a time. Each is added as `0x582D50` adds it, at the list's end, so
//! that two of one id stack, and counted down on the world's tick; whenever the list changes the
//! creature's fight values are made anew with it (`0x5796A0`, `opensacred_rules::fight::Effect`):
//! the hero's attack, defence, damage, resistances, attack speed and regeneration, an NPC's with
//! its pace and its hits' share too. Of the adder's own cases by the effect's id (#761), the
//! module follows the poison's merging (30, #1354) and its cure and the potions' 33 held alone
//! (#1353), and the conversion's turn of sides (14); the others are followed where a feature
//! reads them, else left out. At each tick an NPC's bleeding (23) and a creature's poison (30)
//! strike their bearer, and an NPC whose time (29) is out dies.

use bevy::prelude::*;
use opensacred_rules::fight::{Effect, combat_with, regeneration_with};

use crate::fight::{self, Base, Combat, Experience, Holding, Ranked, Regenerates, Rules};
use crate::frame::{Step, WorldTick};
use crate::hero::Body;
use crate::npcs::{self, Arms, Npc};

/// An effect, the seconds it still holds, and the creature that gave it where that counts: a
/// poison's (the adder's eighth argument, first in the entry's list at `+0x08`).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Timed {
    pub effect: Effect,
    pub left: f32,
    /// Not saved: an entity names a slot of the running world alone, so that a loaded effect
    /// has none (docs/decisions/interface.md, "Poison's source not saved"; #2308).
    #[serde(skip)]
    pub source: Option<Entity>,
}

/// A creature's effects, in the order they were added.
#[derive(Component, Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Effects(Vec<Timed>);

impl Effects {
    /// Adds `effect` for `seconds` at the list's end.
    pub fn add(&mut self, effect: Effect, seconds: f32) {
        self.0.push(Timed {
            effect,
            left: seconds,
            source: None,
        });
    }

    /// Adds a strike's poison from `source` as `0x582D50` adds the effect 30 (at `0x583E40`;
    /// #1354): to the first effect 30 that holds, unless `source` gave it, both doses spread over
    /// the longer time and the source the larger dose's, its whole value as it was
    /// (`opensacred_rules::fight::poison_added`); else at the list's end, slowing by its whole
    /// value and gathering from its key (`Effect::art`, `+0x1C`). Whether it changed the list.
    /// A bearer immune to poison (the stats' `+0x148` bit `0x10`, which set 56 sets at seven
    /// pieces, `opensacred_rules::set_bonuses`) is not followed: nothing reads the flag.
    pub fn poison(&mut self, poison: opensacred_rules::fight::Poison, source: Entity) -> bool {
        let held = self
            .0
            .iter_mut()
            .find(|timed| timed.effect.id == opensacred_rules::fight::effects::SLOWS);
        let Some(timed) = held else {
            self.0.push(Timed {
                effect: Effect {
                    whole: poison.whole,
                    art: poison.gathered,
                    ..Effect::new(opensacred_rules::fight::effects::SLOWS, poison.value)
                },
                left: poison.seconds,
                source: Some(source),
            });
            return true;
        };
        if timed.source == Some(source) {
            return false;
        }
        let was = (timed.effect.value, timed.left);
        let added = (poison.value, poison.seconds);
        if opensacred_rules::fight::poison_outweighs(was, added) {
            timed.source = Some(source);
        }
        (timed.effect.value, timed.left) = opensacred_rules::fight::poison_added(was, added);
        true
    }

    /// Whether it bleeds, an effect 23 holding.
    pub fn bleeds(&self) -> bool {
        self.0
            .iter()
            .any(|timed| timed.effect.id == opensacred_rules::arts::BLEEDS)
    }

    /// Adds a script's bleeding (`SetNPCState`'s `0x84`, `0x461D1F`, #3444): the effect 23 of the
    /// whole value 1 and the value `value` for `seconds`, keyed by no art and with no source, by
    /// which its bearer strikes itself ([`bleed`]).
    pub fn bleed_by_script(&mut self, value: f32, seconds: f32) {
        self.0.push(Timed {
            effect: Effect {
                whole: 1,
                ..Effect::new(opensacred_rules::arts::BLEEDS, value)
            },
            left: seconds,
            source: None,
        });
    }

    /// Ends every bleeding (`SetNPCState`'s `0x84 (0, 0)`, `0x461D29`–`0x461DBD`): their seconds
    /// set to 0.001.
    pub fn stop_bleeding(&mut self) {
        for timed in &mut self.0 {
            if timed.effect.id == opensacred_rules::arts::BLEEDS {
                timed.left = CURED_LEFT;
            }
        }
    }

    /// Adds a strike's open wound from `source` (`opensacred_rules::fight::open_wound`, #1727):
    /// the effect 23 of the value `value` for `seconds`, unless it bleeds already. Whether it went.
    pub fn wound(&mut self, value: f32, seconds: f32, source: Entity) -> bool {
        if self.bleeds() {
            return false;
        }
        self.0.push(Timed {
            effect: Effect::new(opensacred_rules::arts::BLEEDS, value),
            left: seconds,
            source: Some(source),
        });
        true
    }

    /// Adds the effect 38 from `source` for `seconds` (`0x56E0A4`, #3438): its own of that source
    /// takes the new seconds, else a new one at the list's end.
    pub fn spare(&mut self, seconds: f32, source: Entity) {
        let id = opensacred_rules::fight::SPARED_BY_FIRE;
        let held = self
            .0
            .iter_mut()
            .find(|timed| timed.effect.id == id && timed.source == Some(source));
        match held {
            Some(timed) => timed.left = seconds,
            None => self.0.push(Timed {
                effect: Effect::new(id, 0.0),
                left: seconds,
                source: Some(source),
            }),
        }
    }

    /// Adds a potion's effect 33 for `seconds` as `0x582D50` does (at `0x584328`): one of the
    /// same whole value alone, which keeps its place and value and takes the longer time.
    pub fn add_alone(&mut self, effect: Effect, seconds: f32) {
        let held = self
            .0
            .iter_mut()
            .find(|timed| timed.effect.id == effect.id && timed.effect.whole == effect.whole);
        match held {
            Some(timed) => timed.left = timed.left.max(seconds),
            None => self.add(effect, seconds),
        }
    }

    /// Adds the butterflies' effect 31 `effect` for `seconds` as `0x582D50` does (at `0x58437C`;
    /// #3023): to the first that holds, its seconds then `seconds`, its art grown by `effect`'s
    /// where that is the larger, its value the larger of the two; else at the list's end. The art
    /// that its butterflies then take (`0x584528`): the one that holds, else `effect`'s.
    pub fn knock_out(&mut self, effect: Effect, seconds: f32) -> u32 {
        let held = self.0.iter_mut().find(|timed| timed.effect.id == effect.id);
        let Some(timed) = held else {
            self.add(effect, seconds);
            return effect.art;
        };
        timed.left = seconds;
        if timed.effect.art < effect.art {
            timed.effect.art = timed.effect.art.saturating_add(effect.art);
        }
        timed.effect.value = timed.effect.value.max(effect.value);
        timed.effect.art
    }

    /// A green potion's cure of `per_cent` (`0x582D50` at `0x583F1B`): each effect 30 keeps the
    /// value `potions::cured` leaves it, or ends at once.
    pub fn cure(&mut self, per_cent: u32) {
        for timed in &mut self.0 {
            if timed.effect.id != opensacred_rules::fight::effects::SLOWS {
                continue;
            }
            match opensacred_rules::potions::cured(timed.effect.value, per_cent) {
                Some(value) => timed.effect.value = value,
                None => {
                    timed.effect.value = 0.0;
                    timed.left = CURED_LEFT;
                }
            }
        }
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

    /// The seconds the first effect of `id` still holds.
    pub fn left(&self, id: u32) -> Option<f32> {
        self.0
            .iter()
            .find(|timed| timed.effect.id == id)
            .map(|timed| timed.left)
    }

    /// Gives the first effect of `id` `seconds` more; whether one holds.
    pub fn prolong(&mut self, id: u32, seconds: f32) -> bool {
        match self.0.iter_mut().find(|timed| timed.effect.id == id) {
            Some(timed) => {
                timed.left += seconds;
                true
            }
            None => false,
        }
    }

    /// Takes off every effect of `id`.
    pub fn remove(&mut self, id: u32) {
        self.0.retain(|timed| timed.effect.id != id);
    }

    /// Takes off every effect keyed by one of `arts` (`0x5888F0`).
    pub fn take_off(&mut self, arts: &[u32]) {
        self.0.retain(|timed| !arts.contains(&timed.effect.art));
    }

    /// Ends every effect keyed by one of `arts` at the tick's wear, its seconds set to 0, as
    /// `0x5888F0` does, so that what its end does follows (the Daemon's form's, `daemon`).
    pub fn end(&mut self, arts: &[u32]) {
        for timed in &mut self.0 {
            if arts.contains(&timed.effect.art) {
                timed.left = 0.0;
            }
        }
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
    /// physical damage its strikes take in all, of the strikes' and the kiss's, and of the
    /// scripts', keyed by no art and of no source, which its bearer strikes itself with
    /// (`0x5604FB`).
    fn bleed(&mut self, seconds: f32) -> (u32, u32) {
        let (mut struck, mut own) = (0, 0);
        for timed in &mut self.0 {
            if timed.effect.id == opensacred_rules::arts::BLEEDS {
                let (whole, strike) =
                    opensacred_rules::arts::bleed(timed.effect.whole, timed.effect.value, seconds);
                timed.effect.whole = whole;
                if timed.source.is_none() && timed.effect.art == 0 {
                    own += strike.unwrap_or(0);
                } else {
                    struck += strike.unwrap_or(0);
                }
            }
        }
        (struck, own)
    }

    /// A step of `seconds` of each poison (the effect 30, `opensacred_rules::fight::poison_tick`):
    /// the poison's damage of each strike and the creature that gave the poison; on a bearer that
    /// is dead, `dead`, each ends at once instead (`0x5605E9`).
    fn poison_step(&mut self, seconds: f32, dead: bool) -> Vec<(u32, Option<Entity>)> {
        let mut strikes = Vec::new();
        for timed in &mut self.0 {
            if timed.effect.id != opensacred_rules::fight::effects::SLOWS {
                continue;
            }
            if dead {
                timed.left = CURED_LEFT;
                continue;
            }
            let (gathered, strike) =
                opensacred_rules::fight::poison_tick(timed.effect.art, timed.effect.value, seconds);
            timed.effect.art = gathered;
            strikes.extend(strike.map(|strike| (strike, timed.source)));
        }
        strikes
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

/// The seconds that an effect 30 cured wholly keeps (the float `0x3A83126F`), so that it goes at
/// the next tick.
const CURED_LEFT: f32 = 0.001;

/// The key of the effect 30 that a heal gives its target with its health, the creature's event 6
/// of the sub-kind 3 (the healing's action 3, `0x5B53A3`; a soul's, `0x5ABD80`'s case `0x30C`):
/// of the whole value and the value 0 for 1 s, a cure of its key's per cent, so every poison's
/// value 0, its key of 90 and above (the adder's row 30, #3139).
pub(crate) const HEAL_CURES: u32 = 100;

/// Cures the poisons of `target` as a heal's effect 30 does ([`HEAL_CURES`]).
pub(crate) fn heal_cures(commands: &mut Commands, target: Entity) {
    commands
        .entity(target)
        .entry::<Effects>()
        .and_modify(|mut effects| effects.cure(HEAL_CURES));
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
                commands.entity(entity).insert(npcs::Dies::unkilled());
            }
        }
    }
}

/// The most of a tick's time that the bleeding and the poison take (`0x5375C0`).
const STEP_AT_MOST: f32 = 0.3;

/// A script's bleeding of the hero (`SetNPCState`'s `0x84`): its value and seconds, or none to
/// end every bleeding (#3444).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct ScriptBleeding(pub Option<(f32, f32)>);

/// Gives the hero the scripts' bleedings, or ends them ([`ScriptBleeding`]).
fn script_bleedings(
    mut commands: Commands,
    mut bleedings: MessageReader<ScriptBleeding>,
    heroes: Query<Entity, With<Body>>,
) {
    let Some(hero) = heroes.iter().next() else {
        bleedings.clear();
        return;
    };
    for &ScriptBleeding(bleeding) in bleedings.read() {
        debug!("effects: the scripts bleed the hero: {bleeding:?}");
        commands
            .entity(hero)
            .entry::<Effects>()
            .or_default()
            .and_modify(move |mut effects| match bleeding {
                Some((value, seconds)) => effects.bleed_by_script(value, seconds),
                None => effects.stop_bleeding(),
            });
    }
}

/// The strike of a bleeding's `damage`, fixed and physical, which rolls no hit (the event 5), from
/// `attacker`, of the poison's own kind 17 where `own` (`0x56052C`).
fn bled(attacker: Entity, target: Entity, damage: u32, own: bool) -> fight::Hit {
    fight::Hit {
        attacker,
        target,
        share: 1.0,
        ranged: false,
        art: None,
        fixed: Some([
            f32::from(u16::try_from(damage).unwrap_or(u16::MAX)),
            0.0,
            0.0,
            0.0,
        ]),
        sure: true,
        poison: own,
        ward: false,
        hurled: false,
        object: false,
        mount: false,
        marks: false,
    }
}

/// Each creature's bleeding at each tick of the world (the creature's update `0x5375C0` at
/// `0x537F07`, `0x560340`): an NPC's strikes' and kiss's bleeding sends it the strike of its
/// physical damage from the hero, the death's kiss's giver; a script's, the hero's too, from the
/// bearer itself, of the kind 17 (`0x5604FB`, `0x56052C`; #3444). Each rolls no hit.
fn bleed(
    tick: Res<WorldTick>,
    mut hits: MessageWriter<fight::Hit>,
    heroes: Query<Entity, With<Body>>,
    mut creatures: Query<(Entity, &mut Effects, Has<Body>), Without<npcs::Dies>>,
) {
    let (Some(seconds), Some(hero)) = (tick.seconds, heroes.iter().next()) else {
        return;
    };
    for (entity, mut effects, is_hero) in &mut creatures {
        let (struck, own) = effects
            .bypass_change_detection()
            .bleed(seconds.min(STEP_AT_MOST));
        if struck > 0 && !is_hero {
            debug!("effects: {entity} bleeds {struck}");
            hits.write(bled(hero, entity, struck, false));
        }
        if own > 0 {
            debug!("effects: {entity} bleeds {own} by itself");
            hits.write(bled(entity, entity, own, true));
        }
    }
}

/// Each creature's poisons at each tick of the world (`0x560340` at `0x5605D3`, the hero's too;
/// #1354): each strike sends its bearer the strike of its poison's damage from the creature that
/// poisoned it, or from itself where none is (`0x560766`), the event 5 of the kind 17 with fixed
/// damage, which rolls no hit and poisons no more (`0x5607C3`), so that a death of the poison
/// is its poisoner's kill; a dead bearer's poisons end. Each strike makes the generic system's
/// variant 6 at its bearer, green glows rising about its figure, and at the horse of a riding one
/// (`0x560872`, `0x560968`; `group_looks::poison_glow`, #3081).
fn poison(
    mut commands: Commands,
    tick: Res<WorldTick>,
    (mut hits, seats): (MessageWriter<fight::Hit>, Query<&crate::riding::Seat>),
    mut creatures: Query<(Entity, &mut Effects, &fight::Health), Without<npcs::Dies>>,
    strikers: Query<(), (With<fight::Combat>, Without<npcs::Dies>)>,
) {
    let Some(seconds) = tick.seconds else {
        return;
    };
    for (entity, mut effects, health) in &mut creatures {
        if !effects.holds(opensacred_rules::fight::effects::SLOWS) {
            continue;
        }
        let strikes = effects
            .bypass_change_detection()
            .poison_step(seconds.min(STEP_AT_MOST), health.current == 0);
        for (strike, source) in strikes {
            let attacker = source
                .filter(|&source| strikers.contains(source))
                .unwrap_or(entity);
            debug!("effects: the poison of {attacker} strikes {entity} for {strike}");
            let mount = seats.get(entity).ok().and_then(crate::riding::Seat::ridden);
            commands.queue(move |world: &mut World| {
                crate::group_looks::poison_glow(world, entity);
                if let Some(mount) = mount {
                    crate::group_looks::poison_glow(world, mount);
                }
            });
            hits.write(fight::Hit {
                attacker,
                target: entity,
                share: 1.0,
                ranged: false,
                art: None,
                fixed: Some([
                    0.0,
                    0.0,
                    0.0,
                    f32::from(u16::try_from(strike).unwrap_or(u16::MAX)),
                ]),
                sure: true,
                poison: true,
                ward: false,
                hurled: false,
                object: false,
                mount: false,
                marks: false,
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
        let holds = holding.map(|holding| holding.0.clone()).unwrap_or_default();
        let skills = fight::hero_skills(base, level, &holds);
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

/// The tick's wear of the effects, which a system that sees an effect's end before it goes runs
/// before.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Wears;

/// The creatures' effects.
pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ScriptBleeding>().add_systems(
            Update,
            (
                wear.run_if(crate::frame::ticked).in_set(Wears),
                script_bleedings,
                bleed.run_if(crate::frame::ticked),
                poison.run_if(crate::frame::ticked),
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
    fn a_scripts_bleeding_strikes_its_bearer_itself_until_it_is_ended() {
        let mut effects = Effects::default();
        effects.bleed_by_script(10.0, 600.0);
        // From the whole 1, 300 gathered each 0.3 s: the fourth step passes 990 and strikes 12,
        // all of its own, none of another's.
        let steps: Vec<(u32, u32)> = (0..4).map(|_| effects.bleed(0.3)).collect();
        assert_eq!(steps, [(0, 0), (0, 0), (0, 0), (0, 12)]);
        // A strike's wound beside it strikes from its giver.
        let giver = Entity::from_raw_u32(7).expect("an entity");
        let mut wounded = Effects::default();
        assert!(wounded.wound(10.0, 5.0, giver));
        let mut strikes = (0, 0);
        for _ in 0..4 {
            let (struck, own) = wounded.bleed(0.3);
            strikes = (strikes.0 + struck, strikes.1 + own);
        }
        assert_eq!(strikes, (12, 0));
        // `(0, 0)` ends every bleeding, the wound's too.
        effects.stop_bleeding();
        wounded.stop_bleeding();
        assert!(effects.left(opensacred_rules::arts::BLEEDS) == Some(CURED_LEFT));
        assert!(wounded.left(opensacred_rules::arts::BLEEDS) == Some(CURED_LEFT));
    }

    #[test]
    fn the_butterflies_effect_renews_its_seconds_and_grows_its_art() {
        let of = |art| Effect {
            art,
            ..Effect::new(opensacred_rules::fight::effects::KNOCKOUT, 0.0)
        };
        let mut effects = Effects::default();
        assert_eq!(effects.knock_out(of(500), 10.0), 500);
        // A smaller art renews the seconds alone; a larger one grows the held art by itself.
        effects.0[0].left = 3.0;
        assert_eq!(effects.knock_out(of(400), 10.0), 500);
        assert_eq!(effects.0[0].left, 10.0);
        assert_eq!(effects.knock_out(of(600), 10.0), 1100);
        assert_eq!(effects.0.len(), 1);
    }

    #[test]
    fn a_saved_poison_keeps_its_dose_but_not_its_source() {
        let mut effects = Effects::default();
        let poison = opensacred_rules::fight::Poison {
            whole: 4,
            value: 0.4,
            seconds: 12.0,
            gathered: 3,
        };
        assert!(effects.poison(poison, Entity::PLACEHOLDER));
        assert_eq!(effects.timed()[0].source, Some(Entity::PLACEHOLDER));
        // An entity names a slot of the running world alone (#2308).
        let bytes = postcard::to_stdvec(&effects).unwrap();
        let read: Effects = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(read.timed()[0].source, None);
        assert_eq!(read.timed()[0].effect, effects.timed()[0].effect);
        assert!((read.timed()[0].left - 12.0).abs() < f32::EPSILON);
    }

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

    #[test]
    fn a_potions_effect_holds_alone_by_its_whole_value_and_a_cure_eases_the_poison() {
        let potion = |whole, value| Effect {
            whole,
            ..Effect::new(33, value)
        };
        let mut effects = Effects::default();
        effects.add_alone(potion(5, 0.5), 60.0);
        effects.add_alone(potion(4, 0.8), 30.0);
        // A second of a whole value held only lengthens the first to the longer time.
        effects.add_alone(potion(5, 0.9), 90.0);
        effects.add_alone(potion(4, 0.9), 10.0);
        let held: Vec<_> = effects
            .timed()
            .iter()
            .map(|timed| (timed.effect.whole, timed.effect.value, timed.left))
            .collect();
        assert_eq!(held, vec![(5, 0.5, 90.0), (4, 0.8, 30.0)]);
        let poison = Effect {
            whole: 20,
            ..Effect::new(30, 8.0)
        };
        effects.add(poison, 20.0);
        effects.cure(50);
        assert_eq!(effects.timed()[2].effect.value, 4.0);
        effects.cure(100);
        assert_eq!(effects.timed()[2].effect.value, 0.0);
        assert!(effects.wear(0.1));
        assert!(!effects.holds(30));
    }

    #[test]
    fn a_poison_joins_anothers_but_not_its_own_and_strikes_from_its_poisoner() {
        let poison = |value, seconds| opensacred_rules::fight::Poison {
            whole: 10,
            value,
            seconds,
            gathered: 0,
        };
        let (a, b) = (
            Entity::from_raw_u32(1).unwrap(),
            Entity::from_raw_u32(2).unwrap(),
        );
        let mut effects = Effects::default();
        assert!(effects.poison(poison(2.0, 3.0), a));
        // The same poisoner's again adds nothing.
        assert!(!effects.poison(poison(9.0, 9.0), a));
        // Another's: both doses over the longer time, the larger dose's poisoner.
        assert!(effects.poison(poison(4.0, 6.0), b));
        let held = effects.timed()[0];
        assert_eq!(
            (held.effect.value, held.left, held.source),
            (5.0, 6.0, Some(b))
        );
        assert_eq!((held.effect.whole, effects.timed().len()), (10, 1));
        // It strikes from its poisoner once it has gathered enough, and ends on a dead bearer.
        let mut strikes = Vec::new();
        for _ in 0..10 {
            strikes.extend(effects.poison_step(0.3, false));
        }
        assert!(!strikes.is_empty());
        assert!(strikes.iter().all(|&(_, source)| source == Some(b)));
        assert!(effects.poison_step(0.3, true).is_empty());
        assert!(effects.wear(0.01));
        assert!(!effects.holds(30));
    }
}
