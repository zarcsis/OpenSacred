//! The hero's remarks outside the fight (#1176; docs/re/sound.md, "The hero's comments"): lines
//! by name that the sound manager's event 9 and its `0x696060` play as the scripts, the world and
//! the player give them cause (`0x693FE0`), at the hero in the middle, at the sample's own rate.
//!
//! On the quests and on idling (#1246), each stopping the line by name that plays, the comments'
//! clock left alone: `GETNQ` as a side quest is taken in a talk (`0x46C160`), `REJECTNQ` at the
//! scripts' `RejectQuest`, and `CANCELMYIDLE` once the play has run 120 s without the player's
//! input (`0x424A60`).
//!
//! On the land (#1247), each case with its timer (`+0x14`, by the event's sub-command), which every
//! frame runs on: `REGIONTOOSTRONG` or `REGIONTOOWEAK` as the hero crosses into another area of a
//! level 5 above its own or 3 below (`0x6263B0`), stopping the line by name; `REGIONPACIFIED`, out
//! of the fight, at the same crossing into an area pacified and at the scripts' `RegionFrei`; and
//! `COMEINTOTOWN` or `COMEINTOVILLAGE`, out of the fight and once in 300 s, as it steps onto a
//! settlement's ground outside the dungeons (`0x593CE0`), waiting for silence and for its line's
//! wait, setting the clock back and making the line wait 60 s.

use std::collections::HashMap;

use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::{MouseButtonInput, MouseWheel};
use bevy::log::debug;
use bevy::prelude::{
    App, AssetServer, Assets, Commands, CursorMoved, Entity, GlobalTransform, IntoScheduleConfigs,
    Message, MessageReader, MessageWriter, NonSend, OnEnter, Plugin, Query, Res, ResMut, Resource,
    Time, Transform, Update, Vec3, With, Without,
};
use bevy_kira_audio::prelude::{Audio, AudioControl, AudioInstance};
use opensacred_rules::spawning::Rng;

use crate::ai::Mind;
use crate::daynight::Dungeons;
use crate::fight::Health;
use crate::frame::{Playing, Unpaused};
use crate::game::{AppState, Data};
use crate::hero::{self, Body};
use crate::npcs::{Dies, Npcs};
use crate::sfx::{self, Sfx};
use crate::terrain::Heights;
use crate::voices::{self, NamedLine, Speaking, Voices};
use crate::{music, paths, scripts};

/// A remark of the hero's, which a module asks for.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Remark {
    /// `GETNQ` (9, `0x40`): a side quest entered in a talk.
    TakesQuest,
    /// `REJECTNQ`: the scripts' `RejectQuest` (126, `0x696060`).
    RejectsQuest,
    /// `CANCELMYIDLE` (9, `0x100`): 120 s of play without the player's input.
    Idles,
    /// `REGIONTOOSTRONG` or `REGIONTOOWEAK` (9, `0x30`): an area's border crossed into a land too
    /// strong or too weak for the hero.
    Region { strong: bool },
    /// `REGIONPACIFIED` (9, `0x35`): an area pacified come into, or one that the scripts'
    /// `RegionFrei` pacifies, `forced` past the case's timer.
    Pacified { forced: bool },
    /// `COMEINTOTOWN` or `COMEINTOVILLAGE` (9, `0xCC`): a settlement's ground entered, or left.
    Settlement { entering: bool },
}

impl Remark {
    /// How many lines a class has, one of which `rand() %` this picks.
    fn count(self) -> u32 {
        match self {
            Self::TakesQuest | Self::RejectsQuest => 11,
            Self::Idles => 14,
            Self::Region { .. } => 6,
            Self::Pacified { .. } => 1,
            Self::Settlement { .. } => 12,
        }
    }

