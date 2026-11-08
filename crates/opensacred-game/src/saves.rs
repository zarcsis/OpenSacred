//! The saves (#1095; docs/re/saves.md): a game kept in a file of its own and given back, as the
//! original's `cEngine::save` keeps its game in `SAVE/GAMEnn.PAK`, in the game's own versioned
//! format (PLAN.md ADR-08). A save is postcard: its [`Header`], which the saves' list reads alone,
//! then its [`Hero`], its [`Scripts`] and its [`World`]; a save of another version is refused, as
//! the original's load refuses one (`0x61B6F0`). The saves lie in `saves/` beside the settings'
//! file.
//!
//! The first part (#1957) keeps the hero: its class, name, difficulty, values, items, arts,
//! combos, effects, statistics and place, and the calendar. A load starts a game of the save's
//! hero as a new game starts, then puts the saved values back once the hero stands, after the
//! resets of a game's start (`OnEnter(InGame)`), the hero where it stood.
//!
//! The second (#1958) keeps the scripts: the game's seed, their state, the functions done and
//! their `rand()`, and what their play changed of the placements. A load's start runs of the
//! save's seed, making the saved game's tree and placements again, and takes the save's scripts
//! in place of its run's, none of its windows shown again ([`crate::scripts::Started::resume`]),
//! and the play's records of the world placed again ([`crate::npcs::Npcs::resume`]).
//!
//! The third (#1959) keeps the world: the creatures that the sectors keep, those of the sectors in
//! the world gone as their sectors keep them going, which a load's sectors put back as they come
//! in ([`crate::npcs::Npcs::remember`]); the hero's followers, put back beside it; the drops; the
//! town stash; the merchants' stock; the land explored and the map's pin; and the weather.
//!
//! The quick save (#1988): F9 and Control+Z save the game as `QUICKSAVE` in the slot 0's file
//! `GAME00` where the interface allows a save ([`allowed`]), F8 and Control+X load it. The
//! scripts' autosave (#1989): an `AutoSave` saves the game into the first free slot, named by its
//! number's text, where `[gameplay] autosave` is on ([`autosave`]).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail, ensure};
use bevy::prelude::{
    App, Commands, Entity, IntoScheduleConfigs, NextState, OnEnter, Plugin, Query, Real, Res,
    ResMut, Resource, Time, Transform, Update, Vec2, Vec3, With, in_state, info, not,
    resource_exists, warn,
};
use opensacred_rules::arts::Arts;
use opensacred_rules::drops::Difficulty;
use opensacred_rules::explored::{Explored, Sectors};
use opensacred_rules::fight::Experience;
use opensacred_rules::trade::stock::Stock;
use serde::{Deserialize, Serialize};

use crate::art_slots::ArtSlots;
use crate::combos::HeroCombos;
use crate::daynight::Clock;
use crate::effects::Effects;
use crate::explored::{Land, MapSectors};
use crate::fight::Health;
use crate::frame::Step;
use crate::game::{AppState, Class, NewGame};
use crate::hero::{Arrival, Body};
use crate::merchant::Wares;
use crate::pickup::{self, Pack};
use crate::save_picture::Picture;
use crate::scripts::{HeroHome, Teleport};
use crate::sets::WeaponSets;
use crate::settings::SettingsFile;
use crate::statistics::{History, Kills, Tally};
use crate::terrain::Heights;
use crate::town_stash::Chest;
use crate::world_map::Map;
use crate::{arts, drops, making};

/// The saves' version: a save of another is refused, but those of the version before it, which
/// are as these without the picture after their header (#1991).
pub const VERSION: u32 = 4;
const UNPICTURED: u32 = 3;
/// The saves' directory beside the settings' file, and their extension.
const DIRECTORY: &str = "saves";
const EXTENSION: &str = "sav";

/// What the saves' list shows of a save, read alone: the version, its name, the hero's class,
/// name, difficulty and level, the game's calendar and the computer's time of the save, as the
/// original's header holds them (`0x619BF0`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Header {
    pub version: u32,
    pub name: String,
    pub class: Class,
    pub hero: String,
    pub difficulty: Difficulty,
    pub level: u32,
    /// The game's time of day, in seconds.
    pub clock: f64,
    /// The computer's time of the save, in seconds since 1970.
    pub saved: u64,
}

/// The hero as a save keeps it.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Hero {
    /// Where it stands, in world units over the ground's plane, and its floor, 0 the ground.
    pub at: (f32, f32),
    pub floor: u8,
    pub experience: Experience,
    pub base: opensacred_rules::fight::Base,
    pub health: u32,
    pub effects: Effects,
    pub arts: Arts,
    pub slots: ArtSlots,
    pub selected: Option<u16>,
    pub combos: HeroCombos,
    pub pack: pickup::Kept,
    pub sets: WeaponSets,
    pub home: Option<Teleport>,
    pub tally: Tally,
    pub kills: Kills,
    pub history: History,
    /// The seconds played, which the creatures' levels and the fills follow (`drops::Played`).
    pub played: f32,
    pub clock: Clock,
}

