//! The creatures' and the heroes' voices in the fight (#711; docs/re/sound.md, "The fight's
//! sounds"): a creature speaks through its type's voice (`tables/voices.ron`, `0x6852B0`) at a
//! strike's start that does not chain (`ATTACK`), at a hit that did damage (`NORMALHIT`, or
//! `CRITICALHIT` below a quarter of its health) and at its death (`DEATH`). A line plays when its
//! roll gives 0, the heroes' taking one of their two profiles; it waits for nothing of its sound
//! id but at a death; it gives way to the creature's own line still playing but at a hit, which
//! stops that line, and at a death; it records the wait of its id. One line a creature.
//!
//! The hero speaks lines of its own too, MP3 samples by name (`0x693FE0`, #713): a battle cry at a
//! combat art, one in three; its death's line; and its comments in the fight on one clock, which
//! each comment that plays sets back to 0 (`+0xB7E8`, #1175; docs/re/sound.md, "The hero's
//! comments"): a taunt at a boss's or a champion's hit while the hero's own grunt still sounds,
//! its kill and its hit, the hero hit near its death, and a foe too weak or too strong for it.
//!
//! The line plays at the volume of its place, lowered with the distance from the hero and panned
//! by its side as the original places it (`sfx::placed`, `0x678260`), and at a rate of its
//! sample's times 0.45 + (`rand() % 10`) / 90 + 0.5, 0.95 to 1.05 (`0x693510`, #1140).
//!
//! Between the fights the creatures make their idle noises (#714; docs/re/sound.md, "The idle
//! noises"), as the sound manager's update does (`0x69B770`) at each world tick: a creature near
//! the hero that is not hostile to it rolls one in 70, one hostile to it one in 20 while it does
//! not fight, then its type's noise rolls its own (`tables/idle_noises.ron`) and plays a variant
//! of its profile's group 0 or one of its run of sound ids, unless the creature speaks or the id
//! waits; the id then waits the noise's seconds, and the rate's factor adds the noise's own.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use bevy::log::debug;
use bevy::prelude::{
    Added, App, AssetServer, Assets, Commands, Component, Entity, GlobalTransform, Handle, Has,
    IntoScheduleConfigs, Message, MessageReader, MessageWriter, Mut, Plugin, Query, Res, ResMut,
    Resource, Time, Update, Vec3, With, Without, World,
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

/// The animals' types, which the comments on a kill, a boss and a foe's strength leave out
/// (`0x687656`).
const ANIMALS: std::ops::RangeInclusive<u32> = 512..=596;

/// How near a foe stands that the hero names as it attacks (`ENEMYTYPE`, the record's `+0x14`),
/// in the place's pixels.
const NAMES_WITHIN: i32 = 149;

/// The kinds of foe that the hero names as it attacks (`ENEMYTYPE`, `0x686254`–`0x686CFE`), its
/// lines `SOUND_FX_ENEMYTYPE_<KIND>nn_<CLASS>`: the orcs and the thieves for every class, the
/// others for the Dwarf and the Daemon alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Foe {
    Orc,
    Thief,
    DarkElf,
    Demon,
    Demordrey,
    Dragon,
    Giant,
    Mage,
    Monster,
    Undead,
    Goblin,
}

impl Foe {
    fn name(self) -> &'static str {
        match self {
            Self::Orc => "ORC",
            Self::Thief => "THIEF",
            Self::DarkElf => "DARKELF",
            Self::Demon => "DEMON",
            Self::Demordrey => "DEMORDREY",
            Self::Dragon => "DRAGON",
            Self::Giant => "GIANT",
            Self::Mage => "MAGE",
            Self::Monster => "MONSTER",
            Self::Undead => "UNDEAD",
            Self::Goblin => "GOBLIN",
        }
    }
}

/// What a type's name makes of a creature that the hero may name: a kind of foe, or the search's
/// end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Named {
    Foe(Foe),
    Ends,
}

/// The parts of the monsters' types' names, each after `TYPE_NPC_`.
const MONSTERS: [&str; 13] = [
    "GHUL",
    "MINOTAUR",
    "GARGOYLE",
    "TATZELANNOY",
    "TROLL",
    "HARPIEN",
    "FLIGHT_L",
    "FEUER",
    "GLORBFRESSER",
    "FETZER",
    "BLUTSCHRECKE",
    "STACHERALB",
    "SCHLEIMWURM",
];

/// What the type's name `name` makes of a creature for `ENEMYTYPE` (`0x686254`, the names of
/// `0x43CED0` searched for parts, in this order): a soldier of none of Demordrey's types is a
/// deserter only by a shown name that holds the German `desertiert`, which the English text never
/// does (**hypothesis**), and the search ends at it with no line.
fn named(name: &str) -> Option<Named> {
    let has = |parts: &[&str]| parts.iter().any(|part| name.contains(part));
    let foe = if has(&["TYPE_NPC_ORC_"]) {
        Foe::Orc
    } else if has(&["TYPE_NPC_THIEF_"]) {
        Foe::Thief
    } else if has(&["TYPE_NPC_DARKELVE"]) {
        Foe::DarkElf
    } else if has(&[
        "BIG_DEMON",
        "FLIGHT_DEMON",
        "GARGOYLE_LESSERDEMON",
        "ANDUCAR_DEMON",
        "SPIDER_M2_DEMON",
        "GIGANT_SPIDER_DEMON",
    ]) {
        Foe::Demon
    } else if has(&[
        "SOLDIER01",
        "SOLDIER02",
        "SOLDIER03",
        "SOLDIER04",
        "SOLDIER05",
        "SOLDIER14",
        "SOLDIER23",
    ]) {
        Foe::Demordrey
    } else if has(&["TYPE_NPC_SOLDIER"]) {
        return Some(Named::Ends);
    } else if has(&["TYPE_NPC_DRAGON"]) {
        Foe::Dragon
    } else if name == "TYPE_NPC_RIESE" || has(&["TYPE_NPC_BAUMBART"]) {
        Foe::Giant
    } else if has(&["TYPE_NPC_BLACK_MAGICIAN"]) {
        Foe::Mage
    } else if MONSTERS
        .iter()
        .any(|part| name.contains(&format!("TYPE_NPC_{part}")))
    {
        Foe::Monster
    } else if has(&["GOBLIN_UNDEAD", "GOBLIN_MAGIC_UNDEAD", "UPIRATE", "ZOMBIE"]) {
        Foe::Undead
    } else if has(&["TYPE_NPC_GOBLIN"]) {
        Foe::Goblin
    } else {
        return None;
    };
    Some(Named::Foe(foe))
}

