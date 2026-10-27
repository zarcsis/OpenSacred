//! The fight's sounds but the voices (#712; docs/re/sound.md, "The fight's sounds"): the profiles
//! of `sound/sound.ron`, a variant of a group picked as the original's `0x4261D0` picks one, the
//! samples, and a sound's volume and pan by its place; and the weapons' sounds: a strike's swing
//! (`WOOSH`) by the class of what the striker holds, at its hit moment within reach or in the
//! air; at the target, the striker's weapon's `HIT` when the hit did damage, else its `PARADE`,
//! by the item's sound profile (`items.pak +0x24`), the fist's for bare hands; and a shot's
//! `SHOOT` by the bow's profile. The world's other sounds that the modules ask for ([`Play`]):
//! the doors', the items' taken and the level-up's (#1166, #1167; docs/re/sound.md, "The
//! sources").
//!
//! The volume falls and the pan follows the place from the hero as the original's `0x678260`
//! gives them (docs/re/sound.md, "A sound's place", #731).

use std::collections::HashMap;

use bevy::log::debug;
use bevy::prelude::{
    Added, App, AssetServer, Camera, Entity, GlobalTransform, Handle, IntoScheduleConfigs, Message,
    MessageReader, MessageWriter, Plugin, Projection, Query, Res, ResMut, Resource, Update, Vec3,
    With,
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
/// The weapons' group of a miss, which an animal takes (`0x532731`).
const MISS: u8 = 0;
/// The animals that take a weapon's miss where a strike does no damage, by their types (as
/// compared at `0x53274E`–`0x532850`).
fn misses(target: u32) -> bool {
    matches!(target, 512 | 536 | 543 | 545 | 567..=570 | 573..=582 | 584..=588 | 591)
}

/// The weapons whose miss an animal hears: those whose type's name past `TYPE_WEAPON_` holds
/// `SWORD`, `MACE`, `AXE` or `DAGGER` (the strings at `0x956024`–`0x956038`), by their types.
#[derive(Resource, Default)]
pub struct Missing(std::collections::HashSet<u32>);

impl Missing {
    /// The weapons of the types' names `names`.
    pub fn of(names: &[opensacred_data::tables::TypeName]) -> Self {
        Self(
            names
                .iter()
                .filter(|n| {
                    n.name.get(12..).is_some_and(|rest| {
                        ["SWORD", "MACE", "AXE", "DAGGER"]
                            .iter()
                            .any(|part| rest.contains(part))
                    })
                })
                .map(|n| n.kind)
                .collect(),
        )
    }
}

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

    /// A number from 0 to 1 at random, as the client's `rand()` / 32767.
    pub fn share(&mut self) -> f32 {
        self.roll(0x8000) / 32_767.0
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
/// The volume at the listener `listener` (`+0x84`, [`Listener`]) narrows the hearing distance as
/// it rises, and the factor is taken of the volume at zoom 1, so that a sound at the listener
/// plays louder zoomed in and quieter zoomed out.
#[expect(
    clippy::cast_precision_loss,
    reason = "a distance in pixels, far below 2^24"
)]
pub fn placed(x: i32, y: i32, listener: Listener) -> (f32, f32) {
    let at_listener = listener.0;
    let hearing = HEARING * (1.5 - at_listener);
    let near = distance(x, y) as f32;
    let d = near.min(hearing);
    let [first, first_volume, second, second_volume] = CURVE;
    let (first, second) = (first * hearing, second * hearing);
    let first_volume = (0.5 - first_volume) * (at_listener - 0.5) + first_volume;
    let volume = if d < first {
        at_listener - (at_listener - first_volume) / first * d
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
pub fn placed_at(at: Vec3, listener: Option<Vec3>, volume: Listener) -> (f32, f32) {
    let offset = listener.map_or(Vec3::ZERO, |hero| at - hero);
    let (cx, cy) = (offset.x / iso::CELL, offset.z / iso::CELL);
    placed(
        (48.0 * (cx - cy)).round() as i32,
        (24.0 * (cx + cy)).round() as i32,
        volume,
    )
}

/// The integer distance of `at` from the listener at `listener` in the original's screen pixels
/// at zoom 1, as the sound manager measures its lists' records (`0x6912B0`, `0x678260`'s rule).
#[expect(
    clippy::cast_possible_truncation,
    reason = "an offset in pixels within the world, far below 2^31"
)]
pub fn screen_distance(at: Vec3, listener: Vec3) -> i32 {
    let offset = at - listener;
    let (cx, cy) = (offset.x / iso::CELL, offset.z / iso::CELL);
    distance(
        (48.0 * (cx - cy)).round() as i32,
        (24.0 * (cx + cy)).round() as i32,
    )
}

