//! The saves (#1095; docs/re/saves.md): a game kept in a file of its own and given back, as the
//! original's `cEngine::save` keeps its game in `SAVE/GAMEnn.PAK`, in the game's own versioned
//! format (PLAN.md ADR-08). A save is postcard: its [`Header`], which the saves' list reads alone,
//! then its [`Hero`] and its [`Scripts`]; a save of another version is refused, as the original's
//! load refuses one (`0x61B6F0`). The saves lie in `saves/` beside the settings' file.
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
//! and the play's records of the world placed again ([`crate::npcs::Npcs::resume`]). The world
//! (#1959) is a later part.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail, ensure};
use bevy::prelude::{
    App, Commands, Entity, IntoScheduleConfigs, NextState, OnEnter, Plugin, Query, Res, ResMut,
    Resource, Transform, Update, Vec3, With, World, in_state, info, resource_exists, warn,
};
use opensacred_rules::arts::Arts;
use opensacred_rules::drops::Difficulty;
use opensacred_rules::fight::Experience;
use serde::{Deserialize, Serialize};

use crate::art_slots::ArtSlots;
use crate::combos::HeroCombos;
use crate::daynight::Clock;
use crate::effects::Effects;
use crate::fight::Health;
use crate::frame::Step;
use crate::game::{AppState, Class, NewGame};
use crate::hero::{Arrival, Body};
use crate::pickup::{self, Pack};
use crate::scripts::{HeroHome, Teleport};
use crate::sets::WeaponSets;
use crate::settings::SettingsFile;
use crate::statistics::{History, Kills, Tally};
use crate::terrain::Heights;
use crate::{arts, drops, making};

/// The saves' version: a save of another is refused.
pub const VERSION: u32 = 2;
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

/// Writes a save of `header`, `hero` and `scripts` at `path`.
pub fn write(path: &Path, header: &Header, hero: &Hero, scripts: &Scripts) -> Result<()> {
    let mut bytes = postcard::to_stdvec(header).context("writing the header")?;
    bytes.extend(postcard::to_stdvec(hero).context("writing the hero")?);
    bytes.extend(postcard::to_stdvec(scripts).context("writing the scripts")?);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("making {}", dir.display()))?;
    }
    fs::write(path, bytes).with_context(|| format!("writing {}", path.display()))
}

/// A save's header and the bytes after it, refused where its version is not [`VERSION`].
fn header(bytes: &[u8]) -> Result<(Header, &[u8])> {
    let (header, rest) =
        postcard::take_from_bytes::<Header>(bytes).context("reading the header")?;
    if header.version != VERSION {
        bail!(
            "a save of the version {}, where the game reads {VERSION}",
            header.version
        );
    }
    Ok((header, rest))
}

/// Reads the save at `path`.
pub fn read(path: &Path) -> Result<(Header, Hero, Scripts)> {
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let (header, rest) = header(&bytes).with_context(|| path.display().to_string())?;
    let (hero, rest) = postcard::take_from_bytes(rest)
        .with_context(|| format!("{}: reading the hero", path.display()))?;
    let scripts = postcard::from_bytes(rest)
        .with_context(|| format!("{}: reading the scripts", path.display()))?;
    Ok((header, hero, scripts))
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
            let (header, _) = header(&bytes).ok()?;
            let stem = path.file_stem()?.to_string_lossy().into_owned();
            Some((stem, header))
        })
        .collect();
    saves.sort_by(|a, b| b.1.saved.cmp(&a.1.saved).then_with(|| a.0.cmp(&b.0)));
    saves
}

