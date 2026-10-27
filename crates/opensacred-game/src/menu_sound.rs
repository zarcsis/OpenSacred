//! The menus' sound (docs/re/menus.md, "The sound"; #1128): `SOUND_FX_MENU_ADDON` as their music,
//! looping from their start through all their screens unless the music is off (`--no-music`);
//! each drop's sound as it falls into the water (`ripples::Fallen`), a cave's on the choices of the
//! hero; and the idle sounds, a roll every half a second, the woods' or the dungeons' by the last
//! drop's water, as the original's sound manager plays them in its menus' state (`0x69B770`).
//! A drop's and an idle sound play at the effects' volume, panned and at a rate at random; the
//! choices' figures speak their classes' introductions (`choice`).

use bevy::prelude::*;
use bevy_kira_audio::AudioApp;
use bevy_kira_audio::prelude::{Audio, AudioChannel, AudioControl, AudioSource};

use crate::game::{AppState, Shows};
use crate::music;
use crate::ripples::Fallen;
use crate::sfx::{self, Sfx};
use crate::sound;

/// `SOUND_FX_MENU_ADDON`, the menus' music (`mp3/MENU_ADDON.mp3`).
const MUSIC: u32 = 6700;
/// The drops' sounds, `SOUND_FX_WATERDRIPPLE01` and `02`, and the caves' (`_CAVE01`, `_CAVE02`).
const DRIPS: [u32; 2] = [7015, 7016];
const CAVE_DRIPS: [u32; 2] = [7017, 7018];
/// How often the idle sounds roll, in seconds, and the odds that a roll plays one.
const IDLE_EVERY: f32 = 0.5;
const IDLE_ODDS: u32 = 5;
/// The idle sounds' runs, their first id and how many: `SOUND_FX_IDLEFX_WOOD01` to `20`,
/// `_GLOBAL01` to `10` and `_DUNGEON01` to `16`; the main menu takes the woods' one roll in four,
/// the choices the dungeons' one in three, the rest the global ones.
const WOOD: (u32, u32) = (16160, 20);
const GLOBAL: (u32, u32) = (16120, 10);
const DUNGEON: (u32, u32) = (16220, 16);
const WOOD_ODDS: u32 = 4;
const DUNGEON_ODDS: u32 = 3;
/// The drops' pans, 0.1 and up to 0.8 more of 0 to 1, the idle sounds' 0.3 and up to 0.4 more,
/// and both's rate factor, 0.3 and up to 0.4 more, which the client plays at the sample's rate
/// times the factor + 0.5 (`0x693510`).
const DRIP_PAN: (f32, f32) = (0.1, 0.8);
const IDLE_PAN: (f32, f32) = (0.3, 0.4);
const RATE: (f32, f32) = (0.3, 0.4);

/// The channel of the figures' introductions on the choice of the hero (`choice`), which one stop
/// silences whether it plays or still loads.
#[derive(Resource)]
pub struct Introductions;

/// The idle sounds' clock and whether the last drop fell into a cave's water, which chooses
/// them, as the sound manager keeps it from the drop's event (`+0x88`).
#[derive(Resource, Default)]
struct Idle {
    since: f32,
    cave: bool,
    /// The menus' music, held while it plays.
    music: Option<Handle<AudioSource>>,
}

/// A number from 0 to 1 at random, as the client's `rand()` / 32767.
fn share(sfx: &mut Sfx) -> f32 {
    f32::from(u16::try_from(sfx.dice.below(0x8000)).unwrap_or(0)) / 32_767.0
}

/// Plays the sample of the sound `id` at the effects' volume, at the pan of `pan` and the rate of
/// `rate`, each a base and up to a span more of a roll, the pan from 0 left to 1 right.
fn play(
    (sfx, audio, assets): (&mut Sfx, &Audio, &AssetServer),
    id: u32,
    pan: (f32, f32),
    rate: (f32, f32),
) {
    let Some((id, file)) = sfx.with_id(id) else {
        return;
    };
    let pan = pan.0 + pan.1 * share(sfx);
    let rate = rate.0 + rate.1 * share(sfx) + 0.5;
    let sample = sfx.sample(id, file, assets);
    audio
        .play(sample)
        .with_volume(sfx::decibels(1.0))
        .with_panning(pan * 2.0 - 1.0)
        .with_playback_rate(f64::from(rate));
    debug!("menu: sound {id}, panned {pan:.2}, at a rate of {rate:.2}");
}

/// Each drop's sound as it falls, one of its water's two at even odds.
fn drip(
    mut fallen: MessageReader<Fallen>,
    mut idle: ResMut<Idle>,
    mut sfx: ResMut<Sfx>,
    audio: Res<Audio>,
    assets: Res<AssetServer>,
) {
    for drop in fallen.read() {
        idle.cave = drop.cave;
        let drips = if drop.cave { CAVE_DRIPS } else { DRIPS };
        let id = drips[usize::try_from(sfx.dice.below(2)).unwrap_or(0)];
        play((&mut sfx, &audio, &assets), id, DRIP_PAN, RATE);
    }
}

/// Every half a second of the menus, one chance in five of an idle sound: the woods' one in four
/// on the main menu, the dungeons' one in three on the choices, else a global one.
fn murmur(
    time: Res<Time<Real>>,
    mut idle: ResMut<Idle>,
    mut sfx: ResMut<Sfx>,
    audio: Res<Audio>,
    assets: Res<AssetServer>,
) {
    idle.since += time.delta_secs();
    if idle.since <= IDLE_EVERY {
        return;
    }
    idle.since = 0.0;
    if sfx.dice.below(IDLE_ODDS) != 1 {
        return;
    }
    let (own, odds) = if idle.cave {
        (DUNGEON, DUNGEON_ODDS)
    } else {
        (WOOD, WOOD_ODDS)
    };
    let (first, count) = if sfx.dice.below(odds) == 0 {
        own
    } else {
        GLOBAL
    };
    let id = first + sfx.dice.below(count);
    play((&mut sfx, &audio, &assets), id, IDLE_PAN, RATE);
}

/// The menus' music from their start, looping, unless the music is off; the menus' start closes
/// whatever music played (the sound event (12, 2, 0), `0x67A7A0`).
fn start(
    mut idle: ResMut<Idle>,
    channel: Res<AudioChannel<music::Channel>>,
    index: Option<Res<sound::Index>>,
    shows: Option<Res<Shows>>,
    assets: Res<AssetServer>,
) {
    channel.stop();
    if shows.is_some_and(|shows| shows.no_music()) {
        return;
    }
    let Some(file) = index.as_deref().and_then(|index| {
        index
            .0
            .music
            .iter()
            .find(|music| music.id == Some(MUSIC))
            .map(sound::music_file)
    }) else {
        return;
    };
    debug!("menu: music {file}");
    let track = assets.load(file);
    channel
        .play(track.clone())
        .looped()
        .with_volume(music::VOLUME);
    idle.music = Some(track);
}

/// Stops the menus' music as a game starts, whose own music follows.
fn stop(mut idle: ResMut<Idle>, channel: Res<AudioChannel<music::Channel>>) {
    channel.stop();
    idle.music = None;
}

/// The menus' music, the drops' and the idle sounds.
pub struct MenuSoundPlugin;

impl Plugin for MenuSoundPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Idle>()
            .add_audio_channel::<Introductions>()
            .add_systems(OnEnter(AppState::Menu), start)
            .add_systems(OnExit(AppState::Menu), stop)
            .add_systems(
                Update,
                (drip, murmur)
                    .run_if(in_state(AppState::Menu))
                    .run_if(resource_exists::<Sfx>),
            );
    }
}
