//! The game's states (docs/10 §4): the menu, a game loading for a chosen hero, and the game,
//! which start and end inside the process. What every game shares (the world's index, the
//! sprites, in `main`; the tables, when first asked for, `tables`) is read once; what a game
//! starts from ([`NewGame`]) is loaded
//! on Bevy's async compute pool behind a loading screen, and each module takes back what it made
//! when the game is left (`OnExit(AppState::InGame)`). The camera and the light stay for every
//! game: the interface of the menus draws through the same camera.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use bevy::prelude::*;
use bevy::tasks::futures::check_ready;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use bevy::text::FontSize;
use clap::{Args, ValueEnum};

use crate::files;
use crate::tables::Tables;
use crate::{
    ai, arsenal, arts, atmosphere, daynight, fight, forms, hero, iso, lines, making, motions,
    music, npcs, scripts, sectors, sound, terrain,
};

/// Where the process is.
#[derive(States, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AppState {
    /// The main menu, before a game and after one.
    #[default]
    Menu,
    /// A game loading.
    Loading,
    /// A game.
    InGame,
}

/// The heroes whose models have their own clips: the model, the clips of standing, walking and
/// running and the footstep profile of each.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Class {
    #[default]
    Seraphim,
    Gladiator,
    Mage,
    WoodElf,
    DarkElf,
    Daemon,
    Dwarf,
    Vampiress,
}

impl Class {
    /// The class's name in the menus.
    pub fn name(self) -> &'static str {
        match self {
            Class::Seraphim => "Seraphim",
            Class::Gladiator => "Gladiator",
            Class::Mage => "Mage",
            Class::WoodElf => "Wood Elf",
            Class::DarkElf => "Dark Elf",
            Class::Daemon => "Daemon",
            Class::Dwarf => "Dwarf",
            Class::Vampiress => "Vampiress",
        }
    }

    /// The hero's script tree (`netscript/<tree>.nsc`), whose start script places the hero and
    /// whose scripts the NPCs and the objects (`npcs`).
    fn tree(self) -> &'static str {
        match self {
            Class::Seraphim => "type_npc_seraphim",
            Class::Gladiator => "type_npc_gladiator",
            Class::Mage => "type_npc_magician",
            Class::WoodElf => "type_npc_elve",
            Class::DarkElf => "type_npc_darkelve",
            Class::Daemon => "type_npc_daemonin",
            Class::Dwarf => "type_npc_zwerg",
            Class::Vampiress => "type_npc_vampirelady",
        }
    }

    /// The hero's type: its `Creature.pak` record, whose id is also the `items.pak` slot of the
    /// class's own model (1 `SERAPHIM.GRN` … 9 `Daemonia.grn`), and the type of the class's
    /// template (`heroes.ron`; docs/02 §6.2): the Vampiress's by day, 6, her night's form 7 the
    /// art 1023's (#758).
    pub fn creature(self) -> u32 {
        match self {
            Class::Seraphim => 1,
            Class::Gladiator => 2,
            Class::Mage => 3,
            Class::DarkElf => 4,
            Class::WoodElf => 5,
            Class::Vampiress => 6,
            Class::Dwarf => 8,
            Class::Daemon => 9,
        }
    }

    /// The item that the class has in a slot of its own whenever that slot is empty, and the slot
    /// (`0x426580`; docs/re/equipment.md, "Worn items and the classes' defaults"): the
    /// Seraphim's hair, the Gladiator's belt, the Battle Mage's hood, the Vampiress's hair and
    /// the Dwarf's goggles.
    pub fn default_item(self) -> Option<(u8, u32)> {
        match self {
            Class::Seraphim => Some((0, 4007)),
            Class::Gladiator => Some((2, 4054)),
            Class::Mage => Some((0, 3219)),
            Class::Vampiress => Some((0, 4028)),
            Class::Dwarf => Some((0, 3018)),
            Class::WoodElf | Class::DarkElf | Class::Daemon => None,
        }
    }

    /// The model's `.GRN` name, the idle, walk and run clips and the footstep profile.
    pub fn preset(self) -> (&'static str, [&'static str; 3], &'static str) {
        const FEMALE: &str = "footsteps_hero_female";
        const MALE: &str = "footsteps_hero_male";
        match self {
            Class::Seraphim => (
                "SERAPHIM.GRN",
                ["SERA_IDLE_BH", "SERA_WALK_1H", "SERA_RUN_1H"],
                FEMALE,
            ),
            Class::Gladiator => (
                "GLADIATOR.GRN",
                ["GLAD_IDLE_BH", "GLAD_WALK_BH", "GLAD_RUN_1H"],
                MALE,
            ),
            Class::Mage => (
                "MAGICIAN.GRN",
                ["MAGE_IDLE_BH", "MAGE_WALK_BH", "MAGE_RUN_1H"],
                MALE,
            ),
            Class::WoodElf => (
                "ELVE_SORCERESS.GRN",
                ["ELFF_IDLE_BH", "ELFF_WALK_1H", "ELFF_RUN_1H"],
                FEMALE,
            ),
            Class::DarkElf => (
                "DARKELVE.GRN",
                ["DELV_IDLE_1H", "DELV_WALK_BH", "DELV_RUN_BH"],
                MALE,
            ),
            Class::Daemon => (
                "DAEMONIA.GRN",
                ["DAEM_IDLE_BH", "DAEM_WALK_BH", "DAEM_RUN_BH"],
                FEMALE,
            ),
            Class::Dwarf => (
                "DWARF.GRN",
                ["DWAR_IDLE_BH", "DWAR_WALK_1H", "DWAR_RUN_1H"],
                MALE,
            ),
            Class::Vampiress => (
                "VLADY_D.GRN",
                ["VMPD_IDLE_BH", "VMPD_WALK_1H", "VMPD_RUN_1H"],
                FEMALE,
            ),
        }
    }
}

