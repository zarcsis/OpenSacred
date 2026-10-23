//! The creatures' and the heroes' voices in the fight (#711; docs/re/sound.md, "The fight's
//! sounds"): a creature speaks through its type's voice (`tables/voices.ron`, `0x6852B0`) at a
//! strike's start that does not chain (`ATTACK`), at a hit that did damage (`NORMALHIT`, or
//! `CRITICALHIT` below a quarter of its health) and at its death (`DEATH`). A line plays when its
//! roll gives 0, the heroes' taking one of their two profiles; it waits for nothing of its sound
//! id but at a death; it gives way to the creature's own line still playing but at a hit, which
//! stops that line, and at a death; it records the wait of its id. One line a creature.
//!
//! The line plays at the original's volume, 0.45 + (`rand() % 10`) / 90, lowered with the
//! distance from the hero and panned by its side on the screen (docs/decisions/2026-10-23-voices-place.md:
//! the original's falloff, `0x678260`, is not read yet).

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use bevy::log::debug;
use bevy::prelude::{
    Added, App, AssetServer, Assets, Camera3d, Commands, Component, Entity, GlobalTransform,
    Handle, Has, IntoScheduleConfigs, Message, MessageReader, MessageWriter, Plugin, Query, Res,
    ResMut, Resource, Time, Update, Vec3, With,
};
use bevy_kira_audio::prelude::{Audio, AudioControl, AudioInstance, AudioTween, PlaybackState};
use opensacred_data::tables::{VOICES, Voice};

use crate::frame::Unpaused;
use crate::hero::{self, Body};
use crate::npcs::Dies;
use crate::sfx::{self, Sfx};
use crate::spawning::Rng;

/// The groups of a creature's profile that the fight passes (docs/re/sound.md, "Profile groups").
pub const ATTACK: u8 = 1;
pub const NORMAL_HIT: u8 = 2;
pub const CRITICAL_HIT: u8 = 3;
pub const DEATH: u8 = 7;

/// The share of its most health under which a hit's line is `CRITICALHIT` (`0x532161`: the
/// record's `+0x2C` below 26, **hypothesis**: its health in per cent).
const CRITICAL: u32 = 26;

/// A creature speaks: the group of its profile that the fight passes.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Speak {
    pub speaker: Entity,
    pub group: u8,
}

impl Speak {
    /// The line of a hit that did damage to `target`, at `health` of `most` left.
    pub fn hit(target: Entity, health: u32, most: u32) -> Self {
        let critical = most > 0 && health.saturating_mul(100) < CRITICAL * most;
        Self {
            speaker: target,
            group: if critical { CRITICAL_HIT } else { NORMAL_HIT },
        }
    }
}

/// The line a creature speaks, while it plays.
#[derive(Component)]
struct Speaking(Handle<AudioInstance>);

/// The voices of the types, the waits of the sound ids that played, and the dice.
#[derive(Resource)]
pub struct Voices {
    voices: HashMap<u32, Voice>,
    /// Until when, in the game's seconds, a sound id waits.
    waits: HashMap<u32, f64>,
    dice: Rng,
}

impl Voices {
    /// Reads `tables/voices.ron`; without it (assets converted before it), no creature speaks.
    pub fn read(assets: &Path) -> Result<Self> {
        let path = assets.join("tables").join(VOICES);
        let voices = if path.exists() {
            let text =
                fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
            let table: opensacred_data::tables::Voices =
                ron::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
            table
                .voices
                .into_iter()
                .map(|v| (u32::from(v.creature), v))
                .collect()
        } else {
            HashMap::new()
        };
        Ok(Self {
            voices,
            waits: HashMap::new(),
            dice: Rng::of(0, 0, 3),
        })
    }

    /// The line of `creature`'s voice for `group`, if its roll gives one: the profile (one of
    /// the heroes' two at random), a variant of its group (`Sfx::pick`), the sound id and its
    /// file, and the voice's wait.
    fn line(&mut self, sfx: &mut Sfx, creature: u32, group: u8) -> Option<(u32, String, f32)> {
        let voice = self.voices.get(&creature)?;
        if !rolls(voice.roll(group), &mut self.dice) {
            return None;
        }
        let count = u32::try_from(voice.profiles.len()).ok()?;
        let profile = voice.profiles[usize::try_from(self.dice.below(count)).ok()?];
        let wait = voice.wait;
        let (id, file) = sfx.pick(u32::from(profile), group)?;
        Some((id, file, wait))
    }
}

/// Whether a line of a group of roll `n` plays: always for 1, never for 0, else when
/// `rand() % n` gives 0.
fn rolls(n: u8, dice: &mut Rng) -> bool {
    match n {
        0 => false,
        1 => true,
        n => dice.below(u32::from(n)) == 0,
    }
}

/// What a line of `group` does: plays, plays over the creature's own line (stopping it), or
/// gives way. `speaking` the creature's own line still playing, `waiting` its sound id's wait
/// still running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Play {
    Plays,
    Interrupts,
    GivesWay,
}