    /// The name of the line `n`, from 0, of the hero of the type `creature`: none of
    /// `REJECTNQ` for the Dwarf and the Daemon, whose lines `0x696060`'s table of seven classes
    /// leaves out.
    fn name(self, creature: u32, n: u32) -> Option<String> {
        let class = voices::class(creature)?;
        let n = n + 1;
        Some(match self {
            Self::TakesQuest => format!("SOUND_FX_GETNQ{class}{n:02}"),
            Self::RejectsQuest if creature > 7 => return None,
            Self::RejectsQuest => format!("SOUND_FX_REJECTNQ{class}{n:02}"),
            Self::Idles => format!("SOUND_FX_CANCELMYIDLE_{n:02}_{class}"),
            // The Gladiator's first too weak line is its first too strong one, the two cases'
            // tails merged (`0x67C9F6`).
            Self::Region { strong } => {
                let strong = strong || (creature == GLADIATOR && n == 1);
                let stem = if strong { "STRONG" } else { "WEAK" };
                format!("SOUND_FX_REGIONTOO{stem}_{n:02}_{class}")
            }
            Self::Pacified { .. } => format!("SOUND_FX_REGIONPACIFIED01_{class}"),
            // Six of the town, then six of the village, by consecutive ids: the Wood Elf, who
            // lacks her fifth of the village, says her fourth for it.
            Self::Settlement { .. } if n <= 6 => format!("SOUND_FX_COMEINTOTOWN_{n:02}_{class}"),
            Self::Settlement { .. } => {
                let n = if creature == WOOD_ELF && n == 11 {
                    4
                } else {
                    n - 6
                };
                format!("SOUND_FX_COMEINTOVILLAGE_{n:02}_{class}")
            }
        })
    }

    /// The key of its case's timer, the event's sub-command, if it has one.
    fn timer(self) -> Option<u8> {
        match self {
            Self::Region { .. } => Some(REGION),
            Self::Pacified { .. } => Some(PACIFIED),
            Self::Settlement { .. } => Some(SETTLEMENT),
            _ => None,
        }
    }
}

/// The types of the Gladiator and of the Wood Elf.
const GLADIATOR: u32 = 2;
const WOOD_ELF: u32 = 5;
/// The cases' timers, by the event's sub-command.
const REGION: u8 = 0x30;
const PACIFIED: u8 = 0x35;
const SETTLEMENT: u8 = 0xCC;
/// How long the play runs without the player's input before the hero remarks on it, in seconds
/// (`0x424A60`).
const IDLE: f64 = 120.0;
/// How far a hostile creature stands from the hero, in the place's pixels, for the hero to be in a
/// fight (`0x678F80`).
const FIGHT_WITHIN: i32 = 800;
/// The flag of a settlement's ground in a cell's flags (`+0x1E` bit 1, docs/02 §5.2).
const SETTLED: u8 = 2;

/// The seconds of play since the player's last input or the last idle remark (`cStatsManager`'s
/// clock `+0x56F8` against its stamp `+0x5714`), the cases' timers, the sectors' areas, the
/// sector and the cell the hero stood in at the last frame, and the remarks' rolls.
#[derive(Resource)]
struct Remarks {
    idle: f64,
    timers: HashMap<u8, f64>,
    areas: HashMap<(u32, u32), u8>,
    sector: Option<(u32, u32)>,
    cell: Option<paths::Cell>,
    dice: Rng,
}

impl Default for Remarks {
    fn default() -> Self {
        Self {
            idle: 0.0,
            timers: HashMap::new(),
            areas: HashMap::new(),
            sector: None,
            cell: None,
            dice: Rng::of(0, 0, 0x1246),
        }
    }
}

impl Remarks {
    /// `0x691700(key, limit, made, 1)`: whether the timer `key`, made at `made`, is past
    /// `limit`, which then starts it again.
    fn passes(&mut self, key: u8, limit: f64, made: f64) -> bool {
        let timer = self.timers.entry(key).or_insert(made);
        if *timer > limit {
            *timer = 0.0;
            return true;
        }
        false
    }

    /// Whether the case of `remark` lets it speak, its timers moved as the case moves them; a
    /// case out of the fight's only `calm`.
    fn lets(&mut self, remark: Remark, calm: bool) -> bool {
        match remark {
            Remark::Region { .. } => self.passes(REGION, 40.0, 41.0),
            Remark::Pacified { forced } => {
                let timer = *self.timers.entry(PACIFIED).or_insert(0.0);
                if !calm || !(forced || timer > 40.0) {
                    return false;
                }
                self.timers.insert(PACIFIED, 0.0);
                self.timers.insert(REGION, 30.0);
                true
            }
            Remark::Settlement { entering } => {
                calm && self.passes(SETTLEMENT, 300.0, 41.0) && entering
            }
            _ => true,
        }
    }
}

