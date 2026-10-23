//! `build`, the converter's stage B (PLAN.md ADR-05, #694): the game's assets in `<out>` from the
//! masters that the extraction commands (stage A) write into `<assets-src>`. For now it copies
//! each master as it is, so that the game reads what it read before while the later sub-issues
//! change the formats one kind at a time (#696). It records `<out>/manifest/build.ron`: the
//! masters it built from and the files it wrote, each with its SHA-256; a master whose hash is
//! the one recorded and whose output is in place is not built again, and the outputs of masters
//! that are gone are removed.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use rayon::prelude::*;

use crate::manifest::{self, FileHash, RunManifest};

/// The manifest's name under `<out>/manifest/`.
pub const NAME: &str = "build";

/// What a build did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub masters: usize,
    pub built: usize,
    pub kept: usize,
    pub removed: usize,
}

/// The masters under `src`, but its manifests, each with its SHA-256: from stage A's manifests
/// where they record it at its size, else hashed.
fn masters(src: &Path) -> Result<Vec<FileHash>> {
    let mut paths = Vec::new();
    manifest::files(src, src, &mut paths)?;
    paths.retain(|p| !p.starts_with(&format!("{}/", manifest::DIR)));
    paths.sort();
    let mut recorded: HashMap<String, FileHash> = HashMap::new();
    let dir = src.join(manifest::DIR);
    if dir.exists() {
        for entry in fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
            let path = entry
                .with_context(|| format!("reading {}", dir.display()))?
                .path();
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if let Some(run) = manifest::read(src, stem)? {
                recorded.extend(run.outputs.into_iter().map(|o| (o.path.clone(), o)));
            }
        }
    }
    paths
        .par_iter()
        .map(|path| {
            let size = fs::metadata(src.join(path))
                .with_context(|| format!("reading {}", src.join(path).display()))?
                .len();
            match recorded.get(path) {
                Some(known) if known.size == size => Ok(known.clone()),
                _ => manifest::hash(&src.join(path), path.clone()),
            }
        })
        .collect()
}

/// Builds the game's assets in `out` from the masters in `src`; `force` builds every master.
pub fn build(src: &Path, out: &Path, force: bool) -> Result<Summary> {
    let masters = masters(src)?;
    let last = if force {
        None
    } else {
        manifest::read(out, NAME)?
    };
    let built_from: HashMap<&str, &FileHash> = last
        .as_ref()
        .map(|run| run.sources.iter().map(|s| (s.path.as_str(), s)).collect())
        .unwrap_or_default();
    let written: HashMap<&str, &FileHash> = last
        .as_ref()
        .map(|run| run.outputs.iter().map(|o| (o.path.as_str(), o)).collect())
        .unwrap_or_default();
    // A master is built again unless it is the one built last and its output is in place.
    let outcomes: Vec<(FileHash, bool)> = masters
        .par_iter()
        .map(|master| {
            let target = out.join(&master.path);
            let same = built_from.get(master.path.as_str()) == Some(&master)
                && written
                    .get(master.path.as_str())
                    .is_some_and(|o| fs::metadata(&target).is_ok_and(|m| m.len() == o.size));
            if !same {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("creating {}", parent.display()))?;
                }
                fs::copy(src.join(&master.path), &target).with_context(|| {
                    format!("building {} from {}", target.display(), master.path)
                })?;
            }
            Ok((
                FileHash {
                    path: master.path.clone(),
                    size: master.size,
                    sha256: master.sha256.clone(),
                },
                !same,
            ))
        })
        .collect::<Result<_>>()?;
    // The outputs of masters that are gone.
    let now: HashSet<&str> = masters.iter().map(|m| m.path.as_str()).collect();
    let mut removed = 0;
    for path in written.keys().filter(|p| !now.contains(**p)) {
        let target = out.join(path);
        if target.exists() {
            fs::remove_file(&target).with_context(|| format!("removing {}", target.display()))?;
            removed += 1;
        }
    }
    let count = now.len();
    let built = outcomes.iter().filter(|(_, built)| *built).count();
    let outputs: Vec<FileHash> = outcomes.into_iter().map(|(file, _)| file).collect();
    manifest::write(
        out,
        &RunManifest {
            command: NAME.to_owned(),
            converter: manifest::converter()?,
            sources: masters,
            outputs,
        },
    )?;
    Ok(Summary {
        masters: count,
        built,
        kept: count - built,
        removed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::scratch_dir;

    fn write(dir: &Path, path: &str, text: &str) {
        let path = dir.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn a_build_copies_the_masters_and_redoes_only_what_changed() {
        let src = scratch_dir("build-src");
        let out = scratch_dir("build-out");
        write(&src, "tables/items.ron", "[]");
        write(&src, "sound/samples/00002.flac", "fLaC");
        write(&src, "manifest/tables.ron", "not a manifest");
        let first = build(&src, &out, false).unwrap();
        assert_eq!((first.masters, first.built, first.kept), (2, 2, 0));
        assert_eq!(
            fs::read_to_string(out.join("tables/items.ron")).unwrap(),
            "[]"
        );
        // Stage A's manifests are not masters.
        assert!(!out.join("manifest/tables.ron").exists());
        // Nothing changed: nothing is built.
        let again = build(&src, &out, false).unwrap();
        assert_eq!((again.built, again.kept), (0, 2));
        // A changed master is built again, a master gone takes its output with it.
        write(&src, "tables/items.ron", "[()]");
        fs::remove_file(src.join("sound/samples/00002.flac")).unwrap();
        let changed = build(&src, &out, false).unwrap();
        assert_eq!((changed.built, changed.kept, changed.removed), (1, 0, 1));
        assert_eq!(
            fs::read_to_string(out.join("tables/items.ron")).unwrap(),
            "[()]"
        );
        assert!(!out.join("sound/samples/00002.flac").exists());
        // An output lost is built again; `force` builds everything.
        fs::remove_file(out.join("tables/items.ron")).unwrap();
        assert_eq!(build(&src, &out, false).unwrap().built, 1);
        assert_eq!(build(&src, &out, true).unwrap().built, 1);
        fs::remove_dir_all(src).unwrap();
        fs::remove_dir_all(out).unwrap();
    }
}