/// What a game starts from: the hero's class and the options that belong to a game, on the
/// command line and, later, in the console's `new-game`.
#[derive(Args, Clone, Debug, Default)]
pub struct NewGame {
    /// The hero: its model, clips and footsteps, which the options below override (the
    /// Seraphim by default)
    #[arg(long, value_enum)]
    pub class: Option<Class>,
    /// The sector to start in, as X,Y on the 100 x 100 grid, at its centre; by default the
    /// hero's start in its start script
    #[arg(long, value_parser = parse_position)]
    pub sector: Option<(u32, u32)>,
    /// The camera's zoom at the start, 0.5 to 2 as in the original: 2 shows twice the world (1
    /// by default; the mouse wheel changes it)
    #[arg(long)]
    pub zoom: Option<f32>,
    /// The hero model, relative to the asset directory
    #[arg(long)]
    pub hero: Option<String>,
    /// The hero's clip when standing
    #[arg(long)]
    pub idle: Option<String>,
    /// The hero's clip when walking
    #[arg(long)]
    pub walk: Option<String>,
    /// The hero's clip when running
    #[arg(long)]
    pub run: Option<String>,
    /// Walk rather than run, as though Shift were held (the console's `gait run` lets it go)
    #[arg(long)]
    pub walking: bool,
    /// The hero's creature record in Creature.pak, for its speeds (by default the class's own)
    #[arg(long)]
    pub creature: Option<u32>,
    /// The script tree (`netscript/<tree>.nsc`) whose start script places the hero and whose
    /// scripts the NPCs and the objects; by default the class's
    #[arg(long)]
    pub tree: Option<String>,
    /// The hero's footstep sound profile
    #[arg(long)]
    pub footsteps: Option<String>,
    /// No music and no ambient loops
    #[arg(long)]
    pub no_music: bool,
    /// Send the hero this many cells along X,Y (screen right-down, left-down) at the start
    #[arg(long, value_parser = parse_offset, allow_hyphen_values = true)]
    pub target: Option<(f32, f32)>,
    /// The time of day at the start, as HH:MM (08:00, as in the original)
    #[arg(long, value_parser = daynight::parse_time)]
    pub time: Option<daynight::Clock>,
    /// The hero's name (by default the class's own, as the choice of the hero gives it)
    #[arg(long)]
    pub name: Option<String>,
    /// The difficulty: bronze (by default), silver, gold, platinum or niobium
    #[arg(long, value_parser = parse_difficulty)]
    pub difficulty: Option<opensacred_rules::drops::Difficulty>,
}

/// The game's difficulty (`0x182EE44`, docs/re/fight.md, "The difficulty"), which its rules take
/// (#1086): bronze unless the game started on another.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Difficulty(pub opensacred_rules::drops::Difficulty);

/// The names of the difficulties on the command line, bronze's first.
pub const DIFFICULTIES: [&str; 5] = ["bronze", "silver", "gold", "platinum", "niobium"];

/// The index of `game`'s difficulty, 0 bronze to 4 niobium, as the scripts test it.
fn difficulty_index(game: &NewGame) -> u8 {
    u8::try_from(game.difficulty.unwrap_or_default().index()).unwrap_or(0)
}

/// A difficulty by its name.
fn parse_difficulty(text: &str) -> Result<opensacred_rules::drops::Difficulty, String> {
    DIFFICULTIES
        .iter()
        .position(|name| name.eq_ignore_ascii_case(text.trim()))
        .map(|index| opensacred_rules::drops::Difficulty::ALL[index])
        .ok_or_else(|| format!("{text}: not one of {}", DIFFICULTIES.join(", ")))
}

impl NewGame {
    /// Whether these options name a game rather than leave it to the menu: any but the zoom and
    /// the music, which are about how it shows.
    pub fn named(&self) -> bool {
        self.class.is_some()
            || self.sector.is_some()
            || self.hero.is_some()
            || self.idle.is_some()
            || self.walk.is_some()
            || self.run.is_some()
            || self.walking
            || self.creature.is_some()
            || self.tree.is_some()
            || self.footsteps.is_some()
            || self.target.is_some()
            || self.time.is_some()
    }
}

