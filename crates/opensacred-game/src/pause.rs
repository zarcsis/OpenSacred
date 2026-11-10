//! The pause (docs/10 §4): Esc in the game stops it under the original's Esc menu (`esc_menu`), and again goes
//! on; P under the busy dialog's pause window (`busy_dialog`, #1687). Bevy's virtual time stops, so that everything on the game's time stands still: the hero,
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

/// A pause under the busy dialog's pause window instead of the menu, the key P's (`busy_dialog`,
/// #1687), which Esc leaves alone.
#[derive(Resource, Default)]
pub struct Busy(pub bool);

/// The scripts' `GameActive` (131, the record walker's `0x475D50`; #2425): `asked` what the last
/// of them asked and not yet done, `false` to stand the game's time as the options and the busy
/// dialog do (`0x6413E0(1)`, `0x641720`), `true` to let it run; and whether the pause standing
/// is theirs, which alone they end. Their pause shows no menu, as [`Still`].
#[derive(Resource, Default)]
pub struct Halt {
    pub asked: Option<bool>,
    held: bool,
}

/// How long the sound takes to fade out and in.
const FADE: Duration = Duration::from_millis(200);

/// The channels of the sound: the music's, the ambient loops' and the effects', if the game has
/// its sound.
type Channels<'w> = (
    Option<Res<'w, AudioChannel<music::Channel>>>,
    Option<Res<'w, AudioChannel<atmosphere::Channel>>>,
    Option<Res<'w, Audio>>,
);

/// Esc pauses the game, and again goes on, but from the key P's pause.
pub(crate) fn toggle(
    keys: crate::controls::Keys,
    (play, busy): (Res<State<Play>>, Res<Busy>),
    mut next: ResMut<NextState<Play>>,
) {
    if keys.just_pressed(crate::controls::Action::Back) && !busy.0 {
        next.set(match play.get() {
            Play::Running => Play::Paused,
            Play::Paused => Play::Running,
        });
    }
}

/// Carries out what the scripts' `GameActive` asked: a running game paused without the menu, or
/// the scripts' own pause ended; a pause of the player's or a movie's they leave alone.
fn halt(
    mut halt: ResMut<Halt>,
    play: Res<State<Play>>,
    mut next: ResMut<NextState<Play>>,
    mut still: ResMut<Still>,
) {
    let Some(active) = halt.asked.take() else {
        return;
    };
    match (active, play.get()) {
        (false, Play::Running) => {
            still.0 = true;
            halt.held = true;
            next.set(Play::Paused);
        }
        (true, Play::Paused) if halt.held => next.set(Play::Running),
        _ => {}
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
            .init_resource::<Busy>()
            .init_resource::<Halt>()
            .configure_sets(Update, Unpaused.run_if(in_state(Play::Running)))
            .add_systems(
                Update,
                (
                    toggle.in_set(crate::frame::Control),
                    halt.after(crate::frame::Control),
                )
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(OnEnter(Play::Paused), pause)
            .add_systems(
                OnExit(Play::Paused),
                (
                    resume,
                    |(mut still, mut busy, mut halt): (
                        ResMut<Still>,
                        ResMut<Busy>,
                        ResMut<Halt>,
                    )| {
                        still.0 = false;
                        busy.0 = false;
                        halt.held = false;
                    },
                ),
            )
            .add_systems(OnExit(AppState::InGame), |mut halt: ResMut<Halt>| {
                *halt = Halt::default();
            });
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

    #[test]
    fn the_scripts_game_active_stands_the_game_and_ends_only_its_own_pause() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .insert_state(AppState::InGame)
            .add_plugins(PausePlugin)
            .init_resource::<ButtonInput<KeyCode>>();
        app.update();
        let ask = |app: &mut App, active: bool| {
            app.world_mut().resource_mut::<Halt>().asked = Some(active);
            app.update();
            app.update();
            *app.world().resource::<State<Play>>().get()
        };
        // `GameActive 0` stands the game with no menu, `GameActive 1` lets it run (#2425).
        assert_eq!(ask(&mut app, false), Play::Paused);
        assert!(app.world().resource::<Still>().0);
        assert!(app.world().resource::<Time<Virtual>>().is_paused());
        assert_eq!(ask(&mut app, true), Play::Running);
        assert!(!app.world().resource::<Still>().0);
        // A pause of the player's they leave alone.
        app.world_mut()
            .resource_mut::<NextState<Play>>()
            .set(Play::Paused);
        app.update();
        assert_eq!(ask(&mut app, true), Play::Paused);
    }
}
