//! Music: the peace stream of the region's music and the ambient loop in turns, as the client
//! plays them (docs/re/sound.md, "The region's music" and "The peace stream's alternation";
//! #1177). Each sector names its music by sound id (`sectors.keyx +0x2D8`: `MUSIC_WOOD01` at the
//! start, `MUSIC_DESERT01`, `MUSIC_KARIBIK`, …); by day the music and the ambient loop of the
//! place's nature (`atmosphere`) take the stream for 180 s each in turn
//! (`opensacred_rules::music::Stream`), the first by a roll, crossfading over 7 s, the tracks
//! looping. The dusk and the night ask for the ambient loop and hold the timer, so that the music
//! comes back 180 s into the dawn or the day; a sector without music keeps the ambient loop;
//! entering a sector of another music in the music's phase crossfades to it. The spot loops of
//! the cemetery and the arenas (`ATMOSPOT_*`) take the stream while the hero stands in their
//! sectors, the cycle held (**hypothesis** for what plays there: the original plays the arenas'
//! on a stream of their own and asks for the ambient loop at the cemetery). Only the tracks
//! playing are loaded: `bevy_kira_audio` decodes a whole file into memory, some 85 MB for a track
//! of four minutes, as it does not stream.
//!
//! The campaign's sieges and its world's change override a sector's music (`0x67A7A0`, #55,
//! [`music_of`]): the scripts' variables `RegionChange`, a bit for each sieged region, and
//! `Globalchange1`, whose bit 1 the evil's turn of the quest 45 sets, give the villages' and
//! Mascarell's music the military siege's, and any music of a sieged region the undead siege's.
//!
//! The hero's death plays `MUSIC_DEATH` once over the stream, on a channel of its own
//! (`0x694FE0`, the once-stream `+0x60C`, #734), at once and unfaded, and turns the stream to the
//! ambient loop (`0x6853F7`).
//!
//! The scripts play on both (#1243, docs/re/sound.md, "Scripts and notices"): a `PlayMusic`
//! crossfades the stream to its music until the stream turns, the hero comes into a sector of
//! music or the scripts give the stream back; a `PlayJingle` plays once over it, turning the
//! music's phase to the ambient loop as the death does; `SetScriptSoundControle` takes the stream
//! from every other music for 300 s at most (`opensacred_rules::music::Stream::control`), the
//! death's music and the fanfares too; and the `ExitQuest` of a side quest sounds a fanfare, the
//! music not dipping under it (docs/decisions/sound.md, "Music kept under a fanfare").

use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bevy::log::debug;
use bevy::prelude::{
    Added, App, AssetServer, Assets, Handle, IntoScheduleConfigs, MessageReader, NonSend, OnExit,
    Plugin, Query, Res, ResMut, Resource, Time, Transform, Update, With, resource_exists,
};
use bevy_kira_audio::prelude::{
    AudioChannel, AudioControl, AudioInstance, AudioSource, AudioTween, Decibels,
};
use opensacred_data::sound::Sound;
use opensacred_data::world::{SIDE, World};
use opensacred_rules::music::{CROSSFADE, Phase, Stream};
use opensacred_rules::spawning::Rng;

use crate::daynight::{self, Clock};
use crate::game::AppState;
use crate::npcs::Dies;
use crate::scripts::ScriptSound;
use crate::{frame, hero, iso};
use bevy_kira_audio::AudioApp;