/// `x,y` of an offset in cells.
fn parse_offset(text: &str) -> Result<(f32, f32), String> {
    let (x, y) = text.split_once(',').ok_or("expected X,Y")?;
    let number = |s: &str| s.trim().parse::<f32>().map_err(|e| format!("{s}: {e}"));
    Ok((number(x)?, number(y)?))
}

/// `x,y` of a grid position.
fn parse_position(text: &str) -> Result<(u32, u32), String> {
    let (x, y) = text.split_once(',').ok_or("expected X,Y")?;
    let number = |s: &str| s.trim().parse::<u32>().map_err(|e| format!("{s}: {e}"));
    Ok((number(x)?, number(y)?))
}

/// What every game reads: the asset directory, the world's index, the sprites and the shadow
/// atlas, read once at the start.
#[derive(Resource, Clone)]
pub struct Data(pub Arc<sectors::Shared>);

/// The game to load when `AppState::Loading` comes.
#[derive(Resource, Clone, Default)]
pub struct Request(pub NewGame);

/// The command line's options about how a game shows (`--zoom`, `--no-music`), which hold for
/// every game of the run that a `new-game` starts, unless its own line sets them (#831).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct Shows {
    zoom: Option<f32>,
    no_music: bool,
}

impl Shows {
    /// Whether the music is off, in the games and in the menus (`menu_sound`).
    pub fn no_music(self) -> bool {
        self.no_music
    }

    /// How `game` says its games show.
    pub fn of(game: &NewGame) -> Self {
        Self {
            zoom: game.zoom,
            no_music: game.no_music,
        }
    }

    /// `game` with what it leaves unset taken from these.
    pub fn fill(self, mut game: NewGame) -> NewGame {
        game.zoom = game.zoom.or(self.zoom);
        game.no_music |= self.no_music;
        game
    }
}

/// The process ends, failing, when its game does not load: a game that the command line named,
/// for a run that has nothing else to do (a screenshot, a benchmark).
#[derive(Resource)]
pub struct QuitOnFailure;

/// The camera's zoom at a game's start.
#[derive(Resource)]
struct Zoom(f32);

/// A game as loaded off the main thread.
struct Loaded {
    tree: String,
    at: (u32, u32),
    first: sectors::Built,
    heights: terrain::Heights,
    hero: hero::Hero,
    footsteps: sound::Footsteps,
    npcs: npcs::Npcs,
    scripts: scripts::Started,
    clock: daynight::Clock,
    hero_light: daynight::HeroLight,
    daemon: ai::Daemon,
    texts: lines::Texts,
    difficulty: Difficulty,
    rules: fight::Rules,
    arts: arts::HeroArts,
    forms: Option<forms::Forms>,
    arsenal: Option<arsenal::Arsenal>,
    zoom: f32,
    music: Option<(music::Music, atmosphere::Atmosphere)>,
    backpack: crate::pickup::Starting,
    sets: crate::sets::Starting,
    maker: arsenal::Maker,
}

/// The record of creature `id` in `Creature.pak`: the hero's speeds (see `hero::Hero`) and the
/// values of its fight.
pub fn record(tables: &Tables, id: u32) -> Result<opensacred_data::tables::Creature> {
    tables
        .creatures()?
        .iter()
        .find(|c| c.id == id)
        .cloned()
        .ok_or_else(|| {
            anyhow!(
                "no creature {id} in {}",
                tables.path(opensacred_data::tables::CREATURES).display()
            )
        })
}

/// The types of the creatures, the `items.pak` slots of the categories 2, 3 and 32 (`0x426610`).
fn creature_types(tables: &Tables) -> Result<std::collections::HashSet<u32>> {
    Ok(tables
        .items()?
        .iter()
        .filter(|item| matches!(item.category(), 2 | 3 | 32))
        .map(|item| item.slot)
        .collect())
}

/// The template of the hero of `class` (`heroes.ron`, docs/02 §9): the items it wears and holds
/// at the start and its combat arts; none where the index or the class's template is lacking.
fn template(assets: &Path, class: Class) -> Option<opensacred_data::heroes::HeroEntry> {
    files::read_to_string(assets.join("heroes").join(opensacred_data::heroes::INDEX))
        .ok()
        .and_then(|text| ron::from_str::<opensacred_data::heroes::Heroes>(&text).ok())
        .and_then(|index| {
            index
                .heroes
                .into_iter()
                .find(|hero| hero.class == class.creature())
        })
}

