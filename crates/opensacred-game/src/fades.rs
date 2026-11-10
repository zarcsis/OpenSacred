//! The scripts' fades (#2463; docs/re/netscript.md, `InfoPlayer`): `InfoPlayer`'s mode 5 fades
//! the whole screen, the interface with it, to black over 2.5 s and holds it black, and its mode
//! 7 fades from black back to the play over 2.5 s, the overlay gone then (the engine's event
//! `(0x13, 4, 1 or 0)`, `cEngine::setFadeMode` at `0x613370`'s case 4, drawn by
//! `cEngine_renderFadeOverlay` `0x60E060` after the interface, its alpha `min(255,
//! t·255/2500)` by the frames' time). Each starts its time anew, so that 5 and then 7 at once
//! snap to black and fade in: a new game's start (the start script's 5, `Pre_Tutorial`'s 7),
//! Malorka's boat and the cutscenes of quests 45, 55 and 81. A movie shows over it; the cursor's
//! own image, the system's, stays over it.

use bevy::prelude::*;

use crate::game::AppState;
use crate::scripts;

/// The fades' modes of `InfoPlayer`: out to black, and in from it.
const OUT: i32 = 5;
const IN: i32 = 7;
/// How long a fade runs, in seconds (`0x60E060`: 2,500 ms).
const SECONDS: f32 = 2.5;
/// The overlay's layer: over the interface and its windows, under the movies (`movies`) and the
/// tools.
const LAYER: i32 = i32::MAX - 4;

/// The fade that runs: out to black or in from it, and the real time at which it began.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
struct Fade {
    out: bool,
    since: f32,
}

impl Fade {
    /// Its black's share at `now`: rising from 0 to 1 over [`SECONDS`] out, falling from 1 to 0
    /// in.
    fn black(&self, now: f32) -> f32 {
        let done = ((now - self.since) / SECONDS).clamp(0.0, 1.0);
        if self.out { done } else { 1.0 - done }
    }

    /// Whether it is over: a fade in once its black is gone; a fade out holds black.
    fn over(&self, now: f32) -> bool {
        !self.out && now - self.since >= SECONDS
    }
}

/// The overlay that the fades darken the screen with.
#[derive(Component)]
struct Overlay;

/// Takes the fades' modes from the scripts' `InfoPlayer`s before their texts are shown, the last
/// of the frame's counting.
fn take(mut infos: ResMut<scripts::Infos>, mut commands: Commands, time: Res<Time<Real>>) {
    let mut last = None;
    infos.0.retain(|(mode, _)| match *mode {
        OUT | IN => {
            last = Some(*mode == OUT);
            false
        }
        _ => true,
    });
    if let Some(out) = last {
        debug!("fades: the scripts fade {}", if out { "out" } else { "in" });
        commands.insert_resource(Fade {
            out,
            since: time.elapsed_secs(),
        });
    }
}

/// Draws the fade that runs over the screen, and takes the overlay away once a fade in is over.
fn draw(
    mut commands: Commands,
    fade: Option<Res<Fade>>,
    time: Res<Time<Real>>,
    mut overlays: Query<(Entity, &mut BackgroundColor), With<Overlay>>,
) {
    let now = time.elapsed_secs();
    let Some(fade) = fade else {
        for (overlay, _) in &overlays {
            commands.entity(overlay).despawn();
        }
        return;
    };
    if fade.over(now) {
        commands.remove_resource::<Fade>();
        return;
    }
    let colour = Color::BLACK.with_alpha(fade.black(now));
    match overlays.iter_mut().next() {
        Some((_, mut background)) => background.0 = colour,
        None => {
            commands.spawn((
                Overlay,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(colour),
                GlobalZIndex(LAYER),
                Pickable::IGNORE,
                DespawnOnExit(AppState::InGame),
            ));
        }
    }
}

/// Drops a fade as the game ends.
fn clear(mut commands: Commands) {
    commands.remove_resource::<Fade>();
}

/// The scripts' fades.
pub struct FadesPlugin;

impl Plugin for FadesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                take.after(crate::talk::popups).before(crate::talk::notices),
                draw,
            )
                .chain()
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(OnExit(AppState::InGame), clear);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fade_out_darkens_over_2_5_s_and_holds_and_a_fade_in_clears() {
        let out = Fade {
            out: true,
            since: 10.0,
        };
        assert_eq!(out.black(10.0), 0.0);
        assert!((out.black(11.25) - 0.5).abs() < 1e-6);
        assert_eq!(out.black(12.5), 1.0);
        assert_eq!(out.black(60.0), 1.0);
        assert!(!out.over(60.0));
        let back = Fade {
            out: false,
            since: 10.0,
        };
        assert_eq!(back.black(10.0), 1.0);
        assert!((back.black(11.25) - 0.5).abs() < 1e-6);
        assert!(!back.over(12.4));
        assert!(back.over(12.5));
    }

    #[test]
    fn the_last_fade_of_a_frame_counts_and_the_other_infos_stay() {
        let mut app = App::new();
        app.init_resource::<scripts::Infos>()
            .init_resource::<Time<Real>>()
            .add_systems(Update, take);
        app.world_mut().resource_mut::<scripts::Infos>().0 = vec![
            (OUT, String::new()),
            (1, "res:17631".into()),
            (IN, String::new()),
        ];
        app.update();
        assert_eq!(
            app.world().resource::<scripts::Infos>().0,
            [(1, "res:17631".to_owned())]
        );
        assert!(!app.world().resource::<Fade>().out);
    }
}
