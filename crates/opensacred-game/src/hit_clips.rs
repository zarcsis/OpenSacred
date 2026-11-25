//! The struck creature's hit clip over the clip in play (#3637; docs/re/fight.md, "The struck
//! creature's hit clip"; docs/re/motions.md, "A clip over the clip in play"): a blow that leaves
//! a creature alive, a hit or a miss (`fight::Struck`, the event 5's handler `0x534FAB`), and a
//! knock above 10 (`making::hit_clip`, #3141) play its slot 175 once at three times the rate over
//! the clip in play, as `0x593770` adds a Granny control after the creature's own: its weight
//! from 0.25, 0.1 for a creature of the heroes' class struck by a blow, rising toward 1 over 10 s
//! (`opensacred_rules::arts::hit_clip_weight`), the pose moved by it toward the hit clip's, the
//! clip in play going on beneath it with its time and its hits; a clip started after it hides it
//! as its blend rises. A blow plays none while the creature casts (`creature_casts::Casting`), is
//! held by a knock or rides, nor on the hero but standing idle; an NPC walking that a ranged
//! weapon's shot strikes from farther than 300 stops instead (`0x5358E0`–`0x53594E`, **seen**,
//! #3708). Two clips asked of a creature in one frame, the cannon's and the Daemon's blast's
//! (`0x5178C9`, `0x5B1F6C`), are laid one over the other, the pose moved by `1 − (1 − w)²`
//! (#3708).

use bevy::prelude::*;
use opensacred_rules::arts::{HEROES_HIT_CLIP_WEIGHT, HIT_CLIP_RATE, HIT_CLIP_WEIGHT};
use opensacred_rules::minds::HEROES_CLASS;

use crate::fight::Struck;
use crate::frame::{Playing, Unpaused};
use crate::game::AppState;

/// How long a clip started after a hit clip takes to hide it, the creatures' clips' blend in.
const HIDDEN_OVER: f32 = 0.25;

/// A creature's hit clip asked this frame, with its weight at its start
/// ([`opensacred_rules::arts::HIT_CLIP_WEIGHT`]) and how many were asked, each a control laid over
/// the last ([`ask_clip`]).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(crate) struct HitClipAsked {
    pub(crate) weight: f32,
    pub(crate) layers: u8,
}

/// Asks the hit clip of `creature` from the weight `weight`: a second ask in the same frame lays a
/// second over it, as each call of `0x593770` adds a control (the cannon's strike, the Daemon's
/// blast; #3708).
pub(crate) fn ask_clip(commands: &mut Commands, creature: Entity, weight: f32) {
    commands
        .entity(creature)
        .entry::<HitClipAsked>()
        .and_modify(|mut asked| asked.layers = asked.layers.saturating_add(1))
        .or_insert(HitClipAsked { weight, layers: 1 });
}

/// How far, in the world's units, a ranged weapon's shot strikes a walking NPC from for it to stop
/// rather than play its hit clip (`0x5358E0`).
const STOPS_FROM: f32 = 300.0;

/// A hit clip playing over a creature's clip in play: its animation player and node, its weight
/// at its start, the seconds since its start, the clip in play as it started, and the seconds
/// since a clip started after it, which hides it.
#[derive(Component, Debug)]
struct Layered {
    player: Entity,
    node: AnimationNodeIndex,
    start: f32,
    layers: u8,
    seconds: f32,
    over: Option<AnimationNodeIndex>,
    hidden: Option<f32>,
}

/// The weight a clip node takes in an animation graph whose other clips weigh `others` in all, so
/// that the pose is theirs moved by the share `share` toward its own, as Granny lays a control of
/// a weight below 1 over the pose so far (`0x10007420`): the graph normalises its clips' weights,
/// so `share · others / (1 − share)`.
fn layered_weight(share: f32, others: f32) -> f32 {
    if share >= 1.0 {
        f32::MAX
    } else {
        share.max(0.0) * others / (1.0 - share)
    }
}

/// The share of the pose that `layers` controls of the weight `weight` each take, laid one over
/// the other: `1 − (1 − w)^layers`.
fn layered_share(weight: f32, layers: u8) -> f32 {
    1.0 - (1.0 - weight).powi(i32::from(layers.max(1)))
}

/// What a blow's struck creature is and does, by which it plays its hit clip or not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Struckee {
    alive: bool,
    casting: bool,
    held: bool,
    rides: bool,
    /// Whether it is the hero, and whether the hero stands idle (the AI's state 0, its action 0).
    hero: Option<bool>,
    heroes_class: bool,
}

/// The weight at its start of the hit clip that a blow plays on its struck creature, none where
/// it plays none (the event 5's handler, `0x534FAB`–`0x535986`): one that the blow left alive and
/// that neither casts, is held nor rides, the hero only standing idle; 0.1 for the heroes' class.
fn blow_plays(struck: Struckee) -> Option<f32> {
    let plays = struck.alive
        && !struck.casting
        && !struck.held
        && !struck.rides
        && struck.hero.is_none_or(|idle| idle);
    plays.then_some(if struck.heroes_class {
        HEROES_HIT_CLIP_WEIGHT
    } else {
        HIT_CLIP_WEIGHT
    })
}