/// The scripts as a save keeps them (#1958; the original's quests, `sdk_SAVELOAD_SERIAL`): the
/// game's seed (`seed::Game`), whose start makes the saved game's tree and placements again, the
/// scripts' state, functions done and `rand()`, and what their play changed of the placements.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Scripts {
    pub seed: u64,
    pub scripts: crate::scripts::Saved,
    pub npcs: crate::npcs::Saved,
}

/// The world as a save keeps it (#1959; the original's object manager, its weather and its map,
/// docs/re/saves.md): the creatures that the sectors keep, those of the sectors in the world gone
/// as their sectors keep them going (`npcs::Creatures`), the hero's followers, the drops lying
/// and flying, the town stash, the merchants' stock, the land explored and the sectors entered,
/// the world map's pin, and the weather, its timers of real time from the save's.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct World {
    pub creatures: crate::npcs::Creatures,
    pub followers: Vec<crate::npcs::Following>,
    pub drops: Vec<drops::Laid>,
    pub stash: Chest,
    pub wares: Stock,
    pub land: Explored,
    pub sectors: Sectors,
    pub pin: Option<(Vec2, Vec2)>,
    pub weather: Option<opensacred_rules::weather::Weather>,
}

/// The saves' directory: `saves/` beside the settings' file, else in the configuration
/// directory.
fn directory(settings: Option<&SettingsFile>) -> Option<PathBuf> {
    settings
        .and_then(SettingsFile::path)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .or_else(crate::settings::config_dir)
        .map(|dir| dir.join(DIRECTORY))
}

/// The file of the save `name`: a name of letters, digits, blanks, `-` and `_`.
fn file(dir: &Path, name: &str) -> Result<PathBuf> {
    let name = name.trim();
    ensure!(
        !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_')),
        "{name:?}: a save's name is letters, digits, blanks, - and _"
    );
    Ok(dir.join(format!("{name}.{EXTENSION}")))
}

/// Writes a save of `header`, its `picture`, `hero`, `scripts` and `world` at `path`, the picture
/// after the header, which the saves' list reads with it.
pub fn write(
    path: &Path,
    (header, picture): (&Header, Option<&Picture>),
    (hero, scripts, world): (&Hero, &Scripts, &World),
) -> Result<()> {
    let mut bytes = postcard::to_stdvec(header).context("writing the header")?;
    bytes.extend(postcard::to_stdvec(&picture).context("writing the picture")?);
    bytes.extend(postcard::to_stdvec(hero).context("writing the hero")?);
    bytes.extend(postcard::to_stdvec(scripts).context("writing the scripts")?);
    bytes.extend(postcard::to_stdvec(world).context("writing the world")?);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("making {}", dir.display()))?;
    }
    fs::write(path, bytes).with_context(|| format!("writing {}", path.display()))
}

/// A save's header, its picture and the bytes after them, refused where its version is neither
/// [`VERSION`] nor the one before, which has no picture.
fn header(bytes: &[u8]) -> Result<(Header, Option<Picture>, &[u8])> {
    let (header, rest) =
        postcard::take_from_bytes::<Header>(bytes).context("reading the header")?;
    let (picture, rest) = match header.version {
        VERSION => postcard::take_from_bytes(rest).context("reading the picture")?,
        UNPICTURED => (None, rest),
        version => bail!("a save of the version {version}, where the game reads {VERSION}"),
    };
    Ok((header, picture, rest))
}

/// Reads the save at `path`.
pub fn read(path: &Path) -> Result<(Header, Hero, Scripts, World)> {
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let (header, _, rest) = header(&bytes).with_context(|| path.display().to_string())?;
    let (hero, rest) = postcard::take_from_bytes(rest)
        .with_context(|| format!("{}: reading the hero", path.display()))?;
    let (scripts, rest) = postcard::take_from_bytes(rest)
        .with_context(|| format!("{}: reading the scripts", path.display()))?;
    let world = postcard::from_bytes(rest)
        .with_context(|| format!("{}: reading the world", path.display()))?;
    Ok((header, hero, scripts, world))
}

/// The saves in `dir`, each its file's name and its header, the newest first; a file that does
/// not read is left out.
pub fn list(dir: &Path) -> Vec<(String, Header)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut saves: Vec<(String, Header)> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == EXTENSION))
        .filter_map(|path| {
            let bytes = fs::read(&path).ok()?;
            let (header, _, _) = header(&bytes).ok()?;
            let stem = path.file_stem()?.to_string_lossy().into_owned();
            Some((stem, header))
        })
        .collect();
    saves.sort_by(|a, b| b.1.saved.cmp(&a.1.saved).then_with(|| a.0.cmp(&b.0)));
    saves
}

/// A slot's save as the saves' window lists it (#1990): its file's name, its header and its
/// picture.
#[derive(Clone, Debug)]
pub struct Slot {
    pub stem: String,
    pub header: Header,
    pub picture: Option<Picture>,
}

/// Whether `stem` names a slot's file: `GAME` and digits as its fifth and sixth characters
/// (`0x71EAA0`), the slot 0 the quick save's.
fn slot_of(stem: &str) -> Option<u32> {
    let rest = stem.strip_prefix("GAME")?;
    let digits = rest.get(..2)?;
    digits
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| rest.parse().ok())
        .flatten()
}

