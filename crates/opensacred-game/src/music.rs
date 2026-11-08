//! Music: the peace stream of the region's music and the ambient loop in turns, as the client
//! plays them (docs/re/sound.md, "The region's music" and "The peace stream's alternation";
//! #1177). Each sector names its music by sound id (`sectors.keyx +0x2D8`: `MUSIC_WOOD01` at the
//! start, `MUSIC_DESERT01`, `MUSIC_KARIBIK`, …); by day the music and the ambient loop of the
//! place's nature (`atmosphere`) take the stream for 180 s each in turn
//! (`opensacred_rules::music::Stream`), the first by a roll, crossfading over 7 s, the tracks
//! looping. The dusk and the night ask for the ambient loop and hold the timer, so that the music
//! comes back 180 s into the dawn or the day; a sector without music keeps the region's music of
//! the last sector of one, as the original posts it no region's event (`0x626915`), and the
//! stream goes on with it; entering a sector of another music in the music's phase crossfades to
//! it. The spot loops of the cemetery and the arenas (`ATMOSPOT_*`) take the stream while the
//! hero stands in their sectors, the cycle held (**hypothesis** for what plays there: the
//! original plays the arenas' on a stream of their own and asks for the ambient loop at the
//! cemetery). Only the tracks playing are loaded: `bevy_kira_audio` decodes a whole file into
//! memory, some 85 MB for a track of four minutes, as it does not stream.
//!
//! The campaign's sieges and its world's change override a sector's music (`0x67A7A0`, #55,
//! [`music_of`]): the scripts' variables `RegionChange`, a bit for each sieged region, and
//! `Globalchange1`, whose bit 1 the evil's turn of the quest 45 sets, give the villages' and
//! Mascarell's music the military siege's, and any music of a sieged region the undead siege's.
//!
//! The fights' music and jingles (#1948, [`fight`]): the foes about the hero weighed once a
//! second play a jingle once over the stream, or cut in a fight's music on the music's
//! channel, which the fight's end gives back to the stream (`opensacred_rules::music::Fight`).
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
use std::time::Duration;

use bevy::log::debug;
use bevy::prelude::{
    Added, App, AssetServer, Assets, GlobalTransform, Handle, IntoScheduleConfigs, MessageReader,
    NonSend, OnExit, Plugin, Query, Res, ResMut, Resource, Time, Transform, Update, With, Without,
    resource_exists,
};
use bevy_kira_audio::prelude::{
    AudioChannel, AudioControl, AudioInstance, AudioSource, AudioTween,
};
use opensacred_data::sound::Sound;
use opensacred_data::world::{SIDE, World};
use opensacred_rules::music::{CROSSFADE, Cue, Fight, Foe, Phase, Scene, Stream};
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
/// The channel of the music, apart from the effects.
#[derive(Resource)]
pub struct Channel;

/// The channel of a music played once over the stream, the death's (the once-stream `+0x60C`).
#[derive(Resource)]
pub struct Once;

#[derive(Resource)]
pub struct Music {
    /// Each sector's music, area and nature, by the sector, and the track of each music.
    sectors: HashMap<(u32, u32), (u32, u8, u8)>,
    tracks: HashMap<u32, String>,
    /// Each sound's id by its name, lower-cased, the ambient loops' ids (`ATMO…`), which play at
    /// the effects' volume, and the streams' factors other than 1 by their ids (`+0x6B4`).
    ids: HashMap<String, u32>,
    atmo: HashSet<u32>,
    factors: HashMap<u32, f32>,
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
    /// The rolls of the fanfares and of the peace stream's first phase.
    dice: Rng,
    /// The track on the music's channel.
    playing: Option<Playing>,
    /// The death's music, held while it plays.
    once: Option<Handle<AudioSource>>,
    /// The music of the hero's sector, its overrides applied (`+0x692`), and its nature
    /// (`+0xA36`), the last sector's of music in a sector without.
    region: Option<u32>,
    nature: u8,
    /// The fights' weighing, the fight's music that takes the channel, and whether it cuts in
    /// at once.
    fight: Fight,
    fighting: Option<u32>,
    cut: bool,
    /// The groups of the jingles' profiles, `music` (1) and `jingle_extension` (2).
    jingles: HashMap<u32, Vec<Vec<u32>>>,
}