/// Sets the idle clock back at every key, mouse button, wheel and motion of the cursor
/// (`0x423FD0`, which the engine's events of the kinds 1 to 4 and two input handlers call;
/// **hypothesis**: the keys and the mouse, its motion among them).
fn input(
    mut remarks: ResMut<Remarks>,
    mut keys: MessageReader<KeyboardInput>,
    mut buttons: MessageReader<MouseButtonInput>,
    mut wheel: MessageReader<MouseWheel>,
    mut moved: MessageReader<CursorMoved>,
) {
    let any = keys.read().count() + buttons.read().count() + wheel.read().count();
    if any + moved.read().count() > 0 {
        remarks.idle = 0.0;
    }
}

/// Starts the idle clock and the hero's places with each game, and takes the sectors' areas.
fn begin(mut remarks: ResMut<Remarks>, data: Option<Res<Data>>) {
    remarks.idle = 0.0;
    remarks.sector = None;
    remarks.cell = None;
    if let Some(data) = data {
        remarks.areas = data
            .0
            .world
            .sectors
            .iter()
            .map(|s| ((s.x, s.y), s.area))
            .collect();
    }
}

/// Runs the idle clock while the play runs with a hero, and past [`IDLE`] asks for the remark
/// and starts it again (`0x424A60`).
fn idle(
    mut remarks: ResMut<Remarks>,
    time: Res<Time>,
    heroes: Query<(), With<Body>>,
    mut posts: MessageWriter<Remark>,
) {
    if !heroes.is_empty() && idles(&mut remarks.idle, f64::from(time.delta_secs())) {
        posts.write(Remark::Idles);
    }
}

/// A frame of `seconds` on the idle clock `idle`: whether it passed [`IDLE`], the clock then
/// started again.
fn idles(idle: &mut f64, seconds: f64) -> bool {
    *idle += seconds;
    if *idle > IDLE {
        *idle = 0.0;
        return true;
    }
    false
}

/// The remark of a crossing from an area of the level `before` into one of the level `after` by a
/// hero of the level `hero` (`0x62686D`–`0x6268A3`): too strong where the new level stands above
/// the hero's plus 5 and the one before did not reach it, else too weak where the new stands below
/// the hero's less 3 and the one before above it, never for a hero below the level 3.
fn region(before: u32, after: u32, hero: u32) -> Option<Remark> {
    if before < hero + 5 && after > hero + 5 {
        return Some(Remark::Region { strong: true });
    }
    let weak = hero.checked_sub(3)?;
    (before > weak && after < weak).then_some(Remark::Region { strong: false })
}

/// Asks for the land's remarks as the hero moves (#1247): at a sector of another area, the
/// region's by the two sectors' levels against the hero's, then the pacified area's where the new
/// area is pacified and the one before not (`0x6263B0`, the world view's centre sector, here the
/// hero's); at a cell that comes onto a settlement's ground or leaves it, outside the dungeons,
/// the settlement's (`0x593CE0`).
fn land(
    mut remarks: ResMut<Remarks>,
    heroes: Query<(&Transform, Option<&Health>), With<Body>>,
    (npcs, dungeons, heights): (Option<Res<Npcs>>, Option<Res<Dungeons>>, Res<Heights>),
    scripts: Option<NonSend<scripts::Live>>,
    mut posts: MessageWriter<Remark>,
) {
    let Ok((at, health)) = heroes.single() else {
        return;
    };
    let (x, z) = (at.translation.x, at.translation.z);
    let sector = music::sector_of(x, z);
    if let Some(before) = remarks.sector.replace(sector)
        && before != sector
    {
        let area = |sector| remarks.areas.get(&sector).copied().unwrap_or(0);
        let (was, is) = (area(before), area(sector));
        if was != is {
            let level = |sector| npcs.as_ref().map_or(0, |npcs| npcs.level(sector));
            let hero = health.map_or(1, |health| health.level);
            debug!(
                "remarks: the hero of the level {hero} crosses from the area {was} of the level {}                  into the area {is} of the level {}",
                level(before),
                level(sector)
            );
            if let Some(remark) = region(level(before), level(sector), hero) {
                posts.write(remark);
            }
            let pacified = |area| scripts.as_ref().is_some_and(|live| live.pacified(area));
            if pacified(is) && !pacified(was) {
                posts.write(Remark::Pacified { forced: false });
            }
        }
    }
    let cell = paths::cell_of(x, z);
    if let Some(before) = remarks.cell.replace(cell)
        && before != cell
        && !dungeons.is_some_and(|dungeons| dungeons.sector(sector))
    {
        let settled = |cell| {
            heights
                .flags(cell)
                .is_some_and(|flags| flags & SETTLED != 0)
        };
        let entering = settled(cell);
        if settled(before) != entering {
            debug!(
                "remarks: the hero {} a settlement's ground at {cell:?}",
                if entering { "comes onto" } else { "leaves" }
            );
            posts.write(Remark::Settlement { entering });
        }
    }
}

