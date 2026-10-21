//! `opensacred-oracle`: a development tool, run locally (docs/04 §2.3, PLAN.md §4.2). It loads
//! the developer's own GOG `Sacred.exe` 2.28 into the Unicorn emulator, runs the fight's formula
//! functions of docs/re/fight.md on sets of inputs, and writes their outputs as JSON vectors to a
//! directory outside the repository, which `opensacred-rules`' tests compare its formulas with.
//!
//! Unicorn is GPL-2.0: the emulator is the feature `emulator`, and no crate of the converter or
//! the game depends on this one, which has no library (docs/08 §4). Without the feature the
//! oracle checks the installation and the image, and stops.

mod pe;
mod vectors;

#[cfg(feature = "emulator")]
mod fight;
#[cfg(feature = "emulator")]
mod machine;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;

use crate::pe::Image;

/// Runs the fight's formulas of your own Sacred.exe in the emulator and writes their vectors.
#[derive(Parser)]
#[command(version = None)]
struct Cli {
    /// The GOG installation (its Sacred.exe and bin/Balance.bin must be the manifest's)
    #[arg(long, default_value = "OriginalGame")]
    game_dir: PathBuf,
    /// Where the vectors go, outside the repository [default: OPENSACRED_VECTORS, else
    /// %LOCALAPPDATA%\OpenSacred\vectors on Windows, $XDG_DATA_HOME/OpenSacred/vectors elsewhere]
    #[arg(long)]
    out: Option<PathBuf>,
    /// The precision of the FPU in bits of mantissa: 24, 53 (the runtime's default) or 64
    #[arg(long, default_value_t = 53)]
    precision: u8,
}

/// The size of the balance's image (docs/02 §6.3), `bin/Balance.bin`'s first word, which the
/// client loads to `0xAD4490`.
const BALANCE_SIZE: usize = 4896;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let exe = vectors::checked(&cli.game_dir, "Sacred.exe")?;
    let balance = vectors::checked(&cli.game_dir, "bin/Balance.bin")?;
    let balance = balance
        .get(4..4 + BALANCE_SIZE)
        .context("bin/Balance.bin: shorter than its image")?;
    let image = Image::parse(&exe).context("Sacred.exe")?;
    println!(
        "Sacred.exe: an image of {:#x} bytes at {:#x}",
        image.memory.len(),
        image.base
    );
    for section in &image.sections {
        println!(
            "Sacred.exe: {} at {:#x}, {:#x} bytes",
            section.name,
            image.base + section.address,
            section.size
        );
    }
    let mut dlls: Vec<&str> = image.imports.iter().map(|i| i.dll.as_str()).collect();
    dlls.dedup();
    println!(
        "Sacred.exe: {} imports of {} DLLs",
        image.imports.len(),
        dlls.len()
    );
    let out = vectors::prepare(&vectors::directory(cli.out.as_deref())?)?;
    run(&image, balance, &out, cli.precision, &vectors::sha256(&exe))
}

#[cfg(not(feature = "emulator"))]
fn run(_: &Image, _: &[u8], out: &Path, precision: u8, _: &str) -> Result<()> {
    anyhow::bail!(
        "built without the emulator, so nothing ran ({precision}-bit FPU, the vectors to {}): \
         cargo run --release -p opensacred-oracle --features emulator (CMake, a C compiler, Ninja \
         and libclang, docs/03 §3.4)",
        out.display()
    )
}

#[cfg(feature = "emulator")]
fn run(image: &Image, balance: &[u8], out: &Path, precision: u8, exe: &str) -> Result<()> {
    use crate::machine::{Machine, control_word};
    use crate::vectors::About;

    let control = control_word(precision)?;
    let mut machine = Machine::new(image, control)?;
    // As the client holds it once it has read the balance.
    machine.write(0xAD_4490, balance)?;
    fight::prepare(&mut machine)?;
    let about = |set, function: u32, description| About {
        set,
        function: format!("{function:#X}"),
        description,
        sacred_exe: exe.to_owned(),
        fpu_control_word: control,
    };

    let ends = fight::level_ends(&mut machine)?;
    let levels = fight::levels_of(&mut machine, &ends)?;
    let files = [
        vectors::write(
            out,
            about(
                "level_end",
                0x57_F6E0,
                "the experience at which each level ends",
            ),
            &ends,
        )?,
        vectors::write(
            out,
            about(
                "level_of",
                0x57_F8D0,
                "the level of an experience, by the level held",
            ),
            &levels,
        )?,
        vectors::write(
            out,
            about(
                "hit_chance",
                0x43_EEE0,
                "a strike's hit chance, an attack against a defence",
            ),
            &fight::chances(&mut machine)?,
        )?,
        vectors::write(
            out,
            about(
                "dealt",
                0x43_EEE0,
                "a kind's damage that its resistance lets through, by the attacker's levels \
                 above the defender's",
            ),
            &fight::dealt(&mut machine)?,
        )?,
        vectors::write(
            out,
            about(
                "most_health",
                0x56_58F0,
                "a creature's most health by its base attributes and its level, of a hero's \
                 type or not, on the first difficulty",
            ),
            &fight::most_health(&mut machine)?,
        )?,
    ];
    for file in files {
        println!("{}", file.display());
    }
    Ok(())
}