/// Loads the game `game` would start, of the seed `seed`: the hero at its start, the first sector
/// built, the start script's NPCs and objects, the footsteps, the music.
fn load(
    shared: &sectors::Shared,
    sound: &opensacred_data::sound::Sound,
    (game, seed): (&NewGame, crate::seed::Game),
    speed: f32,
) -> Result<Loaded> {
    let assets = &shared.assets;
    let tables = &shared.tables;
    let class = game.class.unwrap_or_default();
    let tree = game.tree.clone().unwrap_or_else(|| class.tree().into());
    // The tree's start script and its sectors' and regions' functions, run in Luau (#726, #585),
    // each region's by the areas of the world's sectors.
    let areas: std::collections::HashMap<(u32, u32), u8> = shared
        .world
        .sectors
        .iter()
        .map(|entry| ((entry.x, entry.y), entry.area))
        .collect();
    let scripts::Ran {
        tree: scripts,
        started,
    } = scripts::run(
        assets,
        (&tree, difficulty_index(game)),
        |at| areas.get(&at).copied(),
        creature_types(tables)?,
        seed,
    )?;
    let side = u32::try_from(opensacred_data::world::SIDE)?;
    // The sector given, at its centre, or the start script's start cell.
    let start = match game.sector {
        Some(sector) => Err(sector),
        None => match npcs::start_position(&scripts) {
            Some((cell, _)) => Ok(cell),
            None => Err((50, 39)),
        },
    };
    // The start's floor, where the start script's `StartPosition` names one (#1017).
    let floor = game
        .sector
        .is_none()
        .then(|| npcs::start_position(&scripts).and_then(|(_, floor)| floor))
        .flatten();
    let at = match start {
        Ok((x, y)) => (x / side, y / side),
        Err(sector) => sector,
    };
    let first = sectors::build(
        shared,
        at,
        crate::parallel::Spread::Wide,
        &terrain::Decoded::default(),
    )?
    .ok_or_else(|| anyhow!("no sector at {},{}", at.0, at.1))?;
    let mut heights = terrain::Heights::default();
    heights.add(&first.sector);
    let center = match start {
        Ok((x, y)) => {
            let mut position = iso::ground(x as f32 + 0.5, y as f32 + 0.5);
            position.y = heights
                .standing(position.x, position.z, None)
                .unwrap_or(0.0);
            position
        }
        Err(_) => terrain::start(&first.sector),
    };
    // The hero's template's items (#486): their values in its fight, the class of those in its
    // hands, by which it picks its clips, and their models (#488).
    let template = template(assets, class);
    let backpack = crate::pickup::Starting(
        template
            .as_ref()
            .map(|hero| hero.backpack.clone())
            .unwrap_or_default(),
    );
    let worn = template
        .as_ref()
        .map(|hero| hero.worn.clone())
        .unwrap_or_default();
    // The Seraphim's 1018 puts an item of its own in her hand (#754).
    let seraphim = class == Class::Seraphim && game.hero.is_none();
    let items: Vec<u32> = worn
        .iter()
        .map(|worn| worn.item)
        .chain(seraphim.then_some(opensacred_rules::arts::BFG_ITEM))
        .collect();
    let difficulty = game.difficulty.unwrap_or_default();
    let npcs = npcs::Npcs::read(shared, &scripts, &items, difficulty)?;
    let holding = fight::Holding(npcs.hero_holds(&worn));
    // Its first weapon set, the template's items in its hands (#1347).
    let sets = crate::sets::Starting(
        npcs.hero_hands(&worn)
            .map(|item| item.and_then(|item| worn.iter().find(|worn| worn.item == item).cloned())),
    );
    let (held_class, held) = npcs.hero_gear(&worn);
    bevy::log::debug!("game: the hero holds {held:?}, of the class {held_class}");
    let (grn, presets, footsteps) = class.preset();
    let [idle, walk, run] = presets;
    let model = match &game.hero {
        Some(model) => model.clone(),
        None => npcs::model_files(assets)?
            .remove(grn)
            .ok_or_else(|| anyhow!("no model {grn} in {}", assets.display()))?,
    };
    let profile = game.footsteps.clone().unwrap_or_else(|| footsteps.into());
    let hits = npcs::hit_fractions(tables);
    let points = npcs::hit_points(tables);
    let mut clips = clips_of(
        assets,
        &model,
        [idle, walk, run],
        (class.creature(), held_class),
        &hits,
        &points,
    );
    for (clip, chosen) in clips
        .moving
        .iter_mut()
        .zip([&game.idle, &game.walk, &game.run])
    {
        if let Some(chosen) = chosen {
            clip.clone_from(chosen);
        }
    }
    let [idle, walk, run] = clips.moving.clone();
    let footsteps = sound::Footsteps::read(tables, [&walk, &run], &profile, sound)?;
    // The balance's values of the fight, and the hero's own by its type's record and its
    // template's items (#486).
    let balance = tables.balance().ok();
    let rules = fight::Rules::at(balance, difficulty);
    let record = record(tables, game.creature.unwrap_or(class.creature()))?;
    // The Vampiress's other form, her night's (#758).
    let forms = (class == Class::Vampiress && game.hero.is_none())
        .then(|| {
            let base = fight::Base::of(&record);
            let night = forms::Form::night(shared, &hits, &points, &base)?;
            let steps = sound::Footsteps::read(
                tables,
                [&night.clips.moving[1], &night.clips.moving[2]],
                &profile,
                sound,
            )?;
            let day = forms::Form {
                creature: class.creature(),
                model: model.clone(),
                clips: clips.clone(),
                base,
                speeds: record.speed,
                size: opensacred_rules::minds::size(class.creature(), held_class != 0),
                armed: true,
            };
            anyhow::Ok(forms::Forms::new(day, night, steps))
        })
        .transpose()?;
    // The arms that the heroes' arts change (`arsenal`).
    let kind = class.creature();
    let arms = |worn: &[opensacred_data::heroes::Worn], clips| {
        let (held_class, held) = npcs.hero_gear(worn);
        arsenal::Arms {
            form: forms::Form {
                creature: kind,
                model: model.clone(),
                clips,
                base: fight::Base::of(&record),
                speeds: record.speed,
                size: opensacred_rules::minds::size(kind, held_class != 0),
                armed: true,
            },
            held,
            holds: npcs.hero_holds(worn),
            armed: npcs.hero_armed(worn),
            blade: npcs.hero_blade(worn),
        }
    };
    // The Seraphim's own arms and those of her 1018's effect: her template's items but those in
    // her hands, and the item 3809 in her right hand, a fixed item of its record's values (#754).
    let bfg = seraphim
        .then(|| {
            let words = tables
                .weapons()?
                .iter()
                .find(|weapon| weapon.id == opensacred_rules::arts::BFG_ITEM)
                .map(|weapon| weapon.words_84.clone())
                .ok_or_else(|| anyhow!("no weapon {}", opensacred_rules::arts::BFG_ITEM))?;
            let mut bearing = npcs.hero_put_away(&worn);
            bearing.push(opensacred_data::heroes::Worn {
                id: 0,
                item: opensacred_rules::arts::BFG_ITEM,
                words,
            });
            let (class_borne, _) = npcs.hero_gear(&bearing);
            let borne = clips_of(assets, &model, presets, (kind, class_borne), &hits, &points);
            anyhow::Ok(arms(&bearing, borne))
        })
        .transpose()?;
    // The Gladiator's arms without the weapon his 1013 hurls, and the weapon as it lies, of its
    // template's values (#755).
    let hurled = (class == Class::Gladiator && game.hero.is_none())
        .then(|| npcs.hero_hurls(&worn))
        .flatten()
        .map(|(thrown, held, rest)| {
            let (class_rest, _) = npcs.hero_gear(&rest);
            let bare = clips_of(assets, &model, presets, (kind, class_rest), &hits, &points);
            arsenal::Hurled {
                arms: arms(&rest, bare),
                item: opensacred_rules::drops::Dropped {
                    kind: opensacred_rules::drops::Kind::Weapon,
                    item: thrown.item,
                    level: 1,
                    key: 16,
                    quality: 0,
                    amount: 1,
                    values: Some(fight::ItemValues::of_record(&thrown.words)),
                    named: None,
                    socketed: Vec::new(),
                },
                held,
            }
        });
    let arsenal = (bfg.is_some() || hurled.is_some()).then(|| {
        let mut arsenal = arsenal::Arsenal::new(arms(&worn, clips.clone()));
        if let Some(bfg) = bfg {
            arsenal = arsenal.with_bfg(bfg);
        }
        if let Some(hurled) = hurled {
            arsenal = arsenal.with_hurled(hurled);
        }
        arsenal
    });
    // What makes its arms anew as its hands' items change, a weapon set chosen (#1348).
    let maker = arsenal::Maker {
        assets: assets.to_path_buf(),
        model: model.clone(),
        presets,
        kind,
        record: record.clone(),
        hits: hits.clone(),
        points: points.clone(),
        worn: npcs.hero_put_away(&worn),
        words: tables
            .weapons()?
            .iter()
            .map(|weapon| (weapon.id, weapon.words_84.clone()))
            .collect(),
    };
    let mut hero = hero::Hero::new(model, [idle, walk, run], center);
    hero.floor = floor;
    hero.stance = clips.stance;
    hero.attacks = clips.attacks;
    hero.arts = clips.arts;
    hero.size = opensacred_rules::minds::size(class.creature(), held_class != 0);
    hero.creature = class.creature();
    hero.armed = npcs.hero_armed(&worn);
    hero.blade = npcs.hero_blade(&worn);
    hero.held = held;
    let base = fight::Base::of(&record);
    let skills = fight::hero_skills(&base, fight::HERO_LEVEL, &holding.0);
    // Its template's combat arts that no rune taught, with their records' times shortened by its
    // skills (#506, #507).
    let arts = arts::HeroArts::new_game(
        assets,
        class.creature(),
        template.as_ref().map_or(&[][..], |hero| &hero.arts),
        skills.clone(),
        balance,
    );
    bevy::log::debug!("game: the hero's arts {:?}", arts.lines());
    let values = fight::combat(&base, fight::HERO_LEVEL, holding.0.clone(), &skills, &rules);
    bevy::log::debug!(
        "game: the hero holds and wears {} items: attack {:.2}, defence {:.2}, damage {:?}, \
         resistances {:?}, attack speed {}",
        worn.len(),
        values.attack,
        values.defence,
        values.damage,
        values.resistance,
        values.speed
    );
    // Its walk and run, and their clips' rate, by its pace (`0x4F75F0`, #599).
    hero.pace = values.pace_factor();
    hero.combat = Some(fight::Combat(values));
    let most = fight::hero_health(base.attributes, fight::HERO_LEVEL, &skills, &rules);
    hero.vitals = Some((
        fight::Health {
            current: most,
            most,
            level: fight::HERO_LEVEL,
        },
        fight::Regenerates::new(fight::regeneration(
            &base,
            fight::HERO_LEVEL,
            &skills,
            &rules,
        )),
        base,
        holding,
    ));
    hero.dying = clips.dying;
    hero.pickup = clips.pickup;
    hero.speeds = record.speed.map(|s| f32::from(s) * hero.pace * speed);
    if game.walking {
        hero.walking = true;
        hero.gait = hero::Gait::Walk;
    }
    hero.target = game.target.map(|(dx, dy)| center + iso::ground(dx, dy));
    let music = if game.no_music {
        None
    } else {
        Some((
            music::Music::new(&shared.world, sound),
            atmosphere::Atmosphere::new(sound),
        ))
    };
    Ok(Loaded {
        npcs,
        scripts: started,
        tree,
        at,
        first,
        heights,
        hero,
        footsteps,
        clock: game.time.clone().unwrap_or_default(),
        hero_light: daynight::HeroLight {
            doubled: class == Class::WoodElf,
        },
        daemon: ai::Daemon(class == Class::Daemon),
        texts: lines::Texts::read(assets),
        difficulty: Difficulty(difficulty),
        rules,
        arts,
        forms,
        arsenal,
        zoom: game.zoom.unwrap_or(1.0),
        music,
        backpack,
        sets,
        maker,
    })
}

