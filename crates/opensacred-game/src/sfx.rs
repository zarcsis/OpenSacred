//! The fight's sounds but the voices (#712; docs/re/sound.md, "The fight's sounds"): the profiles
//! of `sound/sound.ron`, a variant of a group picked as the original's `0x4261D0` picks one, the
//! samples, and a sound's volume and pan by its place; and the weapons' sounds: a strike's swing
//! (`WOOSH`) by the class of what the striker holds, at its hit moment within reach or in the
//! air; at the target, the striker's weapon's `HIT` when the hit did damage, else its `PARADE`,
//! by the item's sound profile (`items.pak +0x24`), the fist's for bare hands; and a shot's
//! `SHOOT` by the bow's profile.
//!
//! The volume falls and the pan follows the place from the hero as the original's `0x678260`
//! gives them (docs/re/sound.md, "A sound's place", #731).

use std::collections::HashMap;

use bevy::log::debug;
use bevy::prelude::{
    App, AssetServer, Entity, GlobalTransform, Handle, IntoScheduleConfigs, Message, MessageReader,
    Plugin, Query, Res, ResMut, Resource, Update, Vec3, With,
};
use bevy_kira_audio::prelude::{Audio, AudioControl, AudioSource, Decibels};
use opensacred_data::sound::Sound;

use crate::frame::Unpaused;
use crate::hero::{self, Body};
use crate::iso;
use crate::sound::sample_file;
use crate::spawning::Rng;

/// The slots of a profile's group, over which its variants repeat in turn (docs/02 §10.1).
const SLOTS: u32 = 8;
/// The sound manager's values that place a sound (`0x678260`, docs/re/sound.md, "A sound's place"),
/// as the running original holds them (#731): the volume at the listener (`+0x84`), the hearing
/// distance's base (`+0xB790`), the distance within which a sound pans less the nearer it is
/// (`+0xB798`), and the curve's two points (`+0xB7A0` to `+0xB7AC`): the first's distance in the
/// hearing distance and its volume between 0.5 and the volume at the listener, the second's
/// distance and volume.
const NEAR_VOLUME: f32 = 0.75;
const HEARING: f32 = 1_200.0;
const CENTRED_WITHIN: f32 = 260.0;
const CURVE: [f32; 4] = [0.5, 0.5, 0.9, 0.1];

/// The weapons' groups (docs/re/sound.md, "Profile groups").
const HIT: u8 = 1;
const PARADE: u8 = 2;
const SHOOT: u8 = 3;
const WOOSH: u8 = 5;
/// The fist's profile, for a striker with bare hands.
const FIST: u32 = 502;

/// The profiles' sample variants and the samples loaded, and the dice.
#[derive(Resource)]
pub struct Sfx {
    /// Each profile's groups: the variants' sound ids and sample files.
    profiles: HashMap<u32, Vec<Vec<(u32, String)>>>,
    /// The sound ids and sample files by the original's names (`SOUND_FX_HITENEMY_01_SERA`).
    named: HashMap<String, (u32, String)>,
    /// The sample files by their sound ids.
    files: HashMap<u32, String>,
    samples: HashMap<u32, Handle<AudioSource>>,
    pub(crate) dice: Rng,
}

impl Sfx {
    /// The profiles of the sound index `sound`.
    pub fn new(sound: &Sound) -> Self {
        let files: HashMap<u32, String> = sound
            .samples
            .iter()
            .map(|&id| (id, sample_file(id)))
            .collect();
        let profiles = sound
            .profiles
            .iter()
            .map(|profile| {
                let groups = profile
                    .groups
                    .iter()
                    .map(|ids| {
                        ids.iter()
                            .filter_map(|id| files.get(id).map(|f| (*id, f.clone())))
                            .collect()
                    })
                    .collect();
                (profile.slot, groups)
            })
            .collect();
        let named = sound
            .names
            .iter()
            .filter_map(|n| {
                let file = files.get(&n.id)?;
                Some((n.name.clone(), (n.id, file.clone())))
            })
            .collect();
        Self {
            profiles,
            named,
            files,
            samples: HashMap::new(),
            dice: Rng::of(0, 0, 4),
        }
    }

    /// A variant of `profile`'s `group`: a slot of 8 at random, the group's variants repeating
    /// over them (`0x4261D0`); none for an empty group.
    pub fn pick(&mut self, profile: u32, group: u8) -> Option<(u32, String)> {
        let variants = self.profiles.get(&profile)?.get(usize::from(group))?;
        if variants.is_empty() {
            return None;
        }
        let slot = usize::try_from(self.dice.below(SLOTS)).ok()?;
        variants.get(slot % variants.len()).cloned()
    }

    /// The sound id and the sample file of the sound named `name`.
    pub fn named(&self, name: &str) -> Option<(u32, String)> {
        self.named.get(name).cloned()
    }

    /// The sound id and the sample file of the sound `id`, if it has one.
    pub fn with_id(&self, id: u32) -> Option<(u32, String)> {
        Some((id, self.files.get(&id)?.clone()))
    }

