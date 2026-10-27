//! The world's ambience of the sound manager (#1169; docs/re/sound.md, "Ambience"): the idle
//! effects and the water's loops, which the original plays in its 2D sound (`SOUNDQUALITY` 1).
//!
//! Every 0.5 s (`0x69B770`, `+0xB7D0`) a roll of `rand() % 10`: on 1 one of the ten
//! `IDLEFX_GLOBAL`, on 2 and a `rand() % 3` of 0 one of the region's by its music and the phase
//! of the day; each unless its id waits, in the middle at a pan, a rate and a volume of their own
//! rolls, the id then waiting 5 s (60 s for `IDLEFX_TOWN03`, 120 s for `IDLEFX_WOOD10`).
//!
//! The water (`0x6906C0`): the count `c` of the liquid cells on the screen, any material, while
//! one of them is of a clear water, makes `d = c + 2c·((v + 0.5)³ − 1)` at the listener's volume
//! `v`, by which three loops play, `WATER_TINY` from 30, `WATER_NORMAL` from 300 and `WATER_BIG`
//! from 700, each at a volume of the count alone and stopped below its threshold. Their pan is
//! the true middle of those clear cells' (docs/decisions/2026-10-27-water-sound-place.md).

use std::collections::HashMap;

use bevy::prelude::*;
use bevy_kira_audio::prelude::{AudioApp, AudioChannel, AudioControl};
use opensacred_rules::spawning::Rng;

use crate::daynight::{Clock, Phase};
use crate::frame::{Playing, Unpaused};
use crate::game::AppState;
use crate::game::Data;
use crate::hero::Body;
use crate::music;
use crate::sfx::{self, Sfx};
use crate::terrain::Heights;
use crate::{iso, liquids, paths};

/// How often the idle effects roll, in seconds (`0x890C58`).
const TICK: f32 = 0.5;
/// The global idle effects (`IDLEFX_GLOBAL01`–`10`).
const GLOBAL: u32 = 16_120;
/// How long a played idle effect's id waits, and the two that wait longer.
const WAIT: f64 = 5.0;
const LONGER: [(u32, f64); 2] = [(16_202, 60.0), (16_169, 120.0)];
/// The seed of the ambience's `rand()` (**estimate**: any).
const SEED: usize = 0x78;

/// The water's loops, their thresholds of `d` and the volume each takes (`0x6906C0`).
const WATERS: [(u32, f32); 3] = [(216, 30.0), (217, 300.0), (218, 700.0)];
/// The original's screen at zoom 1, in its pixels, whose tile rows it counts the liquids of.
const SCREEN: Vec2 = Vec2::new(1_024.0, 768.0);

/// The channels of the water's three loops.
#[derive(Resource)]
struct Tiny;
#[derive(Resource)]
struct Normal;
#[derive(Resource)]
struct Big;

/// The ambience's clock, the sectors' music, the region's music last heard, the ids that wait,
/// and the dice.
#[derive(Resource)]
struct Ambience {
    since: f32,
    sectors: HashMap<(u32, u32), u32>,
    region: u32,
    waits: HashMap<u32, f64>,
    dice: Rng,
    /// Which of the water's loops play.
    waters: [bool; 3],
}

impl Ambience {
    /// The ambience of a new game in the world `world`.
    fn new(world: &opensacred_data::world::World) -> Self {
        Self {
            since: 0.0,
            sectors: world
                .sectors
                .iter()
                .filter(|sector| sector.music != 0)
                .map(|sector| ((sector.x, sector.y), sector.music))
                .collect(),
            region: 0,
            waits: HashMap::new(),
            dice: Rng::of(0, 0, SEED),
            waters: [false; 3],
        }
    }

    fn below(&mut self, n: u32) -> u32 {
        self.dice.below(n)
    }

    /// A number from 0 to 1 at random, as `0x5295A0` gives `rand()` / 32767.
    fn share(&mut self) -> f32 {
        f32::from(u16::try_from(self.dice.below(0x8000)).unwrap_or(0)) / 32_767.0
    }
}

