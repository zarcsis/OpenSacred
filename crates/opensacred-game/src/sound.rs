//! Footsteps: while the hero walks or runs, each footstep of the clip plays a sample of the
//! hero's footstep profile, the variants in turn (docs/02 §10.2), through Kira (ADR-07).
//!
//! The moments of the footsteps are the first two parameters of the clip's motion in
//! `MOTIONS.PAK` (`tables/motions.ron`): fractions of the clip about half a cycle apart for the
//! run and walk clips, such as 0.5 and 0.0 for `SERA_RUN_1H` and 0.63 and 0.06 for
//! `SERA_WALK_1H` (**hypothesis**, docs/02 §4.1). The group of the profile follows the nature
//! of the cell under the hero (docs/02 §5.2), `BRIDGE` on a bridge over a liquid as the client's
//! step gives it (`0x639080`, `terrain::Heights::step_nature`), as the client's switch at
//! `0x68EDDA` maps them (docs/re/sound.md).
//!
//! The sound index, `sound/sound.ron`, is parsed once at the start ([`Index`], #827): the weapons'
//! sounds (`sfx`), the footsteps, the music and the ambient loops take their samples from it.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use bevy::log::debug;
use bevy::prelude::{
    AnimationClip, AnimationPlayer, App, AssetServer, Assets, Handle, IntoScheduleConfigs, OnEnter,
    Plugin, Query, Res, ResMut, Resource, Time, Transform, Update,
};
use bevy_kira_audio::prelude::{Audio, AudioControl, AudioInstance, AudioSource, PlaybackState};
use opensacred_data::sound::{EXTENSION, Music, Sound};

use crate::files;
use crate::frame::Step;
use crate::game::AppState;
use crate::hero::{Body, Clips, Gait};
use crate::terrain::Heights;

/// The groups of a footstep profile, named by their samples (docs/02 §10.2).
const GRASS: usize = 1;
const WATER: usize = 2;
const SNOW: usize = 3;
const SWAMP: usize = 4;
const STONE: usize = 5;
const SAND: usize = 6;
const WOOD: usize = 7;
const GROUPS: usize = 8;

/// The footstep group of a cell's nature, as the client's switch at `0x68EDDA` maps them:
/// `DESERT`, `KIESROAD` and `KIES` sand, `ARCTIC` snow, `SUMPF` swamp, `WATERA` and `WATERB`
/// water, `LAVA`, `BRIDGE`, `DUNGEON`, `STONEROAD` and `IND-STONE` stone, `IND-WOOD` wood, the
/// rest (`WALD`, `GRASS`, none) grass.
fn group(nature: u8) -> usize {
    match nature {
        0x1 | 0xC | 0xF => SAND,
        0x2 => SNOW,
        0x5 => SWAMP,
        0x9 | 0xA => WATER,
        0x6 | 0x7 | 0x8 | 0xB | 0xD => STONE,
        0xE => WOOD,
        _ => GRASS,
    }
}

/// The sound index, `sound/sound.ron`, as read at the start.
#[derive(Resource, Clone)]
pub struct Index(pub Arc<Sound>);

impl Index {
    pub fn read(assets: &Path) -> Result<Self> {
        let path = assets.join("sound").join(opensacred_data::sound::INDEX);
        let text =
            files::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let sound = ron::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        Ok(Self(Arc::new(sound)))
    }
}

/// The file of the sample `id`, relative to the asset directory: `sound/samples/01020.opus`.
pub fn sample_file(id: u32) -> String {
    format!(
        "sound/{}",
        opensacred_data::sound::sample_file(id, EXTENSION)
    )
}

/// The file of `music`, relative to the asset directory: `sound/music/MUSIC_WOOD01.opus`.
pub fn music_file(music: &Music) -> String {
    format!("sound/{}", music.file(EXTENSION))
}

/// The footstep sounds of the hero's walk and run clips.
#[derive(Resource)]
pub struct Footsteps {
    /// The moments of the steps of the walk and of the run clip, as fractions of the clip.
    moments: [Vec<f32>; 2],
    /// The sample files of each group, relative to the asset directory.
    files: [Vec<String>; GROUPS],
    samples: [Vec<Handle<AudioSource>>; GROUPS],
    next: usize,
    /// The clip and its fraction at the previous frame.
    last: Option<(Gait, f32)>,
    /// The last step played and when: a step still queued after `STUCK` means there is no audio
    /// device, and the steps stop rather than pile up in the channel's queue.
    played: Option<(Handle<AudioInstance>, Duration)>,
}

/// How long a step may wait in the queue before the steps stop.
const STUCK: Duration = Duration::from_secs(2);

