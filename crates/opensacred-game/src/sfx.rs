//! The fight's sounds but the voices (#712; docs/re/sound.md, "The fight's sounds"): the profiles
//! of `sound/sound.ron`, a variant of a group picked as the original's `0x4261D0` picks one, the
//! samples, and a sound's volume and pan by its place; and the weapons' sounds: a strike's swing
//! (`WOOSH`) by the class of what the striker holds, at its hit moment within reach or in the
//! air; at the target, the striker's weapon's `HIT` when the hit did damage, else its `PARADE`,
//! by the item's sound profile (`items.pak +0x24`), the fist's for bare hands, an animal that
//! takes no damage hearing a sword's, a mace's, an axe's or a dagger's miss instead and nothing
//! of another weapon ([`Missing`], #1168); a shot's `SHOOT` by the bow's profile, and a plain
//! arrow's own `HIT` by it where the arrow ends in what it hits (#3208); and a gun's
//! fire at each of its shots ([`Gunfire`], #1963). The world's and the interface's other sounds
//! that the modules ask for, at an entity or in the middle ([`Play`]) or at a point ([`PlayAt`],
//! #1173): the doors', the items' taken and the level-up's among them (#1166, #1167;
//! docs/re/sound.md, "The sources").
//!
//! The volume falls and the pan follows the place from the hero as the original's `0x678260`
//! gives them (docs/re/sound.md, "A sound's place", #731), by the volume at the listener that the
//! camera's zoom sets ([`Listener`], #1181); a placed sound is placed again at every frame as it
//! plays, following its entity and the hero ([`Placed`], #3334).

use std::collections::HashMap;

use bevy::log::debug;
use bevy::prelude::{
    App, AssetServer, Assets, Camera, Entity, GlobalTransform, Handle, IntoScheduleConfigs,
    Message, MessageReader, MessageWriter, OnExit, Plugin, Projection, Query, Res, ResMut,
    Resource, Time, Update, Vec3, With,
};
use bevy_kira_audio::prelude::{
    Audio, AudioControl, AudioInstance, AudioSource, AudioTween, Decibels, PlaybackState,
};
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
            dice: crate::seed::dice(4),
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
pub fn placed(x: i32, y: i32, listener: Listener) -> (f32, f32) {
    placed_by(x, y, listener, 1.0)
}