/// Asks the hit clip of each creature that a blow left alive and that plays one ([`blow_plays`]).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn ask(
    mut commands: Commands,
    mut blows: MessageReader<Struck>,
    creatures: Query<(
        Option<&crate::fight::Health>,
        Option<&crate::ai::Mind>,
        Has<crate::creature_casts::Casting>,
        Has<crate::npcs::Dies>,
        Option<&crate::hero::Body>,
        Has<crate::npcs::Walk>,
    )>,
    places: Query<&GlobalTransform>,
    heroes: Query<
        (
            Has<crate::attack::Attack>,
            Has<crate::attack::Striking>,
            Has<crate::hero_defence::Defending>,
            Has<crate::making::Making>,
            Has<crate::fight::Downed>,
            Option<&crate::riding::Seat>,
        ),
        With<crate::hero::Body>,
    >,
) {
    for blow in blows.read() {
        let Ok((health, mind, casting, dies, body, walks)) = creatures.get(blow.target) else {
            continue;
        };
        // A walking NPC that a ranged weapon's shot strikes from afar stops and stands, and plays
        // no hit clip (`0x5358E0`–`0x53594E`; seen: a goblin shot by the Seraphim from 762 to 966
        // stood two ticks and came on again, #3708).
        let from_afar = blow.shooter.is_some_and(|shooter| {
            places
                .get(shooter)
                .ok()
                .zip(places.get(blow.target).ok())
                .is_some_and(|(shooter, target)| {
                    (target.translation() - shooter.translation()).xz().length() > STOPS_FROM
                })
        });
        if walks && body.is_none() && from_afar {
            debug!("hit clips: {} stops, shot from afar", blow.target);
            commands.entity(blow.target).remove::<crate::npcs::Walk>();
            continue;
        }
        let hero = body.map(|body| {
            body.moving().is_none()
                && heroes.get(blow.target).is_ok_and(
                    |(attack, striking, defending, making, downed, seat)| {
                        !(attack || striking || defending || making || downed)
                            && seat.and_then(crate::riding::Seat::ridden).is_none()
                    },
                )
        });
        let struck = Struckee {
            alive: !dies && health.is_none_or(|health| health.current > 0),
            casting,
            held: mind.is_some_and(|mind| mind.held()),
            rides: mind.is_some_and(|mind| mind.rides.is_some()),
            hero,
            heroes_class: hero.is_some() || mind.is_some_and(|mind| mind.class == HEROES_CLASS),
        };
        if let Some(weight) = blow_plays(struck) {
            ask_clip(&mut commands, blow.target, weight);
        }
    }
}

/// Starts each hit clip asked: its node's clip once from its start at three times the rate by the
/// game's speed and its motion's own factor (`0x593770`), the hero's of its own clips and an NPC's
/// of its graph, none where the model lacks the slot 175.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn start(
    mut commands: Commands,
    mut asked: Query<(Entity, &HitClipAsked, Option<&mut crate::npcs::Npc>)>,
    heroes: Query<(), With<crate::hero::Body>>,
    (hero_clips, data, speed, factors): (
        Option<Res<crate::hero::Clips>>,
        Option<Res<crate::npcs::Npcs>>,
        Res<crate::hero::GameSpeed>,
        Res<crate::motions::Factors>,
    ),
    (gltfs, mut graphs): (
        Res<Assets<bevy::gltf::Gltf>>,
        ResMut<Assets<AnimationGraph>>,
    ),
    mut players: Query<(&mut AnimationPlayer, &AnimationTransitions)>,
) {
    for (entity, asked, npc) in &mut asked {
        commands.entity(entity).remove::<HitClipAsked>();
        let layer = if heroes.contains(entity) {
            hero_clips
                .as_deref()
                .and_then(|clips| Some((clips.player?, clips.hit.clone()?)))
        } else {
            npc.zip(data.as_deref()).and_then(|(mut npc, data)| {
                crate::npcs::hit_layer(&mut npc, data, (&gltfs, &mut graphs))
            })
        };
        let Some((player_entity, (node, clip))) = layer else {
            continue;
        };
        let Ok((mut player, transitions)) = players.get_mut(player_entity) else {
            continue;
        };
        let rate = speed.0 * HIT_CLIP_RATE * factors.of(&clip);
        debug!(
            "hit clips: {entity}'s hit clip over its clip in play from the weight {} at the rate \
             {rate}, {} laid",
            asked.weight, asked.layers
        );
        player
            .start(node)
            .set_repeat(bevy::animation::RepeatAnimation::Never)
            .set_speed(rate)
            .set_weight(0.0)
            .replay();
        commands.entity(entity).insert(Layered {
            player: player_entity,
            node,
            start: asked.weight,
            layers: asked.layers,
            seconds: 0.0,
            over: transitions.get_main_animation(),
            hidden: None,
        });
    }
}