/// The region's idle effect for the region's music `music` at the phase `phase`, the rolls taken
/// by `roll` (`rand() % n`), as the switch at `0x6A096C` chooses it; none for another music, and
/// by night and at dawn one in two in the towns.
pub fn regional(music: u32, phase: Phase, mut roll: impl FnMut(u32) -> u32) -> Option<u32> {
    match music {
        6510 => Some(16_100 + roll(6)),
        6520..=6523 | 6706 | 6707 => Some(16_220 + roll(16)),
        6540 => Some(16_180 + roll(16)),
        6563 => Some(16_140 + roll(16)),
        6544 | 6550 | 6551 | 6702 => match phase {
            Phase::Day | Phase::Dusk => Some(16_200 + roll(14)),
            Phase::Night => (roll(2) == 0).then(|| 16_216 + roll(2)),
            Phase::Dawn => (roll(2) == 0).then(|| 16_214 + roll(4)),
        },
        6560..=6562 | 6701 | 6703..=6705 => Some(match phase {
            Phase::Day => 16_160 + roll(20),
            Phase::Dusk => 16_170 + roll(10),
            Phase::Night => 16_173 + roll(7),
            Phase::Dawn => 16_171 + roll(9),
        }),
        _ => None,
    }
}

/// A whole number of cells or pixels as a float.
#[expect(
    clippy::cast_precision_loss,
    reason = "counts and offsets of cells on a screen, far below 2^24"
)]
fn to_f32(n: i32) -> f32 {
    n as f32
}

/// The water's `d` for the count `c` of liquid cells at the listener's volume `v`.
fn spread(count: u32, listener: f32) -> f32 {
    let c = to_f32(i32::try_from(count).unwrap_or(i32::MAX));
    c + 2.0 * c * ((listener + 0.5).powi(3) - 1.0)
}

/// The volume of each of the water's loops at `d`, none below its threshold: `WATER_TINY` 0.4 of
/// `d / 300`, `WATER_NORMAL` 0.4 of `(d − 300) / 400` and `WATER_BIG` `(d − 700) / 500`, each at
/// most whole.
fn water_volumes(d: f32) -> [Option<f32>; 3] {
    let from = |threshold: f32, volume: f32| (d >= threshold).then_some(volume.min(1.0));
    [
        from(WATERS[0].1, 0.4 * (d / 300.0).min(1.0)),
        from(WATERS[1].1, 0.4 * ((d - 300.0) / 400.0).min(1.0)),
        from(WATERS[2].1, (d - 700.0) / 500.0),
    ]
}

/// Plays the idle effects at each tick of 0.5 s.
fn idle(
    time: Res<Time>,
    mut ambience: ResMut<Ambience>,
    mut sfx: ResMut<Sfx>,
    clock: Option<Res<Clock>>,
    heroes: Query<&Transform, With<Body>>,
    assets: Res<AssetServer>,
    audio: Res<bevy_kira_audio::Audio>,
) {
    let ambience = &mut *ambience;
    ambience.since += time.delta_secs();
    if ambience.since < TICK {
        return;
    }
    ambience.since = 0.0;
    let now = time.elapsed_secs_f64();
    ambience.waits.retain(|_, until| *until > now);
    // The region's music, the last sector's of a music of its own (**hypothesis** for `+0x692`).
    if let Some(at) = heroes.iter().next()
        && let Some(&id) = ambience
            .sectors
            .get(&music::sector_of(at.translation.x, at.translation.z))
    {
        if ambience.region != id {
            debug!("ambience: the region's music {id}");
        }
        ambience.region = id;
    }
    let roll = ambience.below(10);
    let mut play = |ambience: &mut Ambience, id: u32, span: f32, wait: f64| {
        let (pan, rate) = (ambience.share(), ambience.share());
        let (pan, rate) = (0.3 + span * pan, 0.3 + span * rate + 0.5);
        let volume = 0.2 + 0.3 * ambience.share();
        let Some((id, file)) = sfx.with_id(id) else {
            return;
        };
        let sample = sfx.sample(id, file, &assets);
        audio
            .play(sample)
            .with_volume(sfx::decibels(volume))
            .with_panning(pan * 2.0 - 1.0)
            .with_playback_rate(f64::from(rate));
        ambience.waits.insert(id, now + wait);
        debug!("ambience: {id} at {volume:.2}, panned {pan:.2}, at a rate of {rate:.2}");
    };
    if roll == 1 {
        // A waiting id plays nothing (`0x679170`).
        let id = GLOBAL + ambience.below(10);
        if !ambience.waits.contains_key(&id) {
            play(ambience, id, 0.4, WAIT);
        }
    }
    let third = ambience.below(3);
    if roll == 2 && third == 0 {
        let phase = clock.as_ref().map_or(Phase::Day, |clock| clock.phase());
        let region = ambience.region;
        let id = regional(region, phase, |n| ambience.below(n));
        if let Some(id) = id
            && !ambience.waits.contains_key(&id)
        {
            let wait = LONGER
                .iter()
                .find(|&&(longer, _)| longer == id)
                .map_or(WAIT, |&(_, wait)| wait);
            play(ambience, id, 0.6, wait);
        }
    }
}

