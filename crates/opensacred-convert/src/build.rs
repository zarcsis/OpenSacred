//! `build`, the converter's stage B (PLAN.md ADR-05, #694): the game's assets in `<out>` from the
//! masters that the extraction commands (stage A) write into `<assets-src>`. It encodes the
//! textures' PNG masters into KTX2 ([`crate::ktx`], #700), with them the models' references to
//! them and the images they embed and the world index's, and the cutscenes' masters into AV1 and
//! Opus WebM (#698); it takes the other masters as they are, but writes the text files as UTF-8
//! with LF line ends ([`crate::utf8`], #835), while the later sub-issues change the formats one
//! kind at a time (#696), and packs the outputs into ZIP archives by kind,
//! [`opensacred_data::archive`]'s (#699): `data.zip` (the tables, texts, scripts and heroes),
//! `models.zip`, `music.zip`, `sound.zip`, `textures.zip`, `ui.zip`, `video.zip` and `world.zip`,
//! the entries in the order of their paths.
//!
//! It records `<out>/manifest/build.ron`: the masters it built from and each archive's entries,
//! `<archive>/<path>`, each with its SHA-256. An archive whose masters are all the ones recorded
//! and whose entries are in place is kept; one written again copies the entries whose masters did
//! not change from the old archive as they are, without making them again; the textures are all
//! made again when the world index changes, as it decides how they are encoded. The archives no
//! master makes any more and the loose files of the builds before #699 are removed.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use opensacred_data::archive::{self, Archive, Raw, Writer};
use opensacred_data::texture;
use rayon::prelude::*;
use sha2::{Digest, Sha256};

use crate::ktx::Use;
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
    /// A texture's PNG master, encoded into KTX2 for its use.
    Texture(Use),
    /// A model's glTF binary, its images the KTX2 textures.
    Model,
    /// The world index, naming the KTX2 textures.
    WorldIndex,
    /// A sample's or, if `true`, a music file's FLAC master, encoded into Ogg Opus.
    Sound(bool),
    /// A cutscene's FFV1 and FLAC master, encoded into AV1 and Opus WebM.
    Video,
    /// A script's Luau source, compiled into Luau's bytecode (#723).
    Luau,
}

impl Make {
    /// Every way.
    const ALL: [Self; 9] = [
        Self::Copy,
        Self::Texture(Use::Read),
        Self::Texture(Use::Sampled),
        Self::Model,
        Self::WorldIndex,
        Self::Sound(false),
        Self::Sound(true),
        Self::Video,
        Self::Luau,
    ];

    /// The way's name in the manifest and its version, raised when the converter makes its
    /// outputs otherwise, so that the next build makes them again; 0 for the ways as they were
    /// before the versions (#700), which a manifest without them records.
    fn way(self) -> (&'static str, u32) {
        match self {
            // 1: the text files UTF-8 without a BOM with LF line ends (#835).
            Self::Copy => ("copy", 1),
            // 2: the levels' alpha never lowered (#816).
            Self::Texture(_) => ("texture", 2),
            Self::Model => ("model", 1),
            Self::WorldIndex => ("world-index", 1),
            Self::Sound(_) => ("sound", 1),
            Self::Video => ("video", 0),
            Self::Luau => ("luau", 1),
        }
    }
}

/// The world index among the masters.
fn world_index() -> String {
    format!("world/{}", opensacred_data::world::INDEX)
}

