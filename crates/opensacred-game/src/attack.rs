//! The hero's attack (#283; docs/re/fight.md, "The hero's attack"): a left click on a hostile
//! creature sends the hero at it (`0x6172C0`, the AI's states 1 and 4): it comes into the band of
//! its reach and strikes once, again while the button stays held or for each click during a
//! strike (#705), until the creature dies or the player sends it elsewhere; with Ctrl held it
//! strikes once, at once, where it stands, and a click on the ground strikes the air there. Its hits resolve as the creatures' (`fight`). Where it stands, when it strikes and when
//! its hits land are the rules' (`opensacred_rules::minds`). With a bow it shoots arrows
//! (`arrows`), and with the Seraphim's effects of 1018 and 1019 bolts (`bolts`).

use std::time::Duration;

use bevy::prelude::*;
use opensacred_rules::arts::{BFG, Bolt, RAILGUN};
use opensacred_rules::minds;

use crate::ai::Mind;
use crate::fight::{self, Health};
use crate::frame::{Control, Step, Unpaused, WorldTick};
use crate::hero::{self, Body, Clips, GameSpeed};
use crate::npcs::Dies;
use crate::spawning::Rng;

/// How near the cursor's line of sight a creature is picked, and the height of the part of it
/// that picks, in world units (**hypothesis**: the original picks by the model's shape).
pub(crate) const PICKED_WITHIN: f32 = 45.0;
const PICKED_HEIGHT: f32 = 110.0;
/// The blend into a strike's clip.
const BLEND: Duration = Duration::from_millis(100);

/// Whether the left button's press went to an attack: then it does not steer the hero while it
/// is held (`hero::steer`).
#[derive(Resource, Default)]
pub struct Aim(pub bool);

/// The hero's attack: its target, or none for a strike in the air at `point`; whether it strikes
/// at once, where it stands, once; whether it ends with its strike, the player having sent the
/// hero elsewhere; whether the button that sent it is still held; whether a click still asks
/// for a strike that has not begun; whether its first strike spoke its `ATTACK` line; where its
/// AI stands; and the combat art it is a shot of (a special arrow), if any.
#[derive(Component)]
pub struct Attack {
    target: Option<Entity>,
    point: Vec3,
    at_once: bool,
    ends: bool,
    held: bool,
    asked: bool,
    spoke: bool,
    phase: Phase,
    special: Option<crate::arrows::Special>,
}

/// Where the hero's attack stands in the AI's state 4 (`0x4E0300`), which steps at the world's
/// ticks alone (#617): the original's hero, sent by a click at a target in its band, took the
/// state 4 at the click's tick, its sub-state 2 at the next and struck at the one after.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Sent by a click, an order that the next tick takes (`0x60F130`, after the hero's update).
    Ordered,
    /// Deciding (the sub-state 0, or 6 after a strike that did not chain).
    Deciding,
    /// Coming on to the band (the sub-state 1), its move ending in it.
    ComesOn,
    /// In the band, turned to its target (the sub-state 2): it strikes at the next tick.
    InBand,
}

impl Attack {
    /// The creature it attacks, if any.
    pub fn target(&self) -> Option<Entity> {
        self.target
    }

    /// The hero's attack on `target`, as a click on it orders it, the button not held (the
    /// console's `attack`).
    pub fn ordered(target: Entity) -> Self {
        Self {
            target: Some(target),
            point: Vec3::ZERO,
            at_once: false,
            ends: false,
            held: false,
            asked: true,
            spoke: false,
            phase: Phase::Ordered,
            special: None,
        }
    }

    /// The hero's shot of the combat art `special` at `target`, at once, where it stands, once,
    /// at the next tick (the action 5 that the Wood Elf's executor sets for her special arrows,
    /// `0x50AC00`).
    pub fn shot(target: Entity, special: crate::arrows::Special) -> Self {
        Self {
            at_once: true,
            phase: Phase::InBand,
            special: Some(special),
            ..Self::ordered(target)
        }
    }
}

