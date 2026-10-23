//! The game's states (docs/10 §4): the menu, a game loading for a chosen hero, and the game,
//! which start and end inside the process. What every game shares (the world's index, the
//! sprites, the tables) is read once, in `main`; what a game starts from ([`NewGame`]) is loaded
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

use crate::{
    ai, arts, atmosphere, daynight, fight, hero, iso, lines, motions, music, npcs, sectors, sound,
    terrain,
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
        }
    }

    /// The texture of the class's portrait: `gui_qbp_<class>` of `texture.pak`.
    pub fn portrait(self) -> &'static str {
        match self {
            Class::Seraphim => "gui_qbp_sera",
            Class::Gladiator => "gui_qbp_glad",
            Class::Mage => "gui_qbp_mage",
            Class::WoodElf => "gui_qbp_welf",
            Class::DarkElf => "gui_qbp_delf",
            Class::Daemon => "gui_qbp_dae",
            Class::Dwarf => "gui_qbp_dwa",
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
        }
    }

    /// The hero's type: its `Creature.pak` record, whose id is also the `items.pak` slot of the
    /// class's own model (1 `SERAPHIM.GRN` … 9 `Daemonia.grn`), and the type of the class's
    /// template (`heroes.ron`; docs/02 §6.2).
    pub fn creature(self) -> u32 {
        match self {
            Class::Seraphim => 1,
            Class::Gladiator => 2,
            Class::Mage => 3,
            Class::DarkElf => 4,
            Class::WoodElf => 5,
            Class::Dwarf => 8,
            Class::Daemon => 9,
        }
    }

    /// The model's `.GRN` name, the idle, walk and run clips and the footstep profile.
    fn preset(self) -> (&'static str, [&'static str; 3], &'static str) {
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
                "AMAZONE.GRN",
                ["AMAZ_IDLE_BH", "AMAZ_WALK_1H", "AMAZ_RUN_1H"],
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
    /// Walk rather than run at the start (the R key toggles it)
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
    clock: daynight::Clock,
    hero_light: daynight::HeroLight,
    daemon: ai::Daemon,
    texts: lines::Texts,
    rules: fight::Rules,
    arts: arts::HeroArts,
    zoom: f32,
    music: Option<(music::Music, atmosphere::Atmosphere)>,
}

/// The record of creature `id` in `Creature.pak`: the hero's speeds (see `hero::Hero`) and the
/// values of its fight.
fn record(assets: &Path, id: u32) -> Result<opensacred_data::tables::Creature> {
    let path = assets
        .join("tables")
        .join(opensacred_data::tables::CREATURES);
    let text =
        std::fs::read_to_string(&path).map_err(|e| anyhow!("reading {}: {e}", path.display()))?;
    let creatures: Vec<opensacred_data::tables::Creature> =
        ron::from_str(&text).map_err(|e| anyhow!("parsing {}: {e}", path.display()))?;
    creatures
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| anyhow!("no creature {id} in {}", path.display()))
}

/// The template of the hero of `class` (`heroes.ron`, docs/02 §9): the items it wears and holds
/// at the start and its combat arts; none where the index or the class's template is lacking.
fn template(assets: &Path, class: Class) -> Option<opensacred_data::heroes::HeroEntry> {
    std::fs::read_to_string(assets.join("heroes").join(opensacred_data::heroes::INDEX))
        .ok()
        .and_then(|text| ron::from_str::<opensacred_data::heroes::Heroes>(&text).ok())
        .and_then(|index| {
            index
                .heroes
                .into_iter()
                .find(|hero| hero.class == class.creature())
        })
}