/// The clips of the model `model` of a hero of type `kind` holding what is of `class`: its idle,
/// walk and run by the model's slots (`0x5467A0`), the `presets` where the slots cannot be read;
/// its fight stance and its attacks, with the fractions of each at which they hit (`hits`); its
/// dying clips, the slots 173 and 174; the clips of its arts' actions with their hit points
/// (`points`, #509); and its pick-up, the slot 172 (#1219).
pub fn clips_of(
    assets: &Path,
    model: &str,
    presets: [&str; 3],
    (kind, class): (u32, u8),
    hits: &std::collections::HashMap<String, [f32; 2]>,
    points: &std::collections::HashMap<String, Vec<f32>>,
) -> forms::Clips {
    let slots = motions::Slots::read(&assets.join(model)).ok();
    let moving = [
        motions::Action::Stand,
        motions::Action::Walk,
        motions::Action::Run,
    ]
    .into_iter()
    .zip(presets)
    .map(|(action, preset)| {
        slots
            .as_ref()
            .and_then(|slots| slots.clip(action, class))
            .map_or_else(|| preset.to_owned(), |(name, _)| name.to_owned())
    })
    .collect::<Vec<_>>();
    let stance = slots
        .as_ref()
        .and_then(|slots| slots.stance(class).map(str::to_owned));
    let arts = making::clips(kind, class, slots.as_ref(), points);
    let dying = slots
        .as_ref()
        .map(|slots| {
            [173, 174]
                .iter()
                .filter_map(|&slot| slots.slot(slot).map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let pickup = slots
        .as_ref()
        .and_then(|slots| slots.pickup().map(str::to_owned));
    let attacks = slots
        .map(|slots| slots.attacks(class))
        .unwrap_or_default()
        .into_iter()
        .map(|name| {
            let fractions = hits
                .get(&name.to_ascii_uppercase())
                .copied()
                .unwrap_or_default();
            (name, fractions)
        })
        .collect();
    forms::Clips {
        moving: [moving[0].clone(), moving[1].clone(), moving[2].clone()],
        stance,
        attacks,
        arts,
        dying,
        pickup,
    }
}

/// The game loading.
#[derive(Resource)]
struct Pending(Task<Result<Loaded>>);

/// The original's interface font (`opensacred-convert ui`).
pub const FONT: &str = "ui/fonts/antqs__.ttf";
/// The interface's gold, in the frame's sRGB values (`hud`).
pub const GOLD: Color = Color::linear_rgb(0.86, 0.74, 0.45);

/// The original's loading screen (`opensacred-convert ui`), 1024 × 768, relative to the asset
/// directory, if it was converted.
#[derive(Resource)]
pub struct LoadingScreen(pub Option<String>);

impl LoadingScreen {
    /// The first of the original's two loading screens of Ancaria (`loading0`, `loading1`).
    pub fn find(assets: &Path) -> Self {
        let file = "ui/screens/loading0.ktx2";
        Self(files::exists(assets.join(file)).then(|| file.into()))
    }
}

/// Starts loading the requested game, behind the original's loading screen, or a word where
/// there is none; the game's seed the run's, the count of its games before it mixed in
/// (`seed::Game`).
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources as arguments"
)]
fn begin(
    mut commands: Commands,
    request: Res<Request>,
    data: Res<Data>,
    index: Res<sound::Index>,
    speed: Res<hero::GameSpeed>,
    screen: Res<LoadingScreen>,
    assets: Res<AssetServer>,
    mut games: Local<u64>,
) {
    let (shared, sound) = (Arc::clone(&data.0), Arc::clone(&index.0));
    let (game, speed) = (request.0.clone(), speed.0);
    let seed = crate::seed::Game::of(*games);
    *games += 1;
    commands.insert_resource(seed);
    let task = AsyncComputeTaskPool::get()
        .spawn(async move { load(&shared, &sound, (&game, seed), speed) });
    commands.insert_resource(Pending(task));
    let mut root = commands.spawn((
        DespawnOnExit(AppState::Loading),
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
    ));
    match &screen.0 {
        // The original's screen, as high as the window (`hud`).
        Some(file) => root.with_child((
            ImageNode::new(crate::hud::image(&assets, file.clone())),
            Node {
                width: Val::Px(1024.0),
                height: Val::Px(768.0),
                ..default()
            },
        )),
        None => root.with_child((
            Text::new("Loading"),
            TextFont {
                font: assets.load(FONT).into(),
                font_size: FontSize::Px(32.0),
                ..default()
            },
            TextColor(GOLD),
        )),
    };
}

/// Puts the loaded game in place and plays it, or logs why it did not load.
fn finish(
    mut commands: Commands,
    pending: Option<ResMut<Pending>>,
    data: Res<Data>,
    quit: Option<Res<QuitOnFailure>>,
    mut next: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut pending) = pending else {
        return;
    };
    let Some(result) = check_ready(&mut pending.0) else {
        return;
    };
    commands.remove_resource::<Pending>();
    let loaded = match result {
        Ok(loaded) => loaded,
        Err(e) => {
            error!("the game did not load: {e:#}");
            if quit.is_some() {
                exit.write(AppExit::error());
            } else {
                next.set(AppState::Menu);
            }
            return;
        }
    };
    let (npcs, objects, closed) = loaded.npcs.counts();
    info!(
        "{}: {npcs} NPCs and {objects} objects at the start, {closed} cells of closed triggers",
        loaded.tree
    );
    commands.insert_resource(sectors::Streamer::new(
        Arc::clone(&data.0),
        loaded.at,
        loaded.first,
    ));
    commands.insert_resource(loaded.heights);
    commands.insert_resource(loaded.hero);
    commands.insert_resource(loaded.footsteps);
    commands.insert_resource(loaded.npcs);
    commands.insert_resource(loaded.scripts);
    commands.insert_resource(loaded.clock);
    commands.insert_resource(loaded.hero_light);
    commands.insert_resource(loaded.daemon);
    commands.insert_resource(loaded.texts);
    commands.insert_resource(loaded.difficulty);
    commands.insert_resource(loaded.rules);
    commands.insert_resource(loaded.arts);
    match loaded.forms {
        Some(forms) => commands.insert_resource(forms),
        None => commands.remove_resource::<forms::Forms>(),
    }
    match loaded.arsenal {
        Some(arsenal) => commands.insert_resource(arsenal),
        None => commands.remove_resource::<arsenal::Arsenal>(),
    }
    commands.insert_resource(Zoom(loaded.zoom));
    commands.insert_resource(loaded.backpack);
    commands.insert_resource(loaded.sets);
    commands.insert_resource(loaded.maker);
    if let Some((music, atmosphere)) = loaded.music {
        commands.insert_resource(music);
        commands.insert_resource(atmosphere);
    }
    next.set(AppState::InGame);
}

