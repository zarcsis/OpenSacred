//! The heroes' exports (#2551, a part of #2358; docs/re/underworld.md, "When the original writes
//! one"): a hero of a game written to one of the choice's eight slots, `heroes/hero%02d.hero`
//! beside the saves, in the game's own format (PLAN.md, ADR-08) in place of the original's
//! `Save\Hero%.2d.pax`: its summary, which the choices' pedestals read alone, and the hero as a
//! save keeps it, whose character a game started with it takes again. The console's
//! `export SLOT` writes one.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::game::Class;
use crate::saves::{self, Hero};

/// The exports' directory beside the saves', the files' extension, and the slots of the
/// choice's pedestals.
const DIRECTORY: &str = "heroes";
const EXTENSION: &str = "hero";
pub const SLOTS: u8 = 8;

/// What the choices' pedestals show of an exported hero (the original's summary `0xC3`, docs/02
/// §9): the version of the saves it was written by, its name, class and level, the items in its
/// equipment's slots by the slots, what its figure wears (`0x6F4E60`), its wins of each
/// difficulty (#2464), which the choice offers the difficulties by (`0x6F4F20`), and its hardcore
/// mode, which the game does not have.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Summary {
    pub version: u32,
    pub name: String,
    pub class: Class,
    pub level: u32,
    pub equipment: Vec<(u8, u32)>,
    pub wins: [u8; 5],
    pub hardcore: bool,
}

/// The exports' directory: `heroes/` beside the saves' directory.
pub fn directory(world: &bevy::prelude::World) -> Option<PathBuf> {
    saves::saves_directory(world).and_then(|saves| saves.parent().map(|dir| dir.join(DIRECTORY)))
}

/// The file of the slot `slot` (`Save\Hero%.2d.pax`'s format, `0x95E4D4`).
pub fn file(dir: &Path, slot: u8) -> PathBuf {
    dir.join(format!("hero{slot:02}.{EXTENSION}"))
}

/// Writes the hero of the game in `world` to the slot `slot`, over what it held (the original's
/// action 6, `0x70E502`, after `UI_MSG_OVERWRITE_CHAR` on a taken slot): its summary and the
/// hero as a save keeps it.
pub fn export(world: &mut bevy::prelude::World, slot: u8) -> Result<PathBuf> {
    ensure!(slot < SLOTS, "a hero's slot is 0 to {}", SLOTS - 1);
    let dir = directory(world).context("no directory for the heroes")?;
    let (header, hero) = saves::gather(world, "export")?;
    let wins = world
        .get_resource::<crate::statistics::Wins>()
        .copied()
        .unwrap_or_default();
    let summary = Summary {
        version: saves::VERSION,
        name: header.hero,
        class: header.class,
        level: header.level,
        equipment: hero.pack.equipment(),
        wins: wins.0,
        hardcore: false,
    };
    let path = file(&dir, slot);
    write(&path, &summary, &hero)?;
    Ok(path)
}

/// Writes `summary` and `hero` at `path`, the summary first, which a pedestal reads alone.
fn write(path: &Path, summary: &Summary, hero: &Hero) -> Result<()> {
    let mut bytes = postcard::to_stdvec(summary).context("writing the summary")?;
    bytes.extend(postcard::to_stdvec(hero).context("writing the hero")?);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("making {}", dir.display()))?;
    }
    fs::write(path, bytes).with_context(|| format!("writing {}", path.display()))
}

/// The summary of the exported hero at `path` and the bytes after it, refused where it was
/// written by another version of the saves.
fn summary_of(bytes: &[u8]) -> Result<(Summary, &[u8])> {
    let (summary, rest): (Summary, _) =
        postcard::take_from_bytes(bytes).context("reading the summary")?;
    ensure!(
        summary.version == saves::VERSION,
        "an exported hero of the version {}, not {}",
        summary.version,
        saves::VERSION
    );
    Ok((summary, rest))
}

/// The exported hero at `path`: its summary and the hero.
pub fn read(path: &Path) -> Result<(Summary, Hero)> {
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let (summary, rest) = summary_of(&bytes)?;
    let (hero, _): (Hero, _) = postcard::take_from_bytes(rest).context("reading the hero")?;
    Ok((summary, hero))
}

/// The exported heroes of `dir`, each slot's summary, those of no other version.
pub fn list(dir: &Path) -> Vec<(u8, Summary)> {
    (0..SLOTS)
        .filter_map(|slot| {
            let bytes = fs::read(file(dir, slot)).ok()?;
            match summary_of(&bytes) {
                Ok((summary, _)) => Some((slot, summary)),
                Err(e) => {
                    bevy::log::warn!("exports: the slot {slot}: {e:#}");
                    None
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_exported_hero_is_read_back_with_its_summary_and_listed_by_its_slot() {
        let dir = std::env::temp_dir().join(format!("opensacred-exports-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let hero = saves::tests::hero();
        let summary = Summary {
            version: saves::VERSION,
            name: "Elda".to_owned(),
            class: Class::Seraphim,
            level: 25,
            equipment: vec![(0, 1712), (13, 2080)],
            wins: [0, 1, 0, 0, 0],
            hardcore: false,
        };
        write(&file(&dir, 3), &summary, &hero).unwrap();
        let (read_summary, read_hero) = read(&file(&dir, 3)).unwrap();
        assert_eq!(read_summary, summary);
        assert_eq!(read_hero.experience, hero.experience);
        assert_eq!(list(&dir), vec![(3, summary.clone())]);
        // One of another version is not listed.
        let old = Summary {
            version: saves::VERSION - 1,
            ..summary
        };
        write(&file(&dir, 5), &old, &hero).unwrap();
        assert_eq!(list(&dir).len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }
}
