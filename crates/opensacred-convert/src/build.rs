//! `build`, the converter's stage B (PLAN.md ADR-05, #694): the game's assets in `<out>` from the
//! masters that the extraction commands (stage A) write into `<assets-src>`. It encodes the
//! cutscenes' masters into AV1 and Opus WebM (#698), takes the other masters as they are, while
//! the later sub-issues change the formats one kind at a time (#696), and packs the outputs into
//! ZIP archives by kind, [`opensacred_data::archive`]'s (#699): `data.zip` (the tables, texts,
//! scripts and heroes), `models.zip`, `music.zip`, `sound.zip`, `textures.zip`, `ui.zip`,
//! `video.zip` and `world.zip`, the entries in the order of their paths.
//!
//! It records `<out>/manifest/build.ron`: the masters it built from and each archive's entries,
//! `<archive>/<path>`, each with its SHA-256. An archive whose masters are all the ones recorded
//! and whose entries are in place is kept; one written again copies the entries whose masters did
//! not change from the old archive as they are, without making them again. The archives no master
//! makes any more and the loose files of the builds before #699 are removed.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use opensacred_data::archive::{self, Archive, Raw, Writer};
use rayon::prelude::*;
use sha2::{Digest, Sha256};

use crate::manifest::{self, FileHash, RunManifest};
use crate::verify::hex;

/// The manifest's name under `<out>/manifest/`.
pub const NAME: &str = "build";
/// The directory under `<out>` where the outputs made by a tool wait to be packed.
const STAGING: &str = ".staging";
/// The entries made at once, in parallel, before they are written in their order.
const BATCH: usize = 64;

/// How stage B makes a master's output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Make {
    Copy,
    /// A cutscene's FFV1 and FLAC master, encoded into AV1 and Opus WebM.
    Video,
    /// The cutscenes' index, naming the WebM files.
    VideoIndex,
}

/// The output's path of the master at `path`, and how it is made.
fn make(path: &str) -> (String, Make) {
    let video = crate::video::DIR;
    if path == format!("{video}/{}", opensacred_data::video::INDEX) {
        return (path.to_owned(), Make::VideoIndex);
    }
    match path
        .strip_prefix(&format!("{video}/"))
        .and_then(|name| name.strip_suffix(".mkv"))
    {
        Some(stem) => (format!("{video}/{stem}.webm"), Make::Video),
        None => (path.to_owned(), Make::Copy),
    }
}

/// The archive of the output at `path`: one by kind.
fn archive_of(path: &str) -> String {
    let top = path.split_once('/').map_or("", |(top, _)| top);
    let kind = match top {
        "heroes" | "netscript" | "tables" | "text" | "" => "data",
        "sound" if path.starts_with("sound/music/") => "music",
        other => other,
    };
    format!("{kind}.{}", archive::EXTENSION)
}

/// Whether the output at `path` is deflated in its archive: all but the formats that are
/// compressed already, stored so that they can be read in place.
fn deflated(path: &str) -> bool {
    let extension = path.rsplit_once('.').map_or("", |(_, e)| e);
    !matches!(
        extension,
        "png" | "glb" | "flac" | "mp3" | "ogg" | "opus" | "webm" | "ktx2" | "sector"
    )
}

/// What a build did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub masters: usize,
    pub built: usize,
    pub kept: usize,
    pub removed: usize,
    pub archives: usize,
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

/// An archive's entry to write: its master, its path and how it is made.
struct Planned<'a> {
    master: &'a FileHash,
    path: String,
    how: Make,
}

/// The output of `planned`, made from its master.
fn output(src: &Path, staging: &Path, planned: &Planned) -> Result<Vec<u8>> {
    let from = src.join(&planned.master.path);
    let what = || format!("building {} from {}", planned.path, planned.master.path);
    match planned.how {
        Make::Copy => fs::read(&from).with_context(what),
        Make::Video => {
            let target = staging.join(&planned.path);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("creating {}", parent.display()))?;
            }
            crate::video::encode(&from, &target).with_context(what)?;
            let data = fs::read(&target).with_context(what)?;
            fs::remove_file(&target).with_context(|| format!("removing {}", target.display()))?;
            Ok(data)
        }
        Make::VideoIndex => {
            let text = fs::read_to_string(&from).with_context(what)?;
            Ok(crate::video::built_index(&text)?.into_bytes())
        }
    }
}