/// The strike the hero plays (the attack action 3, `0x4E4CF0`): its clip's node, its length,
/// the fractions at which it hits, how many hits it has sent, and the bolt it shoots, if it is a
/// shot of bolts (the action 8, `0x4DCB80`).
#[derive(Component)]
pub struct Striking {
    node: AnimationNodeIndex,
    length: f32,
    fractions: [f32; 2],
    struck: u8,
    bolt: Option<Bolt>,
}

/// The dice of the hero's strikes.
#[derive(Resource)]
struct Dice(Rng);

/// The distance of `point`, or of the nearest of the points up to [`PICKED_HEIGHT`] over it, from
/// the line of `ray`.
pub(crate) fn from_line(ray: Ray3d, point: Vec3) -> f32 {
    let direction = ray.direction.as_vec3();
    (0..=4_u8)
        .map(|step| {
            let up = point + Vec3::Y * (PICKED_HEIGHT * f32::from(step) / 4.0);
            (up - ray.origin).cross(direction).length()
        })
        .fold(f32::INFINITY, f32::min)
}

/// On a press of the left button, the hero's attack: a hostile creature alive under the cursor
/// becomes its target, unless Alt is held over an item or its label, which comes first then
/// (the pick's classes in the order 2, 1, 0, 3, `0x95F934`; `labels`); with Ctrl held it strikes
/// at once, at the creature or in the air at the cursor; a press elsewhere ends the attack once
/// its strike ends, and steers the hero. On the button's release the attack ends with its strike,
/// unless a click still asks for one.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
pub(crate) fn aim(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    pointer: Res<crate::hud::Pointer>,
    camera: Single<(&Camera, &GlobalTransform), crate::iso::Eye<With<Camera>>>,
    mut aim: ResMut<Aim>,
    // A creature of a dialogue is spoken to, hostile or not (`0x53A170`, `crate::talk`).
    (creatures, items, labels): (
        Query<
            (Entity, &GlobalTransform, &Mind, &Health),
            (Without<Dies>, Without<crate::npcs::Talk>),
        >,
        Query<(&GlobalTransform, &crate::drops::Lying)>,
        Res<crate::labels::Shown>,
    ),
    mut heroes: Query<
        (
            Entity,
            &Transform,
            Option<&mut Attack>,
            Has<crate::making::Playing>,
        ),
        (With<Body>, Without<Dies>),
    >,
) {
    if !buttons.pressed(MouseButton::Left) {
        aim.0 = false;
        for (_, _, attack, _) in &mut heroes {
            if let Some(mut attack) = attack.filter(|attack| attack.held) {
                attack.held = false;
            }
        }
    }
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(cursor) = pointer.0 else {
        return;
    };
    let (camera, view) = *camera;
    let Ok(ray) = camera.viewport_to_world(view, cursor) else {
        return;
    };
    let items_first = crate::labels::held(&keys)
        && (labels.under(cursor).is_some()
            || items.iter().any(|(at, lying)| {
                lying.resting && from_line(ray, at.translation()) <= PICKED_WITHIN
            }));
    let picked = creatures
        .iter()
        .filter(|(_, _, mind, health)| !items_first && mind.hostile() && health.current > 0)
        .map(|(entity, at, ..)| (entity, from_line(ray, at.translation())))
        .filter(|&(_, distance)| distance <= PICKED_WITHIN)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(entity, _)| entity);
    let at_once = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    for (hero, transform, attack, playing) in &mut heroes {
        // Where the cursor's line of sight meets the hero's height: the air it strikes.
        let along = (transform.translation.y - ray.origin.y) / ray.direction.y;
        let point = ray.origin + ray.direction.as_vec3() * along;
        aim.0 = picked.is_some() || at_once;
        let next = Attack {
            target: picked,
            point,
            at_once,
            ends: false,
            held: true,
            asked: true,
            spoke: false,
            phase: Phase::Ordered,
            special: None,
        };
        match (aim.0, attack) {
            // A click on the same target goes on with its attack, which spoke at its first
            // strike (the original's sub-state 1 anew, `0x4E4CF0`).
            (true, Some(mut attack)) => {
                let spoke = attack.spoke && attack.target == next.target;
                *attack = Attack { spoke, ..next };
            }
            (true, None) => {
                let mut hero = commands.entity(hero);
                // An art under way keeps its order, as the button leaves it (`making::aim`):
                // `making::make` alone ends its clip, and the attack waits for that (`engage`,
                // #1215).
                if !playing {
                    hero.remove::<crate::making::Making>();
                }
                hero.insert(next);
            }
            (false, Some(mut attack)) => attack.ends = true,
            (false, None) => {}
        }
        if let Some(target) = picked {
            debug!("attack: the hero attacks {target}");
        }
    }
}

