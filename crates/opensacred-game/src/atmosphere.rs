//! The atmosphere: the ambient loop of the peace stream's ambient phase (`music`, #1177), as the
//! sound manager chooses it (`0x696A90`, `opensacred_rules::music::ambient_loop`;
//! docs/re/sound.md, "The atmosphere by the region"; #1093): by the region's music and the nature
//! of the hero's sector (`sectors.keyx +0x1D8`), the last sector's of music in a sector without,
//! the loops of the night in the dusk and by night (the phase of the day at `+0xA34`,
//! docs/re/daynight.md). Where the choice gives none, the loop that plays stays. It plays on a
//! channel of its own, rising as the music falls and falling as it rises, over the stream's
//! crossfade of 7 s; in its phase the loop changes as soon as the place calls for another
//! (**hypothesis**: the original chooses it again at each region's event). A loop left keeps its
//! place in the fights' calm and comes back there, in the music's table of places, as the
//! region's music does (`music::Music::leave`; docs/re/sound.md, "The tracks' places"; #3067).

use std::collections::HashMap;
use std::time::Duration;

use bevy::log::debug;
use bevy::prelude::{
    App, AssetServer, Assets, Handle, IntoScheduleConfigs, OnExit, Plugin, Res, ResMut, Resource,
    Update, resource_exists,
};
use bevy_kira_audio::prelude::{
    AudioChannel, AudioControl, AudioInstance, AudioSource, AudioTween,
};

use crate::daynight::{Clock, Phase};
use crate::frame::{Playing, Unpaused};
use crate::game::AppState;
use bevy_kira_audio::AudioApp;

/// The peace stream's crossfade (`0x69C4BB`).
const FADE: Duration = Duration::from_secs(7);
/// The channel of the ambient loops.
#[derive(Resource)]
pub struct Channel;

#[derive(Resource, Default)]
pub struct Atmosphere {
    /// The loop file playing, relative to the asset directory, and whether it is a music that the
    /// place takes for its loop (the pub's, the undead siege's), at the music's volume.
    playing: Option<String>,
    music: bool,
    /// The loop playing: its sound id, its instance and its source.
    heard: Option<(u32, Handle<AudioInstance>, Handle<AudioSource>)>,
    /// Every loop played so far, kept loaded: `bevy_kira_audio` decodes a whole file, and the
    /// loops are a score.
    loaded: HashMap<String, Handle<AudioSource>>,
}

/// The loop playing left as its fade-out begins, its place kept in calm (`music::Music::leave`).
fn leave_loop(
    atmosphere: &mut Atmosphere,
    music: Option<&mut crate::music::Music>,
    (instances, sources): (&Assets<AudioInstance>, &Assets<AudioSource>),
) {
    if let Some((id, instance, source)) = atmosphere.heard.take()
        && let Some(music) = music
    {
        music.leave(id, (instances.get(&instance), sources.get(&source)));
    }
}

/// Plays the place's loop in the stream's ambient phase, where it was last left in calm, and lets
/// it fall in the other.
pub fn choose(
    mut atmosphere: ResMut<Atmosphere>,
    mut music: Option<ResMut<crate::music::Music>>,
    channel: Res<AudioChannel<Channel>>,
    (assets, instances, sources): (
        Res<AssetServer>,
        Res<Assets<AudioInstance>>,
        Res<Assets<AudioSource>>,
    ),
    clock: Res<Clock>,
) {
    // The music's phase or a spot loop: the loop falls.
    let Some(music) = music.as_deref_mut().filter(|music| music.ambient()) else {
        if atmosphere.playing.take().is_some() {
            leave_loop(
                &mut atmosphere,
                music.as_deref_mut(),
                (&instances, &sources),
            );
            channel.stop().fade_out(AudioTween::linear(FADE));
        }
        return;
    };
    // The scripts hold the stream: no loop changes (`0x694616`).
    if music.holds() {
        return;
    }
    let night = matches!(clock.phase(), Phase::Dusk | Phase::Night);
    let Some((id, file)) = music.place_loop(night) else {
        return;
    };
    if atmosphere.playing.as_deref() == Some(file) {
        return;
    }
    debug!("atmosphere: {file}");
    let track = atmosphere
        .loaded
        .entry(file.to_owned())
        .or_insert_with(|| assets.load(file.to_owned()))
        .clone();
    // At its factor, at the effects' volume; a music that the choice takes for a loop (the
    // pub's, the undead siege's) at the music's (`volumes`).
    let volume = crate::sfx::decibels(music.factor(id));
    let file = file.to_owned();
    atmosphere.music = !music.atmo(id);
    leave_loop(&mut atmosphere, Some(music), (&instances, &sources));
    let place = music.place(id);
    channel.stop().fade_out(AudioTween::linear(FADE));
    let mut playing = channel.play(track.clone());
    playing
        .looped()
        .with_volume(volume)
        .fade_in(AudioTween::linear(FADE));
    if let Some(place) = place {
        debug!("atmosphere: {id} opens at {place:.1} s");
        playing.start_from(place);
    }
    atmosphere.heard = Some((id, playing.handle(), track));
    atmosphere.playing = Some(file);
}

impl Atmosphere {
    /// Whether its channel plays a music that the place takes for its loop, at the music's
    /// volume (`volumes`).
    pub fn plays_music(&self) -> bool {
        self.playing.is_some() && self.music
    }
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