/// An entry ready to be written: copied from the old archive with its recorded hash, or made.
enum Packed {
    Kept(Raw, FileHash),
    Made(Vec<u8>),
}

/// What the last build recorded.
struct Last<'a> {
    built_from: HashMap<&'a str, &'a FileHash>,
    written: HashMap<&'a str, &'a FileHash>,
}

/// Writes the archive `name` of `entries` into `out`, or keeps it; its entries' hashes and how many
/// were made.
fn pack(
    src: &Path,
    out: &Path,
    name: &str,
    entries: &[Planned],
    last: &Last,
    force: bool,
) -> Result<(Vec<FileHash>, usize)> {
    let target = out.join(name);
    let old = if target.exists() {
        Some(Archive::open(&target).with_context(|| format!("reading {}", target.display()))?)
    } else {
        None
    };
    // An entry's last output where its master is the one built last and the old archive holds it.
    let kept: Vec<Option<&FileHash>> = entries
        .iter()
        .map(|planned| {
            let recorded = last
                .written
                .get(format!("{name}/{}", planned.path).as_str())
                .copied()?;
            let same = last.built_from.get(planned.master.path.as_str()) == Some(&planned.master);
            let held = old.as_ref()?.size(&planned.path) == Some(recorded.size);
            (!force && same && held).then_some(recorded)
        })
        .collect();
    let old_count = old.as_ref().map_or(0, |a| a.names().count());
    if kept.iter().all(Option::is_some) && old_count == entries.len() {
        return Ok((kept.into_iter().flatten().cloned().collect(), 0));
    }
    let staging = out.join(STAGING);
    let temporary = out.join(format!("{name}.part"));
    let mut writer =
        Writer::create(&temporary).with_context(|| format!("creating {}", temporary.display()))?;
    let what = || format!("writing {}", temporary.display());
    let mut outputs = Vec::with_capacity(entries.len());
    let mut built = 0;
    let planned: Vec<(&Planned, Option<&FileHash>)> = entries.iter().zip(kept).collect();
    for batch in planned.chunks(BATCH) {
        let packed: Vec<Packed> = batch
            .par_iter()
            .map(|&(planned, kept)| match (kept, &old) {
                (Some(kept), Some(old)) => {
                    let raw = old
                        .raw(&planned.path)
                        .ok_or_else(|| anyhow!("{} lost {}", target.display(), planned.path))?
                        .with_context(|| format!("reading {}", target.display()))?;
                    Ok(Packed::Kept(raw, kept.clone()))
                }
                _ => output(src, &staging, planned).map(Packed::Made),
            })
            .collect::<Result<_>>()?;
        for ((planned, _), packed) in batch.iter().zip(packed) {
            match packed {
                Packed::Kept(raw, hash) => {
                    writer.add_raw(&planned.path, &raw).with_context(what)?;
                    outputs.push(hash);
                }
                Packed::Made(data) => {
                    writer
                        .add(&planned.path, &data, deflated(&planned.path))
                        .with_context(what)?;
                    outputs.push(FileHash {
                        path: format!("{name}/{}", planned.path),
                        size: u64::try_from(data.len())?,
                        sha256: hex(&Sha256::digest(&data)),
                    });
                    built += 1;
                }
            }
        }
    }
    writer.finish().with_context(what)?;
    fs::rename(&temporary, &target)
        .with_context(|| format!("renaming {} to {}", temporary.display(), target.display()))?;
    Ok((outputs, built))
}

