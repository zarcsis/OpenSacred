//! The bodies' end (#2501; docs/re/fight.md, "The bodies' end"): any creature's body lies 20, 5
//! or 2 s after its death by the options' detail (`opensacred_rules::undead::vanishes_after`,
//! `0x54101B`), then vanishes at once, its figure in pale green sparks rising where it lay (780
//! homing on nothing, `soul_sparks`), and is destroyed 25, 10 or 7 s after its death
//! (`undead::destroy`); time alone counts, not the hero's distance nor the bodies' number, but a
//! death beside the Dark Elf's soulcatcher, which puts its stamp back by 18, 3 or 1 s ([`PutBack`],
//! `0x53E889`, #3387). A body
//! lies on while its flags word holds `0x2000` (the tag `0x45`), a death's function still stands
//! for it, it has a dialogue, a stable's stamp or the trader's, or it rises again
//! (`npcs::RisesAgain`), and vanishes at the first check once the hold ends (`0x54140A`). Some
//! vanish at their first tick of lying instead (#2514, `undead::at_once`): the Vampiress's raised
//! dead in black smoke, the ghosts, the visions, the crow, the types without a dying clip and the
//! bodies on water or lava in blue-white sparks, the bodies turned to stone in pale green dust
//! (781 at their vertices, [`Ends`], its dusts drifting with the renderer's wind of their start,
//! `daynight::Clock::wind`, #3154), the dummy with no effect, unless their flag, a death's
//! function or a dialogue holds them. The undead of the class 5 and the bodies that burst end
//! their own ways (`undead`). With the violence on, a body that bleeds lays a blood puddle under
//! it at its first tick of lying past its dying clip's end (907, `marks::puddle`, #2516, #3082),
//! the hero's too (`hero::fall`); a big demon's makes its death there instead, the violence on or
//! off (919, `demon_deaths`, #2537).

use bevy::prelude::*;
use opensacred_rules::particles::{self, BodyEnd};
use opensacred_rules::quests::TRADER;
use opensacred_rules::undead::{self, AtOnce, LIES_ON, Lying};

use crate::effects::Effects;
use crate::frame::{Playing, Step};
use crate::game::AppState;
use crate::npcs::{Corpse, Dies, Npc, Npcs, OnDeath, RisesAgain, Talk, Trades, Vision};
use crate::spawning::Rng;
use crate::undead::{Bursts, Destroyed, gone};

/// A body that lies: how long it has lain and how long it lies before it vanishes in its soul,
/// what it is for its end at its first tick of lying ([`undead::at_once`]; what keeps it from the
/// liquids' end and its stone tested at each tick), whether it then goes at once with no soul
/// and whatever holds it, a medium spider's (#2504), and whether it lays a puddle at the first
/// tick of lying past its dying clip's end (#2516, #3082).
#[derive(Component, Clone, Copy, Debug)]
struct Lies {
    lain: f32,
    time: f32,
    body: Lying,
    spider: bool,
    puddle: bool,
    /// Whether it makes a big demon's death then (#2537).
    demon: bool,
}

/// A body's times of lying as a save keeps them (#3762): how long it has lain, when it
/// vanishes, and how long it still waits to be destroyed (`undead::Destroyed`).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedLies {
    lain: f32,
    time: f32,
    destroyed: Option<f32>,
}

/// The times of lying of the body `entity`, if it lies.
pub(crate) fn saved_lies(world: &World, entity: Entity) -> Option<SavedLies> {
    let lies = world.get::<Lies>(entity)?;
    Some(SavedLies {
        lain: lies.lain,
        time: lies.time,
        destroyed: world.get::<Destroyed>(entity).map(|destroyed| destroyed.0),
    })
}

/// A loaded save's body's times of lying, which its first tick of lying takes in place of its
/// death's ([`lie`], #3762).
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct Resumed(pub(crate) SavedLies);

/// A body that a script's `SetNPCState` of `0x80` lets go (`0x46224D`–`0x4622B6`; docs/re/
/// netscript.md, "`SetNPCState`", #3445): its flags word's hold cleared and its death's stamp put
/// back by 19, 4 or 1 s at the detail 2, 1 or 0, so that it vanishes a second later and is
/// destroyed six seconds later, whatever the detail ([`release`]).
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct Released;

/// How long after its release a body vanishes, and is destroyed: its stamp put back by its
/// detail's vanishing less one second.
const RELEASED_VANISHES: f32 = 1.0;
const RELEASED_DESTROYED: f32 = 6.0;