/// Lays each hit clip over its creature's clip in play at each frame, its share of the pose
/// rising from its weight at its start ([`opensacred_rules::arts::hit_clip_weight`]) and hidden
/// as a clip started after it blends in; it ends with its clip, or once hidden.
fn lay(
    mut commands: Commands,
    time: Res<Time>,
    mut layered: Query<(Entity, &mut Layered)>,
    mut players: Query<(&mut AnimationPlayer, &AnimationTransitions)>,
) {
    let dt = time.delta_secs();
    for (entity, mut layer) in &mut layered {
        let Ok((mut player, transitions)) = players.get_mut(layer.player) else {
            commands.entity(entity).remove::<Layered>();
            continue;
        };
        layer.seconds += dt;
        if transitions.get_main_animation() != layer.over {
            layer.hidden = Some(layer.hidden.map_or(0.0, |hidden| hidden + dt));
        }
        let shown = layer
            .hidden
            .map_or(1.0, |hidden| (1.0 - hidden / HIDDEN_OVER).max(0.0));
        let finished = player
            .animation(layer.node)
            .is_none_or(bevy::animation::ActiveAnimation::is_finished);
        if finished || shown <= 0.0 {
            player.stop(layer.node);
            commands.entity(entity).remove::<Layered>();
            continue;
        }
        let others: f32 = player
            .playing_animations()
            .filter(|&(&node, _)| node != layer.node)
            .map(|(_, active)| active.weight())
            .sum();
        let one = opensacred_rules::arts::hit_clip_weight(layer.start, layer.seconds);
        let share = layered_share(one, layer.layers) * shown;
        if let Some(active) = player.animation_mut(layer.node) {
            active.set_weight(layered_weight(share, others));
        }
    }
}

pub struct HitClipsPlugin;

impl Plugin for HitClipsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (ask.after(crate::fight::resolve), start, lay)
                .chain()
                .in_set(Playing)
                .in_set(Unpaused)
                .run_if(in_state(AppState::InGame)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_layered_clip_moves_the_pose_by_its_share() {
        // Against others of 1 in all, a share of 0.25 weighs a third, so that a value of the
        // others' 0 and its 1 blends to 0.25, as the graph's normalised blend gives it.
        let weight = layered_weight(0.25, 1.0);
        assert!((weight - 1.0 / 3.0).abs() < 1e-6);
        assert!((weight / (1.0 + weight) - 0.25).abs() < 1e-6);
        // Two others fading across each other weigh 1.5 in all.
        let weight = layered_weight(0.1, 1.5);
        assert!((weight / (1.5 + weight) - 0.1).abs() < 1e-6);
        assert_eq!(layered_weight(0.0, 1.0), 0.0);
        assert_eq!(layered_weight(1.0, 1.0), f32::MAX);
    }

    #[test]
    fn two_clips_of_one_frame_move_the_pose_as_two_controls_laid_over_each_other() {
        assert!((layered_share(0.25, 1) - 0.25).abs() < 1e-6);
        assert!((layered_share(0.25, 2) - 0.4375).abs() < 1e-6);
        assert!((layered_share(0.1, 0) - 0.1).abs() < 1e-6);
        // Two asks in one frame make one request of two layers.
        let mut world = World::new();
        let creature = world.spawn_empty().id();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, &world);
            ask_clip(&mut commands, creature, 0.25);
            ask_clip(&mut commands, creature, 0.25);
        }
        queue.apply(&mut world);
        assert_eq!(
            world.get::<HitClipAsked>(creature),
            Some(&HitClipAsked {
                weight: 0.25,
                layers: 2
            })
        );
    }

    #[test]
    fn a_blow_plays_the_hit_clip_on_whom_it_leaves_alive_and_free() {
        let npc = Struckee {
            alive: true,
            ..Struckee::default()
        };
        assert_eq!(blow_plays(npc), Some(0.25));
        assert_eq!(
            blow_plays(Struckee {
                heroes_class: true,
                ..npc
            }),
            Some(0.1)
        );
        // Dead, casting, held or riding: none.
        for none in [
            Struckee {
                alive: false,
                ..npc
            },
            Struckee {
                casting: true,
                ..npc
            },
            Struckee { held: true, ..npc },
            Struckee { rides: true, ..npc },
        ] {
            assert_eq!(blow_plays(none), None, "{none:?}");
        }
        // The hero standing idle alone, at 0.1.
        let hero = Struckee {
            hero: Some(true),
            heroes_class: true,
            ..npc
        };
        assert_eq!(blow_plays(hero), Some(0.1));
        assert_eq!(
            blow_plays(Struckee {
                hero: Some(false),
                ..hero
            }),
            None
        );
    }
}