/// The hero's attack (the AI's state 4 for a hero, docs/re/fight.md), deciding at the world's
/// ticks ([`Phase`], #617): the click's order taken at a tick, it decides at the next; farther
/// than its band from its target it runs to it, after it as it moves, its move ending in the band
/// at any frame; in the band, or at once, it stops and turns to the target, and at the next tick
/// strikes: one of its attack clips at random, once, at the game's speed, which hits at the first
/// tick past the clip's fractions (0.5 for a first of 0; a second where there is one) when the
/// target is then within 1.5 times the reach, its strikes at an even pace. It strikes again while
/// the button that sent it is held or a click asks for another (the original's attack action keeps
/// its first half while the button is held, #705), until the target dies; a strike at once or in
/// the air is one. With the button held, the next strike starts as soon as this one's hits are
/// struck and its clip has the turn to the next left (`minds::strike_gives_way`, #707), at any
/// frame as the clip queued at the hit starts; a click waits for the clip's end, which a tick
/// sees, and strikes then; a strike that does not chain ends there and its AI decides again at
/// the next tick.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn engage(
    mut commands: Commands,
    tick: Res<WorldTick>,
    mut dice: ResMut<Dice>,
    clips: Option<Res<Clips>>,
    speed: Res<GameSpeed>,
    animations: Res<Assets<AnimationClip>>,
    mut heroes: Query<
        (
            Entity,
            &mut Transform,
            &mut Body,
            &mut Attack,
            Option<&mut Striking>,
            Option<&fight::Combat>,
            Option<&crate::hero::Size>,
            Option<&fight::Holding>,
            Option<&fight::Frenzy>,
            Option<&crate::effects::Effects>,
            Has<crate::making::Playing>,
        ),
        Without<Dies>,
    >,
    targets: Query<(&GlobalTransform, &Health, Option<&crate::ai::Mind>), Without<Dies>>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    (mut hits, mut shots, mut bolts): (
        MessageWriter<fight::Hit>,
        MessageWriter<crate::arrows::Loose>,
        MessageWriter<crate::bolts::Fire>,
    ),
    mut lines: MessageWriter<crate::voices::Speak>,
    mut blows: MessageWriter<crate::sfx::Blow>,
) {
    let Some(clips) = clips else {
        return;
    };
    for (
        hero,
        mut transform,
        mut body,
        mut attack,
        striking,
        combat,
        size,
        holding,
        frenzy,
        effects,
        playing,
    ) in &mut heroes
    {
        // Ordered while an art plays, it waits for the art's clip to end (`aim`, #1215).
        if playing {
            continue;
        }
        let target = attack
            .target
            .and_then(|target| targets.get(target).ok())
            .filter(|(_, health, _)| health.current > 0);
        // With the Seraphim's effect 17, or 16 beyond 200, it shoots bolts (`0x53A170` at
        // `0x53AACE`).
        let bolt = effects.and_then(|effects| {
            let distance = target.map(|(at, ..)| {
                (at.translation() - transform.translation)
                    .with_y(0.0)
                    .length()
            });
            Bolt::of(effects.holds(BFG), effects.holds(RAILGUN), distance)
        });
        // With a bow or a crossbow in hand it shoots (the AI's state 8 for a hero, #784).
        let ranged = holding.is_some_and(|holding| holding.0.ranged) || bolt.is_some();
        // The reach of the hero and its target, by their sizes (`0x4DBC60`, #642).
        let reach = minds::reach(
            size.map_or(minds::SIZE, |size| size.0),
            target
                .and_then(|(_, _, mind)| mind)
                .map_or(minds::SIZE, |mind| mind.size()),
        );
        let target = target.map(|(at, ..)| at.translation());
        let here = transform.translation;
        let Some((mut player, mut transitions)) =
            clips.player.and_then(|player| players.get_mut(player).ok())
        else {
            continue;
        };
        // At its attack speed (`0x4E4CF0`): the hero's `+0x210` stays 1, held or clicked (#704).
        // Quickened by the Dwarf's frenzy (`0x584FE0`).
        let attack_speed = combat.map_or(1.0, |combat| {
            opensacred_rules::arts::quickened(combat.speed, frenzy.map_or(0.0, |frenzy| frenzy.0))
                / 100.0
        });
        // A shot of bolts at its own rate (`0x4DCB80`).
        let rate = |bolt: Option<Bolt>| bolt.map_or(attack_speed, Bolt::rate);
        let ticked = tick.seconds.is_some();
        let mut blend = BLEND;
        // Whether a strike starts at this frame whatever the AI's step: the next of a held button
        // or of a click.
        let mut now = false;
        if let Some(mut striking) = striking {
            let Some(active) = player.animation(striking.node) else {
                commands.entity(hero).remove::<Striking>();
                continue;
            };
            let done = active.seek_time() / striking.length;
            // The action tests its hits at its steps, the world's ticks, alone (`0x4EA1F0`).
            if ticked && minds::hit_due(striking.fractions, striking.struck, done) {
                striking.struck += 1;
                // The swing within reach or in the air (`0x4EA6FD`, #712).
                if attack.target.is_none() {
                    blows.write(crate::sfx::Blow::Swing { striker: hero });
                }
                if let Some(bolt) = striking.bolt {
                    // Its bolts leave at the clip's first hit toward its target, or the point it
                    // strikes (`0x4DDD2A`, `0x4DE162`, `0x4DE5EC`), which the bolts fly and hit.
                    if striking.struck == 1 {
                        bolts.write(crate::bolts::Fire {
                            shooter: hero,
                            toward: target.unwrap_or(attack.point),
                            bolt,
                        });
                    }
                } else if ranged {
                    // Its shot leaves at the clip's first hit, an arrow from its left hand toward
                    // its target (`0x4DD90E`, `0x4EB700`), which the arrows fly and hit.
                    if let (Some(entity), 1) = (attack.target, striking.struck) {
                        shots.write(crate::arrows::Loose {
                            shooter: hero,
                            target: entity,
                            special: attack.special,
                        });
                    }
                } else if let (Some(entity), Some(at)) = (attack.target, target)
                    && minds::hero_hits((at - here).with_y(0.0).length(), reach)
                {
                    blows.write(crate::sfx::Blow::Swing { striker: hero });
                    hits.write(fight::Hit {
                        fixed: None,
                        sure: false,
                        poison: false,
                        attacker: hero,
                        target: entity,
                        share: 1.0,
                        ranged: false,
                        art: None,
                    });
                }
            }
            // With the button held, the next strike starts before this one's clip ends, the one
            // fading into the other over what is left (`0x5491F0`, #707).
            let rate = rate(striking.bolt);
            let left = (striking.length - active.seek_time()).max(0.0) / (speed.0 * rate);
            if attack.held
                && !attack.at_once
                && !attack.ends
                && target.is_some()
                && minds::strike_gives_way(striking.fractions, striking.struck, left, rate)
            {
                commands.entity(hero).remove::<Striking>();
                blend = Duration::from_secs_f32(left.max(0.001));
                now = true;
            } else if !(ticked && active.is_finished()) {
                continue;
            } else {
                commands.entity(hero).remove::<Striking>();
                if minds::attack_ends(
                    attack.at_once,
                    attack.ends,
                    target.is_some(),
                    attack.held || attack.asked,
                ) {
                    commands.entity(hero).remove::<Attack>();
                    continue;
                }
                if !attack.asked {
                    // Not chained, the action ends and the AI decides again (the sub-state 6).
                    attack.phase = Phase::Deciding;
                    continue;
                }
                now = true;
            }
        }
        if !now && !ticked {
            // Between the ticks only its move to the band ends there (`0x4F4770`).
            if attack.phase == Phase::ComesOn
                && !attack.ends
                && let Some(at) = target
            {
                let distance = (at - here).with_y(0.0).length();
                let comes_on = if ranged {
                    minds::hero_comes_on_to_shoot(false, distance, true)
                } else {
                    minds::hero_comes_on(false, distance, reach)
                };
                if !comes_on {
                    body.stop();
                }
            }
            continue;
        }
        let aimed = match (attack.target, target) {
            (Some(_), Some(at)) => at,
            (None, _) if attack.at_once => attack.point,
            _ => {
                commands.entity(hero).remove::<Attack>();
                continue;
            }
        };
        if attack.ends {
            commands.entity(hero).remove::<Attack>();
            continue;
        }
        if !now && attack.phase == Phase::Ordered {
            // The click's order taken at this tick, it decides at the next.
            attack.phase = Phase::Deciding;
            continue;
        }
        let distance = (aimed - here).with_y(0.0).length();
        let comes_on = if ranged {
            minds::hero_comes_on_to_shoot(attack.at_once, distance, attack.phase == Phase::ComesOn)
        } else {
            minds::hero_comes_on(attack.at_once, distance, reach)
        };
        if comes_on {
            body.send(aimed);
            attack.phase = Phase::ComesOn;
            continue;
        }
        body.stop();
        transform.rotation = hero::facing((aimed - here).with_y(0.0));
        if !now && attack.phase != Phase::InBand {
            // In the band, turned to its target, it strikes at the next tick.
            attack.phase = Phase::InBand;
            continue;
        }
        let count = u32::try_from(clips.attacks.len()).unwrap_or(0);
        let Some((node, clip, fractions)) = usize::try_from(dice.0.below(count))
            .ok()
            .and_then(|index| clips.attacks.get(index))
        else {
            commands.entity(hero).remove::<Attack>();
            continue;
        };
        let length = animations
            .get(clip)
            .map_or(1.0, AnimationClip::duration)
            .max(0.01);
        transitions
            .play(&mut player, *node, blend)
            .set_speed(speed.0 * rate(bolt));
        attack.asked = false;
        attack.phase = Phase::Deciding;
        // The attack's first strike speaks (`0x4E52D1`, the action's sub-state 0); the strikes it
        // chains do not (#711).
        if !attack.spoke {
            attack.spoke = true;
            lines.write(crate::voices::Speak {
                speaker: hero,
                group: crate::voices::ATTACK,
            });
        }
        commands.entity(hero).insert(Striking {
            node: *node,
            length,
            fractions: *fractions,
            struck: 0,
            bolt,
        });
    }
}