/// Removes the file `path` under `out`, and the directories it leaves empty; whether it was there.
fn remove(out: &Path, path: &str) -> Result<bool> {
    let target = out.join(path);
    if !target.is_file() {
        return Ok(false);
    }
    fs::remove_file(&target).with_context(|| format!("removing {}", target.display()))?;
    let mut dir = target.parent();
    while let Some(parent) = dir.filter(|d| *d != out) {
        if fs::remove_dir(parent).is_err() {
            break;
        }
        dir = parent.parent();
    }
    Ok(true)
}

/// Removes the outputs that the extractions wrote into `out` before the two stages (#696), as
/// their records there name them, and the records: loose files that would take the archives'
/// place. The number of files removed.
fn remove_extracted(out: &Path) -> Result<usize> {
    let dir = out.join(manifest::DIR);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Ok(0);
    };
    let mut removed = 0;
    for entry in entries {
        let path = entry
            .with_context(|| format!("reading {}", dir.display()))?
            .path();
        let Some(command) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if command == NAME || path.extension().is_none_or(|e| e != "ron") {
            continue;
        }
        let Some(run) = manifest::read(out, command)? else {
            continue;
        };
        for output in &run.outputs {
            removed += usize::from(remove(out, &output.path)?);
        }
        manifest::forget(out, command)?;
    }
    Ok(removed)
}

