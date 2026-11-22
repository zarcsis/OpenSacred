//! The sound's three volumes and its switch (#2341; docs/re/sound.md, "The volumes"): the
//! effects', the speech's and the music's, each a level from 0 to 1 (the sound manager's `+0x78`,
//! `+0x74` and `+0x7C`) that every sound of its kind plays at, times its own.
//!
//! Each channel plays through a track of its own whose volume control holds its kind's level: the
//! main channel's effects, the water's and the rain's loops at the effects'; the lines by name on
//! [`Speech`] and the choice of the hero's introductions at the speech's; the music once at the
//! music's; and the music's and the ambient loops' channels at the music's or, for a stream whose
//! name starts with `ATMO`, the effects' (`0x677A60`, `0x677750`). The movies play apart, at no
//! volume of these (`0x6A0D80`), through a track of their own whose control stays whole.
//!
//! The levels are the settings' 0 to 125 over 128 at the start (`initApp`), and over 126 while a
//! slider of the options window moves (`0x716E00`); the switch off silences every track but the
//! movies' (`SOUND`, `0x6902F0`), the sliders then moving no level.
//!
//! A run muted (`--mute`, [`Mute`]) holds every track silent from its start, the movies' too,
//! whatever the levels, so that a game driven in the background is not heard (#3412); the game
//! plays on as it would, its levels, its music and its sounds as a player's.

use bevy::prelude::*;
use bevy_kira_audio::effect::{VolumeControlBuilder, VolumeControlHandle};
use bevy_kira_audio::prelude::{AudioTween, Decibels};
use bevy_kira_audio::{AudioApp, AudioTrack, MainTrack};

use crate::settings::Settings;

/// The kinds of sound, each at its own volume.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Effects,
    Speech,
    Music,
}

/// The channel of the lines by name: the hero's comments and battle cries, the hirelings' lines,
/// the dialogues' voice-overs and the scripts' `PlaySound`s (`0x693FE0`), at the speech's volume.
#[derive(Resource)]
pub struct Speech;

/// A run that plays no sound (`--mute`): inserted before the plugins, so that every track starts
/// silent, and kept for the run.
#[derive(Resource)]
pub struct Mute;

/// The levels of the three volumes from 0 to 1, and whether the sound is on.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Levels {
    pub effects: f32,
    pub speech: f32,
    pub music: f32,
    pub on: bool,
}

/// The settings' volume over which the start takes its level (`0x8906F8`, 1/128), and the one
/// over which a slider's position gives its level while it moves (`0x716E00`).
const AT_START: f32 = 128.0;
const AS_MOVED: f32 = 126.0;

impl Default for Levels {
    fn default() -> Self {
        Self::at_start(&crate::settings::Sound::default())
    }
}

impl Levels {
    /// The levels at the start, of the settings' `sound` (`initApp`).
    pub fn at_start(sound: &crate::settings::Sound) -> Self {
        Self {
            effects: f32::from(sound.effects) / AT_START,
            speech: f32::from(sound.speech) / AT_START,
            music: f32::from(sound.music) / AT_START,
            on: sound.on,
        }
    }

    /// The level of the kind `kind`, none while the sound is off.
    pub fn of(&self, kind: Kind) -> f32 {
        if !self.on {
            return 0.0;
        }
        match kind {
            Kind::Effects => self.effects,
            Kind::Speech => self.speech,
            Kind::Music => self.music,
        }
    }

    /// The level of a slider of the options window moved to `position` (`0x716E00`), which the
    /// sound off leaves as it was (`0x6934A0`).
    pub fn moved(&mut self, kind: Kind, position: u8) {
        if !self.on {
            return;
        }
        let level = f32::from(position) / AS_MOVED;
        match kind {
            Kind::Effects => self.effects = level,
            Kind::Speech => self.speech = level,
            Kind::Music => self.music = level,
        }
    }
}

/// What a channel's track takes its level from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lane {
    /// A kind's own channel.
    Of(Kind),
    /// The music's channel: the music's, or the effects' for an ambient or spot loop.
    Music,
    /// The ambient loops' channel: the effects', or the music's for a music that the place takes
    /// for its loop (the pub's, the undead siege's).
    Atmosphere,
    /// The movies' channel, whole.
    Movies,
}

/// The level of the track of `lane` at `levels`: `music_loop` whether the music's channel plays
/// an ambient or spot loop, `music_on_loop` whether the ambient loops' channel plays a music; none
/// in a run `muted`.
fn level(
    lane: Lane,
    levels: &Levels,
    (music_loop, music_on_loop): (bool, bool),
    muted: bool,
) -> f32 {
    if muted {
        return 0.0;
    }
    let kind = match lane {
        Lane::Of(kind) => kind,
        Lane::Music if music_loop => Kind::Effects,
        Lane::Music => Kind::Music,
        Lane::Atmosphere if music_on_loop => Kind::Music,
        Lane::Atmosphere => Kind::Effects,
        Lane::Movies => return 1.0,
    };
    levels.of(kind)
}

/// The channels' tracks' volume controls, and the level each was set to last.
#[derive(Resource, Default)]
struct Controls(Vec<(Lane, VolumeControlHandle, Option<f32>)>);

/// A track with a volume control for the channel `T`, its control kept in `controls`, and the
/// recording's tap after it, which takes the channel's sound at its level (`sound_tap`, #1230);
/// silent from the start in a run muted.
fn track<T: Resource>(app: &mut App, controls: &mut Controls, lane: Lane) {
    let muted = app.world().contains_resource::<Mute>();
    let mut track = AudioTrack::new();
    let control = track.add_effect(VolumeControlBuilder::new(if muted {
        Decibels::SILENCE
    } else {
        Decibels::IDENTITY
    }));
    track.add_effect(crate::sound_tap::TapBuilder);
    app.add_audio_channel_with_track::<T>(track);
    controls.0.push((lane, control, muted.then_some(0.0)));
}

