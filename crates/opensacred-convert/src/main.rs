//! `opensacred-convert`: offline converter of the GOG Sacred Gold 2.28 installation into open formats
//! (PLAN.md §7). Checks the installation against the SHA-256 manifest and converts textures,
//! models, the world, the sound, the texts, the cutscenes, the scripts, the tables and the heroes.

mod animation;
mod coverage;
mod embedded;
mod ffmpeg;
mod gltf;
mod heroes;
mod manifest;
mod models;
mod netscript;
mod sound;
mod tables;
mod text;
mod textures;
mod ui;
mod verify;
mod video;
mod world;

#[cfg(test)]
mod testing;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    version = env!("OPENSACRED_VERSION"),
    about = "Convert a GOG Sacred Gold 2.28 installation into open formats"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run every conversion in order: textures, models, world, sound, text, video, ui, netscript,
    /// tables, heroes; skip those whose manifest under <OUT>/manifest/ still holds
    All {
        /// Game installation directory
        #[arg(long, default_value = "OriginalGame")]
        game_dir: PathBuf,
        /// Output directory for converted assets
        #[arg(long, default_value = "assets")]
        out: PathBuf,
        /// Run every conversion even if its outputs are up to date
        #[arg(long)]
        force: bool,
    },
    /// Show what becomes of every file of the reference installation
    Coverage,
    /// Check the installation against the embedded SHA-256 manifest
    Verify {
        /// Game installation directory
        #[arg(long, default_value = "OriginalGame")]
        game_dir: PathBuf,
    },
    /// Convert TEX textures (PAK/texture*.pak) into RGBA PNG under <OUT>/textures/
    Textures {
        /// Game installation directory
        #[arg(long, default_value = "OriginalGame")]
        game_dir: PathBuf,
        /// Output directory for converted assets
        #[arg(long, default_value = "assets")]
        out: PathBuf,
    },
    /// Convert the GRN models and motions of PAK/models*.pak into glTF binaries under
    /// <OUT>/models/; a model's file carries the motions of its slots as animations, and its
    /// textures are the PNGs of `textures` in the same <OUT>
    Models {
        /// Game installation directory
        #[arg(long, default_value = "OriginalGame")]
        game_dir: PathBuf,
        /// Output directory for converted assets
        #[arg(long, default_value = "assets")]
        out: PathBuf,
        /// Convert only the record with this name, such as SERAPHIM.GRN (repeatable)
        #[arg(long = "model", value_name = "NAME")]
        only: Vec<String>,
    },
    /// Convert the world (World/sectors.*, Static.PAK, Floor.PAK, PAK/tiles.pak) into
    /// <OUT>/world/world.ron and one sector file per sector; tile sheets are the PNGs of
    /// `textures` in the same <OUT>
    World {
        /// Game installation directory
        #[arg(long, default_value = "OriginalGame")]
        game_dir: PathBuf,
        /// Output directory for converted assets
        #[arg(long, default_value = "assets")]
        out: PathBuf,
        /// Convert only the sector at this grid position, such as 50,39
        #[arg(long, value_name = "X,Y", value_parser = parse_position)]
        sector: Option<(u32, u32)>,
    },
    /// Convert the samples of PAK/sound.pak (WAV to FLAC with FFmpeg, MP3 copied), the profiles
    /// of PAK/sndProfiles.pak and the music of mp3/ into <OUT>/sound/
    Sound {
        /// Game installation directory
        #[arg(long, default_value = "OriginalGame")]
        game_dir: PathBuf,
        /// Output directory for converted assets
        #[arg(long, default_value = "assets")]
        out: PathBuf,
        /// Convert only the sample with this id (repeatable)
        #[arg(long = "sample", value_name = "ID")]
        only: Vec<usize>,
    },
    /// Convert the strings of scripts/us/global.res into the Fluent file <OUT>/text/en-US/global.ftl
    Text {
        /// Game installation directory
        #[arg(long, default_value = "OriginalGame")]
        game_dir: PathBuf,
        /// Output directory for converted assets
        #[arg(long, default_value = "assets")]
        out: PathBuf,
    },
    /// Convert the loading and save screens (BMP to PNG), the fonts and the credits into <OUT>/ui/
    Ui {
        /// Game installation directory
        #[arg(long, default_value = "OriginalGame")]
        game_dir: PathBuf,
        /// Output directory for converted assets
        #[arg(long, default_value = "assets")]
        out: PathBuf,
    },
    /// Convert the NetScript bytecode of the 20 trees of bin/ into one tree file each under
    /// <OUT>/netscript/
    Netscript {
        /// Game installation directory
        #[arg(long, default_value = "OriginalGame")]
        game_dir: PathBuf,
        /// Output directory for converted assets
        #[arg(long, default_value = "assets")]
        out: PathBuf,
        /// Also write a readable listing of each tree (<tree>.txt, about 130 MB in all)
        #[arg(long)]
        listing: bool,
    },
    /// Convert the creature, weapon, item, motion and trigger tables and the data tables of bin/
    /// into RON files under <OUT>/tables/
    Tables {
        /// Game installation directory
        #[arg(long, default_value = "OriginalGame")]
        game_dir: PathBuf,
        /// Output directory for converted assets
        #[arg(long, default_value = "assets")]
        out: PathBuf,
    },
    /// Convert the hero templates of Templates/ into hero files under <OUT>/heroes/
    Heroes {
        /// Game installation directory
        #[arg(long, default_value = "OriginalGame")]
        game_dir: PathBuf,
        /// Output directory for converted assets
        #[arg(long, default_value = "assets")]
        out: PathBuf,
    },
    /// Convert the WMV cutscenes of movie/ into AV1 and Opus WebM files under <OUT>/video/ with
    /// FFmpeg
    Video {
        /// Game installation directory
        #[arg(long, default_value = "OriginalGame")]
        game_dir: PathBuf,
        /// Output directory for converted assets
        #[arg(long, default_value = "assets")]
        out: PathBuf,
        /// Convert only the cutscene with this name, such as intro (repeatable)
        #[arg(long = "movie", value_name = "NAME")]
        only: Vec<String>,
    },
}