/// Loads the game `game` would start: the hero at its start, the first sector built, the start
/// script's NPCs and objects, the footsteps, the music.
fn load(shared: &sectors::Shared, game: &NewGame, speed: f32) -> Result<Loaded> {
    let assets = &shared.assets;
    let class = game.class.unwrap_or_default();
    let tree = game.tree.clone().unwrap_or_else(|| class.tree().into());
    let side = u32::try_from(opensacred_data::world::SIDE)?;
    // The sector given, at its centre, or the start script's start cell.
    let start = match game.sector {
        Some(sector) => Err(sector),
        None => match npcs::start_position(assets, &tree)? {
            Some(cell) => Ok(cell),
            None => Err((50, 39)),
        },
    };
    let at = match start {
        Ok((x, y)) => (x / side, y / side),
        Err(sector) => sector,
    };
    let first =
        sectors::build(shared, at)?.ok_or_else(|| anyhow!("no sector at {},{}", at.0, at.1))?;
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
    let worn = template
        .as_ref()
        .map(|hero| hero.worn.clone())
        .unwrap_or_default();
    let npcs = npcs::Npcs::read(
        assets,
        &tree,
        &worn.iter().map(|worn| worn.item).collect::<Vec<_>>(),
    )?;
    let holding = fight::Holding(npcs.hero_holds(&worn));
    let (held_class, held) = npcs.hero_gear(&worn);
    bevy::log::debug!("game: the hero holds {held:?}, of the class {held_class}");
    let (grn, [idle, walk, run], footsteps) = class.preset();
    let model = match &game.hero {
        Some(model) => model.clone(),
        None => npcs::model_files(assets)?
            .remove(grn)
            .ok_or_else(|| anyhow!("no model {grn} in {}", assets.display()))?,
    };
    let profile = game.footsteps.clone().unwrap_or_else(|| footsteps.into());
    // Its idle, walk and run for what it holds by its model's slots (`0x5467A0`), the class's
    // presets where the model's slots cannot be read; its fight stance and its attacks for what
    // it holds, and the fractions of its attacks at which they hit.
    let slots = motions::Slots::read(&assets.join(&model)).ok();
    let clip = |action, preset: &str| {
        slots
            .as_ref()
            .and_then(|slots| slots.clip(action, held_class))
            .map_or_else(|| preset.to_owned(), |(name, _)| name.to_owned())
    };
    let idle = game
        .idle
        .clone()
        .unwrap_or_else(|| clip(motions::Action::Stand, idle));
    let walk = game
        .walk
        .clone()
        .unwrap_or_else(|| clip(motions::Action::Walk, walk));
    let run = game
        .run
        .clone()
        .unwrap_or_else(|| clip(motions::Action::Run, run));
    let footsteps = sound::Footsteps::read(assets, [&walk, &run], &profile)?;
    let stance = slots
        .as_ref()
        .and_then(|slots| slots.stance(held_class).map(str::to_owned));
    let hits = npcs::hit_fractions(assets);
    let dying = slots
        .as_ref()
        .map(|slots| {
            [173, 174]
                .iter()
                .filter_map(|&slot| slots.slot(slot).map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let attacks = slots
        .map(|slots| slots.attacks(held_class))
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
    // The balance's values of the fight, and the hero's own by its type's record and its
    // template's items (#486).
    let balance =
        std::fs::read_to_string(assets.join("tables").join(opensacred_data::tables::BALANCE))
            .ok()
            .and_then(|text| ron::from_str::<opensacred_data::tables::Balance>(&text).ok());
    let rules = fight::Rules::of(balance.as_ref());
    let record = record(assets, game.creature.unwrap_or(class.creature()))?;
    let mut hero = hero::Hero::new(model, [idle, walk, run], center);
    hero.stance = stance;
    hero.attacks = attacks;
    hero.held = held;
    let base = fight::Base::of(&record);
    let skills = fight::skills(&base, fight::HERO_LEVEL, true);
    // Its template's combat arts that no rune taught, with their records' times shortened by its
    // skills (#506, #507).
    let arts = arts::HeroArts::new_game(
        assets,
        class.creature(),
        template.as_ref().map_or(&[][..], |hero| &hero.arts),
        skills.clone(),
        balance.as_ref(),
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
    hero.dying = dying;
    hero.speeds = record.speed.map(|s| f32::from(s) * hero.pace * speed);
    if game.walking {
        hero.gait = hero::Gait::Walk;
    }
    hero.target = game.target.map(|(dx, dy)| center + iso::ground(dx, dy));
    let music = if game.no_music {
        None
    } else {
        Some((
            music::Music::read(assets)?,
            atmosphere::Atmosphere::read(assets)?,
        ))
    };
    Ok(Loaded {
        npcs,
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
        rules,
        arts,
        zoom: game.zoom.unwrap_or(1.0),
        music,
    })
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
        let file = "ui/screens/loading0.png";
        Self(assets.join(file).is_file().then(|| file.into()))
    }
}

/// Starts loading the requested game, behind the original's loading screen, or a word where
/// there is none.
fn begin(
    mut commands: Commands,
    request: Res<Request>,
    data: Res<Data>,
    speed: Res<hero::GameSpeed>,
    screen: Res<LoadingScreen>,
    assets: Res<AssetServer>,
) {
    let (shared, game, speed) = (Arc::clone(&data.0), request.0.clone(), speed.0);
    let task = AsyncComputeTaskPool::get().spawn(async move { load(&shared, &game, speed) });
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
    commands.insert_resource(loaded.clock);
    commands.insert_resource(loaded.hero_light);
    commands.insert_resource(loaded.daemon);
    commands.insert_resource(loaded.texts);
    commands.insert_resource(loaded.rules);
    commands.insert_resource(loaded.arts);
    commands.insert_resource(Zoom(loaded.zoom));
    if let Some((music, atmosphere)) = loaded.music {
        commands.insert_resource(music);
        commands.insert_resource(atmosphere);
    }
    next.set(AppState::InGame);
}

/// The camera that every game and menu draws through, and the light.
fn setup(mut commands: Commands) {
    commands.spawn((iso::camera(Vec3::ZERO, 1.0), daynight::Night::default()));
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
    camera: Single<(&mut Transform, &mut Projection), With<Camera3d>>,
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
        // The templates' types (docs/02 §6.2), but the vampiress's.
        assert!(types.iter().all(|t| [1, 2, 3, 4, 5, 8, 9].contains(t)));
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
    fn a_games_systems_run_only_while_it_is_played() {
        #[derive(Resource, Default)]
        struct Runs(u32);
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
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