    /// The sample of the sound `id`, loaded from `file` once.
    pub fn sample(&mut self, id: u32, file: String, assets: &AssetServer) -> Handle<AudioSource> {
        self.samples
            .entry(id)
            .or_insert_with(|| assets.load(file))
            .clone()
    }

    /// A number of `0..n` at random, as a float.
    pub fn roll(&mut self, n: u32) -> f32 {
        f32::from(u16::try_from(self.dice.below(n)).unwrap_or(0))
    }
}

/// The distance of a sound `x`, `y` screen pixels from the listener, as the original approximates
/// it with integers (`0x678260`): 1,007 of the longer side and 441 of the shorter per 1,024, less
/// 40 of the longer while it is under 16 times the shorter.
fn distance(x: i32, y: i32) -> i32 {
    let (x, y) = (x.abs(), y.abs());
    let (long, short) = (x.max(y), x.min(y));
    let mut sum = long * 1_007 + short * 441;
    if long < short * 16 {
        sum -= long * 40;
    }
    (sum + 512) >> 10
}

/// The volume's factor, 1 at the listener, and the panning, −1 left to 1 right, of a sound `x`,
/// `y` screen pixels at zoom 1 from the listener, right and down (`0x678260`, at the factor 1).
/// The volume falls along three lines to silence at the hearing distance, 900 pixels; the pan
/// goes by the side, the more the less the sound lies above or below the listener, and back to
/// the middle within 260 pixels.
#[expect(
    clippy::cast_precision_loss,
    reason = "a distance in pixels, far below 2^24"
)]
pub fn placed(x: i32, y: i32) -> (f32, f32) {
    let hearing = HEARING * (1.5 - NEAR_VOLUME);
    let near = distance(x, y) as f32;
    let d = near.min(hearing);
    let [first, first_volume, second, second_volume] = CURVE;
    let (first, second) = (first * hearing, second * hearing);
    let first_volume = (0.5 - first_volume) * (NEAR_VOLUME - 0.5) + first_volume;
    let volume = if d < first {
        NEAR_VOLUME - (NEAR_VOLUME - first_volume) / first * d
    } else if d < second {
        first_volume - (d - first) * (first_volume - second_volume) / (second - first)
    } else if d < hearing {
        second_volume - (d - second) * second_volume / (hearing - second)
    } else {
        0.0
    };
    if d <= 0.0 {
        return (volume / NEAR_VOLUME, 0.0);
    }
    // The pan from 0, left, to 1, right: by the side, half way back for the share of the distance
    // that runs up or down the screen, wholly to the side's sign straight above or below (the
    // original's quirk).
    let up = -y as f32 / d;
    let side = match (x.signum(), up <= 0.0) {
        (1, true) => up * 0.5 + 1.0,
        (1, false) => 1.0 - up * 0.5,
        (-1, true) => -(up * 0.5),
        (-1, false) => up * 0.5,
        _ => up,
    };
    let centred = (1.0 - d / CENTRED_WITHIN).clamp(0.0, 1.0);
    let pan = if side < 0.5 {
        (0.5 - side) * centred + side
    } else {
        side - (side - 0.5) * centred
    };
    (volume / NEAR_VOLUME, (pan * 2.0 - 1.0).clamp(-1.0, 1.0))
}

/// The volume's factor and the panning of a sound at `at`, by the listener's place, the hero's:
/// its offset in the original's screen pixels, `48 (cx − cy)` right and `24 (cx + cy)` down.
#[expect(
    clippy::cast_possible_truncation,
    reason = "an offset in pixels within the world, far below 2^31"
)]
pub fn placed_at(at: Vec3, listener: Option<Vec3>) -> (f32, f32) {
    let offset = listener.map_or(Vec3::ZERO, |hero| at - hero);
    let (cx, cy) = (offset.x / iso::CELL, offset.z / iso::CELL);
    placed(
        (48.0 * (cx - cy)).round() as i32,
        (24.0 * (cx + cy)).round() as i32,
    )
}

/// A volume's factor in decibels.
pub fn decibels(volume: f32) -> Decibels {
    Decibels(20.0 * volume.max(1e-4).log10())
}

/// The swing's profile of the class of what a striker holds (`0x546F80`; the switch at
/// `0x4EA6FD`): 0 and 5 the knife's, 1, 2 and 8 the sword's, 3 the axe's, 4 the staff's, 6 the
/// thrown knife's, 9 and 10 the long staff's; none for the rest.
pub fn swing_profile(class: u8) -> Option<u32> {
    match class {
        0 | 5 => Some(503),
        1 | 2 | 8 => Some(500),
        3 => Some(509),
        4 => Some(505),
        6 => Some(504),
        9 | 10 => Some(506),
        _ => None,
    }
}

/// What a striker holds, for its weapons' sounds: the class of it (`motions::class`), and the
/// sound profile of its weapon, `items.pak +0x24` of the weapon's own type, 0 for one without
/// (the heroes' starting weapons, whose type has no slot of its own: their hits sound by their
/// swing alone, as the original's, #709), none for bare hands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Armed {
    pub class: u8,
    pub weapon: Option<u32>,
}