struct Playing {
    /// Its sound's id.
    id: u32,
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
            .map(|s| ((s.x, s.y), (s.music, s.area, s.nature)))
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
        let factors = sound
            .names
            .iter()
            .map(|name| (name.id, opensacred_rules::music::factor(&name.name)))
            .filter(|&(_, factor)| factor != 1.0)
            .collect();
        Self {
            sectors,
            tracks,
            ids,
            atmo,
            factors,
            stream: Stream::default(),
            fading: Duration::ZERO,
            dark: false,
            spot: false,
            sector: None,
            scripted: None,
            dice: crate::seed::dice(0x1243),
            playing: None,
            once: None,
            region: None,
            nature: 0,
            fight: Fight::default(),
            fighting: None,
            cut: false,
            jingles: sound
                .profiles
                .iter()
                .filter(|profile| matches!(profile.slot, 1 | 2))
                .map(|profile| (profile.slot, profile.groups.clone()))
                .collect(),
        }
    }

    /// The factor of the stream `id`'s volume (`+0x6B4`, `opensacred_rules::music::factor`).
    pub fn factor(&self, id: u32) -> f32 {
        self.factors.get(&id).copied().unwrap_or(1.0)
    }

    /// Whether the music's channel plays an ambient or a spot loop (`ATMO…`), at the effects'
    /// volume (`volumes`).
    pub fn plays_atmo(&self) -> bool {
        self.playing
            .as_ref()
            .is_some_and(|playing| self.atmo.contains(&playing.id))
    }

    /// Whether the ambient loop of the place takes the stream now (`atmosphere`).
    pub fn ambient(&self) -> bool {
        !self.spot
            && self.scripted.is_none()
            && self.fighting.is_none()
            && self.stream.phase() == Some(Phase::Ambient)
    }

    /// A jingle of the profile `profile`'s group `group` once on the once-stream: a slot of 8
    /// at random, the group's variants repeating over them (`0x4261D0`).
    fn jingle(
        &mut self,
        (profile, group): (u32, u8),
        once: &AudioChannel<Once>,
        assets: &AssetServer,
    ) {
        let Some(variants) = self
            .jingles
            .get(&profile)
            .and_then(|groups| groups.get(usize::from(group)))
            .filter(|variants| !variants.is_empty())
        else {
            return;
        };
        let slot = usize::try_from(self.dice.below(8)).unwrap_or(0);
        let id = variants[slot % variants.len()];
        self.once(id, once, assets);
    }

    /// The hero in `sector`, whose music, its overrides applied, and nature are `here`, none for a
    /// sector without music. A sector of music come into gives the stream what its phase holds,
    /// unless the scripts hold it (the region's event, `0x6824AF`); a sector without music posts
    /// no region's event (`0x626915`), so that the region's music and the nature stay the last
    /// sector's of one.
    fn stand(&mut self, sector: (u32, u32), here: Option<(u32, u8)>) {
        if self.sector.is_some_and(|before| before != sector)
            && here.is_some()
            && !self.stream.controlled()
        {
            self.scripted = None;
        }
        self.sector = Some(sector);
        if let Some((music, nature)) = here {
            self.region = Some(music);
            self.nature = nature;
        }
        self.spot = self.region.is_some_and(|id| SPOTS.contains(&id));
    }

    /// The ambient loop that the place calls for, the dusk and the night's by `night`
    /// (`opensacred_rules::music::ambient_loop`): its id and its track, none where the original
    /// plays none, or before the first sector of music.
    pub fn place_loop(&self, night: bool) -> Option<(u32, &str)> {
        let id = opensacred_rules::music::ambient_loop(self.region?, self.nature, night)?;
        Some((id, self.tracks.get(&id)?.as_str()))
    }

    /// Whether the sound `id` is an ambient loop by its name (`ATMO…`), played at the effects'
    /// level rather than the music's (`0x69C4BB`).
    pub fn atmo(&self, id: u32) -> bool {
        self.atmo.contains(&id)
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
        let arena = self.region == Some(ARENA_INDOOR);
        let Some(file) = self.tracks.get(&id).filter(|_| !arena).cloned() else {
            return;
        };
        debug!("music: {file} once");
        let track = assets.load(file);
        once.stop();
        once.play(track.clone())
            .with_volume(crate::sfx::decibels(self.factor(id)));
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
    if music.fighting.is_some() {
        return music.fighting;
    }
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
/// first stands in the world, the dusk and the night asking for the ambient loop, and so in the
/// music's phase the region's music that cannot open (none before the first sector of music), the
/// timer counting by day and dawn and switching past 180 s, held in a spot loop's sector; and the
/// hero's death playing its music once over it, asking for the ambient loop.
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
        // `rand() & 1` at the first region's event, of the run's rolls.
        let roll = music.dice.below(2);
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
    let here = music
        .sectors
        .get(&sector)
        .map(|&(id, area, nature)| (music_of(id, area, bits(SIEGES), bits(CHANGES)), nature));
    music.stand(sector, here);
    let has_music = !music.spot
        && music
            .region
            .is_some_and(|id| music.tracks.contains_key(&id));
    // The dusk's and the night's events ask for the ambient loop (`0x691310`), and a music that
    // cannot open does in its phase; where the place has no loop, the music plays on.
    let dark = matches!(
        clock.phase(),
        daynight::Phase::Dusk | daynight::Phase::Night
    );
    let has_loop = music.place_loop(dark).is_some();
    if (dark && !music.dark) || (!has_music && !music.spot) {
        music.stream.now();
    }
    music.dark = dark;
    let crossfading = now < music.fading;
    let seconds = f64::from(time.delta_secs());
    // A fight's music holds the cycle, its timer counting on (`+0xAF28`).
    let holds = music.spot || music.fighting.is_some();
    if let Some(phase) =
        music
            .stream
            .frame(seconds, (!dark, holds), (crossfading, has_music, has_loop))
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
    bodies: Query<(), With<hero::Body>>,
) {
    if bodies.is_empty() {
        return;
    }
    let want = wanted(&music, music.region);
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
    // A fight's music cuts in at once, the track before stopped (`playMusic(id, 1, 0)`).
    let cut = std::mem::take(&mut music.cut) && music.fighting.is_some();
    music.fading = time.elapsed() + FADE;
    let fade = if cut {
        AudioTween::default()
    } else {
        AudioTween::linear(Duration::from_secs_f32(CROSSFADE))
    };
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
    // At its factor; an `ATMO` name, a spot loop or the scripts' loop, at the effects' volume as
    // the ambient loops, the others at the music's (`volumes`, `0x694510`).
    let id = want.unwrap_or_default();
    let mut playing = audio.play(track.clone());
    playing
        .looped()
        .with_volume(crate::sfx::decibels(music.factor(id)));
    if !cut {
        playing.fade_in(fade);
    }
    let instance = playing.handle();
    music.playing = Some(Playing {
        id,
        _track: track,
        instance,
        file,
    });
}