/// `MUSIC_DEATH`, the music of the hero's death.
const DEATH: u32 = 0x1969;
/// `JINGLE_FANFARE01` and `02`, the fanfares of the side quests' ends, and the third, which none
/// plays (`0x67E3D5`).
const FANFARES: [u32; 3] = [0x198D, 0x198E, 0x198F];
/// `ATMOSPOT_ARENA_INDOOR`, whose sectors play nothing once (`0x694FE0`).
const ARENA_INDOOR: u32 = 0x196C;
/// The prefix of the sounds' names, which the scripts may leave out (`0x4ADAA0`).
const PREFIX: &str = "SOUND_FX_";
/// `MUSIC_VILLAGE01` and `MUSIC_MASCARELL`, and the sieges' music that takes their place,
/// `MUSIC_VILLAGE_SIEGE_MILITARY` and `MUSIC_VILLAGE_SIEGE_UNDEAD` (`0x67A7A0`).
const VILLAGE: u32 = 0x1996;
const MASCARELL: u32 = 0x1990;
const SIEGE_MILITARY: u32 = 0x1997;
const SIEGE_UNDEAD: u32 = 0x1998;
/// The spot loops that sectors name for their music: `ATMOSPOT_CEMETERY`,
/// `ATMOSPOT_ARENA_INDOOR` and `ATMOSPOT_ARENA_OUTDOOR` (`0x67A7A0`'s own ways for them).
const SPOTS: [u32; 3] = [0x1968, 0x196C, 0x196D];
/// The scripts' variables of the sieges and of the world's change, found case aside
/// (`0x6263B0`).
const SIEGES: &str = "RegionChange";
const CHANGES: &str = "Globalchange1";
/// The crossfade of the peace stream (`0x69C4BB`).
const FADE: Duration = Duration::from_secs(7);
/// Below the effects, so that the footsteps stay audible.
pub const VOLUME: Decibels = Decibels(-8.0);

/// The channel of the music, apart from the effects.
#[derive(Resource)]
pub struct Channel;

/// The channel of a music played once over the stream, the death's (the once-stream `+0x60C`).
#[derive(Resource)]
pub struct Once;

#[derive(Resource)]
pub struct Music {
    /// Each sector's music and area, by the sector, and the track of each music.
    sectors: HashMap<(u32, u32), (u32, u8)>,
    tracks: HashMap<u32, String>,
    /// Each sound's id by its name, lower-cased, and the ambient loops' ids (`ATMO…`), which play
    /// at the effects' level.
    ids: HashMap<String, u32>,
    atmo: HashSet<u32>,
    /// The peace stream's phase and timer.
    stream: Stream,
    /// Until when the stream's crossfade runs, in the game's time.
    fading: Duration,
    /// Whether the last frame was of the dusk or the night.
    dark: bool,
    /// Whether the hero stands in a sector of a spot loop.
    spot: bool,
    /// The sector the hero stood in at the last frame.
    sector: Option<(u32, u32)>,
    /// The music that the scripts' `PlayMusic` put on the stream, until the stream turns.
    scripted: Option<u32>,
    /// The rolls of the fanfares.
    dice: Rng,
    /// The track on the music's channel.
    playing: Option<Playing>,
    /// The death's music, held while it plays.
    once: Option<Handle<AudioSource>>,
}

struct Playing {
    /// Held while the track plays, so that it stays loaded.
    _track: Handle<AudioSource>,
    instance: Handle<AudioInstance>,
    /// The track, relative to the asset directory.
    file: String,
}

impl Music {
    /// Takes the sectors' music from the world index `world` and the tracks from the sound index
    /// `sound`.
    pub fn new(world: &World, sound: &Sound) -> Self {
        let sectors = world
            .sectors
            .iter()
            .filter(|s| s.music != 0)
            .map(|s| ((s.x, s.y), (s.music, s.area)))
            .collect();
        let tracks: HashMap<u32, String> = sound
            .music
            .iter()
            .filter_map(|m| Some((m.id?, crate::sound::music_file(m))))
            .collect();
        let ids = sound
            .names
            .iter()
            .map(|name| (name.name.to_ascii_lowercase(), name.id))
            .collect();
        let atmo = sound
            .names
            .iter()
            .filter(|name| {
                name.name
                    .strip_prefix(PREFIX)
                    .is_some_and(|rest| rest.starts_with("ATMO"))
            })
            .map(|name| name.id)
            .collect();
        let roll = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        Self {
            sectors,
            tracks,
            ids,
            atmo,
            stream: Stream::default(),
            fading: Duration::ZERO,
            dark: false,
            spot: false,
            sector: None,
            scripted: None,
            dice: Rng::of(roll, 0, 0x1243),
            playing: None,
            once: None,
        }
    }