/// The hero's attack.
pub struct AttackPlugin;

impl Plugin for AttackPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Aim>()
            .insert_resource(Dice(Rng::of(0, 0, 1)))
            .add_systems(
                Update,
                (
                    aim.in_set(Control).before(hero::steer),
                    engage.after(hero::steer).before(hero::take_orders),
                )
                    .in_set(Unpaused)
                    .in_set(Step::HeroMove),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_creature_is_picked_near_the_line_of_sight_up_to_its_height() {
        let ray = Ray3d::new(Vec3::new(0.0, 1000.0, 0.0), Dir3::NEG_Y);
        // Straight under the line, or under it by its height, it is on the line.
        assert!(from_line(ray, Vec3::new(0.0, 0.0, 0.0)) < 1e-3);
        // Beside it, by the distance.
        assert!((from_line(ray, Vec3::new(30.0, 0.0, 0.0)) - 30.0).abs() < 1e-3);
        // A slanted line meets the creature's head before its feet.
        let slanted = Ray3d::new(
            Vec3::new(0.0, 1000.0, 0.0),
            Dir3::new(Vec3::new(1.0, -10.0, 0.0)).unwrap(),
        );
        let feet = Vec3::new(100.0 - PICKED_HEIGHT / 10.0, 0.0, 0.0);
        assert!(from_line(slanted, feet) < 1.0);
    }
}