/// The volume at the listener (`+0x84`), which the zoom sets (#1181): `cWorldView_setZoom`
/// (`0x628130`) sends the sound manager the event `(9, 0x15)` of the zoom in hundredths, which
/// its handler (`0x67C105`) holds within 50 to 200 and makes 1 − (z − 50)·0.005 below 100, 0.75
/// at 100 and 0.75 − (z − 100)·0.0025 above: 1 at the zoom 0.5, 0.5 at 2.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Listener(pub f32);

impl Default for Listener {
    fn default() -> Self {
        Self(NEAR_VOLUME)
    }
}

impl Listener {
    /// The volume at the listener at the zoom `zoom`.
    pub fn of_zoom(zoom: f32) -> Self {
        let z = (zoom * 100.0).trunc().clamp(50.0, 200.0);
        Self(if z > 100.0 {
            0.75 - (z - 100.0) * 0.0025
        } else if z < 100.0 {
            1.0 - (z - 50.0) * 0.005
        } else {
            0.75
        })
    }
}

/// Keeps the volume at the listener by the camera's zoom.
fn listen(mut listener: ResMut<Listener>, projections: Query<&Projection, With<Camera>>) {
    let Some(zoom) = projections.iter().find_map(|projection| match projection {
        Projection::Orthographic(ortho) => Some(ortho.scale),
        _ => None,
    }) else {
        return;
    };
    let now = Listener::of_zoom(zoom);
    if *listener != now {
        *listener = now;
    }
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

/// The objects' groups: a door's opening and closing (`0x695800`, from `0x44BCD0`).
pub const OPEN: u8 = 1;
pub const CLOSE: u8 = 3;

/// The profile of a door of the type `door`, an object of category 10, as `0x695800` chooses it:
/// the grid 603, the barn's 604, the dungeon's and the gate's 606, the prison's 607, the fence's
/// 608, any other whose type's name holds `TYPE_OBJECT_DOOR` the wooden door 602; the secret door
/// 5221 (`TYPE_OBJECT_S_DOOR`), the only door whose name does not, none.
pub fn door_profile(door: u32) -> Option<u32> {
    match door {
        5221 => None,
        5674 => Some(603),
        5673 | 5749 => Some(604),
        5700..=5709 | 5740 | 5741 => Some(606),
        5720..=5729 => Some(607),
        5743 | 5745 => Some(608),
        _ => Some(602),
    }
}

/// A sound of the world that a module asks for (#1166, #1167): a variant of a profile's group or
/// a sound, at an object or a creature, by its place's volume and pan, or in the middle (the
/// original's creature −1), at the volume 1 and a rate of its sample's of `playSFX`'s factor plus
/// 0.5 (`0x693510`).
#[derive(Message, Clone, Copy, Debug)]
pub struct Play {
    pub sound: Of,
    pub at: Option<Entity>,
    /// The rate's factor: a base, and what a roll adds to it.
    pub rate: (f32, Roll),
}

/// What a [`Play`] plays.
#[derive(Clone, Copy, Debug)]
pub enum Of {
    /// A variant of a profile's group ([`Sfx::pick`]).
    Profile(u32, u8),
    /// The sound of an id.
    Id(u32),
}

/// What a roll adds to a rate's factor.
#[derive(Clone, Copy, Debug)]
pub enum Roll {
    None,
    /// `rand() % 10` / 90.
    Tenths,
    /// `rand()` / 32767 times the span.
    Share(f32),
}

/// What a striker holds, for its weapons' sounds: the class of it (`motions::class`), and the
/// sound profile of its weapon, `items.pak +0x24` of the weapon's own type, 0 for one without
/// (the heroes' starting weapons, whose type has no slot of its own: their hits sound by their
/// swing alone, as the original's, #709), none for bare hands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Armed {
    pub class: u8,
    pub weapon: Option<u32>,
    /// The weapon's type, an `items.pak` slot.
    pub kind: Option<u32>,
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
    (arms, minds, missing): (
        Query<&crate::npcs::Arms>,
        Query<&crate::ai::Mind>,
        Option<Res<Missing>>,
    ),
    places: Query<&GlobalTransform>,
    (listeners, volume): (Query<&GlobalTransform, With<Body>>, Res<Listener>),
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
            } => {
                let armed = armed(striker);
                // A strike that does no damage to an animal sounds the miss of a weapon of its
                // kinds, else nothing (`0x532731`).
                let animal = minds
                    .get(target)
                    .is_ok_and(|mind| misses(mind.thinks.creature));
                let group = match (landed, animal) {
                    (true, _) => HIT,
                    (false, false) => PARADE,
                    (false, true) => {
                        let heard = armed.kind.is_some_and(|kind| {
                            missing.as_ref().is_some_and(|m| m.0.contains(&kind))
                        });
                        if !heard {
                            continue;
                        }
                        MISS
                    }
                };
                (target, armed.at_target(), group, 0.5)
            }
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
        let (near, pan) = placed_at(place.translation(), listener, *volume);
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

