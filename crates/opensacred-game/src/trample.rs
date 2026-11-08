//! The horse's trample, its override (docs/re/horses.md, "Fighting from horseback", #2302): an
//! art that may not be made on horseback, made from the saddle, reaches the horse instead
//! (`0x52FEEB`), which, its timer run out and no trample running, takes the effect 24 for its
//! seconds (`0x582D50`, [`Trampling`]) and starts the rider's timer of its regeneration
//! ([`Timer`]). At each of its moves while it runs (`0x5941A0` at `0x596E48`), the horse takes
//! the hero's near hostiles within two thirds of the reach of the two: a big one against a light
//! horse stops it and takes one strike, any other is knocked back and falls, and each is struck
//! for a fixed amount, which the trample's damage in all pays for until it is spent. Meanwhile no
//! creature stops the rider's steps (`hero::run`).

use bevy::prelude::*;
use opensacred_rules::horses;
use opensacred_rules::minds;

use crate::fight;
use crate::frame::{Step, WorldTick};
use crate::game::AppState;
use crate::hero::{self, Body};
use crate::npcs::{Dies, Npcs};
use crate::riding::{Horse, Seat};

/// The balance's `HorseOverrideTime`, the trample's seconds before the rider's skill
/// (`0xAD5704`), where the balance lacks it.
pub(crate) const OVERRIDE_TIME: f32 = 10.0;
/// The riding skill.
const RIDING: u8 = 22;
/// The sight of the trample's sparks, the Gladiator's awe's (842's `+0x3C` 1014), which
/// `hit_sparks` bursts at a creature's neck.
pub(crate) const TRAMPLE_SPARKS: u16 = 1014;

/// A horse's trample while it runs (the effect 24 on it, its adder's `+0x2B7` bit `0x10`): what
/// is left of the damage it deals in all (the effect's value, truncated as it is given), the
/// most one strike deals (its factor), the seconds left, the horse's place at its last move, and
/// its strikes sent but not yet resolved, each its target, the most health it may take and the
/// passes since.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Trampling {
    pub value: f32,
    pub factor: f32,
    pub left: f32,
    at: Vec3,
    pending: Vec<(Entity, f32, u8)>,
}

/// How many passes a strike sent waits for its resolution before it no longer holds back the
/// damage in all (a target gone meanwhile resolves none).
const PENDING_PASSES: u8 = 2;

/// The rider's timer of the trample's regeneration (`+0x4E8` and `+0x4EC` of the horse and the
/// rider, which `0x552F60` sets as the trample starts): its seconds left and its whole, by which
/// a new trample waits and the art slots' horse images fill.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct Timer {
    pub left: f32,
    pub whole: f32,
}

impl Timer {
    /// Whether a new trample may start: its regeneration run out (`1 − [+0x4E8]/[+0x4EC]` at
    /// least 1).
    pub fn ready(&self) -> bool {
        self.left <= 0.0
    }

    /// The share of its regeneration run, `1 − left/whole`, 1 where none runs, by which the art
    /// slots' horse images fill (`0x6E359D`).
    pub fn share(&self) -> f32 {
        if self.whole > 0.0 {
            (1.0 - self.left / self.whole).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }
}

/// An art of the hero's that may not be made on horseback, made from the saddle: the event
/// `0x105` that reaches its horse (`0x52FEEB`), whose trample starts where it may.
#[derive(Message, Clone, Copy, Debug, Default)]
pub struct Start;

pub struct TramplePlugin;

impl Plugin for TramplePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Timer>()
            .add_message::<Start>()
            .add_systems(OnEnter(AppState::InGame), |mut commands: Commands| {
                commands.insert_resource(Timer::default());
            })
            .add_systems(
                Update,
                (start, wear, pass, taken, end)
                    .chain()
                    .after(hero::run)
                    .in_set(Step::HeroMove)
                    .in_set(crate::frame::Unpaused),
            );
    }
}

/// The balance's `HorseOverrideTime` (`0xAD5704`), [`OVERRIDE_TIME`] where it lacks it.
pub(crate) fn override_time(data: Option<&crate::game::Data>) -> f32 {
    data.and_then(|data| data.0.tables.balance().ok())
        .and_then(|balance| balance.f32("HorseOverrideTime"))
        .unwrap_or(OVERRIDE_TIME)
}

