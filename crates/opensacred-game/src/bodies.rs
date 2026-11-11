//! The bodies' end (#2501; docs/re/fight.md, "The bodies' end"): any creature's body lies 20, 5
//! or 2 s after its death by the options' detail (`opensacred_rules::undead::vanishes_after`,
//! `0x54101B`), then vanishes at once, its figure in pale green sparks rising where it lay (780
//! homing on nothing, `soul_sparks`), and is destroyed 25, 10 or 7 s after its death
//! (`undead::destroy`); time alone counts, not the hero's distance nor the bodies' number. A body
//! lies on while its flags word holds `0x2000` (the tag `0x45`), a death's function still stands
//! for it, it has a dialogue, a stable's stamp or the trader's, or it rises again
//! (`npcs::RisesAgain`), and vanishes at the first check once the hold ends (`0x54140A`). Some
//! vanish at their first tick of lying instead (#2514, `undead::at_once`): the Vampiress's raised
//! dead in black smoke, the ghosts, the visions, the crow, the types without a dying clip and the
//! bodies on water or lava in blue-white sparks, the bodies turned to stone in pale green dust
//! (781 at their vertices, [`Ends`]), the dummy with no effect, unless their flag, a death's
//! function or a dialogue holds them. The undead of the class 5 and the bodies that burst end
//! their own ways (`undead`).

use bevy::prelude::*;
use opensacred_rules::particles::{self, BodyEnd};
use opensacred_rules::quests::TRADER;
use opensacred_rules::undead::{self, AtOnce, LIES_ON, Lying};

use crate::effects::Effects;
use crate::frame::{Playing, Step};
use crate::game::AppState;
use crate::npcs::{Corpse, Dies, Npc, Npcs, OnDeath, RisesAgain, Talk, Trades};
use crate::spawning::Rng;
use crate::undead::{Bursts, Destroyed, gone};

/// A body that lies: how long it has lain and how long it lies before it vanishes in its soul,
/// what it is for its end at its first tick of lying ([`undead::at_once`]; what keeps it from the
/// liquids' end and its stone tested at each tick), and whether it then goes at once with no soul
/// and whatever holds it, a medium spider's (#2504).
#[derive(Component, Clone, Copy, Debug)]
struct Lies {
    lain: f32,
    time: f32,
    body: Lying,
    spider: bool,
}

/// The dice of the bursts' rolls (`rand()`).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x781))
    }
}