/// The camera that every game and menu draws through, and the light.
fn setup(mut commands: Commands) {
    commands
        .spawn((iso::camera(Vec3::ZERO, 1.0), daynight::Night::default()))
        .with_child(iso::untinted());
    commands.insert_resource(GlobalAmbientLight {
        brightness: 600.0,
        ..default()
    });
    // From the upper left of the screen, as the original's sprites are shaded.
    commands.spawn((
        DirectionalLight {
            illuminance: 4_000.0,
            ..default()
        },
        Transform::IDENTITY.looking_to(Vec3::new(1.0, -1.5, -0.2), Vec3::Y),
    ));
}

/// Looks at the hero at the game's zoom.
fn place_camera(
    hero: Res<hero::Hero>,
    zoom: Res<Zoom>,
    camera: Single<(&mut Transform, &mut Projection), crate::iso::Eye>,
) {
    let (mut transform, mut projection) = camera.into_inner();
    *transform = iso::view_of(hero.position);
    if let Projection::Orthographic(ortho) = &mut *projection {
        ortho.scale = zoom.0.clamp(*iso::ZOOM.start(), *iso::ZOOM.end());
    }
}

/// Takes back the resources of the game left; the modules despawn what they spawned.
fn leave(mut commands: Commands) {
    commands.remove_resource::<sectors::Streamer>();
    commands.remove_resource::<terrain::Heights>();
    commands.remove_resource::<hero::Hero>();
    commands.remove_resource::<sound::Footsteps>();
    commands.remove_resource::<npcs::Npcs>();
    commands.remove_resource::<daynight::Clock>();
    commands.remove_resource::<daynight::HeroLight>();
    commands.remove_resource::<Zoom>();
    commands.remove_resource::<music::Music>();
    commands.remove_resource::<atmosphere::Atmosphere>();
}