/// Starts the riding hero's horse's trample at a [`Start`] where its regeneration has run out
/// and it holds none (`0x52FEEB`): the effect 24 of the value, the factor and the seconds of
/// [`horses::trample`] by the horse's row and level and the hero's level and riding skill, and
/// the timer of its regeneration on the rider.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn start(
    mut commands: Commands,
    mut starts: MessageReader<Start>,
    (arts, experience, npcs, data): (
        Option<Res<crate::arts::HeroArts>>,
        Res<fight::Experience>,
        Option<Res<Npcs>>,
        Option<Res<crate::game::Data>>,
    ),
    mut timer: ResMut<Timer>,
    heroes: Query<&Seat, With<Body>>,
    horses: Query<(&Horse, &crate::ai::Mind, &Transform, Has<Trampling>), Without<Dies>>,
) {
    if starts.read().count() == 0 {
        return;
    }
    let Some(horse) = heroes.iter().find_map(Seat::ridden) else {
        return;
    };
    let Ok((kind, mind, at, running)) = horses.get(horse) else {
        return;
    };
    if running || !timer.ready() {
        info!("trample: {horse} may not trample yet");
        return;
    }
    let skill = arts
        .as_deref()
        .and_then(|arts| arts.skills.iter().find(|&&(skill, _)| skill == RIDING))
        .map_or(0, |&(_, level)| level);
    let row = npcs
        .as_deref()
        .map(|npcs| npcs.horse_row(kind.class))
        .unwrap_or_default();
    let trample = horses::trample(
        &row,
        (mind.level, experience.level, skill),
        override_time(data.as_deref()),
    );
    commands.entity(horse).insert(Trampling {
        value: trample.damage.trunc(),
        factor: trample.most,
        left: trample.seconds,
        at: at.translation,
        pending: Vec::new(),
    });
    *timer = Timer {
        left: trample.regeneration,
        whole: trample.regeneration,
    };
    info!(
        "trample: {horse} tramples for {:.1} s, {} in all, at most {:.1} a strike, again in {:.1} s",
        trample.seconds,
        trample.damage.trunc(),
        trample.most,
        trample.regeneration
    );
}

/// A black rider's charge again (`0x4E33D5`–`0x4E343D`, #2288): the effect 24 of the value 0 on
/// its horse for 10 s, its floops showing, its trample striking nothing.
pub(crate) fn charge(world: &mut World, rider: Entity) {
    let Some(horse) = world.get::<Seat>(rider).and_then(Seat::ridden) else {
        return;
    };
    let Some(at) = world.get::<Transform>(horse).map(|at| at.translation) else {
        return;
    };
    world.entity_mut(horse).insert(Trampling {
        value: 0.0,
        factor: 0.0,
        left: opensacred_rules::minds::CHARGE_EFFECT_SECONDS,
        at,
        pending: Vec::new(),
    });
}

/// The trample's seconds and the rider's timer run at each tick of the world.
fn wear(
    mut commands: Commands,
    tick: Res<WorldTick>,
    mut timer: ResMut<Timer>,
    mut horses: Query<(Entity, &mut Trampling)>,
) {
    let Some(seconds) = tick.seconds else {
        return;
    };
    timer.left = (timer.left - seconds).max(0.0);
    for (horse, mut trampling) in &mut horses {
        trampling.left -= seconds;
        if trampling.left <= 0.0 {
            info!("trample: {horse}'s trample ends");
            commands.entity(horse).remove::<Trampling>();
        }
    }
}