impl Footsteps {
    /// Reads the moments of the steps of the `walk` and `run` clips, and takes the samples of
    /// `profile` from the sound index `sound`.
    pub fn read(
        tables: &crate::tables::Tables,
        [walk, run]: [&str; 2],
        profile: &str,
        sound: &Sound,
    ) -> Result<Self> {
        let motions = tables.motions()?;
        let moments = [walk, run].map(|clip| {
            let name = format!("{clip}.GRN");
            motions
                .iter()
                .find(|m| m.name.eq_ignore_ascii_case(&name))
                .map(|m| m.floats_46[..2].to_vec())
                .unwrap_or_default()
        });
        let groups = sound
            .profiles
            .iter()
            .find(|p| p.name == profile)
            .map(|p| p.groups.as_slice())
            .unwrap_or_default();
        let files = std::array::from_fn(|g| {
            groups
                .get(g)
                .into_iter()
                .flatten()
                .filter(|id| sound.samples.contains(id))
                .map(|&id| sample_file(id))
                .collect()
        });
        Ok(Self {
            moments,
            files,
            samples: Default::default(),
            next: 0,
            last: None,
            played: None,
        })
    }
}

pub fn load(mut footsteps: ResMut<Footsteps>, assets: Res<AssetServer>) {
    let footsteps = &mut *footsteps;
    for (samples, files) in footsteps.samples.iter_mut().zip(&footsteps.files) {
        *samples = files.iter().map(|file| assets.load(file.clone())).collect();
    }
}

/// Whether the clip's fraction passed `moment` between `last` and `now`, the clip looping.
fn passed(last: f32, now: f32, moment: f32) -> bool {
    if now >= last {
        last < moment && moment <= now
    } else {
        moment > last || moment <= now
    }
}

/// Plays a step each time the walk or the run clip passes one of its footstep moments.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources one by one"
)]
pub fn step(
    mut footsteps: ResMut<Footsteps>,
    bodies: Query<(&Body, &Transform)>,
    heights: Res<Heights>,
    clips: Option<Res<Clips>>,
    players: Query<&AnimationPlayer>,
    animations: Res<Assets<AnimationClip>>,
    audio: Res<Audio>,
    time: Res<Time>,
) {
    let Some((gait, at)) = bodies
        .iter()
        .find_map(|(body, transform)| body.moving().map(|gait| (gait, transform.translation)))
    else {
        footsteps.last = None;
        return;
    };
    let gait = Some(gait);
    let fraction = clips.as_ref().zip(gait).and_then(|(clips, gait)| {
        let player = players.get(clips.player?).ok()?;
        let (clip, node) = clips.of(gait);
        let duration = animations.get(clip)?.duration();
        let active = player.animation(*node)?;
        (duration > 0.0).then(|| (active.seek_time() / duration).rem_euclid(1.0))
    });
    let (Some(gait), Some(now)) = (gait, fraction) else {
        footsteps.last = None;
        return;
    };
    let moments = match gait {
        Gait::Walk => 0,
        Gait::Run => 1,
    };
    // The ground's group, or grass where the profile has nothing for it.
    let wanted = heights.step_nature(at.x, at.z).map_or(GRASS, group);
    let group = if footsteps.samples[wanted].is_empty() {
        GRASS
    } else {
        wanted
    };
    if let Some((last_gait, last)) = footsteps.last
        && last_gait == gait
        && !footsteps.samples[group].is_empty()
    {
        for i in 0..footsteps.moments[moments].len() {
            if passed(last, now, footsteps.moments[moments][i]) {
                let stuck = footsteps.played.as_ref().is_some_and(|(step, at)| {
                    audio.state(step) == PlaybackState::Queued
                        && time.elapsed().saturating_sub(*at) > STUCK
                });
                if stuck {
                    continue;
                }
                let samples = &footsteps.samples[group];
                let sample = samples[footsteps.next % samples.len()].clone();
                debug!("footstep at {now:.3} of the {gait:?} clip, group {group}");
                let step = audio.play(sample).handle();
                footsteps.played = Some((step, time.elapsed()));
                footsteps.next += 1;
            }
        }
    }
    footsteps.last = Some((gait, now));
}

/// The footsteps of the hero's run and walk.
pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), load)
            .add_systems(
                Update,
                step.in_set(Step::Footsteps).in_set(crate::frame::Unpaused),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_moment_passes_once_a_loop() {
        assert!(passed(0.4, 0.6, 0.5));
        assert!(!passed(0.5, 0.6, 0.5));
        // The clip wraps around: 0.9 → 0.1 passes 0.0 and 0.95.
        assert!(passed(0.9, 0.1, 0.000_001));
        assert!(passed(0.9, 0.1, 0.95));
        assert!(!passed(0.9, 0.1, 0.5));
    }
}
