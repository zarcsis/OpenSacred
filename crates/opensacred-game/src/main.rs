//! `opensacred-game`: the game on Bevy (PLAN.md §6.2), reading only the converted assets of
//! `assets/`. The start of M2 (PLAN.md §10, step 7): the original's isometric view of one
//! sector's ground and the hero model with an animation, at any resolution.

// The packaged build opens no console window (#1495).
#![cfg_attr(feature = "packaged", windows_subsystem = "windows")]

mod ai;
mod ambience;
mod arrow_elements;
mod arrows;
mod arsenal;
mod art_burn;
mod art_slots;
mod art_sounds;
mod arts;
mod atmosphere;
mod attack;
mod backpack;
mod banishing_runes;
mod bar;
mod bar_tips;
mod battle_fogs;
mod beacons;
mod beams;
mod belt;
mod benchmark;
mod blacksmith;
mod blast;
mod blended;
mod blood_fountains;
mod bolt_looks;
mod bolt_trails;
mod bolts;
mod bonus_list;
mod breath;
mod bubble_traps;
mod buildings;
mod burning_bones;
mod burns;
mod busy_dialog;
mod butterflies;
mod cast_sights;
mod chain_bolts;
mod choice;
mod choirs;
mod combos;
mod combos_page;
mod command;
mod compass;
mod console;
mod containers;
mod control_glow;
mod controls;
mod creature_casts;
mod crowd;
mod cursors;
mod daemon;
mod daynight;
mod debug;
mod demon_fires;
mod demon_forms;
mod doors;
mod dragon_fires;
mod drops;
mod dryad_ghosts;
mod dwarf_fires;
mod dwarf_magics;
mod effects;
mod equipment;
mod esc_menu;
mod explored;
mod explosions;
mod fight;
mod files;
mod fire_skins;
mod first_start;
mod fliers;
mod flying_skulls;
mod foe_panel;
mod followers;
mod forge;
mod forms;
mod frame;
mod fx;
mod fx_lights;
mod fx_loops;
mod game;
mod glance;
mod glitter;
mod goblin_impacts;
mod gun_fires;
mod guns;
mod healings;
mod held;
mod hell_disks;
mod hell_screams;
mod help;
mod hero;
mod hero_held;
mod hit_sparks;
mod holy_lights;
mod homing_fires;
mod hover;
mod hud;
mod hums;
mod hurled;
mod ice_splinters;
mod icebergs;
mod icons;
mod inventory;
mod iso;
mod item_glows;
mod journal;
mod killer_plants;
mod labels;
mod lava_bursts;
mod lens_creatures;
mod light_blades;
mod lightning_strikes;
mod lines;
mod liquids;
mod lost_souls;
mod low_health;
mod magic_lights;
mod making;
mod master;
mod menu;
mod menu_sound;
mod mercenaries;
mod merchant;
mod meteors;
mod mines;
mod minimap;
mod model_effects;
mod models;
mod motions;
mod movies;
mod music;
mod npcs;
mod opus;
mod overlay;
mod overview;
mod own_texts;
mod parallel;
mod paths;
mod pause;
mod pick;
mod pickup;
mod place_names;
mod plant_cages;
mod points;
mod poison_rings;
mod pong;
mod prisons;
mod puffs;
mod purchase;
mod purgatories;
mod quest_book;
mod reanimations;
mod regions;
mod remarks;
mod remote;
mod rewards;
mod ripples;
mod runes;
mod save_picture;
mod saves;
mod saves_window;
#[cfg(test)]
mod scene_tests;
mod screenshot;
mod script_attacks;
mod script_particles;
mod scripts;
mod sectors;
mod seed;
mod set_icon;
mod sets;
mod settings;
mod sfx;
mod shaders;
mod shadows;
mod skies;
mod skins;
mod soul_sparks;
mod soulcatchers;
mod souls;
mod sound;
mod sparkles;
mod spells;
mod spells_page;
mod spikes;
mod srgb;
mod stargates;
mod stashes;
mod statics;
mod statistics;
mod steps;
mod stones;
mod summon_looks;
mod summonings;
mod tables;
mod talk;
mod tentacles;
mod terrain;
mod tooltip;
mod torches;
mod town_stash;
mod trap_balls;
mod trap_landings;
mod twisters;
mod undead;
mod vamp_casts;
mod voices;
mod waterfalls;
mod weather;
mod world_map;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use anyhow::Result;
use bevy::log::{DEFAULT_FILTER, LogPlugin};
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
    /// Directory of the converted assets (opensacred-convert all): assets, or in the packaged
    /// build assets in the user's data directory (%LOCALAPPDATA%\OpenSacred on Windows)
    #[arg(long)]
    assets: Option<PathBuf>,
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
    /// Listen for remote control on this port of 127.0.0.1 (Bevy's Remote Protocol): given none,
    /// 15702, or under a Claude Code session a port of the session's own that its
    /// tools/game/remote.py finds; the window then opens without taking the focus
    #[arg(long, value_name = "PORT", num_args = 0..=1)]
    remote: Option<Option<u16>>,
    /// Keep the window hidden, a window as with --windowed, for a run driven from afar: it is on no
    /// screen, taskbar or Alt-Tab, and draws on and takes its screenshots
    #[arg(long, conflicts_with = "borderless")]
    hidden: bool,
    /// The seed of the run's rolls, which the log tells of every run, to play one again; the
    /// clock's by default, as the original seeds its rolls with the time
    #[arg(long)]
    seed: Option<u64>,
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