/// Builds the game's assets in `out` from the masters in `src`; `force` builds every master.
pub fn build(src: &Path, out: &Path, force: bool) -> Result<Summary> {
    let masters = masters(src)?;
    let recorded = manifest::read(out, NAME)?;
    let last = Last {
        built_from: recorded
            .as_ref()
            .map(|run| run.sources.iter().map(|s| (s.path.as_str(), s)).collect())
            .unwrap_or_default(),
        written: recorded
            .as_ref()
            .map(|run| run.outputs.iter().map(|o| (o.path.as_str(), o)).collect())
            .unwrap_or_default(),
    };
    let mut archives: BTreeMap<String, Vec<Planned>> = BTreeMap::new();
    for master in &masters {
        let (path, how) = make(&master.path);
        archives
            .entry(archive_of(&path))
            .or_default()
            .push(Planned { master, path, how });
    }
    for entries in archives.values_mut() {
        entries.sort_by(|a, b| a.path.cmp(&b.path));
    }
    fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let packed: Vec<(Vec<FileHash>, usize)> = archives
        .par_iter()
        .map(|(name, entries)| pack(src, out, name, entries, &last, force))
        .collect::<Result<_>>()?;
    let staging = out.join(STAGING);
    if staging.exists() {
        fs::remove_dir_all(&staging).with_context(|| format!("removing {}", staging.display()))?;
    }
    // The archives no master makes any more, and the loose files of the builds before #699 and of
    // the extractions before #696.
    let mut removed = remove_extracted(out)?;
    let mut gone = HashSet::new();
    for path in last.written.keys() {
        match path.split_once('/') {
            Some((archive, _)) if archive.ends_with(&format!(".{}", archive::EXTENSION)) => {
                if !archives.contains_key(archive) && gone.insert(archive) {
                    removed += usize::from(remove(out, archive)?);
                }
            }
            _ => removed += usize::from(remove(out, path)?),
        }
    }
    let count = masters.len();
    let archive_count = archives.len();
    drop(archives);
    let built = packed.iter().map(|(_, built)| built).sum();
    let outputs: Vec<FileHash> = packed.into_iter().flat_map(|(files, _)| files).collect();
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
        archives: archive_count,
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

    fn read(out: &Path, archive: &str, path: &str) -> Option<String> {
        let archive = Archive::open(&out.join(archive)).ok()?;
        Some(String::from_utf8(archive.read(path)?.unwrap()).unwrap())
    }

    #[test]
    fn the_cutscenes_masters_are_built_into_webm() {
        assert_eq!(
            make("video/intro.mkv"),
            ("video/intro.webm".to_owned(), Make::Video)
        );
        assert_eq!(
            make("video/video.ron"),
            ("video/video.ron".to_owned(), Make::VideoIndex)
        );
        assert_eq!(
            make("tables/items.ron"),
            ("tables/items.ron".to_owned(), Make::Copy)
        );
    }

    #[test]
    fn the_outputs_go_into_an_archive_by_kind() {
        assert_eq!(archive_of("tables/items.ron"), "data.zip");
        assert_eq!(archive_of("netscript/StartCode.nsc"), "data.zip");
        assert_eq!(archive_of("sound/music/m01.flac"), "music.zip");
        assert_eq!(archive_of("sound/samples/00002.flac"), "sound.zip");
        assert_eq!(archive_of("textures/texture/00001-a.png"), "textures.zip");
        assert!(deflated("tables/items.ron"));
        assert!(!deflated("textures/texture/00001-a.png"));
        assert!(!deflated("world/sectors/1-2.sector"));
    }

    #[test]
    fn a_build_packs_the_masters_and_redoes_only_what_changed() {
        let src = scratch_dir("build-src");
        let out = scratch_dir("build-out");
        write(&src, "tables/items.ron", "[]");
        write(&src, "text/en-US/global.ftl", "a = b");
        write(&src, "sound/samples/00002.flac", "fLaC");
        write(&src, "manifest/tables.ron", "not a manifest");
        // A loose output of a build before the archives, and one of an extraction before the two
        // stages, with its record.
        write(&out, "tables/items.ron", "[]");
        write(&out, "text/en-US/global.ftl", "a = old");
        manifest::write(
            &out,
            &RunManifest {
                command: "text".to_owned(),
                converter: String::new(),
                sources: Vec::new(),
                outputs: vec![FileHash {
                    path: "text/en-US/global.ftl".to_owned(),
                    size: 7,
                    sha256: String::new(),
                }],
            },
        )
        .unwrap();
        manifest::write(
            &out,
            &RunManifest {
                command: NAME.to_owned(),
                converter: String::new(),
                sources: Vec::new(),
                outputs: vec![FileHash {
                    path: "tables/items.ron".to_owned(),
                    size: 2,
                    sha256: String::new(),
                }],
            },
        )
        .unwrap();
        let first = build(&src, &out, false).unwrap();
        assert_eq!(
            (first.masters, first.built, first.kept, first.archives),
            (3, 3, 0, 2)
        );
        assert_eq!(first.removed, 2);
        assert!(!out.join("tables").exists());
        assert!(!out.join("text").exists());
        assert!(manifest::read(&out, "text").unwrap().is_none());
        assert_eq!(read(&out, "data.zip", "tables/items.ron").unwrap(), "[]");
        assert_eq!(
            read(&out, "sound.zip", "sound/samples/00002.flac").unwrap(),
            "fLaC"
        );
        // Stage A's manifests are not masters.
        assert!(read(&out, "data.zip", "manifest/tables.ron").is_none());
        let data = fs::read(out.join("data.zip")).unwrap();
        // Nothing changed: nothing is built, and the archives stay as they were.
        let again = build(&src, &out, false).unwrap();
        assert_eq!((again.built, again.kept), (0, 3));
        assert_eq!(fs::read(out.join("data.zip")).unwrap(), data);
        // A changed master is built again, the others of its archive copied; a master gone takes
        // its archive with it.
        write(&src, "tables/items.ron", "[()]");
        fs::remove_file(src.join("sound/samples/00002.flac")).unwrap();
        let changed = build(&src, &out, false).unwrap();
        assert_eq!((changed.built, changed.kept, changed.removed), (1, 1, 1));
        assert_eq!(read(&out, "data.zip", "tables/items.ron").unwrap(), "[()]");
        assert_eq!(
            read(&out, "data.zip", "text/en-US/global.ftl").unwrap(),
            "a = b"
        );
        assert!(!out.join("sound.zip").exists());
        // `force` builds everything, into the same bytes.
        let data = fs::read(out.join("data.zip")).unwrap();
        assert_eq!(build(&src, &out, true).unwrap().built, 2);
        assert_eq!(fs::read(out.join("data.zip")).unwrap(), data);
        fs::remove_dir_all(src).unwrap();
        fs::remove_dir_all(out).unwrap();
    }
}
