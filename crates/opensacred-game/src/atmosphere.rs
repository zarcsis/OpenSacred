//! The atmosphere: the ambient loop of the peace stream's ambient phase (`music`, #1177), chosen
//! by the nature of the cell under the hero (docs/02 §5.2), as the client chooses it in a switch
//! at `0x696C27`: `DESERT` and `KIES` play `ATMO_DESERT`, `ARCTIC` `ATMO_ICE`, `WALD`, `GRASS`
//! and `SUMPF` `ATMO_WOOD`, `LAVA` `ATMO_VULCANO`, `DUNGEON` `ATMO_DUNGEON`; the other natures
//! (bridges, waters, roads, floors) keep the loop that plays. The same function picks the
//! `…_NIGHT` loops in the dusk and by night (a second switch at `0x696BDA`, by the phase of the
//! day at `+0xA34`, docs/re/daynight.md); the regions' exceptions are not followed. It plays on a
//! channel of its own, rising as the music falls and falling as it rises, over the stream's
//! crossfade of 7 s; the loop changes with the nature under the hero in its phase (**hypothesis**:
//! the original chooses it again at the region's events).

use std::collections::HashMap;
use std::time::Duration;

use bevy::log::debug;
use bevy::prelude::{
    App, AssetServer, Handle, IntoScheduleConfigs, OnExit, Plugin, Query, Res, ResMut, Resource,
    Time, Transform, Update, Vec3, With, resource_exists,
};
use bevy_kira_audio::prelude::{AudioChannel, AudioControl, AudioSource, AudioTween, Decibels};
use opensacred_data::sound::Sound;

use crate::daynight::{Clock, Phase};
use crate::frame::{Playing, Unpaused};
use crate::game::AppState;
use crate::hero::Body;
use crate::terrain::Heights;
use bevy_kira_audio::AudioApp;

/// The ambient loop of each nature by day, by the client's switch; `None` keeps the loop.
const DAY: [Option<&str>; 16] = [
    None,
    Some("SOUND_FX_ATMO_DESERT"),
    Some("SOUND_FX_ATMO_ICE"),
    Some("SOUND_FX_ATMO_WOOD"),
    Some("SOUND_FX_ATMO_WOOD"),
    Some("SOUND_FX_ATMO_WOOD"),
    Some("SOUND_FX_ATMO_VULCANO"),
    None,
    Some("SOUND_FX_ATMO_DUNGEON"),
    None,
    None,
    None,
    None,
    None,
    None,
    Some("SOUND_FX_ATMO_DESERT"),
];
/// The ambient loop of each nature in the dusk and by night (`0x696BDA`).
const NIGHT: [Option<&str>; 16] = [
    None,
    Some("SOUND_FX_ATMO_DESERT_NIGHT"),
    Some("SOUND_FX_ATMO_ICE_NIGHT"),
    Some("SOUND_FX_ATMO_WOOD_NIGHT"),
    Some("SOUND_FX_ATMO_WOOD_NIGHT"),
    Some("SOUND_FX_ATMO_WOOD_NIGHT"),
    Some("SOUND_FX_ATMO_VULCANO_NIGHT"),
    None,
    Some("SOUND_FX_ATMO_WOOD_NIGHT"),
    None,
    None,
    None,
    None,
    None,
    None,
    Some("SOUND_FX_ATMO_DESERT_NIGHT"),
];
/// The peace stream's crossfade (`0x69C4BB`).
const FADE: Duration = Duration::from_secs(7);
/// How long the hero stays on ground that calls for another loop before it changes: walking along
/// a border changes nothing.
const SETTLE: Duration = Duration::from_secs(2);
/// Below the music, at the effects' level (`0x69C4BB`'s target for an `ATMO` name).
pub const VOLUME: Decibels = Decibels(-12.0);

/// The channel of the ambient loops.
#[derive(Resource)]
pub struct Channel;

#[derive(Resource)]
pub struct Atmosphere {
    /// The loop file of each nature, relative to the asset directory, by day and by night.
    files: [Option<String>; 16],
    night: [Option<String>; 16],
    /// The loop file playing.
    playing: Option<String>,
    /// The nature of the last ground of a loop, which a ground of none keeps.
    nature: Option<u8>,
    /// The loop the ground calls for, and since when.
    wanted: Option<(String, Duration)>,
    /// Every loop played so far, kept loaded: `bevy_kira_audio` decodes a whole file, and there
    /// are five loops.
    loaded: HashMap<String, Handle<AudioSource>>,
}

impl Atmosphere {
    /// Finds the loops' files by their sound names in the sound index `sound`.
    pub fn new(sound: &Sound) -> Self {
        let file = |name: &str| {
            let id = sound.names.iter().find(|n| n.name == name)?.id;
            let music = sound.music.iter().find(|m| m.id == Some(id))?;
            Some(crate::sound::music_file(music))
        };
        Self {
            files: DAY.map(|name| name.and_then(file)),
            night: NIGHT.map(|name| name.and_then(file)),
            playing: None,
            nature: None,
            wanted: None,
            loaded: HashMap::new(),
        }
    }
}