/// The assets' directory when `--assets` names none: `assets` in the working directory, or in the
/// packaged build `assets` in the user's data directory, where the converter writes them for the
/// player (#1495).
fn default_assets() -> PathBuf {
    let packaged = cfg!(feature = "packaged")
        .then(opensacred_data::converted::assets_dir)
        .flatten();
    packaged.unwrap_or_else(|| PathBuf::from("assets"))
}

fn run(cli: Cli) -> Result<AppExit> {
    let given = cli.assets.clone().unwrap_or_else(default_assets);
    let assets = std::path::absolute(&given)
        .map_err(|e| anyhow::anyhow!("resolving {}: {e}", given.display()))?;
    // The assets' layers: the loose files, then the archives (#699).
    files::init(&assets).map_err(|e| anyhow::anyhow!("opening {}: {e}", assets.display()))?;
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
    let shared = Arc::new(sectors::Shared {
        world,
        sprites: statics::read_sprites(&assets)?,
        tables: tables::Tables::new(&assets),
        assets: assets.clone(),
        shadows: shadows.clone(),
    });
    let sound_index = sound::Index::read(&assets)?;
    let (settings_file, mut settings) = settings::SettingsFile::read(cli.settings);
    // The keys, `controls.toml` beside the settings (#1553).
    let (controls, control_warnings) = controls::Controls::read(settings_file.path());
    // A screenshot, a recording and a benchmark run in a window of the given size, so that their
    // frames do not depend on the monitor.
    let fixed = cli.screenshot.is_some() || cli.record.is_some() || cli.benchmark.is_some();
    // A run driven by a script, from afar or for a frame plays no movie by itself (#1946).
    let driven = fixed || cli.remote.is_some() || cli.exec.is_some() || cli.script.is_some();
    let movies = movies::Automatic(settings.movies.show && !driven);
    if cli.windowed || cli.hidden || fixed {
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
    // The run's seed, before the plugins make their generators (#1644).
    seed::init(cli.seed);
    let mut app = App::new();
    // Before Bevy's plugins, whose `LogPlugin` takes the log's layer from it.
    app.insert_resource(journal::Journal::new(log_file));
    // The asset server reads the assets' layers (#699); before `AssetPlugin`.
    let root = assets.clone();
    app.register_asset_source(
        bevy::asset::io::AssetSourceId::Default,
        bevy::asset::io::AssetSourceBuilder::new(move || Box::new(files::AssetFiles::new(&root))),
    );
    app.add_plugins(
        DefaultPlugins
            .set(LogPlugin {
                // gilrs warns at every start about a device it has no mapping for, such as a
                // keyboard's game-controller collection; the game reads no gamepad yet (#440).
                // `RUST_LOG=gilrs::gamepad=warn` shows it again.
                filter: format!("{DEFAULT_FILTER},gilrs::gamepad=error"),
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
                    // Hidden, it renders all the same, on a locked desktop too; the game hangs when a
                    // hidden window is shown again (Bevy 0.20, #556), so it stays hidden.
                    visible: !cli.hidden,
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
        ambience::AmbiencePlugin,
        arrow_elements::ArrowElementsPlugin,
        arrows::ArrowsPlugin,
        arsenal::ArsenalPlugin,
        art_burn::ArtBurnPlugin,
        art_slots::ArtSlotsPlugin,
        art_sounds::ArtSoundsPlugin,
        arts::ArtsPlugin,
    ))
    .add_plugins((
        atmosphere::AtmospherePlugin,
        attack::AttackPlugin,
        backpack::BackpackPlugin,
        banishing_runes::BanishingRunesPlugin,
        bar::BarPlugin,
        bar_tips::BarTipsPlugin,
        battle_fogs::BattleFogsPlugin,
        beacons::BeaconsPlugin,
        beams::BeamsPlugin,
        belt::BeltPlugin,
        benchmark::BenchmarkPlugin,
        bevy_kira_audio::AudioPlugin,
    ))
    .add_plugins((
        blacksmith::BlacksmithPlugin,
        blast::BlastPlugin,
        blended::BlendedPlugin,
        blood_fountains::BloodFountainsPlugin,
        bolt_looks::BoltLooksPlugin,
        bolt_trails::BoltTrailsPlugin,
        bolts::BoltsPlugin,
        breath::BreathPlugin,
        bubble_traps::BubbleTrapsPlugin,
        buildings::BuildingsPlugin,
        burning_bones::BurningBonesPlugin,
        burns::BurnsPlugin,
        busy_dialog::BusyDialogPlugin,
        butterflies::ButterfliesPlugin,
    ))
    .add_plugins(cast_sights::CastSightsPlugin)
    .add_plugins((
        chain_bolts::ChainBoltsPlugin,
        choice::ChoicePlugin,
        choirs::ChoirsPlugin,
        combos::CombosPlugin,
        combos_page::CombosPagePlugin,
        command::CommandPlugin,
        compass::CompassPlugin,
        console::ConsolePlugin,
        containers::ContainersPlugin,
        control_glow::ControlGlowPlugin,
        controls::ControlsPlugin,
        creature_casts::CreatureCastsPlugin,
        crowd::CrowdPlugin,
        cursors::CursorsPlugin,
        daemon::DaemonPlugin,
    ))
    .add_plugins((
        daynight::DayNightPlugin,
        demon_fires::DemonFiresPlugin,
        demon_forms::DemonFormsPlugin,
        doors::DoorsPlugin,
        dragon_fires::DragonFiresPlugin,
        dryad_ghosts::DryadGhostsPlugin,
        drops::DropsPlugin,
        dwarf_fires::DwarfFiresPlugin,
        dwarf_magics::DwarfMagicsPlugin,
        effects::EffectsPlugin,
        equipment::EquipmentPlugin,
        esc_menu::EscMenuPlugin,
        explored::ExploredPlugin,
        explosions::ExplosionsPlugin,
    ))
    .add_plugins((
        fight::FightPlugin,
        fire_skins::FireSkinsPlugin,
        fliers::FliersPlugin,
        flying_skulls::FlyingSkullsPlugin,
        foe_panel::FoePanelPlugin,
        followers::FollowersPlugin,
        forge::ForgePlugin,
        forms::FormsPlugin,
        frame::FramePlugin,
        fx::FxPlugin,
        fx_lights::FxLightsPlugin,
        fx_loops::FxLoopsPlugin,
        game::GamePlugin,
        glance::GlancePlugin,
        glitter::GlitterPlugin,
    ))
    .add_plugins(goblin_impacts::GoblinImpactsPlugin)
    .add_plugins((gun_fires::GunFiresPlugin, guns::GunsPlugin))
    .add_plugins((
        healings::HealingsPlugin,
        hell_disks::HellDisksPlugin,
        hell_screams::HellScreamsPlugin,
        held::HeldPlugin,
        help::HelpPlugin,
        hero::HeroPlugin,
        hero_held::HeroHeldPlugin,
        hit_sparks::HitSparksPlugin,
        holy_lights::HolyLightsPlugin,
        homing_fires::HomingFiresPlugin,
        hover::HoverPlugin,
        hums::HumsPlugin,
        hud::HudPlugin,
        hurled::HurledPlugin,
        ice_splinters::IceSplintersPlugin,
    ))
    .add_plugins((
        icebergs::IcebergsPlugin,
        icons::IconsPlugin,
        inventory::InventoryPlugin,
        item_glows::ItemGlowsPlugin,
        journal::JournalPlugin,
        killer_plants::KillerPlantsPlugin,
        labels::LabelsPlugin,
        lava_bursts::LavaBurstsPlugin,
        light_blades::LightBladesPlugin,
        lightning_strikes::LightningStrikesPlugin,
        lines::LinesPlugin,
        liquids::LiquidsPlugin,
        lost_souls::LostSoulsPlugin,
        low_health::LowHealthPlugin,
    ))
    .add_plugins((
        magic_lights::MagicLightsPlugin,
        making::MakingPlugin,
        master::MasterPlugin,
        menu::MenuPlugin,
        menu_sound::MenuSoundPlugin,
        mercenaries::MercenariesPlugin,
        merchant::MerchantPlugin,
        meteors::MeteorsPlugin,
        mines::MinesPlugin,
        model_effects::ModelEffectsPlugin,
        models::ModelsPlugin,
        movies::MoviesPlugin,
        music::MusicPlugin,
        npcs::NpcsPlugin,
        opus::OpusPlugin,
    ))
    .add_plugins((
        overlay::OverlayPlugin,
        overview::OverviewPlugin,
        pause::PausePlugin,
        pickup::PickupPlugin,
        plant_cages::PlantCagesPlugin,
        points::PointsPlugin,
        poison_rings::PoisonRingsPlugin,
        pong::PongPlugin,
        prisons::PrisonsPlugin,
        puffs::PuffsPlugin,
        purchase::PurchasePlugin,
        purgatories::PurgatoriesPlugin,
        quest_book::QuestBookPlugin,
        reanimations::ReanimationsPlugin,
        regions::RegionsPlugin,
    ))
    .add_plugins((remarks::RemarksPlugin, rewards::RewardsPlugin))
    .add_plugins((
        ripples::RipplesPlugin,
        runes::RunesPlugin,
        save_picture::SavePicturePlugin,
        saves::SavesPlugin,
        saves_window::SavesWindowPlugin,
        screenshot::ScreenshotPlugin,
        script_attacks::ScriptAttacksPlugin,
        script_particles::ScriptParticlesPlugin,
        scripts::ScriptsPlugin,
        sectors::SectorsPlugin,
        seed::SeedPlugin,
        set_icon::SetIconPlugin,
        sets::SetsPlugin,
        settings::SettingsPlugin,
        sfx::SfxPlugin,
    ))
    .add_plugins((
        shaders::ShaderLibrary,
        shadows::ShadowsPlugin,
        skies::SkiesPlugin,
        skins::SkinsPlugin,
        soul_sparks::SoulSparksPlugin,
        soulcatchers::SoulcatchersPlugin,
        souls::SoulsPlugin,
        sound::SoundPlugin,
        sparkles::SparklesPlugin,
        spikes::SpikesPlugin,
    ))
    .add_plugins((
        spells::SpellsPlugin,
        spells_page::SpellsPagePlugin,
        srgb::SrgbPlugin,
        stargates::StargatesPlugin,
        stashes::StashesPlugin,
        statics::StaticsPlugin,
        statistics::StatisticsPlugin,
        steps::StepsPlugin,
        stones::StonesPlugin,
        summon_looks::SummonLooksPlugin,
        summonings::SummoningsPlugin,
        talk::TalkPlugin,
        tentacles::TentaclesPlugin,
        terrain::GroundPlugin,
        tooltip::TooltipPlugin,
    ))
    .add_plugins((
        torches::TorchesPlugin,
        town_stash::TownStashPlugin,
        trap_balls::TrapBallsPlugin,
        trap_landings::TrapLandingsPlugin,
        twisters::TwistersPlugin,
        undead::UndeadPlugin,
        vamp_casts::VampCastsPlugin,
        voices::VoicesPlugin,
        waterfalls::WaterfallsPlugin,
        weather::WeatherPlugin,
        world_map::WorldMapPlugin,
    ))
    .insert_resource(console::Console::new(console::history_file(&settings_file)))
    .insert_resource(settings)
    .insert_resource(settings_file)
    .insert_resource(controls)
    .insert_resource(controls::Warnings(control_warnings))
    .insert_resource(daynight::LightTypes::read(&shared.tables)?)
    .insert_resource(waterfalls::Types::read(&shared.tables)?)
    .insert_resource(dungeons)
    .insert_resource(hero::GameSpeed::read(&shared.tables)?)
    .insert_resource(statics::TreeFade::read(&shared.tables)?)
    .insert_resource(game::Data(shared))
    .insert_resource(daynight::GlowTexture(glow))
    .insert_resource(ClearColor(Color::BLACK))
    .insert_resource(liquids::Liquids::read(&assets)?)
    .insert_resource(voices::Voices::read(&assets)?)
    .insert_resource(talk::Portraits::read(&assets)?)
    .insert_resource(bar::UiImages::read(&assets)?)
    .insert_resource(minimap::Minimaps::read(&assets))
    .insert_resource(weather::Rolls::read(&assets)?)
    .insert_resource(icons::Files::find(&assets))
    .insert_resource(sfx::Sfx::new(&sound_index.0))
    .insert_resource(sound_index)
    .insert_resource(hud::Hud {
        cursors: hud::cursor_files(&assets),
    })
    .insert_resource(menu::Images::find(&assets))
    .insert_resource(menu::Words::read(&assets))
    .insert_resource(choice::Images::find(&assets))
    .insert_resource(saves_window::Images::find(&assets))
    .insert_resource(game::LoadingScreen::find(&assets))
    .add_systems(
        Update,
        (iso::zoom, iso::zoom_keys)
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
    // Its first windows closed as `new-game` closes them, unless left open (#1343).
    if named && !cli.game.leave_open {
        app.insert_resource(talk::CloseFirst);
    }
    app.insert_resource(game::Shows::of(&cli.game));
    app.insert_resource(game::Request(cli.game));
    app.insert_resource(movies);
    if named {
        app.insert_state(game::AppState::Loading);
    } else {
        // The start's logos before the main menu (#1946).
        app.insert_resource(movies::Logos);
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
        app.add_plugins(remote::RemoteControlPlugin::new(port)?);
    }
    Ok(app.run())
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            // Its usage, its help or its version too, which the packaged build cannot print.
            write_failure(&e.render().to_string());
            e.exit()
        }
    };
    // The packaged game's assets made first, or made again after an update (#1498).
    if cfg!(feature = "packaged")
        && cli.assets.is_none()
        && let Some(assets) = opensacred_data::converted::assets_dir()
    {
        let need = first_start::need(&assets, env!("OPENSACRED_VERSION"));
        if need != first_start::Need::Nothing {
            let log_file = cli.log_file.clone().or_else(|| {
                settings::state_dir().map(|dir| dir.join("logs").join("opensacred.log"))
            });
            return first_start::run((need, assets), cli.hidden, log_file, cli.screenshot.clone());
        }
    }
    match run(cli) {
        Ok(AppExit::Success) => ExitCode::SUCCESS,
        Ok(AppExit::Error(code)) => ExitCode::from(code.get()),
        Err(e) => {
            eprintln!("error: {e:#}");
            write_failure(&format!("error: {e:#}\n"));
            ExitCode::FAILURE
        }
    }
}

/// The packaged build, which has no console, writes the error it ends with into
/// `logs\opensacred-error.log` in the user's state directory, beside the log (#1495).
fn write_failure(text: &str) {
    if !cfg!(feature = "packaged") {
        return;
    }
    let Some(dir) = settings::state_dir().map(|dir| dir.join("logs")) else {
        return;
    };
    // Nothing more can be told of a failure to write it.
    let _ = std::fs::create_dir_all(&dir)
        .and_then(|()| std::fs::write(dir.join("opensacred-error.log"), text));
}