/// The slots' saves in `dir` (`0x71EAA0`): the quick save of the slot 0 apart, the others by
/// their files' names as text, as the original sorts them; a file that does not read is left out.
pub fn slots(dir: &Path) -> (Option<Slot>, Vec<Slot>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return (None, Vec::new());
    };
    let mut slots: Vec<(u32, Slot)> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == EXTENSION))
        .filter_map(|path| {
            let stem = path.file_stem()?.to_string_lossy().into_owned();
            let slot = slot_of(&stem)?;
            let bytes = fs::read(&path).ok()?;
            let (header, picture, _) = header(&bytes).ok()?;
            Some((
                slot,
                Slot {
                    stem,
                    header,
                    picture,
                },
            ))
        })
        .collect();
    slots.sort_by(|a, b| a.1.stem.cmp(&b.1.stem));
    let quick = slots
        .iter()
        .position(|(slot, _)| *slot == 0)
        .map(|at| slots.remove(at).1);
    (quick, slots.into_iter().map(|(_, slot)| slot).collect())
}

/// The saves' directory of this run.
pub fn saves_directory(world: &bevy::prelude::World) -> Option<PathBuf> {
    directory(world.get_resource::<SettingsFile>())
}

/// Erases the save `stem` of `dir` (the saves' window's Delete, `0x71CC90`'s action 8).
pub fn erase(dir: &Path, stem: &str) -> Result<()> {
    let path = file(dir, stem)?;
    fs::remove_file(&path).with_context(|| format!("erasing {}", path.display()))?;
    info!("saves: {} erased", path.display());
    Ok(())
}

/// The hero and the game as they stand, to save as `name`.
fn gather(world: &mut bevy::prelude::World, name: &str) -> Result<(Header, Hero)> {
    let request = world
        .get_resource::<crate::game::Request>()
        .map(|request| request.0.clone())
        .unwrap_or_default();
    let mut heroes = world.query_filtered::<(
        &Transform,
        &Body,
        &crate::fight::Base,
        &Health,
        Option<&Effects>,
    ), With<Body>>();
    let (transform, body, base, health, effects) =
        heroes.iter(world).next().context("no hero to save")?;
    // A dead hero, which rises at its home, is not saved: its save would load it dead.
    ensure!(health.current > 0, "the hero is dead");
    let floor = world
        .get_resource::<Heights>()
        .and_then(|heights| heights.floor(body.level()))
        .map_or(0, |(_, floor)| floor);
    let (at, base, health, effects) = (
        (transform.translation.x, transform.translation.z),
        base.0,
        health.current,
        effects.cloned().unwrap_or_default(),
    );
    let resource = |what: &str| format!("no {what} to save");
    let experience = *world
        .get_resource::<crate::fight::Experience>()
        .with_context(|| resource("experience"))?;
    let difficulty = world
        .get_resource::<crate::game::Difficulty>()
        .map(|d| d.0)
        .unwrap_or_default();
    let clock = world
        .get_resource::<Clock>()
        .cloned()
        .with_context(|| resource("clock"))?;
    let hero = Hero {
        at,
        floor,
        experience: experience.0,
        base,
        health,
        effects,
        arts: world
            .get_resource::<arts::HeroArts>()
            .map(|arts| arts.arts.clone())
            .with_context(|| resource("arts"))?,
        slots: world
            .get_resource::<ArtSlots>()
            .cloned()
            .unwrap_or_default(),
        selected: world.get_resource::<making::Selected>().and_then(|s| s.0),
        combos: world
            .get_resource::<HeroCombos>()
            .cloned()
            .unwrap_or_default(),
        pack: world
            .get_resource::<Pack>()
            .map(Pack::kept)
            .with_context(|| resource("pack"))?,
        sets: world
            .get_resource::<WeaponSets>()
            .cloned()
            .unwrap_or_default(),
        home: world.get_resource::<HeroHome>().and_then(|home| home.0),
        tally: world.get_resource::<Tally>().copied().unwrap_or_default(),
        kills: world.get_resource::<Kills>().cloned().unwrap_or_default(),
        history: world.get_resource::<History>().cloned().unwrap_or_default(),
        played: world
            .get_resource::<drops::Played>()
            .map_or(0.0, drops::Played::seconds),
        clock: clock.clone(),
    };
    let header = Header {
        version: VERSION,
        name: name.trim().to_owned(),
        class: request.class.unwrap_or_default(),
        hero: request
            .name
            .clone()
            .unwrap_or_else(|| request.class.unwrap_or_default().name().to_owned()),
        difficulty,
        level: experience.0.level,
        clock: clock.minutes() * 60.0,
        saved: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs()),
    };
    Ok((header, hero))
}

/// The scripts as they stand, to save.
fn scripts(world: &bevy::prelude::World) -> Result<Scripts> {
    let seed = world
        .get_resource::<crate::seed::Game>()
        .context("no game's seed to save")?;
    let live = world
        .get_non_send::<crate::scripts::Live>()
        .context("the scripts do not run")?;
    let npcs = world
        .get_resource::<crate::npcs::Npcs>()
        .context("no NPCs to save")?;
    Ok(Scripts {
        seed: seed.0,
        scripts: live.saved(),
        npcs: npcs.saved(),
    })
}