/// The creatures' types that the hero may name as it attacks ([`named`]), by the types' names of
/// a new game's tables.
#[derive(Resource, Default)]
struct Names(HashMap<u32, Named>);

impl Names {
    fn of(names: &[opensacred_data::tables::TypeName]) -> Self {
        Self(
            names
                .iter()
                .filter_map(|n| named(&n.name).map(|named| (n.kind, named)))
                .collect(),
        )
    }
}

/// The kind of the first foe within [`NAMES_WITHIN`] of the hero at `at`, of the creatures' places
/// and types `foes`, the nearest first, whose type names one that the hero of type `creature` has
/// lines for (`0x68616B`; **hypothesis** for the order, the sound manager's list's): one of a kind
/// without lines passes to the next, a soldier of none of Demordrey's types ends the search.
fn foe_near(
    (at, creature): (Vec3, u32),
    names: &Names,
    foes: impl Iterator<Item = (Vec3, u32)>,
    sfx: &Sfx,
) -> Option<Foe> {
    let mut near: Vec<(i32, u32)> = foes
        .map(|(place, kind)| (sfx::screen_distance(place, at), kind))
        .filter(|&(distance, _)| distance <= NAMES_WITHIN)
        .collect();
    near.sort_by_key(|&(distance, _)| distance);
    for (_, kind) in near {
        match names.0.get(&kind) {
            Some(Named::Ends) => return None,
            Some(&Named::Foe(foe))
                if HeroLine::EnemyType(foe)
                    .name(creature, 0)
                    .is_some_and(|name| sfx.named(&name).is_some()) =>
            {
                return Some(foe);
            }
            _ => {}
        }
    }
    None
}

/// The names of a new game's types.
fn names(mut commands: Commands, data: Option<Res<crate::game::Data>>) {
    let names = data
        .as_ref()
        .and_then(|data| data.0.tables.type_names().ok());
    commands.insert_resource(names.map_or_else(Names::default, Names::of));
}

/// The idle noises' rolls at each world tick (`0x69B770`): one in 20 for a creature hostile to the
/// hero (the update's first list), one in 70 for the others (the second).
const HOSTILE_IDLE: u8 = 20;
const PEACEFUL_IDLE: u8 = 70;
/// The profiles' group of the idle noises.
const IDLE: u8 = 0;
/// The Gladiator's creature type, whose battle cry's roll is one in two (`0x4FD0F0`).
const GLADIATOR: u32 = 2;
/// The hero's cries at a critical hit (`SOUND_FX_HERO_CRITICALHIT01`, `02`).
const HERO_CRIES: [u32; 2] = [16301, 16302];

/// The hero makes a combat art: its battle cry, one in three, one in two for the Gladiator, or
/// surely after the art's own cry (`0x4FD0F0`, `art_sounds`).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BattleCry {
    pub hero: Entity,
    pub sure: bool,
}

/// The hero's lines by name (`0x693FE0`): the name's stem, how many a class has, and the roll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HeroLine {
    HitEnemy,
    KillEnemy,
    BossMob,
    EnemyHitMe,
    EnemyTooWeak,
    EnemyTooStrong,
    /// The kind of the foe the hero attacks.
    EnemyType(Foe),
    /// A battle cry, `sure` after an art's own cry, `night` in the Vampiress's night's form.
    BattleCry {
        sure: bool,
        night: bool,
    },
    EnemyKillMe,
    /// The hero's breath (`0x68BEA0`), by its sound id rather than as a line by name.
    Breathing,
    /// The foes on the run (`0x68BEA0`).
    FoeOnTheRun,
    EnemyFlees,
}

