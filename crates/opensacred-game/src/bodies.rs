//! The bodies' end (#2501; docs/re/fight.md, "The bodies' end"): any creature's body lies 20, 5
//! or 2 s after its death by the options' detail (`opensacred_rules::undead::vanishes_after`,
//! `0x54101B`), then vanishes at once, its figure in pale green sparks rising where it lay (780
//! homing on nothing, `soul_sparks`), and is destroyed 25, 10 or 7 s after its death
//! (`undead::destroy`); time alone counts, not the hero's distance nor the bodies' number. A body
//! lies on while its flags word holds `0x2000` (the tag `0x45`), a death's function still stands
//! for it, it has a dialogue, a stable's stamp or the trader's, or it rises again
//! (`npcs::RisesAgain`), and vanishes at the first check once the hold ends (`0x54140A`). The
//! undead of the class 5 and the bodies that burst end their own ways (`undead`).

use bevy::prelude::*;
use opensacred_rules::quests::TRADER;
use opensacred_rules::undead::{self, LIES_ON};

use crate::frame::{Playing, Step};
use crate::game::AppState;
use crate::npcs::{Corpse, Dies, OnDeath, RisesAgain, Talk, Trades};
use crate::undead::{Bursts, Destroyed, gone};

/// A body that lies until its seconds run out ([`vanish`]), and whether it then goes at once,
/// with no soul and whatever holds it: a medium spider's (#2504).
#[derive(Component, Clone, Copy, Debug)]
struct Lies(f32, bool);

/// Gives each new body of a creature that ends no other way its time to lie and its time to be
/// destroyed, both from its death (`0x53E880`): a body put back lying, which its dialogue held as
/// it went (#2457), has lain its time already.
fn lie(
    mut commands: Commands,
    lying: Query<(Entity, &Corpse, &Dies, Has<Bursts>), Added<Corpse>>,
    settings: Option<Res<crate::settings::Settings>>,
) {
    let detail = crate::settings::detail(settings.as_deref());
    for (entity, corpse, dies, bursts) in &lying {
        if bursts
            || dies.rises
            || corpse.class == undead::CLASS
            || corpse.creature == undead::FIRE_GHOUL
        {
            continue;
        }
        let destroyed = Destroyed(undead::destroyed_after(detail));
        if corpse.creature == undead::MEDIUM_SPIDER {
            commands
                .entity(entity)
                .insert((Lies(undead::FIRST_LYING_TICK, true), destroyed));
            continue;
        }
        let lies = if dies.fallen {
            0.0
        } else {
            undead::vanishes_after(detail)
        };
        commands
            .entity(entity)
            .insert((Lies(lies, false), destroyed));
    }
}

/// What holds a body lying past its time (`0x54140A`'s tests): its flags word's `0x2000`, a
/// death's function that still stands for it (`0x4A71F0`), a dialogue whose icon is not `0xFFFF`
/// (`0x499E90`: its dialogue's, a stable's stamp or the trader's, which give the icon 4), or the
/// `+0x2B7` bit 8 of one that rises again (`0x53FBDC`).
fn held(
    corpse: &Corpse,
    on_death: Option<&OnDeath>,
    (talks, stabled, trades): (bool, bool, Option<&Trades>),
    rises: bool,
) -> bool {
    corpse.flags & LIES_ON != 0
        || on_death.is_some_and(|on_death| on_death.0.holds())
        || talks
        || stabled
        || trades.is_some_and(|trades| trades.0 & TRADER != 0)
        || rises
}

/// Each body that has lain its time and that nothing holds vanishes (`0x54140A`): its figure in
/// the soul's sparks rising where it lies (780 by the FX event `0xC`, `0x541491`), and it is
/// hidden and out of the world, its destruction to come (`undead::destroy`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn vanish(
    mut commands: Commands,
    time: Res<Time>,
    mut lying: Query<(
        Entity,
        &mut Lies,
        &Corpse,
        &GlobalTransform,
        Option<&OnDeath>,
        (
            Has<Talk>,
            Has<crate::stables::Stabled>,
            Option<&Trades>,
            Has<RisesAgain>,
        ),
    )>,
) {
    for (entity, mut lies, corpse, at, on_death, (talks, stabled, trades, rises)) in &mut lying {
        lies.0 -= time.delta_secs();
        if lies.0 > 0.0 {
            continue;
        }
        // A medium spider's goes at its first tick of lying, as it is (`0x54045E`).
        if lies.1 {
            debug!("bodies: {entity}, a {}, goes at once", corpse.creature);
            commands.entity(entity).remove::<Lies>();
            gone(&mut commands, entity);
            continue;
        }
        if held(corpse, on_death, (talks, stabled, trades), rises) {
            continue;
        }
        debug!("bodies: {entity}, a {}, vanishes", corpse.creature);
        commands.spawn((
            Transform::from_translation(at.translation()),
            crate::soul_sparks::Sparks {
                from: entity,
                rising: true,
            },
            DespawnOnExit(AppState::InGame),
        ));
        commands.entity(entity).remove::<Lies>();
        gone(&mut commands, entity);
    }
}

