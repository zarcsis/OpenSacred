//! The creatures' and the heroes' voices in the fight (#711; docs/re/sound.md, "The fight's
//! sounds"): a creature speaks through its type's voice (`tables/voices.ron`, `0x6852B0`) at a
//! strike's start that does not chain (`ATTACK`), at a hit that did damage (`NORMALHIT`, or
//! `CRITICALHIT` below a quarter of its health) and at its death (`DEATH`). A line plays when its
//! roll gives 0, the heroes' taking one of their two profiles; it waits for nothing of its sound
//! id but at a death; it gives way to the creature's own line still playing but at a hit, which
//! stops that line, and at a death; it records the wait of its id. One line a creature.
//!
//! The hero speaks lines of its own too, MP3 samples by name (`0x693FE0`, #713): a taunt when a
//! creature takes a hit's line, one in five and 10 s after the last; a battle cry at a combat art,
//! one in three; and its death's line.
//!
//! The line plays at the original's volume, 0.45 + (`rand() % 10`) / 90, lowered with the
//! distance from the hero and panned by its side as the original places it (`sfx::placed`,
//! `0x678260`).
//!
//! Between the fights the creatures make their idle noises (#714; docs/re/sound.md, "The idle
//! noises"), as the sound manager's update does (`0x69B770`) at each world tick: a creature near
//! the hero that is not hostile to it rolls one in 70, one hostile to it one in 20 while it does
//! not fight, then its type's noise rolls its own (`tables/idle_noises.ron`) and plays a variant
//! of its profile's group 0 or one of its run of sound ids, unless the creature speaks or the id
//! waits; the id then waits the noise's seconds, and the volume adds the noise's own.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use bevy::log::debug;
use bevy::prelude::{
    Added, App, AssetServer, Assets, Commands, Component, Entity, GlobalTransform, Handle, Has,
    IntoScheduleConfigs, Message, MessageReader, MessageWriter, Plugin, Query, Res, ResMut,
    Resource, Time, Update, With, Without,
};
use bevy_kira_audio::prelude::{Audio, AudioControl, AudioInstance, AudioTween, PlaybackState};
use opensacred_data::tables::{IDLE_NOISES, IdleNoise, IdleNoises, IdleSound, VOICES, Voice};

use crate::files;
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

/// The hero's taunts' wait, in seconds.
const TAUNTS_AFTER: f64 = 10.0;

/// The idle noises' rolls at each world tick (`0x69B770`): one in 20 for a creature hostile to the
/// hero (the update's first list), one in 70 for the others (the second).
const HOSTILE_IDLE: u8 = 20;
const PEACEFUL_IDLE: u8 = 70;
/// The profiles' group of the idle noises.
const IDLE: u8 = 0;

/// The hero makes a combat art: its battle cry, one in three.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BattleCry {
    pub hero: Entity,
}

/// The hero's lines by name (`0x693FE0`): the name's stem, how many a class has, and the roll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HeroLine {
    HitEnemy,
    BattleCry,
    EnemyKillMe,
}

impl HeroLine {
    fn stem(self) -> &'static str {
        match self {
            Self::HitEnemy => "HITENEMY",
            Self::BattleCry => "BATTLECRY",
            Self::EnemyKillMe => "ENEMYKILLME",
        }
    }

    fn count(self) -> u32 {
        match self {
            Self::HitEnemy => 11,
            Self::BattleCry => 10,
            Self::EnemyKillMe => 6,
        }
    }

    fn roll(self) -> u8 {
        match self {
            Self::HitEnemy => 5,
            Self::BattleCry => 3,
            Self::EnemyKillMe => 1,
        }
    }

    /// The sound's name of the line `n` of the hero of type `creature`.
    fn name(self, creature: u32, n: u32) -> Option<String> {
        let class = match creature {
            1 => "SERA",
            2 => "GLAD",
            3 => "MAGE",
            4 => "DELF",
            5 => "HELF",
            6 | 7 => "VAMP",
            8 => "DWARF",
            9 => "DEMON",
            _ => return None,
        };
        Some(format!("SOUND_FX_{}_{:02}_{class}", self.stem(), n + 1))
    }
}

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
    /// When the hero last taunted.
    taunted: Option<f64>,
    /// The idle noises of the types hostile to the hero, and of the others.
    hostile_idle: HashMap<u32, IdleNoise>,
    peaceful_idle: HashMap<u32, IdleNoise>,
    dice: Rng,
}

