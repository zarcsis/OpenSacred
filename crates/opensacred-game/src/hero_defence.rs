//! The hero's defence under Shift (#3392; docs/re/fight.md, "The hero's defence"): standing with
//! no order, in its fight stance, Shift held (the walk's key) and no mouse button held, the hero
//! plays its defending clip of what it holds (`motions::Slots::defend_slot`, the slots 53 to 65
//! that its model has), blended in over 0.25 s at its attack speed, at the world's ticks, again
//! and again while that holds, each clip ending 0.2 s before its end (`0x544890` at
//! `0x5460EB`–`0x546467`, `0x544B6A`–`0x5450B4`). While the clip plays past its blend, a strike
//! meets the hero's `D` eight times and its four resistances twice (`fight::resolve`,
//! `opensacred_rules::fight::HERO_DEFENCE`). A class whose model lacks the slot does not defend:
//! the Dwarf and the Daemon, the Wood Elf with her bow.

use std::time::Duration;

use bevy::prelude::*;

use crate::frame::{Step, Unpaused, WorldTick};
use crate::hero::{self, Body, Clips, GameSpeed};

/// The defending clip's blend in (`0x593320`'s 0.25), and how much of it is left as it ends
/// (`0x545204`).
const BLEND: f32 = 0.25;
const ENDS_LEFT: f32 = 0.2;

/// The hero playing its defending clip: whether its blend in is over, from which a strike meets
/// its defence.
#[derive(Component, Debug, Default)]
pub struct Defending {
    pub on: bool,
}

/// Starts, holds and ends the hero's defending clip.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn defend(
    mut commands: Commands,
    tick: Res<WorldTick>,
    keys: crate::controls::Keys,
    buttons: Res<ButtonInput<MouseButton>>,
    nearest: Res<crate::crowd::Nearest>,
    (clips, speed, animations): (
        Option<Res<Clips>>,
        Res<GameSpeed>,
        Res<Assets<AnimationClip>>,
    ),
    mut heroes: Query<
        (
            Entity,
            &Body,
            Option<&mut Defending>,
            (
                Option<&crate::fight::Holding>,
                Option<&crate::fight::Combat>,
            ),
            (
                Has<crate::attack::Attack>,
                Has<crate::attack::Striking>,
                Has<crate::making::Playing>,
                Has<crate::npcs::Dies>,
                Has<crate::fight::Downed>,
            ),
        ),
        With<Body>,
    >,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    let Some(clips) = clips else {
        return;
    };
    let Some((node, clip)) = clips.defend.clone() else {
        return;
    };
    let Some((mut player, mut transitions)) = clips.player.and_then(|e| players.get_mut(e).ok())
    else {
        return;
    };
    let length = animations
        .get(&clip)
        .map_or(0.0, AnimationClip::duration)
        .max(0.01);
    for (hero, body, defending, (holding, combat), (attack, striking, making, dies, downed)) in
        &mut heroes
    {
        let busy = attack || striking || making || dies || downed || body.moving().is_some();
        if let Some(mut defending) = defending {
            // Its clip plays out to 0.2 s before its end, then it stands, and the next tick's test
            // starts it again; an order, a walk or a fall ends it at once.
            let left = player
                .animation(node)
                .filter(|_| transitions.get_main_animation() == Some(node))
                .map(|active| {
                    (
                        (length - active.seek_time()).max(0.0) / active.speed().max(0.001),
                        active,
                    )
                });
            match left {
                Some((left, active)) if !busy && left >= ENDS_LEFT => {
                    defending.on = active.elapsed() >= BLEND;
                }
                _ => {
                    commands.entity(hero).remove::<Defending>();
                }
            }
            continue;
        }
        let ranged = holding.is_some_and(|holding| holding.0.ranged);
        let stance =
            nearest.found && nearest.distance < opensacred_rules::fight::in_fight_within(ranged);
        let button =
            buttons.any_pressed([MouseButton::Left, MouseButton::Right, MouseButton::Middle]);
        if busy
            || tick.seconds.is_none()
            || !stance
            || button
            || !keys.pressed(crate::controls::Action::Walk)
        {
            continue;
        }
        // At its attack speed over 100 (`0x584FE0`, held at 220).
        let rate = combat.map_or(100.0, |combat| combat.speed.min(220.0)) / 100.0;
        transitions
            .play(&mut player, node, Duration::from_secs_f32(BLEND))
            .set_speed(speed.0 * rate);
        commands.entity(hero).insert(Defending::default());
        debug!("hero defence: {hero} defends");
    }
}

/// The hero's defence under Shift.
pub struct HeroDefencePlugin;

impl Plugin for HeroDefencePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            defend
                .after(hero::run)
                .in_set(Unpaused)
                .in_set(Step::HeroMove),
        );
    }
}
