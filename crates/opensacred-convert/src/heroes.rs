//! `heroes` subcommand: the hero templates `Templates/Hero*.ptx` → one hero file each under
//! `<out>/heroes/` with the `heroes.ron` index (docs/02 §9; the format is
//! [`opensacred_data::heroes`]). The saves of `save/` are not read: the game rewrites them, and the
//! original saves are not supported (PLAN.md, ADR-08).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use opensacred_data::heroes::{Hero, HeroEntry, Heroes, INDEX, Section};
use opensacred_formats::amh::Amh;

use crate::verify::{GOG_MANIFEST, parse_manifest, read_game_file};

pub fn output_dir(out: &Path) -> PathBuf {
    out.join("heroes")
}

/// The hero templates of the manifest.
fn sources() -> Result<Vec<String>> {
    let mut paths: Vec<String> = parse_manifest(GOG_MANIFEST)?
        .into_iter()
        .map(|e| e.path.to_owned())
        .filter(|p| {
            let lower = p.to_ascii_lowercase();
            lower.starts_with("templates/") && lower.ends_with(".ptx")
        })
        .collect();
    paths.sort();
    Ok(paths)
}

/// `Templates/Hero00.ptx` → `templates/hero00.hero`.
fn file_name(source: &str) -> String {
    let lower = source.to_ascii_lowercase();
    let stem = lower.rsplit_once('.').map_or(lower.as_str(), |(s, _)| s);
    format!("{stem}.hero")
}

/// Converts the templates; returns how many.
pub fn convert(game_dir: &Path, out: &Path) -> Result<usize> {
    let dir = output_dir(out);
    let mut heroes = Vec::new();
    for source in sources()? {
        let data = read_game_file(&game_dir.join(&source))?;
        let amh = Amh::parse(&data).with_context(|| source.clone())?;
        let info = amh.summary().with_context(|| source.clone())?;
        let hero = Hero {
            sections: amh
                .sections
                .into_iter()
                .map(|s| {
                    Ok(Section {
                        slot: u8::try_from(s.slot)?,
                        tag: s.tag,
                        size: s.size,
                        data: s.data,
                    })
                })
                .collect::<Result<_>>()?,
        };
        let file = file_name(&source);
        let path = dir.join(&file);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        let bytes = hero
            .encode()
            .with_context(|| format!("encoding {source}"))?;
        fs::write(&path, bytes).with_context(|| format!("writing {}", path.display()))?;
        heroes.push(HeroEntry {
            source,
            file,
            level: info.level,
            class: info.class,
        });
    }
    let count = heroes.len();
    let text = ron::ser::to_string_pretty(&Heroes { heroes }, ron::ser::PrettyConfig::default())
        .context("writing heroes.ron")?;
    let path = dir.join(INDEX);
    fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{repo_root, scratch_dir};

    #[test]
    fn hero_files_keep_their_directory() {
        assert_eq!(file_name("Templates/Hero00.ptx"), "templates/hero00.hero");
        assert_eq!(sources().unwrap().len(), 8);
    }

    /// Every hero converts and reads back; each template is of another class.
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn every_hero_converts() {
        let out = scratch_dir("heroes");
        let game = repo_root().join("OriginalGame");
        assert_eq!(convert(&game, &out).unwrap(), 8);
        let dir = output_dir(&out);
        let index: Heroes = ron::from_str(&fs::read_to_string(dir.join(INDEX)).unwrap()).unwrap();
        let mut classes: Vec<u32> = index.heroes.iter().map(|h| h.class).collect();
        classes.sort_unstable();
        classes.dedup();
        assert_eq!(classes.len(), 8);
        for entry in &index.heroes {
            let hero = Hero::decode(&fs::read(dir.join(&entry.file)).unwrap()).unwrap();
            assert_eq!(hero.sections.len(), 8);
        }
        fs::remove_dir_all(out).unwrap();
    }
}