/// The world as it stands, to save.
fn world_of(world: &mut bevy::prelude::World) -> Result<World> {
    let creatures = crate::npcs::creatures(world).context("no NPCs to save")?;
    let followers = crate::npcs::following(world);
    let drops = drops::laid(world);
    // The weather's timers from the save's time of real time, which a load's run starts anew.
    let now = world
        .get_resource::<Time<Real>>()
        .map_or(0.0, |time| time.elapsed_secs_f64() * 1_000.0);
    let weather = world
        .get_resource::<crate::weather::Weather>()
        .map(|weather| {
            let mut state = weather.state.clone();
            state.rebase(-now);
            state
        });
    Ok(World {
        creatures,
        followers,
        drops,
        stash: world.get_resource::<Chest>().cloned().unwrap_or_default(),
        wares: world
            .get_resource::<Wares>()
            .map(|wares| wares.0.clone())
            .unwrap_or_default(),
        land: world
            .get_resource::<Land>()
            .map(|land| land.0.clone())
            .unwrap_or_default(),
        sectors: world
            .get_resource::<MapSectors>()
            .map(|sectors| sectors.0.clone())
            .unwrap_or_default(),
        pin: world.get_resource::<Map>().and_then(Map::pinned),
        weather,
    })
}

/// Saves the game as `name`: where its file went.
pub fn save(world: &mut bevy::prelude::World, name: &str) -> Result<PathBuf> {
    save_in(world, name, name)
}

/// Saves the game as `name` in the file `stem` of the saves' directory, as the original's save
/// keeps its name inside its slot's file (`GAME%.2d.PAK`, docs/re/saves.md).
pub(crate) fn save_in(world: &mut bevy::prelude::World, stem: &str, name: &str) -> Result<PathBuf> {
    let dir = directory(world.get_resource::<SettingsFile>()).context("no directory for saves")?;
    let path = file(&dir, stem)?;
    let (header, hero) = gather(world, name)?;
    let scripts = scripts(world)?;
    let kept = world_of(world)?;
    // The picture last taken (`save_picture`), as the original's save keeps its last capture.
    let picture = world
        .get_resource::<crate::save_picture::Taken>()
        .and_then(|taken| taken.picture.clone());
    write(&path, (&header, picture.as_ref()), (&hero, &scripts, &kept))?;
    info!("saves: {} saved in {}", header.name, path.display());
    Ok(path)
}

/// The saves of this run's directory.
pub fn saves(world: &bevy::prelude::World) -> Vec<(String, Header)> {
    directory(world.get_resource::<SettingsFile>())
        .map(|dir| list(&dir))
        .unwrap_or_default()
}

/// A saved hero and world to put back once the loaded game's hero stands.
#[derive(Resource)]
pub struct Restore(pub Box<Hero>, pub Box<World>);

/// A loaded save's scripts and what its sectors kept of their creatures, which its game's load
/// takes ([`crate::game`]'s `begin`).
#[derive(Resource)]
pub struct Resume(pub Scripts, pub crate::npcs::Creatures);

/// Loads the save `name`: a game of its hero starts of its scripts and its sectors' creatures,
/// whose hero and world [`restore`] puts back.
pub fn load(world: &mut bevy::prelude::World, name: &str) -> Result<Header> {
    let dir = directory(world.get_resource::<SettingsFile>()).context("no directory for saves")?;
    let (header, hero, scripts, mut kept) = read(&file(&dir, name)?)?;
    // The game starts in the hero's sector, which [`restore`] puts it back in.
    let cell = crate::paths::cell_of(hero.at.0, hero.at.1);
    let side = i32::try_from(opensacred_data::world::SIDE).unwrap_or(64);
    let sector = |c: i32| u32::try_from(c.div_euclid(side)).unwrap_or(0);
    let game = NewGame {
        class: Some(header.class),
        name: Some(header.hero.clone()),
        difficulty: Some(header.difficulty),
        sector: Some((sector(cell.0), sector(cell.1))),
        ..NewGame::default()
    };
    let game = world
        .get_resource::<crate::game::Shows>()
        .copied()
        .unwrap_or_default()
        .fill(game);
    // Its start shows no window, and the saved game's are left to the player.
    world.remove_resource::<crate::talk::CloseFirst>();
    world.insert_resource(crate::game::Request(game));
    let creatures = std::mem::take(&mut kept.creatures);
    // The hero of a game that the load leaves is not the save's: [`restore`] waits for the load's
    // own, which `hero::leave` would mark only a frame later, as the state changes.
    if let Some(mut ready) = world.get_resource_mut::<crate::hero::Ready>() {
        ready.0 = false;
    }
    world.insert_resource(Restore(Box::new(hero), Box::new(kept)));
    world.insert_resource(Resume(scripts, creatures));
    world
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Loading);
    info!("saves: loading {}", header.name);
    Ok(header)
}