    /// Whether the ambient loop of the place takes the stream now (`atmosphere`).
    pub fn ambient(&self) -> bool {
        !self.spot && self.scripted.is_none() && self.stream.phase() == Some(Phase::Ambient)
    }

    /// Whether the scripts hold the stream, so that the ambient loop does not change either.
    pub fn holds(&self) -> bool {
        self.stream.controlled()
    }

    /// The id of the sound that the scripts name `name`: `SOUND_FX_` put before it unless its
    /// first eight characters are `SOUND_FX`, found case aside (`0x4ADAA0`, `0x676170`); none for
    /// a name of no sound.
    fn id_of(&self, name: &str) -> Option<u32> {
        let named = name
            .get(..PREFIX.len() - 1)
            .is_some_and(|start| start.eq_ignore_ascii_case(&PREFIX[..PREFIX.len() - 1]));
        let full = if named {
            name.to_ascii_lowercase()
        } else {
            format!("{PREFIX}{name}").to_ascii_lowercase()
        };
        self.ids.get(&full).copied()
    }

    /// Plays the music `id` once on the once-stream, at once, stopping what plays there
    /// (`0x694FE0`): nothing in a sector of the indoor arena's music, nor for a music of no track.
    fn once(&mut self, id: u32, once: &AudioChannel<Once>, assets: &AssetServer) {
        let arena = self
            .sector
            .and_then(|sector| self.sectors.get(&sector))
            .is_some_and(|&(music, _)| music == ARENA_INDOOR);
        let Some(file) = self.tracks.get(&id).filter(|_| !arena).cloned() else {
            return;
        };
        debug!("music: {file} once");
        let track = assets.load(file);
        once.stop();
        once.play(track.clone()).with_volume(VOLUME);
        self.once = Some(track);
    }
}