fn play(group: u8, speaking: bool, waiting: bool) -> Play {
    match group {
        DEATH => Play::Interrupts,
        NORMAL_HIT if waiting => Play::GivesWay,
        NORMAL_HIT if speaking => Play::Interrupts,
        NORMAL_HIT => Play::Plays,
        CRITICAL_HIT if speaking => Play::GivesWay,
        CRITICAL_HIT => Play::Plays,
        _ if speaking || waiting => Play::GivesWay,
        _ => Play::Plays,
    }
}

/// Plays the lines that the fight asks for.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn speak(
    mut commands: Commands,
    mut lines: MessageReader<Speak>,
    mut voices: ResMut<Voices>,
    mut sfx: ResMut<Sfx>,
    hero: Option<Res<hero::Hero>>,
    speakers: Query<(
        &GlobalTransform,
        Option<&crate::ai::Mind>,
        Has<Body>,
        Option<&Speaking>,
    )>,
    heroes: Query<&GlobalTransform, With<Body>>,
    camera: Query<&GlobalTransform, With<Camera3d>>,
    assets: Res<AssetServer>,
    audio: Res<Audio>,
    mut instances: ResMut<Assets<AudioInstance>>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs_f64();
    let listener = heroes.iter().next().map(GlobalTransform::translation);
    let right = camera
        .iter()
        .next()
        .map_or(Vec3::X, |c| c.right().as_vec3());
    for line in lines.read() {
        let Ok((at, mind, is_hero, speaking)) = speakers.get(line.speaker) else {
            continue;
        };
        let creature = if is_hero {
            let Some(hero) = &hero else {
                continue;
            };
            hero.creature
        } else if let Some(mind) = mind {
            mind.thinks.creature
        } else {
            continue;
        };
        let Some((id, file, wait)) = voices.line(&mut sfx, creature, line.group) else {
            continue;
        };
        let still = speaking.is_some_and(|s| {
            matches!(
                audio.state(&s.0),
                PlaybackState::Playing { .. } | PlaybackState::Queued
            )
        });
        let waiting = voices.waits.get(&id).is_some_and(|&until| until > now);
        match play(line.group, still, waiting) {
            Play::GivesWay => continue,
            Play::Interrupts => {
                if let Some(speaking) = speaking
                    && let Some(mut line) = instances.get_mut(&speaking.0)
                {
                    line.stop(AudioTween::default());
                }
            }
            Play::Plays => {}
        }
        let loud = 0.45 + sfx.roll(10) / 90.0;
        let (near, pan) = sfx::placed_at(at.translation(), listener, right);
        let volume = loud * near;
        if volume <= 0.0 {
            continue;
        }
        let sample = sfx.sample(id, file, &assets);
        let instance = audio
            .play(sample)
            .with_volume(sfx::decibels(volume))
            .with_panning(pan)
            .handle();
        voices.waits.insert(id, now + f64::from(wait));
        debug!(
            "voices: {} of type {creature} speaks group {} ({id}), at {volume:.2}, panned {pan:.2}",
            line.speaker, line.group
        );
        commands.entity(line.speaker).insert(Speaking(instance));
    }
}

/// A creature that dies speaks its death.
fn dies(dying: Query<Entity, Added<Dies>>, mut lines: MessageWriter<Speak>) {
    for speaker in &dying {
        lines.write(Speak {
            speaker,
            group: DEATH,
        });
    }
}

/// The voices of the fight.
pub struct VoicesPlugin;

impl Plugin for VoicesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Speak>()
            .add_systems(Update, (dies, speak).chain().in_set(Unpaused));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_plays_by_its_roll() {
        let mut dice = Rng::of(0, 0, 3);
        assert!(rolls(1, &mut dice));
        assert!(!rolls(0, &mut dice));
        // One in four over many rolls.
        let played = (0..4_000).filter(|_| rolls(4, &mut dice)).count();
        assert!((800..1_200).contains(&played), "{played}");
    }

    #[test]
    fn a_line_gives_way_or_interrupts_by_its_group() {
        // An attack's line gives way to the creature's own line and to its id's wait.
        assert_eq!(play(ATTACK, false, false), Play::Plays);
        assert_eq!(play(ATTACK, true, false), Play::GivesWay);
        assert_eq!(play(ATTACK, false, true), Play::GivesWay);
        // A hit's line stops the creature's own, but waits for its id.
        assert_eq!(play(NORMAL_HIT, true, false), Play::Interrupts);
        assert_eq!(play(NORMAL_HIT, false, true), Play::GivesWay);
        // A critical hit's gives way to the creature's own line, not to the wait.
        assert_eq!(play(CRITICAL_HIT, true, false), Play::GivesWay);
        assert_eq!(play(CRITICAL_HIT, false, true), Play::Plays);
        // A death always plays, over the creature's own line.
        assert_eq!(play(DEATH, true, true), Play::Interrupts);
    }

    #[test]
    fn a_hit_below_a_quarter_of_the_health_is_critical() {
        let target = Entity::PLACEHOLDER;
        assert_eq!(Speak::hit(target, 30, 100).group, NORMAL_HIT);
        assert_eq!(Speak::hit(target, 26, 100).group, NORMAL_HIT);
        assert_eq!(Speak::hit(target, 25, 100).group, CRITICAL_HIT);
        assert_eq!(Speak::hit(target, 10, 52).group, CRITICAL_HIT);
    }
}