/// Whether the hero at `at` stands out of the fight (`0x678F80`): no living creature hostile to it
/// within [`FIGHT_WITHIN`], nor one that fights, wherever it stands (**hypothesis** for the
/// states 1, 3 and 4 of the sound manager's records, and the foes engaged within them).
fn calm<'a>(
    at: Vec3,
    creatures: impl IntoIterator<Item = (&'a GlobalTransform, &'a Mind)>,
) -> bool {
    !creatures.into_iter().any(|(place, mind)| {
        mind.hostile()
            && (mind.fighting() || sfx::screen_distance(place.translation(), at) < FIGHT_WITHIN)
    })
}

/// Plays the remarks asked for, each as its case lets it (`Remarks::lets`): one of the class's
/// lines by `rand()`, by name, at the hero; the settlement's waiting for silence and for its
/// line's wait, setting the comments' clock back and making the line wait 60 s; the others
/// stopping the line by name that plays (`0x693FE0`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn remark(
    mut commands: Commands,
    mut posts: MessageReader<Remark>,
    (mut remarks, mut voices, time): (ResMut<Remarks>, Option<ResMut<Voices>>, Res<Time>),
    (hero, mut sfx, heard): (Option<Res<hero::Hero>>, ResMut<Sfx>, Res<sfx::Listener>),
    heroes: Query<(Entity, &GlobalTransform), With<Body>>,
    creatures: Query<(&GlobalTransform, &Mind), (Without<Body>, Without<Dies>)>,
    (assets, audio, mut instances, mut named): (
        Res<AssetServer>,
        Res<Audio>,
        ResMut<Assets<AudioInstance>>,
        ResMut<NamedLine>,
    ),
) {
    let seconds = f64::from(time.delta_secs());
    for timer in remarks.timers.values_mut() {
        *timer += seconds;
    }
    let now = time.elapsed_secs_f64();
    let hero_at = heroes.single().ok().map(|(_, at)| at.translation());
    for &post in posts.read() {
        let quiet = post.timer().is_none()
            || matches!(post, Remark::Region { .. })
            || hero_at.is_some_and(|at| calm(at, creatures.iter()));
        // The roll first, as the cases draw it.
        let n = remarks.dice.below(post.count());
        if !remarks.lets(post, quiet) {
            continue;
        }
        let (Some(hero), Ok((entity, at))) = (hero.as_ref(), heroes.single()) else {
            continue;
        };
        let Some(name) = post.name(hero.creature, n) else {
            continue;
        };
        let Some((id, file)) = sfx.named(&name) else {
            debug!("remarks: no line {name}");
            continue;
        };
        let waits = matches!(post, Remark::Settlement { .. });
        if waits
            && (named.plays(&audio)
                || voices
                    .as_ref()
                    .is_some_and(|voices| voices.waiting(id, now)))
        {
            continue;
        }
        let at = at.translation();
        let (volume, pan) = sfx::placed_at(at, Some(at), *heard);
        if volume <= 0.0 {
            continue;
        }
        debug!("remarks: the hero says {name}");
        let sample = sfx.sample(id, file, &assets);
        let instance = audio
            .play(sample)
            .with_volume(sfx::decibels(volume))
            .with_panning(pan)
            .handle();
        if waits && let Some(voices) = voices.as_mut() {
            voices.commented(id, now, 60.0);
        }
        named.name(&mut instances, Some(instance.clone()));
        commands.entity(entity).insert(Speaking(instance, true));
    }
}

/// The hero's remarks, the idle clock and the land's.
pub struct RemarksPlugin;