/// [`placed`] at the place's factor `factor` (`0x678260`'s second argument; docs/re/sound.md, "A
/// sound's place"; #3330): it scales the hearing distance and the volumes of the curve's two later
/// points, never the volume at the listener, so that a sound of a factor above 1 grows louder with
/// its distance; beyond the sound manager's records' reach, 1,200 pixels whatever the factor, no
/// sound is placed and it stays silent (`+0xB790`).
#[expect(
    clippy::cast_precision_loss,
    reason = "a distance in pixels, far below 2^24"
)]
pub fn placed_by(x: i32, y: i32, listener: Listener, factor: f32) -> (f32, f32) {
    let at_listener = listener.0;
    let hearing = HEARING * (1.5 - at_listener) * factor;
    let near = distance(x, y) as f32;
    if near > HEARING {
        return (0.0, 0.0);
    }
    let d = near.min(hearing);
    let [first, first_volume, second, second_volume] = CURVE;
    let (first, second) = (first * hearing, second * hearing);
    let first_volume = ((0.5 - first_volume) * (at_listener - 0.5) + first_volume) * factor;
    let second_volume = second_volume * factor;
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
pub fn placed_at(at: Vec3, listener: Option<Vec3>, volume: Listener) -> (f32, f32) {
    placed_at_by(at, listener, volume, 1.0)
}

/// [`placed_at`] at the place's factor `factor` ([`placed_by`]).
#[expect(
    clippy::cast_possible_truncation,
    reason = "an offset in pixels within the world, far below 2^31"
)]
pub fn placed_at_by(at: Vec3, listener: Option<Vec3>, volume: Listener, factor: f32) -> (f32, f32) {
    let offset = listener.map_or(Vec3::ZERO, |hero| at - hero);
    let (cx, cy) = (offset.x / iso::CELL, offset.z / iso::CELL);
    placed_by(
        (48.0 * (cx - cy)).round() as i32,
        (24.0 * (cx + cy)).round() as i32,
        volume,
        factor,
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
fn listen(
    mut listener: ResMut<Listener>,
    projections: Query<&Projection, crate::iso::Eye<With<Camera>>>,
) {
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

/// A placed sound as it plays, which [`replace`] places again at every frame by its entity's
/// place, or its point, and the listener's, as the original's update places each placed channel
/// again at each of the world's ticks (`0x6A0035`, `0x678260` at `0x6A006E`; docs/re/sound.md, "A
/// sound's place", #3334).
#[derive(Clone, Debug)]
pub struct Following {
    handle: Handle<AudioInstance>,
    /// The entity that it plays at, none for a point or once the entity is gone.
    of: Option<Entity>,
    /// Its place, the entity's last.
    at: Vec3,
    /// The place's factor ([`placed_by`]).
    pub factor: f32,
    /// The share of the place's volume that it plays at.
    pub scale: f32,
    /// The most volume that it plays at.
    pub most: f32,
    /// How long its instance has been waited for, its sample loading.
    waited: f32,
}

impl Following {
    /// The sound of `handle` at `at`, the place of the entity `of` where it plays at one, at the
    /// factor 1 and its place's whole volume.
    pub fn new(handle: Handle<AudioInstance>, of: Option<Entity>, at: Vec3) -> Self {
        Self {
            handle,
            of,
            at,
            factor: 1.0,
            scale: 1.0,
            most: f32::INFINITY,
            waited: 0.0,
        }
    }
}

/// The placed sounds that play ([`Following`]).
#[derive(Resource, Default, Debug)]
pub struct Placed(Vec<Following>);

impl Placed {
    /// Places `sound` again at every frame while it plays.
    pub fn follow(&mut self, sound: Following) {
        self.0.push(sound);
    }
}

/// How long a placed sound's instance is waited for before the sound is let go (its sample never
/// loaded).
const STARTING: f32 = 10.0;

/// What becomes of a placed sound at `at` at a frame, with the listener at `listener`, whose
/// entity is gone where `gone`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Again {
    /// It is placed again.
    Placed,
    /// Beyond the sound manager's records' reach, 1,200 pixels whatever the factor, it keeps its
    /// volume and pan, as the manager holds no record of its object (`0x6A0035`, `0x68407F`).
    Kept,
    /// It is let go: the record of a thing gone, dropped beyond the reach, is not made again
    /// (`0x683964`–`0x683B65`).
    Dropped,
}

fn again(at: Vec3, listener: Vec3, gone: bool) -> Again {
    if f64::from(screen_distance(at, listener)) <= f64::from(HEARING) {
        Again::Placed
    } else if gone {
        Again::Dropped
    } else {
        Again::Kept
    }
}

/// Places the placed sounds again by their entities' places and the hero's, and lets go of those
/// that have stopped.
fn replace(
    mut placed: ResMut<Placed>,
    places: Query<&GlobalTransform>,
    (listeners, volume): (Query<&GlobalTransform, With<Body>>, Res<Listener>),
    mut instances: ResMut<Assets<AudioInstance>>,
    time: Res<Time>,
) {
    let listener = listeners.iter().next().map(GlobalTransform::translation);
    let dt = time.delta_secs();
    placed.0.retain_mut(|sound| {
        let Some(mut instance) = instances.get_mut(&sound.handle) else {
            sound.waited += dt;
            return sound.waited < STARTING;
        };
        if matches!(instance.state(), PlaybackState::Stopped) {
            return false;
        }
        if let Some(of) = sound.of {
            match places.get(of) {
                Ok(place) => sound.at = place.translation(),
                Err(_) => sound.of = None,
            }
        }
        let Some(listener) = listener else {
            return true;
        };
        match again(sound.at, listener, sound.of.is_none()) {
            Again::Placed => {
                let (near, pan) = placed_at_by(sound.at, Some(listener), *volume, sound.factor);
                let volume = (near * sound.scale).min(sound.most);
                instance.set_decibels(decibels(volume), AudioTween::default());
                instance.set_panning(pan, AudioTween::default());
                true
            }
            Again::Kept => true,
            Again::Dropped => false,
        }
    });
}

/// Lets go of the placed sounds as the game ends.
fn forget(mut placed: ResMut<Placed>) {
    placed.0.clear();
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

/// A sound of the world at a point rather than at an entity, as [`Play`] plays one: an object's
/// that goes as it sounds, a trap's explosion (#1173).
#[derive(Message, Clone, Copy, Debug)]
pub struct PlayAt {
    pub sound: Of,
    pub point: Vec3,
    /// The rate's factor: a base, and what a roll adds to it.
    pub rate: (f32, Roll),
    /// The place's factor (`playSFX`'s seventh argument, [`placed_by`]): 1 but the gun's fire's 5.
    pub factor: f32,
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
    /// `rand() % 20` / 90.
    Twentieths,
    /// `rand()` / 32767 times the span.
    Share(f32),
}

/// What a striker holds, for its weapons' sounds: the class of it (`motions::class`), and the
/// sound profile of its weapon, `items.pak +0x24` of the weapon's type or of its parent where it
/// has no slot of its own (the heroes' starting weapons, #2488), 0 for one without (the types of
/// #2496, silent), none for bare hands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Armed {
    pub class: u8,
    pub weapon: Option<u32>,
    /// The weapon's type, an `items.pak` slot.
    pub kind: Option<u32>,
    /// Whether it holds a gun in its right hand (the weapon kinds 13 and 14), whose shots fire
    /// ([`Gunfire`]).
    pub gun: bool,
}

impl Armed {
    /// The profile of the sound at the target: the weapon's, or the fist's for bare hands.
    fn at_target(self) -> u32 {
        self.weapon.unwrap_or(FIST)
    }
}

/// A weapon's sound: a strike's swing by its striker, the sound at a hit's target by its
/// striker's weapon (`landed` whether the hit did damage), a shot by its shooter's, or a plain
/// arrow's own hit by its shooter's at the creature it ends in.
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
    Arrow {
        shooter: Entity,
        struck: Entity,
    },
}

/// A shot's clip that starts, or a gun art's shot (`0x4DCB80`, the Dwarf's executor): the gun's
/// fire where its shooter holds a gun ([`gunfire`], #1963).
#[derive(Message, Clone, Copy, Debug)]
pub struct Gunfire {
    pub shooter: Entity,
}

/// The gun's fire's place's factor (`0x7C3C19`, #3330).
const GUNFIRE_FACTOR: f32 = 5.0;

/// The gun's fire's samples: `DWARF_PISTOL` for the guns 1855 and 1857, `DWARF_RIFLE` for 1856
/// and every other (`0x7C3AC0`, its event's `+0x38`).
const PISTOL: u32 = 2126;
const RIFLE: u32 = 2127;
const PISTOLS: [u32; 2] = [1855, 1857];

/// The gun's fire's sound (1172 `gunfire`, its start `0x7C3AC0`): its sample by the gun at the
/// shooter, the rate's factor 0.4 and up to 0.2 more (`rand()` / 32767 · 0.2), so 0.9 to 1.1
/// times the sample's; at the place's factor 5.0 ([`GUNFIRE_FACTOR`]), louder with its distance and
/// heard to 1,200 pixels, taken from its start where the original's update takes it from its
/// first tick (a sample once placed is not placed again, #3334).
fn gun_sample(kind: Option<u32>) -> u32 {
    if kind.is_some_and(|kind| PISTOLS.contains(&kind)) {
        PISTOL
    } else {
        RIFLE
    }
}

/// Sounds the gun's fire of each [`Gunfire`] whose shooter holds a gun.
fn gunfire(
    mut fires: MessageReader<Gunfire>,
    hero: Option<Res<hero::Hero>>,
    npcs: Option<Res<crate::npcs::Npcs>>,
    (heroes, arms, places): (
        Query<(), With<Body>>,
        Query<&crate::npcs::Arms>,
        Query<&GlobalTransform>,
    ),
    mut points: MessageWriter<PlayAt>,
) {
    for &Gunfire { shooter } in fires.read() {
        let armed = armed_of(shooter, hero.as_deref(), &heroes, &arms, npcs.as_deref());
        let Ok(place) = places.get(shooter) else {
            continue;
        };
        if armed.gun {
            points.write(PlayAt {
                sound: Of::Id(gun_sample(armed.kind)),
                point: place.translation(),
                rate: (0.4, Roll::Share(0.2)),
                factor: GUNFIRE_FACTOR,
            });
        }
    }
}

/// What a creature holds, by its arms (`npcs::Arms`) or the hero's.
pub(crate) fn armed_of(
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
    (assets, audio, mut placing): (Res<AssetServer>, Res<Audio>, ResMut<Placed>),
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
            // The plain arrow's own hit, by the weapon's type that its shot gave it, its `HIT`
            // as the shot's `SHOOT` (`0x5BB010`'s case `0xBFE`, `0x5BBE00`).
            Blow::Arrow { shooter, struck } => {
                let Some(profile) = armed(shooter).weapon else {
                    continue;
                };
                (struck, profile, HIT, 0.5)
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
        let handle = audio
            .play(sample)
            .with_volume(decibels(volume))
            .with_panning(pan)
            .handle();
        let mut following = Following::new(handle, Some(at), place.translation());
        following.scale = loud;
        placing.follow(following);
        debug!(
            "sfx: {blow:?}: profile {profile} group {group} ({id}) at {volume:.2}, panned {pan:.2}"
        );
    }
}

/// Plays the sounds of the world and of the interface that the modules ask for, in a pause too.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn plays(
    (mut plays, mut points): (MessageReader<Play>, MessageReader<PlayAt>),
    mut sfx: ResMut<Sfx>,
    places: Query<&GlobalTransform>,
    (listeners, volume, levels): (
        Query<&GlobalTransform, With<Body>>,
        Res<Listener>,
        Option<Res<crate::volumes::Levels>>,
    ),
    (assets, audio, mut placing): (Res<AssetServer>, Res<Audio>, ResMut<Placed>),
) {
    let listener = listeners.iter().next().map(GlobalTransform::translation);
    // Each sound at its place, an entity's or a point, at its factor, an entity's that is gone
    // dropped, or in the middle.
    type Asked = (Of, Option<(Option<Entity>, Vec3)>, (f32, Roll), f32);
    let asked: Vec<Asked> = plays
        .read()
        .filter_map(|play| match play.at {
            Some(at) => places.get(at).ok().map(|place| {
                (
                    play.sound,
                    Some((Some(at), place.translation())),
                    play.rate,
                    1.0,
                )
            }),
            None => Some((play.sound, None, play.rate, 1.0)),
        })
        .chain(
            points
                .read()
                .map(|play| (play.sound, Some((None, play.point)), play.rate, play.factor)),
        )
        .collect();
    // No volume above 1 reaches the original's samples, its place's times the effects' level
    // (`0x693D8C`, `0x6A0D30`): of the game's, which take the place's over the near volume on the
    // effects' channel, at most 1 over the near volume times that level.
    let most = levels
        .filter(|levels| levels.effects > 0.0)
        .map_or(f32::INFINITY, |levels| 1.0 / (NEAR_VOLUME * levels.effects));
    for (sound, place, rate, factor) in asked {
        let (volume, pan) = match place {
            Some((_, at)) => placed_at_by(at, listener, *volume, factor),
            None => placed(0, 0, *volume),
        };
        let volume = volume.min(most);
        if volume <= 0.0 {
            continue;
        }
        let picked = match sound {
            Of::Profile(profile, group) => sfx.pick(profile, group),
            Of::Id(id) => sfx.with_id(id),
        };
        let Some((id, file)) = picked else {
            continue;
        };
        let (base, roll) = rate;
        let rate =
            base + match roll {
                Roll::None => 0.0,
                Roll::Tenths => sfx.roll(10) / 90.0,
                Roll::Twentieths => sfx.roll(20) / 90.0,
                Roll::Share(span) => span * sfx.share(),
            } + 0.5;
        let sample = sfx.sample(id, file, &assets);
        let handle = audio
            .play(sample)
            .with_volume(decibels(volume))
            .with_panning(pan)
            .with_playback_rate(f64::from(rate))
            .handle();
        if let Some((of, at)) = place {
            let mut following = Following::new(handle, of, at);
            following.factor = factor;
            following.most = most;
            placing.follow(following);
        }
        debug!("sfx: {sound:?} ({id}) at {volume:.2}, panned {pan:.2}, at a rate of {rate:.2}");
    }
}

