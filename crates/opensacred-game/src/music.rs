//! Music: until the regions' music is recovered (M4), the tracks of the `PEACE` group of the
//! `music` sound profile play one after another, a random one first, each fading in. Only the
//! track playing is loaded: `bevy_kira_audio` decodes a whole file into memory, some 85 MB for a
//! track of four minutes, as it does not stream.
//!
//! The profile names the music by sound id in groups that the client names `PEACE`, `NEARBY`,
//! `FIGHT` and the jingles of danger and of fights (docs/re/sound.md): `PEACE` and `NEARBY` hold
//! the wood tracks (`MUSIC_WOOD01` to `03`), `FIGHT` the first four fight tracks. `PEACE` plays
//! here; where the client switches to the others is not read yet.

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use bevy::log::debug;
use bevy::prelude::{AssetServer, Handle, Res, ResMut, Resource, Time};
use bevy_kira_audio::prelude::{
    AudioChannel, AudioControl, AudioInstance, AudioSource, Decibels, PlaybackState,
};
use opensacred_data::sound::Sound;

const PROFILE: &str = "music";
/// The `PEACE` group of the profile.
const PEACE: usize = 0;
const FADE_IN: Duration = Duration::from_secs(2);
/// Below the effects, so that the footsteps stay audible.
const VOLUME: Decibels = Decibels(-8.0);
/// How long a track may wait for its file before the next one is tried: a file that fails to
/// load or decode would otherwise stop the music for good.
const WAIT: Duration = Duration::from_secs(30);

/// The channel of the music, apart from the effects.
#[derive(Resource)]
pub struct Channel;

#[derive(Resource)]
pub struct Music {
    /// The tracks, relative to the asset directory.
    files: Vec<String>,
    next: usize,
    playing: Option<Playing>,
}

struct Playing {
    /// Held while the track plays, so that it stays loaded.
    _track: Handle<AudioSource>,
    instance: Handle<AudioInstance>,
    /// Whether it has started: a track waits for its file to load.
    started: bool,
    /// When it was asked for.
    asked: Duration,
}

impl Music {
    /// Reads the tracks of the `music` profile's `PEACE` group.
    pub fn read(assets: &Path) -> Result<Self> {
        let path = assets.join("sound").join(opensacred_data::sound::INDEX);
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let sound: Sound =
            ron::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        let ids = sound
            .profiles
            .iter()
            .find(|p| p.name == PROFILE)
            .and_then(|p| p.groups.get(PEACE))
            .cloned()
            .unwrap_or_default();
        let files = ids
            .iter()
            .filter_map(|id| sound.music.iter().find(|m| m.id == Some(*id)))
            .map(|m| format!("sound/{}", m.file))
            .collect();
        // A random first track, by the clock.
        let next = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        Ok(Self {
            files,
            next: usize::try_from(next).unwrap_or(0),
            playing: None,
        })
    }
}

/// Starts the next track once the one playing has ended, or has waited too long to start.
pub fn play(
    mut music: ResMut<Music>,
    audio: Res<AudioChannel<Channel>>,
    assets: Res<AssetServer>,
    time: Res<Time>,
) {
    if music.files.is_empty() {
        return;
    }
    let now = time.elapsed();
    if let Some(playing) = &mut music.playing {
        let waited = now.saturating_sub(playing.asked) >= WAIT;
        match audio.state(&playing.instance) {
            PlaybackState::Stopped if playing.started => {}
            PlaybackState::Stopped | PlaybackState::Queued if !waited => return,
            PlaybackState::Stopped | PlaybackState::Queued => {
                debug!(
                    "music: {} did not start",
                    music.files[music.next.saturating_sub(1) % music.files.len()]
                );
                audio.stop();
            }
            _ => {
                playing.started = true;
                return;
            }
        }
    }
    let index = music.next % music.files.len();
    debug!("music: {}", music.files[index]);
    let track = assets.load(music.files[index].clone());
    let instance = audio
        .play(track.clone())
        .with_volume(VOLUME)
        .linear_fade_in(FADE_IN)
        .handle();
    music.playing = Some(Playing {
        _track: track,
        instance,
        started: false,
        asked: now,
    });
    music.next = index + 1;
}