/// Puts a loaded save's hero back, once its hero stands in place and animates (`hero::Ready`),
/// its values made at its spawn: its values, items, arts, combos, statistics and the calendar,
/// after the resets of a game's start, and the hero where it stood; then its world
/// ([`restore_world`]).
fn restore(world: &mut bevy::prelude::World) {
    let mut heroes = world.query_filtered::<Entity, With<Body>>();
    let Some(entity) = heroes.iter(world).next() else {
        return;
    };
    let Some(Restore(hero, kept)) = world.remove_resource::<Restore>() else {
        return;
    };
    let hero = *hero;
    if let Ok(mut entity) = world.get_entity_mut(entity) {
        if let Some(mut base) = entity.get_mut::<crate::fight::Base>() {
            base.0 = hero.base;
        }
        // Its health at its level, so that no level-up fills it (`fight::level_up`).
        if let Some(mut health) = entity.get_mut::<Health>() {
            health.current = hero.health;
            health.level = hero.experience.level;
        }
        entity.insert(hero.effects);
    }
    // The taskbar's "+" as the hero's points left it (`0x585260`, #1992).
    world.insert_resource(crate::points::Plus::loaded(&hero.experience));
    world.insert_resource(crate::fight::Experience(hero.experience));
    if let Some(mut arts) = world.get_resource_mut::<arts::HeroArts>() {
        arts.arts = hero.arts;
    }
    world.insert_resource(hero.slots);
    world.insert_resource(making::Selected(hero.selected));
    world.insert_resource(hero.combos);
    if let Some(mut pack) = world.get_resource_mut::<Pack>() {
        pack.restore(hero.pack);
    }
    world.insert_resource(hero.sets);
    world.insert_resource(HeroHome(hero.home));
    world.insert_resource(hero.tally);
    world.insert_resource(hero.kills);
    world.insert_resource(hero.history);
    world.insert_resource(drops::Played::of(hero.played));
    world.insert_resource(hero.clock);
    // The scripts' timers count on from the clock put back, not the minutes across its change.
    if let Some(mut live) = world.get_non_send_mut::<crate::scripts::Live>() {
        live.clock_set();
    }
    if let Err(e) = world.run_system_cached(refit) {
        warn!("saves: making the hero's values anew: {e}");
    }
    let position = Vec3::new(hero.at.0, 0.0, hero.at.1);
    if let Err(e) = crate::command::teleport(world, position, Arrival::On(Some(hero.floor))) {
        warn!("saves: putting the hero back: {e}");
    }
    info!("saves: the hero is back");
    restore_world(world, *kept);
}

/// Puts a loaded save's world back after the resets of a game's start, its sectors' creatures put
/// back as they come in (`npcs::Npcs::remember`): the town stash, the merchants' stock, the land
/// explored, the map's pin and the weather, its timers on this run's real time; the drops, laid
/// once the tables are read (`drops::Again`); and the hero's followers beside it.
fn restore_world(world: &mut bevy::prelude::World, kept: World) {
    let World {
        creatures: _,
        followers,
        drops: laid,
        stash,
        wares,
        land,
        sectors,
        pin,
        weather,
    } = kept;
    world.insert_resource(stash);
    world.insert_resource(Wares(wares));
    world.insert_resource(Land(land));
    world.insert_resource(MapSectors(sectors));
    if let Some(mut map) = world.get_resource_mut::<Map>() {
        map.pin_again(pin);
    }
    let now = world
        .get_resource::<Time<Real>>()
        .map_or(0.0, |time| time.elapsed_secs_f64() * 1_000.0);
    if let (Some(mut state), Some(mut weather)) =
        (weather, world.get_resource_mut::<crate::weather::Weather>())
    {
        state.rebase(now);
        weather.state = state;
    }
    world.insert_resource(drops::Again(laid));
    crate::npcs::follow_again(world, followers);
    info!("saves: the world is back");
}

/// The loaded hero's values made anew at its level with its effects, as a point spent makes them
/// (`statistics::refit`): its attack, defence, damage, resistances, regeneration, most health
/// and its arts' skills.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn refit(
    experience: Res<crate::fight::Experience>,
    rules: Res<crate::fight::Rules>,
    mut heroes: Query<
        (
            &crate::fight::Base,
            Option<&crate::fight::Holding>,
            Option<&Effects>,
            &mut crate::fight::Combat,
            &mut crate::fight::Regenerates,
            &mut Health,
        ),
        With<Body>,
    >,
    (mut arts, mut hero): (
        Option<ResMut<arts::HeroArts>>,
        Option<ResMut<crate::hero::Hero>>,
    ),
) {
    for (base, holding, effects, mut combat, mut regenerates, mut health) in &mut heroes {
        crate::statistics::refit(
            experience.level,
            &rules,
            (base, holding, effects),
            (&mut combat, &mut regenerates, &mut health),
            (arts.as_deref_mut(), hero.as_deref_mut()),
        );
    }
}

/// The quick save's name and its file, the slot 0 (`cEngine_receiveEvent`, `0x618E3A`), which F8
/// loads.
pub const QUICK_SAVE: &str = "QUICKSAVE";
const QUICK_FILE: &str = "GAME00";

/// The file of the first slot from 1 that no save holds (`0x619B80`: `GAME01` and on, the slot
/// 511 where every one to 509 is taken), which an autosave takes.
pub(crate) fn free_slot(dir: &Path) -> String {
    (1..0x1FE_u16)
        .map(|slot| format!("GAME{slot:02}"))
        .find(|stem| !dir.join(format!("{stem}.{EXTENSION}")).exists())
        .unwrap_or_else(|| "GAME511".to_owned())
}