/// `x,y` of a grid position.
fn parse_position(text: &str) -> Result<(u32, u32), String> {
    let (x, y) = text.split_once(',').ok_or("expected X,Y")?;
    let number = |s: &str| s.trim().parse::<u32>().map_err(|e| format!("{s}: {e}"));
    Ok((number(x)?, number(y)?))
}

impl Command {
    /// The manifest a run records: the installation, the output directory, the command's name,
    /// and whether the run writes all of its outputs.
    fn manifest(&self) -> Option<(&PathBuf, &PathBuf, &'static str, bool)> {
        Some(match self {
            Command::Textures { game_dir, out } => (game_dir, out, "textures", true),
            Command::Models {
                game_dir,
                out,
                only,
            } => (game_dir, out, "models", only.is_empty()),
            Command::World {
                game_dir,
                out,
                sector,
            } => (game_dir, out, "world", sector.is_none()),
            Command::Sound {
                game_dir,
                out,
                only,
            } => (game_dir, out, "sound", only.is_empty()),
            Command::Text { game_dir, out } => (game_dir, out, "text", true),
            Command::Video {
                game_dir,
                out,
                only,
            } => (game_dir, out, "video", only.is_empty()),
            Command::Ui { game_dir, out } => (game_dir, out, "ui", true),
            Command::Netscript { game_dir, out, .. } => (game_dir, out, "netscript", true),
            Command::Tables { game_dir, out } => (game_dir, out, "tables", true),
            Command::Heroes { game_dir, out } => (game_dir, out, "heroes", true),
            Command::All { .. } | Command::Coverage | Command::Verify { .. } => return None,
        })
    }
}

fn main() -> Result<ExitCode> {
    match Cli::parse().command {
        Command::All {
            game_dir,
            out,
            force,
        } => {
            let start = Instant::now();
            let converter = manifest::converter()?;
            for command in [
                Command::Textures {
                    game_dir: game_dir.clone(),
                    out: out.clone(),
                },
                Command::Models {
                    game_dir: game_dir.clone(),
                    out: out.clone(),
                    only: Vec::new(),
                },
                Command::World {
                    game_dir: game_dir.clone(),
                    out: out.clone(),
                    sector: None,
                },
                Command::Sound {
                    game_dir: game_dir.clone(),
                    out: out.clone(),
                    only: Vec::new(),
                },
                Command::Text {
                    game_dir: game_dir.clone(),
                    out: out.clone(),
                },
                Command::Video {
                    game_dir: game_dir.clone(),
                    out: out.clone(),
                    only: Vec::new(),
                },
                Command::Ui {
                    game_dir: game_dir.clone(),
                    out: out.clone(),
                },
                Command::Netscript {
                    game_dir: game_dir.clone(),
                    out: out.clone(),
                    listing: false,
                },
                Command::Tables {
                    game_dir: game_dir.clone(),
                    out: out.clone(),
                },
                Command::Heroes {
                    game_dir: game_dir.clone(),
                    out: out.clone(),
                },
            ] {
                if let Some((game_dir, out, name, _)) = command.manifest()
                    && !force
                    && manifest::up_to_date(game_dir, out, name, &converter)?
                {
                    println!("{name}: up to date");
                    continue;
                }
                run(command)?;
            }
            println!("all done in {:.1} s", start.elapsed().as_secs_f64());
            Ok(ExitCode::SUCCESS)
        }
        command => run(command),
    }
}