/// Plays the sounds of the world that the modules ask for.
fn plays(
    mut plays: MessageReader<Play>,
    mut sfx: ResMut<Sfx>,
    places: Query<&GlobalTransform>,
    (listeners, volume): (Query<&GlobalTransform, With<Body>>, Res<Listener>),
    assets: Res<AssetServer>,
    audio: Res<Audio>,
) {
    let listener = listeners.iter().next().map(GlobalTransform::translation);
    for play in plays.read() {
        let (volume, pan) = match play.at {
            Some(at) => {
                let Ok(place) = places.get(at) else {
                    continue;
                };
                placed_at(place.translation(), listener, *volume)
            }
            None => placed(0, 0, *volume),
        };
        if volume <= 0.0 {
            continue;
        }
        let picked = match play.sound {
            Of::Profile(profile, group) => sfx.pick(profile, group),
            Of::Id(id) => sfx.with_id(id),
        };
        let Some((id, file)) = picked else {
            continue;
        };
        let (base, roll) = play.rate;
        let rate =
            base + match roll {
                Roll::None => 0.0,
                Roll::Tenths => sfx.roll(10) / 90.0,
                Roll::Share(span) => span * sfx.share(),
            } + 0.5;
        let sample = sfx.sample(id, file, &assets);
        audio
            .play(sample)
            .with_volume(decibels(volume))
            .with_panning(pan)
            .with_playback_rate(f64::from(rate));
        debug!(
            "sfx: {:?} ({id}) at {volume:.2}, panned {pan:.2}, at a rate of {rate:.2}",
            play.sound
        );
    }
}

/// The fire ghoul, which burns away as it dies (`DIEFLAMES`, #1168).
const FIRE_GHOUL: u32 = 73;
const DIES_IN_FLAMES: u32 = 7029;

/// A fire ghoul dying sounds `DIEFLAMES` at itself (the death's action, `0x53ECA0`, its case of
/// the type 73 at `0x5416DD`); the magic skeletons' and the undead goblins' cases, which sound it
/// in a branch not read, do not.
fn burns_away(
    dying: Query<(Entity, &crate::ai::Mind), Added<crate::npcs::Dies>>,
    mut sounds: MessageWriter<Play>,
) {
    for (creature, mind) in &dying {
        if mind.thinks.creature == FIRE_GHOUL {
            sounds.write(Play {
                sound: Of::Id(DIES_IN_FLAMES),
                at: Some(creature),
                rate: (0.5, Roll::None),
            });
        }
    }
}