/// The numbers of the scripts' `AutoSave`s run (`scripts`; 121, `0x4AC740`; #1989), which
/// [`autosave`] saves the game by.
#[derive(Resource, Default)]
pub struct AutoSaves(pub Vec<u32>);

/// Saves the game where the scripts ran an `AutoSave` and the autosave is on (`[gameplay]
/// autosave`, the original's `AUTOSAVE`, `0x81B6F0`): into the first free slot, named by the text
/// of its number (`cEngine::save`, `0x619BF0`), whatever windows show, as the record asks the
/// engine without the autosave's test of the interface (`0x618FA0`).
fn autosave(world: &mut bevy::prelude::World) {
    let numbers = std::mem::take(&mut world.resource_mut::<AutoSaves>().0);
    let Some(&number) = numbers.last() else {
        return;
    };
    let on = world
        .get_resource::<crate::settings::Settings>()
        .is_some_and(|settings| settings.gameplay.autosave);
    if !on {
        info!("saves: the scripts' autosave {number} left, the autosave being off");
        return;
    }
    // Its picture first, as the record's `(0xF, 3)` comes before its save (`0x4AC740`).
    picture_first(world, Waits::Auto(number));
}

/// The autosave of `number`, its picture taken.
fn autosave_now(world: &mut bevy::prelude::World, number: u32) {
    let name = world
        .get_resource::<crate::lines::Texts>()
        .and_then(|texts| texts.named(&number.to_string()))
        .map_or_else(|| number.to_string(), str::to_owned);
    let Some(dir) = directory(world.get_resource::<SettingsFile>()) else {
        warn!("saves: no directory for the autosave {number}");
        return;
    };
    if let Err(e) = save_in(world, &free_slot(&dir), &name) {
        warn!("saves: the autosave {number}: {e:#}");
    }
}

/// The windows whose showing refuses a save (`0x75ADB0`): the merchant's, the smith's, the combat
/// master's and the town stash's (the players' trade, its fifth, the game has not).
pub type Refusing<'w> = (
    Option<Res<'w, crate::merchant::Merchant>>,
    Option<Res<'w, crate::blacksmith::Blacksmith>>,
    Option<Res<'w, crate::master::Master>>,
    Option<Res<'w, crate::town_stash::Opened>>,
);

/// Whether the interface allows a save: none of the `refusing` windows shows (`0x75ADB0`).
pub fn allowed((merchant, smith, master, stash): &Refusing) -> bool {
    merchant.is_none()
        && smith.is_none()
        && master.is_none()
        && stash.as_ref().is_none_or(|stash| stash.0.is_none())
}

/// A quick save or its load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Quick {
    Save,
    Load,
}

/// The quick save or load that its key asked for this frame, which [`quick`] carries out, as the
/// original's key sets the engine's request that its loop carries out (`+0x54` bit `0x40`).
#[derive(Resource, Default)]
struct Asked(Option<Quick>);

/// F9 and Control+Z ask for the quick save where the interface allows one, F8 and Control+X for
/// its load (`cEngine_receiveEvent`, `0x618090`; docs/re/saves.md, "When it saves").
fn quick_keys(mut keys: crate::controls::KeysMut, refusing: Refusing, mut asked: ResMut<Asked>) {
    if keys.take(crate::controls::Action::QuickSave) {
        if allowed(&refusing) {
            asked.0 = Some(Quick::Save);
        } else {
            info!("saves: no quick save while a trader's or a stash's window shows");
        }
    } else if keys.take(crate::controls::Action::QuickLoad) {
        asked.0 = Some(Quick::Load);
    }
}

/// Carries out the quick save or load asked for, as the console's `save` and `load` do, the save
/// once its picture is taken, as F9 takes it right before (`0x618E3A`).
fn quick(world: &mut bevy::prelude::World) {
    let Some(asked) = world.resource_mut::<Asked>().0.take() else {
        return;
    };
    match asked {
        Quick::Save => picture_first(world, Waits::Quick),
        Quick::Load => {
            if let Err(e) = load(world, QUICK_FILE) {
                warn!("saves: the quick load: {e:#}");
            }
        }
    }
}

/// A save that waits for its picture (#1991): the quick save or an autosave of its number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Waits {
    Quick,
    Auto(u32),
}

/// The save waiting for its picture, the pictures taken as it began to wait, and the frames it
/// has waited.
#[derive(Resource, Default)]
struct Waiting(Option<(Waits, u32, u32)>);

/// The frames a save waits for its picture at most, after which it saves with the last taken.
const PICTURE_WAIT: u32 = 30;

/// Takes the save's picture, and has `save` wait for it ([`waited`]).
fn picture_first(world: &mut bevy::prelude::World, save: Waits) {
    let since = world
        .get_resource::<crate::save_picture::Taken>()
        .map_or(0, |taken| taken.count);
    crate::save_picture::take_in(world);
    world.resource_mut::<Waiting>().0 = Some((save, since, 0));
}

