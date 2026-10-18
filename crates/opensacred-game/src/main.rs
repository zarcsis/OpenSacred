//! `opensacred-game`: the game on Bevy (PLAN.md §6.2), reading only the converted assets of
//! `assets/`. The start of M2 (PLAN.md §10, step 7): the original's isometric view of one
//! sector's ground and the hero model with an animation, at any resolution.

mod atmosphere;
mod benchmark;
mod hero;
mod hud;
mod iso;
mod liquids;
mod music;
mod npcs;
#[cfg(test)]
mod scene_tests;
mod screenshot;
mod sectors;
mod shadows;
mod sound;
mod statics;
mod terrain;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};
use bevy_kira_audio::AudioApp;
use clap::{Parser, ValueEnum};

/// The heroes whose models have their own clips: the model, the clips of standing, walking and
/// running, the portrait and the footstep profile of each.
#[derive(Clone, Copy, ValueEnum)]
enum Class {
    Seraphim,
    Gladiator,
    Mage,
    WoodElf,
    DarkElf,
    Daemon,
    Dwarf,
}

impl Class {
    /// The hero's script tree (`netscript/<tree>.nsc`), whose start script places the hero, the
    /// NPCs and the objects.
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

    /// The model's `.GRN` name, the idle, walk and run clips, the portrait's texture name and
    /// the footstep profile.
    fn preset(self) -> (&'static str, [&'static str; 3], &'static str, &'static str) {
        const FEMALE: &str = "footsteps_hero_female";
        const MALE: &str = "footsteps_hero_male";
        match self {
            Class::Seraphim => (
                "SERAPHIM.GRN",
                ["SERA_IDLE_BH", "SERA_WALK_1H", "SERA_RUN_1H"],
                "gui_qbp_sera",
                FEMALE,
            ),
            Class::Gladiator => (
                "GLADIATOR.GRN",
                ["GLAD_IDLE_BH", "GLAD_WALK_BH", "GLAD_RUN_1H"],
                "gui_qbp_glad",
                MALE,
            ),
            Class::Mage => (
                "MAGICIAN.GRN",
                ["MAGE_IDLE_BH", "MAGE_WALK_BH", "MAGE_RUN_1H"],
                "gui_qbp_mage",
                MALE,
            ),
            Class::WoodElf => (
                "AMAZONE.GRN",
                ["AMAZ_IDLE_BH", "AMAZ_WALK_1H", "AMAZ_RUN_1H"],
                "gui_qbp_welf",
                FEMALE,
            ),
            Class::DarkElf => (
                "DARKELVE.GRN",
                ["DELV_IDLE_1H", "DELV_WALK_BH", "DELV_RUN_BH"],
                "gui_qbp_delf",
                MALE,
            ),
            Class::Daemon => (
                "DAEMONIA.GRN",
                ["DAEM_IDLE_BH", "DAEM_WALK_BH", "DAEM_RUN_BH"],
                "gui_qbp_dae",
                FEMALE,
            ),
            Class::Dwarf => (
                "DWARF.GRN",
                ["DWAR_IDLE_BH", "DWAR_WALK_1H", "DWAR_RUN_1H"],
                "gui_qbp_dwa",
                MALE,
            ),
        }
    }
}

