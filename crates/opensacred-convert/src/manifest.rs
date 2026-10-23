//! The run manifests (PLAN.md §7: "writes a manifest with the source of every output file"):
//! after a full run, each subcommand records under `<out>/manifest/<command>.ron` the converter
//! that ran, the installation files it read and the files it wrote, each with its SHA-256. `all`
//! skips a command whose manifest names the same converter and sources and whose outputs are all
//! in place, and two runs that write the same manifests wrote the same files.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::coverage::{Status, status};
use crate::verify::{GOG_MANIFEST, parse_manifest, sha256_file};

/// The manifest directory under the output directory.
pub const DIR: &str = "manifest";

/// Files a command reads besides those that `coverage` attributes to it.
const EXTRA_SOURCES: &[(&str, &[&str])] = &[
    ("models", &["PAK/texture.pak", "PAK/texture03.pak"]),
    ("world", &["PAK/texture.pak", "PAK/Items.pak"]),
    (
        "tables",
        &["PAK/models.pak", "PAK/models03.pak", "Sacred.exe"],
    ),
    ("sound", &["Sacred.exe"]),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunManifest {
    pub command: String,
    /// SHA-256 of the converter executable.
    pub converter: String,
    /// The installation files read, as the reference manifest names them.
    pub sources: Vec<FileHash>,
    /// The files written, relative to the output directory.
    pub outputs: Vec<FileHash>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileHash {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

fn manifest_path(out: &Path, command: &str) -> PathBuf {
    out.join(DIR).join(format!("{command}.ron"))
}

/// The installation files `command` reads.
pub fn sources(command: &str) -> Result<Vec<String>> {
    let extra = EXTRA_SOURCES
        .iter()
        .find(|(c, _)| *c == command)
        .map_or(&[][..], |(_, paths)| paths);
    let mut paths: Vec<String> = parse_manifest(GOG_MANIFEST)?
        .into_iter()
        .filter(|e| {
            matches!(status(e.path), Status::Converted(c) | Status::Partial(c, _) if c == command)
                || extra.iter().any(|x| x.eq_ignore_ascii_case(e.path))
        })
        .map(|e| e.path.to_owned())
        .collect();
    paths.sort();
    Ok(paths)
}

fn hash(path: &Path, name: String) -> Result<FileHash> {
    let (sha256, size) =
        sha256_file(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(FileHash {
        path: name,
        size,
        sha256,
    })
}

/// SHA-256 of the running converter.
pub fn converter() -> Result<String> {
    let exe = std::env::current_exe().context("finding the converter executable")?;
    Ok(hash(&exe, String::new())?.sha256)
}

/// The files under `dir`, as paths relative to `base` with `/` separators, sorted.
fn files(base: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry
            .with_context(|| format!("reading {}", dir.display()))?
            .path();
        if path.is_dir() {
            files(base, &path, out)?;
        } else {
            let relative = path.strip_prefix(base).unwrap_or(&path);
            out.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

fn hash_all(root: &Path, paths: &[String]) -> Result<Vec<FileHash>> {
    paths
        .par_iter()
        .map(|p| hash(&root.join(p), p.clone()))
        .collect()
}

/// Records the run of `command`: its sources in `game_dir` and every file under
/// `<out>/<command>/`.
pub fn record(game_dir: &Path, out: &Path, command: &str) -> Result<RunManifest> {
    let sources = hash_all(game_dir, &sources(command)?)?;
    let mut outputs = Vec::new();
    files(out, &out.join(command), &mut outputs)?;
    outputs.sort();
    let manifest = RunManifest {
        command: command.to_owned(),
        converter: converter()?,
        sources,
        outputs: hash_all(out, &outputs)?,
    };
    let path = manifest_path(out, command);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let text = ron::ser::to_string_pretty(&manifest, ron::ser::PrettyConfig::default())
        .with_context(|| format!("writing the manifest of {command}"))?;
    fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
    Ok(manifest)
}

/// Forgets the run of `command`, after a run that wrote only some of its outputs.
pub fn forget(out: &Path, command: &str) -> Result<()> {
    let path = manifest_path(out, command);
    match fs::remove_file(&path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(e).with_context(|| format!("removing {}", path.display()))
        }
        _ => Ok(()),
    }
}

/// The recorded run of `command`, if any.
pub fn read(out: &Path, command: &str) -> Result<Option<RunManifest>> {
    let path = manifest_path(out, command);
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    Ok(ron::from_str(&text).ok())
}

/// Whether the recorded run of `command` still holds: the same converter, sources with the
/// same hashes, and every output in place with its size.
pub fn up_to_date(game_dir: &Path, out: &Path, command: &str, converter: &str) -> Result<bool> {
    let Some(manifest) = read(out, command)? else {
        return Ok(false);
    };
    if manifest.converter != converter {
        return Ok(false);
    }
    let names: Vec<String> = manifest.sources.iter().map(|s| s.path.clone()).collect();
    if names != sources(command)? || hash_all(game_dir, &names)? != manifest.sources {
        return Ok(false);
    }
    let mut outputs = Vec::new();
    files(out, &out.join(command), &mut outputs)?;
    outputs.sort();
    let recorded: Vec<&String> = manifest.outputs.iter().map(|o| &o.path).collect();
    if outputs.iter().collect::<Vec<_>>() != recorded {
        return Ok(false);
    }
    Ok(manifest
        .outputs
        .iter()
        .all(|o| fs::metadata(out.join(&o.path)).is_ok_and(|m| m.len() == o.size)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{repo_root, scratch_dir};
    use crate::{heroes, netscript, tables, text, ui};

    /// The subcommands that write a manifest, each into the output subdirectory of its name.
    const COMMANDS: [&str; 10] = [
        "textures",
        "models",
        "world",
        "sound",
        "text",
        "video",
        "ui",
        "netscript",
        "tables",
        "heroes",
    ];

    #[test]
    fn every_command_reads_its_sources() {
        let total: usize = COMMANDS.iter().map(|c| sources(c).unwrap().len()).sum();
        assert!(total >= 305);
        assert_eq!(sources("text").unwrap(), ["scripts/us/global.res"]);
        let models = sources("models").unwrap();
        assert!(models.contains(&"PAK/texture.pak".to_owned()));
        assert!(models.contains(&"PAK/models03.pak".to_owned()));
    }

    /// Two runs write the same files, and a recorded run holds until an output changes.
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn runs_are_deterministic_and_recorded() {
        let game = repo_root().join("OriginalGame");
        let runs: Vec<PathBuf> = ["a", "b"]
            .iter()
            .map(|name| scratch_dir(&format!("manifest-{name}")))
            .collect();
        let mut manifests = Vec::new();
        for out in &runs {
            text::convert(&game, out).unwrap();
            ui::convert(&game, out).unwrap();
            netscript::convert(&game, out, false).unwrap();
            tables::convert(&game, out).unwrap();
            heroes::convert(&game, out).unwrap();
            let recorded: Vec<RunManifest> = ["text", "ui", "netscript", "tables", "heroes"]
                .iter()
                .map(|c| record(&game, out, c).unwrap())
                .collect();
            manifests.push(recorded);
        }
        assert_eq!(manifests[0], manifests[1]);
        let converter = converter().unwrap();
        assert!(up_to_date(&game, &runs[0], "tables", &converter).unwrap());
        assert!(!up_to_date(&game, &runs[0], "tables", "another").unwrap());
        fs::write(runs[0].join("tables").join("extra.ron"), "()").unwrap();
        assert!(!up_to_date(&game, &runs[0], "tables", &converter).unwrap());
        for out in runs {
            fs::remove_dir_all(out).unwrap();
        }
    }
}