/// Plays the loop in the stream's ambient phase and lets it fall in the other, and changes it
/// once the hero has stayed on a nature with another one for `SETTLE`.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
pub fn choose(
    mut atmosphere: ResMut<Atmosphere>,
    music: Option<Res<crate::music::Music>>,
    bodies: Query<&Transform, With<Body>>,
    heights: Res<Heights>,
    time: Res<Time>,
    channel: Res<AudioChannel<Channel>>,
    assets: Res<AssetServer>,
    clock: Res<Clock>,
) {
    let Some(hero) = bodies.iter().next() else {
        return;
    };
    // The music's phase or a spot loop: the loop falls.
    if !music.as_ref().is_some_and(|music| music.ambient()) {
        if atmosphere.playing.take().is_some() {
            channel.stop().fade_out(AudioTween::linear(FADE));
        }
        atmosphere.wanted = None;
        return;
    }
    // The scripts hold the stream: no loop changes (`0x694616`).
    if music.is_some_and(|music| music.holds()) {
        return;
    }
    let has_loop = |nature: u8| {
        atmosphere
            .files
            .get(usize::from(nature))
            .is_some_and(Option::is_some)
    };
    // A ground of no loop keeps the one before; with none before, the loop of the nearest ground
    // about the hero that has one (**hypothesis**: the original's nature under a building, at the
    // start in the chapel of 50,39 `ATMO_WOOD`, is its ground's about it).
    let here = heights
        .nature(hero.translation.x, hero.translation.z)
        .filter(|&nature| has_loop(nature));
    let Some(nature) = here
        .or(atmosphere.nature)
        .or_else(|| nearest(&heights, hero.translation, has_loop))
    else {
        return;
    };
    atmosphere.nature = Some(nature);
    let files = match clock.phase() {
        Phase::Dusk | Phase::Night => &atmosphere.night,
        Phase::Day | Phase::Dawn => &atmosphere.files,
    };
    let Some(Some(file)) = files.get(usize::from(nature)).cloned() else {
        return;
    };
    let now = time.elapsed();
    if atmosphere.playing.as_ref() == Some(&file) {
        atmosphere.wanted = None;
        return;
    }
    // The first loop plays at once; a change waits for the hero to stay.
    if atmosphere.playing.is_some() {
        match &atmosphere.wanted {
            Some((wanted, since)) if *wanted == file => {
                if now.saturating_sub(*since) < SETTLE {
                    return;
                }
            }
            _ => {
                atmosphere.wanted = Some((file, now));
                return;
            }
        }
    }
    debug!("atmosphere: {file}");
    let track = atmosphere
        .loaded
        .entry(file.clone())
        .or_insert_with(|| assets.load(file.clone()))
        .clone();
    channel.stop().fade_out(AudioTween::linear(FADE));
    channel
        .play(track)
        .looped()
        .with_volume(VOLUME)
        .fade_in(AudioTween::linear(FADE));
    atmosphere.playing = Some(file);
    atmosphere.wanted = None;
}

/// How far about the hero, in cells, a ground of a loop is looked for ([`nearest`]).
const NEAREST: i32 = 12;

/// The nature of the ground nearest the cell of `at` whose nature `has_loop`, ring by ring up to
/// [`NEAREST`] cells, none where none is in.
fn nearest(heights: &Heights, at: Vec3, has_loop: impl Fn(u8) -> bool) -> Option<u8> {
    let (x, y) = crate::paths::cell_of(at.x, at.z);
    (1..=NEAREST).find_map(|ring| {
        (-ring..=ring)
            .flat_map(|dy| (-ring..=ring).map(move |dx| (dx, dy)))
            .filter(|&(dx, dy)| dx.abs() == ring || dy.abs() == ring)
            .find_map(|(dx, dy)| {
                let point = crate::iso::ground((x + dx) as f32 + 0.5, (y + dy) as f32 + 0.5);
                heights
                    .nature(point.x, point.z)
                    .filter(|&nature| has_loop(nature))
            })
    })
}

/// Stops the ambient loop of the game left, at once.
pub fn leave(channel: Res<AudioChannel<Channel>>) {
    channel.stop();
}

/// The ambient loops, in a game whose music is on (`game`).
pub struct AtmospherePlugin;

impl Plugin for AtmospherePlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_channel::<Channel>()
            .add_systems(
                Update,
                choose
                    .after(crate::music::alternate)
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(resource_exists::<Atmosphere>),
            )
            .add_systems(OnExit(AppState::InGame), leave);
    }
}