#[derive(Parser)]
#[command(
    version = env!("OPENSACRED_VERSION"),
    about = "OpenSacred: the game on the converted assets"
)]
struct Cli {
    /// Directory of the converted assets (opensacred-convert all)
    #[arg(long, default_value = "assets")]
    assets: PathBuf,
    /// The sector to start in, as X,Y on the 100 × 100 grid, at its centre; by default the
    /// hero's start in its start script
    #[arg(long, value_parser = parse_position)]
    sector: Option<(u32, u32)>,
    /// Window width in physical pixels
    #[arg(long, default_value_t = 1920)]
    width: u32,
    /// Window height in physical pixels
    #[arg(long, default_value_t = 1080)]
    height: u32,
    /// The camera's zoom at the start, 0.5 to 2 as in the original (the mouse wheel changes it)
    #[arg(long, default_value_t = 1.0)]
    zoom: f32,
    /// The hero: its model, clips, portrait and footsteps, which the options below override
    #[arg(long, value_enum, default_value = "seraphim")]
    class: Class,
    /// The hero model, relative to the asset directory
    #[arg(long)]
    hero: Option<String>,
    /// The hero's clip when standing
    #[arg(long)]
    idle: Option<String>,
    /// The hero's clip when walking
    #[arg(long)]
    walk: Option<String>,
    /// The hero's clip when running
    #[arg(long)]
    run: Option<String>,
    /// Walk rather than run at the start (the R key toggles it)
    #[arg(long)]
    walking: bool,
    /// The hero's creature record in Creature.pak, for its speeds
    #[arg(long, default_value_t = 1)]
    creature: u32,
    /// The script tree whose start script places the hero, the NPCs and the objects
    /// (netscript/<TREE>.nsc); by default the class's
    #[arg(long)]
    tree: Option<String>,
    /// The hero's portrait in the HUD, relative to the asset directory
    #[arg(long)]
    portrait: Option<String>,
    /// The hero's footstep sound profile
    #[arg(long)]
    footsteps: Option<String>,
    /// No music and no ambient loops
    #[arg(long)]
    no_music: bool,
    /// Send the hero this many cells along X,Y (screen right-down, left-down) at the start
    #[arg(long, value_parser = parse_offset, allow_hyphen_values = true)]
    target: Option<(f32, f32)>,
    /// Save one frame to this PNG once the scene is ready, then exit
    #[arg(long)]
    screenshot: Option<PathBuf>,
    /// Frames to render between the scene being ready and the screenshot or the recording
    #[arg(long, default_value_t = 30)]
    frames: u32,
    /// Save that many frames to DIR/frame-NNNN.png once the scene is ready, then exit
    #[arg(long, value_name = "DIR")]
    record: Option<PathBuf>,
    /// Frames to save with --record
    #[arg(long, default_value_t = 90)]
    record_frames: u32,
    /// Render without vsync for this many seconds, print the frame rate and exit
    #[arg(long, value_name = "SECONDS")]
    benchmark: Option<f32>,
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

/// The walk and run speeds of creature `id` (see `hero::Hero`).
fn speeds(assets: &std::path::Path, id: u32) -> Result<[f32; 2]> {
    let path = assets
        .join("tables")
        .join(opensacred_data::tables::CREATURES);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;
    let creatures: Vec<opensacred_data::tables::Creature> =
        ron::from_str(&text).map_err(|e| anyhow::anyhow!("parsing {}: {e}", path.display()))?;
    let creature = creatures
        .iter()
        .find(|c| c.id == id)
        .ok_or_else(|| anyhow::anyhow!("no creature {id} in {}", path.display()))?;
    Ok(creature.speed.map(|s| f32::from(s) * iso::MODEL_SCALE))
}

/// The cell nearest to `cell` in `sector` that lies outside a building (cell flag bit 1, a room's
/// interior) and in front of it on screen (further down, `x + y` greater): the hero would stand
/// under a roof that the game does not lift yet, or behind the walls (docs/decisions.md). `cell`
/// itself where it is outside, or where no cell of the sector is.
fn outside(sector: &opensacred_data::world::Sector, (x, y): (u32, u32)) -> (u32, u32) {
    const INTERIOR: u8 = 0x2;
    let side = opensacred_data::world::SIDE;
    let (Ok(sx), Ok(sy)) = (
        usize::try_from(sector.x).map(|v| v * side),
        usize::try_from(sector.y).map(|v| v * side),
    ) else {
        return (x, y);
    };
    let (Ok(cx), Ok(cy)) = (usize::try_from(x), usize::try_from(y)) else {
        return (x, y);
    };
    let inside = |c: usize, r: usize| {
        sector
            .cells
            .get(r * side + c)
            .is_some_and(|cell| cell.flags & INTERIOR != 0)
    };
    let (Some(c0), Some(r0)) = (cx.checked_sub(sx), cy.checked_sub(sy)) else {
        return (x, y);
    };
    if c0 >= side || r0 >= side || !inside(c0, r0) {
        return (x, y);
    }
    let mut best: Option<(usize, usize, usize)> = None;
    for r in 0..side {
        for c in 0..side {
            if inside(c, r) || c + r <= c0 + r0 {
                continue;
            }
            let d = c.abs_diff(c0).pow(2) + r.abs_diff(r0).pow(2);
            if best.is_none_or(|(_, _, b)| d < b) {
                best = Some((c, r, d));
            }
        }
    }
    match best {
        Some((c, r, _)) => (
            u32::try_from(sx + c).unwrap_or(x),
            u32::try_from(sy + r).unwrap_or(y),
        ),
        None => (x, y),
    }
}

fn light(mut commands: Commands) {
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

/// The zoom at the start.
#[derive(Resource)]
struct Zoom(f32);

fn camera(mut commands: Commands, hero: Res<hero::Hero>, zoom: Res<Zoom>) {
    commands.spawn(iso::camera(hero.position, zoom.0));
}

fn run(cli: Cli) -> Result<AppExit> {
    let assets = std::path::absolute(&cli.assets)
        .map_err(|e| anyhow::anyhow!("resolving {}: {e}", cli.assets.display()))?;
    let shared = sectors::Shared {
        world: terrain::read_world(&assets)?,
        sprites: statics::read_sprites(&assets)?,
        assets: assets.clone(),
    };
    let tree = cli.tree.unwrap_or_else(|| cli.class.tree().into());
    let side = u32::try_from(opensacred_data::world::SIDE)?;
    // The sector given, at its centre, or the start script's start cell.
    let start = match cli.sector {
        Some(sector) => Err(sector),
        None => match npcs::start_position(&assets, &tree)? {
            Some(cell) => Ok(cell),
            None => Err((50, 39)),
        },
    };
    let at = match start {
        Ok((x, y)) => (x / side, y / side),
        Err(sector) => sector,
    };
    let first = sectors::build(&shared, at)?
        .ok_or_else(|| anyhow::anyhow!("no sector at {},{}", at.0, at.1))?;
    let mut heights = terrain::Heights::default();
    heights.add(&first.sector);
    let center = match start {
        Ok(cell) => {
            let (x, y) = outside(&first.sector, cell);
            let mut position = iso::ground(x as f32 + 0.5, y as f32 + 0.5);
            position.y = heights.at(position.x, position.z).unwrap_or(0.0);
            position
        }
        Err(_) => terrain::center(&first.sector),
    };
    let (grn, [idle, walk, run], portrait, footsteps) = cli.class.preset();
    let model = match cli.hero {
        Some(model) => model,
        None => npcs::model_files(&assets)?
            .remove(grn)
            .ok_or_else(|| anyhow::anyhow!("no model {grn} in {}", assets.display()))?,
    };
    let portrait = match cli.portrait {
        Some(portrait) => portrait,
        None => hud::find_texture(&assets, portrait)
            .and_then(|path| {
                path.strip_prefix(&assets)
                    .ok()
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
            })
            .unwrap_or_default(),
    };
    let footsteps_profile = cli.footsteps.unwrap_or_else(|| footsteps.into());
    let idle = cli.idle.unwrap_or_else(|| idle.into());
    let walk = cli.walk.unwrap_or_else(|| walk.into());
    let run = cli.run.unwrap_or_else(|| run.into());
    let footsteps = sound::Footsteps::read(&assets, [&walk, &run], &footsteps_profile)?;
    let mut hero = hero::Hero::new(model, [idle, walk, run], center);
    hero.speeds = speeds(&assets, cli.creature)?;
    if cli.walking {
        hero.gait = hero::Gait::Walk;
    }
    hero.target = cli.target.map(|(dx, dy)| center + iso::ground(dx, dy));
    let streamer = sectors::Streamer::new(shared, at, first);
    let npcs = npcs::Npcs::read(&assets, &tree)?;
    let (npc_count, object_count) = npcs.counts();
    println!("{tree}: {npc_count} NPCs and {object_count} objects in the start script");
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "OpenSacred".into(),
                    // The size in physical pixels, whatever the desktop's scaling.
                    resolution: WindowResolution::new(cli.width, cli.height)
                        .with_scale_factor_override(1.0),
                    present_mode: if cli.benchmark.is_some() {
                        PresentMode::AutoNoVsync
                    } else {
                        PresentMode::AutoVsync
                    },
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                file_path: assets.to_string_lossy().into_owned(),
                ..default()
            }),
    )
    .add_plugins((bevy_kira_audio::AudioPlugin, statics::StaticsPlugin))
    .insert_resource(ClearColor(Color::BLACK))
    .insert_resource(footsteps)
    .insert_resource(streamer)
    .insert_resource(npcs)
    .insert_resource(heights)
    .insert_resource(liquids::Liquids::read(&assets)?)
    .insert_resource(Zoom(cli.zoom))
    .insert_resource(hero)
    .insert_resource(hud::Hud {
        portrait,
        cursor: hud::cursor_file(&assets),
    })
    .init_resource::<hero::Ready>()
    .add_systems(
        Startup,
        (
            camera,
            light,
            hero::load,
            sound::load,
            shadows::setup,
            hud::setup,
            hud::cursor,
        ),
    )
    .add_systems(
        Update,
        (
            hero::spawn,
            hero::animate,
            iso::zoom,
            hero::toggle,
            hero::steer,
            hero::run,
            sound::step,
            hero::follow,
            sectors::stream,
            npcs::place,
            npcs::animate,
            shadows::add,
            shadows::follow,
        )
            .chain(),
    )
    .add_systems(Update, (hud::scale, hud::gait, liquids::animate));
    if !cli.no_music {
        app.add_audio_channel::<atmosphere::Channel>()
            .add_audio_channel::<music::Channel>()
            .insert_resource(music::Music::read(&assets)?)
            .insert_resource(atmosphere::Atmosphere::read(&assets)?)
            .add_systems(Update, (music::play, atmosphere::choose));
    }
    if let Some(seconds) = cli.benchmark {
        app.insert_resource(benchmark::Benchmark::new(seconds))
            .add_systems(Update, benchmark::measure);
    }
    if let Some(dir) = cli.record {
        std::fs::create_dir_all(&dir)
            .map_err(|e| anyhow::anyhow!("creating {}: {e}", dir.display()))?;
        app.insert_resource(screenshot::Recording::new(
            dir,
            cli.frames,
            cli.record_frames,
        ))
        .add_systems(Update, screenshot::record);
    }
    if let Some(path) = cli.screenshot {
        app.insert_resource(screenshot::Request::new(path, cli.frames))
            .add_systems(Update, screenshot::take);
    }
    Ok(app.run())
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(AppExit::Success) => ExitCode::SUCCESS,
        Ok(AppExit::Error(code)) => ExitCode::from(code.get()),
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}