/// Saves the save that waited, once its picture is taken or it has waited long enough.
fn waited(world: &mut bevy::prelude::World) {
    let Some((save, since, frames)) = world.resource_mut::<Waiting>().0.take() else {
        return;
    };
    let taken = world
        .get_resource::<crate::save_picture::Taken>()
        .map_or(0, |taken| taken.count);
    if taken <= since && frames < PICTURE_WAIT {
        world.resource_mut::<Waiting>().0 = Some((save, since, frames + 1));
        return;
    }
    match save {
        Waits::Quick => {
            if let Err(e) = save_in(world, QUICK_FILE, QUICK_SAVE) {
                warn!("saves: the quick save: {e:#}");
            }
        }
        Waits::Auto(number) => autosave_now(world, number),
    }
}

/// Forgets a load that did not get as far as its game (the menu again before its hero stood), so
/// that no later game takes its hero or its scripts.
fn forget(mut commands: Commands) {
    commands.remove_resource::<Restore>();
    commands.remove_resource::<Resume>();
    commands.remove_resource::<drops::Again>();
}

/// The saves' restoring of a loaded game's hero.
pub struct SavesPlugin;

impl Plugin for SavesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Asked>()
            .init_resource::<AutoSaves>()
            .init_resource::<Waiting>()
            .add_systems(OnEnter(AppState::Menu), forget)
            .add_systems(
                Update,
                restore
                    .after(Step::HeroSpawn)
                    .run_if(in_state(AppState::InGame))
                    .run_if(resource_exists::<Restore>)
                    .run_if(|ready: Option<Res<crate::hero::Ready>>| ready.is_some_and(|r| r.0)),
            )
            // Not while a load puts its game back, whose hero a save would not hold yet.
            .add_systems(
                Update,
                (quick_keys, quick, autosave, waited)
                    .chain()
                    .run_if(in_state(AppState::InGame))
                    .run_if(not(resource_exists::<Restore>)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_quick_keys_ask_for_a_save_but_under_a_stash_and_for_its_load() {
        use bevy::input::ButtonState;
        use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
        let mut app = App::new();
        app.add_plugins((bevy::MinimalPlugins, bevy::input::InputPlugin))
            .init_resource::<Asked>()
            .add_systems(Update, quick_keys);
        app.update();
        let press = |app: &mut App, keys: &[KeyCode]| {
            for state in [ButtonState::Pressed, ButtonState::Released] {
                for &key in keys {
                    app.world_mut().write_message(KeyboardInput {
                        key_code: key,
                        logical_key: Key::F9,
                        state,
                        text: None,
                        repeat: false,
                        window: Entity::PLACEHOLDER,
                    });
                }
                app.update();
            }
            app.world_mut().resource_mut::<Asked>().0.take()
        };
        assert_eq!(press(&mut app, &[KeyCode::F9]), Some(Quick::Save));
        assert_eq!(
            press(&mut app, &[KeyCode::ControlLeft, KeyCode::KeyZ]),
            Some(Quick::Save)
        );
        assert_eq!(press(&mut app, &[KeyCode::F8]), Some(Quick::Load));
        assert_eq!(
            press(&mut app, &[KeyCode::ControlRight, KeyCode::KeyX]),
            Some(Quick::Load)
        );
        // The town stash's window refuses the save, not the load.
        app.insert_resource(crate::town_stash::Opened(Some(Entity::PLACEHOLDER)));
        assert_eq!(press(&mut app, &[KeyCode::F9]), None);
        assert_eq!(press(&mut app, &[KeyCode::F8]), Some(Quick::Load));
        app.insert_resource(crate::town_stash::Opened(None));
        assert_eq!(press(&mut app, &[KeyCode::F9]), Some(Quick::Save));
    }

    #[test]
    fn an_autosave_takes_the_first_slot_from_1_that_no_save_holds() {
        let dir = scratch("slots");
        assert_eq!(free_slot(&dir), "GAME01");
        for stem in ["GAME00", "GAME01", "GAME03"] {
            fs::write(dir.join(format!("{stem}.{EXTENSION}")), b"").unwrap();
        }
        assert_eq!(free_slot(&dir), "GAME02");
        fs::write(dir.join(format!("GAME02.{EXTENSION}")), b"").unwrap();
        assert_eq!(free_slot(&dir), "GAME04");
        fs::remove_dir_all(&dir).unwrap();
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "opensacred-game-saves-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn header(name: &str, saved: u64) -> Header {
        Header {
            version: VERSION,
            name: name.to_owned(),
            class: Class::Dwarf,
            hero: "Dwarf".to_owned(),
            difficulty: Difficulty::Silver,
            level: 7,
            clock: 8.0 * 3600.0,
            saved,
        }
    }

    fn hero() -> Hero {
        Hero {
            at: (1234.5, 6789.0),
            floor: 1,
            experience: Experience::default(),
            base: opensacred_rules::fight::Base::default(),
            health: 99,
            effects: Effects::default(),
            arts: Arts::default(),
            slots: ArtSlots::default(),
            selected: Some(1078),
            combos: HeroCombos::default(),
            pack: Pack::default().kept(),
            sets: WeaponSets::default(),
            home: Some(Teleport {
                cell: (3236, 2511),
                layer: 1,
                heading: None,
            }),
            tally: Tally::default(),
            kills: Kills::default(),
            history: History::default(),
            played: 125.0,
            clock: Clock::at(21, 30),
        }
    }

    /// The scripts of a game in the middle of its quests: a variable, a quest set up, a timer, a
    /// function done, the `rand()` on, two batches of the play's records of the world, a trigger
    /// opened and a quest's item taken.
    fn saved_scripts() -> Scripts {
        use opensacred_data::netscript::{Body, Operand, Record, Value};
        let mut state = opensacred_rules::quests::Scripts::default();
        state.variables.set("played", 3);
        state.quests.set_up(1202, &mut |_| true);
        state.timers.set(7, "zeit1202", 4320);
        let record = |kind, name: &str| Record {
            kind,
            body: Body::Operands(vec![Operand {
                tag: 0x01,
                value: Value::Str(name.to_owned()),
            }]),
        };
        Scripts {
            seed: 0x5EED,
            scripts: crate::scripts::Saved {
                state,
                done: vec!["Region3Init".to_owned()],
                rng: opensacred_rules::spawning::Rng::of(1, 2, 3),
            },
            npcs: crate::npcs::Saved {
                records: vec![
                    vec![record(1, "res:giver"), record(3, "chest")],
                    vec![record(5, "res:giver")],
                ],
                opened: ["gate".to_owned()].into(),
                taken: [12].into(),
                ..crate::npcs::Saved::default()
            },
        }
    }

    /// The world of a game played a while: gold in the town stash, the land about a cell explored
    /// and its sector entered, the map's pin and the weather (the creatures', the followers' and
    /// the drops' parts are `npcs`' and `drops`' tests').
    fn saved_world() -> World {
        let mut world = World::default();
        let gold = opensacred_rules::drops::gold_pile(250);
        world.stash.items.push((1, gold));
        let stamp = opensacred_rules::explored::Stamp::at(600, false);
        world.land.stamp((3200, 2500), stamp);
        world.sectors.enter((50, 39));
        world.pin = Some((Vec2::new(10.0, 20.0), Vec2::new(3200.0, 2500.0)));
        world.weather = Some(opensacred_rules::weather::Weather::default());
        world
    }

    #[test]
    fn a_save_reads_back_as_written_and_lists_the_newest_first() {
        let dir = scratch("round");
        let parts = (&hero(), &saved_scripts(), &saved_world());
        let picture = Picture([0x0F, 0xF0].repeat(128 * 96));
        write(
            &file(&dir, "first").unwrap(),
            (&header("first", 10), Some(&picture)),
            parts,
        )
        .unwrap();
        write(
            &file(&dir, "second one").unwrap(),
            (&header("second one", 20), None),
            parts,
        )
        .unwrap();
        // The picture after the header, read with it.
        let bytes = fs::read(file(&dir, "first").unwrap()).unwrap();
        assert_eq!(super::header(&bytes).unwrap().1, Some(picture));
        let (read_header, read_hero, read_scripts, read_world) =
            read(&file(&dir, "first").unwrap()).unwrap();
        assert_eq!(read_header, header("first", 10));
        assert_eq!(read_hero.at, (1234.5, 6789.0));
        assert_eq!((read_hero.floor, read_hero.health), (1, 99));
        assert_eq!(read_hero.selected, Some(1078));
        assert_eq!(read_hero.home.map(|home| home.cell), Some((3236, 2511)));
        assert!((read_hero.clock.minutes() - (21.0 * 60.0 + 30.0)).abs() < 1e-6);
        // The scripts whole: their state, the functions done, the `rand()` and the placements'.
        assert_eq!(read_scripts, saved_scripts());
        // The world whole: the stash's gold, the land and the sectors, the pin, the weather.
        assert_eq!(read_world, saved_world());
        assert!(read_world.land.explored((3200, 2500)));
        assert!(read_world.sectors.entered((50, 39)));
        let names: Vec<String> = list(&dir).into_iter().map(|(name, _)| name).collect();
        assert_eq!(names, ["second one", "first"]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_save_of_another_version_or_a_bad_name_is_refused() {
        let dir = scratch("version");
        let old = Header {
            version: VERSION + 1,
            ..header("old", 1)
        };
        let parts = (&hero(), &saved_scripts(), &World::default());
        write(&file(&dir, "old").unwrap(), (&old, None), parts).unwrap();
        let error = format!("{:#}", read(&file(&dir, "old").unwrap()).unwrap_err());
        assert!(error.contains("version"), "{error}");
        assert!(list(&dir).is_empty());
        // A save of the version before reads, without a picture.
        let unpictured = Header {
            version: UNPICTURED,
            ..header("unpictured", 2)
        };
        let mut bytes = postcard::to_stdvec(&unpictured).unwrap();
        bytes.extend(postcard::to_stdvec(&hero()).unwrap());
        bytes.extend(postcard::to_stdvec(&saved_scripts()).unwrap());
        bytes.extend(postcard::to_stdvec(&World::default()).unwrap());
        fs::write(file(&dir, "unpictured").unwrap(), &bytes).unwrap();
        assert_eq!(
            read(&file(&dir, "unpictured").unwrap()).unwrap().0,
            unpictured
        );
        assert_eq!(super::header(&bytes).unwrap().1, None);
        for bad in ["", "../up", "a/b", "a.sav"] {
            assert!(file(&dir, bad).is_err(), "{bad}");
        }
        fs::remove_dir_all(&dir).unwrap();
    }
}
