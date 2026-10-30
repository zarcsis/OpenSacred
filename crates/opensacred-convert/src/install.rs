//! The player's run (#1497, #1494): from the installation to the game's assets in one command,
//! as the game's first start calls it. The installation found as `locate` finds it, or named, is
//! checked against the manifests; the masters are made in a working directory beside the
//! assets', `build` writes the assets there too, the mark of the conversion is written into them
//! last (`opensacred_data::converted`), and they take the place of the old assets only then, so
//! that a run cut off leaves the old ones or none, never half of them; the masters are removed.
//! The progress goes to stdout as JSON lines among the commands' own lines, for the game.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use opensacred_data::converted::{Converted, MARK};
use serde::Serialize;

use crate::locate::{self, Release};
use crate::verify;

/// The stages of the run and their weights, their seconds in a run on the development machine
/// (2026-10-30, 8.1 minutes in all, #1497): the check, the extractions in `all`'s order, the
/// build.
pub const STAGES: [(&str, f32); 12] = [
    ("verify", 8.0),
    ("textures", 24.0),
    ("models", 4.0),
    ("world", 17.0),
    ("sound", 45.0),
    ("text", 5.0),
    ("video", 6.0),
    ("ui", 2.0),
    ("netscript", 17.0),
    ("tables", 45.0),
    ("heroes", 1.0),
    ("build", 306.0),
];

/// The disk that a run takes at its peak, the masters (6.5 GB) and the assets (3.5 GB) with a
/// margin, measured on 2026-10-30.
pub const NEEDED_BYTES: u64 = 11 << 30;

/// A line of the progress: a stage started, the run done, or its error.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Line<'a> {
    /// The stage `stage`, the `step`-th of `steps`, starts with the share `done` of the run
    /// behind it, and ends at the share `until`.
    Stage {
        stage: &'a str,
        step: usize,
        steps: usize,
        done: f32,
        until: f32,
    },
    /// The assets are in `assets`.
    Done { assets: &'a Path },
    /// The run failed.
    Error { message: String },
}

/// Where the progress goes: as JSON lines, or as plain lines.
#[derive(Debug, Clone, Copy)]
pub struct Progress {
    pub json: bool,
}

impl Progress {
    pub fn tell(self, line: &Line<'_>) {
        if self.json {
            match serde_json::to_string(line) {
                Ok(text) => println!("{text}"),
                Err(e) => eprintln!("progress: {e}"),
            }
            return;
        }
        match line {
            Line::Stage {
                stage, step, steps, ..
            } => println!("install: {stage} ({step} of {steps})"),
            Line::Done { assets } => println!("install: the assets are in {}", assets.display()),
            Line::Error { message } => eprintln!("install: {message}"),
        }
    }

    /// The stage `name` starts.
    pub fn stage(self, name: &str) {
        let at = STAGES
            .iter()
            .position(|&(stage, _)| stage == name)
            .unwrap_or(0);
        self.tell(&Line::Stage {
            stage: name,
            step: at + 1,
            steps: STAGES.len(),
            done: share_before(at),
            until: share_before(at + 1),
        });
    }
}

/// The share of the run that the stages before the `index`-th take, by their weights; from 0.0,
/// as a float sum of nothing is −0.0.
fn share_before(index: usize) -> f32 {
    let weights = |n: usize| {
        STAGES
            .iter()
            .take(n)
            .fold(0.0, |sum, &(_, weight)| sum + weight)
    };
    weights(index) / weights(STAGES.len())
}

/// The installation to convert: `given`, which must match the manifests, or the first place
/// found that does; and its release.
pub fn installation(given: Option<PathBuf>) -> Result<(PathBuf, Release)> {
    let manifest = verify::parse_manifest(verify::GOG_MANIFEST)?;
    let steam = verify::parse_manifest(verify::STEAM_MANIFEST)?;
    let check = |dir: &Path| -> Result<std::result::Result<Release, String>> {
        let report = verify::verify(dir, &manifest, &steam)?;
        Ok(match report.problems.first() {
            None if report.variants.is_empty() => Ok(Release::Gog),
            None => Ok(Release::Steam),
            Some((path, problem)) => Err(format!(
                "{} files missing or changed, such as {path}: {problem}",
                report.problems.len()
            )),
        })
    };
    if let Some(dir) = given {
        return match check(&dir)? {
            Ok(release) => Ok((dir, release)),
            Err(problems) => bail!(
                "{} is not an installation of Sacred Gold 2.28 (English): {problems}",
                dir.display()
            ),
        };
    }
    let mut refused = Vec::new();
    for place in locate::places() {
        match check(&place.dir)? {
            Ok(release) => return Ok((place.dir, release)),
            Err(problems) => refused.push(format!("{}: {problems}", place.dir.display())),
        }
    }
    if refused.is_empty() {
        bail!("no installation of Sacred Gold 2.28 found; name it with --game-dir");
    }
    bail!(
        "no installation of Sacred Gold 2.28 (English) found: {}; name another with --game-dir",
        refused.join("; ")
    )
}

/// The default directory of the assets: the player's (`opensacred_data::converted`).
pub fn default_out() -> Result<PathBuf> {
    opensacred_data::converted::assets_dir()
        .context("no data directory (LOCALAPPDATA, or XDG_DATA_HOME or HOME); name one with --out")
}

