//! The hero's attack (#283; docs/re/fight.md, "The hero's attack"): a left click on a hostile
//! creature sends the hero at it (`0x6172C0`, the AI's states 1 and 4): it comes into the band of
//! its reach and strikes once, again while the button stays held or for each click during a
//! strike (#705), until the creature dies or the player sends it elsewhere; with Ctrl held it
//! strikes once, at once, where it stands, and a click on the ground strikes the air there. Its hits resolve as the creatures' (`fight`). Where it stands, when it strikes and when
//! its hits land are the rules' (`opensacred_rules::minds`).

use std::time::Duration;

use bevy::prelude::*;
use opensacred_rules::minds;

use crate::ai::Mind;
use crate::fight::{self, Health};
use crate::frame::{Control, Step, Unpaused};
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
/// hero elsewhere; whether the button that sent it is still held; and whether a click still asks
/// for a strike that has not begun.
#[derive(Component)]
pub struct Attack {
    target: Option<Entity>,
    point: Vec3,
    at_once: bool,
    ends: bool,
    held: bool,
    asked: bool,
}

impl Attack {
    /// The creature it attacks, if any.
    pub fn target(&self) -> Option<Entity> {
        self.target
    }
}

/// The strike the hero plays (the attack action 3, `0x4E4CF0`): its clip's node, its length,
/// the fractions at which it hits, and how many hits it has sent.
#[derive(Component)]
pub struct Striking {
    node: AnimationNodeIndex,
    length: f32,
    fractions: [f32; 2],
    struck: u8,
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
/// becomes its target; with Ctrl held it strikes at once, at the creature or in the air at the
/// cursor; a press elsewhere ends the attack once its strike ends, and steers the hero. On the
/// button's release the attack ends with its strike, unless a click still asks for one.
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
    camera: Single<(&Camera, &GlobalTransform)>,
    mut aim: ResMut<Aim>,
    creatures: Query<(Entity, &GlobalTransform, &Mind, &Health), Without<Dies>>,
    mut heroes: Query<(Entity, &Transform, Option<&mut Attack>), (With<Body>, Without<Dies>)>,
) {
    if !buttons.pressed(MouseButton::Left) {
        aim.0 = false;
        for (.., attack) in &mut heroes {
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
    let picked = creatures
        .iter()
        .filter(|(_, _, mind, health)| mind.hostile() && health.current > 0)
        .map(|(entity, at, ..)| (entity, from_line(ray, at.translation())))
        .filter(|&(_, distance)| distance <= PICKED_WITHIN)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(entity, _)| entity);
    let at_once = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    for (hero, transform, attack) in &mut heroes {
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
        };
        match (aim.0, attack) {
            (true, Some(mut attack)) => *attack = next,
            (true, None) => {
                commands
                    .entity(hero)
                    .remove::<crate::making::Making>()
                    .insert(next);
            }
            (false, Some(mut attack)) => attack.ends = true,
            (false, None) => {}
        }
        if let Some(target) = picked {
            debug!("attack: the hero attacks {target}");
        }
    }
}

/// The hero's attack (the AI's state 4 for a hero, docs/re/fight.md): farther than its band from
/// its target it runs to it, after it as it moves; in the band, or at once, it stops, turns to
/// the target and strikes: one of its attack clips at random, once, at the game's speed, which
/// hits at the clip's fractions (0.5 for a first of 0; a second where there is one) when the
/// target is then within 1.5 times the reach, its strikes at an even pace. It strikes again while
/// the button that sent it is held or a click asks for another (the original's attack action keeps
/// its first half while the button is held, #705), until the target dies; a strike at once or in
/// the air is one.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn engage(
    mut commands: Commands,
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
        ),
        Without<Dies>,
    >,
    targets: Query<(&GlobalTransform, &Health, Option<&crate::ai::Mind>), Without<Dies>>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    mut hits: MessageWriter<fight::Hit>,
) {
    let Some(clips) = clips else {
        return;
    };
    for (hero, mut transform, mut body, mut attack, striking, combat, size) in &mut heroes {
        let target = attack
            .target
            .and_then(|target| targets.get(target).ok())
            .filter(|(_, health, _)| health.current > 0);
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
        if let Some(mut striking) = striking {
            let Some(active) = player.animation(striking.node) else {
                commands.entity(hero).remove::<Striking>();
                continue;
            };
            let done = active.seek_time() / striking.length;
            if minds::hit_due(striking.fractions, striking.struck, done) {
                striking.struck += 1;
                if let (Some(entity), Some(at)) = (attack.target, target)
                    && minds::hero_hits((at - here).with_y(0.0).length(), reach)
                {
                    hits.write(fight::Hit {
                        attacker: hero,
                        target: entity,
                        share: 1.0,
                        ranged: false,
                        art: None,
                    });
                }
            }
            if active.is_finished() {
                commands.entity(hero).remove::<Striking>();
                if minds::attack_ends(
                    attack.at_once,
                    attack.ends,
                    target.is_some(),
                    attack.held || attack.asked,
                ) {
                    commands.entity(hero).remove::<Attack>();
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
        if minds::hero_comes_on(attack.at_once, (aimed - here).with_y(0.0).length(), reach) {
            body.send(aimed);
            continue;
        }
        body.stop();
        transform.rotation = hero::facing((aimed - here).with_y(0.0));
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
        // At its attack speed (`0x4E4CF0`): the hero's `+0x210` stays 1, held or clicked (#704).
        let attack_speed = combat.map_or(1.0, |combat| combat.clip_speed());
        transitions
            .play(&mut player, *node, BLEND)
            .set_speed(speed.0 * attack_speed);
        attack.asked = false;
        commands.entity(hero).insert(Striking {
            node: *node,
            length,
            fractions: *fractions,
            struck: 0,
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