impl Armed {
    /// The profile of the sound at the target: the weapon's, or the fist's for bare hands.
    fn at_target(self) -> u32 {
        self.weapon.unwrap_or(FIST)
    }
}

/// A weapon's sound: a strike's swing by its striker, the sound at a hit's target by its
/// striker's weapon (`landed` whether the hit did damage), or a shot by its shooter's.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blow {
    Swing {
        striker: Entity,
    },
    Target {
        striker: Entity,
        target: Entity,
        landed: bool,
    },
    Shot {
        shooter: Entity,
    },
}

/// What a creature holds, by its arms (`npcs::Arms`) or the hero's.
fn armed_of(
    entity: Entity,
    hero: Option<&hero::Hero>,
    heroes: &Query<(), With<Body>>,
    arms: &Query<&crate::npcs::Arms>,
    npcs: Option<&crate::npcs::Npcs>,
) -> Armed {
    if heroes.contains(entity) {
        return hero.map(|hero| hero.armed).unwrap_or_default();
    }
    arms.get(entity)
        .ok()
        .map(|arms| arms.armed(npcs))
        .unwrap_or_default()
}

/// Plays the weapons' sounds.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn blows(
    mut blows: MessageReader<Blow>,
    mut sfx: ResMut<Sfx>,
    hero: Option<Res<hero::Hero>>,
    npcs: Option<Res<crate::npcs::Npcs>>,
    heroes: Query<(), With<Body>>,
    arms: Query<&crate::npcs::Arms>,
    places: Query<&GlobalTransform>,
    listeners: Query<&GlobalTransform, With<Body>>,
    assets: Res<AssetServer>,
    audio: Res<Audio>,
) {
    let listener = listeners.iter().next().map(GlobalTransform::translation);
    let armed = |entity| armed_of(entity, hero.as_deref(), &heroes, &arms, npcs.as_deref());
    for &blow in blows.read() {
        let (at, profile, group, loud) = match blow {
            Blow::Swing { striker } => {
                let Some(profile) = swing_profile(armed(striker).class) else {
                    continue;
                };
                (striker, profile, WOOSH, (0.975 + sfx.roll(5) / 90.0) * 0.5)
            }
            Blow::Target {
                striker,
                target,
                landed,
            } => (
                target,
                armed(striker).at_target(),
                if landed { HIT } else { PARADE },
                0.5,
            ),
            Blow::Shot { shooter } => {
                let Some(profile) = armed(shooter).weapon else {
                    continue;
                };
                (shooter, profile, SHOOT, 0.5)
            }
        };
        let Ok(place) = places.get(at) else {
            continue;
        };
        let Some((id, file)) = sfx.pick(profile, group) else {
            continue;
        };
        let (near, pan) = placed_at(place.translation(), listener);
        let volume = loud * near;
        if volume <= 0.0 {
            continue;
        }
        let sample = sfx.sample(id, file, &assets);
        audio
            .play(sample)
            .with_volume(decibels(volume))
            .with_panning(pan);
        debug!(
            "sfx: {blow:?}: profile {profile} group {group} ({id}) at {volume:.2}, panned {pan:.2}"
        );
    }
}

/// The weapons' sounds.
pub struct SfxPlugin;

impl Plugin for SfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Blow>()
            .add_systems(Update, blows.in_set(Unpaused));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The volume and the pan of the original's own calls (#731), from 0 to 1 and at its volume
    /// 0.75 at the listener: three creatures about the Seraphim at the start.
    #[test]
    fn a_sound_falls_and_pans_as_the_originals() {
        let original = |x: i32, y: i32| {
            let (volume, pan) = placed(x, y);
            (volume * NEAR_VOLUME, f32::midpoint(pan, 1.0))
        };
        let close = |(a, b): (f32, f32), (c, d): (f32, f32)| {
            assert!((a - c).abs() < 1e-3 && (b - d).abs() < 1e-3, "{a} {b}");
        };
        assert_eq!(distance(647, 470), 813);
        close(original(-239, 223), (0.571, 0.346));
        close(original(647, 470), (0.097, 0.711));
        close(original(-149, -927), (0.0, 0.515));
        close(original(0, 7), (0.746, 0.46));
        assert_eq!(placed(0, 0), (1.0, 0.0));
    }

    #[test]
    fn the_swing_is_the_profile_of_the_class() {
        assert_eq!(swing_profile(0), Some(503));
        assert_eq!(swing_profile(1), Some(500));
        assert_eq!(swing_profile(8), Some(500));
        assert_eq!(swing_profile(3), Some(509));
        assert_eq!(swing_profile(7), None);
    }

    #[test]
    fn bare_hands_sound_by_the_fist_and_a_weapon_by_its_profile() {
        assert_eq!(Armed::default().at_target(), FIST);
        let sword = Armed {
            class: 1,
            weapon: Some(500),
        };
        assert_eq!(sword.at_target(), 500);
        // A starting weapon without a profile of its own is silent at the target.
        let starting = Armed {
            class: 1,
            weapon: Some(0),
        };
        assert_eq!(starting.at_target(), 0);
    }
}