/// `DIEFLAMES`, the sound of a creature that burns away (`undead`).
pub(crate) const DIES_IN_FLAMES: u32 = 7029;
/// `CHAR_SPLASH`, the sound of a creature that bursts (`0x4E8DA0`, `undead`, #1260).
pub(crate) const SPLASH: u32 = 3000;

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
            .add_message::<Gunfire>()
            .add_message::<Play>()
            .add_message::<PlayAt>()
            .init_resource::<Listener>()
            .init_resource::<Placed>()
            .add_systems(
                bevy::prelude::OnEnter(crate::game::AppState::InGame),
                missing,
            )
            .add_systems(OnExit(crate::game::AppState::InGame), forget)
            .add_systems(
                Update,
                // The asked sounds play in a pause too, the interface's alone being asked then:
                // the menu over a game's click as it shows (#1767); the placed ones are placed
                // again as the world goes, at its ticks in the original.
                (
                    listen,
                    (blows, gunfire).in_set(Unpaused),
                    plays,
                    replace.in_set(Unpaused),
                )
                    .chain(),
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
    fn a_gunshot_grows_louder_with_its_distance_and_falls_silent_past_the_records() {
        // The original's volume at the factor 5, zoom 1 (docs/re/sound.md, "A gunshot against a
        // plain sample"), along the screen's level: `distance(x, 0)` is `⌊(1,007 x + 512) / 1,024⌋`.
        let at = |x: i32| placed_by(x, 0, Listener::default(), GUNFIRE_FACTOR).0 * NEAR_VOLUME;
        assert!((at(0) - 0.75).abs() < 1e-6);
        // 450 pixels away: 0.75 + 1.75 · 450 / 2,250.
        let d = distance(458, 0);
        assert_eq!(d, 450);
        assert!((at(458) - 1.1).abs() < 1e-3, "{}", at(458));
        // 1,200 pixels away, the records' reach: 2.24 times a plain sample at the listener.
        assert_eq!(distance(1_220, 0), 1_200);
        assert!((at(1_220) - 1.683_333).abs() < 1e-3, "{}", at(1_220));
        assert_eq!(at(1_222), 0.0);
        // A plain sample is silent from 900 pixels.
        assert_eq!(placed_by(916, 0, Listener::default(), 1.0).0, 0.0);
        assert!(placed_by(905, 0, Listener::default(), 1.0).0 > 0.0);
    }

    #[test]
    fn a_placed_sound_follows_within_the_records_reach() {
        let listener = Vec3::new(100.0, 0.0, 100.0);
        // A cell along the screen's level is 48 pixels at the zoom 1.
        let cells = |n: f32| listener + Vec3::new(n * iso::CELL, 0.0, -n * iso::CELL) * 0.5;
        assert_eq!(screen_distance(cells(25.0), listener), 1_180);
        assert_eq!(again(listener, listener, false), Again::Placed);
        assert_eq!(again(cells(25.0), listener, true), Again::Placed);
        assert_eq!(screen_distance(cells(25.5), listener), 1_204);
        // Beyond it a living thing's sound keeps where it is, a thing's gone is let go.
        assert_eq!(again(cells(25.5), listener, false), Again::Kept);
        assert_eq!(again(cells(25.5), listener, true), Again::Dropped);
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
            gun: false,
        };
        assert_eq!(sword.at_target(), 500);
        // A weapon of no profile, its parent's none either, is silent at the target (#2496).
        let starting = Armed {
            class: 1,
            weapon: Some(0),
            kind: None,
            gun: false,
        };
        assert_eq!(starting.at_target(), 0);
    }

    #[test]
    fn a_guns_fire_is_the_pistols_for_its_two_pistols_and_the_rifles_else() {
        assert_eq!(gun_sample(Some(1855)), PISTOL);
        assert_eq!(gun_sample(Some(1857)), PISTOL);
        assert_eq!(gun_sample(Some(1856)), RIFLE);
        assert_eq!(gun_sample(Some(5000)), RIFLE);
        assert_eq!(gun_sample(None), RIFLE);
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