/// Runs a command, then records its manifest, or forgets it after a run that wrote only some of
/// the outputs.
fn run(command: Command) -> Result<ExitCode> {
    let target = command
        .manifest()
        .map(|(game_dir, out, name, full)| (game_dir.clone(), out.clone(), name, full));
    let code = execute(command)?;
    if let Some((game_dir, out, name, full)) = target {
        if full {
            let recorded = manifest::record(&game_dir, &out, name)?;
            println!(
                "{name}: {} outputs recorded in {}",
                recorded.outputs.len(),
                out.join(manifest::DIR).display()
            );
        } else {
            manifest::forget(&out, name)?;
        }
    }
    Ok(code)
}

fn execute(command: Command) -> Result<ExitCode> {
    match command {
        Command::All { .. } => unreachable!("`all` runs the other commands"),
        Command::Coverage => {
            let coverage = coverage::coverage()?;
            println!(
                "{} files converted, {} in part, {} skipped, {} not yet",
                coverage.converted,
                coverage.partial.len(),
                coverage.skipped,
                coverage.pending.len(),
            );
            for (path, missing) in &coverage.partial {
                println!("  in part: {path} (missing: {missing})");
            }
            let mut pending: Vec<(&str, usize)> = Vec::new();
            for (_, what) in &coverage.pending {
                match pending.iter_mut().find(|(w, _)| w == what) {
                    Some((_, n)) => *n += 1,
                    None => pending.push((what, 1)),
                }
            }
            for (what, n) in pending {
                println!("  not yet: {what} ({n} files)");
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Verify { game_dir } => {
            let manifest = verify::parse_manifest(verify::GOG_MANIFEST)?;
            let report = verify::verify(&game_dir, &manifest)?;
            const SHOWN: usize = 20;
            for (path, problem) in report.problems.iter().take(SHOWN) {
                eprintln!("{path}: {problem}");
            }
            if report.problems.len() > SHOWN {
                eprintln!("... and {} more", report.problems.len() - SHOWN);
            }
            println!(
                "{} of {} files match the GOG 2.28 manifest ({:.2} GiB read)",
                manifest.len() - report.problems.len(),
                manifest.len(),
                report.bytes as f64 / f64::from(1 << 30),
            );
            Ok(if report.problems.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        Command::Textures { game_dir, out } => {
            let start = Instant::now();
            for (pak, dir_name) in textures::PAKS {
                let dst = textures::output_dir(&out, dir_name);
                let summary = textures::convert_pak(&game_dir.join(pak), &dst)?;
                println!(
                    "{pak}: {} textures ({} ARGB4444, {} BGRA8888), {} empty -> {}",
                    summary.argb4444 + summary.bgra8888,
                    summary.argb4444,
                    summary.bgra8888,
                    summary.empty,
                    dst.display(),
                );
                if summary.repaired_frames > 0 {
                    println!(
                        "  repaired: {} texels that are not opaque in {} frames of the lavas",
                        summary.repaired_texels, summary.repaired_frames,
                    );
                }
            }
            println!("done in {:.1} s", start.elapsed().as_secs_f64());
            Ok(ExitCode::SUCCESS)
        }
        Command::Models {
            game_dir,
            out,
            only,
        } => {
            let start = Instant::now();
            let textures = models::TextureIndex::read(&game_dir)?;
            for (pak, dir_name) in models::PAKS {
                let dst = models::output_dir(&out, dir_name);
                let summary = models::convert_pak(&game_dir.join(pak), &dst, &textures, &only)?;
                let s = summary.stats;
                println!(
                    "{pak}: {} models, {} motions -> {}\n  {} meshes ({} helper meshes left out), \
                     {} primitives, {} vertices, {} triangles\n  replaced: {} zero normals; \
                     reduced to 4 influences: {} vertices; outer scale/shear nodes: {} bones, \
                     sampled for shear: {} tracks\n  embedded textures: {} written into the file, {} \
                     bound to texture.pak by name; texture materials without image: {} not in \
                     texture.pak\n  {} animations, {} tracks, {} keyframes; left out: {} tracks of \
                     bones the model lacks, {} second tracks of a bone; held in the bind pose: {} \
                     bones; {} keys where a rotation curve jumps",
                    summary.models,
                    summary.motions,
                    dst.display(),
                    s.meshes,
                    s.helper_meshes,
                    s.primitives,
                    s.vertices,
                    s.triangles,
                    s.zero_normals,
                    s.reduced_vertices,
                    s.sheared_bones,
                    s.sheared_tracks,
                    s.embedded_textures,
                    s.twinned_textures,
                    s.unnamed_textures,
                    s.animations,
                    s.tracks,
                    s.keyframes,
                    s.unbound_tracks,
                    s.duplicate_tracks,
                    s.rest_tracks,
                    s.rotation_jumps,
                );
            }
            println!("done in {:.1} s", start.elapsed().as_secs_f64());
            Ok(ExitCode::SUCCESS)
        }
        Command::World {
            game_dir,
            out,
            sector,
        } => {
            let start = Instant::now();
            let summary = world::convert(&game_dir, &out, sector)?;
            println!(
                "world: {} sectors ({:.1} MB), {} floor overlays, {} static objects ({} with a \
                 sprite, {} miniatures) -> {}\n  {} sprites over {} atlases; {} tile sheets \
                 without an image",
                summary.sectors,
                summary.bytes as f64 / 1e6,
                summary.overlays,
                summary.statics,
                summary.sprite_statics,
                summary.miniatures,
                world::output_dir(&out).display(),
                summary.sprites,
                summary.atlases,
                summary.missing_sheets,
            );
            println!("done in {:.1} s", start.elapsed().as_secs_f64());
            Ok(ExitCode::SUCCESS)
        }
        Command::Sound {
            game_dir,
            out,
            only,
        } => {
            let start = Instant::now();
            let summary = sound::convert(&game_dir, &out, &only)?;
            println!(
                "sound: {} FLAC and {} MP3 samples, {} profiles, {} music files ({:.1} MB) -> {}",
                summary.flac,
                summary.mp3,
                summary.profiles,
                summary.music,
                summary.bytes as f64 / 1e6,
                sound::output_dir(&out).display(),
            );
            println!("done in {:.1} s", start.elapsed().as_secs_f64());
            Ok(ExitCode::SUCCESS)
        }
        Command::Video {
            game_dir,
            out,
            only,
        } => {
            let start = Instant::now();
            let summary = video::convert(&game_dir, &out, &only)?;
            println!(
                "video: {} cutscenes ({:.1} MB) -> {}",
                summary.movies,
                summary.bytes as f64 / 1e6,
                video::output_dir(&out).display(),
            );
            println!("done in {:.1} s", start.elapsed().as_secs_f64());
            Ok(ExitCode::SUCCESS)
        }
        Command::Ui { game_dir, out } => {
            let summary = ui::convert(&game_dir, &out)?;
            println!(
                "ui: {} screens, {} fonts, {} texts -> {}",
                summary.screens,
                summary.fonts,
                summary.texts,
                ui::output_dir(&out).display(),
            );
            Ok(ExitCode::SUCCESS)
        }
        Command::Netscript {
            game_dir,
            out,
            listing,
        } => {
            let start = Instant::now();
            let summary = netscript::convert(&game_dir, &out, listing)?;
            println!(
                "netscript: {} trees, {} records in {} functions and the start scripts ({:.1} MB) -> {}",
                summary.trees,
                summary.records,
                summary.functions,
                summary.bytes as f64 / 1e6,
                netscript::output_dir(&out).display(),
            );
            println!("done in {:.1} s", start.elapsed().as_secs_f64());
            Ok(ExitCode::SUCCESS)
        }
        Command::Tables { game_dir, out } => {
            let summary = tables::convert(&game_dir, &out)?;
            println!(
                "tables: {} creatures, {} weapons, {} items, {} motions, {} triggers, {} files of bin/ -> {}",
                summary.creatures,
                summary.weapons,
                summary.items,
                summary.motions,
                summary.triggers,
                summary.bin_tables,
                tables::output_dir(&out).display(),
            );
            Ok(ExitCode::SUCCESS)
        }
        Command::Heroes { game_dir, out } => {
            let templates = heroes::convert(&game_dir, &out)?;
            println!(
                "heroes: {templates} templates -> {}",
                heroes::output_dir(&out).display(),
            );
            Ok(ExitCode::SUCCESS)
        }
        Command::Text { game_dir, out } => {
            let summary = text::convert(&game_dir, &out)?;
            println!(
                "text: {} strings ({:.1} MB of Fluent) -> {}",
                summary.strings,
                summary.bytes as f64 / 1e6,
                text::output_dir(&out).display(),
            );
            Ok(ExitCode::SUCCESS)
        }
    }
}