impl HeroLine {
    fn stem(self) -> &'static str {
        match self {
            Self::HitEnemy => "HITENEMY",
            Self::KillEnemy => "KILLENEMY",
            Self::BossMob => "BOSSMOB",
            Self::EnemyHitMe => "ENEMYHITME",
            Self::EnemyTooWeak => "ENEMYTOOWEAK",
            Self::EnemyTooStrong => "ENEMYTOOSTRONG",
            Self::EnemyType(_) => "ENEMYTYPE",
            Self::BattleCry { .. } => "BATTLECRY",
            Self::EnemyKillMe => "ENEMYKILLME",
            Self::Breathing => "BREATHING",
            Self::FoeOnTheRun => "FOE_ON_THE_RUN",
            Self::EnemyFlees => "ENEMYFLEES",
        }
    }

    fn count(self) -> u32 {
        match self {
            Self::HitEnemy => 11,
            Self::KillEnemy => 14,
            Self::BossMob | Self::EnemyHitMe | Self::EnemyTooWeak | Self::EnemyTooStrong => 6,
            Self::EnemyType(_) => 5,
            Self::BattleCry { .. } => 10,
            Self::EnemyKillMe => 6,
            Self::Breathing | Self::FoeOnTheRun => 10,
            Self::EnemyFlees => 5,
        }
    }

    /// The roll of the line of the hero of type `creature`: one in this many.
    fn roll(self, creature: u32) -> u8 {
        match self {
            Self::HitEnemy => 5,
            Self::KillEnemy | Self::BossMob => 3,
            Self::EnemyHitMe => 4,
            Self::EnemyType(_) => 8,
            Self::EnemyTooWeak | Self::EnemyTooStrong => 20,
            Self::BattleCry { sure: true, .. } => 1,
            Self::BattleCry { .. } if creature == GLADIATOR => 2,
            Self::BattleCry { .. } => 3,
            Self::EnemyKillMe | Self::Breathing | Self::FoeOnTheRun | Self::EnemyFlees => 1,
        }
    }

    /// The sound's name of the line `n` of the hero of type `creature`: the Vampiress's battle
    /// cries in her night's form her own (the event `(9, 0x49)` sets the manager's `+0xAF48`).
    fn name(self, creature: u32, n: u32) -> Option<String> {
        let class = match creature {
            6 | 7 if matches!(self, Self::BattleCry { night: true, .. }) => "REALVAMP",
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
        Some(match self {
            Self::EnemyType(foe) => {
                format!(
                    "SOUND_FX_{}_{}{:02}_{class}",
                    self.stem(),
                    foe.name(),
                    n + 1
                )
            }
            Self::Breathing | Self::FoeOnTheRun | Self::EnemyFlees => {
                format!("SOUND_FX_{}{:02}_{class}", self.stem(), n + 1)
            }
            _ => format!("SOUND_FX_{}_{:02}_{class}", self.stem(), n + 1),
        })
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

/// The line a creature speaks, while it plays, and whether it is a hero's line by name (the
/// original's MP3 lines, `0x693FE0`).
#[derive(Component)]
struct Speaking(Handle<AudioInstance>, bool);

/// The line by name that plays: the sound manager's one at a time (`0x693FE0`, `+0xB7E0`), which
/// the hero's lines by name and the dialogues' voice-overs (`talk`) share, so that each stops the
/// one before, and which `0x693FB0` stops (#1238).
#[derive(Resource, Default)]
pub struct NamedLine(Option<Handle<AudioInstance>>);

impl NamedLine {
    /// Whether a line by name plays.
    fn plays(&self, audio: &Audio) -> bool {
        self.0.as_ref().is_some_and(|line| {
            matches!(
                audio.state(line),
                PlaybackState::Playing { .. } | PlaybackState::Queued
            )
        })
    }

    /// `line` now the line by name, the one before stopped; with none, only the stop
    /// (`0x693FB0`). The line stopped, if one was.
    fn name(
        &mut self,
        instances: &mut Assets<AudioInstance>,
        line: Option<Handle<AudioInstance>>,
    ) -> Option<Handle<AudioInstance>> {
        let old = std::mem::replace(&mut self.0, line)?;
        if let Some(mut instance) = instances.get_mut(&old) {
            debug!("voices: the line by name stops");
            instance.stop(AudioTween::default());
        }
        Some(old)
    }
}

/// [`NamedLine::name`] in `world`: `line`, a dialogue's voice-over, now the line by name, or the
/// line by name stopped (`talk`).
pub fn name_line(world: &mut World, line: Option<Handle<AudioInstance>>) {
    if !world.contains_resource::<NamedLine>() {
        return;
    }
    world.resource_scope(|world, mut named: Mut<NamedLine>| {
        if let Some(mut instances) = world.get_resource_mut::<Assets<AudioInstance>>() {
            named.name(&mut instances, line);
        }
    });
}

/// The voices of the types, the waits of the sound ids that played, and the dice.
#[derive(Resource)]
pub struct Voices {
    voices: HashMap<u32, Voice>,
    /// Until when, in the game's seconds, a sound id waits.
    waits: HashMap<u32, f64>,
    /// When the hero last made a comment (`+0xB7E8` counts the seconds since).
    commented: Option<f64>,
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
            commented: None,
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
    /// its wait and what it adds to the rate's factor.
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
        Some((id, file, noise.wait, noise.rate))
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

    /// The hero's `line`, of the hero of type `creature`, if its roll gives one: one of the
    /// class's at random.
    fn hero_line(&mut self, sfx: &Sfx, creature: u32, line: HeroLine) -> Option<(u32, String)> {
        if !rolls(line.roll(creature), &mut self.dice) {
            return None;
        }
        let name = line.name(creature, self.dice.below(line.count()))?;
        sfx.named(&name)
    }

    /// The comment that `heard` makes the hero say (`0x6852B0`), its line's sound and file, at
    /// `now`: the clock set back to 0 when one comes, even one that names no line.
    fn comment(
        &mut self,
        sfx: &Sfx,
        creature: u32,
        (heard, hero): (Heard, HeroState),
        now: f64,
    ) -> Option<(u32, String)> {
        let clock = self.commented.map_or(f64::INFINITY, |at| now - at);
        let (line, n) = comment(heard, hero, clock, &mut self.dice)?;
        self.commented = Some(now);
        let name = line.name(creature, n)?;
        // The sixth line that a class lacks is its fifth again (by its id, `0x689199`).
        sfx.named(&name).or_else(|| {
            (n == 5 && matches!(line, HeroLine::EnemyTooWeak | HeroLine::EnemyTooStrong))
                .then(|| line.name(creature, 4))
                .flatten()
                .and_then(|name| sfx.named(&name))
        })
    }
}

/// A line of the fight as the hero's comments hear it: its group, whether the hero speaks it,
/// and its speaker's type, whether it is a boss or a champion (the sound manager's record's flag
/// 4) and its level; at the hero's attack, the kind of the foe near it that it has lines for.
#[derive(Clone, Copy, Debug)]
struct Heard {
    group: u8,
    hero: bool,
    kind: u32,
    marked: bool,
    level: u32,
    foe: Option<Foe>,
}

/// The hero as its comments look at it: its level, its health in per cent, and whether its own
/// grunt of its profile still sounds (the record's `+0x34`).
#[derive(Clone, Copy, Debug)]
struct HeroState {
    level: u32,
    health: u32,
    grunts: bool,
}

/// The comment the line `heard` makes, the clock `clock` seconds on, by the rolls of `dice`
/// (`0x6852B0`, its blocks in order, at most one): a taunt (`HITENEMY`) at a boss's or a
/// champion's hit while the hero's grunt sounds, more than 10 s on, one in five; the kind of the
/// foe near the hero as it attacks (`ENEMYTYPE`), more than 20 s on, one in eight; the hero's own
/// hit near its death (`ENEMYHITME`), more than 20 s on, one in four, silent; a boss's or a
/// champion's death (`KILLENEMY`, 14 lines of 15 rolls) or hit (`BOSSMOB`, a second roll of one in
/// four past 30 s), more than 10 s on, one in three, the hero silent and the speaker no animal; a
/// foe hit two levels or more below the hero at 71 % of its health or more (`ENEMYTOOWEAK`), or
/// one striking two levels or more above it at 29 % or less (`ENEMYTOOSTRONG`), more than 15 s on,
/// one in 20, silent. The line and its number, `None` for no comment; a number past the class's
/// lines names none, and still sets the clock back.
fn comment(heard: Heard, hero: HeroState, clock: f64, dice: &mut Rng) -> Option<(HeroLine, u32)> {
    let hit = matches!(heard.group, NORMAL_HIT | CRITICAL_HIT);
    if !heard.hero && hit && heard.marked && hero.grunts && clock > 10.0 && dice.below(5) == 0 {
        return Some((HeroLine::HitEnemy, dice.below(11)));
    }
    if heard.hero
        && heard.group == ATTACK
        && clock > 20.0
        && dice.below(8) == 0
        && let Some(foe) = heard.foe
    {
        return Some((HeroLine::EnemyType(foe), dice.below(5)));
    }
    if heard.hero
        && heard.group == CRITICAL_HIT
        && clock > 20.0
        && dice.below(4) == 0
        && !hero.grunts
    {
        return Some((HeroLine::EnemyHitMe, dice.below(6)));
    }
    let foe = !heard.hero && !ANIMALS.contains(&heard.kind) && !hero.grunts;
    if foe && heard.marked && (heard.group == DEATH || hit) && clock > 10.0 && dice.below(3) == 0 {
        if heard.group == DEATH {
            // One roll of 15 of 14 lines, the clock set back even by the last.
            let n = dice.below(15);
            return Some((HeroLine::KillEnemy, n));
        }
        if clock > 30.0 && dice.below(4) == 0 {
            return Some((HeroLine::BossMob, dice.below(6)));
        }
    }
    if foe && clock > 15.0 {
        let weak = hit && hero.level >= heard.level + 2 && hero.health >= 71;
        let strong = heard.group == ATTACK && heard.level >= hero.level + 2 && hero.health <= 29;
        if weak && dice.below(20) == 0 {
            return Some((HeroLine::EnemyTooWeak, dice.below(6)));
        }
        if strong && dice.below(20) == 0 {
            return Some((HeroLine::EnemyTooStrong, dice.below(6)));
        }
    }
    None
}

/// A line to play: who speaks it, its sound id and file, the wait it records, and what it does
/// to the speaker's own line.
struct Said {
    speaker: Entity,
    id: u32,
    file: String,
    wait: Option<f32>,
    play: Play,
    named: bool,
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
    (hero, forms, mut sounds, heard): (
        Option<Res<hero::Hero>>,
        Option<Res<crate::forms::Forms>>,
        MessageWriter<sfx::Play>,
        Res<sfx::Listener>,
    ),
    speakers: Query<(
        &GlobalTransform,
        Option<&crate::ai::Mind>,
        Has<Body>,
        Option<&Speaking>,
        (Option<&crate::fight::Health>, Option<&crate::fight::Ranked>),
    )>,
    heroes: Query<(Entity, &GlobalTransform), With<Body>>,
    assets: Res<AssetServer>,
    audio: Res<Audio>,
    mut instances: ResMut<Assets<AudioInstance>>,
    mut named_line: ResMut<NamedLine>,
    time: Res<Time>,
    (names, foes): (
        Option<Res<Names>>,
        Query<(&GlobalTransform, &crate::ai::Mind), (Without<Body>, Without<Dies>)>,
    ),
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
    // The hero as its comments look at it: its level, its health in per cent and whether its
    // own grunt of its profile still sounds.
    let percent = |health: Option<&crate::fight::Health>| {
        health.map_or(0, |h| {
            u32::try_from(100 * u64::from(h.current) / u64::from(h.most.max(1))).unwrap_or(0)
        })
    };
    let hero_state = the_hero.and_then(|(hero, _)| speakers.get(hero).ok()).map(
        |(_, _, _, speaking, (health, _))| HeroState {
            level: health.map_or(1, |h| h.level),
            health: percent(health),
            grunts: speaking.is_some_and(|s| !s.1 && playing(Some(s))),
        },
    );
    let mut said = Vec::new();
    for line in lines.read() {
        let Ok((_, mind, is_hero, speaking, (health, ranked))) = speakers.get(line.speaker) else {
            continue;
        };
        if is_hero && line.group == DEATH {
            // The hero's death speaks its line by name (`0x6852B0`, `0x693FE0`).
            if let Some((id, file)) = hero_type
                .and_then(|creature| voices.hero_line(&sfx, creature, HeroLine::EnemyKillMe))
            {
                said.push(Said {
                    speaker: line.speaker,
                    id,
                    file,
                    wait: None,
                    play: Play::Interrupts,
                    named: true,
                });
            }
            continue;
        }
        // The hero's comments on the line, which stop its line by name that plays (`0x6852B0`,
        // `0x693FE0`).
        let heard = Heard {
            group: line.group,
            hero: is_hero,
            kind: mind.map_or(0, |mind| mind.thinks.creature),
            marked: ranked.is_some_and(|r| {
                matches!(
                    r.0,
                    opensacred_rules::fight::Rank::Boss | opensacred_rules::fight::Rank::Champion
                )
            }),
            level: health.map_or(1, |h| h.level),
            // The foes hostile to the hero about it as it attacks (`0x68616B`).
            foe: listener
                .zip(hero_type)
                .zip(names.as_deref())
                .filter(|_| is_hero && line.group == ATTACK)
                .and_then(|(hero, names)| {
                    let near = foes
                        .iter()
                        .filter(|(_, mind)| mind.hostile())
                        .map(|(at, mind)| (at.translation(), mind.thinks.creature));
                    foe_near(hero, names, near, &sfx)
                }),
        };
        if let (Some((hero, _)), Some(creature), Some(state)) = (the_hero, hero_type, hero_state)
            && let Some((id, file)) = voices.comment(&sfx, creature, (heard, state), now)
        {
            debug!("voices: the hero comments with {id}");
            said.push(Said {
                speaker: hero,
                id,
                file,
                wait: None,
                play: Play::Interrupts,
                named: true,
            });
        }
        // The hero's cry at a critical hit, at the voice's rate's factor, before its line's own
        // test, which stops the line by name that plays, its own or a dialogue's (`0x6852B0` at
        // `0x686F4F`, `0x693FB0`).
        let mut speaking = speaking;
        let mut cried = false;
        if is_hero && line.group == CRITICAL_HIT {
            let cry = HERO_CRIES[usize::try_from(sfx.dice.below(2)).unwrap_or(0)];
            sounds.write(sfx::Play {
                sound: sfx::Of::Id(cry),
                at: Some(line.speaker),
                rate: (0.45, sfx::Roll::Tenths),
            });
            if named_line.plays(&audio) {
                let stopped = named_line.name(&mut instances, None);
                if speaking.is_some_and(|s| s.1 && stopped.as_ref() == Some(&s.0)) {
                    speaking = None;
                    cried = true;
                }
            }
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
        let play = match play(line.group, playing(speaking), waiting) {
            // The line by name that the cry stopped gives way.
            Play::Plays if cried => Play::Interrupts,
            play => play,
        };
        if play != Play::GivesWay {
            said.push(Said {
                speaker: line.speaker,
                id,
                file,
                wait: Some(wait),
                play,
                named: false,
            });
        }
    }
    for cry in cries.read() {
        if let Some((id, file)) = hero_type.and_then(|creature| {
            let night = forms.as_ref().is_some_and(|forms| forms.by_night());
            let line = HeroLine::BattleCry {
                sure: cry.sure,
                night,
            };
            voices.hero_line(&sfx, creature, line)
        }) {
            said.push(Said {
                speaker: cry.hero,
                id,
                file,
                wait: None,
                play: Play::Plays,
                named: true,
            });
        }
    }
    for line in said {
        let Ok((at, _, _, speaking, _)) = speakers.get(line.speaker) else {
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
        let rate = rate(sfx.roll(10), 0.0);
        let (volume, pan) = sfx::placed_at(at.translation(), listener, *heard);
        if volume <= 0.0 {
            continue;
        }
        let sample = sfx.sample(line.id, line.file, &assets);
        let instance = audio
            .play(sample)
            .with_volume(sfx::decibels(volume))
            .with_panning(pan)
            .with_playback_rate(f64::from(rate))
            .handle();
        if let Some(wait) = line.wait {
            voices.waits.insert(line.id, now + f64::from(wait));
        }
        debug!(
            "voices: {} speaks {}, at {volume:.2}, panned {pan:.2}, at a rate of {rate:.2}",
            line.speaker, line.id
        );
        // A line by name stops the one that plays, a dialogue's voice-over too.
        if line.named {
            named_line.name(&mut instances, Some(instance.clone()));
        }
        commands
            .entity(line.speaker)
            .insert(Speaking(instance, line.named));
    }
}

/// The rate of a line or an idle noise, of its sample's: `playSFX`'s factor, 0.45 + `roll` / 90
/// for a `roll` of `rand() % 10` plus the noise's own `higher`, and 0.5 (`0x693510`; docs/re/
/// sound.md, #1140).
fn rate(roll: f32, higher: f32) -> f32 {
    0.45 + roll / 90.0 + higher + 0.5
}

/// A creature's motions over its last 16 updates (`0x679040`), by its motion's class: the stand,
/// the walk, the run and the strike (`MOTION_IDLE` 0, `WALK` 1, `RUN` 2, `ATTACK` 4), and where it
/// stood at the last.
#[derive(Component, Default)]
struct Motions {
    classes: [u8; 16],
    at: usize,
    last: Option<Vec3>,
    /// Its move since the last update, along the ground.
    moved: Vec3,
}

/// The motions' classes (`0x679040`).
const STANDS: u8 = 0;
const WALKS: u8 = 1;
const RUNS: u8 = 2;
const STRIKES: u8 = 4;

impl Motions {
    /// Keeps the class of this update and the move from the last.
    fn push(&mut self, class: u8, at: Vec3) {
        self.classes[self.at] = class;
        self.at = (self.at + 1) % self.classes.len();
        self.moved = self.last.map_or(Vec3::ZERO, |last| (at - last).with_y(0.0));
        self.last = Some(at);
    }

    /// The share of the class `class`, in per cent of the 16 updates.
    fn share(&self, class: u8) -> usize {
        100 * self.classes.iter().filter(|&&c| c == class).count() / self.classes.len()
    }
}

/// How near the foes stand that the hero counts in its fight, and that it sees flee, in the
/// place's pixels (`0x68BEA0`).
const ENGAGED_WITHIN: i32 = 450;
const FLEES_WITHIN: i32 = 500;

/// The hero's fight as its comments on the foes on the run see it (`0x68BEA0`): its motions, the
/// counts of the foes engaged over the last five updates (`+0xD96`, `+0xD98`).
#[derive(Default)]
struct Flight {
    hero: Motions,
    engaged: [usize; 5],
    at: usize,
}

/// Whether a creature of `motions` engages in the fight (`0x68BEA0`): a strike's share above 20,
/// a run's above 30 or a walk's above 50.
fn engages(motions: &Motions) -> bool {
    motions.share(STRIKES) > 20 || motions.share(RUNS) > 30 || motions.share(WALKS) > 50
}

/// Whether a creature of `motions` at `at` flees from the hero at `hero` of `hero_motions` (the
/// record's state 5): it neither strikes (a share below 31) nor casts, walks or runs (a share of
/// 51 or more), moves away from the hero, its move's direction within 0.2 of the hero's to it,
/// nearer than [`FLEES_WITHIN`], while the hero runs less than 71 % of the time.
fn flees(motions: &Motions, at: Vec3, (hero, hero_motions): (Vec3, &Motions)) -> bool {
    let away = (at - hero).with_y(0.0).normalize_or_zero();
    let moving = motions.moved.normalize_or_zero();
    motions.share(STRIKES) < 31
        && (motions.share(WALKS) >= 51 || motions.share(RUNS) >= 51)
        && hero_motions.share(RUNS) < 71
        && moving != Vec3::ZERO
        && (moving - away).length() <= 0.2
        && sfx::screen_distance(at, hero) < FLEES_WITHIN
}

/// The hero sees its foes run (`0x68BEA0`, `FOE_ON_THE_RUN` and `ENEMYFLEES`): at each update the
/// motions of the creatures hostile to it are kept, the bosses and the champions and those that
/// engage counted (those nearer than 450 of a level at least the hero's less 2 as engaged); where
/// more of them flee than 0.6 of the most engaged over the last five updates, more than 20 s
/// after the hero's last comment, one in five, `rand() % 15` names one of the ten
/// `FOE_ON_THE_RUN` or the five `ENEMYFLEES` of the hero's class (none for the Dwarf and the
/// Daemon), which plays at the hero while no line by name plays and its sound id does not wait,
/// sets the comments' clock back and waits 60 s.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn flight(
    mut commands: Commands,
    mut state: bevy::prelude::Local<Flight>,
    (hero, mut voices, mut sfx, heard): (
        Option<Res<hero::Hero>>,
        ResMut<Voices>,
        ResMut<Sfx>,
        Res<sfx::Listener>,
    ),
    heroes: Query<
        (
            Entity,
            &Body,
            &GlobalTransform,
            &crate::fight::Health,
            Has<crate::attack::Striking>,
        ),
        Without<Dies>,
    >,
    mut creatures: Query<
        (
            Entity,
            &GlobalTransform,
            &crate::ai::Mind,
            &crate::fight::Health,
            Option<&crate::fight::Ranked>,
            (Option<&crate::npcs::Walk>, Has<crate::npcs::Strikes>),
            Option<&mut Motions>,
        ),
        (Without<Body>, Without<Dies>),
    >,
    (assets, audio, mut named_line): (Res<AssetServer>, Res<Audio>, ResMut<NamedLine>),
    time: Res<Time>,
) {
    let (Some(hero), Ok((entity, body, at, health, striking))) = (hero, heroes.single()) else {
        return;
    };
    let now = time.elapsed_secs_f64();
    let hero_at = at.translation();
    let class = match (striking, body.moving()) {
        (true, _) => STRIKES,
        (false, Some(hero::Gait::Walk)) => WALKS,
        (false, Some(hero::Gait::Run)) => RUNS,
        (false, None) => STANDS,
    };
    state.hero.push(class, hero_at);
    let (mut engaged, mut fleeing) = (0, 0);
    for (creature, at, mind, of, ranked, (walk, strikes), motions) in &mut creatures {
        if !mind.hostile() {
            continue;
        }
        let place = at.translation();
        let class = match walk {
            _ if strikes => STRIKES,
            Some(walk) if walk.runs() => RUNS,
            Some(_) => WALKS,
            None => STANDS,
        };
        let Some(mut motions) = motions else {
            if sfx::screen_distance(place, hero_at) < 2 * FLEES_WITHIN {
                let mut motions = Motions::default();
                motions.push(class, place);
                commands.entity(creature).insert(motions);
            }
            continue;
        };
        motions.push(class, place);
        let marked = ranked.is_some_and(|r| {
            matches!(
                r.0,
                opensacred_rules::fight::Rank::Boss | opensacred_rules::fight::Rank::Champion
            )
        });
        if !marked && !engages(&motions) {
            continue;
        }
        if sfx::screen_distance(place, hero_at) < ENGAGED_WITHIN && of.level + 2 >= health.level {
            engaged += 1;
        }
        if flees(&motions, place, (hero_at, &state.hero)) {
            fleeing += 1;
        }
    }
    let at_now = state.at;
    state.engaged[at_now] = engaged;
    state.at = (at_now + 1) % state.engaged.len();
    let most = state.engaged.iter().copied().max().unwrap_or(0);
    let clock = voices.commented.map_or(f64::INFINITY, |at| now - at);
    // More flee than 0.6 of the most engaged: `fleeing > 0.6·most`, in whole numbers.
    if fleeing == 0 || 10 * fleeing <= 6 * most || clock <= 20.0 || voices.dice.below(5) != 0 {
        return;
    }
    let n = voices.dice.below(15);
    let (line, n) = if n < 10 {
        (HeroLine::FoeOnTheRun, n)
    } else {
        (HeroLine::EnemyFlees, n - 10)
    };
    if matches!(hero.creature, 8 | 9) {
        return;
    }
    let named = named_line.plays(&audio);
    let Some((id, file)) = line
        .name(hero.creature, n)
        .and_then(|name| sfx.named(&name))
    else {
        return;
    };
    if named || voices.waits.get(&id).is_some_and(|&until| until > now) {
        return;
    }
    let (volume, pan) = sfx::placed_at(hero_at, Some(hero_at), *heard);
    if volume <= 0.0 {
        return;
    }
    debug!("voices: the hero sees {fleeing} of {most} foes flee, {id}");
    voices.commented = Some(now);
    voices.waits.insert(id, now + 60.0);
    let rate = rate(sfx.roll(10), 0.0);
    let sample = sfx.sample(id, file, &assets);
    let instance = audio
        .play(sample)
        .with_volume(sfx::decibels(volume))
        .with_panning(pan)
        .with_playback_rate(f64::from(rate))
        .handle();
    // No line by name plays: the slot only takes it.
    named_line.0 = Some(instance.clone());
    commands.entity(entity).insert(Speaking(instance, true));
}

/// How long the hero walks or runs before it breathes hard, and stands before it breathes, in
/// seconds; and how long it rests from a breath that ended (`0x68BEA0`).
const BREATHES_MOVING: f32 = 20.0;
const BREATHES_STANDING: f32 = 15.0;
const BREATH_RESTS: f32 = 10.0;

/// The hero's breath (`0x68BEA0`, `BREATHING`): the seconds it has walked or run (`+0xD70`) and
/// stood (`+0xD80`) without a break, the rest still to run (`+0xD78`), and the breath playing
/// (`+0xD94`) with the times it may still play, for the hero it keeps.
#[derive(Default)]
struct Breath {
    hero: Option<Entity>,
    moved: f32,
    stood: f32,
    rests: f32,
    playing: Option<(Handle<AudioInstance>, u8, (u32, String))>,
}

/// The number of the breath of the hero of type `creature` for its gait, by the roll `n` of
/// `rand() % 5`: walking 1 to 5, running 6 to 10, the Vampiress's 8, 9, 8, 9 and 10, standing 1
/// to 5.
fn breath(creature: u32, gait: Option<hero::Gait>, n: u32) -> u32 {
    match gait {
        Some(hero::Gait::Run) if matches!(creature, 6 | 7) => {
            [8, 9, 8, 9, 10][usize::try_from(n % 5).unwrap_or(0)]
        }
        Some(hero::Gait::Run) => 6 + n % 5,
        _ => 1 + n % 5,
    }
}

/// The hero breathes (`0x68BEA0`): after more than 20 s of walking or running without a break,
/// rested, a breath of its gait twice in a row, but once only if it stops; after more than 15 s
/// of standing without a break, a breath of the walk once; at the hero, at a rate of 0.9 to 1.1
/// of its sample, by its sound id (`SOUND_FX_BREATHINGnn_<CLASS>`). A breath that ends rests the
/// next 10 s; a hero's line by name stops it at once, without the rest. A strike or an art's
/// action breaks both runs, as the original's motions' shares of their last 16 updates do
/// (**hypothesis** for their span, a frame each).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn breathe(
    mut breath_of: bevy::prelude::Local<Breath>,
    hero: Option<Res<hero::Hero>>,
    heroes: Query<
        (
            Entity,
            &Body,
            &GlobalTransform,
            Has<crate::attack::Striking>,
            Has<crate::making::Playing>,
        ),
        Without<Dies>,
    >,
    (mut sfx, named_line): (ResMut<Sfx>, Res<NamedLine>),
    heard: Res<sfx::Listener>,
    (assets, audio, mut instances): (Res<AssetServer>, Res<Audio>, ResMut<Assets<AudioInstance>>),
    time: Res<Time>,
) {
    let (Some(hero), Ok((entity, body, at, striking, acting))) = (hero, heroes.single()) else {
        return;
    };
    let state = &mut *breath_of;
    if state.hero != Some(entity) {
        *state = Breath {
            hero: Some(entity),
            ..Breath::default()
        };
    }
    let dt = time.delta_secs();
    let gait = body.moving();
    let busy = striking || acting;
    state.moved = if gait.is_some() && !busy {
        state.moved + dt
    } else {
        0.0
    };
    state.stood = if gait.is_none() && !busy {
        state.stood + dt
    } else {
        0.0
    };
    state.rests -= dt;
    let named = named_line.plays(&audio);
    if let Some((handle, plays, sound)) = state.playing.take() {
        if named {
            if let Some(mut instance) = instances.get_mut(&handle) {
                instance.stop(AudioTween::default());
            }
            return;
        }
        if matches!(
            audio.state(&handle),
            PlaybackState::Playing { .. } | PlaybackState::Queued
        ) {
            state.playing = Some((handle, plays, sound));
            return;
        }
        // Looped twice while the hero moves on, once when it stops.
        if plays > 1 && gait.is_some() {
            let again = play_breath(
                &mut sfx,
                sound.clone(),
                at.translation(),
                *heard,
                (&assets, &audio),
            );
            state.playing = again.map(|handle| (handle, plays - 1, sound));
            return;
        }
        state.rests = BREATH_RESTS;
        return;
    }
    if named {
        return;
    }
    let (number, plays) = if state.moved > BREATHES_MOVING && state.rests <= 0.0 {
        (breath(hero.creature, gait, sfx.dice.below(5)), 2)
    } else if state.stood > BREATHES_STANDING {
        state.stood = 0.0;
        (breath(hero.creature, None, sfx.dice.below(5)), 1)
    } else {
        return;
    };
    let Some(sound) = HeroLine::Breathing
        .name(hero.creature, number - 1)
        .and_then(|name| sfx.named(&name))
    else {
        return;
    };
    debug!("voices: the hero breathes {}", sound.0);
    let handle = play_breath(
        &mut sfx,
        sound.clone(),
        at.translation(),
        *heard,
        (&assets, &audio),
    );
    state.playing = handle.map(|handle| (handle, plays, sound));
}

/// Plays the breath `sound` at the hero at `at`, at a rate's factor of 0.4 to 0.6 of its sample's
/// (`playSFX`'s, and 0.5).
fn play_breath(
    sfx: &mut Sfx,
    (id, file): (u32, String),
    at: Vec3,
    heard: sfx::Listener,
    (assets, audio): (&AssetServer, &Audio),
) -> Option<Handle<AudioInstance>> {
    let (volume, pan) = sfx::placed_at(at, Some(at), heard);
    if volume <= 0.0 {
        return None;
    }
    let rate = 0.4 + 0.2 * sfx.share() + 0.5;
    let sample = sfx.sample(id, file, assets);
    Some(
        audio
            .play(sample)
            .with_volume(sfx::decibels(volume))
            .with_panning(pan)
            .with_playback_rate(f64::from(rate))
            .handle(),
    )
}

/// The creatures near the hero make their idle noises at the world's tick: those not hostile to
/// it in any state, those hostile while they do not fight (the original's state of rest, the
/// record's `+0xDBC` 0, which `0x68BEA0` keeps from the creature's moves and strikes); the noise
/// plays as the voices do, at the volume of its place and its own rate.
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
    (time, heard): (Res<Time>, Res<sfx::Listener>),
) {
    let Some(listener) = heroes.iter().next().map(GlobalTransform::translation) else {
        return;
    };
    let now = time.elapsed_secs_f64();
    for (creature, at, mind, speaking) in &creatures {
        let (near, pan) = sfx::placed_at(at.translation(), Some(listener), *heard);
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
        let Some((id, file, wait, higher)) =
            voices.idle_noise(&mut sfx, mind.thinks.creature, hostile)
        else {
            continue;
        };
        if voices.waits.get(&id).is_some_and(|&until| until > now) {
            continue;
        }
        let rate = rate(sfx.roll(10), higher);
        let sample = sfx.sample(id, file, &assets);
        let instance = audio
            .play(sample)
            .with_volume(sfx::decibels(near))
            .with_panning(pan)
            .with_playback_rate(f64::from(rate))
            .handle();
        voices.waits.insert(id, now + f64::from(wait));
        debug!(
            "voices: {creature} makes the idle noise {id}, at {near:.2}, panned {pan:.2}, at a \
             rate of {rate:.2}"
        );
        commands.entity(creature).insert(Speaking(instance, false));
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
            .init_resource::<NamedLine>()
            .add_systems(bevy::prelude::OnEnter(crate::game::AppState::InGame), names)
            .add_systems(Update, (dies, speak).chain().in_set(Unpaused))
            .add_systems(
                Update,
                idle.after(speak)
                    .in_set(Unpaused)
                    .run_if(crate::frame::ticked),
            )
            .add_systems(Update, (breathe, flight).after(speak).in_set(Unpaused));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_voice_plays_at_a_rate_about_its_samples() {
        // A line at 0.95 to 1.05 of its sample's rate; a noise of a case's 0.7 higher.
        assert!((rate(0.0, 0.0) - 0.95).abs() < 1e-6);
        assert!((rate(9.0, 0.0) - 1.05).abs() < 1e-6);
        assert!((rate(0.0, 0.7) - 1.65).abs() < 1e-6);
        // The noises converted before name it their volume.
        let noise: IdleNoise = ron::from_str(
            "(creature: 1, roll: 1, sound: Ids(first: 9, count: 2), wait: 0.0, volume: 0.2)",
        )
        .unwrap();
        assert_eq!(noise.rate, 0.2);
    }

    #[test]
    fn the_hero_comments_on_its_foes_by_the_original_conditions() {
        let boss = Heard {
            group: NORMAL_HIT,
            hero: false,
            kind: 300,
            marked: true,
            level: 10,
            foe: None,
        };
        let hero = HeroState {
            level: 10,
            health: 100,
            grunts: true,
        };
        // A taunt at a boss's hit only while the hero's grunt sounds, more than 10 s on.
        let taunts = |heard: Heard, hero: HeroState, clock: f64| {
            let mut dice = Rng::of(0, 0, 1);
            (0..200)
                .filter_map(|_| comment(heard, hero, clock, &mut dice))
                .map(|(line, _)| line)
                .collect::<Vec<_>>()
        };
        let lines = taunts(boss, hero, 11.0);
        assert!(!lines.is_empty() && lines.iter().all(|&l| l == HeroLine::HitEnemy));
        assert!(taunts(boss, hero, 9.0).is_empty());
        let silent = HeroState {
            grunts: false,
            ..hero
        };
        // Silent, a boss's hit makes `BOSSMOB` only past 30 s; a common creature's nothing.
        assert!(taunts(boss, silent, 20.0).is_empty());
        let late = taunts(boss, silent, 31.0);
        assert!(!late.is_empty() && late.iter().all(|&l| l == HeroLine::BossMob));
        let common = Heard {
            marked: false,
            ..boss
        };
        assert!(taunts(common, silent, 100.0).is_empty());
        // A foe two levels below the hero at full health is too weak; an animal never.
        let weak = Heard { level: 8, ..common };
        assert!(taunts(weak, silent, 16.0).contains(&HeroLine::EnemyTooWeak));
        let animal = Heard { kind: 520, ..weak };
        assert!(taunts(animal, silent, 16.0).is_empty());
        // A boss's death, more than 10 s on.
        let dies = Heard {
            group: DEATH,
            ..boss
        };
        assert!(taunts(dies, silent, 11.0).contains(&HeroLine::KillEnemy));
        // The hero's attack names the foe near it, more than 20 s on.
        let attacks = Heard {
            group: ATTACK,
            hero: true,
            kind: 1,
            marked: false,
            level: 10,
            foe: Some(Foe::Orc),
        };
        let named = taunts(attacks, silent, 21.0);
        assert!(!named.is_empty() && named.iter().all(|&l| l == HeroLine::EnemyType(Foe::Orc)));
        assert!(taunts(attacks, silent, 19.0).is_empty());
        assert!(
            taunts(
                Heard {
                    foe: None,
                    ..attacks
                },
                silent,
                21.0
            )
            .is_empty()
        );
        assert_eq!(
            HeroLine::EnemyType(Foe::Orc).name(1, 0).as_deref(),
            Some("SOUND_FX_ENEMYTYPE_ORC01_SERA")
        );
    }

    #[test]
    fn the_hero_breathes_by_its_gait() {
        use hero::Gait;
        // The Seraphim walking 1 to 5, running 6 to 10; the Vampiress lacks 6 and 7.
        assert_eq!([0, 4].map(|n| breath(1, Some(Gait::Walk), n)), [1, 5]);
        assert_eq!([0, 4].map(|n| breath(1, Some(Gait::Run), n)), [6, 10]);
        assert_eq!(
            (0..5)
                .map(|n| breath(6, Some(Gait::Run), n))
                .collect::<Vec<_>>(),
            [8, 9, 8, 9, 10]
        );
        assert_eq!(breath(6, None, 2), 3);
        assert_eq!(
            HeroLine::Breathing.name(2, 5).as_deref(),
            Some("SOUND_FX_BREATHING06_GLAD")
        );
    }

    #[test]
    fn a_foe_flees_running_away_from_the_hero() {
        let hero = Motions::default();
        let mut foe = Motions::default();
        // Sixteen updates running east, the hero to the west of it.
        for x in 0..16 {
            foe.push(RUNS, Vec3::new(100.0 + 10.0 * x as f32, 0.0, 0.0));
        }
        assert_eq!(foe.share(RUNS), 100);
        assert!(engages(&foe));
        assert!(flees(&foe, Vec3::new(250.0, 0.0, 0.0), (Vec3::ZERO, &hero)));
        // Running toward the hero, or far off, it does not flee.
        foe.push(RUNS, Vec3::new(240.0, 0.0, 0.0));
        assert!(!flees(
            &foe,
            Vec3::new(240.0, 0.0, 0.0),
            (Vec3::ZERO, &hero)
        ));
        let mut far = Motions::default();
        for x in 0..16 {
            far.push(RUNS, Vec3::new(5_000.0 + 10.0 * x as f32, 0.0, 0.0));
        }
        assert!(!flees(
            &far,
            Vec3::new(5_150.0, 0.0, 0.0),
            (Vec3::ZERO, &hero)
        ));
        // A creature that stands does not engage.
        assert!(!engages(&Motions::default()));
    }

    #[test]
    fn the_hero_names_its_foes_by_their_types_names() {
        let kind = |name: &str| named(name);
        assert_eq!(kind("TYPE_NPC_ORC_01"), Some(Named::Foe(Foe::Orc)));
        assert_eq!(kind("TYPE_NPC_SOLDIER14"), Some(Named::Foe(Foe::Demordrey)));
        assert_eq!(kind("TYPE_NPC_SOLDIER06"), Some(Named::Ends));
        assert_eq!(kind("TYPE_NPC_RIESE"), Some(Named::Foe(Foe::Giant)));
        assert_eq!(kind("TYPE_NPC_RIESE_02"), None);
        // The lesser demon of the gargoyles is a demon, the other gargoyles monsters.
        assert_eq!(
            kind("TYPE_NPC_GARGOYLE_LESSERDEMON"),
            Some(Named::Foe(Foe::Demon))
        );
        assert_eq!(kind("TYPE_NPC_GARGOYLE01"), Some(Named::Foe(Foe::Monster)));
        // The undead goblins before the goblins.
        assert_eq!(
            kind("TYPE_NPC_GOBLIN_UNDEAD"),
            Some(Named::Foe(Foe::Undead))
        );
        assert_eq!(
            kind("TYPE_NPC_GOBLIN_WARRIOR"),
            Some(Named::Foe(Foe::Goblin))
        );
        assert_eq!(kind("TYPE_NPC_WOLF"), None);
    }

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
            HeroLine::BattleCry {
                sure: false,
                night: false
            }
            .name(9, 9)
            .as_deref(),
            Some("SOUND_FX_BATTLECRY_10_DEMON")
        );
        assert_eq!(
            HeroLine::BattleCry {
                sure: false,
                night: true
            }
            .name(6, 0)
            .as_deref(),
            Some("SOUND_FX_BATTLECRY_01_REALVAMP")
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
            rate: 0.0,
        };
        let mut voices = Voices {
            voices: HashMap::new(),
            waits: HashMap::new(),
            commented: None,
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