impl Plugin for RemarksPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Remark>()
            .init_resource::<Remarks>()
            .add_systems(OnEnter(AppState::InGame), begin)
            .add_systems(Update, input.in_set(Playing))
            .add_systems(
                Update,
                (idle, land, remark).chain().after(input).in_set(Unpaused),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_idle_clock_speaks_past_120_s_and_starts_again() {
        let mut idle = 0.0;
        let frames = |idle: &mut f64, count: u32| (0..count).filter(|_| idles(idle, 0.5)).count();
        assert_eq!(frames(&mut idle, 240), 0);
        assert_eq!(frames(&mut idle, 1), 1);
        assert_eq!(frames(&mut idle, 240), 0);
        assert_eq!(frames(&mut idle, 1), 1);
    }

    #[test]
    fn a_remark_names_the_line_of_the_heros_class() {
        assert_eq!(
            Remark::TakesQuest.name(1, 0).as_deref(),
            Some("SOUND_FX_GETNQSERA01")
        );
        assert_eq!(
            Remark::Idles.name(7, 13).as_deref(),
            Some("SOUND_FX_CANCELMYIDLE_14_VAMP")
        );
        assert_eq!(
            Remark::RejectsQuest.name(5, 10).as_deref(),
            Some("SOUND_FX_REJECTNQHELF11")
        );
        // The Dwarf and the Daemon reject in silence.
        assert_eq!(Remark::RejectsQuest.name(8, 0), None);
        assert_eq!(Remark::RejectsQuest.name(9, 0), None);
        // The Gladiator's first too weak line, and the Wood Elf's fifth of the village.
        let weak = Remark::Region { strong: false };
        assert_eq!(
            weak.name(GLADIATOR, 0).as_deref(),
            Some("SOUND_FX_REGIONTOOSTRONG_01_GLAD")
        );
        assert_eq!(
            weak.name(GLADIATOR, 1).as_deref(),
            Some("SOUND_FX_REGIONTOOWEAK_02_GLAD")
        );
        let village = Remark::Settlement { entering: true };
        assert_eq!(
            village.name(1, 5).as_deref(),
            Some("SOUND_FX_COMEINTOTOWN_06_SERA")
        );
        assert_eq!(
            village.name(WOOD_ELF, 10).as_deref(),
            Some("SOUND_FX_COMEINTOVILLAGE_04_HELF")
        );
        assert_eq!(
            village.name(WOOD_ELF, 11).as_deref(),
            Some("SOUND_FX_COMEINTOVILLAGE_06_HELF")
        );
    }

    #[test]
    fn a_crossing_weighs_the_two_areas_levels_against_the_heros() {
        // A hero of level 10: past 15 too strong, below 7 too weak.
        assert_eq!(region(12, 16, 10), Some(Remark::Region { strong: true }));
        assert_eq!(region(16, 20, 10), None);
        assert_eq!(region(9, 6, 10), Some(Remark::Region { strong: false }));
        assert_eq!(region(6, 2, 10), None);
        assert_eq!(region(9, 9, 10), None);
        // No land too weak for a hero below the level 3.
        assert_eq!(region(5, 0, 2), None);
    }

    #[test]
    fn the_cases_timers_space_the_land_remarks() {
        let mut remarks = Remarks::default();
        let strong = Remark::Region { strong: true };
        // The first crossing speaks; the next within 40 s does not.
        assert!(remarks.lets(strong, false));
        assert!(!remarks.lets(strong, false));
        // A pacified area out of the fight, forced or past 40 s, holds the region's 10 s more.
        assert!(!remarks.lets(Remark::Pacified { forced: true }, false));
        assert!(remarks.lets(Remark::Pacified { forced: true }, true));
        assert!(!remarks.lets(Remark::Pacified { forced: false }, true));
        for timer in remarks.timers.values_mut() {
            *timer += 10.5;
        }
        assert!(remarks.lets(strong, false));
        // The settlement: once in 300 s, its first post making the timer at 41 s; a leaving that
        // passes the gate silent.
        let entering = Remark::Settlement { entering: true };
        assert!(!remarks.lets(entering, true));
        assert!(remarks.timers.get(&SETTLEMENT).is_some_and(|&t| t > 40.0));
        remarks.timers.insert(SETTLEMENT, 301.0);
        assert!(!remarks.lets(Remark::Settlement { entering: false }, true));
        assert!(!remarks.lets(entering, true));
        remarks.timers.insert(SETTLEMENT, 301.0);
        assert!(remarks.lets(entering, true));
    }
}