/// At each move of the hero's trampling horse, at a tick of the world (`0x5941A0` at
/// `0x596E48`): each of the hero's near hostiles in turn (`crowd::Nearest`, `0xAD3CE0`) while the
/// damage in all lasts, within two thirds of the reach of the horse and it
/// ([`horses::tramples`]), is struck by the horse for the lesser of what is left and the most of
/// a strike, a sure strike of that physical damage, its kill the rider's (**hypothesis**: the
/// horse's owner's share of the kill); a big one against a light horse
/// ([`horses::stops_trample`]) stops it and ends the pass, any other not knocked already is
/// knocked back [`horses::TRAMPLE_KNOCK`] and falls by its slot [`horses::TRAMPLE_FALL`] once.
/// The health a strike takes off the creature is taken off the damage in all as it is resolved
/// ([`taken`]); within a pass the strikes not yet resolved hold back as much as they may take,
/// their amounts at most their creatures' health (**estimate**: the original resolves each strike
/// before the next).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn pass(
    mut commands: Commands,
    tick: Res<WorldTick>,
    nearest: Res<crate::crowd::Nearest>,
    mut heroes: Query<(Entity, &Seat, &mut Body)>,
    mut horses: Query<(&Horse, &crate::ai::Mind, &Transform, &mut Trampling), Without<Dies>>,
    creatures: Query<
        (
            &GlobalTransform,
            &crate::ai::Mind,
            &fight::Health,
            Has<crate::making::Knocked>,
        ),
        (Without<Dies>, Without<Body>, Without<Trampling>),
    >,
    (mut hits, mut sparks): (
        MessageWriter<fight::Hit>,
        MessageWriter<crate::hit_sparks::Spark>,
    ),
) {
    if tick.seconds.is_none() {
        return;
    }
    for (hero, seat, mut body) in &mut heroes {
        let Some(horse) = seat.ridden() else {
            continue;
        };
        let Ok((kind, mind, at, mut trampling)) = horses.get_mut(horse) else {
            continue;
        };
        let here = at.translation;
        if (here - trampling.at).xz().length() <= f32::EPSILON {
            continue;
        }
        trampling.at = here;
        // What is left, less what the strikes not yet resolved may take.
        trampling
            .pending
            .retain(|&(.., passes)| passes < PENDING_PASSES);
        for pending in &mut trampling.pending {
            pending.2 += 1;
        }
        let mut left = trampling.value
            - trampling
                .pending
                .iter()
                .map(|&(_, most, _)| most)
                .sum::<f32>();
        for &creature in &nearest.hostiles {
            if left <= 0.0 {
                break;
            }
            let Ok((place, foe, health, knocked)) = creatures.get(creature) else {
                continue;
            };
            if health.current == 0 {
                continue;
            }
            let reach = minds::reach(horses::SIZE, foe.size());
            let distance = crate::making::whole_distance(here, place.translation());
            if !horses::tramples(distance, reach) {
                continue;
            }
            let amount = horses::trample_strike(left, trampling.factor);
            if amount == 0 {
                break;
            }
            hits.write(fight::Hit {
                attacker: hero,
                target: creature,
                share: 1.0,
                ranged: false,
                art: None,
                fixed: Some([fight_f32(amount), 0.0, 0.0, 0.0]),
                sure: true,
                poison: false,
                ward: false,
            });
            let most = fight_f32(amount.min(health.current));
            left -= most;
            trampling.pending.push((creature, most, 0));
            if horses::stops_trample(foe.big, mind.level, kind.class) {
                info!("trample: {creature} stops {horse}");
                body.stop();
                break;
            }
            // The Gladiator's green sparks (842 of the sight 1014, `0x597600`, `0x5978A2`) at the
            // knocked creature's neck, and at the horse for each it strikes.
            if !knocked {
                sparks.write(crate::hit_sparks::Spark {
                    art: TRAMPLE_SPARKS,
                    by: horse,
                    on: creature,
                });
            }
            sparks.write(crate::hit_sparks::Spark {
                art: TRAMPLE_SPARKS,
                by: horse,
                on: horse,
            });
            if !knocked {
                commands
                    .entity(creature)
                    .remove::<(crate::npcs::Walk, crate::npcs::Strikes)>()
                    .insert((
                        crate::making::Knocked {
                            by: horse,
                            from: here,
                            left: horses::TRAMPLE_KNOCK,
                            held: 0.0,
                            throw: None,
                        },
                        crate::npcs::ScriptClip::fall(horses::TRAMPLE_FALL),
                    ));
            }
            info!("trample: {horse} tramples {creature} for {amount}");
        }
    }
}

/// The health that each of a trample's strikes took off its creature, once resolved
/// (`fight::Struck`), is taken off its damage in all, which spent ends it (`0x5941A0`: the value
/// and the seconds set to 0).
fn taken(
    mut commands: Commands,
    mut struck: MessageReader<fight::Struck>,
    mut horses: Query<(Entity, &mut Trampling)>,
) {
    for struck in struck.read() {
        for (horse, mut trampling) in &mut horses {
            let Some(i) = trampling
                .pending
                .iter()
                .position(|&(target, ..)| target == struck.target)
            else {
                continue;
            };
            trampling.pending.remove(i);
            trampling.value -= fight_f32(struck.damage);
            if trampling.value <= 0.0 {
                info!("trample: {horse}'s trample is spent");
                commands.entity(horse).remove::<Trampling>();
            }
        }
    }
}

/// A trample ends with its horse's rider off it, however it got off, and with the horse's death
/// (every dismount clears `+0x2B7` bit `0x10` on both, `0x53ECA0` at the death).
fn end(
    mut commands: Commands,
    heroes: Query<&Seat, With<Body>>,
    horses: Query<(Entity, Has<Dies>), With<Trampling>>,
) {
    for (horse, dies) in &horses {
        let ridden = heroes.iter().any(|seat| seat.ridden() == Some(horse));
        if dies || !ridden {
            commands.entity(horse).remove::<Trampling>();
        }
    }
}

/// A health as the float a strike carries.
#[expect(
    clippy::cast_precision_loss,
    reason = "a health, far below the floats' whole numbers' range"
)]
fn fight_f32(value: u32) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trample_waits_for_its_regeneration() {
        assert!(Timer::default().ready());
        let timer = Timer {
            left: 15.0,
            whole: 20.0,
        };
        assert!(!timer.ready());
        assert!((timer.share() - 0.25).abs() < 1e-6);
        assert_eq!(Timer::default().share(), 1.0);
    }
}