/// The liquid cells on the screen about the camera's point, the hero's, at `zoom`, and the middle
/// of the clear waters' among them.
fn waters_seen(heights: &Heights, at: Vec3, zoom: f32) -> (u32, Option<Vec3>) {
    let half = SCREEN * zoom / 2.0;
    let (hx, hy) = paths::cell_of(at.x, at.z);
    // A cell's offset on the screen is 48 (dx − dy) right and 24 (dx + dy) down: the screen's
    // cells lie within as many cells of the hero's along each axis as its half width in cells
    // and its half height in rows.
    let mut reach = 1;
    while to_f32(reach) * 24.0 < half.x / 2.0 + half.y {
        reach += 1;
    }
    let (mut count, mut clear, mut sum) = (0, 0, Vec3::ZERO);
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            let (sx, sy) = (to_f32(48 * (dx - dy)), to_f32(24 * (dx + dy)));
            if sx.abs() > half.x + 48.0 || sy.abs() > half.y + 24.0 {
                continue;
            }
            let cell = (hx + dx, hy + dy);
            let Some(material) = heights.liquid_material(cell) else {
                continue;
            };
            count += 1;
            if liquids::clear(material) {
                clear += 1;
                sum += iso::ground(to_f32(cell.0) + 0.5, to_f32(cell.1) + 0.5);
            }
        }
    }
    (count, (clear > 0).then(|| sum / to_f32(clear)))
}

/// The channels of the water's three loops.
type WaterChannels<'w> = (
    Res<'w, AudioChannel<Tiny>>,
    Res<'w, AudioChannel<Normal>>,
    Res<'w, AudioChannel<Big>>,
);

/// Plays the water's loops by the liquid cells on the screen.
fn water(
    mut ambience: ResMut<Ambience>,
    mut sfx: ResMut<Sfx>,
    heights: Res<Heights>,
    heroes: Query<&Transform, With<Body>>,
    (projections, heard): (
        Query<&Projection, crate::iso::Eye<With<Camera>>>,
        Res<sfx::Listener>,
    ),
    channels: WaterChannels,
    assets: Res<AssetServer>,
) {
    let Some(at) = heroes.iter().next().map(|t| t.translation) else {
        return;
    };
    let zoom = projections
        .iter()
        .find_map(|p| match p {
            Projection::Orthographic(ortho) => Some(ortho.scale),
            _ => None,
        })
        .unwrap_or(1.0);
    let (count, middle) = waters_seen(&heights, at, zoom);
    // Without a clear water on the screen the manager hears a count of 0 (`0x62E093`).
    let count = if middle.is_some() { count } else { 0 };
    let volumes = water_volumes(spread(count, heard.0));
    let pan = middle.map_or(0.0, |middle| sfx::placed_at(middle, Some(at), *heard).1);
    let (tiny, normal, big) = channels;
    let channels: [&dyn WaterChannel; 3] = [&*tiny, &*normal, &*big];
    let ambience = &mut *ambience;
    for (k, ((id, _), volume)) in WATERS.iter().zip(volumes).enumerate() {
        let channel = channels[k];
        match volume {
            Some(volume) => {
                if !ambience.waters[k] {
                    let Some((id, file)) = sfx.with_id(*id) else {
                        continue;
                    };
                    let rate = 0.4 + 0.2 * ambience.share() + 0.5;
                    channel.start(sfx.sample(id, file, &assets), rate);
                    ambience.waters[k] = true;
                    debug!(
                        "ambience: the water's loop {id} starts, d {:.0}",
                        spread(count, heard.0)
                    );
                }
                channel.set(volume, pan);
            }
            None => {
                if ambience.waters[k] {
                    channel.silence();
                    ambience.waters[k] = false;
                }
            }
        }
    }
}

