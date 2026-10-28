//! The undead that rise again (#1199, `opensacred_rules::undead`; docs/re/fight.md, "The undead
//! that rise again"): a magic skeleton or an undead goblin that falls to a strike, not risen yet
//! nor a boss or a champion, rises in most cases (`0x52E590` at `0x532BD6`), its first death
//! giving no drop, no experience and no kill; it lies 4 s and stands again where it fell with a
//! half, a third or a quarter of its most health, and its next death is its last
//! (`0x53ECA0`'s cases 3 and 5). A magic skeleton that rises drains a lower killer's health as it
//! falls. A skeleton warrior that rises changes its type as it falls, the body of the new type,
//! an arm or the head lost, playing its dying clip in its place, until its last death (#1260).
//! At its last death a creature of the class 5 plays no dying clip: the magic skeletons and the
//! undead goblins burn away a moment later, `DIEFLAMES` at them, the skeleton warriors, the
//! zombies 33 and 34 and the ghouls 36 and 50 burst at once with `CHAR_SPLASH`, and the others
//! vanish; the fire ghoul burns away once its clip has played (`0x541653`'s cases). Their
//! bodies are hidden. The effects 781 and 856, the burst's parts and the warriors' thrown parts
//! are not drawn yet (#1261, #1262, #1264).

use bevy::prelude::*;
use opensacred_rules::undead::{self, End, LIES, RISEN};

use crate::ai::Mind;
use crate::crowd::InTheWay;
use crate::fight::{Health, Rank};
use crate::frame::{Playing, Step};
use crate::npcs::{Corpse, Dies, Npc, OnDeath, ScriptName, Talk, Trades};
use crate::sfx::{DIES_IN_FLAMES, Of, Play, Roll, SPLASH};
use crate::spawning::Rng;
use crate::terrain::{Heights, Level};

/// How long after its last death an undead burns away, in seconds (seen: 0.23 to 0.33 s after
/// the kill, the death action's first steps).
const BURNS: f32 = 0.25;

/// An undead that lies dead before it rises ([`rise`]): whether it was hostile to the hero, the
/// seconds it still lies, and the type that a skeleton warrior changes into as it falls.
#[derive(Component, Clone, Copy, Debug)]
pub struct Rises {
    hostile: bool,
    lies: f32,
    into: Option<u32>,
}

impl Rises {
    /// The type that a skeleton warrior changes into as it falls ([`change`]).
    pub fn changes_into(&self) -> Option<u32> {
        self.into
    }
}

/// An undead at its last death, the seconds before it burns away ([`burn`]).
#[derive(Component, Clone, Copy, Debug)]
struct Burns(f32);

/// A fire ghoul that burns away once its dying clip has played ([`burn_after_clip`]).
#[derive(Component, Clone, Copy, Debug)]
struct BurnsAfterClip;

/// The undead's dice for the health they rise with.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(Rng::of(0, 0, 0x1199))
    }
}