/// The music that a sector of the music `id` in the area `area` plays while the scripts' sieges
/// are `sieges` and their world's change `changes` (`0x67A7A0`): the villages' and Mascarell's
/// the military siege's in the regions 8 to 10 and `0x6F`, and with the change's bit 1 in the
/// regions 2, 4 to 10, 12 to 17, `0x6E` and `0x6F`; any music the undead siege's in a region
/// whose bit the sieges hold, the bit 11 for the region `0x6E`.
fn music_of(id: u32, area: u8, sieges: u32, changes: u32) -> u32 {
    let region = opensacred_rules::quests::region(area);
    let mut music = id;
    if id == VILLAGE || id == MASCARELL {
        if matches!(region, 8..=10 | 0x6F) {
            music = SIEGE_MILITARY;
        }
        if changes & 2 != 0 && matches!(region, 2 | 4..=10 | 12..=17 | 0x6E | 0x6F) {
            music = SIEGE_MILITARY;
        }
    }
    let bit = if region == 0x6E { 11 } else { region };
    if sieges & (1 << (bit & 31)) != 0 {
        music = SIEGE_UNDEAD;
    }
    music
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

/// What the music's channel plays where the hero stands: a sector's spot loop, held whatever
/// the phase, else in the music's phase the sector's music, none without one.
fn wanted(music: &Music, id: Option<u32>) -> Option<u32> {
    if music.scripted.is_some() {
        return music.scripted;
    }
    let id = id?;
    let spot = SPOTS.contains(&id);
    if !spot && music.stream.phase() != Some(Phase::Music) {
        return None;
    }
    Some(id)
}

/// Carries out what the scripts play (`scripts::ScriptSound`, #1243): their control taken or
/// given back, the stream crossfading back to what its phase holds; their music put on the
/// stream; their jingles once over it, a jingle but the fanfares turning the music's phase to the
/// ambient loop (`0x6952B8`); and a side quest's fanfare, `JINGLE_FANFARE01` two times in three
/// and `02` the third (`0x67E3D5`), not while the scripts hold the stream.
pub fn scripted(
    mut music: ResMut<Music>,
    mut sounds: MessageReader<ScriptSound>,
    (once, assets): (Res<AudioChannel<Once>>, Res<AssetServer>),
) {
    for sound in sounds.read() {
        match sound {
            ScriptSound::Control(on) => {
                debug!(
                    "music: the scripts {} the stream",
                    if *on { "take" } else { "give back" }
                );
                music.stream.control(*on);
                if !on {
                    music.scripted = None;
                }
            }
            // A sound of no music file does not open, and the stream keeps its track (`0x6761C0`).
            ScriptSound::Music(name) => match music.id_of(name) {
                Some(id) if music.tracks.contains_key(&id) => {
                    debug!("music: the scripts play {name}");
                    music.scripted = Some(id);
                }
                _ => debug!("music: the scripts' music {name} is no music"),
            },
            ScriptSound::Jingle(name) => {
                let Some(id) = music.id_of(name) else {
                    debug!("music: the scripts' jingle {name} is no sound");
                    continue;
                };
                if !FANFARES.contains(&id) {
                    music.stream.now();
                }
                music.once(id, &once, &assets);
            }
            ScriptSound::Fanfare => {
                if music.stream.controlled() {
                    continue;
                }
                let id = if music.dice.below(3) < 2 {
                    FANFARES[0]
                } else {
                    FANFARES[1]
                };
                music.once(id, &once, &assets);
            }
        }
    }
}

/// Turns the peace stream at each frame (`0x69B770`'s head): its first phase rolled as the hero
/// first stands in the world, the dusk and the night asking for the ambient loop, a sector
/// without music in the music's phase too, the timer counting by day and dawn and switching past
/// 180 s, held in a spot loop's sector; and the hero's death playing its music once over it,
/// asking for the ambient loop.
pub fn alternate(
    mut music: ResMut<Music>,
    (time, clock): (Res<Time>, Res<Clock>),
    bodies: Query<&Transform, With<hero::Body>>,
    falling: Query<(), (With<hero::Body>, Added<Dies>)>,
    (once, assets, scripts): (
        Res<AudioChannel<Once>>,
        Res<AssetServer>,
        Option<NonSend<crate::scripts::Live>>,
    ),
) {
    let Some(hero) = bodies.iter().next() else {
        return;
    };
    let now = time.elapsed();
    if music.stream.phase().is_none() {
        // `rand() & 1` at the first region's event, by the clock.
        let roll = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos() >> 7);
        let phase = music.stream.begin(roll);
        debug!("music: the peace stream begins with {phase:?}");
    }
    // The hero's death (`0x6852B0`, `0x694FE0(MUSIC_DEATH, 1, 0)`): the death's music once at
    // the music's volume, unfaded, over the stream, which turns to the ambient loop; under the
    // scripts' control, the turn alone.
    if !falling.is_empty() {
        if !music.stream.controlled() {
            debug!("music: the hero's death");
            music.once(DEATH, &once, &assets);
        }
        music.stream.now();
    }
    let bits = |name: &str| {
        let value = scripts.as_ref().map_or(0, |scripts| scripts.variable(name));
        u32::from_le_bytes(value.to_le_bytes())
    };
    let sector = sector_of(hero.translation.x, hero.translation.z);
    let id = music
        .sectors
        .get(&sector)
        .map(|&(id, area)| music_of(id, area, bits(SIEGES), bits(CHANGES)));
    // A sector of music come into gives the stream what its phase holds, unless the scripts hold
    // it (the region's event, `0x6824AF`).
    if music.sector.is_some_and(|before| before != sector)
        && id.is_some()
        && !music.stream.controlled()
    {
        music.scripted = None;
    }
    music.sector = Some(sector);
    music.spot = id.is_some_and(|id| SPOTS.contains(&id));
    let has_music = !music.spot && id.is_some_and(|id| music.tracks.contains_key(&id));
    // The dusk's and the night's events ask for the ambient loop (`0x691310`), and a music that
    // cannot open does in its phase.
    let dark = matches!(
        clock.phase(),
        daynight::Phase::Dusk | daynight::Phase::Night
    );
    if (dark && !music.dark) || (!has_music && !music.spot) {
        music.stream.now();
    }
    music.dark = dark;
    let crossfading = now < music.fading;
    let seconds = f64::from(time.delta_secs());
    let holds = music.spot;
    if let Some(phase) = music
        .stream
        .frame(seconds, (!dark, holds), (crossfading, has_music))
    {
        debug!("music: the peace stream turns to {phase:?}");
        music.fading = now + FADE;
        music.scripted = None;
    }
}

