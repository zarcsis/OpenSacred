//! The atmosphere: an ambient loop on a channel of its own beside the music, chosen by the nature
//! of the cell under the hero (docs/02 §5.2), as the client chooses it in a switch at `0x696C27`:
//! `DESERT` and `KIES` play `ATMO_DESERT`, `ARCTIC` `ATMO_ICE`, `WALD`, `GRASS` and `SUMPF`
//! `ATMO_WOOD`, `LAVA` `ATMO_VULCANO`, `DUNGEON` `ATMO_DUNGEON`; the other natures (bridges,
//! waters, roads, floors) keep the loop that plays. The same function picks the `…_NIGHT` loops
//! in the dusk and by night (a second switch at `0x696BDA`, by the phase of the day at `+0xA34`,
//! docs/re/daynight.md) and leaves the loop alone while some regions' music plays
//! (docs/re/sound.md); the regions' exceptions are not followed.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use bevy::log::debug;
use bevy::prelude::{AssetServer, Handle, Query, Res, ResMut, Resource, Time, Transform, With};
use bevy_kira_audio::prelude::{AudioChannel, AudioControl, AudioSource, AudioTween, Decibels};
use opensacred_data::sound::Sound;

use crate::daynight::{Clock, Phase};
use crate::hero::Body;
use crate::terrain::Heights;

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
const FADE: Duration = Duration::from_secs(3);
/// How long the hero stays on ground that calls for another loop before it changes: walking along
/// a border changes nothing.
const SETTLE: Duration = Duration::from_secs(2);
/// Below the music.
const VOLUME: Decibels = Decibels(-12.0);

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
    /// The loop the ground calls for, and since when.
    wanted: Option<(String, Duration)>,
    /// Every loop played so far, kept loaded: `bevy_kira_audio` decodes a whole file, and there
    /// are five loops.
    loaded: HashMap<String, Handle<AudioSource>>,
}

impl Atmosphere {
    /// Finds the loops' files by their sound names (`sound/sound.ron`).
    pub fn read(assets: &Path) -> Result<Self> {
        let path = assets.join("sound").join(opensacred_data::sound::INDEX);
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let sound: Sound =
            ron::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        let file = |name: &str| {
            let id = sound.names.iter().find(|n| n.name == name)?.id;
            let music = sound.music.iter().find(|m| m.id == Some(id))?;
            Some(format!("sound/{}", music.file))
        };
        Ok(Self {
            files: DAY.map(|name| name.and_then(file)),
            night: NIGHT.map(|name| name.and_then(file)),
            playing: None,
            wanted: None,
            loaded: HashMap::new(),
        })
    }
}

/// Changes the loop once the hero has stayed on a nature with another one for `SETTLE`.
pub fn choose(
    mut atmosphere: ResMut<Atmosphere>,
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
    let Some(nature) = heights.nature(hero.translation.x, hero.translation.z) else {
        return;
    };
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