/// The output's path of the master at `path`, and how it is made; `read` holds the textures that
/// the game reads on the CPU besides the cursor's.
fn make(path: &str, read: &HashSet<String>) -> (String, Make) {
    if path == world_index() {
        return (path.to_owned(), Make::WorldIndex);
    }
    if let Some(stem) = path.strip_suffix(".flac")
        && path.starts_with("sound/")
    {
        return (
            format!("{stem}.{}", opensacred_data::sound::EXTENSION),
            Make::Sound(path.starts_with("sound/music/")),
        );
    }
    if path.starts_with("models/") && path.ends_with(".glb") {
        return (path.to_owned(), Make::Model);
    }
    if let Some(stem) = path.strip_suffix(".luau")
        && path.starts_with("netscript/")
    {
        return (format!("{stem}.{}", crate::luau::BYTECODE), Make::Luau);
    }
    let sprite = path.starts_with(&format!("world/{}/", opensacred_data::world::SPRITES_DIR));
    if let Some(stem) = path.strip_suffix(".png")
        && (path.starts_with("textures/") || path.starts_with("ui/screens/") || sprite)
    {
        let cursor = path.starts_with("textures/texture/")
            && stem.ends_with(&format!("-{}", texture::CURSOR));
        // The sprites' images too, which the game packs into its pages (#838).
        let usage = if cursor || sprite || read.contains(path) {
            Use::Read
        } else {
            Use::Sampled
        };
        return (
            format!("{stem}.{}", texture::EXTENSION),
            Make::Texture(usage),
        );
    }
    let video = crate::video::DIR;
    match path
        .strip_prefix(&format!("{video}/"))
        .and_then(|name| name.strip_suffix(".mkv"))
    {
        Some(stem) => (
            format!(
                "{video}/{}",
                opensacred_data::video::file(stem, opensacred_data::video::EXTENSION)
            ),
            Make::Video,
        ),
        None => (path.to_owned(), Make::Copy),
    }
}

/// The textures that the world index among the masters in `src` names, which the game composes on
/// the CPU, and the atlases that the sprites' images were assembled from and that it does not name,
/// which `build` leaves out (#838); none without the index.
fn world_textures(src: &Path) -> Result<(HashSet<String>, HashSet<String>)> {
    let path = src.join(world_index());
    if !path.exists() {
        return Ok((HashSet::new(), HashSet::new()));
    }
    let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let (named, assembled) =
        crate::world::textures_of(&text).with_context(|| format!("reading {}", path.display()))?;
    Ok((named.into_iter().collect(), assembled.into_iter().collect()))
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
    /// The atlases that the sprites' images were assembled from, left out (#838).
    pub left_out: usize,
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
        Make::Copy => {
            let data = fs::read(&from).with_context(what)?;
            if crate::utf8::is_text(&planned.path) {
                crate::utf8::normalized(&data).with_context(what)
            } else {
                Ok(data)
            }
        }
        Make::Texture(usage) => {
            let png = fs::read(&from).with_context(what)?;
            crate::ktx::encode(&png, usage).with_context(what)
        }
        Make::Model => {
            let glb = fs::read(&from).with_context(what)?;
            crate::models::built(&glb).with_context(what)
        }
        Make::WorldIndex => {
            let text = fs::read_to_string(&from).with_context(what)?;
            Ok(crate::world::built_index(&text)
                .with_context(what)?
                .into_bytes())
        }
        Make::Sound(music) => {
            let flac = fs::read(&from).with_context(what)?;
            crate::sound::encode_opus(&flac, music, &planned.master.path).with_context(what)
        }
        Make::Luau => {
            let source = fs::read_to_string(&from).with_context(what)?;
            crate::luau::compile(&source).with_context(what)
        }
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
    }
}

/// An entry ready to be written: copied from the old archive with its recorded hash, or made.
enum Packed {
    Kept(Raw, FileHash),
    Made(Vec<u8>),
}

/// What the last build recorded, and whether the world index is the one it built from.
struct Last<'a> {
    built_from: HashMap<&'a str, &'a FileHash>,
    written: HashMap<&'a str, &'a FileHash>,
    ways: BTreeMap<String, u32>,
    same_world: bool,
}

