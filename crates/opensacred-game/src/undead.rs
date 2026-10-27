//! The undead that rise again (#1199, `opensacred_rules::undead`; docs/re/fight.md, "The undead
//! that rise again"): a magic skeleton or an undead goblin that falls to a strike, not risen yet
//! nor a boss or a champion, rises in most cases (`0x52E590` at `0x532BD6`), its first death
//! giving no drop, no experience and no kill; it lies 4 s and stands again where it fell with a
//! half, a third or a quarter of its most health, and its next death is its last
//! (`0x53ECA0`'s cases 3 and 5). A magic skeleton that rises drains a lower killer's health as it
//! falls. At its last death it plays no dying clip and burns away a moment later, `DIEFLAMES` at
//! it, its body gone. The effects 781 of the rise and 856 of the burn are not drawn yet.

use bevy::prelude::*;
use opensacred_rules::undead::{self, LIES, RISEN};

use crate::ai::Mind;
use crate::fight::{Health, Rank};
use crate::frame::{Playing, Step};
use crate::npcs::{Corpse, Dies, OnDeath, ScriptName, Talk, Trades};
use crate::sfx::{DIES_IN_FLAMES, Of, Play, Roll};
use crate::spawning::Rng;
use crate::terrain::Heights;

/// How long after its last death an undead burns away, in seconds (seen: 0.23 to 0.33 s after
/// the kill, the death action's first steps).
const BURNS: f32 = 0.25;

/// An undead that lies dead before it rises ([`rise`]): whether it was hostile to the hero, and
/// the seconds it still lies.
#[derive(Component, Clone, Copy, Debug)]
pub struct Rises {
    hostile: bool,
    lies: f32,
}

/// An undead at its last death, the seconds before it burns away ([`burn`]).
#[derive(Component, Clone, Copy, Debug)]
struct Burns(f32);

/// The undead's dice for the health they rise with.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(Rng::of(0, 0, 0x1199))
    }
}

/// Whether `mind`'s creature of `rank`, falling to a strike (`plain`: no art's nor shot's),
/// rises again, by a roll of `R(0, 100)` on `dice`: of a type that rises, not risen yet, and no
/// boss nor champion, which its first fight marks as a rise does (`0x532E5A`, `0x532E8D`).
pub fn rolls(dice: &mut Rng, mind: &Mind, rank: Rank, plain: bool) -> Option<Rises> {
    if !undead::rises(mind.creature) || mind.flags & RISEN != 0 || rank != Rank::Common {
        return None;
    }
    undead::rolls(dice.below(101), undead::chance(plain)).then(|| Rises {
        hostile: mind.hostile(),
        lies: LIES,
    })
}

/// A magic skeleton of `(kind, level)` that rises drains `killer` of the level `by`
/// (`0x532E8D`, `opensacred_rules::undead::drained`); the beam it draws is not.
pub fn drain(commands: &mut Commands, killer: Entity, (kind, level): (u32, u32), by: u32) {
    commands
        .entity(killer)
        .entry::<Health>()
        .and_modify(move |mut health| {
            let drained = undead::drained(kind, level, by, health.current);
            if drained > 0 {
                health.current -= drained;
                info!("undead: the {kind} of level {level} drains {killer} of {drained}");
            }
        });
}

/// Each undead that lies its time stands again ([`stand`]).
fn rise(
    mut commands: Commands,
    time: Res<Time>,
    mut lying: Query<(Entity, &mut Rises), With<Corpse>>,
) {
    for (corpse, mut rises) in &mut lying {
        rises.lies -= time.delta_secs();
        if rises.lies > 0.0 {
            continue;
        }
        let hostile = rises.hostile;
        commands.entity(corpse).remove::<Rises>();
        commands.queue(move |world: &mut World| stand(world, corpse, hostile));
    }
}