/// The working directory of a run that writes the assets `out`: beside them, so that they move
/// there by a rename.
pub struct Work {
    out: PathBuf,
    dir: PathBuf,
}

impl Work {
    /// Starts a run into `out`: the working directory made anew, a cut run's removed, and the
    /// disk's free space checked for `needed` bytes.
    pub fn start(out: &Path, needed: u64) -> Result<Self> {
        let name = out
            .file_name()
            .with_context(|| format!("{}: no directory's name", out.display()))?;
        let parent = out.parent().unwrap_or(Path::new("."));
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        let dir = parent.join(format!("{}.work", name.to_string_lossy()));
        if dir.exists() {
            fs::remove_dir_all(&dir).with_context(|| format!("removing {}", dir.display()))?;
        }
        let free = fs4::available_space(parent)
            .with_context(|| format!("the free space of {}", parent.display()))?;
        if free < needed {
            bail!(
                "{:.1} GB free on the disk of {}, and the conversion takes {:.1} GB for its time",
                gigabytes(free),
                parent.display(),
                gigabytes(needed)
            );
        }
        fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        Ok(Self {
            out: out.to_path_buf(),
            dir,
        })
    }

    /// The masters' directory.
    pub fn masters(&self) -> PathBuf {
        self.dir.join("assets-src")
    }

    /// The new assets' directory, before they take the old ones' place.
    pub fn assets(&self) -> PathBuf {
        self.dir.join("assets")
    }

    /// Ends the run: the mark written into the new assets, which then take the old ones' place,
    /// and the working directory with the masters removed.
    pub fn finish(self, mark: &Converted) -> Result<()> {
        let assets = self.assets();
        let text = ron::ser::to_string_pretty(mark, crate::utf8::pretty())?;
        fs::write(assets.join(MARK), text).context("writing the conversion's mark")?;
        let old = self.dir.join("old");
        if self.out.exists() {
            fs::rename(&self.out, &old)
                .with_context(|| format!("moving {} aside", self.out.display()))?;
        }
        fs::rename(&assets, &self.out)
            .with_context(|| format!("moving the new assets to {}", self.out.display()))?;
        fs::remove_dir_all(&self.dir)
            .with_context(|| format!("removing {}", self.dir.display()))?;
        Ok(())
    }
}

fn gigabytes(bytes: u64) -> f64 {
    bytes as f64 / f64::from(1 << 30)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::scratch_dir;

    #[test]
    fn the_progress_is_a_line_of_json_a_stage() {
        let line = serde_json::to_string(&Line::Stage {
            stage: "sound",
            step: 5,
            steps: 12,
            done: 0.25,
            until: 0.5,
        })
        .unwrap();
        assert_eq!(
            line,
            r#"{"stage":{"stage":"sound","step":5,"steps":12,"done":0.25,"until":0.5}}"#
        );
        let done = serde_json::to_string(&Line::Done {
            assets: Path::new("a"),
        })
        .unwrap();
        assert_eq!(done, r#"{"done":{"assets":"a"}}"#);
        // The run starts at 0.0, not −0.0, the build takes most of it, and the stages are `all`'s.
        assert_eq!(share_before(0).to_bits(), 0.0f32.to_bits());
        assert!(share_before(11) < 0.5 && share_before(12) == 1.0);
        assert_eq!(STAGES.map(|(name, _)| name)[1..11], crate::EXTRACTIONS);
    }

    /// The new assets take the old ones' place with their mark, the working directory goes, and
    /// a cut run's working directory is removed at the next start.
    #[test]
    fn the_new_assets_replace_the_old_whole() {
        let root = scratch_dir("install-work");
        let out = root.join("assets");
        fs::create_dir_all(&out).unwrap();
        fs::write(out.join("old.txt"), "old").unwrap();
        fs::create_dir_all(root.join("assets.work").join("left")).unwrap();
        let work = Work::start(&out, 0).unwrap();
        assert!(!root.join("assets.work").join("left").exists());
        fs::create_dir_all(work.masters()).unwrap();
        fs::create_dir_all(work.assets()).unwrap();
        fs::write(work.assets().join("new.txt"), "new").unwrap();
        // The old assets stand until the run's end.
        assert!(out.join("old.txt").exists());
        let mark = Converted {
            version: "0.1.0".into(),
            release: "gog".into(),
        };
        work.finish(&mark).unwrap();
        assert!(out.join("new.txt").exists() && !out.join("old.txt").exists());
        let read: Converted = ron::from_str(&fs::read_to_string(out.join(MARK)).unwrap()).unwrap();
        assert_eq!(read, mark);
        assert!(!root.join("assets.work").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_disk_without_the_room_is_refused_before_anything() {
        let root = scratch_dir("install-room");
        let error = Work::start(&root.join("assets"), u64::MAX)
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("GB free"), "{error}");
        assert!(!root.join("assets.work").exists());
        fs::remove_dir_all(root).unwrap();
    }

    /// A named directory that is no installation is refused with what is wrong.
    #[test]
    fn a_named_directory_that_is_no_installation_is_refused() {
        let root = scratch_dir("install-none");
        let error = installation(Some(root.clone())).unwrap_err().to_string();
        assert!(error.contains("is not an installation"), "{error}");
        assert!(error.contains("missing"), "{error}");
        fs::remove_dir_all(root).unwrap();
    }
}
