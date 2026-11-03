//! The element stones' and the stargates' loops (#1170; docs/re/sound.md, "The sources"): the
//! scripts' FX objects `FX_ELEMENTSTONE01` to `05` (827 to 830, 841) loop `ELEMENT_LOOP` (213),
//! `06` (939) `HEART_GOLD_LOOP` (214), `07` (768) `HEART_ROTTEN_LOOP` (215), and `FX_STARGATE`
//! (802) and `FX_STARGATE_UW` (1180) `SPELL_TELEPORTER` (2087), each while it stands within 1,150
//! of the listener in the original's screen pixels (`0x69F9EF` to `0x69FAB0`), started only off
//! the distance 0 and stopped beyond; at the sample's own rate, at the volume 1, panned and
//! fading by the object's place (`0x6934E0`). They have no model; `npcs` places them.
//!
//! Not followed: the sound manager's list of the objects near the listener that they are taken
//! from (`+0x1EB8`, its count `+0xAF38`), whose places and how an object comes into it are not
//! read.

use bevy::prelude::*;

use crate::frame::Playing;
use crate::game::AppState;

/// How near the listener such an object sounds, in the original's screen pixels (`0x47E`).
const NEAR: i32 = 1_150;

/// The loop of an FX object of type `kind`, if it has one.
pub fn sound_of(kind: u32) -> Option<u32> {
    match kind {
        827..=830 | 841 => Some(213),
        939 => Some(214),
        768 => Some(215),
        802 | 1180 => Some(2087),
        _ => None,
    }
}

/// An FX object that loops its sound near the listener.
#[derive(Component, Clone, Copy, Debug)]
pub struct FxLoop(pub u32);

/// The loops that play, each at its object.
#[derive(Resource, Default)]
struct Loops(Vec<(Entity, Handle<bevy_kira_audio::AudioInstance>)>);

/// Each such object within [`NEAR`] of the hero and not at its very place loops; a loop stops when
/// its object goes or lies further; each plays at its object's place.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn sing(
    mut loops: ResMut<Loops>,
    objects: Query<(Entity, &FxLoop, &GlobalTransform)>,
    heroes: Query<&GlobalTransform, With<crate::hero::Body>>,
    mut sfx: ResMut<crate::sfx::Sfx>,
    heard: Res<crate::sfx::Listener>,
    assets: Res<AssetServer>,
    audio: Res<bevy_kira_audio::Audio>,
    mut instances: ResMut<Assets<bevy_kira_audio::AudioInstance>>,
) {
    use bevy_kira_audio::prelude::{AudioControl, AudioTween, PlaybackState};
    let Some(hero) = heroes.iter().next().map(GlobalTransform::translation) else {
        return;
    };
    let distance = |at: &GlobalTransform| crate::sfx::screen_distance(at.translation(), hero);
    loops.0.retain(|(object, handle)| {
        // A loop whose instance is not made yet is starting.
        let playing = instances.get(handle).is_none_or(|instance| {
            matches!(
                instance.state(),
                PlaybackState::Playing { .. } | PlaybackState::Queued
            )
        });
        let keep = playing
            && objects
                .get(*object)
                .is_ok_and(|(_, _, at)| distance(at) < NEAR);
        if !keep && let Some(mut instance) = instances.get_mut(handle) {
            instance.stop(AudioTween::default());
        }
        keep
    });
    for (object, &FxLoop(sound), at) in &objects {
        let d = distance(at);
        if d == 0 || d >= NEAR || loops.0.iter().any(|(playing, _)| *playing == object) {
            continue;
        }
        let Some((id, file)) = sfx.with_id(sound) else {
            continue;
        };
        let sample = sfx.sample(id, file, &assets);
        let handle = audio
            .play(sample)
            .looped()
            .with_volume(crate::sfx::decibels(0.0))
            .handle();
        debug!("fx_loops: {object}'s loop {sound} starts");
        loops.0.push((object, handle));
    }
    for (object, handle) in &loops.0 {
        let (Ok((_, _, at)), Some(mut instance)) =
            (objects.get(*object), instances.get_mut(handle))
        else {
            continue;
        };
        let (volume, pan) = crate::sfx::placed_at(at.translation(), Some(hero), *heard);
        instance.set_decibels(crate::sfx::decibels(volume), AudioTween::default());
        instance.set_panning(pan, AudioTween::default());
    }
}

/// Stops the loops as the game ends.
fn end(mut loops: ResMut<Loops>, mut instances: ResMut<Assets<bevy_kira_audio::AudioInstance>>) {
    for (_, handle) in loops.0.drain(..) {
        if let Some(mut instance) = instances.get_mut(&handle) {
            instance.stop(bevy_kira_audio::prelude::AudioTween::default());
        }
    }
}

/// The element stones' and the stargates' loops.
pub struct FxLoopsPlugin;

impl Plugin for FxLoopsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Loops>()
            .add_systems(OnExit(AppState::InGame), end)
            .add_systems(
                Update,
                sing.in_set(Playing).run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stones_and_the_gates_loop_their_sounds() {
        assert_eq!(
            [827, 830, 841, 939, 768, 802, 1180, 878].map(sound_of),
            [
                Some(213),
                Some(213),
                Some(213),
                Some(214),
                Some(215),
                Some(2087),
                Some(2087),
                None
            ]
        );
    }
}