/// The levels at the start, from the settings in force.
fn start(mut commands: Commands, settings: Option<Res<Settings>>) {
    let levels = settings
        .as_deref()
        .map_or_else(Levels::default, |settings| {
            Levels::at_start(&settings.sound)
        });
    commands.insert_resource(levels);
}

/// Sets each track to its kind's level where it changed: the music's and the ambient loops' by
/// what plays on them, every one silent in a run muted.
fn apply(
    levels: Option<Res<Levels>>,
    mut controls: ResMut<Controls>,
    music: Option<Res<crate::music::Music>>,
    atmosphere: Option<Res<crate::atmosphere::Atmosphere>>,
    mute: Option<Res<Mute>>,
) {
    let Some(levels) = levels else {
        return;
    };
    let music_loop = music
        .as_deref()
        .is_some_and(crate::music::Music::plays_atmo);
    let music_on_loop = atmosphere
        .as_deref()
        .is_some_and(crate::atmosphere::Atmosphere::plays_music);
    for (lane, control, set) in &mut controls.0 {
        let level = level(*lane, &levels, (music_loop, music_on_loop), mute.is_some());
        if *set == Some(level) {
            continue;
        }
        *set = Some(level);
        debug!("volumes: {lane:?} at {level:.3}");
        let volume = if level > 0.0 {
            crate::sfx::decibels(level)
        } else {
            Decibels::SILENCE
        };
        control.set_volume(volume, AudioTween::default());
    }
}

/// The volumes' tracks and levels.
pub struct VolumesPlugin;

impl Plugin for VolumesPlugin {
    fn build(&self, app: &mut App) {
        let mut controls = Controls::default();
        track::<MainTrack>(app, &mut controls, Lane::Of(Kind::Effects));
        track::<Speech>(app, &mut controls, Lane::Of(Kind::Speech));
        track::<crate::menu_sound::Introductions>(app, &mut controls, Lane::Of(Kind::Speech));
        track::<crate::ambience::Tiny>(app, &mut controls, Lane::Of(Kind::Effects));
        track::<crate::ambience::Normal>(app, &mut controls, Lane::Of(Kind::Effects));
        track::<crate::ambience::Big>(app, &mut controls, Lane::Of(Kind::Effects));
        track::<crate::skies::RainLoop>(app, &mut controls, Lane::Of(Kind::Effects));
        track::<crate::music::Once>(app, &mut controls, Lane::Of(Kind::Music));
        track::<crate::music::Channel>(app, &mut controls, Lane::Music);
        track::<crate::atmosphere::Channel>(app, &mut controls, Lane::Atmosphere);
        track::<crate::movies::Channel>(app, &mut controls, Lane::Movies);
        app.insert_resource(controls)
            .add_systems(Startup, start)
            .add_systems(PostUpdate, apply);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_start_takes_the_volumes_over_128_and_a_slider_over_126() {
        let mut levels = Levels::at_start(&crate::settings::Sound::default());
        assert_eq!(levels.of(Kind::Effects), 0.5);
        levels.moved(Kind::Music, 125);
        assert!((levels.of(Kind::Music) - 125.0 / 126.0).abs() < 1e-6);
        assert_eq!(levels.of(Kind::Speech), 0.5);
        // The sound off: every kind silent, and a slider moves no level.
        levels.on = false;
        assert_eq!(levels.of(Kind::Effects), 0.0);
        levels.moved(Kind::Effects, 10);
        levels.on = true;
        assert_eq!(levels.of(Kind::Effects), 0.5);
    }

    #[test]
    fn a_run_muted_silences_every_track_and_the_movies_play_whole_otherwise() {
        let mut levels = Levels::at_start(&crate::settings::Sound::default());
        levels.music = 0.25;
        let lanes = [
            Lane::Of(Kind::Effects),
            Lane::Of(Kind::Speech),
            Lane::Of(Kind::Music),
            Lane::Music,
            Lane::Atmosphere,
            Lane::Movies,
        ];
        for lane in lanes {
            assert_eq!(level(lane, &levels, (false, false), true), 0.0, "{lane:?}");
        }
        assert_eq!(level(Lane::Movies, &levels, (false, false), false), 1.0);
        assert_eq!(level(Lane::Music, &levels, (false, false), false), 0.25);
        assert_eq!(level(Lane::Music, &levels, (true, false), false), 0.5);
        assert_eq!(level(Lane::Atmosphere, &levels, (false, true), false), 0.25);
        // The sound's switch off silences all but the movies.
        levels.on = false;
        assert_eq!(
            level(Lane::Of(Kind::Speech), &levels, (false, false), false),
            0.0
        );
        assert_eq!(level(Lane::Movies, &levels, (false, false), false), 1.0);
    }

    #[test]
    fn a_run_muted_starts_its_tracks_silent() {
        // No audio device: the plugin builds its controls alone.
        let mut app = App::new();
        app.insert_resource(Mute).add_plugins(VolumesPlugin);
        let controls = &app.world().resource::<Controls>().0;
        assert!(controls.iter().all(|(_, _, set)| *set == Some(0.0)));
        assert!(controls.iter().any(|(lane, ..)| *lane == Lane::Movies));
        let mut app = App::new();
        app.add_plugins(VolumesPlugin);
        let controls = &app.world().resource::<Controls>().0;
        assert!(controls.iter().all(|(_, _, set)| set.is_none()));
    }
}