/// Raises the undead `corpse` (`0x541E8D`, `0x541A0D`): the corpse goes and a creature of its
/// type and level stands on its cell, hostile as it was, with its names, its dialogue, its trades
/// and its death's functions, a half, a third or a quarter of its most health and the mark that
/// its next death is its last.
fn stand(world: &mut World, corpse: Entity, hostile: bool) {
    let (Some(at), Some(dead), Some(level)) = (
        world.get::<Transform>(corpse).map(|at| at.translation),
        world.get::<Corpse>(corpse).copied(),
        world.get::<Health>(corpse).map(|health| health.level),
    ) else {
        return;
    };
    let Some(heights) = world.get_resource::<Heights>() else {
        return;
    };
    let floor = heights
        .floor(dead.level)
        .map_or(0, |(_, floor)| i32::from(floor));
    let (x, y) = crate::paths::cell_of(at.x, at.z);
    let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
        return;
    };
    let kept = (
        world.get::<ScriptName>(corpse).cloned(),
        world.get::<Talk>(corpse).cloned(),
        world.get::<Trades>(corpse).copied(),
        world.get::<OnDeath>(corpse).cloned(),
    );
    world.entity_mut(corpse).despawn();
    let request = crate::npcs::Spawn {
        creature: dead.creature,
        cells: vec![(x, y)],
        floor,
        level: Some(level),
        rank: Rank::Common,
        hostile: Some(hostile),
        immortal: false,
        roams: false,
    };
    let made = match world.run_system_cached_with(crate::npcs::spawn_creatures, request) {
        Ok(Ok(made)) => made,
        Ok(Err(e)) => {
            warn!("undead: {corpse} does not rise: {e}");
            return;
        }
        Err(e) => {
            warn!("undead: {corpse} does not rise: {e}");
            return;
        }
    };
    for entity in made {
        let roll = world
            .get_resource_mut::<Dice>()
            .map_or(0, |mut dice| dice.0.below(3));
        let Ok(mut risen) = world.get_entity_mut(entity) else {
            continue;
        };
        if let Some(mut mind) = risen.get_mut::<Mind>() {
            mind.flags |= RISEN;
        }
        if let Some(mut health) = risen.get_mut::<Health>() {
            health.current = undead::risen_health(health.most, roll).max(1);
            info!(
                "undead: {corpse}, a {}, rises as {entity} at {}/{}",
                dead.creature, health.current, health.most
            );
        }
        let (name, talk, trades, on_death) = kept.clone();
        if let Some(name) = name {
            risen.insert(name);
        }
        if let Some(talk) = talk {
            risen.insert(talk);
        }
        if let Some(trades) = trades {
            risen.insert(trades);
        }
        if let Some(on_death) = on_death {
            risen.insert(on_death);
        }
    }
}

/// An undead that lies at its last death burns away a moment later ([`burn`]); one that rises
/// lies on.
fn last(mut commands: Commands, lying: Query<(Entity, &Corpse, &Dies), Added<Corpse>>) {
    for (entity, corpse, dies) in &lying {
        if undead::rises(corpse.creature) && !dies.rises {
            commands.entity(entity).insert(Burns(BURNS));
        }
    }
}

/// Burns each undead away whose moment came (`0x542015`, `0x541B95`): `DIEFLAMES` at it, and its
/// body hidden and no corpse to raise (`+0x14` bit `0x10000000`, out of its sector's lists).
fn burn(
    mut commands: Commands,
    time: Res<Time>,
    mut burning: Query<(Entity, &mut Burns)>,
    mut sounds: MessageWriter<Play>,
) {
    for (entity, mut burns) in &mut burning {
        burns.0 -= time.delta_secs();
        if burns.0 > 0.0 {
            continue;
        }
        debug!("undead: {entity} burns away");
        sounds.write(Play {
            sound: Of::Id(DIES_IN_FLAMES),
            at: Some(entity),
            rate: (0.5, Roll::None),
        });
        commands
            .entity(entity)
            .remove::<(Burns, Corpse)>()
            .insert(Visibility::Hidden);
    }
}

/// The undead's rise and burn.
pub struct UndeadPlugin;

impl Plugin for UndeadPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>().add_systems(
            Update,
            (last, rise, burn).chain().after(Step::Npcs).in_set(Playing),
        );
    }
}