/// The hero and the game as they stand, to save as `name`.
fn gather(world: &mut World, name: &str) -> Result<(Header, Hero)> {
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
fn scripts(world: &World) -> Result<Scripts> {
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

/// Saves the game as `name`: where its file went.
pub fn save(world: &mut World, name: &str) -> Result<PathBuf> {
    let dir = directory(world.get_resource::<SettingsFile>()).context("no directory for saves")?;
    let path = file(&dir, name)?;
    let (header, hero) = gather(world, name)?;
    write(&path, &header, &hero, &scripts(world)?)?;
    info!("saves: {} saved in {}", header.name, path.display());
    Ok(path)
}

/// The saves of this run's directory.
pub fn saves(world: &World) -> Vec<(String, Header)> {
    directory(world.get_resource::<SettingsFile>())
        .map(|dir| list(&dir))
        .unwrap_or_default()
}

/// A saved hero to put back once the loaded game's hero stands.
#[derive(Resource)]
pub struct Restore(pub Box<Hero>);

/// A loaded save's scripts, which its game's load takes ([`crate::game`]'s `begin`).
#[derive(Resource)]
pub struct Resume(pub Scripts);

/// Loads the save `name`: a game of its hero starts of its scripts, whose values [`restore`]
/// puts back.
pub fn load(world: &mut World, name: &str) -> Result<Header> {
    let dir = directory(world.get_resource::<SettingsFile>()).context("no directory for saves")?;
    let (header, hero, scripts) = read(&file(&dir, name)?)?;
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
    world.insert_resource(Restore(Box::new(hero)));
    world.insert_resource(Resume(scripts));
    world
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Loading);
    info!("saves: loading {}", header.name);
    Ok(header)
}

/// Puts a loaded save's hero back, once its hero stands in place and animates (`hero::Ready`),
/// its values made at its spawn: its values, items, arts, combos, statistics and the calendar,
/// after the resets of a game's start, and the hero where it stood.
fn restore(world: &mut World) {
    let mut heroes = world.query_filtered::<Entity, With<Body>>();
    let Some(entity) = heroes.iter(world).next() else {
        return;
    };
    let Some(Restore(hero)) = world.remove_resource::<Restore>() else {
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

/// Forgets a load that did not get as far as its game (the menu again before its hero stood), so
/// that no later game takes its hero or its scripts.
fn forget(mut commands: Commands) {
    commands.remove_resource::<Restore>();
    commands.remove_resource::<Resume>();
}

/// The saves' restoring of a loaded game's hero.
pub struct SavesPlugin;

impl Plugin for SavesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Menu), forget)
            .add_systems(
                Update,
                restore
                    .after(Step::HeroSpawn)
                    .run_if(in_state(AppState::InGame))
                    .run_if(resource_exists::<Restore>)
                    .run_if(|ready: Option<Res<crate::hero::Ready>>| ready.is_some_and(|r| r.0)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn a_save_reads_back_as_written_and_lists_the_newest_first() {
        let dir = scratch("round");
        write(
            &file(&dir, "first").unwrap(),
            &header("first", 10),
            &hero(),
            &saved_scripts(),
        )
        .unwrap();
        write(
            &file(&dir, "second one").unwrap(),
            &header("second one", 20),
            &hero(),
            &saved_scripts(),
        )
        .unwrap();
        let (read_header, read_hero, read_scripts) = read(&file(&dir, "first").unwrap()).unwrap();
        assert_eq!(read_header, header("first", 10));
        assert_eq!(read_hero.at, (1234.5, 6789.0));
        assert_eq!((read_hero.floor, read_hero.health), (1, 99));
        assert_eq!(read_hero.selected, Some(1078));
        assert_eq!(read_hero.home.map(|home| home.cell), Some((3236, 2511)));
        assert!((read_hero.clock.minutes() - (21.0 * 60.0 + 30.0)).abs() < 1e-6);
        // The scripts whole: their state, the functions done, the `rand()` and the placements'.
        assert_eq!(read_scripts, saved_scripts());
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
        write(&file(&dir, "old").unwrap(), &old, &hero(), &saved_scripts()).unwrap();
        let error = format!("{:#}", read(&file(&dir, "old").unwrap()).unwrap_err());
        assert!(error.contains("version"), "{error}");
        assert!(list(&dir).is_empty());
        for bad in ["", "../up", "a/b", "a.sav"] {
            assert!(file(&dir, bad).is_err(), "{bad}");
        }
        fs::remove_dir_all(&dir).unwrap();
    }
}