/// Gives each new body of a creature that ends no other way its time to lie, what it is for its
/// end at once, and its time to be destroyed, both from its death (`0x53E880`): a body put back
/// lying, which its dialogue held as it went (#2457), has lain its time already.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn lie(
    mut commands: Commands,
    lying: Query<
        (
            Entity,
            &Corpse,
            &Dies,
            &GlobalTransform,
            Option<&Npc>,
            Has<Bursts>,
        ),
        Added<Corpse>,
    >,
    settings: Option<Res<crate::settings::Settings>>,
    (npcs, heights): (Option<Res<Npcs>>, Option<Res<crate::terrain::Heights>>),
) {
    let detail = crate::settings::detail(settings.as_deref());
    for (entity, corpse, dies, at, npc, bursts) in &lying {
        if bursts
            || dies.rises
            || corpse.class == undead::CLASS
            || corpse.creature == undead::FIRE_GHOUL
        {
            continue;
        }
        let at = at.translation();
        let body = Lying {
            creature: corpse.creature,
            flags: corpse.flags,
            table: npcs
                .as_deref()
                .map_or(0, |npcs| npcs.table_flags(corpse.creature)),
            vision: opensacred_rules::spells::VISIONS.contains(&corpse.creature),
            falls: npc.is_none_or(Npc::falls),
            nature: heights
                .as_deref()
                .and_then(|heights| heights.nature(at.x, at.z)),
            kept: false,
            stoned: false,
        };
        let time = if dies.fallen {
            0.0
        } else {
            undead::vanishes_after(detail)
        };
        commands.entity(entity).insert((
            Lies {
                lain: 0.0,
                time,
                body,
                spider: corpse.creature == undead::MEDIUM_SPIDER,
            },
            Destroyed(undead::destroyed_after(detail)),
        ));
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
    held_at_once(corpse, on_death, talks)
        || stabled
        || trades.is_some_and(|trades| trades.0 & TRADER != 0)
        || rises
}

/// What holds a body that ends at once (`0x5404C7`, `0x5407A6`, `0x5409F5`, `0x540D16`): its
/// flags word's `0x2000`, a death's function that still stands for it, or a dialogue from 1 to
/// 59,999, which the stables' and the trader's stamps are not.
fn held_at_once(corpse: &Corpse, on_death: Option<&OnDeath>, talks: bool) -> bool {
    corpse.flags & LIES_ON != 0 || on_death.is_some_and(|on_death| on_death.0.holds()) || talks
}

/// Each body at its ticks of lying (`0x53ECA0`'s sub-mode 2): from its first, a medium spider's
/// goes, and one that ends at once ([`undead::at_once`]) does unless held, in its 781 at its
/// place ([`Ends`]) and hidden; another, once it has lain its time and nothing holds it, vanishes
/// (`0x54140A`): its figure in the soul's sparks rising where it lies (780 by the FX event `0xC`,
/// `0x541491`). Each is hidden and out of the world, its destruction to come
/// (`undead::destroy`).
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
        (Option<&OnDeath>, Option<&Effects>),
        (
            Has<Talk>,
            Has<crate::stables::Stabled>,
            Option<&Trades>,
            Has<RisesAgain>,
            Has<crate::followers::Follower>,
        ),
    )>,
) {
    for (
        entity,
        mut lies,
        corpse,
        at,
        (on_death, effects),
        (talks, stabled, trades, rises, follows),
    ) in &mut lying
    {
        lies.lain += time.delta_secs();
        if lies.lain < undead::FIRST_LYING_TICK {
            continue;
        }
        // A medium spider's goes at its first tick of lying, as it is (`0x54045E`).
        if lies.spider {
            debug!("bodies: {entity}, a {}, goes at once", corpse.creature);
            commands.entity(entity).remove::<Lies>();
            gone(&mut commands, entity);
            continue;
        }
        let at = at.translation();
        let body = Lying {
            kept: follows || rises || talks || on_death.is_some_and(|on_death| on_death.0.holds()),
            stoned: effects
                .is_some_and(|effects| effects.holds_any(&[opensacred_rules::spells::STONE])),
            ..lies.body
        };
        if let Some(end) = undead::at_once(&body) {
            if held_at_once(corpse, on_death, talks) {
                continue;
            }
            debug!(
                "bodies: {entity}, a {}, ends at once: {end:?}",
                corpse.creature
            );
            let burst = match end {
                AtOnce::Hidden => None,
                AtOnce::Smoke => Some(BodyEnd::Smoke),
                AtOnce::Sparks => Some(BodyEnd::Sparks),
                AtOnce::Stone => Some(BodyEnd::Stone),
            };
            if let Some(burst) = burst {
                ends(&mut commands, entity, at, burst);
            }
            commands.entity(entity).remove::<Lies>();
            gone(&mut commands, entity);
            continue;
        }
        if lies.lain < lies.time || held(corpse, on_death, (talks, stabled, trades), rises) {
            continue;
        }
        debug!("bodies: {entity}, a {}, vanishes", corpse.creature);
        commands.spawn((
            Transform::from_translation(at),
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

/// A 781 to burst at a body's vertices as it ends ([`burst`]): the body and the variant.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct Ends {
    from: Entity,
    end: BodyEnd,
}

/// Makes a 781 of the variant `end` at the place `at` of the body `from` (`0x5FBA40(0x30D)` and
/// its FX event `0xD`), which bursts at its vertices ([`burst`]).
pub(crate) fn ends(commands: &mut Commands, from: Entity, at: Vec3, end: BodyEnd) {
    commands.spawn((
        Transform::from_translation(at),
        Ends { from, end },
        DespawnOnExit(AppState::InGame),
    ));
}

/// Bursts each new 781 at its body's vertices as last posed (`0x76D260`: `0x44FC30`,
/// `0x767FB0`), every `⌊V/N⌋`th of its meshes but its worn items' up to its count `N`
/// (`opensacred_rules::particles::body_end`, `vertices`), drawn by `puffs`; none where the body
/// has no meshes.
fn burst(
    mut commands: Commands,
    bursting: Query<(Entity, &Ends, &Transform), Added<Ends>>,
    shapes: crate::vertices::ModelPoints,
    (meshes, mut dice): (Res<Assets<Mesh>>, ResMut<Dice>),
) {
    for (marker, ends, place) in &bursting {
        let origin = place.translation;
        let points: Vec<[f32; 3]> = shapes
            .picked_at_most(ends.from, &meshes, ends.end.count())
            .into_iter()
            .map(|point| crate::puffs::local(point, origin))
            .collect();
        debug!(
            "bodies: {} bursts in {} of {:?}",
            ends.from,
            points.len(),
            ends.end
        );
        crate::puffs::spawn(
            &mut commands,
            particles::body_end(ends.end, &points, &mut dice.0),
            origin,
            None,
        );
        commands.entity(marker).despawn();
    }
}

/// The bodies' lying and vanishing.
pub struct BodiesPlugin;

impl Plugin for BodiesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>().add_systems(
            Update,
            (lie, vanish, burst)
                .chain()
                .after(Step::Npcs)
                .in_set(Playing),
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

    #[test]
    fn the_raised_dead_the_crow_the_stone_and_the_dummy_end_at_their_first_tick_unless_held() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .add_systems(Update, (lie, vanish).chain());
        let spawn = |app: &mut App, creature, flags| {
            app.world_mut()
                .spawn((
                    Corpse {
                        creature,
                        ..corpse(flags)
                    },
                    Dies::new(0),
                    GlobalTransform::default(),
                ))
                .id()
        };
        let raised = spawn(&mut app, 76, undead::RAISED_DEAD);
        let crow = spawn(&mut app, undead::CROW, 0);
        let stoned = spawn(&mut app, 76, 0);
        let mut stone = Effects::default();
        stone.add(
            opensacred_rules::fight::Effect {
                id: opensacred_rules::spells::HELD,
                whole: 0,
                value: 0.0,
                art: u32::from(opensacred_rules::spells::STONE),
                level: 0,
            },
            5.0,
        );
        app.world_mut().entity_mut(stoned).insert(stone);
        let dummy = spawn(&mut app, undead::DUMMY, 0);
        let named = spawn(&mut app, undead::CROW, LIES_ON);
        let goblin = spawn(&mut app, 76, 0);
        // 0.3 or 0.4 s, past the first tick of lying.
        for _ in 0..5 {
            app.update();
        }
        for gone in [raised, crow, stoned, dummy] {
            assert!(app.world().get::<Corpse>(gone).is_none(), "{gone}");
            assert!(app.world().get::<Destroyed>(gone).is_some(), "{gone}");
        }
        assert!(app.world().get::<Corpse>(named).is_some());
        assert!(app.world().get::<Corpse>(goblin).is_some());
        let mut ends = app.world_mut().query::<&Ends>();
        let made: Vec<(Entity, BodyEnd)> = ends
            .iter(app.world())
            .map(|ends| (ends.from, ends.end))
            .collect();
        let wanted = [
            (raised, BodyEnd::Smoke),
            (crow, BodyEnd::Sparks),
            (stoned, BodyEnd::Stone),
        ];
        assert_eq!(made.len(), wanted.len(), "{made:?}");
        assert!(wanted.iter().all(|end| made.contains(end)), "{made:?}");
        let mut sparks = app.world_mut().query::<&crate::soul_sparks::Sparks>();
        assert_eq!(sparks.iter(app.world()).count(), 0);
    }
}