/// Plays on the music's channel what the stream asks of it ([`wanted`]): a track that changes
/// crossfades over 7 s, the one before falling as the new one rises, both linear; the tracks
/// loop. While the scripts hold the stream, only their music and the spot loops change it.
pub fn play(
    mut music: ResMut<Music>,
    audio: Res<AudioChannel<Channel>>,
    mut instances: ResMut<Assets<AudioInstance>>,
    assets: Res<AssetServer>,
    time: Res<Time>,
    bodies: Query<&Transform, With<hero::Body>>,
    scripts: Option<NonSend<crate::scripts::Live>>,
) {
    let Some(hero) = bodies.iter().next() else {
        return;
    };
    let bits = |name: &str| {
        let value = scripts.as_ref().map_or(0, |scripts| scripts.variable(name));
        u32::from_le_bytes(value.to_le_bytes())
    };
    let id = music
        .sectors
        .get(&sector_of(hero.translation.x, hero.translation.z))
        .map(|&(id, area)| music_of(id, area, bits(SIEGES), bits(CHANGES)));
    let want = wanted(&music, id);
    if music.stream.controlled()
        && music.scripted.is_none()
        && !want.is_some_and(|id| SPOTS.contains(&id))
    {
        return;
    }
    let file = want.and_then(|id| music.tracks.get(&id).cloned());
    if music.playing.as_ref().map(|p| &p.file) == file.as_ref() {
        return;
    }
    music.fading = time.elapsed() + FADE;
    let fade = AudioTween::linear(Duration::from_secs_f32(CROSSFADE));
    if let Some(playing) = music.playing.take()
        && let Some(mut instance) = instances.get_mut(&playing.instance)
    {
        instance.stop(fade);
    }
    let Some(file) = file else {
        return;
    };
    debug!("music: {file}");
    let track = assets.load(file.clone());
    // An `ATMO` name, a spot loop or the scripts' loop, at the effects' level as the ambient
    // loops (`0x69C4BB`).
    let volume = if want.is_some_and(|id| music.atmo.contains(&id)) {
        crate::atmosphere::VOLUME
    } else {
        VOLUME
    };
    let instance = audio
        .play(track.clone())
        .looped()
        .with_volume(volume)
        .fade_in(fade)
        .handle();
    music.playing = Some(Playing {
        _track: track,
        instance,
        file,
    });
}

/// Stops the music of the game left, at once.
pub fn leave(audio: Res<AudioChannel<Channel>>, once: Res<AudioChannel<Once>>) {
    audio.stop();
    once.stop();
}

/// The peace stream and the death's music, in a game whose music is on (`game`).
pub struct MusicPlugin;