/// The bodies' lying and vanishing.
pub struct BodiesPlugin;

impl Plugin for BodiesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (lie, vanish).chain().after(Step::Npcs).in_set(Playing),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opensacred_rules::quests::DeathFunctions;

    fn corpse(flags: u32) -> Corpse {
        Corpse {
            creature: 76,
            class: 0,
            dialogue: false,
            level: None,
            hostile: true,
            flags,
        }
    }

    #[test]
    fn a_body_lies_on_while_its_flag_a_function_a_dialogue_or_its_rise_holds_it() {
        let plain = corpse(0);
        assert!(!held(&plain, None, (false, false, None), false));
        assert!(held(&corpse(LIES_ON), None, (false, false, None), false));
        assert!(held(&plain, None, (true, false, None), false));
        assert!(held(&plain, None, (false, true, None), false));
        assert!(held(
            &plain,
            None,
            (false, false, Some(&Trades(TRADER))),
            false
        ));
        assert!(!held(&plain, None, (false, false, Some(&Trades(0))), false));
        assert!(held(&plain, None, (false, false, None), true));
        // A function that the death left: a `wegw` one holds, a sight's does not.
        let mut deaths = DeathFunctions::default();
        deaths.see("look");
        let sight = OnDeath(deaths.clone());
        assert!(!held(&plain, Some(&sight), (false, false, None), false));
        deaths.add("wegweiser");
        let kept = OnDeath(deaths);
        assert!(held(&plain, Some(&kept), (false, false, None), false));
    }

    #[test]
    fn a_body_vanishes_once_it_has_lain_its_time_and_is_destroyed_later() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                250,
            )))
            .add_systems(Update, (lie, vanish).chain());
        let body = app
            .world_mut()
            .spawn((corpse(0), Dies::new(0), GlobalTransform::default()))
            .id();
        let named = app
            .world_mut()
            .spawn((corpse(LIES_ON), Dies::new(0), GlobalTransform::default()))
            .id();
        // A quarter of a second an update (the virtual time's most), the first's perhaps none:
        // 18.75 or 19 s.
        for _ in 0..76 {
            app.update();
        }
        assert!(app.world().get::<Corpse>(body).is_some());
        // 20.75 or 21 s.
        for _ in 0..8 {
            app.update();
        }
        assert!(app.world().get::<Corpse>(body).is_none());
        assert_eq!(
            app.world().get::<Visibility>(body),
            Some(&Visibility::Hidden)
        );
        assert!(app.world().get::<Destroyed>(body).is_some());
        let mut sparks = app.world_mut().query::<&crate::soul_sparks::Sparks>();
        assert!(
            sparks
                .iter(app.world())
                .any(|sparks| sparks.from == body && sparks.rising)
        );
        assert!(app.world().get::<Corpse>(named).is_some());
    }

    #[test]
    fn a_medium_spiders_body_goes_at_its_first_tick_of_lying_held_or_not() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .add_systems(Update, (lie, vanish).chain());
        let spider = app
            .world_mut()
            .spawn((
                Corpse {
                    creature: undead::MEDIUM_SPIDER,
                    ..corpse(LIES_ON)
                },
                Dies::new(0),
                GlobalTransform::default(),
            ))
            .id();
        for _ in 0..5 {
            app.update();
        }
        assert!(app.world().get::<Corpse>(spider).is_none());
        assert!(app.world().get::<Destroyed>(spider).is_some());
        let mut sparks = app.world_mut().query::<&crate::soul_sparks::Sparks>();
        assert_eq!(sparks.iter(app.world()).count(), 0);
    }
}