/// Lets go each body that a script released ([`Released`]): its flags word's `0x2000` cleared,
/// its time to vanish a second from now and to be destroyed six; one not lying yet keeps the mark
/// until it lies.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn release(
    mut commands: Commands,
    mut released: Query<
        (
            Entity,
            &mut Corpse,
            Option<&mut Lies>,
            Option<&mut Destroyed>,
        ),
        With<Released>,
    >,
) {
    for (entity, mut corpse, lies, destroyed) in &mut released {
        corpse.flags &= !LIES_ON;
        let Some(mut lies) = lies else {
            continue;
        };
        lies.time = lies.lain + RELEASED_VANISHES;
        if let Some(mut destroyed) = destroyed {
            destroyed.0 = destroyed.0.min(RELEASED_DESTROYED);
        }
        debug!("bodies: {entity}'s body is let go, to vanish in a second");
        commands.entity(entity).remove::<Released>();
    }
}

/// The seconds that a death beside the Dark Elf's soulcatcher put its stamp back
/// (`opensacred_rules::undead::soulcatcher_back`, `npcs::die`, #3387), by which its body vanishes,
/// is destroyed and rises that much sooner (`undead`).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(crate) struct PutBack(pub(crate) f32);

impl PutBack {
    /// The seconds that `put_back` puts a time counted from the death back, none without one.
    pub(crate) fn of(put_back: Option<&Self>) -> f32 {
        put_back.map_or(0.0, |back| back.0)
    }
}

