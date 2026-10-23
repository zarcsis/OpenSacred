//! The fight's sounds but the voices (#712; docs/re/sound.md, "The fight's sounds"): the profiles
//! of `sound/sound.ron`, a variant of a group picked as the original's `0x4261D0` picks one, the
//! samples, and a sound's volume and pan by its place; and the weapons' sounds: a strike's swing
//! (`WOOSH`) by the class of what the striker holds, at its hit moment within reach or in the
//! air; at the target, the striker's weapon's `HIT` when the hit did damage, else its `PARADE`,
//! by the item's sound profile (`items.pak +0x24`), the fist's for bare hands; and a shot's
//! `SHOOT` by the bow's profile.
//!
//! The volume falls and the pan follows the place from the hero by a plain falloff for now
//! (docs/decisions/2026-10-23-voices-place.md, #731).

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use bevy::log::debug;
use bevy::prelude::{
    App, AssetServer, Camera3d, Entity, GlobalTransform, Handle, IntoScheduleConfigs, Message,
    MessageReader, Plugin, Query, Res, ResMut, Resource, Update, Vec3, With,
};
use bevy_kira_audio::prelude::{Audio, AudioControl, AudioSource, Decibels};
use opensacred_data::sound::Sound;

use crate::frame::Unpaused;
use crate::hero::{self, Body};
use crate::spawning::Rng;

/// The slots of a profile's group, over which its variants repeat in turn (docs/02 §10.1).
const SLOTS: u32 = 8;
/// The distance from the hero, in world units, at which a sound falls silent, and the screen's
/// half width in world units at which it pans to one side (the decision above).
const HEARD_WITHIN: f32 = 1_600.0;
const PANS_AT: f32 = 600.0;

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
    samples: HashMap<u32, Handle<AudioSource>>,
    pub(crate) dice: Rng,
}

impl Sfx {
    /// Reads the profiles of `sound/sound.ron`.
    pub fn read(assets: &Path) -> Result<Self> {
        let path = assets.join("sound").join(opensacred_data::sound::INDEX);
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let sound: Sound =
            ron::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        let files: HashMap<u32, &str> = sound
            .samples
            .iter()
            .map(|s| (s.id, s.file.as_str()))
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
                            .filter_map(|id| files.get(id).map(|f| (*id, format!("sound/{f}"))))
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
                Some((n.name.clone(), (n.id, format!("sound/{file}"))))
            })
            .collect();
        Ok(Self {
            profiles,
            named,
            samples: HashMap::new(),
            dice: Rng::of(0, 0, 4),
        })
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

/// The volume's factor and the panning of a sound `offset` world units from the hero on the
/// ground, `side` of them along the screen's width.
pub fn placed(offset: f32, side: f32) -> (f32, f32) {
    let near = (1.0 - offset / HEARD_WITHIN).clamp(0.0, 1.0);
    (near, (side / PANS_AT).clamp(-1.0, 1.0))
}

/// The volume's factor and the panning of a sound at `at`, by the hero's place and the camera's
/// right, `listener` and `right`.
pub fn placed_at(at: Vec3, listener: Option<Vec3>, right: Vec3) -> (f32, f32) {
    let offset = listener.map_or(Vec3::ZERO, |hero| (at - hero).with_y(0.0));
    placed(
        offset.length(),
        offset.dot(right.with_y(0.0).normalize_or_zero()),
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
    camera: Query<&GlobalTransform, With<Camera3d>>,
    assets: Res<AssetServer>,
    audio: Res<Audio>,
) {
    let listener = listeners.iter().next().map(GlobalTransform::translation);
    let right = camera
        .iter()
        .next()
        .map_or(Vec3::X, |c| c.right().as_vec3());
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
        let (near, pan) = placed_at(place.translation(), listener, right);
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

    #[test]
    fn a_sound_falls_with_the_distance_and_pans_by_its_side() {
        assert_eq!(placed(0.0, 0.0), (1.0, 0.0));
        assert_eq!(placed(800.0, 600.0), (0.5, 1.0));
        assert_eq!(placed(2_000.0, -900.0), (0.0, -1.0));
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
