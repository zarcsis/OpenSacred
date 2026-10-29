//! The pause (docs/10 §4): Esc in the game stops it under the original's Esc menu (`esc_menu`), and again goes
//! on. Bevy's virtual time stops, so that everything on the game's time stands still: the hero,
//! the NPCs and their clips, the clock, the liquids. The world takes no keys or clicks
//! (`frame::Unpaused`), and the sound pauses, fading out, to go on from where it stopped: paused
//! rather than muted, so that no track runs on unheard.

use std::time::Duration;

use bevy::prelude::*;
use bevy_kira_audio::prelude::{Audio, AudioChannel, AudioControl, AudioTween};

use crate::frame::Unpaused;
use crate::game::AppState;
use crate::{atmosphere, music};

/// How a game goes: on, or paused.
#[derive(SubStates, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[source(AppState = AppState::InGame)]
pub enum Play {
    #[default]
    Running,
    Paused,
}

/// A pause without the pause's menu, for a still frame (the command `pause on`).
#[derive(Resource, Default)]
pub struct Still(pub bool);

/// How long the sound takes to fade out and in.
const FADE: Duration = Duration::from_millis(200);

/// The channels of the sound: the music's, the ambient loops' and the effects', if the game has
/// its sound.
type Channels<'w> = (
    Option<Res<'w, AudioChannel<music::Channel>>>,
    Option<Res<'w, AudioChannel<atmosphere::Channel>>>,
    Option<Res<'w, Audio>>,
);

/// Esc pauses the game, and again goes on.
pub(crate) fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    play: Res<State<Play>>,
    mut next: ResMut<NextState<Play>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(match play.get() {
            Play::Running => Play::Paused,
            Play::Paused => Play::Running,
        });
    }
}

fn pause(mut time: ResMut<Time<Virtual>>, (music, atmosphere, effects): Channels) {
    time.pause();
    let tween = AudioTween::linear(FADE);
    if let Some(channel) = music {
        channel.pause().fade_out(tween);
    }
    if let Some(channel) = atmosphere {
        channel.pause().fade_out(tween);
    }
    if let Some(channel) = effects {
        channel.pause().fade_out(tween);
    }
}

fn resume(mut time: ResMut<Time<Virtual>>, (music, atmosphere, effects): Channels) {
    time.unpause();
    let tween = AudioTween::linear(FADE);
    if let Some(channel) = music {
        channel.resume().fade_in(tween);
    }
    if let Some(channel) = atmosphere {
        channel.resume().fade_in(tween);
    }
    if let Some(channel) = effects {
        channel.resume().fade_in(tween);
    }
}

/// The pause: its state, Esc, the time and the sound.
pub struct PausePlugin;

impl Plugin for PausePlugin {
    fn build(&self, app: &mut App) {
        app.add_sub_state::<Play>()
            .init_resource::<Still>()
            .configure_sets(Update, Unpaused.run_if(in_state(Play::Running)))
            .add_systems(
                Update,
                toggle
                    .run_if(in_state(AppState::InGame))
                    .in_set(crate::frame::Control),
            )
            .add_systems(OnEnter(Play::Paused), pause)
            .add_systems(
                OnExit(Play::Paused),
                (resume, |mut still: ResMut<Still>| still.0 = false),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esc_stops_the_games_time_and_its_controls_and_again_goes_on() {
        #[derive(Resource, Default)]
        struct Steered(u32);
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .insert_state(AppState::InGame)
            .add_plugins(PausePlugin)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Steered>()
            .add_systems(
                Update,
                (|mut steered: ResMut<Steered>| steered.0 += 1).in_set(Unpaused),
            );
        let press = |app: &mut App| {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::Escape);
            app.update();
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.release(KeyCode::Escape);
            keys.clear();
            app.update();
        };
        app.update();
        let before = app.world().resource::<Steered>().0;
        assert!(before > 0);
        press(&mut app);
        assert_eq!(*app.world().resource::<State<Play>>().get(), Play::Paused);
        assert!(app.world().resource::<Time<Virtual>>().is_paused());
        let paused = app.world().resource::<Steered>().0;
        app.update();
        assert_eq!(app.world().resource::<Steered>().0, paused);
        press(&mut app);
        assert_eq!(*app.world().resource::<State<Play>>().get(), Play::Running);
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
        assert!(app.world().resource::<Steered>().0 > paused);
    }
}
