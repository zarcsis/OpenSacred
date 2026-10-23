//! The game's files (#699; PLAN.md ADR-05): the converter's `assets/` in layers. A path takes the
//! first layer that has it: the loose directory itself (debugging, HD packs and mods in the
//! making), then the overlay archives in its `overlays/` (HD packs, mods), then the base archives
//! beside the loose files, each set in the order of the archives' names. The archives are ZIP
//! files whose paths are the loose files' ([`opensacred_data::archive`]).
//!
//! Both readers go through the layers: Bevy's asset server through [`AssetFiles`], the default
//! asset source, and the game's own reads of tables, sectors and atlases through [`read`],
//! [`read_to_string`], [`open`], [`exists`] and [`read_dir`], which fall back on the file system
//! for a path outside the assets or before [`init`].

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use bevy::asset::io::{
    AssetReader, AssetReaderError, PathStream, Reader, VecReader, file::FileAssetReader,
};
use bevy::tasks::futures_lite::stream;
use opensacred_data::archive::{self, Archive, OVERLAYS};

/// The archives' name of the path `relative` to the assets: its parts between `/`, `.` and `..`
/// resolved (the world names its atlases `world/../textures/…`); none for a path outside them.
fn name_of(relative: &Path) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for part in relative.components() {
        match part {
            Component::Normal(part) => parts.push(part.to_str()?),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(parts.join("/"))
}

fn invalid(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what.to_owned())
}

/// The assets' layers: the loose directory, then the overlay and the base archives.
#[derive(Debug)]
pub struct Layers {
    root: PathBuf,
    archives: Vec<Archive>,
}

/// The ZIP archives directly in `dir`, in the order of their names.
fn archives_in(dir: &Path) -> io::Result<Vec<Archive>> {
    let mut paths: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && p.extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case(archive::EXTENSION))
            })
            .collect(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };
    paths.sort();
    paths.iter().map(|p| Archive::open(p)).collect()
}

impl Layers {
    /// The layers of the assets at `root`.
    pub fn open(root: &Path) -> io::Result<Self> {
        let mut archives = archives_in(&root.join(OVERLAYS))?;
        archives.extend(archives_in(root)?);
        Ok(Self {
            root: root.to_owned(),
            archives,
        })
    }

    /// The archives' count.
    pub fn archives(&self) -> usize {
        self.archives.len()
    }

    /// `path` relative to the assets, as the archives name it, if it is in them.
    fn relative(&self, path: &Path) -> Option<String> {
        name_of(path.strip_prefix(&self.root).unwrap_or(path))
    }

    /// The data of `name`, relative to the assets, from the first archive that has it.
    fn archived(&self, name: &str) -> Option<io::Result<Vec<u8>>> {
        self.archives.iter().find_map(|archive| archive.read(name))
    }

    /// The data of `path`: the loose file's, else an archive's.
    pub fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        if path.is_file() {
            return fs::read(path);
        }
        self.relative(path)
            .and_then(|name| self.archived(&name))
            .unwrap_or_else(|| Err(not_found(path)))
    }

    /// Whether `path` is a loose file or an archive's entry.
    pub fn exists(&self, path: &Path) -> bool {
        path.exists()
            || self
                .relative(path)
                .is_some_and(|name| self.archives.iter().any(|a| a.contains(&name)))
    }

    /// The names directly in the directory `dir`, loose or archived, sorted.
    fn names_in(&self, dir: &Path) -> io::Result<BTreeSet<String>> {
        let mut names = BTreeSet::new();
        let mut found = false;
        if let Ok(entries) = fs::read_dir(dir) {
            found = true;
            for entry in entries.filter_map(Result::ok) {
                names.insert(entry.file_name().to_string_lossy().into_owned());
            }
        }
        if let Some(prefix) = self.relative(dir) {
            let prefix = if prefix.is_empty() {
                prefix
            } else {
                format!("{}/", prefix.trim_end_matches('/'))
            };
            for archive in &self.archives {
                for name in archive.names() {
                    if let Some(rest) = name.strip_prefix(&prefix) {
                        found = true;
                        let first = rest.split('/').next().unwrap_or(rest);
                        names.insert(first.to_owned());
                    }
                }
            }
        }
        if found {
            Ok(names)
        } else {
            Err(not_found(dir))
        }
    }
}

fn not_found(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("{}: no such file in the assets", path.display()),
    )
}

static LAYERS: OnceLock<Layers> = OnceLock::new();

/// Opens the assets' layers at `root`, once; the number of archives.
pub fn init(root: &Path) -> io::Result<usize> {
    let layers = Layers::open(root)?;
    let count = layers.archives();
    // A second call keeps the first layers.
    let _ = LAYERS.set(layers);
    Ok(count)
}

fn layers() -> Option<&'static Layers> {
    LAYERS.get()
}

/// The data of the asset file at `path`.
pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    match layers() {
        Some(layers) => layers.read(path.as_ref()),
        None => fs::read(path),
    }
}

/// The text of the asset file at `path`.
pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    String::from_utf8(read(path)?).map_err(|_| invalid("an asset file not in UTF-8"))
}

/// Whether the asset file at `path` exists, loose or archived.
pub fn exists(path: impl AsRef<Path>) -> bool {
    match layers() {
        Some(layers) => layers.exists(path.as_ref()),
        None => path.as_ref().exists(),
    }
}

/// The paths directly in the asset directory `dir`, loose or archived, sorted.
pub fn read_dir(dir: impl AsRef<Path>) -> io::Result<Vec<PathBuf>> {
    let dir = dir.as_ref();
    let names = match layers() {
        Some(layers) => layers.names_in(dir)?,
        None => fs::read_dir(dir)?
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect(),
    };
    Ok(names.into_iter().map(|name| dir.join(name)).collect())
}

