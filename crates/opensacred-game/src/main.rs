//! `opensacred-game`: the game on Bevy (PLAN.md §6.2), reading only the converted assets of
//! `assets/`. The start of M2 (PLAN.md §10, step 7): the original's isometric view of one
//! sector's ground and the hero model with an animation, at any resolution.

mod ai;
mod arrows;
mod atmosphere;
mod attack;
mod benchmark;
mod blended;
mod buildings;
mod command;
mod console;
mod crowd;
mod daynight;
mod doors;
mod fight;
mod frame;
mod game;
mod help;
mod hero;
mod hud;
mod iso;
mod journal;
mod lines;
mod liquids;
mod menu;
mod models;
mod motions;
mod music;
mod npcs;
mod overlay;
mod paths;
mod pause;
mod pick;
mod remote;
#[cfg(test)]
mod scene_tests;
mod screenshot;
mod sectors;
mod settings;
mod shaders;
mod shadows;
mod sound;
mod srgb;
mod statics;
mod terrain;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use anyhow::Result;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::window::{MonitorSelection, PresentMode, WindowResolution};
use clap::Parser;
// The fills' rolls live in the rules' crate (#359); `crate::spawning` names them still.
use opensacred_rules::spawning;

#[derive(Parser)]
#[command(
    version = env!("OPENSACRED_VERSION"),
    about = "OpenSacred: the game on the converted assets"
)]
struct Cli {
    /// Directory of the converted assets (opensacred-convert all)
    #[arg(long, default_value = "assets")]
    assets: PathBuf,
    /// The settings file, instead of settings.toml in the user's configuration directory
    /// (%APPDATA%\OpenSacred on Windows)
    #[arg(long, value_name = "FILE")]
    settings: Option<PathBuf>,
    /// The log file, instead of logs\opensacred.log in the user's state directory
    /// (%LOCALAPPDATA%\OpenSacred on Windows); the two before it are kept with a number before the
    /// extension (opensacred.1.log, opensacred.2.log)
    #[arg(long, value_name = "FILE")]
    log_file: Option<PathBuf>,
    /// A window for this run, whatever the settings say (F11 switches)
    #[arg(long, conflicts_with = "borderless")]
    windowed: bool,
    /// Borderless full screen for this run, whatever the settings say (F11 switches)
    #[arg(long)]
    borderless: bool,
    /// The window's width in physical pixels for this run; the settings' by default
    #[arg(long)]
    width: Option<u32>,
    /// The window's height in physical pixels for this run; the settings' by default
    #[arg(long)]
    height: Option<u32>,
    #[command(flatten)]
    game: game::NewGame,
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
    /// Run these commands, between semicolons, from the start (`help` lists them), for example
    /// "new-game --class mage; wait ready; screenshot a.png; quit"
    #[arg(long, value_name = "COMMANDS")]
    exec: Option<String>,
    /// Run the commands of this file from the start, a line each, after those of --exec
    #[arg(long, value_name = "FILE")]
    script: Option<PathBuf>,
    /// Listen for remote control on this port of 127.0.0.1 (Bevy's Remote Protocol, 15702 by
    /// default); the window then opens without taking the focus
    #[arg(long, value_name = "PORT", num_args = 0..=1, default_missing_value = "15702")]
    remote: Option<u16>,
}

/// Threads of Bevy's compute pool, which runs the systems in parallel. Bevy gives it every
/// logical core that its I/O and async pools leave, 24 of 32 on a development machine; with the
/// main and render threads, the audio's and the rest of the system's there were more threads
/// ready to run than cores, and a thread that lost its core waited for the scheduler's next tick
/// (15.6 ms on Windows): a few frames a run took 30-130 ms, every thread of the process stalled
/// with them and the sound crackled (#2). With four, none does, and the frames come faster.
const COMPUTE_THREADS: usize = 4;

/// Bevy's task pools, the compute pool limited to [`COMPUTE_THREADS`].
fn task_pools() -> TaskPoolOptions {
    let mut options = TaskPoolOptions::default();
    options.compute.max_threads = COMPUTE_THREADS;
    options
}