impl Voices {
    /// Reads `tables/voices.ron`; without it (assets converted before it), no creature speaks.
    pub fn read(assets: &Path) -> Result<Self> {
        let path = assets.join("tables").join(VOICES);
        let voices = if files::exists(&path) {
            let text = files::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
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
        // Without `tables/idle_noises.ron` (assets converted before it), no creature makes one.
        let path = assets.join("tables").join(IDLE_NOISES);
        let idle = if files::exists(&path) {
            let text = files::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            ron::from_str(&text).with_context(|| format!("parsing {}", path.display()))?
        } else {
            IdleNoises {
                hostile: Vec::new(),
                peaceful: Vec::new(),
            }
        };
        let by_type = |noises: Vec<IdleNoise>| {
            noises
                .into_iter()
                .map(|n| (u32::from(n.creature), n))
                .collect()
        };
        Ok(Self {
            voices,
            waits: HashMap::new(),
            taunted: None,
            hostile_idle: by_type(idle.hostile),
            peaceful_idle: by_type(idle.peaceful),
            dice: Rng::of(0, 0, 3),
        })
    }

    /// The idle noise of `creature` at this tick, hostile to the hero or not, if the update's roll
    /// and then the type's give one.
    fn idle_roll(&mut self, creature: u32, hostile: bool) -> Option<IdleNoise> {
        let (roll, noises) = if hostile {
            (HOSTILE_IDLE, &self.hostile_idle)
        } else {
            (PEACEFUL_IDLE, &self.peaceful_idle)
        };
        let noise = noises.get(&creature)?.clone();
        (rolls(roll, &mut self.dice) && rolls(noise.roll, &mut self.dice)).then_some(noise)
    }

    /// The idle noise of `creature` at this tick, if the rolls give one: the sound id, its file,
    /// its wait and the volume it adds.
    fn idle_noise(
        &mut self,
        sfx: &mut Sfx,
        creature: u32,
        hostile: bool,
    ) -> Option<(u32, String, f32, f32)> {
        let noise = self.idle_roll(creature, hostile)?;
        let (id, file) = match noise.sound {
            IdleSound::Profile(profile) => sfx.pick(u32::from(profile), IDLE)?,
            IdleSound::Ids { first, count } => {
                sfx.with_id(u32::from(first) + self.dice.below(u32::from(count)))?
            }
        };
        Some((id, file, noise.wait, noise.volume))
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

    /// The hero's `line`, of the hero of type `creature`, if its roll gives one (a taunt also
    /// 10 s after the last, at `now`): one of the class's at random.
    fn hero_line(
        &mut self,
        sfx: &Sfx,
        creature: u32,
        line: HeroLine,
        now: f64,
    ) -> Option<(u32, String)> {
        if line == HeroLine::HitEnemy && self.taunted.is_some_and(|t| now - t < TAUNTS_AFTER) {
            return None;
        }
        if !rolls(line.roll(), &mut self.dice) {
            return None;
        }
        let name = line.name(creature, self.dice.below(line.count()))?;
        let found = sfx.named(&name)?;
        if line == HeroLine::HitEnemy {
            self.taunted = Some(now);
        }
        Some(found)
    }
}

/// A line to play: who speaks it, its sound id and file, the wait it records, and what it does
/// to the speaker's own line.
struct Said {
    speaker: Entity,
    id: u32,
    file: String,
    wait: Option<f32>,
    play: Play,
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

/// Plays the lines that the fight asks for: the creatures' by their voices, and the hero's own.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, its queries with their filters"
)]
fn speak(
    mut commands: Commands,
    mut lines: MessageReader<Speak>,
    mut cries: MessageReader<BattleCry>,
    mut voices: ResMut<Voices>,
    mut sfx: ResMut<Sfx>,
    hero: Option<Res<hero::Hero>>,
    speakers: Query<(
        &GlobalTransform,
        Option<&crate::ai::Mind>,
        Has<Body>,
        Option<&Speaking>,
    )>,
    heroes: Query<(Entity, &GlobalTransform), With<Body>>,
    assets: Res<AssetServer>,
    audio: Res<Audio>,
    mut instances: ResMut<Assets<AudioInstance>>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs_f64();
    let the_hero = heroes.iter().next();
    let listener = the_hero.map(|(_, at)| at.translation());
    let hero_type = hero.as_ref().map(|hero| hero.creature);
    let playing = |speaking: Option<&Speaking>| {
        speaking.is_some_and(|s| {
            matches!(
                audio.state(&s.0),
                PlaybackState::Playing { .. } | PlaybackState::Queued
            )
        })
    };
    let mut said = Vec::new();
    for line in lines.read() {
        let Ok((_, mind, is_hero, speaking)) = speakers.get(line.speaker) else {
            continue;
        };
        if is_hero && line.group == DEATH {
            // The hero's death speaks its line by name (`0x6852B0`, `0x693FE0`).
            if let Some((id, file)) = hero_type
                .and_then(|creature| voices.hero_line(&sfx, creature, HeroLine::EnemyKillMe, now))
            {
                said.push(Said {
                    speaker: line.speaker,
                    id,
                    file,
                    wait: None,
                    play: Play::Interrupts,
                });
            }
            continue;
        }
        // A creature's hit makes the hero taunt (`0x6852B0`'s hero branch).
        if !is_hero
            && matches!(line.group, NORMAL_HIT | CRITICAL_HIT)
            && let (Some((hero, _)), Some(creature)) = (the_hero, hero_type)
            && let Some((id, file)) = voices.hero_line(&sfx, creature, HeroLine::HitEnemy, now)
        {
            said.push(Said {
                speaker: hero,
                id,
                file,
                wait: None,
                play: Play::Plays,
            });
        }
        let creature = match (is_hero, mind) {
            (true, _) => match hero_type {
                Some(creature) => creature,
                None => continue,
            },
            (false, Some(mind)) => mind.thinks.creature,
            (false, None) => continue,
        };
        let Some((id, file, wait)) = voices.line(&mut sfx, creature, line.group) else {
            continue;
        };
        let waiting = voices.waits.get(&id).is_some_and(|&until| until > now);
        let play = play(line.group, playing(speaking), waiting);
        if play != Play::GivesWay {
            said.push(Said {
                speaker: line.speaker,
                id,
                file,
                wait: Some(wait),
                play,
            });
        }
    }
    for cry in cries.read() {
        if let Some((id, file)) = hero_type
            .and_then(|creature| voices.hero_line(&sfx, creature, HeroLine::BattleCry, now))
        {
            said.push(Said {
                speaker: cry.hero,
                id,
                file,
                wait: None,
                play: Play::Plays,
            });
        }
    }
    for line in said {
        let Ok((at, _, _, speaking)) = speakers.get(line.speaker) else {
            continue;
        };
        match line.play {
            Play::GivesWay => continue,
            Play::Plays if playing(speaking) => continue,
            Play::Plays => {}
            Play::Interrupts => {
                if let Some(speaking) = speaking
                    && let Some(mut old) = instances.get_mut(&speaking.0)
                {
                    old.stop(AudioTween::default());
                }
            }
        }
        let loud = 0.45 + sfx.roll(10) / 90.0;
        let (near, pan) = sfx::placed_at(at.translation(), listener);
        let volume = loud * near;
        if volume <= 0.0 {
            continue;
        }
        let sample = sfx.sample(line.id, line.file, &assets);
        let instance = audio
            .play(sample)
            .with_volume(sfx::decibels(volume))
            .with_panning(pan)
            .handle();
        if let Some(wait) = line.wait {
            voices.waits.insert(line.id, now + f64::from(wait));
        }
        debug!(
            "voices: {} speaks {}, at {volume:.2}, panned {pan:.2}",
            line.speaker, line.id
        );
        commands.entity(line.speaker).insert(Speaking(instance));
    }
}

/// The creatures near the hero make their idle noises at the world's tick: those not hostile to
/// it in any state, those hostile while they do not fight (the original's state of rest, the
/// record's `+0xDBC` 0, which `0x68BEA0` keeps from the creature's moves and strikes); the noise
/// plays as the voices do, at most at the full volume.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn idle(
    mut commands: Commands,
    mut voices: ResMut<Voices>,
    mut sfx: ResMut<Sfx>,
    creatures: Query<
        (
            Entity,
            &GlobalTransform,
            &crate::ai::Mind,
            Option<&Speaking>,
        ),
        Without<Dies>,
    >,
    heroes: Query<&GlobalTransform, With<Body>>,
    assets: Res<AssetServer>,
    audio: Res<Audio>,
    time: Res<Time>,
) {
    let Some(listener) = heroes.iter().next().map(GlobalTransform::translation) else {
        return;
    };
    let now = time.elapsed_secs_f64();
    for (creature, at, mind, speaking) in &creatures {
        let (near, pan) = sfx::placed_at(at.translation(), Some(listener));
        if near <= 0.0 {
            continue;
        }
        let speaks = speaking.is_some_and(|s| {
            matches!(
                audio.state(&s.0),
                PlaybackState::Playing { .. } | PlaybackState::Queued
            )
        });
        let hostile = mind.thinks.hostile();
        if speaks || (hostile && mind.thinks.fighting()) {
            continue;
        }
        let Some((id, file, wait, louder)) =
            voices.idle_noise(&mut sfx, mind.thinks.creature, hostile)
        else {
            continue;
        };
        if voices.waits.get(&id).is_some_and(|&until| until > now) {
            continue;
        }
        let loud = (0.45 + sfx.roll(10) / 90.0 + louder).min(1.0);
        let volume = loud * near;
        let sample = sfx.sample(id, file, &assets);
        let instance = audio
            .play(sample)
            .with_volume(sfx::decibels(volume))
            .with_panning(pan)
            .handle();
        voices.waits.insert(id, now + f64::from(wait));
        debug!("voices: {creature} makes the idle noise {id}, at {volume:.2}, panned {pan:.2}");
        commands.entity(creature).insert(Speaking(instance));
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
            .add_message::<BattleCry>()
            .add_systems(Update, (dies, speak).chain().in_set(Unpaused))
            .add_systems(
                Update,
                idle.after(speak)
                    .in_set(Unpaused)
                    .run_if(crate::frame::ticked),
            );
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
    fn the_hero_lines_are_named_by_the_class() {
        assert_eq!(
            HeroLine::HitEnemy.name(1, 0).as_deref(),
            Some("SOUND_FX_HITENEMY_01_SERA")
        );
        assert_eq!(
            HeroLine::BattleCry.name(9, 9).as_deref(),
            Some("SOUND_FX_BATTLECRY_10_DEMON")
        );
        assert_eq!(
            HeroLine::EnemyKillMe.name(5, 5).as_deref(),
            Some("SOUND_FX_ENEMYKILLME_06_HELF")
        );
        assert_eq!(HeroLine::HitEnemy.name(76, 0), None);
    }

    #[test]
    fn an_idle_noise_rolls_one_in_20_hostile_or_70_not_then_its_own() {
        let noise = |creature, roll| IdleNoise {
            creature,
            roll,
            sound: IdleSound::Profile(2014),
            wait: 0.0,
            volume: 0.0,
        };
        let mut voices = Voices {
            voices: HashMap::new(),
            waits: HashMap::new(),
            taunted: None,
            hostile_idle: HashMap::from([(512, noise(512, 2))]),
            peaceful_idle: HashMap::from([(560, noise(560, 2)), (677, noise(677, 1))]),
            dice: Rng::of(0, 0, 3),
        };
        let mut count = |creature, hostile| {
            (0..56_000)
                .filter(|_| voices.idle_roll(creature, hostile).is_some())
                .count()
        };
        // A wolf hostile to the hero: 56,000 / 20 / 2 = 1,400; a pig 400; a novice 800.
        let wolf = count(512, true);
        assert!((1_250..1_550).contains(&wolf), "{wolf}");
        let pig = count(560, false);
        assert!((330..470).contains(&pig), "{pig}");
        let novice = count(677, false);
        assert!((700..900).contains(&novice), "{novice}");
        // No noise of its list, none at all.
        assert_eq!(count(512, false), 0);
        assert_eq!(count(76, true), 0);
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