impl Last<'_> {
    /// Whether the last build made `planned`'s output of its master as this one would.
    fn same(&self, planned: &Planned) -> bool {
        let (way, version) = planned.how.way();
        self.built_from.get(planned.master.path.as_str()) == Some(&planned.master)
            && self.ways.get(way).copied().unwrap_or(0) == version
            && (self.same_world || !matches!(planned.how, Make::Texture(_)))
    }
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
            let held = old.as_ref()?.size(&planned.path) == Some(recorded.size);
            (!force && last.same(planned) && held).then_some(recorded)
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
    let (read, assembled) = world_textures(src)?;
    let recorded = manifest::read(out, NAME)?;
    let built_from: HashMap<&str, &FileHash> = recorded
        .as_ref()
        .map(|run| run.sources.iter().map(|s| (s.path.as_str(), s)).collect())
        .unwrap_or_default();
    let world = world_index();
    let same_world =
        built_from.get(world.as_str()).copied() == masters.iter().find(|m| m.path == world);
    let last = Last {
        built_from,
        written: recorded
            .as_ref()
            .map(|run| run.outputs.iter().map(|o| (o.path.as_str(), o)).collect())
            .unwrap_or_default(),
        ways: recorded
            .as_ref()
            .map(|run| run.ways.clone())
            .unwrap_or_default(),
        same_world,
    };
    let mut archives: BTreeMap<String, Vec<Planned>> = BTreeMap::new();
    for master in masters.iter().filter(|m| !assembled.contains(&m.path)) {
        let (path, how) = make(&master.path, &read);
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
    let left_out = masters
        .iter()
        .filter(|m| assembled.contains(&m.path))
        .count();
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
            ways: Make::ALL
                .iter()
                .map(|how| {
                    let (way, version) = how.way();
                    (way.to_owned(), version)
                })
                .collect(),
        },
    )?;
    Ok(Summary {
        masters: count,
        built,
        kept: count - left_out - built,
        left_out,
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
    fn each_master_is_made_as_its_kind() {
        let read = HashSet::from(["textures/texture/00007-sheet.png".to_owned()]);
        let make = |path| make(path, &read);
        assert_eq!(
            make("video/intro.mkv"),
            ("video/intro.webm".to_owned(), Make::Video)
        );
        assert_eq!(
            make("tables/items.ron"),
            ("tables/items.ron".to_owned(), Make::Copy)
        );
        assert_eq!(
            make("world/world.ron"),
            ("world/world.ron".to_owned(), Make::WorldIndex)
        );
        assert_eq!(
            make("sound/sound.ron"),
            ("sound/sound.ron".to_owned(), Make::Copy)
        );
        assert_eq!(
            make("sound/samples/00002.flac"),
            ("sound/samples/00002.opus".to_owned(), Make::Sound(false))
        );
        assert_eq!(
            make("sound/music/MUSIC_WOOD01.flac"),
            (
                "sound/music/MUSIC_WOOD01.opus".to_owned(),
                Make::Sound(true)
            )
        );
        assert_eq!(
            make("models/models/0661-seraphim.glb"),
            ("models/models/0661-seraphim.glb".to_owned(), Make::Model)
        );
        assert_eq!(
            make("netscript/type_npc_seraphim/sectors/3250511.luau"),
            (
                "netscript/type_npc_seraphim/sectors/3250511.luauc".to_owned(),
                Make::Luau
            )
        );
        assert_eq!(
            make("netscript/type_npc_seraphim/index.ron"),
            (
                "netscript/type_npc_seraphim/index.ron".to_owned(),
                Make::Copy
            )
        );
        // The bytecode of a source, Luau's own; a source that does not compile fails.
        let bytecode = crate::luau::compile(
            "local F = {}
F[\"f\"] = function()
    return true
end
return F
",
        )
        .unwrap();
        assert!(
            bytecode
                .first()
                .is_some_and(|&version| (3..=11).contains(&version))
        );
        assert!(crate::luau::compile("F[").is_err());
        assert_eq!(
            make("textures/texture/00001-dwarf.png"),
            (
                "textures/texture/00001-dwarf.ktx2".to_owned(),
                Make::Texture(Use::Sampled)
            )
        );
        assert_eq!(
            make("ui/screens/loading0.png"),
            (
                "ui/screens/loading0.ktx2".to_owned(),
                Make::Texture(Use::Sampled)
            )
        );
        // The world's sheets and atlases, and the cursor, which the game reads on the CPU.
        assert_eq!(
            make("textures/texture/00007-sheet.png").1,
            Make::Texture(Use::Read)
        );
        assert_eq!(
            make("textures/texture/14791-mouse_main.png").1,
            Make::Texture(Use::Read)
        );
        // The sprites' images, which the game packs on the CPU (#838).
        assert_eq!(
            make("world/sprites/00001.png"),
            (
                "world/sprites/00001.ktx2".to_owned(),
                Make::Texture(Use::Read)
            )
        );
        assert_eq!(archive_of("world/sprites/00001.ktx2"), "world.zip");
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

    /// A text master that a tool saved with a BOM and CRLF is written with LF alone, a binary one
    /// as it is, and one that is not UTF-8 stops the build (#835).
    #[test]
    fn the_text_outputs_are_utf8_with_lf() {
        let src = scratch_dir("build-text-src");
        let out = scratch_dir("build-text-out");
        write(&src, "tables/items.ron", "\u{FEFF}[\r\n    (),\r\n]\r\n");
        write(&src, "ui/texts/credits.txt", "a\r\nb\r\n");
        write(&src, "ui/fonts/antqs__.ttf", "font\r\n");
        build(&src, &out, false).unwrap();
        assert_eq!(
            read(&out, "data.zip", "tables/items.ron").unwrap(),
            "[\n    (),\n]\n"
        );
        assert_eq!(
            read(&out, "ui.zip", "ui/texts/credits.txt").unwrap(),
            "a\nb\n"
        );
        assert_eq!(
            read(&out, "ui.zip", "ui/fonts/antqs__.ttf").unwrap(),
            "font\r\n"
        );
        fs::write(src.join("tables/items.ron"), b"\xE9").unwrap();
        let error = build(&src, &out, false).unwrap_err();
        assert!(
            format!("{error:#}").contains("tables/items.ron"),
            "{error:#}"
        );
        fs::remove_dir_all(src).unwrap();
        fs::remove_dir_all(out).unwrap();
    }

    #[test]
    fn a_build_packs_the_masters_and_redoes_only_what_changed() {
        let src = scratch_dir("build-src");
        let out = scratch_dir("build-out");
        write(&src, "tables/items.ron", "[]");
        write(&src, "text/en-US/global.ftl", "a = b");
        write(&src, "ui/fonts/antqs__.ttf", "font");
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
                ways: BTreeMap::new(),
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
                ways: BTreeMap::new(),
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
            read(&out, "ui.zip", "ui/fonts/antqs__.ttf").unwrap(),
            "font"
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
        fs::remove_file(src.join("ui/fonts/antqs__.ttf")).unwrap();
        let changed = build(&src, &out, false).unwrap();
        assert_eq!((changed.built, changed.kept, changed.removed), (1, 1, 1));
        assert_eq!(read(&out, "data.zip", "tables/items.ron").unwrap(), "[()]");
        assert_eq!(
            read(&out, "data.zip", "text/en-US/global.ftl").unwrap(),
            "a = b"
        );
        assert!(!out.join("ui.zip").exists());
        // `force` builds everything, into the same bytes.
        let data = fs::read(out.join("data.zip")).unwrap();
        assert_eq!(build(&src, &out, true).unwrap().built, 2);
        assert_eq!(fs::read(out.join("data.zip")).unwrap(), data);
        fs::remove_dir_all(src).unwrap();
        fs::remove_dir_all(out).unwrap();
    }

    #[test]
    fn the_textures_are_encoded_for_their_use_and_again_when_the_world_changes() {
        let src = scratch_dir("build-textures-src");
        let out = scratch_dir("build-textures-out");
        let png = crate::textures::encode_png(8, 8, &[255; 256]).unwrap();
        for name in ["00001-sheet", "00002-dwarf"] {
            let path = src.join(format!("textures/texture/{name}.png"));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, &png).unwrap();
        }
        let world = |sheets: &[&str]| {
            let world = opensacred_data::world::World {
                sectors: Vec::new(),
                sheets: sheets
                    .iter()
                    .map(|s| format!("../textures/texture/{s}.png"))
                    .collect(),
                atlases: Vec::new(),
                assembled_from: Vec::new(),
            };
            let text = ron::ser::to_string_pretty(&world, crate::utf8::pretty());
            write(&src, "world/world.ron", &text.unwrap());
        };
        let format = |name: &str| {
            let archive = Archive::open(&out.join("textures.zip")).unwrap();
            let file = archive
                .read(&format!("textures/texture/{name}.ktx2"))
                .unwrap()
                .unwrap();
            let header = ktx2::Reader::new(&file[..]).unwrap().header();
            (header.format.unwrap(), header.level_count)
        };
        world(&["00001-sheet"]);
        write(&src, "tables/items.ron", "[]");
        assert_eq!(build(&src, &out, false).unwrap().built, 4);
        // The world's sheet as it is, for the CPU; the other in BC7 with its mips.
        assert_eq!(format("00001-sheet"), (ktx2::Format::R8G8B8A8_SRGB, 1));
        assert_eq!(format("00002-dwarf"), (ktx2::Format::BC7_SRGB_BLOCK, 4));
        assert!(
            read(&out, "world.zip", "world/world.ron")
                .unwrap()
                .contains("00001-sheet.ktx2")
        );
        assert_eq!(build(&src, &out, false).unwrap().built, 0);
        // A world that names the other: every texture made again, the table kept.
        world(&["00002-dwarf"]);
        assert_eq!(build(&src, &out, false).unwrap().built, 3);
        assert_eq!(format("00001-sheet"), (ktx2::Format::BC7_SRGB_BLOCK, 4));
        assert_eq!(format("00002-dwarf"), (ktx2::Format::R8G8B8A8_SRGB, 1));
        // The manifest of a build before the ways' versions: the ways of a version made again, the
        // copy's among them since #835.
        let mut run = manifest::read(&out, NAME).unwrap().unwrap();
        assert_eq!(run.ways.get("texture"), Some(&2));
        assert_eq!(run.ways.get("copy"), Some(&1));
        run.ways.clear();
        manifest::write(&out, &run).unwrap();
        assert_eq!(build(&src, &out, false).unwrap().built, 4);
        fs::remove_dir_all(src).unwrap();
        fs::remove_dir_all(out).unwrap();
    }

    /// The sprites' images go to `world.zip` for the CPU, and the atlases they were assembled
    /// from leave the game's assets but those that a miniature cuts its square from (#838).
    #[test]
    fn the_sprites_are_read_on_the_cpu_and_their_atlases_left_out() {
        let src = scratch_dir("build-sprites-src");
        let out = scratch_dir("build-sprites-out");
        let png = crate::textures::encode_png(8, 8, &[255; 256]).unwrap();
        for path in [
            "textures/texture/00001-mix0.444.png",
            "textures/texture/00002-mini.png",
            "world/sprites/00001.png",
        ] {
            let path = src.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, &png).unwrap();
        }
        let world = opensacred_data::world::World {
            sectors: Vec::new(),
            sheets: Vec::new(),
            atlases: vec!["../textures/texture/00002-mini.png".to_owned()],
            assembled_from: vec![
                "../textures/texture/00001-mix0.444.png".to_owned(),
                "../textures/texture/00002-mini.png".to_owned(),
            ],
        };
        let text = ron::ser::to_string_pretty(&world, crate::utf8::pretty()).unwrap();
        write(&src, "world/world.ron", &text);
        let summary = build(&src, &out, false).unwrap();
        assert_eq!(
            (
                summary.masters,
                summary.built,
                summary.kept,
                summary.left_out
            ),
            (4, 3, 0, 1)
        );
        let textures = Archive::open(&out.join("textures.zip")).unwrap();
        assert!(
            textures
                .read("textures/texture/00001-mix0.444.ktx2")
                .is_none()
        );
        assert!(textures.read("textures/texture/00002-mini.ktx2").is_some());
        let world = Archive::open(&out.join("world.zip")).unwrap();
        let file = world.read("world/sprites/00001.ktx2").unwrap().unwrap();
        let header = ktx2::Reader::new(&file[..]).unwrap().header();
        assert_eq!(
            (header.format.unwrap(), header.level_count),
            (ktx2::Format::R8G8B8A8_SRGB, 1)
        );
        let index = String::from_utf8(world.read("world/world.ron").unwrap().unwrap()).unwrap();
        assert!(!index.contains("assembled_from"), "{index}");
        assert_eq!(build(&src, &out, false).unwrap().kept, 3);
        fs::remove_dir_all(src).unwrap();
        fs::remove_dir_all(out).unwrap();
    }
}