fn run(cli: Cli) -> Result<AppExit> {
    let assets = std::path::absolute(&cli.assets)
        .map_err(|e| anyhow::anyhow!("resolving {}: {e}", cli.assets.display()))?;
    let world = terrain::read_world(&assets)?;
    let dungeons = daynight::Dungeons::of(&world);
    // A converted texture of `texture.pak`, relative to the asset directory.
    let texture = |name: &str| {
        hud::find_texture(&assets, name).and_then(|path| {
            path.strip_prefix(&assets)
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
        })
    };
    // The lamps' glow, `PARTICLE_GLOW01`, and the statics' shadows, `SHADOW_TREE00`.
    let glow = texture("particle_glow01").unwrap_or_default();
    let shadows = texture("shadow_tree00");
    let shared = sectors::Shared {
        world,
        sprites: statics::read_sprites(&assets)?,
        assets: assets.clone(),
        shadows: shadows.clone(),
    };
    let (settings_file, mut settings) = settings::SettingsFile::read(cli.settings);
    // A screenshot, a recording and a benchmark run in a window of the given size, so that their
    // frames do not depend on the monitor.
    let fixed = cli.screenshot.is_some() || cli.record.is_some() || cli.benchmark.is_some();
    if cli.windowed || fixed {
        settings.display.mode = settings::Mode::Windowed;
    }
    if cli.borderless {
        settings.display.mode = settings::Mode::Borderless;
    }
    settings.display.width = cli.width.unwrap_or(settings.display.width);
    settings.display.height = cli.height.unwrap_or(settings.display.height);
    let log_file = cli
        .log_file
        .or_else(|| settings::state_dir().map(|dir| dir.join("logs").join("opensacred.log")));
    let mut app = App::new();
    // Before Bevy's plugins, whose `LogPlugin` takes the log's layer from it.
    app.insert_resource(journal::Journal::new(log_file));
    app.add_plugins(
        DefaultPlugins
            .set(LogPlugin {
                custom_layer: journal::layer,
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "OpenSacred".into(),
                    // The size in physical pixels, whatever the desktop's scaling.
                    resolution: WindowResolution::new(
                        settings.display.width,
                        settings.display.height,
                    )
                    .with_scale_factor_override(1.0),
                    mode: settings.display.window_mode(MonitorSelection::Primary),
                    present_mode: if cli.benchmark.is_some() {
                        PresentMode::AutoNoVsync
                    } else {
                        PresentMode::AutoVsync
                    },
                    // Driven from afar, it leaves the focus to whoever works on the desktop.
                    focused: cli.remote.is_none(),
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                file_path: assets.to_string_lossy().into_owned(),
                ..default()
            })
            .set(TaskPoolPlugin {
                task_pool_options: task_pools(),
            }),
    )
    // Each module's plugin registers its own systems, resources and observers; one line a
    // module, in alphabetical order.
    // Bevy takes 15 plugins at a time.
    .add_plugins((
        ai::AiPlugin,
        arrows::ArrowsPlugin,
        atmosphere::AtmospherePlugin,
        attack::AttackPlugin,
        benchmark::BenchmarkPlugin,
        bevy_kira_audio::AudioPlugin,
        blended::BlendedPlugin,
        buildings::BuildingsPlugin,
        command::CommandPlugin,
        console::ConsolePlugin,
        crowd::CrowdPlugin,
        daynight::DayNightPlugin,
        doors::DoorsPlugin,
        fight::FightPlugin,
        frame::FramePlugin,
    ))
    .add_plugins((
        game::GamePlugin,
        help::HelpPlugin,
        hero::HeroPlugin,
        hud::HudPlugin,
        journal::JournalPlugin,
        lines::LinesPlugin,
        liquids::LiquidsPlugin,
        menu::MenuPlugin,
        models::ModelsPlugin,
        music::MusicPlugin,
        npcs::NpcsPlugin,
        overlay::OverlayPlugin,
        pause::PausePlugin,
        screenshot::ScreenshotPlugin,
        sectors::SectorsPlugin,
    ))
    .add_plugins((
        settings::SettingsPlugin,
        shaders::ShaderLibrary,
        shadows::ShadowsPlugin,
        sound::SoundPlugin,
        srgb::SrgbPlugin,
        statics::StaticsPlugin,
        terrain::GroundPlugin,
    ))
    .insert_resource(console::Console::new(console::history_file(&settings_file)))
    .insert_resource(settings)
    .insert_resource(settings_file)
    .insert_resource(game::Data(Arc::new(shared)))
    .insert_resource(daynight::LightTypes::read(&assets)?)
    .insert_resource(dungeons)
    .insert_resource(hero::GameSpeed::read(&assets)?)
    .insert_resource(statics::TreeFade::read(&assets)?)
    .insert_resource(daynight::GlowTexture(glow))
    .insert_resource(ClearColor(Color::BLACK))
    .insert_resource(liquids::Liquids::read(&assets)?)
    .insert_resource(hud::Hud {
        cursor: hud::cursor_file(&assets),
    })
    .insert_resource(menu::Images::find(&assets))
    .insert_resource(game::LoadingScreen::find(&assets))
    .add_systems(
        Update,
        iso::zoom
            .in_set(frame::Step::Zoom)
            .in_set(frame::Unpaused)
            .in_set(frame::Control),
    );
    if let Some(shadows) = shadows {
        app.insert_resource(statics::ShadowAtlas(shadows));
    }
    // A game that the command line names starts at once, as if chosen in the menu; one whose run
    // has nothing else to do (a screenshot, a benchmark) ends the process when it does not load.
    let named = cli.game.named() || fixed;
    app.insert_resource(game::Request(cli.game));
    if named {
        app.insert_state(game::AppState::Loading);
    }
    if fixed {
        app.insert_resource(game::QuitOnFailure);
    }
    if let Some(seconds) = cli.benchmark {
        app.insert_resource(benchmark::Benchmark::new(seconds, true));
    }
    if let Some(dir) = cli.record {
        std::fs::create_dir_all(&dir)
            .map_err(|e| anyhow::anyhow!("creating {}: {e}", dir.display()))?;
        app.insert_resource(screenshot::Recording::new(
            dir,
            cli.frames,
            cli.record_frames,
            true,
        ));
    }
    if let Some(path) = cli.screenshot {
        app.insert_resource(screenshot::Request::new(path, cli.frames));
    }
    let mut script = command::Script::default();
    if let Some(exec) = &cli.exec {
        script.push(command::exec_lines(exec));
    }
    if let Some(path) = &cli.script {
        let text = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;
        script.push(command::script_lines(&text));
    }
    app.insert_resource(script);
    if let Some(port) = cli.remote {
        app.add_plugins(remote::RemoteControlPlugin { port });
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