/// One of the water's loops' channels.
trait WaterChannel {
    fn start(&self, sample: Handle<bevy_kira_audio::AudioSource>, rate: f32);
    fn set(&self, volume: f32, pan: f32);
    fn silence(&self);
}

impl<T: Resource> WaterChannel for AudioChannel<T> {
    fn start(&self, sample: Handle<bevy_kira_audio::AudioSource>, rate: f32) {
        self.play(sample)
            .looped()
            .with_volume(sfx::decibels(0.0))
            .with_playback_rate(f64::from(rate));
    }

    fn set(&self, volume: f32, pan: f32) {
        self.set_volume(sfx::decibels(volume));
        self.set_panning(pan);
    }

    fn silence(&self) {
        AudioControl::stop(self);
    }
}

/// A new game's ambience.
fn begin(mut commands: Commands, data: Option<Res<Data>>) {
    if let Some(data) = data {
        commands.insert_resource(Ambience::new(&data.0.world));
    }
}

/// The water's loops stop as the game is left.
fn leave(
    tiny: Res<AudioChannel<Tiny>>,
    normal: Res<AudioChannel<Normal>>,
    big: Res<AudioChannel<Big>>,
) {
    tiny.silence();
    normal.silence();
    big.silence();
}

/// The idle effects and the water's loops.
pub struct AmbiencePlugin;

impl Plugin for AmbiencePlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_channel::<Tiny>()
            .add_audio_channel::<Normal>()
            .add_audio_channel::<Big>()
            .add_systems(OnEnter(AppState::InGame), begin)
            .add_systems(OnExit(AppState::InGame), leave)
            .add_systems(
                Update,
                (idle, water)
                    .run_if(resource_exists::<Ambience>)
                    .in_set(Playing)
                    .in_set(Unpaused),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_regions_idle_effects_follow_their_music_and_the_phase() {
        let first = |_| 0;
        assert_eq!(regional(6510, Phase::Day, first), Some(16_100));
        assert_eq!(regional(6707, Phase::Night, first), Some(16_220));
        assert_eq!(regional(6550, Phase::Dusk, first), Some(16_200));
        assert_eq!(regional(6550, Phase::Night, first), Some(16_216));
        assert_eq!(regional(6550, Phase::Night, |_| 1), None);
        assert_eq!(regional(6560, Phase::Dawn, first), Some(16_171));
        assert_eq!(regional(6599, Phase::Day, first), None);
    }

    #[test]
    fn the_waters_loops_take_the_count_alone() {
        let d = spread(100, sfx::Listener::default().0);
        assert!((d - 290.625).abs() < 1e-3);
        let [tiny, normal, big] = water_volumes(d);
        assert!(tiny.is_some_and(|v| (v - 0.4 * 290.625 / 300.0).abs() < 1e-5));
        assert_eq!((normal, big), (None, None));
        assert_eq!(water_volumes(20.0), [None, None, None]);
        let [tiny, normal, big] = water_volumes(1_500.0);
        assert_eq!((tiny, normal, big), (Some(0.4), Some(0.4), Some(1.0)));
    }
}