/// The weapons whose miss an animal hears, by the types' names of a new game's tables.
fn missing(mut commands: bevy::prelude::Commands, data: Option<Res<crate::game::Data>>) {
    let names = data
        .as_ref()
        .and_then(|data| data.0.tables.type_names().ok());
    commands.insert_resource(names.map_or_else(Missing::default, Missing::of));
}

/// The weapons' sounds and the world's.
pub struct SfxPlugin;

impl Plugin for SfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Blow>()
            .add_message::<Play>()
            .init_resource::<Listener>()
            .add_systems(
                bevy::prelude::OnEnter(crate::game::AppState::InGame),
                missing,
            )
            .add_systems(
                Update,
                (listen, (blows, burns_away, plays).in_set(Unpaused)).chain(),
            );
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
            let (volume, pan) = placed(x, y, Listener::default());
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
        assert_eq!(placed(0, 0, Listener::default()), (1.0, 0.0));
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
    fn the_zoom_sets_the_volume_at_the_listener() {
        let close = |zoom: f32, volume: f32| {
            assert!((Listener::of_zoom(zoom).0 - volume).abs() < 1e-5, "{zoom}");
        };
        close(1.0, 0.75);
        close(0.5, 1.0);
        close(0.75, 0.875);
        close(2.0, 0.5);
        close(1.5, 0.625);
        close(3.0, 0.5);
        // Nearer, louder at the listener and heard less far.
        let (near, _) = placed(0, 0, Listener::of_zoom(0.5));
        assert!((near - 1.0 / 0.75).abs() < 1e-5);
        assert_eq!(placed(700, 0, Listener::of_zoom(0.5)).0, 0.0);
        assert!(placed(700, 0, Listener::of_zoom(2.0)).0 > 0.0);
    }

    #[test]
    fn a_door_sounds_by_its_type() {
        assert_eq!(door_profile(5650), Some(602));
        assert_eq!(door_profile(5674), Some(603));
        assert_eq!(door_profile(5749), Some(604));
        assert_eq!(door_profile(5709), Some(606));
        assert_eq!(door_profile(5741), Some(606));
        assert_eq!(door_profile(5720), Some(607));
        assert_eq!(door_profile(5745), Some(608));
        // The addon's dwarves' doors and the second desert gate by their names.
        assert_eq!(door_profile(5766), Some(602));
        assert_eq!(door_profile(5783), Some(602));
        assert_eq!(door_profile(5221), None);
    }

    #[test]
    fn bare_hands_sound_by_the_fist_and_a_weapon_by_its_profile() {
        assert_eq!(Armed::default().at_target(), FIST);
        let sword = Armed {
            class: 1,
            weapon: Some(500),
            kind: Some(1712),
        };
        assert_eq!(sword.at_target(), 500);
        // A starting weapon without a profile of its own is silent at the target.
        let starting = Armed {
            class: 1,
            weapon: Some(0),
            kind: None,
        };
        assert_eq!(starting.at_target(), 0);
    }

    #[test]
    fn an_animal_hears_the_miss_of_a_sword_a_mace_an_axe_or_a_dagger() {
        let name = |kind: u32, name: &str| opensacred_data::tables::TypeName {
            kind,
            name: name.into(),
        };
        let missing = Missing::of(&[
            name(1, "TYPE_WEAPON_SHORTSWORD"),
            name(2, "TYPE_WEAPON_BATTLEAXE"),
            name(3, "TYPE_WEAPON_STAFF"),
            name(4, "TYPE_NPC_AXEMAN"),
        ]);
        assert!(missing.0.contains(&1) && missing.0.contains(&2));
        assert!(!missing.0.contains(&3));
        assert!(misses(536) && misses(591) && !misses(572));
    }
}
