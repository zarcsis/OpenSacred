//! Music: the music of the sector the hero is in, as the client plays it in peace by day
//! (docs/re/sound.md, "The region's music"): each sector names its music by sound id
//! (`sectors.keyx +0x2D8`: `MUSIC_WOOD01` at the start, `MUSIC_DESERT01`, `MUSIC_KARIBIK`, …), and
//! entering a sector of another music switches to it, fading; a track that ends plays again. In a
//! sector without music the tracks of the `PEACE` group of the `music` sound profile play one
//! after another, a random one first. Each track fades in. Only the track playing is loaded:
//! `bevy_kira_audio` decodes a whole file into memory, some 85 MB for a track of four minutes, as
//! it does not stream.
//!
//! The profile names the music by sound id in groups that the client names `PEACE`, `NEARBY`,
//! `FIGHT` and the jingles of danger and of fights (docs/re/sound.md): `PEACE` and `NEARBY` hold
//! the wood tracks (`MUSIC_WOOD01` to `03`), `FIGHT` the first four fight tracks; where the client
//! switches to the others is not read yet. By night no track starts (the client plays none, its
//! phase of the day at `+0xA34` being 2, docs/re/daynight.md); the one playing plays to its end.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use bevy::log::debug;
use bevy::prelude::{
    App, AssetServer, Handle, IntoScheduleConfigs, OnExit, Plugin, Query, Res, ResMut, Resource,
    Time, Transform, Update, With, resource_exists,
};
use bevy_kira_audio::prelude::{
    AudioChannel, AudioControl, AudioInstance, AudioSource, Decibels, PlaybackState,
};
use opensacred_data::sound::Sound;
use opensacred_data::world::{SIDE, World};

use crate::daynight::{Clock, Phase};
use crate::game::AppState;
use crate::{frame, hero, iso};
use bevy_kira_audio::AudioApp;

const PROFILE: &str = "music";
/// The `PEACE` group of the profile.
const PEACE: usize = 0;
const FADE_IN: Duration = Duration::from_secs(2);
/// How long the music of the sector left fades.
const FADE_OUT: Duration = Duration::from_secs(2);
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
    /// The tracks of the `PEACE` group, relative to the asset directory.
    files: Vec<String>,
    next: usize,
    /// The track of each sector's music, by the sector.
    sectors: HashMap<(u32, u32), String>,
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
    /// The track, relative to the asset directory.
    file: String,
    /// The sector's music it is, or `None` for a track of `PEACE`.
    region: Option<String>,
}

impl Music {
    /// Reads the tracks of the `music` profile's `PEACE` group and the sectors' music.
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
        let track = |id: u32| {
            sound
                .music
                .iter()
                .find(|m| m.id == Some(id))
                .map(|m| format!("sound/{}", m.file))
        };
        let files = ids.iter().filter_map(|&id| track(id)).collect();
        let path = assets.join("world").join(opensacred_data::world::INDEX);
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let world: World =
            ron::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        let sectors = world
            .sectors
            .iter()
            .filter(|s| s.music != 0)
            .filter_map(|s| Some(((s.x, s.y), track(s.music)?)))
            .collect();
        // A random first track, by the clock.
        let next = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        Ok(Self {
            files,
            next: usize::try_from(next).unwrap_or(0),
            sectors,
            playing: None,
        })
    }
}

/// The sector of world `(x, z)`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the sector of a point in the world, a small non-negative whole number"
)]
pub fn sector_of(x: f32, z: f32) -> (u32, u32) {
    let side = SIDE as f32 * iso::CELL;
    ((x / side).max(0.0) as u32, (z / side).max(0.0) as u32)
}

/// Starts the next track once the one playing has ended, or has waited too long to start, and
/// the music of the sector the hero enters when it is another.
pub fn play(
    mut music: ResMut<Music>,
    audio: Res<AudioChannel<Channel>>,
    assets: Res<AssetServer>,
    time: Res<Time>,
    bodies: Query<&Transform, With<hero::Body>>,
    clock: Res<Clock>,
) {
    // Nothing before the hero is in the world: its sector chooses.
    let Some(hero) = bodies.iter().next() else {
        return;
    };
    let region = music
        .sectors
        .get(&sector_of(hero.translation.x, hero.translation.z))
        .cloned();
    let now = time.elapsed();
    // Another sector's music: now, the one playing fading out. Out of a sector with music into
    // one without, the track plays to its end.
    let switch = region.is_some()
        && music
            .playing
            .as_ref()
            .is_some_and(|playing| playing.region != region);
    if switch {
        audio.stop().linear_fade_out(FADE_OUT);
        music.playing = None;
    }
    if region.is_none() && music.files.is_empty() {
        return;
    }
    if let Some(playing) = &mut music.playing {
        let waited = now.saturating_sub(playing.asked) >= WAIT;
        match audio.state(&playing.instance) {
            PlaybackState::Stopped if playing.started => {}
            PlaybackState::Stopped | PlaybackState::Queued if !waited => return,
            PlaybackState::Stopped | PlaybackState::Queued => {
                debug!("music: {} did not start", playing.file);
                audio.stop();
            }
            _ => {
                playing.started = true;
                return;
            }
        }
    }
    if clock.phase() == Phase::Night {
        music.playing = None;
        return;
    }
    let file = match &region {
        Some(file) => file.clone(),
        None => {
            let index = music.next % music.files.len();
            music.next = index + 1;
            music.files[index].clone()
        }
    };
    debug!("music: {file}");
    let track = assets.load(file.clone());
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
        file,
        region,
    });
}

/// Stops the music of the game left, at once.
pub fn leave(audio: Res<AudioChannel<Channel>>) {
    audio.stop();
}

/// The region's music, in a game whose music is on (`game`).
pub struct MusicPlugin;

impl Plugin for MusicPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_channel::<Channel>()
            .add_systems(
                Update,
                play.in_set(frame::Playing).run_if(resource_exists::<Music>),
            )
            .add_systems(OnExit(AppState::InGame), leave);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_lies_in_the_sector_of_its_cell() {
        let start = iso::ground(50.0 * 64.0 + 36.5, 39.0 * 64.0 + 15.5);
        assert_eq!(sector_of(start.x, start.z), (50, 39));
        let corner = iso::ground(51.0 * 64.0, 40.0 * 64.0);
        assert_eq!(sector_of(corner.x + 0.01, corner.z + 0.01), (51, 40));
    }
}