/// An asset file opened for reading: the loose file itself, or an archived one in memory.
pub enum Opened {
    File(BufReader<File>),
    Memory(Cursor<Vec<u8>>),
}

impl Read for Opened {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Opened::File(file) => file.read(buf),
            Opened::Memory(memory) => memory.read(buf),
        }
    }
}

impl BufRead for Opened {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        match self {
            Opened::File(file) => file.fill_buf(),
            Opened::Memory(memory) => memory.fill_buf(),
        }
    }

    fn consume(&mut self, amount: usize) {
        match self {
            Opened::File(file) => file.consume(amount),
            Opened::Memory(memory) => memory.consume(amount),
        }
    }
}

impl Seek for Opened {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        match self {
            Opened::File(file) => file.seek(to),
            Opened::Memory(memory) => memory.seek(to),
        }
    }
}

/// Opens the asset file at `path` for reading, buffered.
pub fn open(path: impl AsRef<Path>) -> io::Result<Opened> {
    let path = path.as_ref();
    if path.is_file() || layers().is_none() {
        return Ok(Opened::File(BufReader::new(File::open(path)?)));
    }
    read(path).map(|data| Opened::Memory(Cursor::new(data)))
}

/// The default asset source of Bevy's asset server over the layers: a loose file first, through
/// Bevy's own file reader, then the archives.
pub struct AssetFiles {
    loose: FileAssetReader,
}

impl AssetFiles {
    /// The asset files under `root`, which [`init`] opened.
    pub fn new(root: &Path) -> Self {
        Self {
            loose: FileAssetReader::new(root),
        }
    }
}

/// An archived asset's data, by its path relative to the assets.
fn archived(path: &Path) -> Option<Vec<u8>> {
    layers()?.archived(&name_of(path)?)?.ok()
}

impl AssetReader for AssetFiles {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<Box<dyn Reader + 'a>, AssetReaderError> {
        match self.loose.read(path).await {
            Ok(reader) => Ok(Box::new(reader)),
            Err(AssetReaderError::NotFound(missing)) => match archived(path) {
                Some(data) => Ok(Box::new(VecReader::new(data))),
                None => Err(AssetReaderError::NotFound(missing)),
            },
            Err(e) => Err(e),
        }
    }

    async fn read_meta<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<dyn Reader + 'a>, AssetReaderError> {
        let reader = self.loose.read_meta(path).await?;
        Ok(Box::new(reader))
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        match self.loose.read_directory(path).await {
            Ok(entries) => Ok(entries),
            Err(AssetReaderError::NotFound(missing)) => {
                let names = layers()
                    .and_then(|layers| layers.names_in(&layers.root.join(path)).ok())
                    .ok_or(AssetReaderError::NotFound(missing))?;
                let paths: Vec<PathBuf> = names.into_iter().map(|n| path.join(n)).collect();
                Ok(Box::new(stream::iter(paths)))
            }
            Err(e) => Err(e),
        }
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        if self.loose.is_directory(path).await.unwrap_or(false) {
            return Ok(true);
        }
        Ok(layers().is_some_and(|layers| {
            let Some(name) = name_of(path) else {
                return false;
            };
            let name = format!("{name}/");
            layers
                .archives
                .iter()
                .any(|a| a.names().any(|k| k.starts_with(&name)))
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ZIP of `files`, the even ones stored and the odd ones deflated.
    fn zip(path: &Path, files: &[(&str, &[u8])]) {
        let mut writer = archive::Writer::create(path).unwrap();
        for (index, (name, data)) in files.iter().enumerate() {
            writer.add(name, data, index % 2 == 1).unwrap();
        }
        writer.finish().unwrap();
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("opensacred-files-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_path_takes_the_first_layer_that_has_it() {
        let root = scratch("layers");
        let text = b"( voices: [] )".repeat(20);
        zip(
            &root.join("data.zip"),
            &[
                ("tables/items.ron", b"[]"),
                ("tables/voices.ron", &text),
                ("tables/balance.ron", b"base"),
            ],
        );
        fs::create_dir_all(root.join(OVERLAYS)).unwrap();
        zip(
            &root.join(OVERLAYS).join("mod.zip"),
            &[("tables/balance.ron", b"overlay")],
        );
        fs::create_dir_all(root.join("tables")).unwrap();
        fs::write(root.join("tables/items.ron"), "loose").unwrap();
        let layers = Layers::open(&root).unwrap();
        assert_eq!(layers.archives(), 2);
        // The loose file before the archives, an overlay before the base, a deflated entry.
        assert_eq!(
            layers.read(&root.join("tables/items.ron")).unwrap(),
            b"loose"
        );
        assert_eq!(
            layers.read(&root.join("tables/balance.ron")).unwrap(),
            b"overlay"
        );
        assert_eq!(layers.read(&root.join("tables/voices.ron")).unwrap(), text);
        assert!(layers.exists(&root.join("tables/voices.ron")));
        // A path through `..`, as the world names its atlases.
        assert_eq!(
            layers
                .read(&root.join("world/../tables/voices.ron"))
                .unwrap(),
            text
        );
        assert_eq!(name_of(Path::new("../tables")), None);
        assert!(!layers.exists(&root.join("tables/none.ron")));
        assert!(layers.read(&root.join("tables/none.ron")).is_err());
        let names: Vec<String> = layers
            .names_in(&root.join("tables"))
            .unwrap()
            .into_iter()
            .collect();
        assert_eq!(names, ["balance.ron", "items.ron", "voices.ron"]);
        fs::remove_dir_all(root).unwrap();
    }
}