/// A body to lay its blood puddle under ([`puddle`]), of its creature's type, whose gib set
/// colours it: an NPC's at its first tick of lying, the hero's as its dying clip ends
/// (`hero::fall`, #3082).
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct Puddles(pub(crate) u32);

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
/// lying, which its dialogue held as it went (#2457), has lain its time already and lays no
/// puddle again (`+0x120`, kept in the saves). With the violence on (`0x819FD0`), a body that
/// does not rise again (`+0x2B7` bit 8) and that bleeds (`opensacred_rules::undead::puddles`)
/// lays its puddle at its first tick of lying past its dying clip's end (`0x53FC5C`, #3082).
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
            (Option<&Npc>, Option<&PutBack>, Option<&Resumed>),
            (Has<Bursts>, Has<RisesAgain>, Has<Vision>),
        ),
        Added<Corpse>,
    >,
    settings: Option<Res<crate::settings::Settings>>,
    (npcs, heights, gibs): (
        Option<Res<Npcs>>,
        Option<Res<crate::terrain::Heights>>,
        Option<Res<crate::blood_fountains::Gibs>>,
    ),
) {
    let detail = crate::settings::detail(settings.as_deref());
    let violence = settings
        .as_deref()
        .is_some_and(|settings| settings.gameplay.violence);
    for (entity, corpse, dies, at, (npc, put_back, resumed), (bursts, rises, vision)) in &lying {
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
            vision,
            falls: npc.is_none_or(Npc::falls),
            nature: heights
                .as_deref()
                .and_then(|heights| heights.nature(at.x, at.z)),
            kept: false,
            stoned: false,
        };
        let back = PutBack::of(put_back);
        let time = if dies.fallen {
            0.0
        } else {
            undead::vanishes_after(detail) - back
        };
        // A loaded save's body lies on for what it had left (#3762).
        let (lain, time, destroyed) = match resumed {
            Some(Resumed(saved)) => {
                commands.entity(entity).remove::<Resumed>();
                (
                    saved.lain,
                    saved.time,
                    saved
                        .destroyed
                        .unwrap_or(undead::destroyed_after(detail) - back),
                )
            }
            None => (0.0, time, undead::destroyed_after(detail) - back),
        };
        let set = gibs.as_deref().map_or(0, |gibs| gibs.of(corpse.creature));
        commands.entity(entity).insert((
            Lies {
                lain,
                time,
                body,
                spider: corpse.creature == undead::MEDIUM_SPIDER,
                puddle: violence && !dies.fallen && !rises && undead::puddles(corpse.creature, set),
                demon: !dies.fallen && undead::BIG_DEMONS.contains(&corpse.creature),
            },
            Destroyed(destroyed),
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
/// (`0x54140A`): its soul leaves where it lies (780 by the FX event `0xC`, `0x541491`;
/// `souls::Vanished`). Each is hidden and out of the world, its destruction to come
/// (`undead::destroy`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn vanish(
    mut commands: Commands,
    mut vanished: MessageWriter<crate::souls::Vanished>,
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
        (Option<&Npc>, Option<&Dies>),
    )>,
    players: Query<&AnimationPlayer>,
) {
    for (
        entity,
        mut lies,
        corpse,
        at,
        (on_death, effects),
        (talks, stabled, trades, rises, follows),
        (npc, dies),
    ) in &mut lying
    {
        lies.lain += time.delta_secs();
        if lies.lain < undead::FIRST_LYING_TICK {
            continue;
        }
        // Its puddle first, whatever becomes of it, once its dying clip has ended (`0x53FC5C`,
        // before `0x540037`): none where it ends at once before.
        let ended = match (npc, dies) {
            (Some(npc), Some(dies)) => crate::npcs::dying_ended(npc, dies, &players),
            _ => true,
        };
        if lies.puddle && ended {
            lies.puddle = false;
            commands.entity(entity).insert(Puddles(corpse.creature));
        }
        // A big demon's death in its puddle's place, with the violence on or off (`0x53FC5C`).
        if lies.demon {
            lies.demon = false;
            crate::demon_deaths::spawn(&mut commands, entity, at.translation());
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
        vanished.write(crate::souls::Vanished { body: entity, at });
        commands.entity(entity).remove::<Lies>();
        gone(&mut commands, entity);
    }
}

/// Lays each new puddle (`0x53FCA4`–`0x53FE25`): over its body's bone that its fountain spurted
/// from, else `Bip01 Spine2` (`opensacred_rules::undead::PUDDLE_BONE`), on the ground there
/// (`0x636520` of 0), of one of its gib set's eight textures (`0x4273C0`); none where the bone or
/// the ground is not found. A body on a building's floor lays it on the ground under the floor
/// too, in list 0 under the floor's sprites, where it does not show (#3285).
fn puddle(
    mut commands: Commands,
    puddles: Query<(Entity, &Puddles, Option<&crate::blood_fountains::Bled>), Added<Puddles>>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (heights, gibs, mut dice): (
        Option<Res<crate::terrain::Heights>>,
        Option<Res<crate::blood_fountains::Gibs>>,
        ResMut<Dice>,
    ),
) {
    for (entity, &Puddles(creature), bled) in &puddles {
        commands.entity(entity).remove::<Puddles>();
        let name = bled.map_or(undead::PUDDLE_BONE, |bled| bled.0);
        let at = crate::making::bone_entity(entity, &[name], &children, (&bones, &worn))
            .and_then(|bone| bones.get(bone).ok())
            .map(|(_, place)| place.translation());
        let ground = at.and_then(|at| {
            heights
                .as_deref()?
                .standing(at.x, at.z, None)
                .map(|y| Vec3::new(at.x, y, at.z))
        });
        let set = gibs.as_deref().map_or(0, |gibs| gibs.of(creature));
        let (Some(place), Some(texture)) = (ground, undead::puddle_texture(set, &mut dice.0))
        else {
            continue;
        };
        debug!("bodies: {entity}, a {creature}, lays a puddle of {texture} under its {name}");
        crate::marks::puddle(&mut commands, place, texture);
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
    (meshes, mut dice, clock): (
        Res<Assets<Mesh>>,
        ResMut<Dice>,
        Option<Res<crate::daynight::Clock>>,
    ),
) {
    let wind = clock
        .as_deref()
        .map_or([0.0; 3], crate::daynight::Clock::wind);
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
            particles::body_end(ends.end, &points, wind, &mut dice.0),
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
            (lie, release, vanish, puddle, burst)
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

    /// The bodies that vanished, as their souls were sent for.
    #[derive(Resource, Default)]
    struct Seen(Vec<Entity>);

    fn seen(mut vanished: MessageReader<crate::souls::Vanished>, mut seen: ResMut<Seen>) {
        seen.0.extend(vanished.read().map(|vanished| vanished.body));
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
            .add_message::<crate::souls::Vanished>()
            .init_resource::<Seen>()
            .add_systems(Update, (lie, vanish, seen).chain());
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
        assert_eq!(app.world().resource::<Seen>().0, [body]);
        assert!(app.world().get::<Corpse>(named).is_some());
    }

    #[test]
    fn a_held_body_that_a_script_lets_go_vanishes_a_second_later() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                250,
            )))
            .add_message::<crate::souls::Vanished>()
            .init_resource::<Seen>()
            .add_systems(Update, (lie, release, vanish, seen).chain());
        let held = app
            .world_mut()
            .spawn((corpse(LIES_ON), Dies::new(0), GlobalTransform::default()))
            .id();
        // Held past its 20 s, some 30 s on.
        for _ in 0..120 {
            app.update();
        }
        assert!(app.world().get::<Corpse>(held).is_some());
        app.world_mut().entity_mut(held).insert(Released);
        app.update();
        assert_eq!(
            app.world().get::<Corpse>(held).map(|corpse| corpse.flags),
            Some(0)
        );
        assert!(app.world().get::<Released>(held).is_none());
        // A second after its release, the release's own frame counted, it vanishes, and is
        // destroyed six seconds after its release.
        for _ in 0..2 {
            app.update();
        }
        assert!(app.world().get::<Corpse>(held).is_some());
        app.update();
        assert!(app.world().get::<Corpse>(held).is_none());
        assert!(
            app.world()
                .get::<Destroyed>(held)
                .is_some_and(|destroyed| destroyed.0 <= 6.0)
        );
    }

    #[test]
    fn a_body_whose_stamp_a_soulcatcher_put_back_vanishes_2_s_after_its_death() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                250,
            )))
            .add_message::<crate::souls::Vanished>()
            .init_resource::<Seen>()
            .add_systems(Update, (lie, vanish, seen).chain());
        let body = app
            .world_mut()
            .spawn((
                corpse(0),
                Dies::new(0),
                GlobalTransform::default(),
                PutBack(undead::soulcatcher_back(2)),
            ))
            .id();
        app.update();
        // Destroyed 7 s after its death.
        assert_eq!(
            app.world()
                .get::<Destroyed>(body)
                .map(|destroyed| destroyed.0),
            Some(7.0)
        );
        // 1.25 or 1.5 s: it lies.
        for _ in 0..5 {
            app.update();
        }
        assert!(app.world().get::<Corpse>(body).is_some());
        // 2.25 or 2.5 s: its soul has left.
        for _ in 0..4 {
            app.update();
        }
        assert!(app.world().get::<Corpse>(body).is_none());
        assert_eq!(app.world().resource::<Seen>().0, [body]);
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
            .add_message::<crate::souls::Vanished>()
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
    fn a_body_that_bleeds_lays_its_puddle_at_its_first_tick_with_the_violence_on() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        for violence in [true, false] {
            let mut app = App::new();
            let mut settings = crate::settings::Settings::default();
            settings.gameplay.violence = violence;
            app.add_plugins(MinimalPlugins)
                .insert_resource(settings)
                .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                    100,
                )))
                .add_message::<crate::souls::Vanished>()
                .add_systems(Update, (lie, vanish).chain());
            let goblin = app
                .world_mut()
                .spawn((corpse(0), Dies::new(0), GlobalTransform::default()))
                .id();
            // A crow ends at once, its puddle laid first.
            let crow = app
                .world_mut()
                .spawn((
                    Corpse {
                        creature: undead::CROW,
                        ..corpse(0)
                    },
                    Dies::new(0),
                    GlobalTransform::default(),
                ))
                .id();
            let demon = app
                .world_mut()
                .spawn((
                    Corpse {
                        creature: undead::BIG_DEMONS[0],
                        ..corpse(0)
                    },
                    Dies::new(0),
                    GlobalTransform::default(),
                ))
                .id();
            let rises = app
                .world_mut()
                .spawn((
                    corpse(0),
                    Dies::new(0),
                    GlobalTransform::default(),
                    RisesAgain,
                ))
                .id();
            app.update();
            assert!(app.world().get::<Puddles>(goblin).is_none());
            for _ in 0..4 {
                app.update();
            }
            assert_eq!(app.world().get::<Puddles>(goblin).is_some(), violence);
            assert_eq!(app.world().get::<Puddles>(crow).is_some(), violence);
            assert!(app.world().get::<Puddles>(demon).is_none());
            // A big demon's death in its place, the violence on or off.
            let mut deaths = app
                .world_mut()
                .query_filtered::<&Transform, With<crate::demon_deaths::Unmade>>();
            assert_eq!(deaths.iter(app.world()).count(), 1);
            assert!(app.world().get::<Puddles>(rises).is_none());
        }
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
            .add_message::<crate::souls::Vanished>()
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