impl Plugin for MusicPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_channel::<Channel>()
            .add_audio_channel::<Once>()
            .add_systems(
                Update,
                (scripted, alternate, play)
                    .chain()
                    .in_set(frame::Playing)
                    .in_set(frame::Unpaused)
                    .run_if(resource_exists::<Music>),
            )
            .add_systems(OnExit(AppState::InGame), leave);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use opensacred_data::sound::{Music as Track, SoundName};

    /// The music of a sound index of the music `MUSIC_WOOD01`, the cemetery's loop and
    /// `ATMO_DUNGEON`.
    fn music() -> Music {
        let names = [
            (0x1980, "MUSIC_WOOD01"),
            (0x1968, "ATMOSPOT_CEMETERY"),
            (0x1954, "ATMO_DUNGEON"),
        ];
        let sound = Sound {
            samples: Vec::new(),
            profiles: Vec::new(),
            music: names
                .iter()
                .map(|&(id, name)| Track {
                    id: Some(id),
                    name: name.to_owned(),
                })
                .collect(),
            names: names
                .iter()
                .map(|&(id, name)| SoundName {
                    id,
                    name: format!("{PREFIX}{name}"),
                })
                .collect(),
        };
        let world = World {
            sectors: Vec::new(),
            sheets: Vec::new(),
            atlases: Vec::new(),
            assembled_from: Vec::new(),
        };
        Music::new(&world, &sound)
    }

    #[test]
    fn the_music_channel_plays_the_music_in_its_phase_and_a_spot_loop_always() {
        let mut music = music();
        // Before the stream begins, nothing.
        assert_eq!(wanted(&music, Some(0x1980)), None);
        music.stream.begin(0);
        assert_eq!(wanted(&music, Some(0x1980)), Some(0x1980));
        assert_eq!(wanted(&music, None), None);
        // The ambient phase: the music silent, a spot loop playing.
        music.stream.begin(0);
        music.stream.now();
        music.stream.frame(0.0, (true, false), (false, true));
        assert!(music.ambient());
        assert_eq!(wanted(&music, Some(0x1980)), None);
        assert_eq!(wanted(&music, Some(0x1968)), Some(0x1968));
        assert!(music.atmo.contains(&0x1968) && !music.atmo.contains(&0x1980));
    }

    #[test]
    fn the_scripts_music_takes_the_stream_by_its_name_case_aside() {
        let mut music = music();
        assert_eq!(music.id_of("ATMO_Dungeon"), Some(0x1954));
        assert_eq!(music.id_of("sound_fx_MUSIC_WOOD01"), Some(0x1980));
        // A name that runs on after the prefix's eighth character is taken whole.
        assert_eq!(music.id_of("SOUND_FXMUSIC_WOOD01"), None);
        assert_eq!(music.id_of("MusicFightGiantSpider"), None);
        // In the ambient phase the scripts' loop takes the stream from the place's.
        music.stream.begin(1);
        music.scripted = music.id_of("ATMO_DUNGEON");
        assert!(!music.ambient());
        assert_eq!(wanted(&music, Some(0x1980)), Some(0x1954));
    }

    #[test]
    fn the_sieges_and_the_worlds_change_take_the_villages_music() {
        // The start's area 1 is the region 1; the area 8 the region 9, the area 24 `0x6E`.
        assert_eq!(music_of(0x1980, 1, 0, 0), 0x1980);
        assert_eq!(music_of(VILLAGE, 1, 0, 0), VILLAGE);
        assert_eq!(music_of(VILLAGE, 8, 0, 0), SIEGE_MILITARY);
        // The evil's turn: the villages of the region 2 too, but not of the region 1.
        assert_eq!(music_of(MASCARELL, 2, 0, 2), SIEGE_MILITARY);
        assert_eq!(music_of(VILLAGE, 1, 0, 2), VILLAGE);
        // A sieged region: any music, the region `0x6E` by the bit 11.
        assert_eq!(music_of(0x1980, 1, 1 << 1, 0), SIEGE_UNDEAD);
        assert_eq!(music_of(VILLAGE, 24, 1 << 11, 2), SIEGE_UNDEAD);
        assert_eq!(music_of(0x1980, 1, 1 << 2, 0), 0x1980);
    }

    #[test]
    fn a_point_lies_in_the_sector_of_its_cell() {
        let start = iso::ground(50.0 * 64.0 + 36.5, 39.0 * 64.0 + 15.5);
        assert_eq!(sector_of(start.x, start.z), (50, 39));
        let corner = iso::ground(51.0 * 64.0, 40.0 * 64.0);
        assert_eq!(sector_of(corner.x + 0.01, corner.z + 0.01), (51, 40));
    }
}