/// The foes that the fights' weighing hears, within the sound manager's first list's reach in
/// the place's pixels (`+0xB790`).
const HEARD_WITHIN: i32 = 1200;
/// The group of the fight's end's jingle, `JINGLE_FIGHT_OVER` (`0x697710`).
const FIGHT_OVER: u8 = 6;

/// Weighs the fight about the hero at each frame (`0x68BEA0`; docs/re/sound.md, "The fights'
/// music and jingles"; #1948) and carries out what it asks: a jingle once over the stream, a
/// fight's music cut in on the music's channel (`MUSIC_WRONGLEVEL` crossfaded), the stream's
/// timer, and the fight's end (`0x697710`), which gives the channel back to the stream, sets a
/// timer past 180 s to 120 s, and, unless the music's phase takes it back by day, plays the
/// end's jingle half of the time (**hypothesis**: the profile 0 that the other half rolls names
/// none). The hostile creatures alive within the first list's reach are the foes, their levels
/// and motions as `voices` keeps them, the foes engaged its count.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their \
              filters"
)]
pub fn fight(
    mut music: ResMut<Music>,
    (time, clock, difficulty, engaged): (
        Res<Time>,
        Res<Clock>,
        Option<Res<crate::game::Difficulty>>,
        Option<Res<crate::voices::Engaged>>,
    ),
    heroes: Query<(&GlobalTransform, &crate::fight::Health), With<hero::Body>>,
    creatures: Query<
        (
            &GlobalTransform,
            &crate::ai::Mind,
            &crate::fight::Health,
            Option<&crate::voices::Motions>,
        ),
        (Without<hero::Body>, Without<Dies>),
    >,
    (once, assets): (Res<AudioChannel<Once>>, Res<AssetServer>),
) {
    let Ok((at, health)) = heroes.single() else {
        return;
    };
    let Some(phase) = music.stream.phase() else {
        return;
    };
    let hero_at = at.translation();
    let foes: Vec<Foe> = creatures
        .iter()
        .filter(|(_, mind, of, _)| mind.hostile() && of.current > 0)
        .map(|(place, _, of, motions)| {
            let [stands, walks, runs, strikes] =
                motions.map_or([0; 4], crate::voices::Motions::shares);
            Foe {
                distance: crate::sfx::screen_distance(place.translation(), hero_at),
                level: of.level,
                stands,
                walks,
                runs,
                strikes,
            }
        })
        .filter(|foe| foe.distance < HEARD_WITHIN)
        .collect();
    let engaged = engaged.map_or(0, |engaged| engaged.0);
    let calm = engaged == 0
        && crate::remarks::calm(
            hero_at,
            creatures.iter().map(|(place, mind, ..)| (place, mind)),
        );
    let scene = Scene {
        foes: &foes,
        hero: health.level,
        engaged,
        phase,
        once: once.is_playing_sound(),
        siege: music.region == Some(SIEGE_MILITARY),
        difficulty: difficulty.map_or(0, |difficulty| difficulty.0.index()),
        calm,
    };
    // A fight's music is refused in the indoor arena and under the scripts' control
    // (`0x695E70`).
    let refused = music.region == Some(ARENA_INDOOR) || music.stream.controlled();
    let seconds = f64::from(time.delta_secs());
    let mut dice = music.dice.clone();
    let cues = music.fight.update(seconds, &scene, refused, &mut dice);
    music.dice = dice;
    for cue in cues {
        match cue {
            Cue::Jingle(jingle, profile) => {
                debug!("music: the fight's jingle {jingle:?} of the profile {profile}");
                music.jingle((u32::from(profile), jingle.group()), &once, &assets);
            }
            Cue::Fight(kind) => {
                let region = music.region.unwrap_or(0);
                let (roll6, roll9) = (music.dice.below(6), music.dice.below(9));
                let track = opensacred_rules::music::fight_track(kind, region, roll6, roll9);
                debug!("music: a fight's music of the kind {kind}, {track}");
                music.fighting = Some(track);
                music.cut = true;
            }
            Cue::WrongLevel => {
                debug!("music: the foes stand far above the hero");
                music.fighting = Some(opensacred_rules::music::WRONG_LEVEL_MUSIC);
                music.cut = false;
            }
            Cue::Now => music.stream.now(),
            Cue::Back => music.stream.back(),
            Cue::End => {
                debug!("music: the fight's music ends");
                music.fighting = None;
                music.stream.after_fight();
                let dark = matches!(
                    clock.phase(),
                    daynight::Phase::Dusk | daynight::Phase::Night
                );
                let back = music.stream.phase() == Some(Phase::Music) && !dark;
                if !back && music.dice.below(2) == 1 {
                    music.jingle((1, FIGHT_OVER), &once, &assets);
                }
            }
        }
    }
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
                (scripted, alternate, fight, play)
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
        music.stream.frame(0.0, (true, false), (false, true, true));
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
    fn a_sector_without_music_keeps_the_last_regions_music() {
        let mut music = music();
        music.stream.begin(0);
        // Before any sector of music, none: the ambient loop's.
        music.stand((53, 45), None);
        assert_eq!(music.region, None);
        music.stand((50, 39), Some((0x1980, 0x40)));
        assert_eq!(wanted(&music, music.region), Some(0x1980));
        // Into 53,45, of no music: the region's music, its nature and the scripts' music stay.
        music.scripted = Some(0x1954);
        music.stand((53, 45), None);
        assert_eq!(
            (music.region, music.nature, music.scripted, music.spot),
            (Some(0x1980), 0x40, Some(0x1954), false)
        );
        // The cemetery's spot loop holds there too.
        music.stand((50, 40), Some((0x1968, 0x30)));
        music.stand((53, 45), None);
        assert!(music.spot);
        assert_eq!(wanted(&music, music.region), Some(0x1968));
    }

    #[test]
    fn a_point_lies_in_the_sector_of_its_cell() {
        let start = iso::ground(50.0 * 64.0 + 36.5, 39.0 * 64.0 + 15.5);
        assert_eq!(sector_of(start.x, start.z), (50, 39));
        let corner = iso::ground(51.0 * 64.0, 40.0 * 64.0);
        assert_eq!(sector_of(corner.x + 0.01, corner.z + 0.01), (51, 40));
    }
}