/// Whether `mind`'s creature of `rank`, falling to a strike (`plain`: no art's nor shot's),
/// rises again, by a roll of `R(0, 100)` on `dice`, and, a skeleton warrior, the type it changes
/// into, by two draws of `rand() % 3`: no boss nor champion, which its first fight marks as a
/// rise does (`0x532E5A`, `0x532E8D`); a magic skeleton or an undead goblin not risen yet, or a
/// skeleton warrior that has a part to lose (`0x532E3C`'s cases, #1260; **hypothesis** that the
/// warriors' bosses and champions do not change either).
pub fn rolls(dice: &mut Rng, mind: &Mind, rank: Rank, plain: bool) -> Option<Rises> {
    let kind = mind.creature;
    let warrior = undead::warrior(kind);
    if rank != Rank::Common || !(warrior || undead::rises(kind) && mind.flags & RISEN == 0) {
        return None;
    }
    if !undead::rolls(dice.below(101), undead::chance(plain)) {
        return None;
    }
    let into = if warrior {
        Some(undead::changes(kind, [dice.below(3), dice.below(3)])?)
    } else {
        None
    };
    Some(Rises {
        hostile: mind.hostile(),
        lies: LIES,
        into,
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

/// Makes a creature of the type `kind` and the level `level`, hostile as `hostile`, on the cell
/// of `old` on the level `on`, with `old`'s names, dialogue, trades and death's functions, and
/// takes `old` away: the creatures made, none where `old` is gone.
fn remake(
    world: &mut World,
    old: Entity,
    (kind, level, hostile): (u32, u32, bool),
    on: Level,
) -> Vec<Entity> {
    let Some(at) = world.get::<Transform>(old).map(|at| at.translation) else {
        return Vec::new();
    };
    let Some(heights) = world.get_resource::<Heights>() else {
        return Vec::new();
    };
    let floor = heights.floor(on).map_or(0, |(_, floor)| i32::from(floor));
    let (x, y) = crate::paths::cell_of(at.x, at.z);
    let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
        return Vec::new();
    };
    let kept = (
        world.get::<ScriptName>(old).cloned(),
        world.get::<Talk>(old).cloned(),
        world.get::<Trades>(old).copied(),
        world.get::<OnDeath>(old).cloned(),
    );
    world.entity_mut(old).despawn();
    let request = crate::npcs::Spawn {
        creature: kind,
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
            warn!("undead: no {kind} in the place of {old}: {e}");
            return Vec::new();
        }
        Err(e) => {
            warn!("undead: no {kind} in the place of {old}: {e}");
            return Vec::new();
        }
    };
    for &entity in &made {
        let Ok(mut new) = world.get_entity_mut(entity) else {
            continue;
        };
        let (name, talk, trades, on_death) = kept.clone();
        if let Some(name) = name {
            new.insert(name);
        }
        if let Some(talk) = talk {
            new.insert(talk);
        }
        if let Some(trades) = trades {
            new.insert(trades);
        }
        if let Some(on_death) = on_death {
            new.insert(on_death);
        }
    }
    made
}

/// Raises the undead `corpse` (`0x541E8D`, `0x541A0D`): the corpse goes and a creature of its
/// type and level stands on its cell, hostile as it was, with its names, its dialogue, its trades
/// and its death's functions, a half, a third or a quarter of its most health and the mark that
/// its next death is its last.
fn stand(world: &mut World, corpse: Entity, hostile: bool) {
    let (Some(dead), Some(level)) = (
        world.get::<Corpse>(corpse).copied(),
        world.get::<Health>(corpse).map(|health| health.level),
    ) else {
        return;
    };
    for entity in remake(world, corpse, (dead.creature, level, hostile), dead.level) {
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
    }
}

/// Changes the skeleton warrior `fallen`, which falls and `rises`, into the type of the change
/// (`0x532E3C`'s cases 0 and 2 to 5, #1260): a body of that type, which lacks an arm or the head,
/// takes its place on its cell and plays its dying clip `choice` (a death at a creature's hands
/// where `by_creature`), to lie and rise as [`rise`] raises it; the part it throws (`0x550580`) is
/// not made.
pub fn change(
    world: &mut World,
    fallen: Entity,
    rises: Rises,
    (choice, by_creature): (u32, bool),
) {
    let Some(into) = rises.into else {
        return;
    };
    let (Some(level), Some(on)) = (
        world.get::<Health>(fallen).map(|health| health.level),
        world.get::<InTheWay>(fallen).map(|way| way.level),
    ) else {
        return;
    };
    let kind = world.get::<Mind>(fallen).map_or(0, |mind| mind.creature);
    for entity in remake(world, fallen, (into, level, rises.hostile), on) {
        let Ok(mut changed) = world.get_entity_mut(entity) else {
            continue;
        };
        if let Some(mut health) = changed.get_mut::<Health>() {
            health.current = 0;
        }
        let mut dies = if by_creature {
            Dies::by_creature(choice)
        } else {
            Dies::new(choice)
        };
        dies.rises = true;
        changed.insert((dies, rises));
        info!("undead: {fallen}, a {kind}, falls as {entity}, a {into}");
    }
}

/// What becomes of each creature of the class 5 that lies at its last death (`0x541653`'s cases,
/// #1260): an undead that rises burns away a moment later ([`burn`]), a skeleton warrior, a
/// zombie or a ghoul bursts at once, with `CHAR_SPLASH` at it (`0x4E8DA0`), and any other
/// vanishes at once; the fire ghoul burns away once its clip has played ([`burn_after_clip`]).
/// One that rises lies on.
fn last(
    mut commands: Commands,
    lying: Query<(Entity, &Corpse, &Dies), Added<Corpse>>,
    mut sounds: MessageWriter<Play>,
) {
    for (entity, corpse, dies) in &lying {
        if dies.rises {
            continue;
        }
        if corpse.creature == undead::FIRE_GHOUL {
            commands.entity(entity).insert(BurnsAfterClip);
            continue;
        }
        if corpse.class != undead::CLASS {
            continue;
        }
        match undead::end(corpse.creature) {
            End::Burns => {
                commands.entity(entity).insert(Burns(BURNS));
            }
            End::Bursts => {
                debug!("undead: {entity}, a {}, bursts", corpse.creature);
                sounds.write(Play {
                    sound: Of::Id(SPLASH),
                    at: Some(entity),
                    rate: (0.5, Roll::None),
                });
                gone(&mut commands, entity);
            }
            End::Vanishes => {
                debug!("undead: {entity}, a {}, vanishes", corpse.creature);
                gone(&mut commands, entity);
            }
        }
    }
}

/// Hides the body of `entity` and leaves no corpse to raise (`+0x14` bit `0x10000000`, out of
/// its sector's lists).
fn gone(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<(Burns, BurnsAfterClip, Corpse)>()
        .insert(Visibility::Hidden);
}

/// Burns each undead away whose moment came (`0x542015`, `0x541B95`): `DIEFLAMES` at it, and its
/// body gone ([`gone`]).
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
        gone(&mut commands, entity);
    }
}

/// Burns each fire ghoul away once its dying clip has played (`0x541653`'s case 4): its body
/// gone; its `DIEFLAMES` sounded as it died (`sfx`).
fn burn_after_clip(
    mut commands: Commands,
    burning: Query<(Entity, &Npc, &Dies), With<BurnsAfterClip>>,
    players: Query<&AnimationPlayer>,
) {
    for (entity, npc, dies) in &burning {
        if npc.died(dies.choice, &players) {
            debug!("undead: {entity}, a fire ghoul, burns away");
            gone(&mut commands, entity);
        }
    }
}

/// The undead's rise, change, burn, burst and vanishing.
pub struct UndeadPlugin;

impl Plugin for UndeadPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>().add_systems(
            Update,
            (last, rise, burn, burn_after_clip)
                .chain()
                .after(Step::Npcs)
                .in_set(Playing),
        );
    }
}