/// What is placed in the world but the camera and the light, which stay for every game.
type Placed = (
    With<GlobalTransform>,
    Without<Camera>,
    Without<DirectionalLight>,
);

/// Warns of what a game left behind.
fn leftovers(placed: Query<(), Placed>) {
    let count = placed.iter().count();
    if count > 0 {
        warn!("the game left {count} entities in the world");
    }
}

/// The states, the loading of a game, the camera and the light.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .init_resource::<Request>()
            .init_resource::<Shows>()
            .add_systems(Startup, setup)
            .add_systems(OnEnter(AppState::Loading), (leftovers, begin))
            .add_systems(Update, finish.run_if(in_state(AppState::Loading)))
            .add_systems(OnEnter(AppState::InGame), place_camera)
            .add_systems(OnExit(AppState::InGame), leave)
            .add_systems(OnEnter(AppState::Menu), leftovers);
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct Line {
        #[command(flatten)]
        game: NewGame,
    }

    #[test]
    fn a_games_options_parse_and_leave_the_defaults_unset() {
        let parse = |args: &[&str]| Line::parse_from(["game"].iter().chain(args)).game;
        let none = parse(&[]);
        assert_eq!(
            (none.class, none.zoom, none.creature, none.time.is_none()),
            (None, None, None, true)
        );
        let game = parse(&[
            "--class",
            "wood-elf",
            "--walking",
            "--creature",
            "7",
            "--target",
            "-3,4",
            "--sector",
            "50,39",
        ]);
        assert_eq!(game.class, Some(Class::WoodElf));
        assert!(game.walking);
        assert_eq!(game.creature, Some(7));
        assert_eq!(game.target, Some((-3.0, 4.0)));
        assert_eq!(game.sector, Some((50, 39)));
    }

    #[test]
    fn each_class_is_a_heroes_type_of_its_own() {
        use clap::ValueEnum;
        let types: Vec<u32> = Class::value_variants()
            .iter()
            .map(|c| c.creature())
            .collect();
        // The templates' types (docs/02 §6.2).
        assert!(types.iter().all(|t| [1, 2, 3, 4, 5, 6, 8, 9].contains(t)));
        let mut unique = types.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), types.len());
        assert_eq!(Class::Daemon.creature(), 9);
    }

    #[test]
    fn a_game_is_named_by_its_options_but_the_zoom_and_the_music() {
        let parse = |args: &[&str]| Line::parse_from(["game"].iter().chain(args)).game;
        assert!(!parse(&[]).named());
        assert!(!parse(&["--zoom", "1.5", "--no-music"]).named());
        assert!(parse(&["--class", "dwarf"]).named());
        assert!(parse(&["--sector", "50,39"]).named());
        assert!(parse(&["--time", "20:00"]).named());
        assert!(parse(&["--walking"]).named());
    }

    #[test]
    fn a_games_line_keeps_how_it_shows_unless_it_sets_it() {
        let parse = |args: &[&str]| Line::parse_from(["game"].iter().chain(args)).game;
        let shows = Shows::of(&parse(&["--zoom", "1.5", "--no-music"]));
        let game = shows.fill(parse(&["--class", "mage"]));
        assert_eq!((game.zoom, game.no_music), (Some(1.5), true));
        assert_eq!(game.class, Some(Class::Mage));
        assert_eq!(shows.fill(parse(&["--zoom", "0.5"])).zoom, Some(0.5));
        let game = Shows::default().fill(parse(&["--class", "mage"]));
        assert_eq!((game.zoom, game.no_music), (None, false));
    }

    #[test]
    fn a_games_systems_run_only_while_it_is_played() {
        #[derive(Resource, Default)]
        struct Runs(u32);
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .init_resource::<Time>()
            .add_plugins(crate::frame::FramePlugin)
            .init_resource::<Runs>()
            .add_systems(
                Update,
                (|mut runs: ResMut<Runs>| runs.0 += 1).in_set(crate::frame::Step::HeroMove),
            );
        app.update();
        assert_eq!(app.world().resource::<Runs>().0, 0);
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        app.update();
        assert_eq!(app.world().resource::<Runs>().0, 2);
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Menu);
        app.update();
        app.update();
        assert_eq!(app.world().resource::<Runs>().0, 2);
    }
}
